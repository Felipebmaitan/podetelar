// MIT. Keep correctness, equal-work eligibility and measured savings separate.
import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const family = process.argv[2];
const root = resolve(import.meta.dirname, "../.local/experiments/deep-pass");
const summary = JSON.parse(
  readFileSync(resolve(root, `${family}-summary.json`)),
);
const assertions = [];
for (const run of summary.runs) {
  assert.equal(run.codecStable, true, `${run.label}: codec changed`);
  if (
    family.startsWith("clip") &&
    run.group === "scroll" &&
    run.variant === "after"
  ) {
    const samples = JSON.parse(readFileSync(run.result)).app.samples.filter(
      (s) => s.phase === "grid",
    );
    for (let i = 0; i < samples[0].frames.length; i++) {
      assert.ok(
        samples.some((s) => s.visiblePictures[i]),
        `${run.label}: tile ${i} was never scrolled into view`,
      );
      assert.ok(
        (samples[0].visiblePictures[i] && samples[0].uploadedFrames[i] > 0) ||
          samples.some(
            (s, j) =>
              j > 0 &&
              s.visiblePictures[i] &&
              s.uploadedFrames[i] > samples[j - 1].uploadedFrames[i],
          ),
        `${run.label}: visible tile ${i} failed to resume`,
      );
      for (let j = 2; j < samples.length; j++)
        if (
          [samples[j - 2], samples[j - 1], samples[j]].every(
            (s) => !s.visiblePictures[i],
          )
        ) {
          assert.equal(
            samples[j].convertedFrames[i],
            samples[j - 1].convertedFrames[i],
            `${run.label}: hidden tile kept converting after its lease expired`,
          );
          assert.equal(
            samples[j].uploadedFrames[i],
            samples[j - 1].uploadedFrames[i],
            `${run.label}: hidden tile kept handing off images`,
          );
        }
    }
  }
  for (const p of run.phases) {
    assert.equal(p.errors, false, `${run.label}/${p.phase}: application error`);
    assert.equal(p.planarGlFaults, 0, `${run.label}/${p.phase}: GL failure`);
    if (p.samples < 4) continue; // Explicitly ineligible below; never count a pause as a win.
    if (p.phase === "stopped") {
      assert.equal(
        p.activeWatches,
        0,
        `${run.label}: Stop did not remove watches`,
      );
      if (p.nativeAtEnd)
        for (const key of [
          "devicesLive",
          "samplesLive",
          "deferredLive",
          "planeBytesLive",
          "stagingLive",
        ])
          assert.equal(
            p.nativeAtEnd[key],
            0,
            `${run.label}: ${key} remained after Stop`,
          );
      if (p.ownersAtEnd?.enabled) {
        assert.equal(
          p.ownersAtEnd.videoTasksStarted,
          p.ownersAtEnd.videoTasksCompleted,
          `${run.label}: video task survived Stop`,
        );
        assert.equal(
          p.ownersAtEnd.rgbaPools,
          0,
          `${run.label}: RGBA pool survived Stop`,
        );
        assert.equal(
          p.ownersAtEnd.rgbaPoolBytes,
          0,
          `${run.label}: pooled RGBA pixels survived Stop`,
        );
      }
      if (
        [
          "planar-memory",
          "shared-4k",
          "shared-contention",
          "shared-recovery",
          "shared-selective",
        ].includes(family)
      )
        assert.equal(
          p.ownersAtEnd?.rgbaGl?.taggedTextures,
          0,
          `${run.label}: stopped preview remained tagged as video`,
        );
    }
    if (
      family.startsWith("clip") &&
      run.variant === "after" &&
      p.phase === "grid"
    ) {
      for (const [i, tile] of p.perTile.entries()) {
        assert.ok(
          tile.decoded > 20,
          `${run.label}: tile ${i} lost incoming frames`,
        );
        if (tile.visible.every((v) => v === false)) {
          assert.equal(
            tile.converted,
            0,
            `${run.label}: clipped tile ${i} still converted`,
          );
          assert.equal(
            tile.handoffs,
            0,
            `${run.label}: clipped tile ${i} still uploaded`,
          );
        }
      }
    }
    if (p.nativeAtEnd) {
      assert.equal(
        p.nativeAtEnd.failures,
        0,
        `${run.label}: deferred materialization failed`,
      );
      assert.equal(p.nativeAtEnd.mapErrors, 0, `${run.label}: readback failed`);
    }
    if (
      family === "shared-selective" &&
      ["focus", "restore"].includes(p.phase)
    ) {
      const expected = p.phase === "focus" ? 3 : 4;
      assert.equal(
        p.activeWatches,
        expected,
        `${run.label}: selective Stop/rewatch count failed`,
      );
      assert.ok(
        p.decoded > expected * 50,
        `${run.label}: remaining or restored subscriptions stalled`,
      );
      assert.equal(
        p.nativeAtEnd.devicesLive,
        run.variant === "after" ? 1 : expected,
        `${run.label}: unexpected device ownership after selective Stop/rewatch`,
      );
    }
  }
  assertions.push({ label: run.label, passed: true });
}

if (family === "ownership")
  for (const run of summary.runs) {
    const cycles = new Set(run.phases.map((p) => p.cycle));
    assert.equal(
      cycles.size,
      7,
      "Ownership qualification did not execute seven cycles",
    );
    for (const cycle of cycles)
      if (cycle > 0)
        assert.ok(
          run.firstHandoffs.some((h) => h.cycle === cycle),
          `${run.label}: missing first handoff after cycle ${cycle}`,
        );
    if (run.variant === "before")
      assert.equal(run.heapActions.length, 0, "Baseline optimized its heap");
    else
      assert.equal(
        run.heapActions.length,
        7,
        "Candidate did not evaluate idle heap optimization in every cycle",
      );
  }

const comparisons = [];
for (const beforeRun of summary.runs.filter((r) => r.variant === "before")) {
  const afterRun = summary.runs.find(
    (r) =>
      r.variant === "after" &&
      r.group === beforeRun.group &&
      r.pair === beforeRun.pair,
  );
  assert.ok(afterRun, `${beforeRun.label}: missing pair`);
  for (const before of beforeRun.phases) {
    const after = afterRun.phases.find(
      (p) => p.phase === before.phase && p.cycle === before.cycle,
    );
    const valid = [before, after].every(
      (p) =>
        p &&
        p.samples >= 4 &&
        p.processSamples >= 4 &&
        !p.errors &&
        Number.isFinite(p.cpuCorePercent),
    );
    const decodeRatio =
      valid && before.decoded > 0 ? after.decoded / before.decoded : null;
    let handoffRatio =
      valid && before.handoffs > 0 ? after.handoffs / before.handoffs : null;
    if (family.startsWith("clip") && valid && before.phase === "grid") {
      const visible = after.perTile
        .map((t, i) => (t.visible.every(Boolean) ? i : -1))
        .filter((i) => i >= 0);
      const sum = (p, key) =>
        visible.reduce((n, i) => n + p.perTile[i][key], 0);
      handoffRatio =
        visible.length && sum(before, "handoffs") > 0
          ? sum(after, "handoffs") / sum(before, "handoffs")
          : null;
    }
    const publicationRatio =
      valid &&
      beforeRun.group === "combined" &&
      before.encodedFramesPerSecond > 0
        ? after.encodedFramesPerSecond / before.encodedFramesPerSecond
        : null;
    const nonInferiorUsefulWork =
      valid &&
      decodeRatio !== null &&
      decodeRatio >= 0.97 &&
      handoffRatio !== null &&
      handoffRatio >= 0.95 &&
      (beforeRun.group !== "combined" ||
        (publicationRatio !== null && publicationRatio >= 0.97));
    const equalUsefulWork =
      nonInferiorUsefulWork &&
      decodeRatio <= 1.03 &&
      handoffRatio <= 1.05 &&
      (publicationRatio === null || publicationRatio <= 1.03);
    if (
      family.startsWith("clip") &&
      beforeRun.group === "audio" &&
      valid &&
      before.phase !== "stopped"
    ) {
      assert.ok(
        before.audio > 100000,
        "Audio fixture did not deliver enough received PCM frames",
      );
      assert.ok(
        after.audio >= before.audio * 0.97,
        "Clipping reduced received audio frames",
      );
      assert.equal(
        after.audioDroppedFrames,
        0,
        "Clipping audio case dropped output frames",
      );
    }
    if (beforeRun.group === "combined" && valid && before.phase !== "stopped") {
      assert.equal(
        before.sharingFraction,
        1,
        "Combined baseline stopped publishing",
      );
      assert.equal(
        after.sharingFraction,
        1,
        "Combined candidate stopped publishing",
      );
      assert.ok(
        before.encodedFramesPerSecond > 30,
        "Combined fixture did not publish enough frames",
      );
      assert.ok(
        after.encodedFramesPerSecond > 30,
        "Combined candidate stalled publication",
      );
      assert.ok(
        after.audio >= before.audio * 0.97,
        "Combined candidate reduced received audio",
      );
    }
    const reduce = (key) =>
      valid && before[key] > 0 && Number.isFinite(after[key])
        ? 100 * (1 - after[key] / before[key])
        : null;
    comparisons.push({
      group: beforeRun.group,
      pair: beforeRun.pair,
      phase: before.phase,
      cycle: before.cycle,
      valid,
      decodeRatio,
      handoffRatio,
      publicationRatio,
      nonInferiorUsefulWork,
      equalUsefulWork,
      cpuReductionPercent: reduce("cpuCorePercent"),
      privateReductionPercent: reduce("privateMiB"),
      residentReductionPercent: reduce("workingSetMiB"),
      gpu3dReductionPercent: reduce("gpu3d"),
      cpuWinAtComparableWork:
        equalUsefulWork && after.cpuCorePercent < before.cpuCorePercent,
      cpuWinAtNonInferiorWork:
        nonInferiorUsefulWork && after.cpuCorePercent < before.cpuCorePercent,
      // No gain assertion for incomplete phases, unequal work or physical output.
    });
  }
}
const output = resolve(root, `${family}-assertions.json`);
writeFileSync(
  output,
  JSON.stringify(
    {
      schema: 1,
      family,
      passed: true,
      assertions,
      comparisons,
      limits:
        "Functional assertions and pair-by-pair measured comparisons. Equal-work eligibility requires consumer delivery within 3% and comparable visible handoffs within 5%; simultaneous publication must stay within 3%. Noninferiority also permits greater delivery/handoff/publication work. These checks do not establish physical display FPS, A/V synchronization, device-loss behavior, or equal latency.",
    },
    null,
    2,
  ),
);
console.log(
  JSON.stringify({
    family,
    passed: true,
    cases: assertions.length,
    grid: comparisons.filter((c) => c.phase === "grid"),
  }),
);
