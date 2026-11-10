import { resolve } from "node:path";
import { writeFileSync } from "node:fs";
import assert from "node:assert/strict";
import { Rooms } from "../apps/server/dist/rooms.js";
import { Usage } from "../apps/server/dist/usage.js";
import { reportCached, cacheStats } from "./experiments/usage-report-cache.mjs";
import {
  reportCachedJson,
  jsonCacheStats,
} from "./experiments/usage-report-json-cache.mjs";
const jsonMode = process.argv[2] === "json";
const cached = jsonMode
  ? (...args) => JSON.parse(reportCachedJson(...args))
  : reportCached;
const statsFor = jsonMode ? jsonCacheStats : cacheStats;
const out = resolve(
  import.meta.dirname,
  "../.local/experiments/deep-pass/usage",
);
const file = resolve(out, `cache-correctness-${Date.now()}.sqlite`);
const base = Date.UTC(2026, 10, 10, 12),
  minute = 60000;
let now = base;
const rooms = new Rooms(file, () => now);
const admitted = rooms.create({
  name: "Synthetic cache qualification",
  displayName: "Repeated name",
});
const roomId = admitted.snapshot.room.id,
  participantId = admitted.snapshot.participant.id;
rooms.close();
const usage = new Usage(file, "http://127.0.0.1:1/unused", () => now),
  checks = [];
const match = (name, from = base, to = base + 2 * minute, page = 0) => {
  assert.deepEqual(cached(usage, from, to, page), usage.report(from, to, page));
  checks.push(name);
};
match("empty response");
usage.remember(roomId, participantId, "Repeated name");
now = base + 58000;
usage.record(
  [
    {
      room: roomId,
      participant: participantId,
      direction: "incoming",
      bytes: 100,
    },
  ],
  now,
  1,
);
match("first sample freshness");
now += 5000;
usage.record(
  [
    {
      room: roomId,
      participant: participantId,
      direction: "incoming",
      bytes: 107,
    },
  ],
  now,
  1,
);
match("fractional cross-minute deltas invalidate overlapping report");
assert.equal(usage.report(base, base + 2 * minute).uploadedBytes, 7);
assert.ok(
  !Number.isInteger(
    usage.report(base, base + 2 * minute).points[0].uploadedBytes,
  ),
);
const mutated = cached(usage, base, base + minute);
mutated.users[0].displayName = "Caller mutation";
mutated.points[0].uploadedBytes = -1;
match("caller cannot mutate cached payload", base, base + minute);
now += 20000;
match("availability expires without a database write");
usage.available = false;
match("collector failure reflected without invalidation");
now += 5000;
usage.record(
  [
    {
      room: roomId,
      participant: participantId,
      direction: "incoming",
      bytes: 1000,
    },
  ],
  now,
  1,
);
const historicalFrom = base,
  historicalTo = base + minute;
match("closed history cached", historicalFrom, historicalTo);
const hits = statsFor(usage).hits;
now += 5000;
usage.record(
  [
    {
      room: roomId,
      participant: participantId,
      direction: "incoming",
      bytes: 1007,
    },
  ],
  now,
  1,
);
match(
  "outside-range write keeps closed report and fresh metadata",
  historicalFrom,
  historicalTo,
);
assert.ok(statsFor(usage).hits > hits);
match("live range updates after write");
now += 5000;
usage.record(
  [
    {
      room: roomId,
      participant: participantId,
      direction: "incoming",
      bytes: 1,
    },
  ],
  now,
  2,
);
match("counter reset and generation change");
usage.remember("room-1", "synthetic-other", "Repeated name");
match("remember invalidates report metadata");
for (let i = 0; i < 12; i++) {
  match(
    `bounded LRU range ${i}`,
    base - (i + 2) * minute,
    base - (i + 1) * minute,
  );
  assert.ok(statsFor(usage).entries <= 4);
  assert.ok(statsFor(usage).accountedBytes <= 2 * 1024 * 1024);
}
match("later empty user page", base, base + 2 * minute, 1);
now += 401 * 86400000;
usage.record([], now, 3);
match("retention pruning invalidates old data");
assert.equal(usage.report(base, base + 2 * minute).uploadedBytes, 0);
const stats = statsFor(usage);
await usage.close();
assert.equal(statsFor(usage), null);
checks.push("close clears cached state");
writeFileSync(
  resolve(
    out,
    jsonMode ? "json-cache-correctness.json" : "cache-correctness.json",
  ),
  JSON.stringify(
    {
      schema: 1,
      passed: true,
      jsonMode,
      checks,
      stats,
      limits:
        "Actual Usage writes, reports and fresh metadata versus bounded cache prototype; synthetic local database, no authorization bypass.",
    },
    null,
    2,
  ),
);
console.log(JSON.stringify({ passed: true, checks: checks.length, stats }));
