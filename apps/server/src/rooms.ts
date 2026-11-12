import {
  createHash,
  createCipheriv,
  createDecipheriv,
  randomBytes,
  randomUUID,
  timingSafeEqual,
} from "node:crypto";
import { mkdirSync } from "node:fs";
import { dirname } from "node:path";
import { DatabaseSync, type StatementSync } from "node:sqlite";
import type {
  CreateRoomInput,
  ManagedRoom,
  Room,
  RoomSession,
} from "@freedomstream/contracts";

export const SESSION_TTL_MS = 24 * 60 * 60 * 1000;
export const EMPTY_ROOM_GRACE_MS = 60_000;
const digest = (value: string) => createHash("sha256").update(value).digest();
export const secretsEqual = (left: string, right: string) =>
  timingSafeEqual(digest(left), digest(right));
const randomSecret = () => randomBytes(32).toString("base64url");

type RoomRow = {
  id: string;
  name: string;
  created_at: number;
  invitation_hash: Uint8Array;
  invitation_enabled: number;
};
type SessionRow = {
  participant_id: string;
  display_name: string;
  expires_at: number;
};
type IssuedSession = { snapshot: RoomSession; token: string };

/** Owns room admission and persistence. Raw invitation/session secrets never enter SQLite. */
export class Rooms {
  private readonly db: DatabaseSync;
  private readonly findStatement: StatementSync;
  private readonly sessionStatement: StatementSync;
  private readonly activeStatement: StatementSync;
  constructor(
    path: string,
    private readonly now: () => number = Date.now,
    private readonly invitationKey?: string,
  ) {
    mkdirSync(dirname(path), { recursive: true });
    this.db = new DatabaseSync(path);
    this.db.exec(
      "PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;",
    );
    const version = this.db.prepare("PRAGMA user_version").get() as {
      user_version: number;
    };
    if (version.user_version > 4) {
      this.db.close();
      throw new Error(
        "The database was created by a newer FreedomStream version.",
      );
    }
    try {
      if (version.user_version < 3)
        this.transaction(() => {
          this.db.exec(`
      CREATE TABLE IF NOT EXISTS rooms (
        id TEXT PRIMARY KEY, name TEXT NOT NULL,
        invitation_hash BLOB NOT NULL, created_at INTEGER NOT NULL
      );
      CREATE TABLE IF NOT EXISTS sessions (
        token_hash BLOB PRIMARY KEY, room_id TEXT NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
        participant_id TEXT NOT NULL, display_name TEXT NOT NULL, expires_at INTEGER NOT NULL
      );
      CREATE INDEX IF NOT EXISTS sessions_expiry ON sessions(expires_at);
    `);
          if (version.user_version < 2)
            this.db.exec(`
      ALTER TABLE rooms ADD COLUMN invitation_enabled INTEGER NOT NULL DEFAULT 1;
      CREATE TABLE removals (
        room_id TEXT NOT NULL, participant_id TEXT NOT NULL,
        retry_until INTEGER NOT NULL, acknowledged INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY(room_id, participant_id)
      );
      CREATE INDEX sessions_identity ON sessions(room_id, participant_id);
    `);
          this.db.exec(`
          ALTER TABLE removals ADD COLUMN next_attempt_at INTEGER NOT NULL DEFAULT 0;
          CREATE INDEX removals_due ON removals(next_attempt_at);
          PRAGMA user_version = 3;
        `);
        });
      if (version.user_version < 4)
        this.transaction(() => {
          this.db.exec(`
            CREATE TABLE IF NOT EXISTS room_lifecycle (
              room_id TEXT PRIMARY KEY REFERENCES rooms(id) ON DELETE CASCADE,
              joined_at INTEGER, empty_since INTEGER, invitation_cipher BLOB
            );
            INSERT OR IGNORE INTO room_lifecycle(room_id, joined_at)
              SELECT id, COALESCE((SELECT MIN(expires_at) - ${SESSION_TTL_MS} FROM sessions WHERE room_id = rooms.id), created_at) FROM rooms;
            PRAGMA user_version = 4;
          `);
        });
      this.expire();
      this.db.exec("UPDATE room_lifecycle SET empty_since = NULL");
    } catch (error) {
      this.db.close();
      throw error;
    }
    this.db.exec(`CREATE TABLE IF NOT EXISTS discord_interactions (
      id TEXT PRIMARY KEY, guild_id TEXT NOT NULL, actor_id TEXT NOT NULL, created_at INTEGER NOT NULL
    ); CREATE INDEX IF NOT EXISTS discord_interactions_time ON discord_interactions(created_at);`);
    this.findStatement = this.db.prepare(
      "SELECT id, name, invitation_hash, invitation_enabled, created_at FROM rooms WHERE id = ?",
    );
    this.sessionStatement = this.db.prepare(
      "SELECT participant_id, display_name, expires_at FROM sessions WHERE token_hash = ? AND room_id = ? AND expires_at > ?",
    );
    this.activeStatement = this.db.prepare(
      "SELECT 1 FROM sessions WHERE room_id = ? AND participant_id = ? AND expires_at > ?",
    );
  }

  private insertRoom(name: string) {
    const invitation = randomSecret();
    const room: RoomRow = {
      id: randomBytes(16).toString("base64url"),
      name,
      invitation_hash: digest(invitation),
      invitation_enabled: 1,
      created_at: this.now(),
    };
    this.db
      .prepare(
        "INSERT INTO rooms(id, name, invitation_hash, created_at) VALUES (?, ?, ?, ?)",
      )
      .run(room.id, room.name, room.invitation_hash, room.created_at);
    this.db
      .prepare(
        "INSERT INTO room_lifecycle(room_id, invitation_cipher) VALUES (?, ?)",
      )
      .run(room.id, this.encryptInvitation(room.id, invitation));
    return { room, invitation };
  }

  create(input: CreateRoomInput): IssuedSession & { invitation: string } {
    return this.transaction(() => {
      const { room, invitation } = this.insertRoom(input.name);
      return { ...this.issue(room, input.displayName), invitation };
    });
  }

  createFromDiscord(
    id: string,
    guildId: string,
    actorId: string,
  ): { room: Room; invitation: string } | "duplicate" | "limited" {
    return this.transaction(() => {
      const now = this.now();
      this.db
        .prepare("DELETE FROM discord_interactions WHERE created_at < ?")
        .run(now - 10 * 60_000);
      if (
        this.db
          .prepare("SELECT 1 FROM discord_interactions WHERE id = ?")
          .get(id)
      )
        return "duplicate";
      const count = (
        this.db
          .prepare(
            "SELECT COUNT(*) AS count FROM discord_interactions WHERE created_at > ? AND (guild_id = ? OR actor_id = ?)",
          )
          .get(now - 60_000, guildId, actorId) as { count: number }
      ).count;
      if (count >= 5) return "limited";
      this.db
        .prepare("INSERT INTO discord_interactions VALUES (?, ?, ?, ?)")
        .run(id, guildId, actorId, now);
      const { room, invitation } = this.insertRoom("Friends");
      return {
        room: {
          id: room.id,
          name: room.name,
          createdAt: new Date(room.created_at).toISOString(),
        },
        invitation,
      };
    });
  }

  createManaged(name: string): { room: Room; invitation: string } {
    return this.transaction(() => {
      const { room, invitation } = this.insertRoom(name);
      return {
        room: {
          id: room.id,
          name: room.name,
          createdAt: new Date(room.created_at).toISOString(),
        },
        invitation,
      };
    });
  }

  /** Ciphertext is recoverable only by the server holding ADMIN_TOKEN. */
  private encryptInvitation(id: string, invitation: string): Buffer | null {
    if (!this.invitationKey) return null;
    const iv = randomBytes(12);
    const cipher = createCipheriv(
      "aes-256-gcm",
      digest(`invitation:${this.invitationKey}`),
      iv,
    );
    cipher.setAAD(Buffer.from(id));
    return Buffer.concat([
      iv,
      cipher.update(invitation, "utf8"),
      cipher.final(),
      cipher.getAuthTag(),
    ]);
  }

  invitation(id: string): string | null {
    if (!this.invitationKey || !this.find(id)?.invitation_enabled) return null;
    const row = this.db
      .prepare("SELECT invitation_cipher FROM room_lifecycle WHERE room_id = ?")
      .get(id) as { invitation_cipher: Uint8Array | null } | undefined;
    if (!row?.invitation_cipher) return null;
    try {
      const data = Buffer.from(row.invitation_cipher);
      const cipher = createDecipheriv(
        "aes-256-gcm",
        digest(`invitation:${this.invitationKey}`),
        data.subarray(0, 12),
      );
      cipher.setAAD(Buffer.from(id));
      cipher.setAuthTag(data.subarray(-16));
      const invitation = Buffer.concat([
        cipher.update(data.subarray(12, -16)),
        cipher.final(),
      ]).toString("utf8");
      return this.acceptsInvitation(id, invitation) ? invitation : null;
    } catch {
      return null;
    }
  }

  lifecycle(id: string): { joined: boolean; emptySince: number | null } | null {
    const row = this.db
      .prepare(
        "SELECT joined_at, empty_since FROM room_lifecycle WHERE room_id = ?",
      )
      .get(id) as
      { joined_at: number | null; empty_since: number | null } | undefined;
    return row
      ? { joined: row.joined_at !== null, emptySince: row.empty_since }
      : null;
  }

  observePresence(id: string, occupied: boolean): boolean {
    const state = this.lifecycle(id);
    if (!state || !state.joined) return false;
    if (occupied) {
      this.db
        .prepare(
          "UPDATE room_lifecycle SET empty_since = NULL WHERE room_id = ?",
        )
        .run(id);
    } else if (state.emptySince === null) {
      this.db
        .prepare("UPDATE room_lifecycle SET empty_since = ? WHERE room_id = ?")
        .run(this.now(), id);
    } else if (this.now() - state.emptySince >= EMPTY_ROOM_GRACE_MS) {
      return this.closeRoom(id);
    }
    return false;
  }

  presenceUnavailable(id: string): void {
    // Unknown time cannot count toward a continuously observed empty period.
    this.db
      .prepare("UPDATE room_lifecycle SET empty_since = NULL WHERE room_id = ?")
      .run(id);
  }

  signalingConnected(id: string): void {
    // SDK reconnects reuse their session. An older empty SFU response must not
    // expire a connection that has just been admitted upstream.
    this.db
      .prepare("UPDATE room_lifecycle SET empty_since = NULL WHERE room_id = ?")
      .run(id);
  }

  closeRoom(id: string): boolean {
    if (!this.find(id)) return false;
    this.transaction(() => {
      this.revokeRows("room_id = ?", [id]);
      this.db.prepare("DELETE FROM rooms WHERE id = ?").run(id);
    });
    return true;
  }

  list(offset: number): { rooms: ManagedRoom[]; nextOffset: number | null } {
    const rows = this.db
      .prepare(
        "SELECT id, name, created_at, invitation_enabled FROM rooms ORDER BY created_at DESC, id DESC LIMIT 51 OFFSET ?",
      )
      .all(offset) as Pick<
      RoomRow,
      "id" | "name" | "created_at" | "invitation_enabled"
    >[];
    return {
      rooms: rows.slice(0, 50).map((row) => ({
        id: row.id,
        name: row.name,
        createdAt: new Date(row.created_at).toISOString(),
        invitationEnabled: Boolean(row.invitation_enabled),
      })),
      nextOffset: rows.length > 50 ? offset + 50 : null,
    };
  }

  acceptsInvitation(id: string, invitation: string): boolean {
    const room = this.find(id);
    return Boolean(
      room?.invitation_enabled &&
      timingSafeEqual(digest(invitation), Buffer.from(room.invitation_hash)),
    );
  }

  join(
    id: string,
    invitation: string,
    displayName: string,
  ): IssuedSession | null {
    const room = this.find(id);
    if (
      !room ||
      !room.invitation_enabled ||
      !timingSafeEqual(digest(invitation), Buffer.from(room.invitation_hash))
    )
      return null;
    this.expire();
    if (
      (
        this.db
          .prepare("SELECT COUNT(*) AS count FROM sessions WHERE room_id = ?")
          .get(id) as { count: number }
      ).count >= 32
    )
      return null;
    return this.transaction(() => this.issue(room, displayName));
  }

  session(id: string, token: string | undefined): RoomSession | null {
    if (!token || !/^[A-Za-z0-9_-]{43}$/.test(token)) return null;
    const room = this.find(id);
    const session = this.sessionStatement.get(digest(token), id, this.now()) as
      SessionRow | undefined;
    return room && session ? this.snapshot(room, session) : null;
  }

  leave(id: string, token: string | undefined): void {
    const member = this.session(id, token);
    if (member) this.revoke(id, member.participant.id);
  }

  active(id: string, identity: string): boolean {
    return Boolean(this.activeStatement.get(id, identity, this.now()));
  }
  access(id: string) {
    const room = this.find(id);
    if (!room) return null;
    const sessions = this.db
      .prepare(
        "SELECT participant_id, display_name, expires_at FROM sessions WHERE room_id = ? AND expires_at > ? ORDER BY expires_at DESC LIMIT 100",
      )
      .all(id, this.now()) as SessionRow[];
    return {
      invitationEnabled: Boolean(room.invitation_enabled),
      sessions: sessions.map((row) => ({
        id: row.participant_id,
        displayName: row.display_name,
        expiresAt: new Date(row.expires_at).toISOString(),
      })),
    };
  }
  change(
    id: string,
    change: { invitation: "rotate" | "disable"; revokeSessions: boolean },
  ): { invitation?: string } | null {
    if (!this.find(id)) return null;
    const invitation =
      change.invitation === "rotate" ? randomSecret() : undefined;
    this.transaction(() => {
      this.db
        .prepare(
          "UPDATE rooms SET invitation_hash = ?, invitation_enabled = ? WHERE id = ?",
        )
        .run(digest(invitation ?? randomSecret()), invitation ? 1 : 0, id);
      this.db
        .prepare(
          "UPDATE room_lifecycle SET invitation_cipher = ? WHERE room_id = ?",
        )
        .run(invitation ? this.encryptInvitation(id, invitation) : null, id);
      if (change.revokeSessions) this.revokeRows("room_id = ?", [id]);
    });
    return invitation ? { invitation } : {};
  }
  revoke(id: string, identity: string): void {
    this.transaction(() =>
      this.revokeRows("room_id = ? AND participant_id = ?", [id, identity]),
    );
  }
  expire(): void {
    this.transaction(() => this.revokeRows("expires_at <= ?", [this.now()]));
  }
  removals(): { room: string; identity: string; retryUntil: number }[] {
    return (
      this.db
        .prepare(
          "SELECT room_id, participant_id, retry_until FROM removals WHERE next_attempt_at <= ? ORDER BY next_attempt_at, rowid LIMIT 128",
        )
        .all(this.now()) as {
        room_id: string;
        participant_id: string;
        retry_until: number;
      }[]
    ).map((row) => ({
      room: row.room_id,
      identity: row.participant_id,
      retryUntil: row.retry_until,
    }));
  }
  scheduleRemoval(room: string, identity: string, nextAttemptAt: number): void {
    this.db
      .prepare(
        "UPDATE removals SET next_attempt_at = ? WHERE room_id = ? AND participant_id = ?",
      )
      .run(nextAttemptAt, room, identity);
  }
  pending(id: string): number {
    return (
      this.db
        .prepare(
          "SELECT COUNT(*) AS count FROM removals WHERE room_id = ? AND acknowledged = 0",
        )
        .get(id) as { count: number }
    ).count;
  }
  acknowledge(room: string, identity: string): void {
    this.db
      .prepare(
        "UPDATE removals SET acknowledged = 1 WHERE room_id = ? AND participant_id = ?",
      )
      .run(room, identity);
  }
  removed(room: string, identity: string): void {
    this.db
      .prepare("DELETE FROM removals WHERE room_id = ? AND participant_id = ?")
      .run(room, identity);
  }
  time(): number {
    return this.now();
  }
  private transaction<T>(run: () => T): T {
    this.db.exec("BEGIN IMMEDIATE");
    try {
      const value = run();
      this.db.exec("COMMIT");
      return value;
    } catch (error) {
      this.db.exec("ROLLBACK");
      throw error;
    }
  }
  private revokeRows(where: string, values: (string | number)[]): void {
    // A short durable tombstone retries removal across an in-flight SFU handshake.
    this.db
      .prepare(
        `INSERT INTO removals(room_id, participant_id, retry_until, next_attempt_at) SELECT room_id, participant_id, ?, ? FROM sessions WHERE ${where} ON CONFLICT(room_id, participant_id) DO UPDATE SET retry_until = excluded.retry_until, next_attempt_at = excluded.next_attempt_at, acknowledged = 0`,
      )
      .run(this.now() + 30_000, this.now(), ...values);
    this.db.prepare(`DELETE FROM sessions WHERE ${where}`).run(...values);
  }

  close(): void {
    this.db.close();
  }

  private find(id: string): RoomRow | undefined {
    return this.findStatement.get(id) as RoomRow | undefined;
  }

  private issue(room: RoomRow, displayName: string): IssuedSession {
    const token = randomSecret();
    const session: SessionRow = {
      participant_id: randomUUID(),
      display_name: displayName,
      expires_at: this.now() + SESSION_TTL_MS,
    };
    this.db
      .prepare(
        "INSERT INTO sessions(token_hash, room_id, participant_id, display_name, expires_at) VALUES (?, ?, ?, ?, ?)",
      )
      .run(
        digest(token),
        room.id,
        session.participant_id,
        session.display_name,
        session.expires_at,
      );
    this.db
      .prepare(
        "UPDATE room_lifecycle SET joined_at = ?, empty_since = NULL WHERE room_id = ?",
      )
      .run(this.now(), room.id);
    return { token, snapshot: this.snapshot(room, session) };
  }

  private snapshot(room: RoomRow, session: SessionRow): RoomSession {
    const publicRoom: Room = {
      id: room.id,
      name: room.name,
      createdAt: new Date(room.created_at).toISOString(),
    };
    return {
      room: publicRoom,
      participant: {
        id: session.participant_id,
        displayName: session.display_name,
      },
      expiresAt: new Date(session.expires_at).toISOString(),
    };
  }
}
