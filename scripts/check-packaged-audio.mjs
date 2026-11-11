// MIT. Same real audio policy as acceptance; stores tone magnitudes, never PCM.
import { mkdirSync, existsSync, readFileSync, writeFileSync } from "node:fs";
import { resolve, join } from "node:path";
import { createHash } from "node:crypto";
import { setTimeout as delay } from "node:timers/promises";
import {
  backgroundAudioExclusions,
  nativeRequest,
  toneFixtures,
} from "../native-e2e/fixtures.ts";
const root = resolve(import.meta.dirname, ".."),
  out = resolve(
    root,
    process.argv[2] ?? ".local/experiments/deep-pass/audio-controls",
  );
if (existsSync(out)) throw Error("Audio control evidence is immutable");
mkdirSync(out, { recursive: true });
const paths = {
  before: resolve(
    root,
    ".local/distribution/freedomstream-windows-x64-0.1.0-daca188dfc3c.exe",
  ),
  after: resolve(
    root,
    ".local/distribution/freedomstream-windows-x64-0.1.0-e24856c63083.exe",
  ),
};
const tone = resolve(
  root,
  ".local/distribution/0.1.0-e24856c63083/bin/freedomstream-publisher.exe",
);
const sha = (path) =>
  createHash("sha256").update(readFileSync(path)).digest("hex");
const runs = [];
for (const [index, variant] of [
  "before",
  "after",
  "after",
  "before",
].entries()) {
  process.env.FREEDOMSTREAM_NATIVE_EXE = paths[variant];
  process.env.FREEDOMSTREAM_NATIVE_TONE_EXE = tone;
  const background = process.argv.includes("--exclude-background")
    ? await backgroundAudioExclusions()
    : [];
  const fixtures = toneFixtures(join(out, `case-${index + 1}`)),
    timers = [];
  let game = fixtures.game(),
    discord = fixtures.discord(),
    capture;
  try {
    await delay(1500);
    capture = nativeRequest("--qualify-audio", {
      choice: { mode: "desktop" },
      exclusions: background,
      seconds: 14,
      playback_tone: true,
    });
    timers.push(
      setTimeout(() => {
        game.kill();
        discord.kill();
        timers.push(
          setTimeout(() => {
            game = fixtures.game();
            discord = fixtures.discord();
          }, 700),
        );
      }, 5500),
    );
    const m = await capture.result;
    const windows = m.windows.map((w) => ({
      second: w.second,
      tones: w.tones,
    }));
    const steady = windows.filter(
      (w) => (w.second >= 2 && w.second <= 3) || w.second >= 10,
    );
    const violations = steady.filter(
      (w) =>
        !(
          w.tones[0] > 0.005 &&
          w.tones[1] < w.tones[0] / 100 &&
          w.tones[2] < w.tones[0] / 100
        ),
    );
    capture = nativeRequest("--qualify-audio", {
      choice: { mode: "desktop" },
      exclusions: [...background, fixtures.application],
      seconds: 4,
    });
    const e = await capture.result;
    const passed =
      m.frames > 48000 * 12 &&
      m.tones[0] > 0.005 &&
      steady.length >= 4 &&
      !violations.length &&
      windows.at(-1).tones[0] > 0.005 &&
      windows.at(-1).tones[1] < 0.0002 &&
      e.frames > 48000 * 3 &&
      e.tones[0] < 0.0002 &&
      e.tones[1] < 0.0002;
    runs.push({
      index: index + 1,
      variant,
      backgroundExclusionCount: background.length,
      executableSha256: sha(paths[variant]),
      toneSha256: sha(tone),
      passed,
      frames: m.frames,
      tones: m.tones,
      windows,
      violations,
      excluded: { frames: e.frames, tones: e.tones },
    });
  } catch {
    runs.push({
      index: index + 1,
      variant,
      passed: false,
      fault: "capture_failed",
    });
  } finally {
    for (const timer of timers) clearTimeout(timer);
    if (capture?.child.exitCode === null) capture.child.kill();
    fixtures.stop();
  }
  writeFileSync(
    join(out, "result.json"),
    JSON.stringify(
      {
        schema: 1,
        runs,
        limits:
          "Real Windows desktop mix; only fixed-frequency magnitudes and frame counts retained. Four alternating controls are not performance benchmarks or proof of physical A/V synchronization.",
      },
      null,
      2,
    ),
  );
  console.log(
    JSON.stringify({
      index: index + 1,
      variant,
      passed: runs.at(-1).passed,
      violations: runs.at(-1).violations?.map((w) => w.second),
    }),
  );
  await delay(500);
}
