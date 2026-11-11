import { expect, test } from "@playwright/test";
import { createTestRoom } from "../scripts/test-room.js";
import { nativeRequest } from "./fixtures.js";

type Report = {
  samples: {
    second: number;
    watching: boolean;
    streams: {
      frames: number;
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
    expect(values(b).length).toBeGreaterThan(0);
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
