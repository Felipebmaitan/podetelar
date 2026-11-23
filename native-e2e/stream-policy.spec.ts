import { expect, test } from "@playwright/test";
import { defaultStreamPolicy, type StreamPolicy } from "@podetelar/contracts";
import { setTimeout as delay } from "node:timers/promises";
import { createTestRoom } from "../scripts/test-room.js";
import { nativeRequest } from "./fixtures.js";

test.skip(
  process.env.PODETELAR_TEST_STREAM_POLICY !== "1",
  "Requires an isolated policy-enabled API and SFU.",
);

for (const codec of ["h264", "h265", "av1"] as const) {
  test(`live ${codec} follows bitrate and geometry changes without losing watch intent`, async ({
    request,
    baseURL,
  }) => {
    expect(new URL(baseURL!).hostname).toMatch(
      /^(localhost|127\.0\.0\.1|\[::1\])$/,
    );
    const headers = { Origin: baseURL! };
    const login = await request.post("/api/admin/login", {
      headers,
      data: { password: process.env.ADMIN_TOKEN },
    });
    expect(login.status()).toBe(200);
    const endpoint = "/api/admin/stream-policy";
    const read = async () =>
      (await (await request.get(endpoint)).json()).policy as StreamPolicy;
    const write = async (policy: StreamPolicy) => {
      const response = await request.put(endpoint, { headers, data: policy });
      expect(response.status()).toBe(200);
    };
    const initial = await read();
    const created = await createTestRoom(
      request,
      baseURL!,
      `Policy acceptance ${codec}`,
    );
    expect(created.status()).toBe(201);
    const { invitationUrl } = await created.json();
    await write({ ...defaultStreamPolicy, revision: initial.revision });
    let sender: ReturnType<typeof nativeRequest> | undefined;
    let receiver: ReturnType<typeof nativeRequest> | undefined;
    try {
      sender = nativeRequest("--publish-fixture", {
        invitationUrl,
        displayName: "Policy publisher",
        codec,
        seconds: 22,
        fixture4k: true,
        followStreamPolicy: true,
        quality: {
          resolution: "p1440",
          fps: 60,
          bitrate: { mode: "manual", bps: 20_000_000 },
          profile: "motion",
        },
      });
      await sender.reported;
      receiver = nativeRequest("--watch-fixture", {
        invitationUrl,
        seconds: 20,
        continuousWatch: true,
      });
      await delay(4_000);
      await write({ ...(await read()), maxBitrateBps: 10_000_000 });
      await delay(5_000);
      await write({ ...(await read()), maxResolution: 1080, maxFps: 30 });
      const [sent, watched] = (await Promise.all([
        sender.result,
        receiver.result,
      ])) as [
        {
          initialSid: string;
          finalSid: string;
          mode: { width: number; height: number; fps: number; bitrate: number };
          metrics: { frames: number; encoder_bitrate: number };
          previewCaptures: number;
        },
        {
          samples: {
            streams: {
              watching: boolean;
              pictureSize: number[] | null;
              frames: number;
            }[];
          }[];
        },
      ];
      expect(sent.finalSid).not.toBe(sent.initialSid);
      expect(sent.mode).toEqual({
        width: 1920,
        height: 1080,
        fps: 30,
        bitrate: 10_000_000,
      });
      expect(sent.metrics.frames).toBeGreaterThan(25);
      expect(sent.metrics.encoder_bitrate).toBeLessThanOrEqual(10_000_000);
      expect(sent.previewCaptures).toBe(1);
      for (const [width, height] of [
        [2560, 1440],
        [1920, 1080],
      ]) {
        expect(
          watched.samples.some(({ streams }) =>
            streams.some(
              ({ watching, frames, pictureSize }) =>
                watching &&
                frames > 5 &&
                pictureSize?.[0] === width &&
                pictureSize?.[1] === height,
            ),
          ),
        ).toBe(true);
      }
    } finally {
      sender?.child.kill();
      receiver?.child.kill();
      await write({ ...initial, revision: (await read()).revision });
    }
  });
}
