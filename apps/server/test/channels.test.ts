import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtempSync, rmSync, realpathSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname, basename } from "node:path";
import { Rooms } from "../src/rooms.js";
const guild = "123456789012345678",
  channel = "223456789012345678",
  key = "channel-test-key-not-a-production-secret";
test("permanent channel invitations survive room closure/restart, rotate and remain distinct from auto admission", () => {
  const dir = mkdtempSync(join(tmpdir(), "podetelar-channels-")),
    path = join(dir, "rooms.db");
  let rooms = new Rooms(path, Date.now, key);
  try {
    assert.equal(rooms.resolveChannel(guild, channel, "Games", "bad"), null);
    assert.equal(rooms.channelDirectory().length, 0);
    const secret = rooms.channelInvitation(guild, channel, "Games")!;
    assert.equal(secret.length, 43);
    const first = rooms.resolveChannel(guild, channel, "Games", secret)!;
    const same = rooms.resolveChannel(guild, channel, "Games")!;
    assert.equal(first.room.id, same.room.id);
    const joined = rooms.join(first.room.id, first.invitation, "Guest")!;
    rooms.manageChannel(guild, channel, "pause");
    assert.equal(rooms.resolveChannel(guild, channel, "Games"), null);
    assert.equal(
      rooms.resolveChannel(guild, channel, "Games", secret)!.room.id,
      first.room.id,
    );
    rooms.manageChannel(guild, channel, "disable");
    assert.equal(rooms.resolveChannel(guild, channel, "Games", secret), null);
    assert.ok(rooms.session(first.room.id, joined.token));
    rooms.manageChannel(guild, channel, "rotate");
    const replacement = rooms.channelInvitation(guild, channel, "Games")!;
    assert.notEqual(replacement, secret);
    assert.equal(rooms.resolveChannel(guild, channel, "Games", secret), null);
    rooms.closeRoom(first.room.id);
    rooms.close();
    rooms = new Rooms(path, Date.now, key);
    assert.equal(rooms.channelInvitation(guild, channel, "Games"), replacement);
    const next = rooms.resolveChannel(guild, channel, "Games", replacement)!;
    assert.notEqual(next.room.id, first.room.id);
    rooms.change(next.room.id, {
      invitation: "disable",
      revokeSessions: false,
    });
    assert.equal(
      rooms.resolveChannel(guild, channel, "Games", replacement),
      null,
      "a room access lock cannot be bypassed by a permanent invitation",
    );
  } finally {
    rooms.close();
    const real = realpathSync(dir);
    assert.equal(dirname(real), realpathSync(tmpdir()));
    assert.ok(basename(real).startsWith("podetelar-channels-"));
    rmSync(real, { recursive: true });
  }
});
test("slash interactions reuse the channel room and keep persistent replay protection", () => {
  const dir = mkdtempSync(join(tmpdir(), "podetelar-channels-"));
  const rooms = new Rooms(join(dir, "rooms.db"), Date.now, key);
  try {
    const location = { guildId: guild, channelId: channel, name: "Games" };
    const a = rooms.createFromDiscord(
      "323456789012345678",
      guild,
      "423456789012345678",
      "A",
      location,
    );
    const b = rooms.createFromDiscord(
      "523456789012345678",
      guild,
      "623456789012345678",
      "B",
      location,
    );
    assert.ok(typeof a === "object" && typeof b === "object");
    assert.equal(a.room.id, b.room.id);
    rooms.manageChannel(guild, channel, "pause");
    const explicit = rooms.createFromDiscord(
      "723456789012345678",
      guild,
      "423456789012345678",
      "Explicit",
      location,
    );
    assert.ok(
      typeof explicit === "object",
      "pausing automatic joins must not disable an explicit command invitation",
    );
    rooms.manageChannel(guild, channel, "resume");
    rooms.manageChannel(guild, channel, "disable");
    assert.equal(
      rooms.createFromDiscord(
        "823456789012345678",
        guild,
        "423456789012345678",
        "Disabled guest",
        location,
      ),
      "unavailable",
    );
    assert.ok(
      rooms.resolveChannel(guild, channel, "Games"),
      "disabling guest invitations must not pause automatic joining",
    );
    assert.equal(
      rooms.createFromDiscord(
        "323456789012345678",
        guild,
        "423456789012345678",
        "A",
        location,
      ),
      "duplicate",
    );
  } finally {
    rooms.close();
    const real = realpathSync(dir);
    assert.equal(dirname(real), realpathSync(tmpdir()));
    assert.ok(basename(real).startsWith("podetelar-channels-"));
    rmSync(real, { recursive: true });
  }
});
