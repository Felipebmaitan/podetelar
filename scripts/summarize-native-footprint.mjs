import { readFileSync, writeFileSync } from "node:fs";

const [input, output] = process.argv.slice(2);
if (!input || !output) throw new Error("Use input.json output.json.");
const raw = JSON.parse(readFileSync(input, "utf8").replace(/^\uFEFF/, ""));
const median = (values) => {
  const sorted = values.toSorted((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2
    ? sorted[middle]
    : (sorted[middle - 1] + sorted[middle]) / 2;
};
const cases = [];
for (const dimensions of [
  [1920, 1080, 1],
  [1920, 1080, 4],
  [3840, 2160, 1],
  [3840, 2160, 4],
]) {
  const [width, height, workers] = dimensions;
  const matching = raw.rows.filter(
    (row) =>
      row.kind === "conversion" &&
      row.result.width === width &&
      row.result.height === height &&
      row.result.workers === workers,
  );
  const variants = {};
  for (const variant of ["before", "after"]) {
    const runs = matching
      .filter((row) => row.variant === variant)
      .map((row) => row.result);
    if (runs.length !== raw.runs)
      throw new Error("Incomplete matched conversion runs.");
    variants[variant] = {
      p50Ms: runs.map((run) => run.p50Ms),
      p95Ms: runs.map((run) => run.p95Ms),
      cpuMsPerFrame: runs.map((run) => run.processCpuMs / run.frames),
      wallMsPerFrame: runs.map((run) => run.wallMs / run.frames),
      rustBytesPerFrame: runs.map((run) => run.rustAllocatedBytes / run.frames),
      rustAllocationsPerFrame: runs.map(
        (run) => run.rustAllocations / run.frames,
      ),
    };
  }
  if (new Set(matching.map((row) => row.result.checksum)).size !== 1)
    throw new Error("Conversion results changed.");
  cases.push({
    width,
    height,
    workers,
    variants,
    p50ReductionPercent:
      100 * (1 - median(variants.after.p50Ms) / median(variants.before.p50Ms)),
  });
}
const wrapper = {};
for (const variant of ["before", "after"]) {
  const runs = raw.rows.filter(
    (row) => row.kind === "wrapper" && row.variant === variant,
  );
  if (runs.length !== raw.runs) throw new Error("Incomplete wrapper runs.");
  wrapper[variant] = {
    privateBytes: runs.map((row) =>
      median(row.result.samples.map((sample) => sample.privateBytes)),
    ),
    workingSetBytes: runs.map((row) =>
      median(row.result.samples.map((sample) => sample.workingSetBytes)),
    ),
    waitingCpuMs: runs.map((row) => row.result.waitingCpuMs),
  };
}
const summary = {
  schema: 1,
  measuredAtUtc: raw.measuredAtUtc,
  runs: raw.runs,
  hashes: raw.hashes,
  payloadSha256: raw.payloadSha256,
  payloadBytes: raw.payloadBytes,
  cases,
  wrapper,
  wrapperPrivateBytesSaved:
    median(wrapper.before.privateBytes) - median(wrapper.after.privateBytes),
  limits: raw.limits,
};
writeFileSync(output, JSON.stringify(summary, null, 2) + "\n", { flag: "wx" });
console.log(
  JSON.stringify({
    cases: cases.map(({ width, height, workers, p50ReductionPercent }) => ({
      width,
      height,
      workers,
      p50ReductionPercent,
    })),
    wrapperPrivateBytesSaved: summary.wrapperPrivateBytesSaved,
  }),
);
