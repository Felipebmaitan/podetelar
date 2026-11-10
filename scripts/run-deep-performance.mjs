// MIT. Sequential actual-app trials; one experimental variable per comparison.
import { spawn } from "node:child_process";
import { resolve } from "node:path";
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { setTimeout as delay } from "node:timers/promises";
const root = resolve(import.meta.dirname, ".."),
  family = process.argv[2] ?? "clip";
const hold = resolve(root, ".local/experiments/deep-pass/hold-benchmarks");
if (!process.argv.includes("--ignore-hold")) {
  if (existsSync(hold))
    console.log(JSON.stringify({ family, waitingForValidationGap: true }));
  while (existsSync(hold)) await delay(1000);
}
const exe = resolve(
  root,
  process.argv[3] ?? ".local/experiments/deep-pass/e1-room.exe",
);
const index = resolve(
  root,
  `.local/experiments/deep-pass/${family}-cases.json`,
);
if (existsSync(index))
  throw Error("Choose a new family label; existing evidence is immutable.");
const runs = [];
const common = {
  ...process.env,
  FREEDOMSTREAM_IMAGE_POOL: "1",
  FREEDOMSTREAM_IMAGE_POOL_LIMIT: "2",
  FREEDOMSTREAM_DEEP_CLIP: "0",
  FREEDOMSTREAM_DEEP_PLANAR: "0",
  FREEDOMSTREAM_DEEP_SHARED_DEVICE: "0",
  FREEDOMSTREAM_DEEP_RELAXED_MAP: "0",
  FREEDOMSTREAM_DEEP_POOL_TRACE: "0",
  FREEDOMSTREAM_AV1_DECODER: "",
  FREEDOMSTREAM_FRAME_REPAINT: "0",
  FREEDOMSTREAM_IMAGE_FALLBACK: "0",
  FREEDOMSTREAM_DEEP_FLUSH_MAP: "0",
  FREEDOMSTREAM_DEEP_HEAP: "0",
  GST_DEBUG: "0",
  FREEDOMSTREAM_COUNT_SDK_LOGS: "0",
  FREEDOMSTREAM_DEEP_NARROW: "0",
  FREEDOMSTREAM_DEEP_SCROLL: "0",
  FREEDOMSTREAM_DEEP_MIXED: "0",
  FREEDOMSTREAM_DEEP_OWNERS: "0",
  FREEDOMSTREAM_BENCH_CONTENTION: "0",
  FREEDOMSTREAM_BENCH_ROUNDS: "1",
  FREEDOMSTREAM_BENCH_STEADY: "0",
  FREEDOMSTREAM_BENCH_AUDIO: "0",
  FREEDOMSTREAM_BENCH_PUBLISH: "0",
  FREEDOMSTREAM_BENCH_FPS: "60",
  FREEDOMSTREAM_BENCH_RESOLUTION: "p1080",
};
const groups =
  family === "clip-gl-confirm"
    ? [{ name: "narrow", pairs: 1, env: { FREEDOMSTREAM_DEEP_NARROW: "1" } }]
    : family === "clip"
      ? [
          { name: "narrow", pairs: 3, env: { FREEDOMSTREAM_DEEP_NARROW: "1" } },
          { name: "wide", pairs: 1, env: {} },
        ]
      : family === "clip-control"
        ? [
            {
              name: "narrow",
              pairs: 3,
              env: { FREEDOMSTREAM_DEEP_NARROW: "1" },
            },
            { name: "wide", pairs: 3, env: {} },
            {
              name: "scroll",
              pairs: 1,
              env: {
                FREEDOMSTREAM_DEEP_NARROW: "1",
                FREEDOMSTREAM_DEEP_SCROLL: "1",
              },
            },
            {
              name: "audio",
              pairs: 1,
              env: {
                FREEDOMSTREAM_DEEP_NARROW: "1",
                FREEDOMSTREAM_BENCH_AUDIO: "1",
              },
            },
            {
              name: "five",
              count: 5,
              pairs: 1,
              env: { FREEDOMSTREAM_DEEP_NARROW: "1" },
            },
          ]
        : family === "clip-wide-sustained"
          ? [
              {
                name: "wide",
                pairs: 3,
                env: {
                  FREEDOMSTREAM_BENCH_STEADY: "60",
                  FREEDOMSTREAM_PERFORMANCE_PUBLISHER: resolve(
                    root,
                    ".local/experiments/final-selected-v4/freedomstream-publisher.exe",
                  ),
                },
              },
            ]
          : family === "calibration"
            ? [
                { name: "four", pairs: 2, env: {} },
                { name: "one", pairs: 1, env: {} },
              ]
            : family === "planar"
              ? [
                  { name: "four", pairs: 3, env: {} },
                  { name: "one", pairs: 3, env: {} },
                  {
                    name: "four4k",
                    pairs: 1,
                    env: { FREEDOMSTREAM_BENCH_RESOLUTION: "p2160" },
                  },
                ]
              : family === "planar-final"
                ? [
                    { name: "four", pairs: 3, env: {} },
                    { name: "one", pairs: 3, env: {} },
                    {
                      name: "one4k",
                      pairs: 2,
                      env: { FREEDOMSTREAM_BENCH_RESOLUTION: "p2160" },
                    },
                    {
                      name: "four4k",
                      pairs: 1,
                      env: { FREEDOMSTREAM_BENCH_RESOLUTION: "p2160" },
                    },
                  ]
                : family === "planar-equal"
                  ? [
                      {
                        name: "four30",
                        pairs: 3,
                        env: { FREEDOMSTREAM_BENCH_FPS: "30" },
                      },
                      {
                        name: "one4k30",
                        pairs: 3,
                        env: {
                          FREEDOMSTREAM_BENCH_FPS: "30",
                          FREEDOMSTREAM_BENCH_RESOLUTION: "p2160",
                        },
                      },
                    ]
                  : family === "planar-codecs"
                    ? [
                        { name: "h264", codec: "h264", pairs: 1, env: {} },
                        { name: "av1", codec: "av1", pairs: 1, env: {} },
                        {
                          name: "av1sw",
                          codec: "av1-software",
                          pairs: 1,
                          env: {},
                        },
                      ]
                    : family === "pool-observation"
                      ? [
                          { name: "four", pairs: 1, env: {} },
                          {
                            name: "four4k",
                            pairs: 1,
                            env: { FREEDOMSTREAM_BENCH_RESOLUTION: "p2160" },
                          },
                        ]
                      : ["shared-device", "relaxed-map"].includes(family)
                        ? [
                            { name: "four", pairs: 3, env: {} },
                            {
                              name: "one4k",
                              pairs: 1,
                              env: { FREEDOMSTREAM_BENCH_RESOLUTION: "p2160" },
                            },
                            {
                              name: "mixed",
                              pairs: 1,
                              env: { FREEDOMSTREAM_DEEP_MIXED: "1" },
                            },
                            {
                              name: "combined",
                              pairs: 1,
                              env: {
                                FREEDOMSTREAM_BENCH_PUBLISH: "1",
                                FREEDOMSTREAM_BENCH_AUDIO: "1",
                              },
                            },
                          ]
                        : family === "ownership"
                          ? [
                              {
                                name: "four",
                                pairs: 1,
                                seconds: 20,
                                env: {
                                  FREEDOMSTREAM_DEEP_OWNERS: "1",
                                  FREEDOMSTREAM_BENCH_ROUNDS: "7",
                                  FREEDOMSTREAM_PERFORMANCE_PUBLISHER: resolve(
                                    root,
                                    ".local/experiments/final-selected-v4/freedomstream-publisher.exe",
                                  ),
                                },
                              },
                            ]
                          : [];
if (!groups.length) throw Error("Unknown comparison family.");
for (const group of groups)
  for (let pair = 1; pair <= group.pairs; pair++)
    for (const variant of pair % 2
      ? ["before", "after"]
      : ["after", "before"]) {
      const label = `deep-${family}-${group.name}-${variant}-${pair}`;
      const env = { ...common, ...group.env };
      const key = family.startsWith("clip")
        ? "FREEDOMSTREAM_DEEP_CLIP"
        : family === "shared-device"
          ? "FREEDOMSTREAM_DEEP_SHARED_DEVICE"
          : family === "relaxed-map"
            ? "FREEDOMSTREAM_DEEP_RELAXED_MAP"
            : family === "ownership"
              ? "FREEDOMSTREAM_DEEP_HEAP"
              : family === "pool-observation"
                ? "FREEDOMSTREAM_DEEP_POOL_TRACE"
                : "FREEDOMSTREAM_DEEP_PLANAR";
      if (family !== "calibration") env[key] = variant === "after" ? "1" : "0";
      if (family === "relaxed-map" && variant === "after")
        env.FREEDOMSTREAM_DEEP_FLUSH_MAP = "1";
      const count = group.count ?? (group.name.startsWith("one") ? 1 : 4);
      const caseExe =
        family === "calibration" && variant === "before"
          ? resolve(root, ".local/experiments/deep-pass/e1-room.exe")
          : exe;
      const child = spawn(
        process.execPath,
        [
          resolve(root, "scripts/local-performance-room.mjs"),
          label,
          String(count),
          group.codec ?? "h265",
          caseExe,
          String(group.seconds ?? 10),
        ],
        {
          cwd: root,
          env,
          windowsHide: true,
          stdio: ["ignore", "pipe", "pipe"],
        },
      );
      let stdout = "",
        stderr = "";
      child.stdout.on("data", (b) => (stdout = (stdout + b).slice(-32000)));
      child.stderr.on("data", (b) => (stderr = (stderr + b).slice(-32000)));
      const code = await new Promise((r) => {
        child.once("exit", r);
        child.once("error", () => r(-1));
      });
      const parsed = stdout
        .trim()
        .split(/\r?\n/)
        .flatMap((s) => {
          try {
            return [JSON.parse(s)];
          } catch {
            return [];
          }
        });
      const result = parsed.find((r) => r.completed && r.result)?.result;
      runs.push({
        label,
        family,
        group: group.name,
        pair,
        variant,
        code,
        result: result ?? null,
      });
      writeFileSync(index, JSON.stringify({ schema: 1, exe, runs }, null, 2));
      console.log(JSON.stringify(runs.at(-1)));
      if (code !== 0 || !result)
        throw Error("Actual-room case failed; safe case index retained.");
      const data = JSON.parse(readFileSync(result));
      if (!data.app?.completed || !data.codecStable)
        throw Error("Completion/codec requirement failed.");
      if (process.argv.includes("--require-gl")) {
        const meters = data.app.samples.map((s) => s.owners?.rgbaGl);
        if (!meters.length || meters.some((m) => !m || m.tagOverflow !== 0))
          throw Error("GL observer missing or tag bound exceeded");
        if (
          !(family.startsWith("planar") && variant === "after") &&
          meters.at(-1).calls <= 0
        )
          throw Error("RGBA path issued no observed video uploads");
        if (
          family.startsWith("planar") &&
          variant === "after" &&
          !data.app.samples.some((s) => s.planarGlUpdates > 0)
        )
          throw Error("Planar path issued no observed video uploads");
      }
    }
console.log(JSON.stringify({ completed: family, cases: runs.length, index }));
