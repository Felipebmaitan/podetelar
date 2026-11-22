import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname, basename } from "node:path";
import { createHash, randomBytes } from "node:crypto";
import { Accounts } from "../src/accounts.js";
import { Rooms } from "../src/rooms.js";
import { DiscordVoice } from "../src/discord-voice.js";
import { GuestProfiles } from "../src/profiles.js";
import { Identities } from "../src/identities.js";
import { buildApp } from "../src/app.js";
import { loadConfig } from "../src/config.js";
const guild = "123456789012345678",
  channel = "223456789012345678",
  user = "323456789012345678",
  origin = "http://localhost:5173",
  admin = "identity-test-admin-not-a-production-secret";
test("signed-in invitation joins use the room's guild nickname, prevent overrides, and honor avatar moderation", async () => {
  const dir = mkdtempSync(join(tmpdir(), "podetelar-identities-")),
    path = join(dir, "rooms.db");
  const rooms = new Rooms(path, Date.now, admin),
    accounts = new Accounts(path, origin);
  const credential = (() => {
    const proof = randomBytes(32).toString("base64url"),
      flow = accounts.start(createHash("sha256").update(proof).digest("hex"))!,
      browser = accounts.authorize(flow.id)!;
    accounts.consume(browser.state, browser.browser);
    accounts.complete(flow.id, { id: user, username: "Global friend" });
    const issued = accounts.poll(flow.id, proof);
    assert.equal(issued.status, "ready");
    if (issued.status !== "ready") throw Error("No credential");
    return issued;
  })();
  const target = rooms.resolveChannel(guild, channel, "Games")!;
  let memberGate: Promise<void> | undefined;
  let memberRequested = () => {};
  class Voice extends DiscordVoice {
    override async member(guildId: string) {
      assert.equal(guildId, guild, "nickname context is the room's guild");
      memberRequested();
      await memberGate;
      return {
        displayName: "Guild nickname",
        avatarUrl: `https://cdn.discordapp.com/guilds/${guild}/users/${user}/avatars/${"a".repeat(32)}.png?size=128`,
      };
    }
  }
  const voice = new Voice(undefined);
  const profiles = new GuestProfiles(path);
  const identities = new Identities(path, rooms, accounts, profiles, voice);
  const config = loadConfig({
    PUBLIC_URL: origin,
    ADMIN_TOKEN: admin,
    LIVEKIT_API_KEY: "identity-test-key",
    LIVEKIT_API_SECRET: "identity-test-media-not-a-production-secret",
    DATABASE_PATH: path,
  });
  config.webRoot = join(dir, "no-web");
  const app = await buildApp(config, {
    usagePolling: false,
    discordVoice: voice,
  });
  try {
    const joined = await app.inject({
      method: "POST",
      url: `/api/rooms/${target.room.id}/join`,
      headers: { origin, authorization: `Bearer ${credential.token}` },
      payload: {
        invitation: target.invitation,
        displayName: "Spoofed nickname",
        deviceId: `pc1_${"e".repeat(64)}`,
      },
    });
    assert.equal(joined.statusCode, 200);
    assert.equal(joined.json().participant.displayName, "Guild nickname");
    const cookie = String(joined.headers["set-cookie"]).split(";")[0]!,
      base = `/api/rooms/${target.room.id}`;
    const profile = (
      await app.inject({ url: `${base}/profiles`, headers: { cookie } })
    ).json().profiles[0];
    assert.equal(profile.discord, true);
    assert.ok(profile.avatarRevision);
    assert.equal(profile.uploadsBlocked, true);
    assert.equal(
      (
        await app.inject({
          method: "PATCH",
          url: `${base}/profile`,
          headers: { origin, cookie },
          payload: { displayName: "Override" },
        })
      ).statusCode,
      409,
    );
    accounts.manage(user, "hide-avatar");
    assert.equal(
      (
        await app.inject({ url: `${base}/profiles`, headers: { cookie } })
      ).json().profiles[0].avatarRevision,
      null,
    );
    let releaseMember = () => {};
    memberGate = new Promise<void>((resolve) => {
      releaseMember = resolve;
    });
    const refresh = identities.refresh(user);
    try {
      const unlink = await app.inject({
        method: "DELETE",
        url: `${base}/discord-identity`,
        headers: { origin, cookie },
      });
      assert.equal(unlink.statusCode, 204);
    } finally {
      releaseMember();
    }
    await refresh;
    assert.equal(
      (
        await app.inject({ url: `${base}/profiles`, headers: { cookie } })
      ).json().profiles[0].discord,
      false,
    );
    assert.equal(
      (
        await app.inject({
          method: "PATCH",
          url: `${base}/profile`,
          headers: { origin, cookie },
          payload: { displayName: "Guest again" },
        })
      ).statusCode,
      200,
    );
    const waitingForMember = new Promise<void>((resolve) => {
      memberRequested = resolve;
    });
    memberGate = new Promise<void>((resolve) => {
      releaseMember = resolve;
    });
    const pendingBind = app.inject({
      method: "POST",
      url: `${base}/discord-identity`,
      headers: { origin, cookie, authorization: `Bearer ${credential.token}` },
    });
    try {
      await waitingForMember;
      const unlink = await app.inject({
        method: "DELETE",
        url: `${base}/discord-identity`,
        headers: { origin, cookie },
      });
      assert.equal(unlink.statusCode, 204);
    } finally {
      releaseMember();
    }
    assert.equal((await pendingBind).statusCode, 401);
    const finalProfile = (
      await app.inject({ url: `${base}/profiles`, headers: { cookie } })
    ).json().profiles[0];
    assert.equal(finalProfile.discord, false);
    assert.equal(finalProfile.displayName, "Guest again");
  } finally {
    await app.close();
    await identities.close();
    await profiles.close();
    accounts.close();
    rooms.close();
    const real = realpathSync(dir);
    assert.equal(dirname(real), realpathSync(tmpdir()));
    assert.ok(basename(real).startsWith("podetelar-identities-"));
    rmSync(real, { recursive: true });
  }
});
