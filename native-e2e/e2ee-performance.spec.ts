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
  samples: { second: number; streams: { frames: number }[] }[];
};
test("measure invitation encryption on/off targeting 35 Mbps with one and four watchers", async ({
  request,
  baseURL,
}, testInfo) => {
  test.skip(
    process.env.FREEDOMSTREAM_MEASURE_E2EE !== "1",
    "Explicit opt-in hardware measurement.",
  );
  test.setTimeout(600_000);
  const samples: unknown[] = [];
  const quality = {
    resolution: "p1080",
    fps: 60,
    profile: "motion",
    bitrate: { mode: "manual", bps: 35_000_000 },
  };
  for (let round = 0; round < 2; round++) {
    for (const watchers of [1, 4]) {
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
        const previousNoise = process.env.FREEDOMSTREAM_FIXTURE_NOISE;
        process.env.FREEDOMSTREAM_FIXTURE_NOISE = "1";
        const sender = nativeRequest("--publish-fixture", {
          invitationUrl,
          displayName: "Measured publisher",
          codec: "h264",
          seconds: 39,
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
          delete process.env.FREEDOMSTREAM_FIXTURE_NOISE;
        else process.env.FREEDOMSTREAM_FIXTURE_NOISE = previousNoise;
        const receivers: ReturnType<typeof nativeRequest>[] = [];
        try {
          await sender.reported;
          for (let index = 0; index < watchers; index++)
            receivers.push(
              nativeRequest("--watch-fixture", {
                invitationUrl,
                seconds: 35,
                consumePictures: true,
                downloadLimitMbps: 200,
              }),
            );
          // Exclude initial admission/decode and the fixture's Stop/rewatch phase.
          await delay(20_000);
          const ids = [
            sender.child.pid!,
            ...receivers.map((r) => r.child.pid!),
          ];
          const first = await cpuSeconds(ids);
          const start = performance.now();
          await delay(8000);
          const last = await cpuSeconds(ids);
          const elapsed = (performance.now() - start) / 1000;
          const window = rateSamples.filter(
            (sample) =>
              sample.at >= start && sample.at <= start + elapsed * 1000,
          );
          expect(window.length).toBeGreaterThanOrEqual(5);
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
          expect(
            watched.every((r) =>
              r.samples.some((s) => s.streams.some((t) => t.frames > 5)),
            ),
          ).toBe(true);
          samples.push({
            round,
            watchers,
            encrypted,
            seconds: elapsed,
            processes,
            machinePercent:
              processes.reduce((sum, p) => sum + p.oneCorePercent, 0) /
              cpus().length,
            delivery: sent.delivery,
            encodedBps: sent.metrics.encoded_bps,
            measurementWindow: {
              samples: window.length,
              deliveryBps: mean(window.map((s) => s.deliveryBps)),
              encodedBps: mean(window.map((s) => s.encodedBps)),
              sentFps: mean(window.map((s) => s.sentFps)),
            },
          });
        } finally {
          sender.child.kill();
          for (const receiver of receivers) receiver.child.kill();
        }
      }
    }
  }
  const report = {
    cpu: cpus()[0]?.model,
    logicalProcessors: cpus().length,
    requestedBps: 35_000_000,
    geometry: "1920x1080@60",
    source: "D3D11 snow fixture",
    build: "debug",
    limitations:
      "Two alternating-order rounds; separate native watcher processes, not a real game or four streams decoded in one app. No latency measurement. Inspect actual delivered bitrate; encoder target alone is not achieved throughput.",
    samples,
  };
  writeFileSync(
    testInfo.outputPath("e2ee-cpu.json"),
    JSON.stringify(report, null, 2),
  );
  await testInfo.attach("e2ee-cpu", {
    body: JSON.stringify(report),
    contentType: "application/json",
  });
});
