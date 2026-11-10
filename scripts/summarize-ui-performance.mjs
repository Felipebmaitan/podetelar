// Only sanitized measurement inputs are read; never room databases/configuration.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const pass = join(root, ".local/experiments/ui-pr22-pass");
execFileSync(
  process.execPath,
  [
    join(root, "scripts/summarize-local-performance.mjs"),
    join(pass, "pr19-summary.json"),
    "ui22-pr19-",
  ],
  { windowsHide: true },
);
const read = (path) =>
  JSON.parse(readFileSync(path, "utf8").replace(/^\uFEFF/, ""));
const summary = read(join(pass, "pr19-summary.json"));
const build = read(join(pass, "pr19-build.json"));
const labels = ["a1", "b1", "b2", "a2"];
assert.equal(summary.runs.length, 4);
const runs = Object.fromEntries(
  labels.map((label) => {
    const run = summary.runs.find((run) => run.label === `ui22-pr19-${label}`);
    assert.ok(run, `Missing ${label}`);
    assert.equal(run.audioEnabled, false);
    assert.equal(run.codecStable, true);
    assert.equal(run.count, 4);
    assert.equal(
      run.appSha256,
      label.startsWith("a") ? build.baselineSha256 : build.candidateSha256,
    );
    for (const row of run.rows) {
      assert.ok(row.samples >= 3);
      assert.deepEqual(row.counterDiscontinuities, []);
      assert.equal(row.activeWatches, row.phase === "stopped" ? 0 : 4);
      if (row.phase !== "stopped") assert.ok(row.decodedFramesPerSecond > 220);
      if (label.startsWith("b")) {
        if (row.phase === "minimized")
          assert.equal(row.convertedFramesPerSecond, 0);
        if (["focus", "fullscreen"].includes(row.phase))
          assert.ok(row.convertedFramesPerSecond < 65);
        if (["grid", "restore"].includes(row.phase))
          assert.ok(row.textureUploadsPerSecond > 180);
      }
    }
    return [label, run];
  }),
);
const median = (values) => {
  const sorted = values.filter(Number.isFinite).sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2
    ? sorted[middle]
    : (sorted[middle - 1] + sorted[middle]) / 2;
};
const value = (label, phase, key) =>
  runs[label].rows.find((row) => row.phase === phase)[key];
const phases = runs.a1.rows.map((row) => ({
  phase: row.phase,
  metrics: Object.fromEntries(
    [
      "cpuCorePercent",
      "privateMiB",
      "workingSetMiB",
      "dedicatedGpuMiB",
      "decodedFramesPerSecond",
      "convertedFramesPerSecond",
      "textureUploadsPerSecond",
    ].map((key) => {
      const before = median([
        value("a1", row.phase, key),
        value("a2", row.phase, key),
      ]);
      const after = median([
        value("b1", row.phase, key),
        value("b2", row.phase, key),
      ]);
      return [
        key,
        {
          before,
          after,
          changePercent: before ? (after / before - 1) * 100 : null,
        },
      ];
    }),
  ),
}));
for (const phase of ["grid", "focus", "fullscreen", "minimized", "restore"]) {
  for (const pair of [
    ["a1", "b1"],
    ["a2", "b2"],
  ]) {
    assert.ok(
      value(pair[1], phase, "cpuCorePercent") <
        value(pair[0], phase, "cpuCorePercent") * 0.9,
      `CPU direction not confirmed: ${phase} ${pair}`,
    );
  }
}
const evidencePaths = [
  "ui-pr22-pass/pr19-build.json",
  "ui-pr22-pass/pr22-baseline-source.json",
  ...summary.runs.flatMap((run) =>
    ["result.json", "app.json", "process.json"].map(
      (file) => `${run.directory}/${file}`,
    ),
  ),
];
const packageAcceptancePath = join(pass, "pr19-package-acceptance.json");
const packageAcceptance = existsSync(packageAcceptancePath)
  ? read(packageAcceptancePath)
  : null;
if (packageAcceptance) {
  evidencePaths.push(
    "ui-pr22-pass/pr19-package-acceptance.json",
    ...packageAcceptance.successes.map((entry) => entry.path),
    `${packageAcceptance.initialAttempt.directory}/receiver-process.json`,
  );
}
const idlePath = join(pass, "pr19-idle-summary.json");
if (existsSync(idlePath)) {
  execFileSync(
    process.execPath,
    [
      join(root, "scripts/summarize-local-performance.mjs"),
      idlePath,
      "ui22-idle-",
    ],
    { windowsHide: true },
  );
}
const idle = existsSync(idlePath) ? read(idlePath) : null;
if (idle) {
  assert.equal(
    idle.runs.length,
    2,
    "Complete both stopped-tail controls before aggregation",
  );
  for (const run of idle.runs) {
    assert.equal(run.audioEnabled, false);
    assert.equal(run.codecStable, true);
    assert.ok(
      run.rows.find((row) => row.phase === "stopped").settledStoppedTail,
    );
    evidencePaths.push(
      ...["result.json", "app.json", "process.json"].map(
        (file) => `${run.directory}/${file}`,
      ),
    );
  }
}
const probePath = join(pass, "pr19-bundled-probe.json");
const bundledProbe = existsSync(probePath) ? read(probePath) : null;
if (bundledProbe) {
  assert.equal(bundledProbe.exitCode, 0);
  evidencePaths.push("ui-pr22-pass/pr19-bundled-probe.json");
}
const evidence = evidencePaths.map((path) => ({
  path,
  sha256: createHash("sha256")
    .update(readFileSync(join(root, ".local/experiments", path)))
    .digest("hex"),
}));
const result = {
  schema: 1,
  implementation: build,
  qualification:
    "Silent four-stream 1080p60 HEVC A/B/B/A on the redesigned UI. Short phase interiors; two runs per variant. No audible, physical A/V, AMD, game, 4K or deployment qualification.",
  units:
    "CPU 100%=one core; MiB=2^20 bytes. Private memory is process commit; working set is resident memory. uploadedFrames/textureUploadsPerSecond are egui image handoffs, not physical GL uploads or display frames. Phase memory medians select the upper middle sample; across-run two-value medians average the two runs.",
  order: labels,
  phases,
  runs: summary.runs,
  packageAcceptance,
  bundledProbe,
  stoppedTailControls: idle?.runs ?? [],
  evidence,
};
const output = join(
  root,
  "docs/verification/native-ui-performance-results.json",
);
writeFileSync(output, JSON.stringify(result, null, 2) + "\n");
console.log(
  JSON.stringify({ output, runs: summary.runs.length, assertions: "passed" }),
);
