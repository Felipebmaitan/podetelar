// Opt-in real-media measurement, not a CPU performance assertion.
import { execFile } from "node:child_process";
import { writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { setTimeout as delay } from "node:timers/promises";
import { promisify } from "node:util";
import { expect, test } from "@playwright/test";
import { createTestRoom } from "../scripts/test-room.js";
import { nativeRequest } from "./fixtures.js";

const execute = promisify(execFile);
async function cpuSeconds(ids: number[]) {
  if (!ids.every((id) => Number.isInteger(id) && id > 0))
    throw new Error("Invalid measurement process.");
  const { stdout } = await execute(
    "powershell.exe",
    [
      "-NoProfile",
      "-WindowStyle",
      "Hidden",
      "-Command",
      `@(${ids.join(",")}) | ForEach-Object { $measurementProcess = Get-Process -Id $_ -ErrorAction Stop; [pscustomobject]@{ id = $_; cpu = $measurementProcess.CPU } } | ConvertTo-Json -Compress`,
    ],
    { windowsHide: true, timeout: 5000 },
  );
  const parsed = JSON.parse(stdout);
  return (Array.isArray(parsed) ? parsed : [parsed]) as {
    id: number;
    cpu: number;
  }[];
}
type Sent = {
  delivery: { bitrate_bps: number; sent_fps: number };
  metrics: { encoded_bps: number };
};
type Received = {
  samples: {
    measurementTimeMs: number;
    receivedBytes: number;
    streams: { frames: number; watching: boolean }[];
    mediaTiming: { kind: string; decodedFrames: number }[];
  }[];
};
test("measure invitation encryption on/off targeting 35 Mbps with one and four watchers", async ({
  request,
  baseURL,
}, testInfo) => {
  const pilot = process.env.PODETELAR_MEASURE_E2EE === "pilot";
  test.skip(
    !pilot && process.env.PODETELAR_MEASURE_E2EE !== "1",
    "Explicit opt-in hardware measurement.",
  );
  test.setTimeout(900_000);
  const samples: unknown[] = [];
  const report = {
    cpu: cpus()[0]?.model,
    logicalProcessors: cpus().length,
    requestedBps: 35_000_000,
    pilot,
    geometry: "1920x1080@60",
    source: "D3D11 snow fixture",
    build: process.env.PODETELAR_MEASUREMENT_BUILD ?? "unspecified",
    limitations:
      "The full run uses two alternating-order rounds; the optional pilot uses one publisher and one watcher for a single pair. Separate native watcher processes, not a real game or four streams decoded in one app. The synthetic watcher requests decoded pictures without a UI renderer. A qualifying window requires measured sender and every receiver within 10% of target. No latency measurement.",
    samples,
  };
  const quality = {
    resolution: "p1080",
    fps: 60,
    profile: "motion",
    bitrate: { mode: "manual", bps: 35_000_000 },
  };
  for (let round = 0; round < (pilot ? 1 : 2); round++) {
    for (const watchers of pilot ? [1] : [1, 4]) {
      for (const encrypted of round === 0 ? [false, true] : [true, false]) {
        const created = await createTestRoom(
          request,
          baseURL!,
          "Local encryption CPU measurement",
        );
        expect(created.status()).toBe(201);
        let { invitationUrl } = await created.json();
        if (encrypted) {
          const protectedResult = (await nativeRequest("--protect-invitation", {
            invitationUrl,
          }).result) as { invitationUrl: string };
          invitationUrl = protectedResult.invitationUrl;
        }
        const previousNoise = process.env.PODETELAR_FIXTURE_NOISE;
        process.env.PODETELAR_FIXTURE_NOISE = "1";
        const sender = nativeRequest("--publish-fixture", {
          invitationUrl,
          displayName: "Measured publisher",
          codec: "h264",
          seconds: 84,
          quality,
          sampleRates: true,
        });
        const rateSamples: {
          at: number;
          deliveryBps: number;
          encodedBps: number;
          sentFps: number;
        }[] = [];
        let pendingRates = "";
        sender.child.stdout!.on("data", (data: Buffer) => {
          pendingRates = (pendingRates + data.toString()).slice(-65536);
          const lines = pendingRates.split("\n");
          pendingRates = lines.pop()!;
          for (const line of lines) {
            try {
              const sample = JSON.parse(line);
              if (
                sample.rateSample === true &&
                Number.isFinite(sample.deliveryBps) &&
                Number.isFinite(sample.encodedBps) &&
                Number.isFinite(sample.sentFps)
              )
                rateSamples.push({
                  at: performance.now(),
                  deliveryBps: sample.deliveryBps,
                  encodedBps: sample.encodedBps,
                  sentFps: sample.sentFps,
                });
            } catch {
              /* Partial/non-metric output is not included in reports. */
            }
          }
        });
        if (previousNoise === undefined)
          delete process.env.PODETELAR_FIXTURE_NOISE;
        else process.env.PODETELAR_FIXTURE_NOISE = previousNoise;
        const receivers: ReturnType<typeof nativeRequest>[] = [];
        try {
          await sender.reported;
          for (let index = 0; index < watchers; index++)
            receivers.push(
              nativeRequest("--watch-fixture", {
                invitationUrl,
                seconds: 80,
                continuousWatch: true,
                sampleRates: true,
                sampleAudio: false,
                consumePictures: true,
                downloadLimitMbps: 200,
              }),
            );
          // WebRTC ramps its bandwidth estimate from startup. Keep subscriptions
          // uninterrupted and wait for actual delivery before sampling process CPU.
          const warmupStart = performance.now();
          const minimumBps = quality.bitrate.bps * 0.9;
          do {
            await delay(1000);
            const recent = rateSamples.slice(-4);
            if (
              recent.length === 4 &&
              recent.every(
                (sample) =>
                  sample.deliveryBps >= minimumBps && sample.sentFps >= 55,
              )
            )
              break;
          } while (performance.now() - warmupStart < 60_000);
          const warmupSeconds = (performance.now() - warmupStart) / 1000;
          const ids = [
            sender.child.pid!,
            ...receivers.map((r) => r.child.pid!),
          ];
          const first = await cpuSeconds(ids);
          const start = performance.now();
          const wallStart = Date.now();
          await delay(12_000);
          const last = await cpuSeconds(ids);
          const elapsed = (performance.now() - start) / 1000;
          const wallEnd = Date.now();
          const window = rateSamples.filter(
            (sample) =>
              sample.at >= start && sample.at <= start + elapsed * 1000,
          );
          expect(window.length).toBeGreaterThanOrEqual(9);
          const mean = (values: number[]) =>
            values.reduce((sum, value) => sum + value, 0) / values.length;
          const processes = last.map((sample) => ({
            role: sample.id === sender.child.pid ? "publisher" : "watcher",
            oneCorePercent:
              (100 *
                (sample.cpu - first.find((s) => s.id === sample.id)!.cpu)) /
              elapsed,
          }));
          const [sent, ...watched] = await Promise.all([
            sender.result as Promise<Sent>,
            ...receivers.map((r) => r.result as Promise<Received>),
          ]);
          const received = watched.map((receiver) => {
            const inside = receiver.samples.filter(
              (s) =>
                s.measurementTimeMs >= wallStart &&
                s.measurementTimeMs <= wallEnd,
            );
            expect(inside.length).toBeGreaterThanOrEqual(9);
            expect(
              inside.every(
                (s) => s.streams.length === 1 && s.streams[0]!.watching,
              ),
            ).toBe(true);
            const first = inside[0]!;
            const last = inside.at(-1)!;
            const seconds =
              (last.measurementTimeMs - first.measurementTimeMs) / 1000;
            expect(seconds).toBeGreaterThanOrEqual(8);
            expect(last.streams[0]!.frames).toBeGreaterThan(
              first.streams[0]!.frames,
            );
            const rtcDecoded = (sample: (typeof inside)[number]) =>
              sample.mediaTiming
                .filter((t) => t.kind === "video")
                .reduce((total, t) => total + t.decodedFrames, 0);
            return {
              bps: ((last.receivedBytes - first.receivedBytes) * 8) / seconds,
              seconds,
              rtcDecodedFps: (rtcDecoded(last) - rtcDecoded(first)) / seconds,
              pictureFrames: last.streams[0]!.frames - first.streams[0]!.frames,
            };
          });
          const sentBps = mean(window.map((s) => s.deliveryBps));
          const nearTarget = (bps: number) =>
            bps >= minimumBps && bps <= quality.bitrate.bps * 1.1;
          samples.push({
            round,
            watchers,
            encrypted,
            seconds: elapsed,
            warmupSeconds,
            withinTenPercentOfTarget:
              nearTarget(sentBps) && received.every((r) => nearTarget(r.bps)),
            processes,
            machinePercent:
              processes.reduce((sum, p) => sum + p.oneCorePercent, 0) /
              cpus().length,
            delivery: sent.delivery,
            encodedBps: sent.metrics.encoded_bps,
            measurementWindow: {
              samples: window.length,
              deliveryBps: sentBps,
              slowestSenderSampleBps: Math.min(
                ...window.map((s) => s.deliveryBps),
              ),
              fastestSenderSampleBps: Math.max(
                ...window.map((s) => s.deliveryBps),
              ),
              received,
              encodedBps: mean(window.map((s) => s.encodedBps)),
              sentFps: mean(window.map((s) => s.sentFps)),
            },
          });
          writeFileSync(
            testInfo.outputPath("e2ee-cpu.json"),
            JSON.stringify(report, null, 2),
          );
          console.log(
            `Round ${round + 1}, ${watchers} viewer(s), E2EE ${encrypted ? "on" : "off"}: ${(sentBps / 1_000_000).toFixed(2)} Mbps sent; ${received.map((r) => (r.bps / 1_000_000).toFixed(2)).join(", ")} Mbps received.`,
          );
        } finally {
          sender.child.kill();
          for (const receiver of receivers) receiver.child.kill();
        }
      }
    }
  }
  await testInfo.attach("e2ee-cpu", {
    body: JSON.stringify(report),
    contentType: "application/json",
  });
});
