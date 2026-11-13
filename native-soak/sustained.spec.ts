import { spawn } from "node:child_process";
import { join } from "node:path";
import { writeFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { createTestRoom } from "../scripts/test-room.js";
import { nativeRequest, toneFixtures } from "../native-e2e/fixtures.js";

test("thirty-minute H264 and AV1 captured A/V timing without an accumulating offset", async ({
  request,
  baseURL,
}, info) => {
  const seconds = 1800;
  const directory = info.outputPath("source");
  const fixture = toneFixtures(directory);
  const source = spawn(
    join(directory, "Game.exe"),
    ["--av-timing-source", String(seconds + 45)],
    { windowsHide: false, stdio: "ignore" },
  );
  const children: ReturnType<typeof nativeRequest>[] = [];
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
          return !!sourceId;
        },
        { timeout: 15_000 },
      )
      .toBe(true);
    const watchers: {
      codec: string;
      watcher: ReturnType<typeof nativeRequest>;
    }[] = [];
    for (const codec of ["h264", "av1"]) {
      const created = await createTestRoom(
        request,
        baseURL!,
        `Sustained ${codec}`,
      );
      expect(created.status()).toBe(201);
      const { invitationUrl } = await created.json();
      const sender = nativeRequest("--publish-fixture", {
        invitationUrl,
        displayName: "Sustained source",
        codec,
        sourceId,
        seconds: seconds + 15,
        quality: {
          resolution: "source",
          fps: 60,
          profile: "motion",
          bitrate: { mode: "manual", bps: 3_000_000 },
        },
        audio: { mode: "application", application: fixture.application },
      });
      children.push(sender);
      await sender.reported;
      const watcher = nativeRequest("--watch-fixture", {
        invitationUrl,
        seconds,
        soak: true,
        continuousWatch: true,
        audiovisualTiming: true,
      });
      watcher.child.stderr.on("data", (data: Buffer) => {
        for (const line of data.toString().split(/\r?\n/)) {
          if (
            /^soak-progress second=\d+ video=\d+ audio=\d+ rendering=(true|false)$/.test(
              line.trim(),
            )
          )
            console.log(codec, line.trim());
        }
      });
      children.push(watcher);
      watchers.push({ codec, watcher });
    }
    for (const { codec, watcher } of watchers) {
      const r = (await watcher.result) as {
        closed: boolean;
        nativeAudioAfterClose: { outputUsers: number };
        audiovisualTiming: {
          available: boolean;
          videoEvents: number;
          audioEvents: number;
          pairedEvents: number;
          videoMailboxToAudioLoopbackMs: number[];
          videoEventTimesSeconds: number[];
          audioEventTimesSeconds: number[];
        };
        samples: {
          second: number;
          nativeAudio: {
            rendering: boolean;
            recordingEnabled: boolean;
            recordingInitialized: boolean;
            outputFailures: number;
          };
          streams: {
            frames: number;
            status: string;
            audio: { frames: number };
          }[];
        }[];
      };
      writeFileSync(info.outputPath(`soak-${codec}.json`), JSON.stringify(r));
      const timing = r.audiovisualTiming;
      expect(timing.available).toBe(true);
      expect(timing.videoEvents).toBeGreaterThan(1000);
      expect(timing.audioEvents).toBeGreaterThan(1000);
      // Both branches carry equal-length markers from a nonrepeating sequence.
      // Retain both event timelines and reject missing events beyond startup/end.
      expect(
        Math.abs(timing.videoEvents - timing.audioEvents),
      ).toBeLessThanOrEqual(2);
      expect(
        timing.pairedEvents / Math.max(timing.videoEvents, timing.audioEvents),
      ).toBeGreaterThan(0.95);
      const median = (a: number[]) =>
        [...a].sort((x, y) => x - y)[Math.floor(a.length / 2)]!;
      const start = timing.videoMailboxToAudioLoopbackMs.slice(10, 70);
      const end = timing.videoMailboxToAudioLoopbackMs.slice(-60);
      const drift = median(end) - median(start);
      writeFileSync(
        info.outputPath(`summary-${codec}.json`),
        JSON.stringify({
          codec,
          seconds,
          pairedEvents: timing.pairedEvents,
          startMedianMs: median(start),
          endMedianMs: median(end),
          driftMs: drift,
          physicalAvSync: "unavailable",
        }),
      );
      expect(Math.abs(drift)).toBeLessThan(20);
      expect(timing.videoEventTimesSeconds.at(-1)).toBeGreaterThan(seconds - 5);
      expect(timing.audioEventTimesSeconds.at(-1)).toBeGreaterThan(seconds - 5);
      const steady = r.samples.filter((s) => s.second >= 30);
      expect(steady.length).toBeGreaterThanOrEqual(58);
      expect(
        steady.every(
          (s) =>
            s.nativeAudio.rendering &&
            !s.nativeAudio.recordingEnabled &&
            !s.nativeAudio.recordingInitialized &&
            s.nativeAudio.outputFailures === 0,
        ),
      ).toBe(true);
      expect(steady.every((s) => s.streams.every((t) => t.status === ""))).toBe(
        true,
      );
      for (let i = 1; i < steady.length; i++) {
        expect(steady[i]!.streams[0]!.frames).toBeGreaterThan(
          steady[i - 1]!.streams[0]!.frames,
        );
        expect(steady[i]!.streams[0]!.audio.frames).toBeGreaterThan(
          steady[i - 1]!.streams[0]!.audio.frames,
        );
      }
      expect(r.closed).toBe(true);
      expect(r.nativeAudioAfterClose.outputUsers).toBe(0);
    }
    await Promise.all(children.map((child) => child.result));
  } finally {
    for (const child of children) child.child.kill();
    source.kill();
    fixture.stop();
  }
});
