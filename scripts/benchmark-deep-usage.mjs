// MIT. Fixed synthetic history; full response equivalence before timing.
import { DatabaseSync } from "node:sqlite";
import { mkdirSync, writeFileSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { createHash } from "node:crypto";
import { performance } from "node:perf_hooks";
import assert from "node:assert/strict";
import { reportBatched } from "./experiments/usage-report.mjs";

const root = resolve(import.meta.dirname, ".."),
  out = resolve(root, ".local/experiments/deep-pass/usage");
mkdirSync(out, { recursive: true });
const source = resolve(root, "apps/server/dist/usage.js");
const { Usage } = await import(pathToFileURL(source));
const file = resolve(out, `fixture-${Date.now()}.sqlite`);
const { Rooms } = await import(
  pathToFileURL(resolve(root, "apps/server/dist/rooms.js"))
);
const rooms = new Rooms(file);
rooms.close();
const now = Date.UTC(2026, 10, 10, 12),
  day = 86400000,
  minute = 60000;
const usage = new Usage(file, "http://127.0.0.1:1/unused", () => now);
const seed = new DatabaseSync(file);
seed.exec("BEGIN");
const user = seed.prepare(
  "INSERT INTO usage_users(id,room,name,admitted_at,device_id) VALUES (?,?,?,?,?)",
);
const row = seed.prepare("INSERT INTO usage_minutes VALUES (?,?,?,?)");
const cov = seed.prepare("INSERT OR IGNORE INTO usage_coverage VALUES (?,?)");
let rows = 0;
for (let u = 0; u < 30; u++) {
  const id = `synthetic-${String(u).padStart(2, "0")}`;
  user.run(
    id,
    `room-${u % 3}`,
    `Repeated name ${u % 4}`,
    now - 400 * day + u,
    u < 4 ? `pc1_${String((u % 2) + 1).padStart(64, "0")}` : null,
  );
  for (let d = 0; d < (u < 5 ? 400 : 7); d++)
    for (let m = 0; m < 240; m++) {
      const at = now - (d + 1) * day + m * minute;
      row.run(at, id, 1000 + (m % 11), 2000 + (u % 3));
      cov.run(at, m % 17 === 0 ? 30000 : 60000);
      rows++;
    }
}
seed.exec("COMMIT");
seed.close();
const cases = [
  { name: "small-1d", from: now - day, to: now, page: 0 },
  { name: "history-30d", from: now - 30 * day, to: now, page: 0 },
  { name: "history-366d", from: now - 366 * day, to: now, page: 0 },
  { name: "second-page", from: now - 7 * day + 123, to: now - 234, page: 1 },
  { name: "empty-page", from: now - 366 * day, to: now, page: 2 },
  { name: "empty-range", from: now, to: now + minute, page: 0 },
];
const results = [];
for (const c of cases) {
  const before = usage.report(c.from, c.to, c.page),
    after = reportBatched(usage, c.from, c.to, c.page);
  assert.deepEqual(after, before, `${c.name}: full response differs`);
  for (let i = 0; i < 2; i++) {
    usage.report(c.from, c.to, c.page);
    reportBatched(usage, c.from, c.to, c.page);
  }
  for (let pair = 1; pair <= 5; pair++)
    for (const variant of pair % 2
      ? ["baseline", "batched"]
      : ["batched", "baseline"]) {
      const cpu = process.cpuUsage(),
        start = performance.now();
      const report =
        variant === "baseline"
          ? usage.report(c.from, c.to, c.page)
          : reportBatched(usage, c.from, c.to, c.page);
      const wallMs = performance.now() - start,
        used = process.cpuUsage(cpu);
      assert.deepEqual(report, before);
      results.push({
        case: c.name,
        pair,
        variant,
        wallMs,
        cpuMs: (used.user + used.system) / 1000,
        users: report.users.length,
        points: report.points.length,
        fullResponseEquivalent: true,
        outputSha256: createHash("sha256")
          .update(JSON.stringify(report))
          .digest("hex"),
      });
    }
  console.log(JSON.stringify({ completed: c.name, pairs: 5 }));
}
await usage.close();
const output = resolve(out, "queries.json");
writeFileSync(
  output,
  JSON.stringify(
    {
      schema: 1,
      fixture: {
        synthetic: true,
        file,
        now,
        rows,
        sessions: 30,
        devices: 2,
        unidentifiedSessions: 26,
        days: 400,
      },
      baselineSourceSha256: createHash("sha256")
        .update(readFileSync(source))
        .digest("hex"),
      candidateSourceSha256: createHash("sha256")
        .update(
          readFileSync(resolve(root, "scripts/experiments/usage-report.mjs")),
        )
        .digest("hex"),
      runs: results,
      limits:
        "Full actual Usage.report versus isolated equivalent batching. Warm local SQLite, not HTTP/VPS throughput; no app default changed.",
    },
    null,
    2,
  ),
);
console.log(JSON.stringify({ output, runs: results.length }));
