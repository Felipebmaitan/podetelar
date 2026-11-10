import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import { Usage, parseUsageMetrics } from "../src/usage.js";
import { Rooms } from "../src/rooms.js";
import { backupRooms } from "../src/backup.js";

test("usage counts actual deltas, keeps equal names separate, splits minute boundaries, and survives backup/restart", async () => {
  const dir = mkdtempSync(join(tmpdir(), "freedomstream-usage-"));
  const path = join(dir, "test.db");
  let now = Date.UTC(2026, 10, 9, 12, 0, 55);
  const rooms = new Rooms(path, () => now);
  const first = rooms.create({ name: "Friends", displayName: "Felipe" });
  const second = rooms.join(
    first.snapshot.room.id,
    first.invitation,
    "Felipe",
  )!;
  let usage = new Usage(path, "http://127.0.0.1:1/metrics", () => now);
  const counter = (bytes: number, other = 0) => [
    {
      room: first.snapshot.room.id,
      participant: first.snapshot.participant.id,
      direction: "incoming" as const,
      bytes,
    },
    {
      room: first.snapshot.room.id,
      participant: second.snapshot.participant.id,
      direction: "outgoing" as const,
      bytes: other,
    },
  ];
  try {
    usage.record(counter(1000), now);
    now += 10000;
    usage.record(counter(2000, 3000), now);
    let report = usage.report(now - 65000, now + 55000);
    assert.equal(report.uploadedBytes, 1000);
    assert.equal(report.downloadedBytes, 3000);
    assert.equal(report.users.length, 2);
    assert.equal(
      report.points.reduce((n, p) => n + p.durationMs, 0),
      10000,
    );
    assert.equal(
      report.points.filter((p) => p.uploadedBytes === 500).length,
      2,
    );
    const newcomer = rooms.join(
      first.snapshot.room.id,
      first.invitation,
      "Ana",
    )!;
    usage.remember(
      first.snapshot.room.id,
      newcomer.snapshot.participant.id,
      "Ana",
    );
    now += 5000;
    usage.record(
      [
        ...counter(2000, 3000),
        {
          room: first.snapshot.room.id,
          participant: newcomer.snapshot.participant.id,
          direction: "incoming",
          bytes: 700,
        },
      ],
      now,
    );
    assert.equal(usage.report(now - 65000, now + 55000).uploadedBytes, 1700);
    usage.record(counter(2000, 3000), now); // duplicate snapshot does not double count
    assert.equal(usage.report(now - 65000, now + 55000).uploadedBytes, 1700);
    now += 5000;
    usage.record(counter(10, 20), now, 2); // SFU restart, no negative/spike
    now += 5000;
    usage.record(counter(30, 60), now, 2);
    assert.equal(usage.report(now - 65000, now + 55000).uploadedBytes, 1720);
    now += 60000;
    usage.record(counter(90000, 90000), now, 2); // outage is unknown, no synthetic allocation
    report = usage.report(now - 120000, now + 60000);
    assert.equal(report.uploadedBytes, 1720);
    await backupRooms(path, join(dir, "backup.db"));
    await usage.close();
    usage = new Usage(
      join(dir, "backup.db"),
      "http://127.0.0.1:1/metrics",
      () => now,
    );
    report = usage.report(now - 120000, now + 60000);
    assert.equal(report.uploadedBytes, 1720);
    assert.equal(report.available, false);
    usage.record(counter(200000), now, 2);
    assert.equal(usage.report(now - 120000, now + 60000).uploadedBytes, 1720);
    assert.equal(usage.report(now + 3600000, now + 7200000).userCount, 0);
    await usage.sample();
    assert.equal(usage.report(now - 120000, now + 60000).available, false);
  } finally {
    await usage.close();
    rooms.close();
    const resolved = realpathSync(dir);
    assert.equal(dirname(resolved), realpathSync(tmpdir()));
    assert.ok(basename(resolved).startsWith("freedomstream-usage-"));
    rmSync(resolved, { recursive: true });
  }
});

test("the private scrape parser requires the instrumented SFU and accepts real Prometheus labels and scientific notation", () => {
  assert.throws(() => parseUsageMetrics("livekit_packet_bytes 100"));
  const result = parseUsageMetrics(
    `freedomstream_usage_enabled 1\nfreedomstream_usage_started_seconds 1794220000.123\nfreedomstream_participant_bytes_total{direction="incoming",participant="12345678-1234-4234-8234-123456789abc",room="freedomstream_abcdefghijklmnopqrstuv"} 1.25e+06\n`,
  );
  assert.equal(result.counters[0]?.bytes, 1250000);
  assert.equal(result.counters[0]?.room, "abcdefghijklmnopqrstuv");
});
