// MIT. Paired decoder microbenchmarks; these are not whole-application results.
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
const root = resolve(import.meta.dirname, "../.local/experiments/deep-pass");
const median = (values) => {
  const ordered = values.filter(Number.isFinite).sort((a, b) => a - b);
  return ordered[Math.floor(ordered.length / 2)] ?? null;
};
const results = [];
for (const name of ["device", "readback", "readbackv3flush"]) {
  const path = resolve(root, `native-${name}/summary.json`);
  if (!existsSync(path)) continue;
  const report = JSON.parse(readFileSync(path));
  for (const size of ["1080p", "4k"])
    for (const contexts of [1, 4])
      for (const visibility of ["visible", "hidden"]) {
        const rows = report.runs.filter(
          (r) =>
            r.case.size === size &&
            r.case.contexts === contexts &&
            r.case.visibility === visibility,
        );
        const variants = {};
        for (const variant of ["before", "after"]) {
          const samples = rows.filter((r) => r.case.variant === variant);
          const values = (key) => samples.map((r) => r.result.runs[0][key]);
          const delta = (key) =>
            samples.map(
              (r) =>
                r.result.runs[0].afterTimed[key] - r.result.runs[0].before[key],
            );
          variants[variant] = {
            trials: samples.length,
            cpuMs: median(values("cpuMs")),
            wallMs: median(values("wallMs")),
            ageP50Ms: median(values("ageP50Ms")),
            ageP95Ms: median(values("ageP95Ms")),
            ageMaxMs: median(values("ageMaxMs")),
            peakPrivateMiB: median(
              samples.map((r) => r.sampledPeakPrivateBytes / 2 ** 20),
            ),
            peakWorkingSetMiB: median(
              samples.map((r) => r.sampledPeakWorkingSetBytes / 2 ** 20),
            ),
            devicesCreated: median(delta("devicesCreated")),
            mapCalls: median(delta("mapCalls")),
            mapBusy: median(delta("mapBusy")),
            blockingFallbacks: median(delta("blockingFallbacks")),
            mapWallMs: median(delta("mapWallNs")) / 1e6,
            conversionMs: median(delta("conversionNs")) / 1e6,
            deviceLockMs: median(delta("deviceLockNs")) / 1e6,
            decoded: median(values("decoded")),
          };
        }
        results.push({ family: name, size, contexts, visibility, ...variants });
      }
}
writeFileSync(
  resolve(root, "native-comparisons.json"),
  JSON.stringify(
    {
      schema: 1,
      results,
      limits:
        "Same 300 encoded fixture frames per context, paced at 60 fps. Includes Configure and Release CPU. Age is decoder-input-to-consumer materialization age, not network or physical display latency. Peak process commit/residency sampled at 50 ms. Three alternating pairs for four visible contexts; other cases are single pairs. Timed samples retain six frames per context for after-timing pixel checks. Compare age and delivered work alongside CPU.",
    },
    null,
    2,
  ),
);
console.log(
  JSON.stringify(
    results.filter((r) => r.contexts === 4 && r.visibility === "visible"),
  ),
);
