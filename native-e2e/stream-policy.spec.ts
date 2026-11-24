import { expect, test } from "@playwright/test";
import { setTimeout as delay } from "node:timers/promises";
import { createTestRoom } from "../scripts/test-room.js";
import { nativeRequest, policyAdministrator } from "./fixtures.js";

test.skip(
  process.env.PODETELAR_TEST_STREAM_POLICY !== "1",
  "Requires an isolated policy-enabled API and SFU.",
);

for (const codec of ["h264", "h265", "av1"] as const) {
  test(`live ${codec} follows bitrate and geometry changes without losing watch intent`, async ({
    request,
    baseURL,
  }) => {
    const { read, write, restore } = await policyAdministrator(
      request,
      baseURL!,
    );
    let sender: ReturnType<typeof nativeRequest> | undefined;
    let receiver: ReturnType<typeof nativeRequest> | undefined;
    try {
      const created = await createTestRoom(
        request,
        baseURL!,
        `Policy acceptance ${codec}`,
      );
      expect(created.status()).toBe(201);
      const { invitationUrl } = await created.json();
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
        watchOncePerPublisher: true,
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
      await restore();
    }
  });
}

test("the enforced removal reaches a native publisher that ignores a new cap", async ({
  request,
  baseURL,
}) => {
  const { read, write, restore } = await policyAdministrator(request, baseURL!);
  let sender: ReturnType<typeof nativeRequest> | undefined;
  try {
    const created = await createTestRoom(
      request,
      baseURL!,
      "Native stream removal",
    );
    expect(created.status()).toBe(201);
    const { invitationUrl } = await created.json();
    sender = nativeRequest("--publish-fixture", {
      invitationUrl,
      displayName: "Noncompliant native publisher",
      codec: "h264",
      seconds: 20,
      quality: {
        resolution: "p1080",
        fps: 30,
        bitrate: { mode: "auto" },
        profile: "motion",
      },
    });
    await sender.reported;
    await write({ ...(await read()), maxResolution: 720 });
    expect(await sender.result).toMatchObject({
      serverEndedVideo: true,
      captureStopped: true,
      roomConnectedAfterStop: true,
    });
  } finally {
    sender?.child.kill();
    await restore();
  }
});
