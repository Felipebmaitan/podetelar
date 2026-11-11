// MIT. Charts use only the sanitized aggregate and preserve individual pairs.
import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { format } from "prettier";

const root = resolve(import.meta.dirname, "..");
const report = JSON.parse(
  readFileSync(
    resolve(root, "docs/verification/deep-performance-results.json"),
  ),
);
assert.equal(report.complete, true);
const median = (values) => {
  const a = values.filter(Number.isFinite).sort((a, b) => a - b);
  return a[Math.floor(a.length / 2)] ?? null;
};
const cases = [];
const definitions = [
  ["cpuCorePercent", "CPU", "% of one core"],
  ["privateMiB", "Private commit", "MiB"],
  ["workingSetMiB", "Resident RAM", "MiB"],
  ["nativePlaneMiBMedian", "Live I420 plane storage", "MiB"],
  ["gpu3d", "GPU 3D engine", "% engine busy"],
  ["gpuVideoDecode", "GPU decode engine", "% engine busy"],
  ["dedicatedGpuMiB", "Dedicated GPU memory", "MiB"],
  ["videoGlPayloadBytesPerSecond", "GL API upload payload", "GB/s", 1e9],
  ["threads", "Threads", "threads"],
];
function appCase(family, group, label, status, note, phase = "grid") {
  const app = report.apps.find((a) => a.family === family);
  if (!app) return;
  const pair = app.paired.find((p) => p.group === group && p.phase === phase);
  if (!pair?.phasePairs) return;
  const metrics = definitions.flatMap(([key, name, unit, divisor = 1]) => {
    const source = pair.metrics[key];
    if (
      !source?.values.length ||
      !source.values.every((v) => v.before > 0 && Number.isFinite(v.after))
    )
      return [];
    return [
      {
        key,
        name,
        unit,
        values: source.values.map((v) => ({
          pair: v.pair,
          cycle: v.cycle,
          before: v.before / divisor,
          after: v.after / divisor,
        })),
      },
    ];
  });
  const delivered = pair.metrics.decoded;
  const handoffs = pair.metrics.handoffs;
  let work =
    phase === "stopped"
      ? "Stopped phases: no active watches; native resources must drain."
      : `Consumer frames/s: ${delivered.beforeMedian?.toFixed(2)} → ${delivered.afterMedian?.toFixed(2)}. Picture handoffs/s: ${handoffs.beforeMedian?.toFixed(2)} → ${handoffs.afterMedian?.toFixed(2)}. Noninferior work: ${pair.nonInferiorWorkPairs}/${pair.phasePairs}; strict comparable work: ${pair.equalWorkPairs}/${pair.phasePairs}.`;
  const publication = pair.metrics.encodedFramesPerSecond;
  if (publication.beforeMedian > 0)
    work += ` Publication frames/s: ${publication.beforeMedian.toFixed(2)} → ${publication.afterMedian.toFixed(2)}.`;
  const audio = pair.metrics.audio;
  if (audio.beforeMedian > 0)
    work += ` Received PCM frames/s: ${audio.beforeMedian.toFixed(0)} → ${audio.afterMedian.toFixed(0)}.`;
  cases.push({
    id: `${family}-${group}-${phase}`,
    label,
    status,
    note,
    work,
    metrics,
  });
}
appCase(
  "clip-control",
  "narrow",
  "E1 · clipped tiles · 4 × 1080p60",
  "Adopted",
  "Three alternating pairs; one picture visible, all four streams remain subscribed. Lower costs apply to clipped pictures.",
);
appCase(
  "clip-gl-confirm",
  "narrow",
  "E1 · actual GL upload confirmation",
  "Adopted; single-pair metrology",
  "Four 1080p60 HEVC streams, one picture visible. API payload is not physical PCIe bandwidth.",
);
appCase(
  "clip-wide-sustained",
  "wide",
  "E1 · all-visible sustained control",
  "No performance win claimed",
  "Three alternating 70-second grid pairs. All four pictures visible; ten warm-up seconds excluded.",
);
appCase(
  "clip-control",
  "five",
  "E1 · five incoming streams",
  "CPU win; RAM cost",
  "Single pair. Resident RAM rises; this is not a memory-saving result.",
);
appCase(
  "planar-final",
  "four",
  "E2 · planar · 4 × 1080p60",
  "Experimental",
  "Three alternating HEVC pairs. Lower upload payload and 3D work; per-pair CPU variability remains visible. New rendering path is disabled by default.",
);
appCase(
  "planar-final",
  "one",
  "E2 · planar · 1 × 1080p60",
  "Experimental",
  "Three alternating pairs. CPU headline must use median paired changes; the ratio of independent medians overstates the typical paired saving.",
);
appCase(
  "planar-equal",
  "four30",
  "E2 · planar · 4 × 1080p30",
  "GPU/RAM benefit; CPU regression",
  "Three alternating pairs at comparable useful work. Lower GPU work does not establish a universal CPU improvement.",
);
appCase(
  "planar-equal",
  "one4k30",
  "E2 · planar · 1 × 4K30",
  "GPU/RAM benefit; CPU cost",
  "Three alternating pairs. Synthetic 4K source on a 1080p display; not physical 4K output qualification.",
);
appCase(
  "planar-final",
  "one4k",
  "E2 · planar · 1 × 4K60",
  "Experimental; work differs in one pair",
  "Two pairs. No equal-work CPU win is established; decoded and handoff rates must be considered.",
);
appCase(
  "planar-final",
  "four4k",
  "E2 · planar · four-4K capacity",
  "Unequal-work capacity result",
  "Single pair. Candidate delivers substantially more consumer frames and handoffs. CPU differences are not equal-work gains or physical displayed FPS.",
);
for (const [group, name] of [
  ["h264", "H.264"],
  ["av1", "hardware AV1"],
  ["av1sw", "software AV1"],
])
  appCase(
    "planar-codecs",
    group,
    `E2 · planar · ${name}`,
    "Experimental; single pair",
    "Four synthetic 1080p60 streams. Software AV1 does not use the custom hardware decode engine.",
  );
appCase(
  "planar-memory",
  "four",
  "E2 · release uploaded CPU planes",
  "Experimental ownership refinement",
  "Three pairs with planar rendering enabled in both variants. Changes only retention of already-uploaded CPU planes, after synchronous client-memory GL calls.",
);
for (const [file, label] of [
  ["production-json-http", "E3 · historical report cache · bursts"],
  ["production-json-steady-http", "E3 · historical report cache · 60 seconds"],
]) {
  const source = report.usage[file],
    runs = source.runs;
  const fields = [
    ["cpuMs", "Server CPU", "ms"],
    ["reportLatencyP50Ms", "Report latency p50", "ms"],
    ["healthLatencyP95Ms", "Health latency p95", "ms"],
    ["rss", "End resident RAM", "MiB"],
  ];
  const metrics = fields.map(([key, name, unit]) => ({
    key,
    name,
    unit,
    values: runs
      .filter((r) => r.variant === "baseline")
      .map((b) => {
        const a = runs.find(
          (r) => r.pair === b.pair && r.variant === source.treatment,
        );
        assert.ok(a);
        return {
          pair: b.pair,
          cycle: 0,
          before: key === "rss" ? b.memory.rss / 2 ** 20 : b[key],
          after: key === "rss" ? a.memory.rss / 2 ** 20 : a[key],
        };
      }),
  }));
  cases.push({
    id: file,
    label,
    status: "Adopted for warm closed-history hits",
    note: source.steady
      ? "One fixed-duration pair, 30 reports and 30 health requests per variant. Cold/live reports still calculate SQL. Sampled RSS peaks remain a cost."
      : "Five alternating pairs, ten reports and forty health requests each. Short-burst end RSS is higher; a measured zero CPU counter sample is below timing resolution, not zero work.",
    work: "Real authenticated HTTP routes; 522,000 synthetic participant-minute rows. Full responses, fresh status and authentication matched.",
    metrics,
  });
}
{
  const source = report.apps.find((a) => a.family === "ownership");
  assert.ok(source);
  const b = source.runs.find((r) => r.variant === "before"),
    a = source.runs.find((r) => r.variant === "after");
  const stopped = [b, a].map((r) =>
    r.phases.filter((p) => p.phase === "stopped"),
  );
  const fields = [
    [
      "tailPrivate",
      "Stopped private commit",
      "MiB",
      (p) => p.stoppedTail.privateMiB,
    ],
    [
      "tailResident",
      "Stopped resident RAM",
      "MiB",
      (p) => p.stoppedTail.workingSetMiB,
    ],
    [
      "heapCommitted",
      "Default heap committed",
      "MiB",
      (p) => p.ownersAtEnd.defaultHeap.committedBytes / 2 ** 20,
    ],
    [
      "heapAllocated",
      "Default heap live allocated",
      "MiB",
      (p) => p.ownersAtEnd.defaultHeap.allocatedBytes / 2 ** 20,
    ],
  ];
  const metrics = fields.map(([key, name, unit, value]) => ({
    key,
    name,
    unit,
    values: stopped[0].map((before) => {
      const after = stopped[1].find((p) => p.cycle === before.cycle);
      assert.ok(after);
      return {
        pair: 1,
        cycle: before.cycle,
        before: value(before),
        after: value(after),
      };
    }),
  }));
  metrics.push({
    key: "restart",
    name: "Rewatch to first handoffs",
    unit: "ms",
    values: b.firstHandoffs
      .filter((h) => h.cycle > 0)
      .map((before) => {
        const after = a.firstHandoffs.find((h) => h.cycle === before.cycle);
        assert.ok(after);
        return {
          pair: 1,
          cycle: before.cycle,
          before: before.rewatchToAllFirstHandoffsMs,
          after: after.rewatchToAllFirstHandoffsMs,
        };
      }),
  });
  const attempts = a.heapActions.filter((h) => h.action.eligible).length;
  cases.push({
    id: "ownership-tail",
    label: "E4 · idle heap optimization",
    status: "Not adopted: no useful persistent memory reduction",
    note: "One before/after run, seven cycles each; cycles are not independent trials. RAM uses the last five seconds of each stopped phase. Restart has six non-prewarmed observations. Diagnostic history is included in this footprint. No working-set trimming.",
    work: `All observed native owners, video tasks and RGBA pools must drain after Stop. Eligible heap actions: ${attempts}/7. Heap counters cover the process default heap, not all driver or process allocations.`,
    metrics,
  });
}
for (const [group, label] of [
  ["four", "four 1080p60"],
  ["one4k", "one 4K60 control"],
  ["mixed", "mixed codecs"],
  ["combined", "publish + watch + audio"],
])
  appCase(
    "shared-device",
    group,
    `E5 · shared decoder device · ${label}`,
    "Experimental",
    "Compatible active same-adapter contexts share a device. Other drivers, physical device removal and AMD remain unqualified.",
  );
appCase(
  "shared-4k",
  "four4k",
  "E5 · sharing · four-4K capacity",
  "Memory benefit with a delivery tradeoff",
  "Single pair: consumer delivery falls about 6.1% while handoffs rise about 10.1%. Neither noninferior delivery nor an equal-work CPU gain is established.",
);
appCase(
  "shared-contention",
  "four",
  "E5 · sharing · synthetic GPU contention",
  "Experimental qualification",
  "Single pair under repeated 4K RGBA/NV12 GPU conversions. This is not a real-game FPS measurement.",
);
appCase(
  "shared-recovery",
  "four",
  "E5 · sharing · repeated recovery",
  "Experimental qualification",
  "One before/after pair, three cycles per process, including Stop/rewatch and Leave/rejoin. Repeated cycles are not independent process trials; physical device removal remains unqualified.",
);
for (const [group, label] of [
  ["four", "four 1080p60"],
  ["one4k", "one 4K60"],
  ["mixed", "mixed codecs"],
  ["combined", "publish + watch + audio"],
])
  appCase(
    "relaxed-map",
    group,
    `E6 · polling readback · ${label}`,
    "Rejected as a default",
    "CPU reduction trades against increased native frame age. Whole-app throughput cannot erase that latency cost.",
  );
for (const name of ["readback", "readbackv3flush"]) {
  const source = report.native.find((n) => n.name === name);
  assert.ok(source);
  for (const size of ["1080p", "4k"]) {
    const runs = source.runs.filter(
      (r) =>
        r.case.size === size &&
        r.case.contexts === 4 &&
        r.case.visibility === "visible",
    );
    const metrics = [
      ["cpuMs", "Decoder probe CPU", "ms"],
      ["ageP95Ms", "Frame age p95", "ms"],
      ["peakPrivateMiB", "Peak private commit", "MiB"],
    ].map(([key, name, unit]) => ({
      key,
      name,
      unit,
      values: runs
        .filter((r) => r.case.variant === "before")
        .map((b) => {
          const a = runs.find(
            (r) => r.case.pair === b.case.pair && r.case.variant === "after",
          );
          assert.ok(a);
          return {
            pair: b.case.pair,
            cycle: 0,
            before:
              key === "peakPrivateMiB"
                ? b.sampledPeakPrivateBytes / 2 ** 20
                : b.runs[0][key],
            after:
              key === "peakPrivateMiB"
                ? a.sampledPeakPrivateBytes / 2 ** 20
                : a.runs[0][key],
          };
        }),
    }));
    cases.push({
      id: `${name}-${size}`,
      label: `E6 · native ${name === "readback" ? "polling" : "flush + polling"} · ${size}`,
      status: "Rejected latency tradeoff",
      note: "Three alternating pairs, four contexts, 300 frames/context. Age is input-to-materialized-frame, not end-to-end display latency.",
      work: "All expected frames and sampled whole-plane hashes match. Probe CPU includes Configure/Release; do not treat as whole-app CPU.",
      metrics,
    });
  }
}
assert.ok(cases.length > 20);
const configurations = [];
for (const c of cases) {
  assert.ok(c.metrics.length);
  c.costId = `${c.id}-cost`;
  configurations.push({
    id: c.costId,
    title: "Observed changes in each category",
    description:
      "Positive = reduction; negative = increased cost. Median of paired percentage changes. These categories are not additive and do not form a total performance score.",
    height: Math.max(240, 50 + c.metrics.length * 36),
    spec: {
      type: "horizontalBar",
      y: "Resource",
      x: "Reduction",
      xLabel: "Reduction versus baseline",
      valueFormat: { style: "percent", maximumFractionDigits: 1 },
      showValues: true,
    },
    columns: ["Resource", "Reduction"],
    rows: c.metrics.map((m) => [
      m.name,
      median(m.values.map((v) => 1 - v.after / v.before)),
    ]),
  });
  for (const m of c.metrics) {
    m.configId = `${c.id}-${m.key}`;
    const multiCycle = new Set(m.values.map((v) => v.cycle)).size > 1;
    const rows = m.values.map((v) => [
      multiCycle ? `Cycle ${v.cycle + 1}` : `Pair ${v.pair}`,
      v.before,
      v.after,
    ]);
    assert.ok(
      rows.every((r) => Number.isFinite(r[1]) && Number.isFinite(r[2])),
    );
    configurations.push({
      id: m.configId,
      title: `${m.name} in each paired observation`,
      description: `Original units: ${m.unit}. Both variants are shown; a few pairs do not establish statistical significance.`,
      height: 280,
      spec: {
        type: "bar",
        x: "Observation",
        y: ["Before", "After"],
        yLabel: m.unit,
        showValues: true,
        valueFormat: { style: "decimal", maximumFractionDigits: 2 },
      },
      columns: ["Observation", "Before", "After"],
      rows,
    });
  }
}
const escapeJson = (data) => JSON.stringify(data).replaceAll("<", "\\u003c");
const template = readFileSync(
  resolve(root, "scripts/experiments/deep-performance-view.html"),
  "utf8",
);
const html = await format(
  template
    .replace(
      "<!-- DEEP_DATA -->",
      `<script type="application/json" id="deep-view-data">${escapeJson(cases)}</script>`,
    )
    .replace(
      "<!-- DEEP_CONFIGS -->",
      configurations
        .map(
          (c) =>
            `<script type="application/json" data-viz-chart>${escapeJson(c)}</script>`,
        )
        .join("\n"),
    ),
  { parser: "html" },
);
assert.ok(Buffer.byteLength(html) < 1_000_000);
const output = resolve(
  root,
  "docs/verification/deep-performance-comparison.html",
);
writeFileSync(output, html);
console.log(
  JSON.stringify({
    output,
    cases: cases.length,
    chartPayloads: configurations.length,
    bytes: Buffer.byteLength(html),
  }),
);
