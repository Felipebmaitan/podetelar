import { backup, DatabaseSync } from "node:sqlite";
import { chmodSync, existsSync, unlinkSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { randomUUID } from "node:crypto";

/** SQLite's online backup includes committed WAL pages without pausing the app. */
export async function backupRooms(source: string, destination: string) {
  const target = resolve(destination);
  if (resolve(source) === target || existsSync(target))
    throw new Error("Choose a new backup path.");
  const temporary = `${target}.partial-${randomUUID()}`;
  const db = new DatabaseSync(source, { readOnly: true });
  try {
    await backup(db, temporary);
    chmodSync(temporary, 0o600);
    verifyBackup(temporary);
    // Exclusive link prevents a concurrent backup overwriting this destination.
    const { linkSync } = await import("node:fs");
    linkSync(temporary, target);
  } finally {
    db.close();
    if (existsSync(temporary)) unlinkSync(temporary);
  }
}

export function verifyBackup(path: string) {
  const db = new DatabaseSync(path, { readOnly: true });
  try {
    const integrity = db.prepare("PRAGMA integrity_check").all();
    const version = db.prepare("PRAGMA user_version").get();
    if (
      integrity.length !== 1 ||
      integrity[0]?.integrity_check !== "ok" ||
      ![3, 4].includes(Number(version?.user_version)) ||
      db.prepare("PRAGMA foreign_key_check").all().length
    )
      throw new Error("Not a healthy schema-3/4 FreedomStream backup.");
    for (const table of ["rooms", "sessions", "removals"])
      db.prepare(`SELECT count(*) FROM ${table}`).get();
    if (version?.user_version === 4)
      db.prepare(
        "SELECT room_id, joined_at, empty_since, invitation_cipher FROM room_lifecycle LIMIT 1",
      ).get();
  } finally {
    db.close();
  }
}

if (
  process.argv[1] &&
  pathToFileURL(process.argv[1]).href === import.meta.url
) {
  const [action, source, target] = process.argv.slice(2);
  if (action === "verify" && source && !target) verifyBackup(source);
  else if (action === "create" && source && target)
    await backupRooms(source, target);
  else
    throw new Error(
      "Usage: backup.js create SOURCE NEW_BACKUP | verify BACKUP",
    );
}
