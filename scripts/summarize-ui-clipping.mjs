// New-UI evidence only. Historical measurements and room credentials are not read.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
const root = resolve(import.meta.dirname, ".."),
  pass = join(root, ".local/experiments/ui-pr23-pass");
const read = (path) =>
  JSON.parse(readFileSync(path, "utf8").replace(/^\uFEFF/, ""));
const hash = (path) =>
  createHash("sha256").update(readFileSync(path)).digest("hex");
const groups = [
  {
    name: "fiveVisible",
    prefix: "ui23-clip-",
    build: "build-five.json",
    count: 5,
  },
  {
    name: "sevenWithClipping",
    prefix: "ui23-seven-",
    build: "build-seven.json",
    count: 7,
  },
];
const evidencePaths = [];
const summaries = {};
const median = (values) => {
  const a = values.filter(Number.isFinite).sort((a, b) => a - b),
    i = Math.floor(a.length / 2);
  return a.length % 2 ? a[i] : (a[i - 1] + a[i]) / 2;
};
for (const group of groups) {
  const output = join(pass, `${group.name}-summary.json`);
  execFileSync(
    process.execPath,
    [
      join(root, "scripts/summarize-local-performance.mjs"),
      output,
      group.prefix,
    ],
    { windowsHide: true },
  );
  const runs = read(output).runs,
    build = read(join(pass, group.build));
  assert.equal(runs.length, 4);
  evidencePaths.push(`ui-pr23-pass/${group.build}`);
  for (const run of runs) {
    const raw = read(
      join(root, ".local/experiments", run.directory, "result.json"),
    );
    const gridSamples = raw.app.samples.filter(
      (sample) => sample.phase === "grid",
    );
    run.gridObservation = {
      sampleCount: gridSamples.length,
      firstSecond: gridSamples[0]?.second,
      lastSecond: gridSamples.at(-1)?.second,
      windowFocused: [
        ...new Set(gridSamples.map((sample) => sample.windowFocused)),
      ],
      windowOccluded: [
        ...new Set(gridSamples.map((sample) => sample.windowOccluded)),
      ],
      lastRectVisibility: gridSamples.at(-1)?.visiblePictures,
    };
    if (group.count === 7)
      assert.ok(
        gridSamples.length >= 7,
        "Insufficient clipped-grid observation coverage",
      );
    assert.equal(run.appSha256, build.exeSha256);
    assert.equal(run.audioEnabled, false);
    assert.equal(run.codecStable, true);
    assert.equal(run.count, group.count);
    assert.equal(run.experimentSettings.deepPlanar, false);
    const grid = run.rows.find((row) => row.phase === "grid");
    assert.ok(grid.decodedFramesPerSecond > group.count * 58);
    assert.equal(grid.activeWatches, group.count);
    const expected =
      group.count === 7 && run.experimentSettings.deepClip
        ? 360
        : group.count * 60;
    assert.ok(Math.abs(grid.convertedFramesPerSecond - expected) < 5);
    assert.equal(
      run.rows.find((row) => row.phase === "minimized")
        .convertedFramesPerSecond,
      0,
    );
    assert.equal(
      run.rows.find((row) => row.phase === "stopped").activeWatches,
      0,
    );
    evidencePaths.push(
      ...["result.json", "app.json", "process.json"].map(
        (file) => `${run.directory}/${file}`,
      ),
    );
  }
  const gridValue = (run, key) =>
    run.rows.find((row) => row.phase === "grid")[key];
  const metrics = Object.fromEntries(
    [
      "cpuCorePercent",
      "privateMiB",
      "workingSetMiB",
      "dedicatedGpuMiB",
      "decodedFramesPerSecond",
      "convertedFramesPerSecond",
      "textureUploadsPerSecond",
    ].map((key) => {
      const before = median(
          runs
            .filter((r) => !r.experimentSettings.deepClip)
            .map((r) => gridValue(r, key)),
        ),
        after = median(
          runs
            .filter((r) => r.experimentSettings.deepClip)
            .map((r) => gridValue(r, key)),
        );
      return [
        key,
        {
          before,
          after,
          changePercent: before ? (after / before - 1) * 100 : null,
        },
      ];
    }),
  );
  const threeD = (enabled) =>
    median(
      runs
        .filter((r) => r.experimentSettings.deepClip === enabled)
        .map((r) => gridValue(r, "gpuEnginesPercent")["3d"]),
    );
  metrics.gpu3dPercent = {
    before: threeD(false),
    after: threeD(true),
    changePercent: (threeD(true) / threeD(false) - 1) * 100,
  };
  const pairs = [1, 2].map((pair) => {
    const before = runs.find((r) => r.label === `${group.prefix}a${pair}`),
      after = runs.find((r) => r.label === `${group.prefix}b${pair}`);
    return {
      pair,
      cpuChangePercent:
        (gridValue(after, "cpuCorePercent") /
          gridValue(before, "cpuCorePercent") -
          1) *
        100,
    };
  });
  if (group.count === 7)
    for (const pair of pairs) assert.ok(pair.cpuChangePercent < -10);
  summaries[group.name] = { build, metrics, pairs, runs };
}
const usage = read(join(pass, "usage/production-json-http.json"));
assert.equal(usage.runs.length, 10);
for (const run of usage.runs) {
  assert.equal(run.fullResponseEquivalent, true);
  assert.equal(run.authenticationAndLogoutPassed, true);
}
const usageMetrics = Object.fromEntries(
  ["cpuMs", "reportLatencyP50Ms", "healthLatencyP95Ms"].map((key) => {
    const before = median(
        usage.runs.filter((r) => r.variant === "baseline").map((r) => r[key]),
      ),
      after = median(
        usage.runs
          .filter((r) => r.variant === "production-json")
          .map((r) => r[key]),
      );
    return [key, { before, after, changePercent: (after / before - 1) * 100 }];
  }),
);
assert.ok(usageMetrics.reportLatencyP50Ms.changePercent < -90);
const packageAcceptance = read(join(pass, "package-acceptance.json"));
assert.equal(packageAcceptance.receive.length, 2);
assert.equal(packageAcceptance.bundledProbe.exitCode, 0);
evidencePaths.push(
  "ui-pr23-pass/package-acceptance.json",
  "ui-pr23-pass/bundled-probe.json",
  ...packageAcceptance.receive.map((entry) => entry.path),
);
evidencePaths.push(
  "ui-pr23-pass/usage/production-json-http.json",
  "ui-pr23-pass/server-code/source.json",
);
const result = {
  schema: 1,
  base: "feefc2f08a9daf4d6207265630bba59f69a9974e",
  ...summaries,
  usage: { metrics: usageMetrics, raw: usage },
  packageAcceptance,
  evidence: evidencePaths.map((path) => ({
    path,
    sha256: hash(join(root, ".local/experiments", path)),
  })),
  limits:
    "Short local B/A/A/B phases, two runs per variant. Five streams remain partly visible: no demonstrated clipping gain. Five-stream a2 has only three grid observations; its CPU/GPU aggregates are not a matched full-window comparison, and apparent regression is not causally established. Seven-stream trials have eight grid observations each, one fully clipped tile and ~419 decoded frames/s. All grid samples report unfocused windows and unknown occlusion; no foreground/display qualification. CPU100%=one core; private=commit, working set=resident; GPU engines separate; handoffs are not physical GL uploads/display frames. Cached HTTP is warm closed history, not normal live dashboard or WAN/VPS qualification. Windows CPU timer granularity can produce 0 ms in short cached bursts; that is not zero work. No audible/physical A/V, AMD GPU, games or physical4K qualification.",
};
const output = join(root, "docs/verification/native-ui-clipping-results.json");
writeFileSync(output, JSON.stringify(result, null, 2) + "\n");
console.log(
  JSON.stringify({
    output,
    assertions: "passed",
    groups: groups.length,
    usagePairs: 5,
  }),
);
