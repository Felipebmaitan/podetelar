import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtempSync, readFileSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import { Rooms } from "../src/rooms.js";
import { backupRooms, verifyBackup } from "../src/backup.js";

test("online backup restores invitations, revocations and committed WAL without replacing a backup", async () => {
  const root = mkdtempSync(join(tmpdir(), "freedomstream-backup-"));
  const source = join(root, "live.db");
  const target = join(root, "restore.db");
  const rooms = new Rooms(source);
  try {
    const created = rooms.create({ name: "Friends", displayName: "Felipe" });
    const revoked = rooms.join(
      created.snapshot.room.id,
      created.invitation,
      "Revoked",
    )!;
    rooms.revoke(created.snapshot.room.id, revoked.snapshot.participant.id);
    await backupRooms(source, target);
    verifyBackup(target);
    const restored = new Rooms(target);
    try {
      assert.ok(restored.session(created.snapshot.room.id, created.token));
      assert.equal(
        restored.session(created.snapshot.room.id, revoked.token),
        null,
      );
      assert.equal(restored.pending(created.snapshot.room.id), 1);
      assert.ok(
        restored.join(created.snapshot.room.id, created.invitation, "Friend"),
      );
    } finally {
      restored.close();
    }
    const before = readFileSync(target);
    await assert.rejects(backupRooms(source, target), /new backup/);
    assert.deepEqual(readFileSync(target), before);
  } finally {
    rooms.close();
    const resolved = realpathSync(root);
    assert.equal(dirname(resolved), realpathSync(tmpdir()));
    assert.ok(basename(resolved).startsWith("freedomstream-backup-"));
    rmSync(resolved, { recursive: true });
  }
});
