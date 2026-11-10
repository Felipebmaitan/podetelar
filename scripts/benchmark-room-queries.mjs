import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { performance, monitorEventLoopDelay } from "node:perf_hooks";
const root = resolve(import.meta.dirname, "..");
const before = resolve(root, ".local/experiments/server/baseline-rooms.mjs");
const after = resolve(root, "apps/server/dist/rooms.js");
const runs = [];
for (let pair = 1; pair <= 5; pair++) {
  for (const mode of pair % 2
    ? ["baseline", "prepared"]
    : ["prepared", "baseline"]) {
    const file = mode === "baseline" ? before : after;
    const { Rooms } = await import(`file:///${file.replaceAll("\\", "/")}`);
    let now = 1_700_000_000_000;
    const rooms = new Rooms(":memory:", () => now);
    const issued = rooms.create({
      name: "Synthetic query room",
      displayName: "Synthetic participant",
    });
    const other = rooms.create({
      name: "Other synthetic room",
      displayName: "Other participant",
    });
    const id = issued.snapshot.room.id;
    const token = issued.token;
    if (rooms.session(other.snapshot.room.id, token) !== null)
      throw new Error("Room separation regressed.");
    for (let i = 0; i < 5000; i++) rooms.session(id, token);
    const start = performance.now(),
      cpu = process.cpuUsage();
    for (let i = 0; i < 100_000; i++)
      if (!rooms.session(id, token))
        throw new Error("Valid authority rejected.");
    const elapsed = performance.now() - start,
      used = process.cpuUsage(cpu);
    rooms.leave(id, token);
    if (rooms.session(id, token) !== null)
      throw new Error("Revocation regressed.");
    now += 86_400_001;
    if (rooms.session(other.snapshot.room.id, other.token) !== null)
      throw new Error("Expiry regressed.");
    rooms.close();
    runs.push({
      pair,
      mode,
      queries: 100_000,
      wallMs: elapsed,
      cpuMs: (used.user + used.system) / 1000,
      revocationExpiryAndRoomSeparationPassed: true,
      sourceSha256: createHash("sha256")
        .update(readFileSync(file))
        .digest("hex"),
    });
  }
}
const output = resolve(root, ".local/experiments/server/queries.json");
writeFileSync(
  output,
  JSON.stringify(
    {
      schema: 1,
      runs,
      limits:
        "Actual Rooms.session implementation with fresh token hashes, room/session reads and current expiry on every call. In-memory SQLite query CPU microbenchmark; not production HTTP throughput or VPS capacity.",
    },
    null,
    2,
  ),
);
console.log(JSON.stringify({ output, runs: runs.length }));
