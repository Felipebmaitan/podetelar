// Sequential, isolated actual-app comparisons. Never benchmark two cases at once.
import { spawn } from "node:child_process";
import { resolve } from "node:path";

const suite = process.argv[2] ?? "receive";
const baseline =
  process.env.FREEDOMSTREAM_BENCH_BASELINE ??
  ".local/experiments/baseline-shared/room-benchmark.exe";
const candidate =
  process.env.FREEDOMSTREAM_BENCH_CANDIDATE ??
  ".local/experiments/deferred-app/room-benchmark.exe";
const cases = [];
function add(label, count, codec, exe, env = {}, seconds = 10) {
  cases.push({ label, count, codec, exe, env, seconds });
}
function pairs(
  name,
  count,
  codec,
  before,
  after,
  beforeEnv = {},
  afterEnv = {},
  repetitions = 3,
) {
  for (let pair = 1; pair <= repetitions; pair++) {
    const rows = [
      [`${name}-before-${pair}`, before, beforeEnv],
      [`${name}-after-${pair}`, after, afterEnv],
    ];
    if (pair % 2 === 0) rows.reverse();
    for (const [label, exe, env] of rows) add(label, count, codec, exe, env);
  }
}
const pool = {
  FREEDOMSTREAM_IMAGE_POOL: "1",
  FREEDOMSTREAM_IMAGE_POOL_LIMIT:
    process.env.FREEDOMSTREAM_BENCH_POOL_LIMIT ?? "2",
};
const driven = { ...pool, FREEDOMSTREAM_FRAME_REPAINT: "1" };
const accepted =
  process.env.FREEDOMSTREAM_BENCH_REPAINT === "1" ? driven : pool;
if (suite === "receive") {
  add("deferred-smoke", 4, "h265", candidate, {}, 5);
  pairs("deferred-pool", 4, "h265", candidate, candidate, {}, pool, 2);
  pairs("deferred-repaint60", 4, "h265", candidate, candidate, pool, driven, 2);
  pairs(
    "deferred-repaint15",
    4,
    "h265",
    candidate,
    candidate,
    { ...pool, FREEDOMSTREAM_BENCH_FPS: "15" },
    { ...driven, FREEDOMSTREAM_BENCH_FPS: "15" },
    2,
  );
} else if (suite === "pool-limit") {
  pairs("deferred-pool-limit", 4, "h265", candidate, candidate, accepted, {
    ...accepted,
    FREEDOMSTREAM_IMAGE_POOL_LIMIT: "1",
  });
} else if (suite === "i420-app") {
  pairs(
    "deferred-i420-reuse",
    4,
    "h265",
    ".local/experiments/deferred-registry/room-benchmark.exe",
    candidate,
    accepted,
    accepted,
  );
} else if (suite === "application") {
  pairs("accepted-four", 4, "h265", baseline, candidate, {}, accepted);
  pairs("accepted-one", 1, "h265", baseline, candidate, {}, accepted);
  for (const codec of ["h264", "av1", "av1-software"])
    for (const count of [1, 4])
      pairs(
        `accepted-${codec}-${count}`,
        count,
        codec,
        baseline,
        candidate,
        {},
        accepted,
        1,
      );
  for (const count of [1, 4])
    pairs(
      `accepted-4k-${count}`,
      count,
      "h265",
      baseline,
      candidate,
      { FREEDOMSTREAM_BENCH_RESOLUTION: "p2160" },
      { ...accepted, FREEDOMSTREAM_BENCH_RESOLUTION: "p2160" },
      1,
    );
} else if (suite === "lifecycle") {
  for (const [name, count, env] of [
    ["disconnected", 0, { FREEDOMSTREAM_BENCH_DISCONNECTED: "1" }],
    ["empty", 0, {}],
    ["publish", 0, { FREEDOMSTREAM_BENCH_PUBLISH: "1" }],
    ["combined", 4, { FREEDOMSTREAM_BENCH_PUBLISH: "1" }],
    ["audio-four", 4, { FREEDOMSTREAM_BENCH_AUDIO: "1" }],
    ["contention-four", 4, { FREEDOMSTREAM_BENCH_CONTENTION: "1" }],
  ])
    pairs(
      `accepted-${name}`,
      count,
      "h265",
      baseline,
      candidate,
      env,
      { ...accepted, ...env },
      1,
    );
} else if (suite === "tokio") {
  pairs("deferred-tokio", 4, "av1-software", candidate, candidate, accepted, {
    ...accepted,
    FREEDOMSTREAM_BENCH_TOKIO: "4",
  });
} else throw new Error("Unknown comparison suite.");

for (const c of cases) {
  const env = { ...process.env };
  for (const key of Object.keys(env))
    if (
      /^FREEDOMSTREAM_(BENCH_|IMAGE_POOL|FRAME_REPAINT|CONVERSION_WORKERS)/.test(
        key,
      )
    )
      delete env[key];
  Object.assign(env, c.env);
  const child = spawn(
    process.execPath,
    [
      resolve("scripts/local-performance-room.mjs"),
      c.label,
      String(c.count),
      c.codec,
      c.exe,
      String(c.seconds),
    ],
    { env, windowsHide: true, stdio: ["ignore", "inherit", "inherit"] },
  );
  const code = await new Promise((done, reject) => {
    child.once("error", reject);
    child.once("close", done);
  });
  if (code !== 0) throw new Error(`Comparison failed: ${c.label}`);
}
console.log(JSON.stringify({ suite, completedCases: cases.length }));
