// Compact, sanitized evidence for the Markdown ledger. Only named measurement
// files are read; generated room configurations, databases and logs are excluded.
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { resolve, join } from "node:path";

const root = resolve(import.meta.dirname, "../.local/experiments");
const evidence = [];
const read = (path) => {
  const file = join(root, path),
    bytes = readFileSync(file);
  evidence.push({
    path,
    sha256: createHash("sha256").update(bytes).digest("hex"),
  });
  return JSON.parse(bytes.toString("utf8").replace(/^\uFEFF/, ""));
};
const median = (a) => {
  const values = a.filter(Number.isFinite).sort((a, b) => a - b);
  return values[Math.floor(values.length / 2)] ?? null;
};
const pick = (value, keys) =>
  Object.fromEntries(keys.filter((k) => k in value).map((k) => [k, value[k]]));
const directories = readdirSync(root, { withFileTypes: true })
  .filter((e) => e.isDirectory())
  .map((e) => e.name);
const all = (directory, pattern) =>
  readdirSync(join(root, directory))
    .filter((f) => pattern.test(f))
    .sort();
const measureOs = (samples) => ({
  sampleCount: samples.length,
  privateMiB: median(samples.map((s) => s.privateBytes / 2 ** 20)),
  workingSetMiB: median(samples.map((s) => s.workingSetBytes / 2 ** 20)),
  dedicatedGpuMiB: median(
    samples.map((s) => s.gpuMemoryBytes?.["dedicated usage"] / 2 ** 20),
  ),
  threads: median(samples.map((s) => s.threads)),
  handles: median(samples.map((s) => s.handles)),
  gpuEnginesPercent: Object.fromEntries(
    Object.keys(samples[0]?.gpuEnginesPercent ?? {}).map((k) => [
      k,
      median(samples.map((s) => s.gpuEnginesPercent?.[k])),
    ]),
  ),
});
const result = {
  schema: 1,
  units: {
    statistics:
      "Summary medians select the upper middle observation for even sample counts. Individual run values and repetition counts remain available.",
    cpu: "One full CPU core = 100%. CPU ms are process CPU time, not elapsed time.",
    memory:
      "MiB = 2^20 bytes. Private bytes are process commit; working set is resident private/shared pages. Neither is EXE size.",
    gpu: "Per-process driver counters; separate engines are not added together.",
    frames:
      "Decoded/conversion/texture-upload counts are distinct; none establishes physical presented FPS.",
  },
  artifacts: existsSync(join(root, "final-selected-v4/source.json"))
    ? read("final-selected-v4/source.json")
    : null,
  artifactHistory: [],
  capacityDiagnostics: [],
  lifecycle: [],
  failedAttempts: [],
  wholeApp: read("ui-summary.json"),
  portable: [],
  nativeGpu: [],
  gpuLeases: [],
  gpuCounters: [],
  textures: [],
  operations: [],
  pipelines: [],
  decoders: [],
  i420Pools: [],
  readbackStaging: [],
  sdkIdle: [],
  sdkLogging: [],
  sdkChurn: [],
  serverQueries: read("server/queries.json"),
  serverMedia: [],
  serverHttp: [],
  quality: [],
  serverScalarValidity: existsSync(join(root, "server/scalar-validity.json"))
    ? read("server/scalar-validity.json")
    : null,
};
for (const directory of [
  "baseline-shared",
  "baseline-steady",
  "fallback",
  "no-last",
  "final-selected",
  "accepted",
  "final-selected-v2",
  "final-selected-v3",
  "final-selected-v4",
])
  if (existsSync(join(root, directory, "source.json")))
    result.artifactHistory.push({
      directory,
      report: read(`${directory}/source.json`),
    });
for (const run of result.wholeApp.runs) {
  const path = `${run.directory}/result.json`,
    raw = read(path);
  run.rawSha256 = evidence.at(-1).sha256;
  run.appCompleted = raw.app.completed;
  run.codecs = [
    ...new Set(raw.app.samples.flatMap((s) => s.videoCodecs ?? [])),
  ];
  if (raw.contentionPipeline) run.contentionPipeline = raw.contentionPipeline;
  if (raw.contentionProcess)
    run.contentionProcess = measureOs(raw.contentionProcess.samples);
  if (run.rounds > 1) {
    result.lifecycle.push({
      directory: run.directory,
      label: run.label,
      appSha256: run.appSha256,
      rounds: run.rounds,
      wallSeconds: raw.app.wallSeconds,
      codecStable: run.codecStable,
      appCompleted: raw.app.completed,
      sampledErrors: run.rows.some((row) => row.errors),
      phases: ["grid", "minimized", "stopped"].map((phase) => {
        const rows = run.rows.filter((row) => row.phase === phase);
        const first = rows[0],
          last = rows.at(-1);
        return {
          phase,
          cycles: rows.map((row) =>
            pick(row, [
              "cycle",
              "cpuCorePercent",
              "privateMiB",
              "workingSetMiB",
              "threads",
              "handles",
              "decodedFramesPerSecond",
              "textureUploadsPerSecond",
              "activeWatches",
              "errors",
              "settledStoppedTail",
              "counterDiscontinuities",
            ]),
          ),
          lastMinusFirst: Object.fromEntries(
            ["privateMiB", "workingSetMiB", "threads", "handles"].map((key) => [
              key,
              Number.isFinite(first?.[key]) && Number.isFinite(last?.[key])
                ? last[key] - first[key]
                : null,
            ]),
          ),
        };
      }),
      limits:
        "Seven Watch/Stop cycles, with Leave/Rejoin on cycles 2/4/6. Same-phase observations show bounded-session trends, not proof of leak freedom. Grid interiors can include reconnection settling; their frame rates are not steady-capacity estimates.",
    });
  }
}
const families = new Map();
for (const run of result.wholeApp.runs.filter(
  (r) => /^v[234]-/.test(r.label) && r.rounds === 1,
)) {
  const match = /^(.*)-(before|after)(?:-\d+)?$/.exec(run.label);
  if (!match) continue;
  const group = families.get(match[1]) ?? { before: [], after: [] };
  group[match[2]].push(run);
  families.set(match[1], group);
}
result.selectedComparisons = [...families].map(([family, variants]) => ({
  family,
  bothVariantsCompleted:
    variants.before.length > 0 && variants.after.length > 0,
  limits:
    "Medians of per-run phase summaries, not confidence intervals. Short single-pair coverage and long steady cases are labelled by run count and duration. CPU reductions must be read alongside delivered work and memory.",
  variants: Object.fromEntries(
    Object.entries(variants).map(([mode, runs]) => [
      mode,
      {
        runs: runs.length,
        directories: runs.map((r) => r.directory),
        appSha256: [...new Set(runs.map((r) => r.appSha256))],
        codecStable:
          runs.length > 0 && runs.every((r) => r.codecStable === true),
        phases: [
          "grid",
          "focus",
          "fullscreen",
          "minimized",
          "restore",
          "stopped",
        ].map((phase) => {
          const rows = runs.flatMap((r) =>
            r.rows.filter((row) => row.phase === phase),
          );
          return {
            phase,
            counterDiscontinuities: [
              ...new Set(rows.flatMap((row) => row.counterDiscontinuities)),
            ],
            ...Object.fromEntries(
              [
                "cpuCorePercent",
                "privateMiB",
                "workingSetMiB",
                "threads",
                "handles",
                "decodedFramesPerSecond",
                "convertedFramesPerSecond",
                "textureUploadsPerSecond",
                "encodedFramesPerSecond",
                "textureBytesMedian",
                "audioSamplesPerSecond",
                "audioQueueMaxMs",
                "subscriptionSetterCallsPerSecond",
              ].map((key) => [key, median(rows.map((row) => row[key]))]),
            ),
            errors: rows.some((row) => row.errors),
          };
        }),
      },
    ]),
  ),
}));
for (const directory of directories) {
  const loadReport = `${directory}/contention.json`;
  if (directory.startsWith("v4-") && existsSync(join(root, loadReport))) {
    const load = read(loadReport);
    if (load.completed === false) {
      const appPath = `${directory}/app.json`;
      result.failedAttempts.push({
        directory,
        fixture: "synthetic-contention",
        report: load,
        appCompleted: existsSync(join(root, appPath))
          ? read(appPath).completed
          : null,
        interpretation:
          "Invalid helper workload; excluded from whole-app comparisons.",
      });
    }
  }
  const diagnostic = `${directory}/diagnostic.json`;
  if (directory.includes("cli-") && existsSync(join(root, diagnostic))) {
    const j = read(diagnostic);
    result.capacityDiagnostics.push({
      ...pick(j, [
        "label",
        "count",
        "codec",
        "resolution",
        "appSha256",
        "publisherSha256",
        "limits",
        "samples",
      ]),
      process: j.process ? measureOs(j.process.samples) : null,
      interpretation:
        "Capacity only. Different delivered rates or changed codecs are not equal-work CPU comparisons.",
    });
  }
  const failure = `${directory}/failure.json`;
  if (existsSync(join(root, failure)))
    result.failedAttempts.push({ directory, report: read(failure) });
  const summary = `${directory}/summary.json`;
  if (
    directory.startsWith("portable-") &&
    existsSync(join(root, directory, "result.json"))
  )
    result.portable.push(read(`${directory}/result.json`));
  if (!existsSync(join(root, summary))) continue;
  if (directory.startsWith("readback-")) {
    result.readbackStaging.push(read(summary));
  } else if (directory.startsWith("gpu-counters-")) {
    const j = read(summary);
    result.gpuCounters.push({
      limits: j.limits,
      exeSha256: j.exeSha256,
      runs: j.runs.map((r) => ({
        size: r.size,
        mode: r.mode,
        pair: r.pair,
        pipeline: pick(r.pipeline, [
          "status",
          "codec",
          "width",
          "height",
          "renderedFrames",
          "droppedFrames",
          "expectedFrames",
          "pacingVerified",
          "processCpuMs",
          "wallMs",
          "cpuRgbaFrames",
          "sinkD3d11Buffers",
        ]),
        process: measureOs(
          r.process.samples.filter((s) => s.second > 1.5 && s.privateBytes > 0),
        ),
      })),
    });
  } else if (directory.startsWith("i420-pool-"))
    result.i420Pools.push(read(summary));
  else if (directory.startsWith("server-http-"))
    result.serverHttp.push(read(summary));
  else if (directory.startsWith("nvidia-quality-"))
    result.quality.push(read(summary));
  else if (directory.startsWith("sdk-churn-")) {
    const j = read(summary);
    result.sdkChurn.push({
      limits: j.limits,
      runs: j.runs.map((r) => ({
        pair: r.pair,
        mode: r.mode,
        exeSha256: r.exeSha256,
        churnWallMs: r.result.rounds.reduce((sum, r) => sum + r.wallMs, 0),
        firstRoundMs: r.result.rounds[0].wallMs,
        lastRoundMs: r.result.rounds.at(-1).wallMs,
        tracksCreatedAndDropped: r.result.rounds.at(-1).tracksCreatedAndDropped,
        processCpuMs: Math.max(...r.processSamples.map((s) => s.cpuMs)),
        peakPrivateMiB:
          Math.max(...r.processSamples.map((s) => s.privateBytes)) / 2 ** 20,
        settled: measureOs(
          r.processSamples.filter((s) => s.second > r.result.wallSeconds - 2),
        ),
      })),
    });
  }
}
for (const name of all("gpu-minimal", /-summary\.json$/)) {
  const j = read(`gpu-minimal/${name}`);
  result.nativeGpu.push({
    ...pick(j, [
      "codec",
      "width",
      "height",
      "fps",
      "frames",
      "fixtureSha256",
      "limits",
    ]),
    runs: j.runs.map((r) => {
      const { pid, ...row } = r;
      return row;
    }),
  });
}
for (const name of all("gpu-flexible", /^result-.*\.json$/))
  result.gpuLeases.push({
    path: `gpu-flexible/${name}`,
    report: read(`gpu-flexible/${name}`),
  });
for (const name of all("textures", /\.json$/))
  result.textures.push(
    pick(read(`textures/${name}`), [
      "width",
      "height",
      "tiles",
      "mode",
      "sync",
      "vsync",
      "completedRequestedFrames",
      "fault",
      "verificationError",
      "verifiedSyntheticTexels",
      "measuredFrames",
      "processCpuOneCorePercent",
      "uploadCallCpuMs",
      "frameIntervalMs",
      "limits",
    ]),
  );
for (const name of all("operations", /^(run|inventory|final)-\d+\.json$/))
  result.operations.push(read(`operations/${name}`));
for (const name of all("sdk-idle", /^\d+-(baseline|gated)\.json$/))
  result.sdkIdle.push(read(`sdk-idle/${name}`));
for (const name of all("sdk-logging", /^\d+-log-(verbose|warning)\.json$/))
  result.sdkLogging.push(read(`sdk-logging/${name}`));
for (const name of all("decoder", /^(run|fused|deferred)-/))
  if (existsSync(join(root, "decoder", name, "summary.json")))
    result.decoders.push(read(`decoder/${name}/summary.json`));
for (const name of all("pipelines", /^run-/))
  if (existsSync(join(root, "pipelines", name, "summary.json")))
    result.pipelines.push(read(`pipelines/${name}/summary.json`));
for (const name of all("server", /^media-/))
  if (existsSync(join(root, "server", name, "result.json")))
    result.serverMedia.push(read(`server/${name}/result.json`));
for (const path of [
  "operations/process-events.json",
  "operations/process-events-status.json",
  "loaded-runtime.json",
  "host.json",
  "implementation-review.json",
  "final-source-checks.json",
  "final-v4-matrix-status.json",
  "final-v4-tail-status.json",
  "decoder/checks-bounded-fallback/result.json",
])
  if (existsSync(join(root, path)))
    (result.additional ??= []).push({ path, report: read(path) });
result.evidence = evidence;
const compact = (value) => {
  if (typeof value === "number")
    return Number.isFinite(value) ? Math.round(value * 10_000) / 10_000 : null;
  if (Array.isArray(value)) return value.map(compact);
  if (value && typeof value === "object")
    return Object.fromEntries(
      Object.entries(value).map(([k, v]) => [k, compact(v)]),
    );
  return value;
};
const output = resolve(
  process.argv[2] ?? "docs/verification/local-performance-results.json",
);
writeFileSync(output, JSON.stringify(compact(result), null, 2) + "\n");
console.log(
  JSON.stringify({
    output,
    evidenceFiles: evidence.length,
    uiRuns: result.wholeApp.runs.length,
  }),
);
