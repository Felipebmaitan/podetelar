import {
  createHash,
  randomBytes,
  randomUUID,
  timingSafeEqual,
} from "node:crypto";
import { mkdirSync } from "node:fs";
import { dirname } from "node:path";
import { DatabaseSync } from "node:sqlite";
import type {
  CreateRoomInput,
  Room,
  RoomSession,
} from "@freedomstream/contracts";

export const SESSION_TTL_MS = 24 * 60 * 60 * 1000;
const digest = (value: string) => createHash("sha256").update(value).digest();
export const secretsEqual = (left: string, right: string) =>
  timingSafeEqual(digest(left), digest(right));
const randomSecret = () => randomBytes(32).toString("base64url");

type RoomRow = {
  id: string;
  name: string;
  created_at: number;
  invitation_hash: Uint8Array;
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
  constructor(
    path: string,
    private readonly now: () => number = Date.now,
  ) {
    mkdirSync(dirname(path), { recursive: true });
    this.db = new DatabaseSync(path);
    this.db.exec(
      "PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;",
    );
    const version = this.db.prepare("PRAGMA user_version").get() as {
      user_version: number;
    };
    if (version.user_version > 1) {
      this.db.close();
      throw new Error(
        "The database was created by a newer FreedomStream version.",
      );
    }
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
      PRAGMA user_version = 1;
    `);
    this.prune();
  }

  create(input: CreateRoomInput): IssuedSession & { invitation: string } {
    const invitation = randomSecret();
    const room: RoomRow = {
      id: randomBytes(16).toString("base64url"),
      name: input.name,
      invitation_hash: digest(invitation),
      created_at: this.now(),
    };
    this.db.exec("BEGIN IMMEDIATE");
    try {
      this.db
        .prepare(
          "INSERT INTO rooms(id, name, invitation_hash, created_at) VALUES (?, ?, ?, ?)",
        )
        .run(room.id, room.name, room.invitation_hash, room.created_at);
      const issued = this.issue(room, input.displayName);
      this.db.exec("COMMIT");
      return { ...issued, invitation };
    } catch (error) {
      this.db.exec("ROLLBACK");
      throw error;
    }
  }

  join(
    id: string,
    invitation: string,
    displayName: string,
  ): IssuedSession | null {
    const room = this.find(id);
    if (
      !room ||
      !timingSafeEqual(digest(invitation), Buffer.from(room.invitation_hash))
    )
      return null;
    this.prune();
    return this.issue(room, displayName);
  }

  session(id: string, token: string | undefined): RoomSession | null {
    if (!token || !/^[A-Za-z0-9_-]{43}$/.test(token)) return null;
    const room = this.find(id);
    const session = this.db
      .prepare(
        "SELECT participant_id, display_name, expires_at FROM sessions WHERE token_hash = ? AND room_id = ? AND expires_at > ?",
      )
      .get(digest(token), id, this.now()) as SessionRow | undefined;
    return room && session ? this.snapshot(room, session) : null;
  }

  leave(id: string, token: string | undefined): void {
    if (token)
      this.db
        .prepare("DELETE FROM sessions WHERE token_hash = ? AND room_id = ?")
        .run(digest(token), id);
  }

  close(): void {
    this.db.close();
  }

  private find(id: string): RoomRow | undefined {
    return this.db
      .prepare(
        "SELECT id, name, invitation_hash, created_at FROM rooms WHERE id = ?",
      )
      .get(id) as RoomRow | undefined;
  }

  private prune(): void {
    this.db
      .prepare("DELETE FROM sessions WHERE expires_at <= ?")
      .run(this.now());
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
