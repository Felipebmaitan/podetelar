import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
const family = process.argv[2] ?? "clip",
  root = resolve(import.meta.dirname, "../.local/experiments/deep-pass");
const cases = JSON.parse(readFileSync(resolve(root, `${family}-cases.json`)));
const median = (a) => {
  a = a.filter(Number.isFinite).sort((a, b) => a - b);
  return a[Math.floor(a.length / 2)] ?? null;
};
const phases = [
  "grid",
  "focus",
  "fullscreen",
  "minimized",
  "restore",
  "stopped",
];
const sum = (a) => a.reduce((s, n) => s + n, 0);
const runs = cases.runs.map((c) => {
  if (c.code !== 0 || !c.result) throw Error("Invalid case");
  const r = JSON.parse(readFileSync(c.result)),
    ready =
      r.app.readyOffsetSeconds ??
      r.app.wallSeconds -
        r.phaseSeconds * 6 * (r.rounds ?? 1) -
        (r.steadySeconds ?? 0);
  const os = r.process.samples.map((s) => ({
    ...s,
    at:
      s.second +
      (r.process.startedAt - (r.app.uiStartedAtUnixMs ?? r.appStartedAt)) /
        1000 -
      ready,
  }));
  return {
    ...c,
    exeSha256:
      r.appSha256 ?? r.executableSha256 ?? r.uiExecutableSha256 ?? null,
    codecStable: r.codecStable,
    heapActions: r.app.heapActions ?? [],
    firstHandoffs: r.app.firstHandoffs ?? [],
    phases: Array.from({ length: phases.length * (r.rounds ?? 1) }, (_, i) => {
      const phase = phases[i % phases.length],
        cycle = Math.floor(i / phases.length);
      const start = i * r.phaseSeconds + (i > 0 ? (r.steadySeconds ?? 0) : 0),
        end = (i + 1) * r.phaseSeconds + (r.steadySeconds ?? 0);
      const interior =
        phase === "grid" && r.steadySeconds > 0
          ? 10
          : family === "ownership" && phase !== "stopped"
            ? 8
            : 1.5;
      const a = r.app.samples.filter(
        (s) =>
          s.phase === phase &&
          s.second >= start + interior &&
          s.second < end - 0.5,
      );
      const p = os.filter((s) => s.at >= start + interior && s.at < end - 0.5),
        first = p[0],
        last = p.at(-1),
        x = a[0],
        y = a.at(-1);
      const tail = phase === "stopped" ? p.filter((s) => s.at >= end - 5) : [];
      const rate = (key) =>
        x && y && y.second > x.second
          ? (sum(y[key]) - sum(x[key])) / (y.second - x.second)
          : null;
      return {
        phase,
        cycle,
        samples: a.length,
        cpuCorePercent:
          first && last && last.second > first.second
            ? (100 * (last.cpuMs - first.cpuMs)) /
              ((last.second - first.second) * 1000)
            : null,
        privateMiB: median(p.map((s) => s.privateBytes / 2 ** 20)),
        workingSetMiB: median(p.map((s) => s.workingSetBytes / 2 ** 20)),
        decoded: rate("frames"),
        converted: rate("convertedFrames"),
        handoffs: rate("uploadedFrames"),
        audio: rate("audioFrames"),
        audioQueueMaxMs: Math.max(
          0,
          ...a.flatMap((s) => s.audioQueueMaxMs ?? []),
        ),
        audioDroppedFrames:
          x && y
            ? sum(y.audioDroppedFrames ?? []) - sum(x.audioDroppedFrames ?? [])
            : null,
        receivedVideoBps: median(a.map((s) => s.videoBps)),
        encodedFramesPerSecond:
          x && y
            ? (y.encodedFrames - x.encodedFrames) / (y.second - x.second)
            : null,
        sharingFraction: a.length
          ? a.filter((s) => s.sharing).length / a.length
          : null,
        actualPlanarGlUpdates:
          x && y
            ? (y.planarGlUpdates - x.planarGlUpdates) / (y.second - x.second)
            : null,
        planarLogicalBytesPerSecond:
          x && y
            ? (y.planarGlBytes - x.planarGlBytes) / (y.second - x.second)
            : null,
        actualRgbaGlCalls:
          x?.owners?.rgbaGl && y?.owners?.rgbaGl
            ? (y.owners.rgbaGl.calls - x.owners.rgbaGl.calls) /
              (y.second - x.second)
            : null,
        rgbaGlPayloadBytesPerSecond:
          x?.owners?.rgbaGl && y?.owners?.rgbaGl
            ? (y.owners.rgbaGl.payloadBytes - x.owners.rgbaGl.payloadBytes) /
              (y.second - x.second)
            : null,
        videoGlPayloadBytesPerSecond:
          x?.owners?.rgbaGl && y?.owners?.rgbaGl
            ? (y.owners.rgbaGl.payloadBytes -
                x.owners.rgbaGl.payloadBytes +
                (y.planarGlBytes - x.planarGlBytes)) /
              (y.second - x.second)
            : null,
        planarGlFaults: Math.max(0, ...a.map((s) => s.planarGlFaults ?? 0)),
        visiblePictures: median(
          a.map((s) => s.visiblePictures?.filter(Boolean).length),
        ),
        windowFocusedFraction: p.length
          ? p.filter((s) => s.windowFocused).length / p.length
          : null,
        activeWatches: median(a.map((s) => s.activeWatches)),
        errors: a.some((s) => s.errorPresent || s.streamErrorCount > 0),
        nativeAtEnd: y?.nativeOwners ?? null,
        ownersAtEnd: y?.owners ?? null,
        nativeStage:
          x?.nativeOwners && y?.nativeOwners
            ? Object.fromEntries(
                [
                  "readbacks",
                  "mapWallNs",
                  "deviceLockNs",
                  "readbackLockNs",
                  "conversionNs",
                  "outputCopyLogs",
                  "poolFullLogs",
                ].map((k) => [
                  k,
                  (y.nativeOwners[k] - x.nativeOwners[k]) /
                    (y.second - x.second),
                ]),
              )
            : null,
        stoppedTail: tail.length
          ? {
              privateMiB: median(tail.map((s) => s.privateBytes / 2 ** 20)),
              workingSetMiB: median(
                tail.map((s) => s.workingSetBytes / 2 ** 20),
              ),
            }
          : null,
        perTile:
          x && y
            ? x.frames.map((_, j) => ({
                visible: a.map((s) => s.visiblePictures?.[j]),
                decoded: (y.frames[j] - x.frames[j]) / (y.second - x.second),
                converted:
                  (y.convertedFrames[j] - x.convertedFrames[j]) /
                  (y.second - x.second),
                handoffs:
                  (y.uploadedFrames[j] - x.uploadedFrames[j]) /
                  (y.second - x.second),
              }))
            : [],
        gpu3d: median(p.map((s) => s.gpuEnginesPercent["3d"])),
        gpuVideoDecode: median(
          p.map((s) => s.gpuEnginesPercent["videodecode"]),
        ),
        gpuVideoEncode: median(
          p.map((s) => s.gpuEnginesPercent["videoencode"]),
        ),
        gpuCopy: median(p.map((s) => s.gpuEnginesPercent["copy"])),
        threads: median(p.map((s) => s.threads)),
        handles: median(p.map((s) => s.handles)),
        dedicatedGpuMiB: median(
          p.map((s) => (s.gpuMemoryBytes["dedicated usage"] ?? NaN) / 2 ** 20),
        ),
        uiIntervals: r.app.uiInvocationIntervals.find((s) => s.phase === phase),
      };
    }),
  };
});
const summaries = [];
for (const group of [...new Set(runs.map((r) => r.group))])
  for (const phase of phases) {
    const variants = {};
    const eligible = (p) =>
      p &&
      p.samples >= 4 &&
      !p.errors &&
      Number.isFinite(p.cpuCorePercent) &&
      (phase === "stopped" || Number.isFinite(p.decoded));
    for (const variant of ["before", "after"]) {
      const rows = [];
      for (const r of runs.filter(
        (r) => r.group === group && r.variant === variant,
      ))
        for (const p of r.phases.filter((p) => p.phase === phase)) {
          const paired = runs
            .find(
              (other) =>
                other.group === group &&
                other.pair === r.pair &&
                other.variant !== variant,
            )
            ?.phases.find(
              (other) => other.phase === phase && other.cycle === p.cycle,
            );
          if (eligible(p) && eligible(paired)) rows.push(p);
        }
      variants[variant] = {
        phasePairs: rows.length,
        ...Object.fromEntries(
          [
            "cpuCorePercent",
            "privateMiB",
            "workingSetMiB",
            "decoded",
            "converted",
            "handoffs",
            "audio",
            "encodedFramesPerSecond",
            "actualPlanarGlUpdates",
            "planarLogicalBytesPerSecond",
            "actualRgbaGlCalls",
            "rgbaGlPayloadBytesPerSecond",
            "videoGlPayloadBytesPerSecond",
            "gpu3d",
            "gpuVideoDecode",
            "gpuVideoEncode",
            "gpuCopy",
            "threads",
            "handles",
            "dedicatedGpuMiB",
          ].map((k) => [k, median(rows.map((r) => r[k]))]),
        ),
      };
    }
    summaries.push({ group, phase, ...variants });
  }
const output = resolve(root, `${family}-summary.json`);
writeFileSync(
  output,
  JSON.stringify(
    {
      schema: 1,
      family,
      runs,
      summaries,
      limits:
        "Individual experimental family versus latest optimized baseline. CPU 100%=one core. Handoffs are not GL uploads; only explicit planar calls count actual GL updates. Phase interiors exclude transitions.",
    },
    null,
    2,
  ),
);
console.log(
  JSON.stringify({
    output,
    runs: runs.length,
    grid: summaries.filter((s) => s.phase === "grid"),
  }),
);
