// Sequential, isolated actual-app comparisons. Never benchmark two cases at once.
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";

const suite = process.argv[2] ?? "receive";
if (existsSync(".local/experiments/stop-before-next-suite")) {
  console.log(`Stopped at the requested experiment boundary before ${suite}.`);
  process.exit(20);
}
const baseline =
  process.env.FREEDOMSTREAM_BENCH_BASELINE ??
  ".local/experiments/baseline-shared/room-benchmark.exe";
const candidate =
  process.env.FREEDOMSTREAM_BENCH_CANDIDATE ??
  ".local/experiments/deferred-app/room-benchmark.exe";
const cases = [];
const labelPrefix = process.env.FREEDOMSTREAM_BENCH_LABEL_PREFIX ?? "";
if (!/^[a-z0-9-]{0,12}$/.test(labelPrefix))
  throw new Error("Invalid comparison label prefix.");
function add(label, count, codec, exe, env = {}, seconds = 10) {
  cases.push({
    label: `${labelPrefix}${label}`,
    count,
    codec,
    exe,
    env,
    seconds,
  });
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
const selectedCache =
  process.env.FREEDOMSTREAM_BENCH_CACHE_POLICY === "one"
    ? {
        ...pool,
        FREEDOMSTREAM_IMAGE_POOL_LIMIT: "1",
        FREEDOMSTREAM_IMAGE_FALLBACK: "1",
      }
    : pool;
const accepted =
  process.env.FREEDOMSTREAM_BENCH_REPAINT === "1"
    ? { ...selectedCache, FREEDOMSTREAM_FRAME_REPAINT: "1" }
    : selectedCache;
if (suite === "staging") {
  const control = ".local/experiments/final-selected-v2/room-benchmark.exe";
  pairs("staging-four", 4, "h265", control, candidate, pool, pool, 1);
  pairs(
    "staging-4k-one",
    1,
    "h265",
    control,
    candidate,
    { ...pool, FREEDOMSTREAM_BENCH_RESOLUTION: "p2160" },
    { ...pool, FREEDOMSTREAM_BENCH_RESOLUTION: "p2160" },
    1,
  );
} else if (suite === "receive") {
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
} else if (suite === "fallback") {
  const after = {
    ...accepted,
    FREEDOMSTREAM_IMAGE_POOL_LIMIT: "1",
    FREEDOMSTREAM_IMAGE_FALLBACK: "1",
  };
  pairs(
    "single-cache-fallback",
    4,
    "h265",
    candidate,
    candidate,
    accepted,
    after,
  );
  pairs(
    "single-cache-fallback-4k",
    4,
    "h265",
    candidate,
    candidate,
    { ...accepted, FREEDOMSTREAM_BENCH_RESOLUTION: "p2160" },
    { ...after, FREEDOMSTREAM_BENCH_RESOLUTION: "p2160" },
    1,
  );
} else if (suite === "final") {
  pairs("final-four", 4, "h265", baseline, candidate, {}, accepted);
  pairs("final-one", 1, "h265", baseline, candidate, {}, accepted);
  for (const codec of ["h264", "av1", "av1-software"])
    pairs(
      `final-${codec}-four`,
      4,
      codec,
      baseline,
      candidate,
      {},
      accepted,
      1,
    );
  pairs(
    "final-4k-one",
    1,
    "h265",
    baseline,
    candidate,
    { FREEDOMSTREAM_BENCH_RESOLUTION: "p2160" },
    { ...accepted, FREEDOMSTREAM_BENCH_RESOLUTION: "p2160" },
    2,
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
    [
      "combined-audio",
      4,
      { FREEDOMSTREAM_BENCH_PUBLISH: "1", FREEDOMSTREAM_BENCH_AUDIO: "1" },
    ],
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
} else if (suite === "last-sample") {
  const control =
    process.env.FREEDOMSTREAM_BENCH_CONTROL ??
    ".local/experiments/fallback/room-benchmark.exe";
  for (const count of [1, 4])
    pairs(
      `gpu-last-sample-${count}`,
      count,
      "h265",
      control,
      candidate,
      accepted,
      accepted,
      1,
    );
} else if (suite === "endurance") {
  add("steady-one-before", 1, "h265", baseline, {
    FREEDOMSTREAM_BENCH_STEADY: "180",
  });
  add("steady-one-after", 1, "h265", candidate, {
    ...accepted,
    FREEDOMSTREAM_BENCH_STEADY: "180",
  });
  add("steady-four-before", 4, "h265", baseline, {
    FREEDOMSTREAM_BENCH_STEADY: "360",
  });
  add("steady-four-after", 4, "h265", candidate, {
    ...accepted,
    FREEDOMSTREAM_BENCH_STEADY: "360",
  });
  add(
    "cycles-four-before",
    4,
    "h265",
    baseline,
    { FREEDOMSTREAM_BENCH_ROUNDS: "7" },
    20,
  );
  add(
    "cycles-four-after",
    4,
    "h265",
    candidate,
    { ...accepted, FREEDOMSTREAM_BENCH_ROUNDS: "7" },
    20,
  );
  add(
    "logging-rate-four",
    4,
    "h265",
    candidate,
    { ...accepted, FREEDOMSTREAM_COUNT_SDK_LOGS: "1" },
    5,
  );
  add("final-4k-four-capacity", 4, "h265", candidate, {
    ...accepted,
    FREEDOMSTREAM_BENCH_RESOLUTION: "p2160",
  });
  add(
    "cli-final-4k-four",
    4,
    "h265",
    resolve(dirname(candidate), "freedomstream-publisher.exe"),
    { FREEDOMSTREAM_BENCH_RESOLUTION: "p2160", FREEDOMSTREAM_BENCH_CLI: "1" },
    5,
  );
} else if (suite === "tokio") {
  pairs("deferred-tokio", 4, "av1-software", candidate, candidate, accepted, {
    ...accepted,
    FREEDOMSTREAM_BENCH_TOKIO: "4",
  });
} else throw new Error("Unknown comparison suite.");

const failures = [];
for (const c of cases) {
  const env = { ...process.env };
  for (const key of Object.keys(env))
    if (
      /^FREEDOMSTREAM_(BENCH_|IMAGE_POOL|IMAGE_FALLBACK|FRAME_REPAINT|CONVERSION_WORKERS)/.test(
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
  if (code === 20) throw new Error(`Stopped before comparison: ${c.label}`);
  if (code !== 0) failures.push({ label: c.label, code });
}
console.log(
  JSON.stringify({
    suite,
    attemptedCases: cases.length,
    completedCases: cases.length - failures.length,
    failures,
  }),
);
if (failures.length) process.exitCode = 1;
