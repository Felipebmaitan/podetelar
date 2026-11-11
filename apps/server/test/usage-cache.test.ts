import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import { Usage } from "../src/usage.js";
import { Rooms } from "../src/rooms.js";

test("closed usage history avoids repeated queries while metadata, backfills and pruning stay fresh", async () => {
  const dir = mkdtempSync(join(tmpdir(), "freedomstream-usage-cache-"));
  const path = join(dir, "test.db");
  const base = Date.UTC(2026, 10, 10, 12);
  let now = base;
  class ObservedUsage extends Usage {
    queries = 0;
    override report(...args: Parameters<Usage["report"]>) {
      this.queries++;
      return super.report(...args);
    }
  }
  const rooms = new Rooms(path, () => now);
  const admitted = rooms.create({ name: "Friends", displayName: 'Ana "雪"' });
  const usage = new ObservedUsage(path, "http://127.0.0.1:1/unused", () => now);
  const counter = (bytes: number) => [
    {
      room: admitted.snapshot.room.id,
      participant: admitted.snapshot.participant.id,
      direction: "incoming" as const,
      bytes,
    },
  ];
  const expected = (from: number, to: number, page = 0) =>
    Usage.prototype.report.call(usage, from, to, page);
  const match = (from = base, to = base + 60000, page = 0) => {
    const value = JSON.parse(usage.reportJson(from, to, page));
    assert.deepEqual(value, expected(from, to, page));
    return value;
  };
  try {
    now = base + 58000;
    usage.record(counter(100), now);
    match();
    match();
    assert.equal(usage.queries, 2, "open minute must not be cached");
    now += 5000;
    usage.record(counter(107), now);
    const first = match();
    assert.ok(first.uploadedBytes > 0);
    assert.ok(
      !Number.isInteger(first.uploadedBytes),
      "preserve fractional minute split",
    );
    const filled = usage.queries;
    const point = first.points[0],
      user = first.users[0];
    assert.ok(point && user);
    point.uploadedBytes = -1;
    user.displayName = "caller mutation";
    assert.equal(match().users[0]?.displayName, 'Ana "雪"');
    assert.equal(
      usage.queries,
      filled,
      "immutable JSON hit avoids report queries",
    );
    now += 5000;
    usage.record(counter(114), now);
    assert.equal(match().lastSampleAt, now);
    assert.equal(
      usage.queries,
      filled,
      "outside-range writes keep closed history",
    );
    now += 20000;
    assert.equal(
      match().available,
      false,
      "availability expires on a cache hit",
    );
    usage.record(counter(200), now, 2);
    assert.equal(match().available, true);
    await usage.sample();
    assert.equal(
      match().available,
      false,
      "failed collector status is never cached",
    );

    // Delayed observations can update an already cached closed minute.
    usage.record(counter(300), base + 59500, 3);
    usage.record(counter(310), base + 60500, 3);
    const beforeBackfill = usage.queries;
    const updated = match();
    assert.equal(usage.queries, beforeBackfill + 1);
    assert.ok(updated.uploadedBytes > first.uploadedBytes);

    usage.remember("synthetic-room", "synthetic-other", "A different user");
    const beforeRemember = usage.queries;
    match();
    assert.equal(usage.queries, beforeRemember + 1);
    for (let i = 1; i <= 5; i++)
      match(base - i * 60000, base - (i - 1) * 60000);
    const beforeEvicted = usage.queries;
    match(base - 60000, base);
    assert.equal(
      usage.queries,
      beforeEvicted + 1,
      "fifth distinct range evicts the oldest of four",
    );
    match(base, base + 60000, 1);
    now += 401 * 86400000;
    usage.record([], now, 4);
    assert.equal(
      match().uploadedBytes,
      0,
      "retention pruning invalidates cached data",
    );
  } finally {
    await usage.close();
    rooms.close();
    const resolved = realpathSync(dir);
    assert.equal(dirname(resolved), realpathSync(tmpdir()));
    assert.ok(basename(resolved).startsWith("freedomstream-usage-cache-"));
    rmSync(resolved, { recursive: true });
  }
});
