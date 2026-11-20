import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname, basename } from "node:path";
import { createHash, randomBytes } from "node:crypto";
import { Accounts } from "../src/accounts.js";
import { Rooms } from "../src/rooms.js";
import { DiscordVoice } from "../src/discord-voice.js";
import { buildApp } from "../src/app.js";
import { loadConfig } from "../src/config.js";
const guild = "123456789012345678",
  a = "223456789012345678",
  b = "223456789012345679",
  user = "323456789012345678",
  origin = "http://localhost:5173",
  admin = "voice-test-admin-not-a-production-secret";
test("voice admission follows confirmed membership, preserves outage sessions, and never controls invitation guests", async () => {
  const dir = mkdtempSync(join(tmpdir(), "podetelar-voice-follow-")),
    path = join(dir, "rooms.db");
  const rooms = new Rooms(path, Date.now, admin),
    accounts = new Accounts(path, origin);
  const proof = randomBytes(32).toString("base64url"),
    flow = accounts.start(createHash("sha256").update(proof).digest("hex"))!,
    browser = accounts.authorize(flow.id)!;
  accounts.consume(browser.state, browser.browser);
  accounts.complete(flow.id, { id: user, username: "Friend" });
  const issued = accounts.poll(flow.id, proof);
  assert.equal(issued.status, "ready");
  if (issued.status !== "ready") throw Error("Login missing");
  const voice = new DiscordVoice(undefined);
  const update = (channel: string | null) =>
    voice.index.apply("VOICE_STATE_UPDATE", {
      guild_id: guild,
      user_id: user,
      channel_id: channel,
    });
  voice.index.apply("GUILD_CREATE", {
    id: guild,
    name: "Friends",
    channels: [
      { id: a, name: "A", type: 2 },
      { id: b, name: "B", type: 2 },
    ],
    voice_states: [],
  });
  voice.index.connected();
  const config = loadConfig({
    PUBLIC_URL: origin,
    ADMIN_TOKEN: admin,
    LIVEKIT_API_KEY: "voice-test-key",
    LIVEKIT_API_SECRET: "voice-test-media-not-a-production-secret",
    DATABASE_PATH: path,
  });
  config.webRoot = join(dir, "none");
  const removed: string[] = [];
  const app = await buildApp(config, {
    usagePolling: false,
    discordVoice: voice,
    mediaDirectory: {
      members: async () => [],
      participant: async () => null,
      attribute: async () => {},
      remove: async (_room, participant) => {
        removed.push(participant);
      },
      rename: async () => {},
    },
  });
  const headers = { origin, authorization: `Bearer ${issued.token}` };
  const auto = () =>
    app.inject({
      method: "POST",
      url: "/api/auth/voice/join",
      headers,
      payload: {
        guildId: guild,
        channelId: a,
        deviceId: `pc1_${"e".repeat(64)}`,
      },
    });
  const status = (room: string, cookie: string) =>
    app.inject({ url: `/api/rooms/${room}/session`, headers: { cookie } });
  try {
    assert.equal((await auto()).statusCode, 409);
    update(a);
    const first = await auto();
    assert.equal(first.statusCode, 200);
    const room = first.json().room.id,
      participant = first.json().participant.id,
      cookie = String(first.headers["set-cookie"]).split(";")[0]!;
    const scopedUrl = new URL(first.json().roomInvitationUrl);
    assert.equal(scopedUrl.origin, origin);
    assert.equal(scopedUrl.pathname, `/r/${room}`);
    const invitation = new URLSearchParams(scopedUrl.hash.slice(1)).get(
      "invite",
    );
    assert.ok(invitation);
    assert.ok(
      invitation !== rooms.invitation(room),
      "channel guests receive the revocable handoff",
    );
    const guest = await app.inject({
      method: "POST",
      url: `/api/rooms/${room}/join`,
      headers,
      payload: { invitation, displayName: "Independent" },
    });
    assert.equal(guest.statusCode, 200);
    const guestCookie = String(guest.headers["set-cookie"]).split(";")[0]!;
    rooms.manageChannel(guild, a, "disable");
    const guestsDisabled = await auto();
    assert.equal(guestsDisabled.statusCode, 200);
    assert.equal(
      guestsDisabled.json().room.id,
      room,
      "one canonical channel room",
    );
    assert.equal(guestsDisabled.json().roomInvitationUrl, null);
    assert.equal(guestsDisabled.json().invitationUrl, null);
    assert.equal(rooms.join(room, invitation, "Guest"), null);
    rooms.manageChannel(guild, a, "enable");
    voice.index.disconnected();
    assert.equal((await status(room, cookie)).statusCode, 200);
    assert.equal((await auto()).statusCode, 409);
    voice.index.connected();
    update(b);
    assert.equal((await status(room, cookie)).statusCode, 401);
    assert.ok(removed.includes(participant));
    assert.equal(
      (await status(room, guestCookie)).statusCode,
      200,
      "authenticated URL admission remains independent",
    );
    update(a);
    const next = await auto();
    assert.equal(next.statusCode, 200);
    const nextCookie = String(next.headers["set-cookie"]).split(";")[0]!;
    update(null);
    assert.equal((await status(room, nextCookie)).statusCode, 401);
    assert.equal((await status(room, guestCookie)).statusCode, 200);
    update(a);
    rooms.manageChannel(guild, a, "pause");
    assert.equal((await auto()).statusCode, 409);
    rooms.manageChannel(guild, a, "resume");
    const final = await auto();
    assert.equal(final.statusCode, 200);
    const finalCookie = String(final.headers["set-cookie"]).split(";")[0]!;
    voice.index.apply("GUILD_DELETE", { id: guild, unavailable: true });
    assert.equal((await status(room, finalCookie)).statusCode, 200);
    voice.index.apply("GUILD_DELETE", { id: guild });
    assert.equal((await status(room, finalCookie)).statusCode, 401);
    voice.index.apply("GUILD_CREATE", {
      id: guild,
      name: "Friends",
      channels: [{ id: a, name: "A", type: 2 }],
      voice_states: [{ user_id: user, channel_id: a }],
    });
    const rejoined = await auto();
    assert.equal(rejoined.statusCode, 200);
    const rejoinedCookie = String(rejoined.headers["set-cookie"]).split(
      ";",
    )[0]!;
    voice.index.disconnected();
    voice.index.apply("READY", { guilds: [] });
    assert.equal((await status(room, rejoinedCookie)).statusCode, 200);
    voice.index.connected();
    assert.equal((await status(room, rejoinedCookie)).statusCode, 401);
    assert.equal((await status(room, guestCookie)).statusCode, 200);
    voice.index.apply("GUILD_CREATE", {
      id: guild,
      channels: [{ id: a, name: "A", type: 2 }],
      voice_states: [{ user_id: user, channel_id: a }],
    });
    const beforeBlock = await auto();
    assert.equal(beforeBlock.statusCode, 200);
    const blockedCookie = String(beforeBlock.headers["set-cookie"]).split(
      ";",
    )[0]!;
    assert.equal(
      (await app.inject({ url: "/api/admin/discord/following" })).statusCode,
      401,
    );
    const login = await app.inject({
      method: "POST",
      url: "/api/admin/login",
      headers: { origin },
      payload: { password: admin },
    });
    assert.equal(login.statusCode, 200);
    const adminCookie = String(login.headers["set-cookie"]).split(";")[0]!;
    const block = await app.inject({
      method: "POST",
      url: "/api/admin/discord/following",
      headers: { origin, cookie: adminCookie },
      payload: { action: "block", userId: user },
    });
    assert.equal(block.statusCode, 200);
    assert.equal((await status(room, blockedCookie)).statusCode, 401);
    assert.equal((await auto()).statusCode, 409);
    assert.equal((await status(room, guestCookie)).statusCode, 200);
  } finally {
    await app.close();
    accounts.close();
    rooms.close();
    const real = realpathSync(dir);
    assert.equal(dirname(real), realpathSync(tmpdir()));
    assert.ok(basename(real).startsWith("podetelar-voice-follow-"));
    rmSync(real, { recursive: true });
  }
});
