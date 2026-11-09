import { readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { setTimeout as delay } from "node:timers/promises";
import { expect, test } from "@playwright/test";
import { nativeRequest, toneFixtures } from "./fixtures.js";

type AudioMetrics = {
  frames: number;
  tones: number[];
  windows: { second: number; tones: number[] }[];
};

test("desktop mix excludes Discord and owned playback across process restarts", async ({
  baseURL,
}, testInfo) => {
  const fixtures = toneFixtures(testInfo.outputPath("audio-applications"));
  let game = fixtures.game();
  let discord = fixtures.discord();
  const profiles = () =>
    readdirSync(tmpdir()).filter((name) =>
      name.startsWith("FreedomStream-viewer-"),
    );
  const original = new Set(profiles());
  const timers: ReturnType<typeof setTimeout>[] = [];
  let capture: ReturnType<typeof nativeRequest> | undefined;
  try {
    await delay(1500);
    capture = nativeRequest("--qualify-audio", {
      choice: { mode: "desktop" },
      seconds: 14,
      viewer_invitation: `${baseURL}/r/${"a".repeat(22)}#invite=${"b".repeat(43)}`,
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
    const metrics = (await capture.result) as AudioMetrics;
    expect(metrics.frames).toBeGreaterThan(48000 * 12);
    expect(metrics.tones[0]).toBeGreaterThan(0.005);
    // Starting/stopping the game produces broadband transients. Measure tone
    // rejection in steady windows on both sides of the deliberate restart.
    const steady = metrics.windows.filter(
      ({ second }) => (second >= 2 && second <= 3) || second >= 10,
    );
    expect(steady.length).toBeGreaterThanOrEqual(4);
    for (const window of steady) {
      expect(window.tones[0]).toBeGreaterThan(0.005);
      expect(window.tones[1]).toBeLessThan(window.tones[0]! / 100);
      expect(window.tones[2]).toBeLessThan(window.tones[0]! / 100);
    }
    expect(metrics.windows.at(-1)!.tones[0]).toBeGreaterThan(0.005);
    expect(metrics.windows.at(-1)!.tones[1]).toBeLessThan(0.0002);
    await expect
      .poll(() => profiles().filter((name) => !original.has(name)))
      .toHaveLength(0);

    capture = nativeRequest("--qualify-audio", {
      choice: { mode: "desktop" },
      exclusions: [fixtures.application],
      seconds: 4,
    });
    const excluded = (await capture.result) as AudioMetrics;
    expect(excluded.frames).toBeGreaterThan(48000 * 3);
    expect(excluded.tones[0]).toBeLessThan(0.0002);
    expect(excluded.tones[1]).toBeLessThan(0.0002);
    console.log(
      "Windows application loopback: game captured, Discord/owned playback excluded, restarted game recovered, explicit exclusion and profile cleanup verified.",
    );
  } finally {
    for (const timer of timers) clearTimeout(timer);
    if (capture?.child.exitCode === null) capture.child.kill();
    fixtures.stop();
  }
});
