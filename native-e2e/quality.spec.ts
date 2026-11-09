import { expect, test, type Page } from "@playwright/test";
import { createTestRoom } from "../scripts/test-room.js";
import { nativeRequest } from "./fixtures.js";

async function observe(page: Page) {
  await page.addInitScript(() => {
    const Native = window.RTCPeerConnection;
    const peers: RTCPeerConnection[] = [];
    (window as unknown as { qualityPeers: RTCPeerConnection[] }).qualityPeers =
      peers;
    window.RTCPeerConnection = class extends Native {
      constructor(config?: RTCConfiguration) {
        super(config);
        peers.push(this);
      }
    };
  });
}
async function received(page: Page) {
  return page.evaluate(async () => {
    const result = {
      width: 0,
      height: 0,
      fps: 0,
      bytes: 0,
      decoded: 0,
      at: performance.now(),
      mime: "",
      received: 0,
      dropped: 0,
      lost: 0,
      packets: 0,
      decodeSeconds: 0,
      decoder: "",
      powerEfficient: false,
      playoutMs: 0,
    };
    for (const peer of (
      window as unknown as { qualityPeers: RTCPeerConnection[] }
    ).qualityPeers) {
      for (const receiver of peer.getReceivers()) {
        const preference = receiver as RTCRtpReceiver & {
          jitterBufferTarget?: number | null;
          playoutDelayHint?: number;
        };
        if (receiver.track.kind === "video")
          result.playoutMs = Math.max(
            result.playoutMs,
            preference.jitterBufferTarget ??
              (preference.playoutDelayHint ?? 0) * 1000,
          );
      }
      const stats = await peer.getStats();
      stats.forEach((stat) => {
        if (
          stat.type === "inbound-rtp" &&
          stat.kind === "video" &&
          Number(stat.framesDecoded ?? 0) > 0
        ) {
          result.width = Number(stat.frameWidth ?? 0);
          result.height = Number(stat.frameHeight ?? 0);
          result.fps = Number(stat.framesPerSecond ?? 0);
          result.bytes += Number(stat.bytesReceived ?? 0);
          result.decoded += Number(stat.framesDecoded ?? 0);
          result.mime = stats.get(stat.codecId)?.mimeType?.toLowerCase() ?? "";
          result.received += Number(stat.framesReceived ?? 0);
          result.dropped += Number(stat.framesDropped ?? 0);
          result.lost += Number(stat.packetsLost ?? 0);
          result.packets += Number(stat.packetsReceived ?? 0);
          result.decodeSeconds += Number(stat.totalDecodeTime ?? 0);
          result.decoder = String(stat.decoderImplementation ?? "unknown");
          result.powerEfficient = Boolean(stat.powerEfficientDecoder);
        }
      });
    }
    return result;
  });
}
const settings = (resolution: string, fps: number, bps: number) => ({
  resolution,
  fps,
  profile: "motion",
  bitrate: { mode: "manual", bps },
});

for (const scenario of ["4k120", "live-bitrate", "reconfigure"] as const) {
  test(`native quality ${scenario} uses selected modes and real received media`, async ({
    page,
    request,
    baseURL,
  }) => {
    const created = await createTestRoom(
      request,
      baseURL!,
      `Quality ${scenario} ${Date.now()}`,
    );
    expect(created.status()).toBe(201);
    const { invitationUrl } = await created.json();
    await observe(page);
    await page.goto(invitationUrl);
    await page.getByLabel("Your name", { exact: true }).fill("Quality viewer");
    await page
      .getByRole("button", { name: "Join private room", exact: true })
      .click();
    const fixture = nativeRequest("--publish-fixture", {
      invitationUrl,
      displayName: `Quality ${scenario}`,
      codec: scenario === "live-bitrate" ? "h264" : "h265",
      seconds: scenario === "reconfigure" ? 30 : 45,
      ...(scenario === "4k120"
        ? { fixture4k: true, quality: settings("p2160", 120, 100_000_000) }
        : scenario === "live-bitrate"
          ? {
              quality: settings("p1080", 120, 3_000_000),
              bitrateAfterSeconds: 8,
              nextBitrate: 25_000_000,
            }
          : {
              quality: settings("p1080", 30, 12_000_000),
              reconfigureAfterSeconds: 10,
              nextQuality: settings("p720", 60, 8_000_000),
              nextCodec: "h264",
            }),
    });
    try {
      let measuredFps: number | undefined;
      const card = page.getByRole("article", {
        name: `Quality ${scenario} screen`,
        exact: true,
      });
      await expect(card).toBeVisible({ timeout: 25000 });
      await card.getByRole("button", { name: "Watch", exact: true }).click();
      await expect
        .poll(async () => (await received(page)).decoded, { timeout: 15000 })
        .toBeGreaterThan(10);
      expect((await received(page)).playoutMs).toBe(300);
      if (scenario === "reconfigure") {
        await expect(
          card.getByRole("button", { name: "Watch", exact: true }),
        ).toBeVisible({ timeout: 18000 });
        await card.getByRole("button", { name: "Watch", exact: true }).click();
        await expect
          .poll(async () => (await received(page)).width, { timeout: 15000 })
          .toBe(1280);
        await expect
          .poll(async () => (await received(page)).fps, { timeout: 15000 })
          .toBeGreaterThan(50);
      } else {
        await expect
          .poll(async () => (await received(page)).width, { timeout: 15000 })
          .toBe(scenario === "4k120" ? 3840 : 1920);
        await expect
          .poll(async () => (await received(page)).fps, { timeout: 18000 })
          .toBeGreaterThan(110);
        // Observe a later window, beyond the live bitrate edit, not just one RTP label.
        await page.waitForTimeout(6000);
        const before = await received(page);
        await page.waitForTimeout(5000);
        const stats = await received(page);
        const steadyFps =
          ((stats.decoded - before.decoded) * 1000) / (stats.at - before.at);
        console.log(
          `${scenario}: five-second receive window ${steadyFps.toFixed(1)} fps; instantaneous RTC label ${stats.fps}`,
        );
        await page.waitForTimeout(8000);
        const settled = await received(page);
        await page.waitForTimeout(5000);
        const settledEnd = await received(page);
        measuredFps =
          ((settledEnd.decoded - settled.decoded) * 1000) /
          (settledEnd.at - settled.at);
        console.log(
          `${scenario}: settled five-second receive window ${measuredFps.toFixed(1)} fps`,
        );
        console.log(
          JSON.stringify({
            scenario,
            receivedFrames: settledEnd.received - settled.received,
            decodedFrames: settledEnd.decoded - settled.decoded,
            droppedFrames: settledEnd.dropped - settled.dropped,
            lostPackets: settledEnd.lost - settled.lost,
            receivedPackets: settledEnd.packets - settled.packets,
            decodeMsPerFrame:
              ((settledEnd.decodeSeconds - settled.decodeSeconds) * 1000) /
              Math.max(1, settledEnd.decoded - settled.decoded),
            decoder: settledEnd.decoder,
            powerEfficient: settledEnd.powerEfficient,
          }),
        );
        expect(stats.height).toBe(scenario === "4k120" ? 2160 : 1080);
        console.log(
          `${scenario}: ${stats.width}×${stats.height}, ${stats.fps} received fps, ${stats.mime}`,
        );
      }
      await expect(
        card.getByLabel(`Quality ${scenario} screen received quality`),
      ).toBeVisible({ timeout: 5000 });
      if (scenario === "live-bitrate") {
        await page
          .getByLabel("Playback buffer", { exact: true })
          .selectOption("low-delay");
        await expect.poll(async () => (await received(page)).playoutMs).toBe(0);
        await page
          .getByLabel("Playback buffer", { exact: true })
          .selectOption("smooth");
        await expect
          .poll(async () => (await received(page)).playoutMs)
          .toBe(300);
      }
      const report = (await fixture.result) as {
        mode: { width: number; height: number; fps: number; bitrate: number };
        metrics: {
          fps: number;
          requested_bitrate: number;
          encoder_bitrate: number;
          preview_frames: number;
          encoded_bps: number;
        };
        delivery: { available: boolean; bitrate_bps: number };
        initialSid: string;
        finalSid: string;
        captureStopped: boolean;
      };
      console.log(
        `${scenario}: native ${report.metrics.fps.toFixed(1)} encoded fps, ${(report.metrics.encoded_bps / 1e6).toFixed(1)} Mbps encoded, ${(report.metrics.encoder_bitrate / 1e6).toFixed(1)} Mbps effective target; ${(report.delivery.bitrate_bps / 1e6).toFixed(1)} Mbps sent`,
      );
      if (measuredFps !== undefined) expect(measuredFps).toBeGreaterThan(110);
      expect(report.captureStopped).toBe(true);
      expect(report.metrics.encoder_bitrate).toBeLessThanOrEqual(
        report.mode.bitrate,
      );
      expect(report.delivery.available).toBe(true);
      expect(report.delivery.bitrate_bps).toBeGreaterThan(0);
      if (scenario === "live-bitrate") {
        expect(report.initialSid).toBe(report.finalSid);
        expect(report.mode).toEqual({
          width: 1920,
          height: 1080,
          fps: 120,
          bitrate: 25_000_000,
        });
        expect(report.metrics.requested_bitrate).toBe(25_000_000);
        expect(report.metrics.preview_frames).toBe(1);
      }
      if (scenario === "reconfigure") {
        expect(report.initialSid).not.toBe(report.finalSid);
        expect(report.mode).toEqual({
          width: 1280,
          height: 720,
          fps: 60,
          bitrate: 8_000_000,
        });
      }
      if (scenario === "4k120") {
        expect(report.mode).toEqual({
          width: 3840,
          height: 2160,
          fps: 120,
          bitrate: 100_000_000,
        });
        expect(report.metrics.fps).toBeGreaterThan(115);
      }
    } finally {
      if (fixture.child.exitCode === null) fixture.child.kill();
    }
  });
}
