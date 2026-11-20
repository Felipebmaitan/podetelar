// Rejected narrower-authority prototype. Never changes product authentication.
import { createHash } from "node:crypto";
import { writeFileSync } from "node:fs";
import { Rooms } from "../apps/server/dist/rooms.js";
const runs = [];
for (let pair = 1; pair <= 5; ++pair) {
  let now = 1_700_000_000_000;
  const rooms = new Rooms(":memory:", () => now);
  const a = rooms.create({ name: "Synthetic validity", displayName: "First" });
  const b = rooms.create({
    name: "Other synthetic validity",
    displayName: "Second",
  });
  // TypeScript's private DB is accessible in this JS-only benchmark. No product
  // API is added and no token/hash or database content is exported.
  const scalar = rooms.db
    .prepare(`SELECT 1 FROM sessions AS s JOIN rooms AS r ON r.id = s.room_id
    WHERE s.token_hash = ? AND s.room_id = ? AND s.expires_at > ?`);
  const narrow = (id, token) =>
    !!scalar.get(createHash("sha256").update(token).digest(), id, now);
  const full = (id, token) => rooms.session(id, token) !== null;
  function equivalent(id, token) {
    if (narrow(id, token) !== full(id, token))
      throw new Error("Validity prototype changed authority.");
  }
  equivalent(a.snapshot.room.id, a.token);
  equivalent(b.snapshot.room.id, a.token);
  for (const mode of pair % 2
    ? ["prepared-session", "scalar-join"]
    : ["scalar-join", "prepared-session"]) {
    const action = mode === "prepared-session" ? full : narrow;
    for (let n = 0; n < 5000; ++n) action(a.snapshot.room.id, a.token);
    const started = performance.now(),
      cpu = process.cpuUsage();
    for (let n = 0; n < 100000; ++n)
      if (!action(a.snapshot.room.id, a.token))
        throw new Error("Valid membership rejected.");
    const used = process.cpuUsage(cpu);
    runs.push({
      pair,
      mode,
      calls: 100000,
      wallMs: performance.now() - started,
      cpuMs: (used.user + used.system) / 1000,
    });
  }
  rooms.leave(a.snapshot.room.id, a.token);
  equivalent(a.snapshot.room.id, a.token);
  now += 86400001;
  equivalent(b.snapshot.room.id, b.token);
  rooms.close();
}
const output = new URL(
  "../.local/experiments/server/scalar-validity.json",
  import.meta.url,
);
writeFileSync(
  output,
  JSON.stringify(
    {
      schema: 1,
      runs,
      revocationExpiryAndRoomSeparationPassed: true,
      decision:
        "Benchmark only. Retain the prepared-statement change; no second authentication API is introduced for a small per-transfer saving.",
      limits:
        "Fresh SHA-256 and current SQLite validity on every call. Boolean-only operation benchmark, not a replacement for session's full returned identity/room contract or measured HTTP throughput.",
    },
    null,
    2,
  ),
);
console.log("Saved the narrower validity operation comparison.");
