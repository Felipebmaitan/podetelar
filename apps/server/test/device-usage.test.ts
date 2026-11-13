import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { Usage } from "../src/usage.js";
import { Rooms } from "../src/rooms.js";
import { buildApp } from "../src/app.js";
import { loadConfig } from "../src/config.js";

const device = (number: number) =>
  `pc1_${number.toString(16).padStart(64, "0")}`;
function temporary() {
  const dir = mkdtempSync(join(tmpdir(), "freedomstream-device-"));
  return {
    path: join(dir, "test.db"),
    clean() {
      const resolved = realpathSync(dir);
      assert.equal(dirname(resolved), realpathSync(tmpdir()));
      assert.ok(basename(resolved).startsWith("freedomstream-device-"));
      rmSync(resolved, { recursive: true });
    },
  };
}

test("devices combine concurrent/reopened sessions across rooms; migration, legacy separation and restart preserve history", async () => {
  const temp = temporary();
  let now = Date.UTC(2026, 10, 13, 12);
  const rooms = new Rooms(temp.path, () => now);
  const first = rooms.create({ name: "First room", displayName: "Same name" });
  const second = rooms.create({ name: "Other room", displayName: "Renamed" });
  const other = rooms.join(
    first.snapshot.room.id,
    first.invitation,
    "Same name",
  )!;
  const legacy = rooms.join(
    first.snapshot.room.id,
    first.invitation,
    "Same name",
  )!;
  const before = new DatabaseSync(temp.path);
  before.exec(`CREATE TABLE usage_users (id TEXT PRIMARY KEY, room TEXT NOT NULL, name TEXT NOT NULL, admitted_at INTEGER NOT NULL DEFAULT 0);
    CREATE TABLE usage_minutes (minute INTEGER NOT NULL, user TEXT NOT NULL, uploaded REAL NOT NULL, downloaded REAL NOT NULL, PRIMARY KEY(minute,user));`);
  before
    .prepare("INSERT INTO usage_users VALUES (?,?,?,?)")
    .run(
      legacy.snapshot.participant.id,
      first.snapshot.room.id,
      "Same name",
      now,
    );
  before
    .prepare("INSERT INTO usage_minutes VALUES (?,?,?,?)")
    .run(now, legacy.snapshot.participant.id, 40, 60);
  before.close();
  let usage = new Usage(temp.path, "http://127.0.0.1:1/unused", () => now);
  // Existing sessions are intentionally unidentified. Only admissions observed
  // with an explicit device ID can acquire a device mapping.
  const admitted = rooms.join(
    first.snapshot.room.id,
    first.invitation,
    "Same name",
  )!;
  const reopened = rooms.join(
    second.snapshot.room.id,
    second.invitation,
    "Renamed",
  )!;
  usage.remember(
    first.snapshot.room.id,
    admitted.snapshot.participant.id,
    "Same name",
    device(1),
  );
  now += 1;
  usage.remember(
    second.snapshot.room.id,
    reopened.snapshot.participant.id,
    "Renamed",
    device(1),
  );
  // Another PC with the same display name must stay separate.
  const separate = rooms.join(
    first.snapshot.room.id,
    first.invitation,
    "Same name",
  )!;
  usage.remember(
    first.snapshot.room.id,
    separate.snapshot.participant.id,
    "Same name",
    device(2),
  );
  const counters = (n: number) =>
    [admitted, reopened, separate].flatMap((session, i) => [
      {
        room: session.snapshot.room.id,
        participant: session.snapshot.participant.id,
        direction: "incoming" as const,
        bytes: n * (i + 1),
      },
      {
        room: session.snapshot.room.id,
        participant: session.snapshot.participant.id,
        direction: "outgoing" as const,
        bytes: n * (i + 1) * 2,
      },
    ]);
  const from = now - 1,
    to = from + 60000;
  try {
    usage.record(counters(100), now);
    now += 5000;
    usage.record(counters(200), now);
    let report = usage.report(from, to);
    assert.equal(report.userCount, 3);
    const grouped = report.users.find((u) => u.id === device(1))!;
    assert.equal(grouped.sessionCount, 2);
    assert.equal(grouped.identityKind, "device");
    assert.equal(grouped.displayName, "Renamed");
    assert.equal(grouped.uploadedBytes, 300);
    assert.equal(grouped.downloadedBytes, 600);
    assert.equal(
      grouped.sessions.reduce((n, s) => n + s.uploadedBytes, 0),
      300,
    );
    assert.equal(
      grouped.points.reduce((n, p) => n + p.downloadedBytes, 0),
      600,
    );
    assert.equal(new Set(grouped.sessions.map((s) => s.roomId)).size, 2);
    assert.equal(report.uploadedBytes, 640);
    assert.equal(report.downloadedBytes, 1260);
    assert.equal(
      report.users.find((u) => u.id === legacy.snapshot.participant.id)
        ?.identityKind,
      "session",
    );
    // A duplicate remember cannot rewrite another session's device mapping.
    usage.remember(
      first.snapshot.room.id,
      admitted.snapshot.participant.id,
      "Changed",
      device(2),
    );
    assert.equal(
      usage.report(from, to).users.find((u) => u.id === device(1))
        ?.sessionCount,
      2,
    );
    assert.throws(() =>
      usage.remember("room", "participant", "Name", "raw-hardware-id"),
    );
    await usage.close();
    usage = new Usage(temp.path, "http://127.0.0.1:1/unused", () => now);
    report = usage.report(from, to);
    assert.equal(report.uploadedBytes, 640);
    assert.equal(report.users.find((u) => u.id === device(1))?.sessionCount, 2);
    assert.equal(
      report.users.some((u) => u.id === other.snapshot.participant.id),
      false,
    );
    now += 401 * 86400000;
    usage.record([], now, 2);
    assert.equal(usage.report(from, to).userCount, 0);
  } finally {
    await usage.close();
    rooms.close();
    temp.clean();
  }
});

test("pagination groups PCs before limiting results and session drill-down is bounded without dropping totals", async () => {
  const temp = temporary();
  let now = Date.UTC(2026, 10, 13, 12);
  const rooms = new Rooms(temp.path, () => now);
  const usage = new Usage(temp.path, "http://127.0.0.1:1/unused", () => now);
  const counters = [];
  try {
    for (let i = 0; i < 77; i++) {
      const session = rooms.create({ name: "Test", displayName: `Name ${i}` });
      usage.remember(
        session.snapshot.room.id,
        session.snapshot.participant.id,
        `Name ${i}`,
        device(i < 53 ? 1 : i - 51),
      );
      counters.push({
        room: session.snapshot.room.id,
        participant: session.snapshot.participant.id,
        direction: "incoming" as const,
        bytes: 0,
      });
    }
    usage.record(counters, now);
    now += 5000;
    usage.record(
      counters.map((c) => ({ ...c, bytes: 100 })),
      now,
    );
    const firstPage = usage.report(now - 5000, now + 55000);
    const secondPage = usage.report(now - 5000, now + 55000, 1);
    assert.equal(firstPage.userCount, 25);
    assert.equal(firstPage.users.length, 24);
    assert.equal(secondPage.users.length, 1);
    assert.equal(
      new Set([...firstPage.users, ...secondPage.users].map((u) => u.id)).size,
      25,
    );
    const grouped = firstPage.users[0]!;
    assert.equal(grouped.id, device(1));
    assert.equal(grouped.sessionCount, 53);
    assert.equal(grouped.sessions.length, 50);
    assert.equal(grouped.uploadedBytes, 5300);
    assert.equal(firstPage.uploadedBytes, 7700);
  } finally {
    await usage.close();
    rooms.close();
    temp.clean();
  }
});

test("admission validates and privately binds device identity without changing authorization or exposing it to participants", async () => {
  const temp = temporary();
  const origin = "https://rooms.example";
  const config = loadConfig({
    PUBLIC_URL: origin,
    ADMIN_TOKEN: "synthetic-administrator-key-not-for-production",
    LIVEKIT_API_KEY: "test-key",
    LIVEKIT_API_SECRET: "synthetic-media-secret-not-for-production",
    DATABASE_PATH: temp.path,
  });
  const app = await buildApp(config, { usagePolling: false });
  const headers = { origin, authorization: `Bearer ${config.adminToken}` };
  const db = new DatabaseSync(temp.path);
  try {
    const created = await app.inject({
      method: "POST",
      url: "/api/rooms",
      headers,
      payload: { name: "Test", displayName: "First", deviceId: device(1) },
    });
    assert.equal(created.statusCode, 201);
    const room = created.json();
    const invitation = new URL(room.invitationUrl).hash.slice(8);
    const join = (id: unknown, secret = invitation) =>
      app.inject({
        method: "POST",
        url: `/api/rooms/${room.room.id}/join`,
        headers: { origin },
        payload: { displayName: "Second", invitation: secret, deviceId: id },
      });
    const joined = await join(device(1));
    assert.equal(joined.statusCode, 200);
    assert.notEqual(joined.json().participant.id, room.participant.id);
    assert.ok(!created.body.includes(device(1)));
    assert.ok(!joined.body.includes(device(1)));
    const stored = db.prepare("SELECT device_id FROM usage_users").all();
    assert.deepEqual(
      stored.map((r) => r.device_id),
      [device(1), device(1)],
    );
    for (const id of [null, {}, "", "pc1_short", `pc1_${"F".repeat(64)}`])
      assert.equal((await join(id)).statusCode, 400);
    assert.equal((await join(device(1), "x".repeat(43))).statusCode, 404);
    assert.equal(
      (await app.inject(`/api/rooms/${room.room.id}/session`)).statusCode,
      401,
    );
    assert.equal(
      (await app.inject("/api/admin/usage?from=0&to=60000")).statusCode,
      401,
    );
    const legacy = await app.inject({
      method: "POST",
      url: `/api/rooms/${room.room.id}/join`,
      headers: { origin },
      payload: { displayName: "Legacy", invitation },
    });
    assert.equal(legacy.statusCode, 200);
    assert.equal(
      db
        .prepare("SELECT device_id FROM usage_users WHERE id=?")
        .get(legacy.json().participant.id)?.device_id,
      null,
    );
  } finally {
    db.close();
    await app.close();
    temp.clean();
  }
});
