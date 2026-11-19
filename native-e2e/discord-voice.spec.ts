import { test, expect, type BrowserContext } from "@playwright/test";
import type { Room as LiveRoom } from "livekit-client";
import { readFileSync, mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname, basename } from "node:path";
import { createServer } from "node:net";
import { createHash, randomBytes } from "node:crypto";
import { buildApp } from "../apps/server/src/app.js";
import { loadConfig } from "../apps/server/src/config.js";
import { Accounts } from "../apps/server/src/accounts.js";
import { DiscordVoice } from "../apps/server/src/discord-voice.js";
import { LivekitDirectory } from "../apps/server/src/media.js";

test("confirmed voice departure removes the real SFU peer while an invitation peer stays connected", async ({
  browser,
}) => {
  test.skip(
    !process.env.PODETELAR_TEST_SFU_URL,
    "Run through native:room-smoke for an isolated real SFU.",
  );
  const reservation = createServer();
  await new Promise<void>((resolve) =>
    reservation.listen(0, "127.0.0.1", resolve),
  );
  const address = reservation.address();
  if (!address || typeof address === "string") throw Error("No test port");
  const port = address.port;
  await new Promise<void>((resolve, reject) =>
    reservation.close((error) => (error ? reject(error) : resolve())),
  );
  const origin = `http://localhost:${port}`,
    dir = mkdtempSync(join(tmpdir(), "podetelar-voice-live-")),
    path = join(dir, "rooms.db");
  const guild = "123456789012345678",
    a = "223456789012345678",
    b = "223456789012345679",
    user = "323456789012345678";
  const config = loadConfig({
    ...process.env,
    PUBLIC_URL: origin,
    DIRECT_URL: origin,
    DATABASE_PATH: path,
    LIVEKIT_INTERNAL_URL: process.env.PODETELAR_TEST_SFU_URL,
    DISCORD_APPLICATION_ID: undefined,
    DISCORD_PUBLIC_KEY: undefined,
    DISCORD_GUILD_IDS: undefined,
    DISCORD_BOT_TOKEN: undefined,
    DISCORD_CLIENT_SECRET: undefined,
  });
  const voice = new DiscordVoice(undefined);
  voice.index.apply("GUILD_CREATE", {
    id: guild,
    name: "Fixture guild",
    channels: [
      { id: a, name: "A", type: 2 },
      { id: b, name: "B", type: 2 },
    ],
    voice_states: [{ user_id: user, channel_id: a }],
  });
  voice.index.connected();
  const app = await buildApp(config, {
    usagePolling: false,
    discordVoice: voice,
  });
  const accounts = new Accounts(path, origin);
  const proof = randomBytes(32).toString("base64url"),
    flow = accounts.start(createHash("sha256").update(proof).digest("hex"))!,
    binding = accounts.authorize(flow.id)!;
  accounts.consume(binding.state, binding.browser);
  accounts.complete(flow.id, { id: user, username: "Voice fixture" });
  const credential = accounts.poll(flow.id, proof);
  if (credential.status !== "ready") throw Error("No fixture login");
  const contexts: BrowserContext[] = [];
  try {
    await app.listen({ port, host: "127.0.0.1" });
    const headers = { origin, authorization: `Bearer ${credential.token}` };
    const admit = () =>
      app.inject({
        method: "POST",
        url: "/api/auth/voice/join",
        headers,
        payload: {
          guildId: guild,
          channelId: a,
          deviceId: `pc1_${"f".repeat(64)}`,
        },
      });
    const auto = await admit();
    expect(auto.statusCode).toBe(200);
    const room = auto.json().room.id,
      autoCookie = String(auto.headers["set-cookie"]).split(";")[0]!;
    const resolved = await app.inject({
      method: "POST",
      url: `/api/channels/${guild}/${a}/resolve`,
      headers: { origin },
      payload: { invitation: new URL(auto.json().invitationUrl).hash.slice(8) },
    });
    expect(resolved.statusCode).toBe(200);
    const invitation = new URL(resolved.json().invitationUrl).hash.slice(8);
    const guest = await app.inject({
      method: "POST",
      url: `/api/rooms/${room}/join`,
      headers: { origin },
      payload: { invitation, displayName: "Independent fixture" },
    });
    expect(guest.statusCode).toBe(200);
    const guestCookie = String(guest.headers["set-cookie"]).split(";")[0]!;
    const connect = async (cookie: string) => {
      const access = await app.inject({
        method: "POST",
        url: `/api/rooms/${room}/native-media-token`,
        headers: { origin, cookie },
      });
      expect(access.statusCode).toBe(200);
      const context = await browser.newContext({ bypassCSP: true });
      contexts.push(context);
      const page = await context.newPage();
      await page.goto(origin);
      await page.addScriptTag({
        content: readFileSync(
          new URL(
            "../node_modules/livekit-client/dist/livekit-client.umd.js",
            import.meta.url,
          ),
          "utf8",
        ),
      });
      await page.evaluate(async (credentials) => {
        const scope = window as unknown as {
          LivekitClient: { Room: new (options: object) => LiveRoom };
          fixtureRoom: LiveRoom;
        };
        scope.fixtureRoom = new scope.LivekitClient.Room({
          autoSubscribe: false,
        });
        await scope.fixtureRoom.connect(
          credentials.serverUrl,
          credentials.token,
        );
      }, access.json());
      return page;
    };
    const controlled = await connect(autoCookie),
      independent = await connect(guestCookie);
    const connection = (page: typeof controlled) =>
      page.evaluate(
        () =>
          (window as unknown as { fixtureRoom: LiveRoom }).fixtureRoom.state,
      );
    voice.index.disconnected();
    await new Promise((resolve) => setTimeout(resolve, 500));
    expect(await connection(controlled)).toBe("connected");
    voice.index.connected();
    voice.index.apply("VOICE_STATE_UPDATE", {
      guild_id: guild,
      user_id: user,
      channel_id: b,
    });
    const directory = new LivekitDirectory(config);
    await expect
      .poll(async () => (await directory.members(room)).map((p) => p.id), {
        timeout: 10000,
      })
      .toEqual([guest.json().participant.id]);
    await expect
      .poll(() => connection(controlled), { timeout: 5000 })
      .not.toBe("connected");
    expect(await connection(independent)).toBe("connected");
    expect(
      (
        await app.inject({
          method: "POST",
          url: `/api/rooms/${room}/native-media-token`,
          headers: { origin, cookie: autoCookie },
        })
      ).statusCode,
    ).toBe(401);
  } finally {
    await Promise.all(contexts.map((context) => context.close()));
    await app.close();
    accounts.close();
    const real = realpathSync(dir);
    expect(dirname(real)).toBe(realpathSync(tmpdir()));
    expect(basename(real).startsWith("podetelar-voice-live-")).toBe(true);
    rmSync(real, { recursive: true });
  }
});
