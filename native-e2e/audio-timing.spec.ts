import { writeFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { createTestRoom } from "../scripts/test-room.js";
import { nativeRequest, toneFixtures } from "./fixtures.js";

test("real playback process-loopback reduces the legacy output delay", async ({}, testInfo) => {
  type Report = {
    sinkBufferUs: number;
    delaysMs: number[];
    physicalSpeakerTiming: string;
  };
  const legacy = (await nativeRequest("--qualify-playback", {}, ["legacy"])
    .result) as Report;
  const report = (await nativeRequest("--qualify-playback", {})
    .result) as Report;
  writeFileSync(
    testInfo.outputPath("loopback.json"),
    JSON.stringify({ legacy, corrected: report }),
  );
  expect(legacy.sinkBufferUs).toBe(200_000);
  expect(report.sinkBufferUs).toBe(40_000);
  expect(legacy.delaysMs).toHaveLength(11);
  expect(report.delaysMs).toHaveLength(11);
  expect(report.delaysMs.every((ms) => ms > 0 && ms < 900)).toBe(true);
  expect(report.physicalSpeakerTiming).toBe("unavailable");
  const median = (samples: number[]) => [...samples].sort((a, b) => a - b)[5]!;
  // A paired device qualification, not a universal absolute latency promise.
  expect(median(legacy.delaysMs) - median(report.delaysMs)).toBeGreaterThan(75);
});

for (const codec of ["h264", "av1"] as const) {
  test(`native ${codec} audio and video timing counters through the actual SFU`, async ({
    request,
    baseURL,
  }, testInfo) => {
    const created = await createTestRoom(
      request,
      baseURL!,
      "A/V timing stages",
    );
    expect(created.status()).toBe(201);
    const { invitationUrl } = await created.json();
    const tones = toneFixtures(testInfo.outputPath("tones"));
    const sender = nativeRequest("--publish-fixture", {
      invitationUrl,
      displayName: "Timing fixture",
      codec,
      seconds: 24,
      sampleRates: true,
      quality: {
        resolution: "p1080",
        fps: 60,
        profile: "motion",
        bitrate: { mode: "manual", bps: 6_000_000 },
      },
      audio: { mode: "application", application: tones.application },
    });
    let receiver: ReturnType<typeof nativeRequest> | undefined;
    try {
      await sender.reported;
      tones.game();
      receiver = nativeRequest("--watch-fixture", {
        invitationUrl,
        seconds: 20,
        diagnostics: true,
      });
      const [sent, received] = await Promise.all([
        sender.result,
        receiver.result,
      ]);
      const watched = received as {
        closed: boolean;
        samples: {
          mediaTiming: { kind: string; bytes: number; decodedFrames: number }[];
          streams: {
            frames: number;
            codec: string;
            audio: { frames: number; tones: number[] };
          }[];
        }[];
      };
      writeFileSync(
        testInfo.outputPath("timing-stages.json"),
        JSON.stringify({ codec, sent, received }),
      );
      expect(watched.closed).toBe(true);
      const streams = watched.samples.flatMap((s) => s.streams);
      expect(
        streams.some(
          (s) => s.frames > 60 && s.codec.toLowerCase().includes(codec),
        ),
      ).toBe(true);
      expect(
        streams.some(
          (s) => s.audio.frames > 48_000 && s.audio.tones[0]! > 0.005,
        ),
      ).toBe(true);
      const timing = watched.samples.flatMap((s) => s.mediaTiming);
      expect(timing.some((s) => s.kind === "audio" && s.bytes > 0)).toBe(true);
      expect(
        timing.some((s) => s.kind === "video" && s.decodedFrames > 60),
      ).toBe(true);
      // These counters describe stages, not the relative timing of matching
      // audiovisual events or physical screen/speaker presentation.
    } finally {
      sender.child.kill();
      receiver?.child.kill();
      tones.stop();
    }
  });
}
