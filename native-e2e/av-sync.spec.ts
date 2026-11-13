import { spawn } from "node:child_process";
import { join } from "node:path";
import { writeFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { createTestRoom } from "../scripts/test-room.js";
import { nativeRequest, toneFixtures } from "./fixtures.js";

for (const codec of ["h264", "av1"] as const) {
  test(`captured flash/click ${codec} event timing through native SFU playback`, async ({
    request,
    baseURL,
  }, testInfo) => {
    const directory = testInfo.outputPath("source");
    const fixture = toneFixtures(directory);
    const source = spawn(join(directory, "Game.exe"), ["--av-timing-source"], {
      // This is our GUI-subsystem source, not a console helper. Its window must
      // be visible for real WGC discovery/capture; no terminal is created.
      windowsHide: false,
      stdio: "ignore",
    });
    let sender: ReturnType<typeof nativeRequest> | undefined;
    let receiver: ReturnType<typeof nativeRequest> | undefined;
    try {
      let sourceId: string | undefined;
      await expect
        .poll(
          async () => {
            const sources = (await nativeRequest("--sources", {}).result) as {
              id: string;
            }[];
            sourceId = sources.find(
              (s) =>
                s.id.startsWith("window:") && s.id.endsWith(`:${source.pid}`),
            )?.id;
            return Boolean(sourceId);
          },
          { timeout: 10_000 },
        )
        .toBe(true);
      const created = await createTestRoom(
        request,
        baseURL!,
        "Captured A/V events",
      );
      expect(created.status()).toBe(201);
      const { invitationUrl } = await created.json();
      sender = nativeRequest("--publish-fixture", {
        invitationUrl,
        displayName: "Flash/click fixture",
        codec,
        sourceId,
        seconds: 36,
        quality: {
          resolution: "source",
          fps: 60,
          profile: "motion",
          bitrate: { mode: "manual", bps: 3_000_000 },
        },
        audio: { mode: "application", application: fixture.application },
      });
      await sender.reported;
      receiver = nativeRequest("--watch-fixture", {
        invitationUrl,
        seconds: 30,
        audiovisualTiming: true,
      });
      const [sent, received] = await Promise.all([
        sender.result,
        receiver.result,
      ]);
      const report = received as {
        audiovisualTiming: {
          available: boolean;
          pairedEvents: number;
          videoEvents: number;
          audioEvents: number;
          videoMailboxToAudioLoopbackMs: number[];
          physicalAvSync: string;
        };
      };
      writeFileSync(
        testInfo.outputPath("av-events.json"),
        JSON.stringify({ codec, sent, received }),
      );
      expect(report.audiovisualTiming.available).toBe(true);
      expect(report.audiovisualTiming.pairedEvents).toBeGreaterThanOrEqual(8);
      expect(
        report.audiovisualTiming.pairedEvents /
          Math.min(
            report.audiovisualTiming.videoEvents,
            report.audiovisualTiming.audioEvents,
          ),
      ).toBeGreaterThanOrEqual(0.8);
      expect(report.audiovisualTiming.physicalAvSync).toBe("unavailable");
      // Capture/callback timing is an instrumented proxy. Physical display and
      // speaker acceptance must not be inferred from this software-stage report.
    } finally {
      receiver?.child.kill();
      sender?.child.kill();
      source.kill();
      fixture.stop();
    }
  });
}
