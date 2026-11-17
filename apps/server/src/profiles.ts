import { randomUUID } from "node:crypto";
import { DatabaseSync } from "node:sqlite";
import sharp from "sharp";
import {
  AVATAR_MAX_BYTES,
  type AdminGuestProfile,
  type GuestProfile,
} from "@podetelar/contracts";
import type { MediaDirectory } from "./media.js";

const PIXEL_LIMIT = 16_000_000;
const STORAGE_LIMIT = 128 * 1024 * 1024;
type Row = {
  id: string;
  device_id: string;
  display_name: string;
  revision: string | null;
  blocked: number;
  updated_at: number;
  bytes: Uint8Array | null;
  animated: number;
};

/** Persistent guest cosmetics. Device claims are intentionally not hardware authentication. */
export class GuestProfiles {
  private db: DatabaseSync;
  private processing = 0;
  private syncing: Promise<void> | null = null;
  constructor(
    path: string,
    private now = Date.now,
  ) {
    this.db = new DatabaseSync(path);
    this.db.exec(`PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;
      CREATE TABLE IF NOT EXISTS guest_profiles (
        id TEXT PRIMARY KEY, device_id TEXT NOT NULL UNIQUE, display_name TEXT NOT NULL,
        revision TEXT, blocked INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL,
        bytes BLOB, animated INTEGER NOT NULL DEFAULT 0
      );
      CREATE TABLE IF NOT EXISTS guest_profile_sessions (
        participant_id TEXT PRIMARY KEY, room_id TEXT NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
        profile_id TEXT NOT NULL REFERENCES guest_profiles(id)
      );
      CREATE TABLE IF NOT EXISTS guest_profile_names (
        participant_id TEXT PRIMARY KEY, room_id TEXT NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
        display_name TEXT NOT NULL, next_attempt_at INTEGER NOT NULL DEFAULT 0
      );
      CREATE INDEX IF NOT EXISTS guest_profile_room ON guest_profile_sessions(room_id);`);
    const columns = this.db
      .prepare("PRAGMA table_info(guest_profile_names)")
      .all();
    if (!columns.some((column) => column.name === "next_attempt_at"))
      this.db.exec(
        "ALTER TABLE guest_profile_names ADD COLUMN next_attempt_at INTEGER NOT NULL DEFAULT 0",
      );
  }
  bind(room: string, participant: string, name: string, device?: string) {
    this.db
      .prepare(
        "DELETE FROM guest_profile_sessions WHERE participant_id NOT IN (SELECT participant_id FROM sessions WHERE expires_at > ?)",
      )
      .run(this.now());
    if (!device) return;
    const existing = this.db
      .prepare("SELECT id FROM guest_profiles WHERE device_id = ?")
      .get(device);
    if (
      !existing &&
      Number(
        this.db.prepare("SELECT COUNT(*) AS n FROM guest_profiles").get()!.n,
      ) >= 10000
    )
      return;
    this.db
      .prepare(
        `INSERT INTO guest_profiles(id,device_id,display_name,updated_at) VALUES (?,?,?,?)
      ON CONFLICT(device_id) DO UPDATE SET display_name=excluded.display_name`,
      )
      .run(randomUUID(), device, name, this.now());
    this.db
      .prepare(
        `INSERT INTO guest_profile_sessions(participant_id,room_id,profile_id)
      SELECT ?,?,id FROM guest_profiles WHERE device_id = ?`,
      )
      .run(participant, room, device);
  }
  private row(room: string, participant: string): Row | undefined {
    return this.db
      .prepare(
        `SELECT p.* FROM guest_profiles p JOIN guest_profile_sessions b ON b.profile_id=p.id
      JOIN sessions s ON s.participant_id=b.participant_id AND s.room_id=b.room_id
      WHERE b.room_id=? AND b.participant_id=? AND s.expires_at>?`,
      )
      .get(room, participant, this.now()) as Row | undefined;
  }
  list(room: string): GuestProfile[] {
    return (
      this.db
        .prepare(
          `SELECT s.participant_id,s.display_name,p.revision,p.blocked FROM sessions s
      LEFT JOIN guest_profile_sessions b ON b.participant_id=s.participant_id
      LEFT JOIN guest_profiles p ON p.id=b.profile_id WHERE s.room_id=? AND s.expires_at>? LIMIT 32`,
        )
        .all(room, this.now()) as {
        participant_id: string;
        display_name: string;
        revision: string | null;
        blocked: number | null;
      }[]
    ).map((p) => ({
      participantId: p.participant_id,
      displayName: p.display_name,
      avatarRevision: p.revision,
      uploadsBlocked: Boolean(p.blocked),
    }));
  }
  rename(room: string, participant: string, name: string) {
    this.db
      .prepare(
        "UPDATE sessions SET display_name=? WHERE room_id=? AND participant_id=? AND expires_at>?",
      )
      .run(name, room, participant, this.now());
    this.db
      .prepare(
        `INSERT INTO guest_profile_names(participant_id,room_id,display_name) VALUES (?,?,?)
      ON CONFLICT(participant_id) DO UPDATE SET display_name=excluded.display_name,next_attempt_at=0`,
      )
      .run(participant, room, name);
    const row = this.row(room, participant);
    if (row) {
      this.db
        .prepare("UPDATE guest_profiles SET display_name=? WHERE id=?")
        .run(name, row.id);
    }
  }
  syncNames(directory: MediaDirectory): Promise<void> {
    if (this.syncing) return this.syncing;
    this.syncing = (async () => {
      this.db
        .prepare(
          `DELETE FROM guest_profile_names WHERE NOT EXISTS (
        SELECT 1 FROM sessions s WHERE s.participant_id=guest_profile_names.participant_id
        AND s.room_id=guest_profile_names.room_id AND s.expires_at>?)`,
        )
        .run(this.now());
      if (!directory.rename) return;
      const rows = this.db
        .prepare(
          `SELECT b.participant_id,b.room_id,b.display_name AS pending_name FROM guest_profile_names b JOIN sessions s
        ON s.participant_id=b.participant_id AND s.room_id=b.room_id WHERE s.expires_at>? AND b.next_attempt_at<=?
        ORDER BY b.next_attempt_at,b.rowid LIMIT 8`,
        )
        .all(this.now(), this.now()) as {
        participant_id: string;
        room_id: string;
        pending_name: string;
      }[];
      await Promise.allSettled(
        rows.map(async (row) => {
          try {
            await directory.rename!(
              row.room_id,
              row.participant_id,
              row.pending_name,
            );
            this.db
              .prepare(
                "DELETE FROM guest_profile_names WHERE participant_id=? AND display_name=?",
              )
              .run(row.participant_id, row.pending_name);
          } catch {
            this.db
              .prepare(
                "UPDATE guest_profile_names SET next_attempt_at=? WHERE participant_id=? AND display_name=?",
              )
              .run(this.now() + 5000, row.participant_id, row.pending_name);
          }
        }),
      );
    })().finally(() => {
      this.syncing = null;
    });
    return this.syncing;
  }
  async upload(room: string, participant: string, input: Buffer) {
    const row = this.row(room, participant);
    if (!row) return "unavailable";
    if (row.blocked) return "blocked";
    if (this.processing >= 2) return "busy";
    if (!input.length || input.length > AVATAR_MAX_BYTES) return "invalid";
    // Reject vector/doc inputs before libvips sees them, irrespective of Content-Type.
    const signature = input.subarray(0, 12);
    const allowed =
      signature.subarray(0, 3).equals(Buffer.from([255, 216, 255])) ||
      signature
        .subarray(0, 8)
        .equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])) ||
      /^GIF8[79]a/.test(signature.toString("ascii")) ||
      (signature.toString("ascii", 0, 4) === "RIFF" &&
        signature.toString("ascii", 8, 12) === "WEBP");
    if (!allowed) return "invalid";
    this.processing++;
    try {
      const image = sharp(input, {
        animated: true,
        limitInputPixels: PIXEL_LIMIT,
        failOn: "warning",
      });
      const meta = await image.metadata();
      const frames = meta.pages ?? 1,
        height = meta.pageHeight ?? meta.height ?? 0,
        width = meta.width ?? 0;
      if (
        !width ||
        !height ||
        width > 4096 ||
        height > 4096 ||
        frames > 200 ||
        width * height * frames > PIXEL_LIMIT ||
        !["jpeg", "png", "webp", "gif"].includes(meta.format ?? "")
      )
        return "invalid";
      const animated = frames > 1;
      const resized = image
        .resize(128, 128, { fit: "cover" })
        .timeout({ seconds: 5 });
      const bytes = await (
        animated ? resized.gif({ effort: 1 }) : resized.png()
      ).toBuffer();
      if (bytes.length > AVATAR_MAX_BYTES) return "invalid";
      // Recheck authority and moderation after asynchronous decoding; deletion cannot resurrect a stale upload.
      const current = this.row(room, participant);
      if (!current || current.id !== row.id) return "unavailable";
      if (current.blocked || current.updated_at !== row.updated_at)
        return "changed";
      const used = Number(
        this.db
          .prepare(
            "SELECT COALESCE(SUM(LENGTH(bytes)),0) AS n FROM guest_profiles",
          )
          .get()!.n,
      );
      if (used - (current.bytes?.length ?? 0) + bytes.length > STORAGE_LIMIT)
        return "full";
      this.db
        .prepare(
          "UPDATE guest_profiles SET bytes=?,revision=?,animated=?,updated_at=? WHERE id=?",
        )
        .run(
          bytes,
          randomUUID(),
          Number(animated),
          Math.max(this.now(), current.updated_at + 1),
          row.id,
        );
      return "ok";
    } catch {
      return "invalid";
    } finally {
      this.processing--;
    }
  }
  remove(room: string, participant: string) {
    const row = this.row(room, participant);
    if (!row) return false;
    this.moderate(row.id, "delete");
    return true;
  }
  avatar(room: string, participant: string, revision: string) {
    const row = this.row(room, participant);
    return row?.revision === revision && row.bytes
      ? {
          bytes: Buffer.from(row.bytes),
          type: row.animated ? "image/gif" : "image/png",
        }
      : null;
  }
  directory(offset: number) {
    const rows = this.db
      .prepare(
        "SELECT id,device_id,display_name,revision,blocked,updated_at,LENGTH(bytes) AS size,animated FROM guest_profiles ORDER BY updated_at DESC,id LIMIT 25 OFFSET ?",
      )
      .all(offset) as (Omit<Row, "bytes"> & { size: number })[];
    return {
      profiles: rows.slice(0, 24).map(
        (row) =>
          ({
            id: row.id,
            deviceId: row.device_id,
            displayName: row.display_name,
            avatarRevision: row.revision,
            uploadsBlocked: Boolean(row.blocked),
            updatedAt: row.updated_at,
            bytes: row.size ?? 0,
            animated: Boolean(row.animated),
          }) satisfies AdminGuestProfile,
      ),
      nextOffset: rows.length > 24 ? offset + 24 : null,
    };
  }
  adminAvatar(id: string, revision: string) {
    const row = this.db
      .prepare("SELECT * FROM guest_profiles WHERE id=? AND revision=?")
      .get(id, revision) as Row | undefined;
    return row?.bytes
      ? {
          bytes: Buffer.from(row.bytes),
          type: row.animated ? "image/gif" : "image/png",
        }
      : null;
  }
  moderate(id: string, action: "delete" | "block" | "allow") {
    const result = this.db
      .prepare(
        `UPDATE guest_profiles SET bytes=CASE WHEN ?='delete' THEN NULL ELSE bytes END,
      revision=CASE WHEN ?='delete' THEN NULL ELSE revision END,animated=CASE WHEN ?='delete' THEN 0 ELSE animated END,
      blocked=CASE WHEN ?='block' THEN 1 WHEN ?='allow' THEN 0 ELSE blocked END,
      updated_at=MAX(?,updated_at+1) WHERE id=?`,
      )
      .run(action, action, action, action, action, this.now(), id);
    return result.changes > 0;
  }
  async close() {
    await this.syncing;
    this.db.close();
  }
}
