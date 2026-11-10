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
  const steady = r.steadySeconds ?? 0;
  const rounds = r.rounds ?? 1;
  const ready = r.app.wallSeconds - r.phaseSeconds * 6 * rounds - steady;
  const os = r.process.samples.map((s) => ({
    ...s,
    relativeSecond:
      s.second + (r.process.startedAt - r.appStartedAt) / 1000 - ready,
  }));
  const rows = Array.from({ length: rounds * phases.length }, (_, index) => {
    const phase = phases[index % phases.length];
    const start = index * r.phaseSeconds + (index ? steady : 0);
    const end = (index + 1) * r.phaseSeconds + steady;
    const interior = index === 0 && steady ? 60 : 1.5;
    const samples = os.filter(
      (s) =>
        s.relativeSecond >= start + interior && s.relativeSecond < end - 0.5,
    );
    const first = samples[0],
      last = samples.at(-1);
    const app = r.app.samples.filter(
      (s) =>
        s.phase === phase &&
        s.second >= start + interior &&
        s.second < end - 0.5,
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
      cycle: Math.floor(index / phases.length),
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
      audioSamplesPerSecond: frameRate("audioFrames"),
      audioQueueMaxMs: Math.max(
        0,
        ...app.flatMap((s) => s.audioQueueMaxMs ?? []),
      ),
      audioDroppedFrames: Math.max(
        0,
        ...app.flatMap((s) => s.audioDroppedFrames ?? []),
      ),
      subscriptionSetterCallsPerSecond:
        a && b && a !== b && Number.isFinite(a.subscriptionSetterCalls)
          ? (b.subscriptionSetterCalls - a.subscriptionSetterCalls) /
            (b.second - a.second)
          : null,
      nativeLogMessagesPerSecond:
        a && b && a !== b && Number.isFinite(a.nativeLogMessages)
          ? (b.nativeLogMessages - a.nativeLogMessages) / (b.second - a.second)
          : null,
      uiIntervals:
        r.app.uiInvocationIntervals?.find((s) => s.phase === phase) ?? null,
      errors: app.some((s) => s.errorPresent || s.streamErrorCount > 0),
    };
  });
  runs.push({
    directory,
    label: r.label,
    count: r.count,
    codec: r.codec,
    codecStable: r.codecStable ?? null,
    phaseSeconds: r.phaseSeconds,
    steadySeconds: steady,
    rounds,
    fps: r.fps ?? null,
    resolution: r.resolution ?? null,
    publishFixture: r.publishFixture ?? false,
    disconnected: r.disconnected ?? false,
    audioEnabled: r.audioEnabled ?? false,
    contention: r.contention ?? false,
    experimentSettings: r.experimentSettings ?? null,
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
