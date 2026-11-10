import { readFileSync, readdirSync, writeFileSync, existsSync } from "node:fs";
import { resolve, join } from "node:path";
const root = resolve(import.meta.dirname, "../.local/experiments");
const median = (values) => {
  const sorted = values.filter(Number.isFinite).sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)] ?? null;
};
const phases = [
  "grid",
  "focus",
  "fullscreen",
  "minimized",
  "restore",
  "stopped",
];
const runs = [];
for (const directory of readdirSync(root)) {
  const file = join(root, directory, "result.json");
  if (!existsSync(file)) continue;
  const r = JSON.parse(readFileSync(file));
  if (
    r.schema !== 1 ||
    r.app?.kind !== "actual-native-ui-room" ||
    !r.appStartedAt ||
    !r.process?.startedAt
  )
    continue;
  const ready = r.app.wallSeconds - r.phaseSeconds * 6;
  const os = r.process.samples.map((s) => ({
    ...s,
    relativeSecond:
      s.second + (r.process.startedAt - r.appStartedAt) / 1000 - ready,
  }));
  const rows = phases.map((phase, index) => {
    const samples = os.filter(
      (s) =>
        s.relativeSecond >= index * r.phaseSeconds + 1.5 &&
        s.relativeSecond < (index + 1) * r.phaseSeconds - 0.5,
    );
    const first = samples[0],
      last = samples.at(-1);
    const app = r.app.samples.filter(
      (s) =>
        s.phase === phase &&
        s.second >= index * r.phaseSeconds + 1.5 &&
        s.second < (index + 1) * r.phaseSeconds - 0.5,
    );
    const a = app[0],
      b = app.at(-1);
    const frameRate = (key) =>
      a && b && a !== b && Array.isArray(a[key]) && Array.isArray(b[key])
        ? (b[key].reduce((s, n) => s + n, 0) -
            a[key].reduce((s, n) => s + n, 0)) /
          (b.second - a.second)
        : null;
    const engines = Object.keys(samples[0]?.gpuEnginesPercent ?? {});
    return {
      phase,
      samples: samples.length,
      cpuCorePercent:
        first && last && first !== last
          ? (100 * (last.cpuMs - first.cpuMs)) /
            ((last.second - first.second) * 1000)
          : null,
      privateMiB: median(samples.map((s) => s.privateBytes / 2 ** 20)),
      workingSetMiB: median(samples.map((s) => s.workingSetBytes / 2 ** 20)),
      threads: median(samples.map((s) => s.threads)),
      handles: median(samples.map((s) => s.handles)),
      gpuEnginesPercent: Object.fromEntries(
        engines.map((k) => [
          k,
          median(samples.map((s) => s.gpuEnginesPercent[k])),
        ]),
      ),
      dedicatedGpuMiB: median(
        samples.map(
          (s) => (s.gpuMemoryBytes["dedicated usage"] ?? NaN) / 2 ** 20,
        ),
      ),
      activeWatches: median(app.map((s) => s.activeWatches)),
      decodedFramesPerSecond: frameRate("frames"),
      convertedFramesPerSecond: frameRate("convertedFrames"),
      textureUploadsPerSecond: frameRate("uploadedFrames"),
      errors: app.some((s) => s.errorPresent || s.streamErrorCount > 0),
    };
  });
  runs.push({
    directory,
    label: r.label,
    count: r.count,
    codec: r.codec,
    phaseSeconds: r.phaseSeconds,
    appSha256: r.appSha256,
    publisherSha256: r.publisherSha256,
    counterFailures: r.process.counterFailures,
    samplerCpuMs: r.process.samplerCpuMs,
    rows,
  });
}
const result = {
  schema: 1,
  runs,
  limits:
    "CPU percentages use one full CPU core = 100%; not whole-machine normalization. Per-phase interior samples exclude transitions. Phase origin inferred from completed wall time, up to close/event scheduling error. Received decoded frames and UI calls are not presented video frames. Short runs need matched repetitions; GPU engine percentages are separate driver counters.",
};
const output = resolve(process.argv[2] ?? join(root, "ui-summary.json"));
writeFileSync(output, JSON.stringify(result, null, 2));
console.log(JSON.stringify({ runs: runs.length, output }));
