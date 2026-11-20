import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import assert from "node:assert/strict";
const family = process.argv[2],
  suffix = process.argv[3] ?? "",
  dir = resolve(
    import.meta.dirname,
    `../.local/experiments/deep-pass/native-${family}${suffix}`,
  );
const report = JSON.parse(readFileSync(resolve(dir, "summary.json"))),
  checks = [];
for (const row of report.runs) {
  assert.ok(row.valid);
  for (const r of row.result.runs) {
    assert.equal(r.decoded, 300 * row.case.contexts);
    assert.equal(r.errors, 0);
    assert.equal(r.afterTimed.failures, 0);
    assert.equal(r.afterTimed.genericFallback - r.before.genericFallback, 0);
    assert.equal(r.afterTimed.mapErrors - r.before.mapErrors, 0);
    for (const key of [
      "devicesLive",
      "samplesLive",
      "deferredLive",
      "planeBytesLive",
      "stagingLive",
    ])
      assert.equal(r.settled[key], 0, `${family}: ${key} did not drain`);
  }
}
checks.push("expected decoded work, bounded readback and teardown");
if (family === "qualification")
  for (const row of report.runs.filter((r) => r.case.variant === "baseline")) {
    const controls = report.runs.filter(
      (r) =>
        r.case.codec === row.case.codec &&
        r.case.contexts === row.case.contexts,
    );
    for (const candidate of controls) {
      assert.equal(
        candidate.result.runs[0].allHashes.length,
        300 * candidate.case.contexts,
      );
      assert.deepEqual(
        candidate.result.runs[0].allHashes,
        row.result.runs[0].allHashes,
      );
    }
    checks.push(
      `all-frame whole-plane hashes ${row.case.codec}/${row.case.contexts}`,
    );
  }
if (["device", "readback"].includes(family))
  for (const row of report.runs.filter((r) => r.case.variant === "before")) {
    const c = report.runs.find(
      (r) =>
        r.case.variant === "after" &&
        r.case.size === row.case.size &&
        r.case.contexts === row.case.contexts &&
        r.case.visibility === row.case.visibility &&
        r.case.pair === row.case.pair,
    );
    assert.ok(c);
    assert.equal(
      c.result.runs[0].sampledWholePlaneHash,
      row.result.runs[0].sampledWholePlaneHash,
    );
    if (family === "device") {
      assert.equal(
        row.result.runs[0].afterTimed.devicesCreated,
        row.case.contexts,
      );
      assert.equal(c.result.runs[0].afterTimed.devicesCreated, 1);
    }
    checks.push(
      `${row.case.size}/${row.case.contexts}/${row.case.visibility}/${row.case.pair}`,
    );
  }
writeFileSync(
  resolve(dir, "assertions.json"),
  JSON.stringify(
    {
      passed: true,
      family,
      checks,
      limits:
        "Correctness/work/ownership assertions; savings must be evaluated separately from raw paired CPU/memory/frame-age results.",
    },
    null,
    2,
  ),
);
console.log(
  JSON.stringify({
    asserted: family,
    checks: checks.length,
    runs: report.runs.length,
  }),
);
