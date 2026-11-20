import { expect } from "@playwright/test";
import { createServer } from "node:net";
import { createHash, randomBytes } from "node:crypto";
import { mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname, basename } from "node:path";
import { buildApp } from "../apps/server/src/app.js";
import { loadConfig } from "../apps/server/src/config.js";
import { Accounts } from "../apps/server/src/accounts.js";
import { DiscordVoice } from "../apps/server/src/discord-voice.js";

// Isolated local HTTP app and real SFU; no external Discord credentials or events.
export async function voiceFixture() {
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
  return {
    app,
    config,
    origin,
    guild,
    a,
    b,
    user,
    credential,
    voice,
    listen: () => app.listen({ port, host: "127.0.0.1" }),
    close: async () => {
      await app.close();
      accounts.close();
      const real = realpathSync(dir);
      expect(dirname(real)).toBe(realpathSync(tmpdir()));
      expect(basename(real).startsWith("podetelar-voice-live-")).toBe(true);
      rmSync(real, { recursive: true });
    },
  };
}
