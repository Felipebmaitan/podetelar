import { writeFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { createTestRoom } from "../scripts/test-room.js";
import { nativeRequest, toneFixtures } from "./fixtures.js";

test("native output independently controls two received streams across mute, Stop and reconnect", async ({
  request,
  baseURL,
}, testInfo) => {
  const created = await createTestRoom(
    request,
    baseURL!,
    "Native output gains",
  );
  expect(created.status()).toBe(201);
  const { invitationUrl } = await created.json();
  const first = toneFixtures(testInfo.outputPath("first"));
  const second = toneFixtures(testInfo.outputPath("second"));
  const senders: ReturnType<typeof nativeRequest>[] = [];
  let receiver: ReturnType<typeof nativeRequest> | undefined;
  try {
    first.game();
    second.game(880);
    for (const [name, application] of [
      ["Volume A", first.application],
      ["Volume B", second.application],
    ]) {
      const sender = nativeRequest("--publish-fixture", {
        invitationUrl,
        displayName: name,
        codec: "h264",
        seconds: 42,
        quality: {
          resolution: "p720",
          fps: 30,
          profile: "motion",
          bitrate: { mode: "manual", bps: 2_000_000 },
        },
        audio: { mode: "application", application },
      });
      senders.push(sender);
      await sender.reported;
    }
    receiver = nativeRequest("--watch-fixture", {
      invitationUrl,
      seconds: 34,
      measureOutputAudio: true,
      reconnectAfterSeconds: 25,
      volumeSchedule: [
        { second: 0, percent: 0, displayName: "Volume A" },
        { second: 4, percent: 100, displayName: "Volume A" },
        { second: 14, percent: 25, displayName: "Volume A" },
        { second: 18, percent: 0, displayName: "Volume A" },
        { second: 22, percent: 100, displayName: "Volume A" },
        { second: 28, percent: 0, displayName: "Volume A" },
      ],
    });
    type Audit = {
      enabled: boolean;
      outputUsers: number;
      outputDelayMs: number | null;
      platformOutputStarts: number;
      platformOutputStops: number;
      recordingEnabled: boolean;
      recordingInitialized: boolean;
    };
    const report = (await receiver.result) as {
      audiovisualTiming: {
        available: boolean;
        outputWindows: {
          second: number;
          tones: number[];
          peak: number;
          channel_difference_peak: number;
          samples: number;
        }[];
      };
      nativeAudioAfterClose: Audit;
      samples: {
        second: number;
        recoveries: number;
        nativeAudio: Audit;
        syncGroups: {
          pairedPublishers: number;
          mismatchedPublishers: number;
          distinctGroups: number;
        };
        streams: { audio: { frames: number } }[];
      }[];
    };
    await Promise.all(senders.map((sender) => sender.result));
    writeFileSync(
      testInfo.outputPath("native-output.json"),
      JSON.stringify(report),
    );
    expect(report.audiovisualTiming.available).toBe(true);
    const windows = report.audiovisualTiming.outputWindows;
    const level = (from: number, to: number, band: number) => {
      const selected = windows.filter(
        (w) => w.second >= from && w.second <= to && w.samples > 40_000,
      );
      expect(selected.length).toBeGreaterThan(0);
      return Math.max(...selected.map((w) => w.tones[band]!));
    };
    const full = level(11, 13, 0);
    expect(full).toBeGreaterThan(0.003);
    // The first source is 440 Hz left / 660 Hz right; B is mono. A real
    // stereo output must retain channel separation after WebRTC's mixer.
    expect(
      Math.max(
        ...windows
          .filter((w) => w.second >= 11 && w.second <= 13)
          .map((w) => w.channel_difference_peak),
      ),
    ).toBeGreaterThan(full * 0.5);
    expect(level(15, 16, 0) / full).toBeGreaterThan(0.12);
    expect(level(15, 16, 0) / full).toBeLessThan(0.4);
    for (const [from, to] of [
      [3, 3],
      [8, 9],
      [19, 20],
      [30, 32],
    ]) {
      expect(level(from!, to!, 0)).toBeLessThan(full * 0.02);
    }
    const other = level(11, 13, 1);
    expect(other).toBeGreaterThan(0.003);
    for (const [from, to] of [
      [3, 3],
      [15, 16],
      [19, 20],
      [30, 32],
    ]) {
      expect(level(from!, to!, 1) / other).toBeGreaterThan(0.75);
      expect(level(from!, to!, 1) / other).toBeLessThan(1.25);
    }
    expect(
      report.samples.some(
        (s) =>
          s.nativeAudio.outputUsers === 2 &&
          s.nativeAudio.outputDelayMs !== null,
      ),
    ).toBe(true);
    expect(
      report.samples
        .filter((s) => s.second >= 8 && s.second <= 9)
        .every((s) => s.nativeAudio.outputUsers === 0),
    ).toBe(true);
    expect(report.samples.some((s) => s.recoveries > 0)).toBe(true);
    expect(
      report.samples.every(
        (s) =>
          !s.nativeAudio.recordingEnabled &&
          !s.nativeAudio.recordingInitialized,
      ),
    ).toBe(true);
    expect(
      report.samples.some(
        (s) =>
          s.syncGroups.pairedPublishers === 2 &&
          s.syncGroups.distinctGroups === 2 &&
          s.syncGroups.mismatchedPublishers === 0,
      ),
    ).toBe(true);
    expect(report.nativeAudioAfterClose.enabled).toBe(true);
    expect(report.nativeAudioAfterClose.outputUsers).toBe(0);
    expect(report.nativeAudioAfterClose.outputDelayMs).toBeNull();
    expect(report.nativeAudioAfterClose.platformOutputStarts).toBeGreaterThan(
      0,
    );
    expect(report.nativeAudioAfterClose.platformOutputStops).toBeGreaterThan(0);
  } finally {
    receiver?.child.kill();
    for (const sender of senders) sender.child.kill();
    first.stop();
    second.stop();
  }
});

test("muted late subscriptions and replacement stay silent beside an audible stream", async ({
  request,
  baseURL,
}, testInfo) => {
  const created = await createTestRoom(
    request,
    baseURL!,
    "Native output transitions",
  );
  expect(created.status()).toBe(201);
  const { invitationUrl } = await created.json();
  const a = toneFixtures(testInfo.outputPath("a"));
  const b = toneFixtures(testInfo.outputPath("b"));
  const senders: ReturnType<typeof nativeRequest>[] = [];
  let receiver: ReturnType<typeof nativeRequest> | undefined;
  try {
    a.game();
    b.game(880);
    for (const [name, application] of [
      ["Muted A", a.application],
      ["Audible B", b.application],
    ]) {
      const sender = nativeRequest("--publish-fixture", {
        invitationUrl,
        displayName: name,
        codec: "h264",
        seconds: 38,
        quality: {
          resolution: "p720",
          fps: 30,
          profile: "motion",
          bitrate: { mode: "manual", bps: 2_000_000 },
        },
        audio: { mode: "application", application },
        ...(name === "Muted A"
          ? {
              reconfigureAfterSeconds: 19,
              nextCodec: "h264",
              nextQuality: {
                resolution: "p720",
                fps: 30,
                profile: "motion",
                bitrate: { mode: "manual", bps: 2_000_000 },
              },
            }
          : {}),
      });
      senders.push(sender);
      await sender.reported;
    }
    receiver = nativeRequest("--watch-fixture", {
      invitationUrl,
      seconds: 28,
      continuousWatch: true,
      measureOutputAudio: true,
      reconnectAfterSeconds: 23,
      volumeSchedule: [{ second: 0, percent: 0, displayName: "Muted A" }],
      watchSchedule: [
        { second: 0, enabled: false, displayName: "Muted A" },
        { second: 6, enabled: true, displayName: "Muted A" },
        { second: 12, enabled: false, displayName: "Muted A" },
        { second: 15, enabled: true, displayName: "Muted A" },
      ],
    });
    const report = (await receiver.result) as {
      audiovisualTiming: {
        available: boolean;
        outputWindows: {
          second: number;
          tones: number[];
          channel_difference_peak: number;
          samples: number;
        }[];
      };
      samples: { second: number; nativeAudio: { outputUsers: number } }[];
    };
    const sent = await Promise.all(senders.map((sender) => sender.result));
    writeFileSync(
      testInfo.outputPath("native-transitions.json"),
      JSON.stringify({ sent, report }),
    );
    expect(report.audiovisualTiming.available).toBe(true);
    const windows = report.audiovisualTiming.outputWindows.filter(
      (w) => w.second >= 3 && w.samples > 0,
    );
    expect(windows.length).toBeGreaterThanOrEqual(24);
    // Inspect every captured buffer's channel difference and each spectral
    // window, including transition seconds; steady-state sampling misses bursts.
    expect(Math.max(...windows.map((w) => w.tones[0]!))).toBeLessThan(0.0004);
    expect(
      Math.max(...windows.map((w) => w.channel_difference_peak)),
    ).toBeLessThan(0.0005);
    expect(
      windows
        .filter(
          (w) => w.samples >= 40_000 && !(23 <= w.second && w.second <= 25),
        )
        .every((w) => w.tones[1]! > 0.01),
    ).toBe(true);
    expect(
      report.samples.some(
        (s) =>
          s.second >= 4 && s.second <= 5 && s.nativeAudio.outputUsers === 1,
      ),
    ).toBe(true);
    expect(
      report.samples.some(
        (s) =>
          s.second >= 8 && s.second <= 11 && s.nativeAudio.outputUsers === 2,
      ),
    ).toBe(true);
    const first = sent[0] as { initialSid: string; finalSid: string };
    expect(first.initialSid).not.toBe(first.finalSid);
  } finally {
    receiver?.child.kill();
    for (const sender of senders) sender.child.kill();
    a.stop();
    b.stop();
  }
});
