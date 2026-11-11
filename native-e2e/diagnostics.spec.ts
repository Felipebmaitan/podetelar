import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { expect, test } from "@playwright/test";
import { createTestRoom } from "../scripts/test-room.js";
import { nativeRequest } from "./fixtures.js";

type Report = {
  samples: {
    second: number;
    watching: boolean;
    streams: {
      frames: number;
      diagnosticRates: [number, number, number] | null;
      diagnosticAttempts: number;
      diagnosticSuccesses: number;
      diagnosticSender?: {
        generation: string;
        stale: boolean;
        sample: {
          window_ms: number;
          metrics: Record<string, number | null>;
        } | null;
      } | null;
    }[];
  }[];
};
test("on-demand AV1 120 FPS publisher -> authenticated server -> two native viewers", async ({
  request,
  baseURL,
}, info) => {
  test.setTimeout(110000);
  const created = await createTestRoom(
    request,
    baseURL!,
    "Diagnostic acceptance",
  );
  expect(created.status()).toBe(201);
  const { invitationUrl } = await created.json();
  const sender = nativeRequest("--publish-fixture", {
    invitationUrl,
    displayName: "Diagnostic publisher",
    codec: "av1",
    seconds: 44,
    quality: {
      resolution: "p1080",
      fps: 120,
      profile: "motion",
      bitrate: { mode: "manual", bps: 25000000 },
    },
  });
  await sender.reported;
  const first = nativeRequest("--watch-fixture", {
    invitationUrl,
    seconds: 38,
    diagnostics: true,
    consumePictures: true,
  });
  const second = nativeRequest("--watch-fixture", {
    invitationUrl,
    seconds: 24,
    downloadLimitMbps: 6,
    diagnostics: true,
  });
  try {
    const [sent, a, b] = await Promise.all([
      sender.result,
      first.result as Promise<Report>,
      second.result as Promise<Report>,
    ]);
    const values = (report: Report) =>
      report.samples.flatMap((s) =>
        s.streams.flatMap((t) =>
          t.diagnosticSender &&
          !t.diagnosticSender.stale &&
          t.diagnosticSender.sample
            ? [t.diagnosticSender.sample]
            : [],
        ),
      );
    expect(values(a).length).toBeGreaterThan(0);
    expect(
      values(b).length,
      JSON.stringify(
        b.samples.map((s) => ({
          second: s.second,
          watching: s.watching,
          diagnostics: s.streams.map((t) => ({
            attempts: t.diagnosticAttempts,
            successes: t.diagnosticSuccesses,
            stale: t.diagnosticSender?.stale,
          })),
        })),
      ),
    ).toBeGreaterThan(0);
    expect(
      values(a).some(
        (s) =>
          s.metrics.requested_fps === 120 &&
          s.metrics.requested_bps === 25000000 &&
          (s.metrics.capture_fps ?? 0) > 0 &&
          (s.metrics.encoded_fps ?? 0) > 0 &&
          (s.metrics.sent_bps ?? 0) > 0,
      ),
    ).toBe(true);
    expect(
      a.samples
        .filter((s) => s.second > 20)
        .some((s) =>
          s.streams.some(
            (t) => t.diagnosticSender?.sample && !t.diagnosticSender.stale,
          ),
        ),
    ).toBe(true);
  } finally {
    sender.child.kill();
    first.child.kill();
    second.child.kill();
  }
});

test("diagnostics distinguish isolated SFU delivery interruption and publisher stall", async ({
  request,
  baseURL,
}) => {
  test.skip(
    !process.env.FREEDOMSTREAM_TEST_SFU_PID ||
      !process.env.FREEDOMSTREAM_TEST_SFU_EXE,
    "Requires the owned local native-room-smoke SFU.",
  );
  test.setTimeout(110000);
  const created = await createTestRoom(
    request,
    baseURL!,
    "Diagnostic interruptions",
  );
  expect(created.status()).toBe(201);
  const { invitationUrl } = await created.json();
  const sender = nativeRequest("--publish-fixture", {
    invitationUrl,
    displayName: "Interruption publisher",
    codec: "av1",
    seconds: 44,
    quality: {
      resolution: "p1080",
      fps: 120,
      profile: "motion",
      bitrate: { mode: "manual", bps: 25000000 },
    },
  });
  await sender.reported;
  const viewer = nativeRequest("--watch-fixture", {
    invitationUrl,
    seconds: 38,
    diagnostics: true,
    consumePictures: true,
  });
  const pause = async (pid: number, executable: string) => {
    await promisify(execFile)(
      "powershell.exe",
      [
        "-NoProfile",
        "-WindowStyle",
        "Hidden",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        resolve("scripts/pause-diagnostic-fixture.ps1"),
        "-TestProcessId",
        String(pid),
        "-ExpectedExecutable",
        executable,
        "-PauseMilliseconds",
        "3000",
      ],
      { windowsHide: true, timeout: 12000 },
    );
  };
  try {
    await delay(15000);
    await pause(
      Number(process.env.FREEDOMSTREAM_TEST_SFU_PID),
      process.env.FREEDOMSTREAM_TEST_SFU_EXE!,
    );
    await delay(6000);
    await pause(sender.child.pid!, process.env.FREEDOMSTREAM_NATIVE_EXE!);
    const [sent, received] = await Promise.all([
      sender.result,
      viewer.result as Promise<Report>,
    ]);
    const windows = received.samples.filter((s) => s.second > 12);
    expect(
      windows
        .filter((s) => s.second < 24)
        .some((s) =>
          s.streams.some((t) => t.diagnosticRates && t.diagnosticRates[0] < 90),
        ),
    ).toBe(true);
    expect(
      windows
        .filter((s) => s.second > 31)
        .some((s) =>
          s.streams.some((t) => t.diagnosticRates && t.diagnosticRates[0] > 90),
        ),
    ).toBe(true);
    const senderWindows = windows.flatMap((s) =>
      s.streams.flatMap((t) =>
        t.diagnosticSender &&
        !t.diagnosticSender.stale &&
        t.diagnosticSender.sample
          ? [t.diagnosticSender.sample.metrics]
          : [],
      ),
    );
    expect(
      senderWindows.some(
        (m) =>
          m.requested_fps === 120 &&
          m.capture_fps !== null &&
          m.capture_fps! < 100,
      ),
    ).toBe(true);
    expect(
      senderWindows.some(
        (m) => m.requested_fps === 120 && (m.capture_fps ?? 0) > 110,
      ),
    ).toBe(true);
    expect((sent as { captureStopped: boolean }).captureStopped).toBe(true);
  } finally {
    sender.child.kill();
    viewer.child.kill();
  }
});
