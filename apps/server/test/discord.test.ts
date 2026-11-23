import assert from "node:assert/strict";
import { generateKeyPairSync, sign } from "node:crypto";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { buildApp } from "../src/app.js";
import { loadConfig } from "../src/config.js";
import { DiscordVoice } from "../src/discord-voice.js";

const application = "111111111111111111";
const guild = "222222222222222222";
const actor = "333333333333333333";
const keys = generateKeyPairSync("ed25519");
const publicKey = keys.publicKey
  .export({ format: "der", type: "spki" })
  .subarray(-32)
  .toString("hex");
const env = {
  ADMIN_TOKEN: "test-only-admin-secret-00000000000000",
  LIVEKIT_API_KEY: "test-api-key",
  LIVEKIT_API_SECRET: "test-only-media-secret-00000000000000",
  PUBLIC_URL: "https://example.test",
  DISCORD_APPLICATION_ID: application,
  DISCORD_PUBLIC_KEY: publicKey,
  DISCORD_GUILD_IDS: guild,
};

test("Discord configuration fails closed", () => {
  for (const invalid of [
    { DISCORD_PUBLIC_KEY: "wrong" },
    { DISCORD_GUILD_IDS: "" },
    { DISCORD_APPLICATION_ID: "wrong" },
    { DISCORD_GUILD_IDS: "*,222222222222222222" },
  ])
    assert.throws(() => loadConfig({ ...env, ...invalid }), /DISCORD/);
  assert.deepEqual(
    loadConfig({ ...env, DISCORD_GUILD_IDS: "*" }).discord?.guildIds,
    ["*"],
  );
  assert.equal(
    loadConfig({
      ...env,
      DISCORD_APPLICATION_ID: "",
      DISCORD_PUBLIC_KEY: "",
      DISCORD_GUILD_IDS: "",
    }).discord,
    undefined,
  );
});

test("admin Discord installation link is based on application configuration", async (t) => {
  const directory = mkdtempSync(join(tmpdir(), "podetelar-discord-install-"));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  for (const guildIds of [undefined, "*", guild]) {
    const config = loadConfig({
      ...env,
      DATABASE_PATH: join(directory, "rooms.db"),
      DISCORD_APPLICATION_ID: guildIds ? application : "",
      DISCORD_PUBLIC_KEY: guildIds ? publicKey : "",
      DISCORD_GUILD_IDS: guildIds,
    });
    const app = await buildApp(config, { usagePolling: false });
    try {
      const url = "/api/admin/discord";
      assert.equal((await app.inject({ url })).statusCode, 401);
      const login = await app.inject({
        method: "POST",
        url: "/api/admin/login",
        headers: { origin: env.PUBLIC_URL },
        payload: { password: env.ADMIN_TOKEN },
      });
      const cookie = String(login.headers["set-cookie"]).split(";")[0]!;
      const response = await app.inject({ url, headers: { cookie } });
      assert.equal(response.statusCode, 200);
      const { installation, online } = response.json();
      assert.equal(online, false);
      if (!guildIds) {
        assert.equal(installation, null);
        continue;
      }
      assert.equal(installation.guildsRestricted, guildIds !== "*");
      const installUrl = new URL(installation.url);
      assert.equal(
        installUrl.origin + installUrl.pathname,
        "https://discord.com/oauth2/authorize",
      );
      assert.deepEqual(Object.fromEntries(installUrl.searchParams), {
        client_id: application,
        integration_type: "0",
        scope: "bot applications.commands",
        permissions: "3072",
      });
    } finally {
      await app.close();
    }
  }
});

test("signed Discord interactions create independent, admitted rooms without browser Origin bypass", async (t) => {
  const directory = mkdtempSync(join(tmpdir(), "podetelar-discord-"));
  const config = loadConfig({
    ...env,
    DATABASE_PATH: join(directory, "rooms.db"),
  });
  config.webRoot = join(directory, "no-web");
  let now = Date.now();
  let app = await buildApp(config, { now: () => now, usagePolling: false });
  t.after(async () => {
    await app.close();
  });
  let nextId = 444444444444444444n;
  const command = (extra = {}) => ({
    id: String(nextId++),
    application_id: application,
    type: 2,
    guild_id: guild,
    member: { user: { id: actor } },
    data: { type: 1, name: "stream" },
    ...extra,
  });
  const send = (
    input: unknown,
    time = Math.floor(now / 1000).toString(),
    suffix = "",
  ) => {
    const payload = JSON.stringify(input);
    const signature = sign(
      null,
      Buffer.from(time + payload),
      keys.privateKey,
    ).toString("hex");
    return app.inject({
      method: "POST",
      url: "/api/discord/interactions",
      headers: {
        "content-type": "application/json",
        "x-signature-ed25519": signature,
        "x-signature-timestamp": time,
      },
      payload: payload + suffix,
    });
  };
  assert.equal(
    (
      await app.inject({
        method: "POST",
        url: "/api/discord/interactions",
        payload: {},
      })
    ).statusCode,
    401,
  );
  assert.equal((await send(command(), undefined, " ")).statusCode, 401);
  assert.equal(
    (await send(command(), String(Math.floor(now / 1000) - 301))).statusCode,
    401,
  );
  assert.deepEqual((await send(command({ type: 1, data: undefined }))).json(), {
    type: 1,
  });
  for (const invalid of [
    { application_id: actor },
    { data: { type: 1, name: "unknown" } },
    { data: { type: 1, name: "stream", options: [1] } },
  ])
    assert.equal((await send(command(invalid))).statusCode, 400);
  for (const invalid of [
    { guild_id: actor },
    { guild_id: undefined, member: undefined },
  ]) {
    const denied = (await send(command(invalid))).json();
    assert.equal(denied.data.flags, 64);
    assert.ok(!denied.data.content.includes("#invite="));
  }
  const input = command();
  const response = await send(input);
  assert.equal(response.statusCode, 200);
  const data = response.json().data;
  assert.equal(data.flags, 4);
  assert.deepEqual(data.allowed_mentions, { parse: [] });
  const url = new URL(data.content.match(/<(https:[^>]+)>/)[1]);
  assert.equal(url.search, "");
  const roomId = url.pathname.split("/").pop();
  assert.equal(
    (await app.inject(`/api/rooms/${roomId}/session`)).statusCode,
    401,
  );
  const joinRoom = (invitation: string) =>
    app.inject({
      method: "POST",
      url: `/api/rooms/${roomId}/join`,
      headers: { origin: config.publicUrl },
      payload: { invitation, displayName: "Friend" },
    });
  assert.equal((await joinRoom("x".repeat(43))).statusCode, 404);
  assert.equal((await joinRoom(url.hash.slice(8))).statusCode, 200);
  assert.equal((await send(input)).json().data.flags, 64);
  await app.close();
  app = await buildApp(config, { now: () => now, usagePolling: false });
  assert.equal((await send(input)).json().data.flags, 64); // Durable replay protection.
  const second = (
    await send(command({ data: { type: 1, name: "telar" } }))
  ).json().data.content;
  assert.ok(!second.includes(roomId));
  for (let i = 0; i < 3; i++)
    assert.equal((await send(command())).json().data.flags, 4);
  assert.equal((await send(command())).json().data.flags, 64);
  now += 61_000;
  assert.equal((await send(command())).json().data.flags, 4);
  assert.equal(
    (
      await app.inject({
        method: "POST",
        url: "/api/rooms",
        payload: { name: "Friends", displayName: "Friend" },
      })
    ).statusCode,
    403,
  );
  assert.equal(
    (
      await app.inject({
        method: "POST",
        url: "/api/discord/interactions?bypass=1",
        payload: {},
      })
    ).statusCode,
    403,
  );
  const address = await app.listen({ host: "127.0.0.1", port: 0 });
  const payload = JSON.stringify(command());
  const timestamp = String(Math.floor(now / 1000));
  const signature = sign(
    null,
    Buffer.from(timestamp + payload),
    keys.privateKey,
  ).toString("hex");
  const http = await fetch(`${address}/api/discord/interactions`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "x-signature-ed25519": signature,
      "x-signature-timestamp": timestamp,
    },
    body: payload,
  });
  assert.equal(http.status, 200);
  assert.equal(
    ((await http.json()) as { data: { flags: number } }).data.flags,
    4,
  );
});

test("all-server mode requires the signed guild-installation owner and still rejects DMs", async (t) => {
  const directory = mkdtempSync(join(tmpdir(), "podetelar-global-discord-"));
  const config = loadConfig({
    ...env,
    DISCORD_GUILD_IDS: "*",
    DATABASE_PATH: join(directory, "rooms.db"),
  });
  config.webRoot = join(directory, "no-web");
  const voice = new DiscordVoice(undefined);
  const app = await buildApp(config, {
    usagePolling: false,
    discordVoice: voice,
  });
  t.after(async () => {
    await app.close();
  });
  let id = 555555555555555555n;
  const command = (extra = {}) => ({
    id: String(id++),
    application_id: application,
    type: 2,
    context: 0,
    guild_id: guild,
    member: { user: { id: actor } },
    authorizing_integration_owners: { "0": guild },
    data: { type: 1, name: "stream" },
    ...extra,
  });
  const send = async (input: unknown) => {
    const payload = JSON.stringify(input);
    const timestamp = String(Math.floor(Date.now() / 1000));
    const signature = sign(
      null,
      Buffer.from(timestamp + payload),
      keys.privateKey,
    ).toString("hex");
    return app.inject({
      method: "POST",
      url: "/api/discord/interactions",
      headers: {
        "content-type": "application/json",
        "x-signature-ed25519": signature,
        "x-signature-timestamp": timestamp,
      },
      payload,
    });
  };
  for (const invalid of [
    { authorizing_integration_owners: undefined },
    { authorizing_integration_owners: { "1": actor } },
    { authorizing_integration_owners: { "0": actor } },
    { guild_id: undefined, member: undefined, context: 1 },
    { context: 2 },
  ]) {
    const body = (await send(command(invalid))).json();
    assert.equal(body.data.flags, 64);
    assert.ok(!body.data.content.includes("#invite="));
  }
  const first = (await send(command())).json();
  assert.equal(first.data.flags, 4);
  const second = (
    await send(
      command({
        guild_id: actor,
        authorizing_integration_owners: { "0": actor },
      }),
    )
  ).json();
  assert.equal(second.data.flags, 4);
  assert.notEqual(first.data.content, second.data.content);

  const privateVoice = "666666666666666666";
  voice.index.apply("GUILD_CREATE", {
    id: guild,
    channels: [{ id: privateVoice, name: "Private voice", type: 2 }],
    voice_states: [{ user_id: actor, channel_id: privateVoice }],
  });
  voice.index.connected();
  const privateRoom = (
    await send(command({ channel_id: "777777777777777777" }))
  ).json();
  assert.equal(privateRoom.data.flags, 64);
  assert.match(
    privateRoom.data.content,
    new RegExp(`/c/${guild}/${privateVoice}#invite=`),
  );
});

test("signed HTTP commands name rooms after the invoking person's visible Discord name", async (t) => {
  const directory = mkdtempSync(join(tmpdir(), "podetelar-discord-names-"));
  const config = loadConfig({
    ...env,
    DATABASE_PATH: join(directory, "rooms.db"),
  });
  config.webRoot = join(directory, "no-web");
  let now = Date.now();
  let nextId = 555555555555555555n;
  for (const [member, expected] of [
    [
      {
        nick: "Felipe",
        user: { id: actor, username: "felipe123", global_name: "Felipão" },
      },
      "Felipe's room",
    ],
    [
      {
        nick: null,
        user: { id: actor, username: "felipe123", global_name: "Felipão" },
      },
      "Felipão's room",
    ],
    [
      {
        nick: "  ",
        user: { id: actor, username: "felipe123", global_name: null },
      },
      "felipe123's room",
    ],
    [{ user: { id: actor } }, `${actor}'s room`],
    [
      { nick: "😀".repeat(27), user: { id: actor } },
      `${"😀".repeat(26)}'s room`,
    ],
    [
      { nick: "\u202eFelipe\n", user: { id: actor, username: "fallback" } },
      "Felipe's room",
    ],
    [
      { nick: "F".repeat(100), user: { id: actor } },
      `${"F".repeat(53)}'s room`,
    ],
  ] as const) {
    now += 61_000;
    config.databasePath = join(directory, `${nextId}.db`);
    const app = await buildApp(config, { now: () => now, usagePolling: false });
    t.after(() => app.close());
    const address = await app.listen({ host: "127.0.0.1", port: 0 });
    for (const name of ["stream", "telar"]) {
      const input = {
        id: String(nextId++),
        application_id: application,
        type: 2,
        guild_id: guild,
        member,
        data: { type: 1, name },
      };
      const payload = JSON.stringify(input);
      const timestamp = String(Math.floor(now / 1000));
      const signature = sign(
        null,
        Buffer.from(timestamp + payload),
        keys.privateKey,
      ).toString("hex");
      const response = await fetch(`${address}/api/discord/interactions`, {
        method: "POST",
        headers: {
          "content-type": "application/json",
          "x-signature-ed25519": signature,
          "x-signature-timestamp": timestamp,
        },
        body: payload,
      });
      assert.equal(response.status, 200);
      const body = (await response.json()) as {
        data: { content: string; allowed_mentions: unknown };
      };
      assert.deepEqual(body.data.allowed_mentions, { parse: [] });
      const url = new URL(body.data.content.match(/<(https:[^>]+)>/)![1]!);
      const roomId = url.pathname.split("/").pop();
      const joined = await app.inject({
        method: "POST",
        url: `/api/rooms/${roomId}/join`,
        headers: { origin: config.publicUrl },
        payload: { invitation: url.hash.slice(8), displayName: "Viewer" },
      });
      assert.equal(joined.statusCode, 200);
      assert.equal(joined.json().room.name, expected);
    }
  }
});
