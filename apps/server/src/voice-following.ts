import { DatabaseSync } from "node:sqlite";
import type { Rooms } from "./rooms.js";
import type { Accounts } from "./accounts.js";
import type { DiscordVoice } from "./discord-voice.js";
import type { Access } from "./access.js";

type Lease = {
  roomId: string;
  participantId: string;
  userId: string;
  loginId: string;
  guildId: string;
  channelId: string;
};
/** Invitation sessions never enter this table. Unknown Gateway state preserves admitted sessions. */
export class VoiceFollowing {
  private db: DatabaseSync;
  private unsubscribe: () => void;
  constructor(
    path: string,
    private rooms: Rooms,
    private accounts: Accounts,
    private voice: DiscordVoice,
    private access: Access,
  ) {
    this.db = new DatabaseSync(path);
    this.db.exec(`PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;
      CREATE TABLE IF NOT EXISTS discord_voice_sessions(participantId TEXT PRIMARY KEY,roomId TEXT NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,userId TEXT NOT NULL,loginId TEXT NOT NULL,guildId TEXT NOT NULL,channelId TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS discord_auto_settings(key TEXT PRIMARY KEY,value INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS discord_auto_blocks(userId TEXT PRIMARY KEY);`);
    this.unsubscribe = voice.index.onChange(() => this.reconcile());
  }
  enabled(user?: string) {
    const global = this.db
      .prepare("SELECT value FROM discord_auto_settings WHERE key='enabled'")
      .get() as { value: number } | undefined;
    return (
      global?.value !== 0 &&
      (!user ||
        !this.db
          .prepare("SELECT 1 FROM discord_auto_blocks WHERE userId=?")
          .get(user))
    );
  }
  channelEnabled(guild: string, channel: string) {
    return this.rooms.channelAutomatic(guild, channel);
  }
  eligible(user: string, guild: string, channel: string) {
    const lookup = this.voice.index.lookup(user, guild);
    return (
      this.enabled(user) &&
      this.channelEnabled(guild, channel) &&
      lookup.known &&
      lookup.locations.some((c) => c.channelId === channel)
    );
  }
  attach(lease: Lease) {
    if (
      !this.accounts.bySession(lease.loginId) ||
      !this.eligible(lease.userId, lease.guildId, lease.channelId) ||
      !this.rooms.active(lease.roomId, lease.participantId)
    )
      return false;
    this.db
      .prepare("INSERT INTO discord_voice_sessions VALUES (?,?,?,?,?,?)")
      .run(
        lease.participantId,
        lease.roomId,
        lease.userId,
        lease.loginId,
        lease.guildId,
        lease.channelId,
      );
    return true;
  }
  reconcile() {
    const leases = this.db
      .prepare("SELECT * FROM discord_voice_sessions")
      .all() as Lease[];
    let revoked = false;
    for (const lease of leases) {
      if (!this.rooms.active(lease.roomId, lease.participantId)) {
        this.db
          .prepare("DELETE FROM discord_voice_sessions WHERE participantId=?")
          .run(lease.participantId);
        continue;
      }
      const lookup = this.voice.index.lookup(lease.userId, lease.guildId);
      if (
        !this.accounts.bySession(lease.loginId) ||
        !this.enabled(lease.userId) ||
        !this.channelEnabled(lease.guildId, lease.channelId) ||
        (lookup.known &&
          !lookup.locations.some((c) => c.channelId === lease.channelId))
      ) {
        this.rooms.revoke(lease.roomId, lease.participantId);
        this.db
          .prepare("DELETE FROM discord_voice_sessions WHERE participantId=?")
          .run(lease.participantId);
        revoked = true;
      }
    }
    if (revoked) void this.access.settle();
  }
  directory() {
    this.reconcile();
    return {
      enabled: this.enabled(),
      blockedUsers: (
        this.db
          .prepare("SELECT userId FROM discord_auto_blocks ORDER BY userId")
          .all() as { userId: string }[]
      ).map((r) => r.userId),
      sessions: this.db
        .prepare("SELECT * FROM discord_voice_sessions")
        .all() as Lease[],
    };
  }
  manage(action: "enable" | "disable" | "block" | "allow", user?: string) {
    if (action === "enable" || action === "disable")
      this.db
        .prepare(
          "INSERT INTO discord_auto_settings VALUES ('enabled',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        )
        .run(Number(action === "enable"));
    else if (action === "block" && user)
      this.db
        .prepare("INSERT OR IGNORE INTO discord_auto_blocks VALUES (?)")
        .run(user);
    else if (action === "allow" && user)
      this.db
        .prepare("DELETE FROM discord_auto_blocks WHERE userId=?")
        .run(user);
    this.reconcile();
  }
  close() {
    this.unsubscribe();
    this.db.close();
  }
}
