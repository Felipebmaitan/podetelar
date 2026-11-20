import { writeFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { createTestRoom } from "../scripts/test-room.js";
import { nativeRequest, toneFixtures } from "./fixtures.js";

test("standard audio resumes after output loss and preserves mute across output changes", async ({
  request,
  baseURL,
}, info) => {
  const created = await createTestRoom(request, baseURL!, "Output recovery");
  expect(created.status()).toBe(201);
  const { invitationUrl } = await created.json();
  const fixture = toneFixtures(info.outputPath("tones"));
  fixture.game();
  const sender = nativeRequest("--publish-fixture", {
    invitationUrl,
    displayName: "Recovery source",
    codec: "h264",
    seconds: 30,
    quality: {
      resolution: "p720",
      fps: 30,
      profile: "motion",
      bitrate: { mode: "manual", bps: 2_000_000 },
    },
    audio: { mode: "application", application: fixture.application },
  });
  let watcher: ReturnType<typeof nativeRequest> | undefined;
  try {
    await sender.reported;
    watcher = nativeRequest("--watch-fixture", {
      invitationUrl,
      seconds: 25,
      continuousWatch: true,
      measureOutputAudio: true,
      outputPauseAfterSeconds: 5,
      outputSwitchAfterSeconds: 16,
      volumeSchedule: [
        { second: 0, percent: 100 },
        { second: 9, percent: 0 },
        { second: 13, percent: 100 },
        { second: 15, percent: 0 },
        { second: 21, percent: 100 },
      ],
    });
    const r = (await watcher.result) as {
      outputPauseApplied: boolean;
      alternateOutputAvailable: boolean;
      samples: {
        second: number;
        nativeAudio: {
          enabled: boolean;
          rendering: boolean;
          outputStarts: number;
          routeChanges: number;
          recordingEnabled: boolean;
          recordingInitialized: boolean;
        };
        streams: { status: string; audio: { frames: number } }[];
      }[];
      audiovisualTiming: {
        available: boolean;
        outputWindows: { second: number; tones: number[]; samples: number }[];
      };
      nativeAudioAfterClose: {
        outputUsers: number;
        outputDelayMs: number | null;
      };
    };
    await sender.result;
    writeFileSync(info.outputPath("output-recovery.json"), JSON.stringify(r));
    expect(r.outputPauseApplied).toBe(true);
    const level = (a: number, b: number) =>
      Math.max(
        ...r.audiovisualTiming.outputWindows
          .filter((w) => w.second >= a && w.second <= b && w.samples > 40_000)
          .map((w) => w.tones[0]!),
      );
    const full = level(3, 4);
    expect(full).toBeGreaterThan(0.003);
    expect(level(6, 7)).toBeLessThan(full * 0.02);
    expect(level(10, 11)).toBeLessThan(full * 0.02);
    expect(level(17, 18)).toBeLessThan(full * 0.02);
    expect(level(14, 14)).toBeGreaterThan(full * 0.75);
    expect(level(23, 24)).toBeGreaterThan(full * 0.75);
    expect(
      r.samples.some(
        (s) => s.second >= 6 && s.second <= 7 && !s.nativeAudio.rendering,
      ),
    ).toBe(true);
    expect(
      r.samples.some(
        (s) =>
          s.second >= 10 &&
          s.nativeAudio.rendering &&
          s.nativeAudio.outputStarts >= 2,
      ),
    ).toBe(true);
    expect(
      r.samples.every(
        (s) =>
          s.nativeAudio.enabled &&
          !s.nativeAudio.recordingEnabled &&
          !s.nativeAudio.recordingInitialized,
      ),
    ).toBe(true);
    expect(
      r.samples
        .filter((s) => s.second >= 22)
        .every((s) => s.streams.every((t) => t.status === "")),
    ).toBe(true);
    if (r.alternateOutputAvailable)
      expect(r.samples.some((s) => s.nativeAudio.routeChanges >= 2)).toBe(true);
    expect(r.nativeAudioAfterClose.outputUsers).toBe(0);
    expect(r.nativeAudioAfterClose.outputDelayMs).toBeNull();
  } finally {
    watcher?.child.kill();
    sender.child.kill();
    fixture.stop();
  }
});
