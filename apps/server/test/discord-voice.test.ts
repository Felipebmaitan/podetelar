import assert from "node:assert/strict";
import { test } from "node:test";
import { VoiceIndex } from "../src/discord-voice.js";
import { DiscordVoice } from "../src/discord-voice.js";
import { Client } from "discord.js";
import { EventEmitter } from "node:events";
const guild = "123456789012345678",
  other = "223456789012345678",
  channel = "323456789012345678",
  second = "423456789012345678",
  user = "523456789012345678";
const snapshot = (id = guild) => ({
  id,
  name: "Friends",
  channels: [
    { id: channel, name: "Games", type: 2 },
    { id: second, name: "Other", type: 2 },
  ],
  voice_states: [{ user_id: user, channel_id: channel }],
});
test("Gateway snapshots and ordered voice changes work across installed servers", () => {
  const index = new VoiceIndex();
  index.apply("READY", {});
  index.apply("GUILD_CREATE", snapshot());
  index.apply("GUILD_CREATE", snapshot(other));
  assert.equal(index.lookup(user).known, false);
  index.connected();
  assert.equal(index.lookup(user).locations.length, 2);
  index.apply("VOICE_STATE_UPDATE", {
    guild_id: guild,
    user_id: user,
    channel_id: second,
  });
  assert.equal(index.lookup(user, guild).locations[0]!.channelId, second);
  const revision = index.status().revision;
  index.apply("VOICE_STATE_UPDATE", {
    guild_id: guild,
    user_id: user,
    channel_id: second,
    self_mute: true,
  });
  assert.equal(index.status().revision, revision);
  index.apply("VOICE_STATE_UPDATE", {
    guild_id: guild,
    user_id: user,
    channel_id: null,
  });
  assert.deepEqual(index.lookup(user, guild), { known: true, locations: [] });
  index.disconnected();
  assert.deepEqual(index.lookup(user), { known: false, locations: [] });
  index.connected();
  assert.equal(index.lookup(user, other).locations.length, 1);
});
test("guild outage is unknown, removal clears state and fresh READY discards stale memberships", () => {
  const index = new VoiceIndex();
  index.apply("GUILD_CREATE", snapshot());
  index.connected();
  index.apply("GUILD_DELETE", { id: guild, unavailable: true });
  assert.equal(index.lookup(user, guild).known, false);
  index.apply("GUILD_CREATE", snapshot());
  assert.equal(index.lookup(user, guild).locations.length, 1);
  index.apply("CHANNEL_DELETE", {
    guild_id: guild,
    id: channel,
    name: "Games",
    type: 2,
  });
  assert.deepEqual(index.lookup(user, guild).locations, []);
  index.apply("GUILD_DELETE", { id: guild });
  assert.equal(index.status().guilds.length, 0);
  index.apply("READY", {});
  index.connected();
  assert.equal(index.lookup(user).locations.length, 0);
});
test("guild policies and malformed events cannot create usable state", () => {
  const index = new VoiceIndex([guild]);
  index.apply("GUILD_CREATE", snapshot(other));
  index.apply("GUILD_CREATE", { ...snapshot(), voice_states: "invalid" });
  index.connected();
  assert.equal(index.status().guilds.length, 0);
});

test("READY guilds remain unknown until their snapshot arrives", () => {
  const index = new VoiceIndex();
  index.apply("READY", { guilds: [{ id: guild }, { id: other }] });
  index.connected();
  index.apply("GUILD_CREATE", snapshot());
  assert.equal(index.lookup(user).known, false);
  assert.equal(index.lookup(user, guild).known, true);
  index.apply("GUILD_CREATE", snapshot(other));
  assert.equal(index.lookup(user).known, true);
});

test("SDK fresh READY and shardReady recover after a non-resumable connection", async () => {
  const client = new Client({ intents: [] });
  client.login = async () => "synthetic-login";
  const gateway = new DiscordVoice(
    {
      applicationId: guild,
      publicKey: "0".repeat(64),
      guildIds: ["*"],
      botToken: "synthetic-bot",
    },
    () => client,
  );
  const emit = (event: string, ...args: unknown[]) =>
    EventEmitter.prototype.emit.call(client, event, ...args);
  try {
    await gateway.start();
    emit("raw", { t: "READY", d: { guilds: [{ id: guild }] } });
    emit("raw", { t: "GUILD_CREATE", d: snapshot() });
    emit("clientReady", client);
    assert.equal(gateway.index.lookup(user).known, true);
    emit("shardReconnecting", 0);
    emit("raw", { t: "READY", d: { guilds: [{ id: guild }] } });
    assert.equal(gateway.index.lookup(user).known, false);
    emit("raw", { t: "GUILD_CREATE", d: snapshot() });
    emit("shardReady", 0, new Set());
    assert.equal(gateway.index.lookup(user).locations.length, 1);
  } finally {
    await gateway.close();
  }
});
