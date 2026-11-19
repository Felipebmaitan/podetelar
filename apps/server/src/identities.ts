import { randomUUID } from "node:crypto";
import { DatabaseSync } from "node:sqlite";
import type { Account, Accounts } from "./accounts.js";
import type { Rooms } from "./rooms.js";
import type { GuestProfiles } from "./profiles.js";
import type { DiscordVoice } from "./discord-voice.js";
type Identity = {
  participant_id: string;
  room_id: string;
  user_id: string;
  login_id: string;
  display_name: string;
  avatar_url: string | null;
  avatar_revision: string | null;
  next_refresh_at: number;
};
type RefreshingIdentity = Identity & { binding_revision: Uint8Array };
export class Identities {
  private db: DatabaseSync;
  private syncing: Promise<void> | null = null;
  private pendingBinds = new Map<string, symbol>();
  private images = new Map<
    string,
    { bytes: Buffer; type: string; expires: number }
  >();
  private downloads = new Map<
    string,
    Promise<{ bytes: Buffer; type: string } | null>
  >();
  constructor(
    path: string,
    private rooms: Rooms,
    private accounts: Accounts,
    private profiles: GuestProfiles,
    private voice: DiscordVoice,
    private now = Date.now,
  ) {
    this.db = new DatabaseSync(path);
    this.db.exec(
      `PRAGMA foreign_keys=ON;PRAGMA busy_timeout=5000;CREATE TABLE IF NOT EXISTS discord_room_identities(participant_id TEXT PRIMARY KEY,room_id TEXT NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,user_id TEXT NOT NULL REFERENCES discord_accounts(user_id) ON DELETE CASCADE,login_id TEXT NOT NULL,display_name TEXT NOT NULL,avatar_url TEXT,avatar_revision TEXT,next_refresh_at INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS discord_identity_revisions(
        participant_id TEXT PRIMARY KEY REFERENCES discord_room_identities(participant_id) ON DELETE CASCADE,
        revision BLOB NOT NULL
      );
      CREATE TRIGGER IF NOT EXISTS discord_identity_bind_insert
        AFTER INSERT ON discord_room_identities
        BEGIN
          INSERT INTO discord_identity_revisions VALUES (new.participant_id, randomblob(16));
        END;
      CREATE TRIGGER IF NOT EXISTS discord_identity_bind_update
        AFTER UPDATE OF login_id ON discord_room_identities
        BEGIN
          UPDATE discord_identity_revisions SET revision=randomblob(16) WHERE participant_id=new.participant_id;
        END;
      INSERT OR IGNORE INTO discord_identity_revisions
        SELECT participant_id, randomblob(16) FROM discord_room_identities;`,
    );
  }
  async display(room: string, account: Account) {
    const context = this.rooms.channelContext(room);
    const member = context
      ? await this.voice.member(context.guildId, account.userId)
      : null;
    return {
      displayName: member?.displayName || account.displayName,
      avatarUrl: member?.avatarUrl || account.avatarUrl,
    };
  }
  async bind(
    room: string,
    participant: string,
    account: Account,
    knownDisplay?: { displayName: string; avatarUrl: string | null },
  ) {
    const attempt = Symbol();
    this.pendingBinds.set(participant, attempt);
    let display: Awaited<ReturnType<Identities["display"]>>;
    let current = false;
    try {
      display = knownDisplay ?? (await this.display(room, account));
    } finally {
      current = this.pendingBinds.get(participant) === attempt;
      if (current) this.pendingBinds.delete(participant);
    }
    if (
      !current ||
      !this.accounts.bySession(account.sessionId) ||
      !this.rooms.active(room, participant)
    )
      return false;
    const previous = this.row(participant);
    const revision = display.avatarUrl
      ? previous?.avatar_url === display.avatarUrl
        ? previous.avatar_revision
        : randomUUID()
      : null;
    this.db
      .prepare(
        `INSERT INTO discord_room_identities VALUES (?,?,?,?,?,?,?,?) ON CONFLICT(participant_id) DO UPDATE SET user_id=excluded.user_id,login_id=excluded.login_id,display_name=excluded.display_name,avatar_url=excluded.avatar_url,avatar_revision=excluded.avatar_revision,next_refresh_at=excluded.next_refresh_at`,
      )
      .run(
        participant,
        room,
        account.userId,
        account.sessionId,
        display.displayName,
        display.avatarUrl,
        revision,
        this.now() + 60000,
      );
    this.profiles.rename(room, participant, display.displayName);
    return true;
  }
  unbind(participant: string) {
    this.pendingBinds.delete(participant);
    this.db
      .prepare("DELETE FROM discord_room_identities WHERE participant_id=?")
      .run(participant);
  }
  private row(participant: string) {
    return this.db
      .prepare("SELECT * FROM discord_room_identities WHERE participant_id=?")
      .get(participant) as Identity | undefined;
  }
  get(participant: string) {
    const row = this.row(participant);
    if (
      !row ||
      !this.accounts.bySession(row.login_id) ||
      !this.rooms.active(row.room_id, participant)
    )
      return null;
    return {
      ...row,
      avatar_revision: this.accounts.avatarAllowed(row.user_id)
        ? row.avatar_revision
        : null,
    };
  }
  list(room: string) {
    return this.profiles.list(room).map((profile) => {
      const identity = this.get(profile.participantId);
      return identity
        ? {
            ...profile,
            displayName: identity.display_name,
            avatarRevision: identity.avatar_revision,
            uploadsBlocked: true,
            discord: true,
          }
        : { ...profile, discord: false };
    });
  }
  refresh(user?: string) {
    if (this.syncing) return this.syncing;
    this.syncing = (async () => {
      const rows = this.db
        .prepare(
          `SELECT i.*, r.revision AS binding_revision
          FROM discord_room_identities i JOIN sessions s ON s.participant_id=i.participant_id
          JOIN discord_identity_revisions r ON r.participant_id=i.participant_id
          WHERE s.expires_at>? AND (? IS NULL OR i.user_id=?) AND (? IS NOT NULL OR i.next_refresh_at<=?)
          ORDER BY i.next_refresh_at LIMIT 8`,
        )
        .all(
          this.now(),
          user ?? null,
          user ?? null,
          user ?? null,
          this.now(),
        ) as RefreshingIdentity[];
      await Promise.allSettled(rows.map((row) => this.refreshBinding(row)));
    })().finally(() => {
      this.syncing = null;
    });
    return this.syncing;
  }
  private async refreshBinding(row: RefreshingIdentity) {
    const account = this.accounts.bySession(row.login_id);
    if (!account) {
      this.db
        .prepare(
          "DELETE FROM discord_room_identities WHERE participant_id=? AND login_id=?",
        )
        .run(row.participant_id, row.login_id);
      return;
    }
    const display = await this.display(row.room_id, account);
    if (
      !this.accounts.bySession(row.login_id) ||
      !this.rooms.active(row.room_id, row.participant_id)
    )
      return;
    const revision = display.avatarUrl
      ? display.avatarUrl === row.avatar_url
        ? row.avatar_revision
        : randomUUID()
      : null;
    const updated = this.db
      .prepare(
        `UPDATE discord_room_identities AS identity
        SET display_name=?,avatar_url=?,avatar_revision=?,next_refresh_at=?
        WHERE participant_id=? AND EXISTS (
          SELECT 1 FROM discord_identity_revisions r WHERE r.participant_id=identity.participant_id AND r.revision=?
        )`,
      )
      .run(
        display.displayName,
        display.avatarUrl,
        revision,
        this.now() + 60000,
        row.participant_id,
        row.binding_revision,
      );
    if (updated.changes)
      this.profiles.rename(
        row.room_id,
        row.participant_id,
        display.displayName,
      );
  }
  async avatar(room: string, participant: string, revision: string) {
    const identity = this.get(participant);
    if (
      !identity ||
      identity.room_id !== room ||
      identity.avatar_revision !== revision ||
      !identity.avatar_url
    )
      return null;
    const url = identity.avatar_url;
    let cached = this.images.get(url);
    if (cached && cached.expires > this.now())
      return { bytes: cached.bytes, type: cached.type };
    if (this.downloads.has(url)) return this.downloads.get(url)!;
    if (this.downloads.size >= 2) return null;
    const pending = (async () => {
      try {
        const parsed = new URL(url);
        if (
          parsed.origin !== "https://cdn.discordapp.com" ||
          !/^\/(?:avatars\/\d{17,20}\/|guilds\/\d{17,20}\/users\/\d{17,20}\/avatars\/)(?:a_)?[a-f0-9]{32}\.(?:png|gif)$/.test(
            parsed.pathname,
          )
        )
          return null;
        const response = await fetch(url, {
          signal: AbortSignal.timeout(5000),
          redirect: "error",
        });
        if (
          !response.ok ||
          Number(response.headers.get("content-length") ?? 0) > 512000
        )
          return null;
        const reader = response.body?.getReader();
        if (!reader) return null;
        let size = 0;
        const chunks: Uint8Array[] = [];
        try {
          for (;;) {
            const part = await reader.read();
            if (part.done) break;
            size += part.value.length;
            if (size > 512000) return null;
            chunks.push(part.value);
          }
        } finally {
          await reader.cancel();
        }
        const bytes = Buffer.concat(chunks),
          type = parsed.pathname.endsWith(".gif") ? "image/gif" : "image/png";
        const current = this.get(participant);
        if (!current || current.avatar_revision !== revision) return null;
        if (this.images.size >= 24)
          this.images.delete(this.images.keys().next().value!);
        this.images.set(url, { bytes, type, expires: this.now() + 60000 });
        return { bytes, type };
      } catch {
        return null;
      }
    })().finally(() => this.downloads.delete(url));
    this.downloads.set(url, pending);
    return pending;
  }
  async close() {
    await this.syncing;
    await Promise.allSettled(this.downloads.values());
    this.images.clear();
    this.db.close();
  }
}
