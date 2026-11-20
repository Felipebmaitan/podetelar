import { expect, test } from "@playwright/test";
import { createTestRoom } from "../scripts/test-room.js";
import { nativeRequest } from "./fixtures.js";

for (const codec of ["av1", "h264"] as const) {
  test(`advanced settings bitrate applies to a live ${codec} publication`, async ({
    request,
    baseURL,
  }, testInfo) => {
    const created = await createTestRoom(
      request,
      baseURL!,
      `Advanced settings ${codec}`,
    );
    expect(created.status()).toBe(201);
    const { invitationUrl } = await created.json();
    const result = (await nativeRequest("--publish-fixture", {
      invitationUrl,
      displayName: "Settings acceptance",
      codec,
      seconds: 12,
      bitrateAfterSeconds: 4,
      nextBitrate: 12_000_000,
      quality: {
        resolution: "p1080",
        fps: 60,
        profile: "motion",
        bitrate: { mode: "manual", bps: 20_000_000 },
      },
    }).result) as {
      initialSid: string;
      finalSid: string;
      captureStopped: boolean;
      mode: { bitrate: number };
      metrics: { encoder_bitrate: number; frames: number };
    };
    expect(result.finalSid).toBe(result.initialSid);
    expect(result.captureStopped).toBe(true);
    expect(result.mode.bitrate).toBe(12_000_000);
    expect(result.metrics.encoder_bitrate).toBeGreaterThan(0);
    expect(result.metrics.encoder_bitrate).toBeLessThanOrEqual(12_000_000);
    expect(result.metrics.frames).toBeGreaterThan(60);
    await testInfo.attach("live-bitrate-acceptance", {
      body: JSON.stringify({
        codec,
        publicationUnchanged: result.initialSid === result.finalSid,
        requestedBitrate: result.mode.bitrate,
        ...result.metrics,
        captureStopped: result.captureStopped,
      }),
      contentType: "application/json",
    });
  });
}
