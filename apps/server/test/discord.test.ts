import assert from "node:assert/strict";
import { generateKeyPairSync, sign } from "node:crypto";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { buildApp } from "../src/app.js";
import { loadConfig } from "../src/config.js";

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
  ])
    assert.throws(() => loadConfig({ ...env, ...invalid }), /DISCORD/);
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

test("signed Discord interactions create independent, admitted rooms without browser Origin bypass", async (t) => {
  const directory = mkdtempSync(join(tmpdir(), "freedomstream-discord-"));
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
