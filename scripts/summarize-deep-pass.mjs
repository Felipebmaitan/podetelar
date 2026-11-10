// MIT. Whitelisted evidence; no room DBs or credentials. Synthetic media is hash-only.
import assert from "node:assert/strict";
import { existsSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { resolve, relative, dirname, isAbsolute } from "node:path";
import { createHash } from "node:crypto";
import { format } from "prettier";

const root = resolve(import.meta.dirname, ".."),
  dir = resolve(root, ".local/experiments/deep-pass");
const partial = process.argv.includes("--partial"),
  evidence = new Map();
const sha = (bytes) => createHash("sha256").update(bytes).digest("hex");
const local = (path) => {
  const name = relative(root, resolve(path));
  assert.ok(
    name && !name.startsWith("..") && !isAbsolute(name),
    "Evidence must stay in the checkout",
  );
  return name.replaceAll("\\", "/");
};
const tracked = (path) => {
  const name = local(path),
    bytes = readFileSync(resolve(root, name));
  const receipt = { path: name, sha256: sha(bytes), bytes: bytes.length };
  if (evidence.has(name))
    assert.deepEqual(
      evidence.get(name),
      receipt,
      "Evidence changed while being read",
    );
  evidence.set(name, receipt);
  return bytes;
};
const json = (path) =>
  JSON.parse(
    tracked(path)
      .toString("utf8")
      .replace(/^\uFEFF/, ""),
  );
const clean = (value) =>
  JSON.parse(
    JSON.stringify(value, (_key, v) =>
      typeof v === "string" && v.toLowerCase().startsWith(root.toLowerCase())
        ? local(v)
        : v,
    ),
  );
const median = (values) => {
  const a = values.filter(Number.isFinite).sort((a, b) => a - b);
  return a[Math.floor(a.length / 2)] ?? null;
};
const metrics = [
  "cpuCorePercent",
  "privateMiB",
  "workingSetMiB",
  "nativePlaneMiBMedian",
  "nativePlaneMiBPeak",
  "decoded",
  "handoffs",
  "videoGlPayloadBytesPerSecond",
  "gpu3d",
  "gpuVideoDecode",
  "gpuVideoEncode",
  "gpuCopy",
  "dedicatedGpuMiB",
  "threads",
  "handles",
  "audio",
  "encodedFramesPerSecond",
];
function pairedApp(report, checks) {
  const groups = [];
  for (const group of new Set(report.runs.map((r) => r.group)))
    for (const phase of [
      "grid",
      "focus",
      "fullscreen",
      "minimized",
      "restore",
      "stopped",
    ]) {
      const pairs = [];
      for (const b of report.runs.filter(
        (r) => r.group === group && r.variant === "before",
      )) {
        const a = report.runs.find(
          (r) =>
            r.group === group && r.variant === "after" && r.pair === b.pair,
        );
        assert.ok(a);
        for (const before of b.phases.filter((p) => p.phase === phase)) {
          const after = a.phases.find(
            (p) => p.phase === phase && p.cycle === before.cycle,
          );
          const comparison = checks.comparisons.find(
            (c) =>
              c.group === group &&
              c.phase === phase &&
              c.pair === b.pair &&
              c.cycle === before.cycle,
          );
          if (!comparison?.valid) continue;
          pairs.push({
            pair: b.pair,
            cycle: before.cycle,
            before,
            after,
            comparison,
          });
        }
      }
      groups.push({
        group,
        phase,
        phasePairs: pairs.length,
        equalWorkPairs: pairs.filter((p) => p.comparison.equalUsefulWork)
          .length,
        nonInferiorWorkPairs: pairs.filter(
          (p) => p.comparison.nonInferiorUsefulWork,
        ).length,
        metrics: Object.fromEntries(
          metrics.map((key) => {
            const values = pairs
              .filter(
                (p) =>
                  Number.isFinite(p.before[key]) &&
                  Number.isFinite(p.after[key]),
              )
              .map((p) => ({
                pair: p.pair,
                cycle: p.cycle,
                before: p.before[key],
                after: p.after[key],
                reductionPercent:
                  p.before[key] > 0
                    ? 100 * (1 - p.after[key] / p.before[key])
                    : null,
              }));
            const reductions = values
              .map((p) => p.reductionPercent)
              .filter(Number.isFinite);
            return [
              key,
              {
                beforeMedian: median(values.map((p) => p.before)),
                afterMedian: median(values.map((p) => p.after)),
                medianReductionPercent: median(reductions),
                minimumReductionPercent: reductions.length
                  ? Math.min(...reductions)
                  : null,
                maximumReductionPercent: reductions.length
                  ? Math.max(...reductions)
                  : null,
                values,
              },
            ];
          }),
        ),
      });
    }
  return groups;
}

const required = {
  "clip-control": 18,
  "clip-gl-confirm": 2,
  "clip-wide-sustained": 6,
  "planar-final": 18,
  "planar-equal": 12,
  "planar-codecs": 6,
  "shared-device": 12,
  "relaxed-map": 12,
  "pool-observation": 4,
  ownership: 2,
  "planar-memory": 8,
  "shared-4k": 2,
  "shared-contention": 2,
  "shared-recovery": 2,
  "shared-selective": 2,
};
const apps = [];
for (const [family, expectedCases] of Object.entries(required)) {
  const index = resolve(dir, `${family}-cases.json`),
    summary = resolve(dir, `${family}-summary.json`),
    checks = resolve(dir, `${family}-assertions.json`);
  if (![index, summary, checks].every(existsSync)) {
    assert.ok(partial, `${family}: missing completed evidence`);
    continue;
  }
  const cases = json(index),
    report = json(summary),
    assertions = json(checks);
  if (
    cases.runs.length !== expectedCases ||
    assertions.assertions.length !== expectedCases
  ) {
    assert.ok(partial, `${family}: incomplete family`);
    continue;
  }
  assert.equal(assertions.passed, true);
  assert.equal(report.runs.length, expectedCases);
  const executableSha = sha(tracked(cases.exe));
  const inputs = [];
  for (const c of cases.runs) {
    assert.equal(c.code, 0);
    assert.ok(c.result);
    const raw = json(c.result),
      caseDir = dirname(c.result);
    assert.equal(
      raw.appSha256,
      executableSha,
      `${c.label}: executable identity mismatch`,
    );
    assert.equal(raw.app.completed, true);
    assert.equal(raw.codecStable, true);
    assert.deepEqual(
      raw.app,
      json(resolve(caseDir, "app.json")),
      `${c.label}: app receipt differs`,
    );
    assert.deepEqual(
      raw.process,
      json(resolve(caseDir, "process.json")),
      `${c.label}: process receipt differs`,
    );
    const variable =
      family === "planar-memory"
        ? ["requestedPlaneRetention"]
        : family.startsWith("clip")
          ? ["deepClip"]
          : family.startsWith("shared-")
            ? ["sharedDecoderDevice"]
            : family === "relaxed-map"
              ? ["relaxedReadback", "flushBeforeReadbackPoll"]
              : family === "ownership"
                ? ["idleHeapOptimization"]
                : family === "pool-observation"
                  ? ["poolTrace"]
                  : ["deepPlanar"];
    const candidateMetadataRecorded = variable.every(
      (key) => raw.experimentSettings[key] !== undefined,
    );
    const legacyClipping =
      !candidateMetadataRecorded &&
      family.startsWith("clip") &&
      variable[0] === "deepClip";
    assert.ok(
      candidateMetadataRecorded || legacyClipping,
      `${c.label}: unexpected missing switch receipt`,
    );
    for (const key of variable)
      if (!legacyClipping)
        assert.equal(
          raw.experimentSettings[key],
          family === "planar-memory"
            ? c.variant === "before"
              ? "1"
              : "0"
            : c.variant === "after",
          `${c.label}: candidate request was not applied`,
        );
    // Older receipts omit later disabled switches. Normalize only off/default
    // requests; enabled settings and all workload/executable identities must match.
    const settings = Object.fromEntries(
      Object.entries(raw.experimentSettings).filter(
        ([key, value]) =>
          !variable.includes(key) &&
          ![false, 0, "0", "default", ""].includes(value),
      ),
    );
    inputs.push({
      group: c.group,
      pair: c.pair,
      variant: c.variant,
      candidateMetadataRecorded,
      work: {
        count: raw.count,
        codec: raw.codec,
        fps: raw.fps ?? 60,
        resolution: raw.resolution ?? "p1080",
        phaseSeconds: raw.phaseSeconds,
        rounds: raw.rounds ?? 1,
        steadySeconds: raw.steadySeconds ?? 0,
        audio: !!raw.audioEnabled,
        publish: !!raw.publishFixture,
        contention: !!raw.contention,
        publisherSha256: raw.publisherSha256,
        fixtureHelperSha256: raw.fixtureHelperSha256,
        settings,
      },
    });
  }
  for (const b of inputs.filter((i) => i.variant === "before"))
    assert.deepEqual(
      b.work,
      inputs.find(
        (a) =>
          a.group === b.group && a.pair === b.pair && a.variant === "after",
      )?.work,
      `${family}/${b.group}/${b.pair}: more than the selected candidate changed`,
    );
  apps.push(
    clean({
      family,
      expectedCases,
      exe: cases.exe,
      exeSha256: executableSha,
      summaries: report.summaries,
      runs: report.runs,
      assertions,
      paired: pairedApp(report, assertions),
      inputs,
    }),
  );
}

const excluded = [];
const inputArtifacts = json(resolve(dir, "input-artifacts.json"));
for (const entry of inputArtifacts.entries)
  assert.equal(sha(tracked(resolve(root, entry.path))), entry.sha256);
for (const app of apps)
  for (const input of app.inputs)
    for (const [role, key] of [
      ["publisher", "publisherSha256"],
      ["fixture-helper", "fixtureHelperSha256"],
    ])
      assert.ok(
        inputArtifacts.entries.some(
          (entry) => entry.role === role && entry.sha256 === input.work[key],
        ),
        `${app.family}: missing frozen ${role}`,
      );
for (const family of ["clip", "planar", "calibration"]) {
  const cases = json(resolve(dir, `${family}-cases.json`));
  const executableSha = sha(tracked(cases.exe));
  const runs = cases.runs.map((c) => {
    assert.equal(c.code, 0);
    assert.ok(c.result);
    const raw = json(c.result),
      folder = dirname(c.result);
    assert.equal(raw.app.completed, true);
    assert.equal(raw.codecStable, true);
    // Calibration deliberately compares two binary identities.
    assert.ok(raw.appSha256);
    json(resolve(folder, "app.json"));
    json(resolve(folder, "process.json"));
    return clean({
      label: c.label,
      code: c.code,
      result: c.result,
      appSha256: raw.appSha256,
      completed: raw.app.completed,
      codecStable: raw.codecStable,
    });
  });
  excluded.push({
    family,
    exeSha256: executableSha,
    reason:
      "Pilot/instrumentation calibration, superseded by the frozen controlled campaign. Not used for performance headlines.",
    runs,
  });
}

const builds = [];
for (const label of [
  "e1",
  "e2",
  "sdk-v2",
  "sdk-v3",
  "sdk-v4",
  "sdk-v6",
  "sdk-v8",
  "protocol-v6",
  "protocol-v8",
  "product-v1",
  "report-memory",
]) {
  const file = resolve(dir, `source-${label}/manifest.json`);
  if (!existsSync(file)) continue;
  const manifest = json(file);
  for (const entry of manifest.files)
    assert.equal(
      sha(tracked(resolve(dirname(file), "source", entry.file))),
      entry.sha256,
    );
  if (manifest.executable)
    assert.equal(
      sha(tracked(resolve(root, manifest.executable.path))),
      manifest.executable.sha256,
    );
  builds.push({ label, ...clean(manifest) });
}
const glow = {};
for (const label of ["sdk-v6", "sdk-v8"]) {
  const folder = resolve(dir, `gl-${label}`);
  if (!existsSync(folder)) continue;
  const meta = json(resolve(folder, "source.json"));
  for (const [file, digest] of Object.entries(meta.generated))
    assert.equal(
      sha(tracked(resolve(folder, "egui_glow-0.37.0", file))),
      digest,
    );
  tracked(resolve(folder, "Cargo.original.lock"));
  tracked(resolve(folder, "Cargo.observer.lock"));
  glow[label] = meta;
}
const decoderSources = {};
for (const label of ["sdk-v6", "sdk-v8"]) {
  const folder = resolve(dir, `decoder-${label}`);
  if (!existsSync(folder)) {
    assert.ok(partial, `Missing generated decoder archive: ${label}`);
    continue;
  }
  const meta = json(resolve(folder, "source.json"));
  for (const [file, digest] of Object.entries(meta.generated))
    assert.equal(sha(tracked(resolve(folder, file))), digest);
  decoderSources[label] = meta;
}
const planarQualification = {};
for (const file of [
  "planar-pixels.json",
  "planar-pixels-v6.json",
  "planar-v8-retain-0-qualification.json",
  "planar-v8-retain-1-qualification.json",
]) {
  const path = resolve(dir, file);
  if (!existsSync(path)) {
    assert.ok(partial, `Missing planar qualification: ${file}`);
    continue;
  }
  const receipt = json(path);
  assert.equal(receipt.completed, true);
  assert.equal(receipt.cases.length, 27);
  assert.ok(receipt.cases.every((c) => c.passed));
  if (file.includes("v8")) {
    assert.equal(receipt.lifecycle.length, 12);
    assert.ok(receipt.lifecycle.every((c) => c.passed));
  }
  planarQualification[file] = receipt;
}

let historyReplay = null;
const replayFile = resolve(dir, "report-history-heap.json");
if (existsSync(replayFile)) {
  const receipt = json(replayFile),
    run = json(resolve(dir, "report-history-run.json"));
  assert.equal(receipt.completed, true);
  assert.ok(receipt.sourceSamples > 700);
  assert.equal(run.exitCode, 0);
  assert.equal(
    sha(tracked(resolve(root, run.input))),
    run.inputSha256.toLowerCase(),
  );
  assert.equal(
    sha(tracked(resolve(root, run.executable))),
    run.executableSha256.toLowerCase(),
  );
  assert.equal(
    receipt.sourceSamples,
    json(resolve(root, run.input)).app.samples.length,
  );
  assert.ok(
    receipt.allClonesRetained.allocatedBytes >
      receipt.afterDroppingClones.allocatedBytes,
  );
  historyReplay = { receipt, run };
} else assert.ok(partial, "Missing diagnostic-history ownership replay");
const helperFailures = [
  [
    "build-sdk-v5.log",
    "Observer preparation failed before any timed trial: missing registry checksum metadata. The later helper verifies the pinned crate archive.",
  ],
  [
    "build-sdk-v7.log",
    "Qualification-only unexposed Glow test API failed compilation; corrected before sdk-v8. No v7 runtime trial.",
  ],
].map(([file, reason]) => {
  const bytes = tracked(resolve(dir, file));
  return { file: local(resolve(dir, file)), sha256: sha(bytes), reason };
});

const native = [];
for (const name of [
  "qualification",
  "device",
  "readback",
  "lifecycle",
  "pool-trace",
  "qualificationv3flush",
  "readbackv3flush",
]) {
  const folder = resolve(dir, `native-${name}`),
    file = resolve(folder, "summary.json");
  assert.ok(existsSync(file), `${name}: missing native evidence`);
  const report = json(file),
    checks = json(resolve(folder, "assertions.json"));
  assert.equal(checks.passed, true);
  for (const row of report.runs) {
    assert.equal(row.valid, true);
    assert.deepEqual(row.result, json(row.file));
  }
  native.push(
    clean({
      name,
      exeSha256: report.exeSha256,
      checks,
      runs: report.runs.map((row) => ({
        case: row.case,
        valid: row.valid,
        exitCode: row.exitCode,
        sampledPeakPrivateBytes: row.sampledPeakPrivateBytes,
        sampledPeakWorkingSetBytes: row.sampledPeakWorkingSetBytes,
        runs: row.result.runs.map(({ allHashes, ...r }) => ({
          ...r,
          allFrameHashCount: allHashes.length,
          allFrameHashDigest: sha(JSON.stringify(allHashes)),
        })),
      })),
    }),
  );
}
const nativeComparisons = json(resolve(dir, "native-comparisons.json"));
const nativeBuilds = {};
for (const version of ["v2", "v3"]) {
  const folder = resolve(dir, `native-artifacts/${version}`),
    meta = json(resolve(folder, "build.json"));
  assert.equal(
    sha(tracked(resolve(folder, "deep-decoder.exe"))),
    meta.exeSha256.toLowerCase(),
  );
  assert.equal(
    sha(tracked(resolve(folder, "deep-decoder.cpp"))),
    meta.probeSha256.toLowerCase(),
  );
  const source = json(resolve(folder, "source.json"));
  for (const [file, digest] of Object.entries(source.generated))
    assert.equal(sha(tracked(resolve(folder, file))), digest);
  nativeBuilds[version] = meta;
}
for (const group of native)
  assert.ok(
    Object.values(nativeBuilds).some(
      (build) =>
        build.exeSha256.toLowerCase() === group.exeSha256.toLowerCase(),
    ),
    `${group.name}: unpreserved probe executable`,
  );

const usage = {};
const serverFolder = resolve(dir, "server-code");
const serverCode = json(resolve(serverFolder, "source.json"));
for (const entry of serverCode.files) {
  assert.equal(
    sha(tracked(resolve(serverFolder, "source", entry.file))),
    entry.sha256,
  );
  assert.equal(
    sha(tracked(resolve(root, entry.file))),
    entry.sha256,
    `Server source/compiled code changed: ${entry.file}`,
  );
}
for (const name of [
  "queries",
  "http",
  "cached-http",
  "cached-json-http",
  "cached-json-steady-http",
  "cache-correctness",
  "json-cache-correctness",
  "production-json-http",
  "production-json-steady-http",
]) {
  usage[name] = clean(json(resolve(dir, `usage/${name}.json`)));
}
const prior = readFileSync(
  resolve(root, "docs/verification/local-performance-results.json"),
  "utf8",
).toLowerCase();
const fixtures = [];
for (const size of ["1080p", "4k"])
  for (const codec of ["h264", "h265", "av1"]) {
    const file = resolve(
        root,
        `.local/experiments/fixtures/pattern-${size}60.${codec}`,
      ),
      bytes = tracked(file),
      digest = sha(bytes);
    assert.ok(
      prior.includes(digest),
      "Synthetic fixture differs from prior recorded evidence",
    );
    fixtures.push({ size, codec, sha256: digest, bytes: bytes.length });
  }
const output = {
  schema: 1,
  base: "e93b71d437810192bd1648e65088f1a6b9196754",
  complete: apps.length === Object.keys(required).length,
  host: {
    os: "Windows 11 Pro build 26300",
    cpu: "Ryzen 7 9800X3D",
    logicalProcessors: 16,
    gpu: "RTX 5080",
    driver: "32.0.16.2114",
    physicalDisplay: "1080p 300 Hz",
    extraAdapter: "Parsec virtual adapter",
  },
  statistics:
    "Upper-middle empirical medians: sorted floor(n/2). Percentage headlines use median of paired percentage changes, not the percentage difference between independent medians. Per-pair raw values and comparisons retained. CPU 100% equals one core. GPU engine counters are separate and must not be summed.",
  apps,
  historyReplay,
  helperFailures,
  inputArtifacts,
  excluded,
  native,
  nativeComparisons,
  nativeBuilds,
  builds,
  glow,
  decoderSources,
  planarQualification,
  fixtures,
  usage,
  serverCode,
  evidence: [...evidence.values()].sort((a, b) => a.path.localeCompare(b.path)),
  limits: [
    "Latest optimized baseline; do not add these percentages to prior gains.",
    "API upload payload is not physical PCIe traffic; handoffs/GL calls are not displayed FPS.",
    "Private bytes are commit; working set is resident memory; driver GPU memory can overlap other accounting.",
    "No AMD, physical 4K120, real-game FPS, physical A/V or deployment qualification is established.",
  ],
};
assert.ok(partial || output.complete, "Required families remain incomplete");
const file = resolve(
  root,
  partial
    ? ".local/experiments/deep-pass/aggregate-partial.json"
    : "docs/verification/deep-performance-results.json",
);
writeFileSync(file, await format(JSON.stringify(output), { parser: "json" }));
console.log(
  JSON.stringify({
    output: file,
    complete: output.complete,
    families: apps.length,
    evidence: evidence.size,
    bytes: statSync(file).size,
  }),
);
