import { expect, request as playwrightRequest, test } from "@playwright/test";
import type { LocalVideoTrack, Room } from "livekit-client";
import type { StreamPolicy } from "@podetelar/contracts";
import type { Page } from "@playwright/test";
import { resolve } from "node:path";
import { createTestRoom } from "../scripts/test-room.js";

type TestWindow = Window & {
  LivekitClient: typeof import("livekit-client");
  policyRoom: Room;
  policyTrack?: LocalVideoTrack;
  paintTimer?: number;
  published: number;
  removed: number;
  localRemoved: number;
  decodedOversize: boolean;
};

test.skip(
  process.env.PODETELAR_TEST_STREAM_POLICY !== "1",
  "Requires an isolated policy-enabled API and SFU.",
);
test.use({ bypassCSP: true });

test("camera caps and Automatic targets survive dashboard reload independently of screen targets", async ({
  page,
  baseURL,
}) => {
  const origin = new URL(baseURL!).origin;
  expect(new URL(origin).hostname).toMatch(
    /^(localhost|127\.0\.0\.1|\[::1\])$/,
  );
  await page.goto("/");
  await page
    .getByLabel("Password", { exact: true })
    .fill(process.env.ADMIN_TOKEN!);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  const open = () =>
    page.getByRole("button", { name: "Stream quality", exact: true }).click();
  await open();
  const endpoint = "/api/admin/stream-policy";
  const read = async () =>
    (await (await page.request.get(endpoint)).json()).policy as StreamPolicy;
  const initial = await read();
  try {
    await page.getByLabel("Maximum video bitrate (Mbps)").fill("10");
    await page.getByLabel("Maximum camera video bitrate (Mbps)").fill("0.75");
    await page.getByLabel("720p at 30 FPS, Mbps", { exact: true }).fill("4");
    await page
      .getByLabel("Camera 720p at 30 FPS, Mbps", { exact: true })
      .fill("0.5");
    await page.getByRole("button", { name: "Save stream policy" }).click();
    await expect(page.getByText("Stream policy saved.")).toBeVisible();
    await page.reload();
    await open();
    await expect(page.getByLabel("Maximum video bitrate (Mbps)")).toHaveValue(
      "10",
    );
    await expect(
      page.getByLabel("Maximum camera video bitrate (Mbps)"),
    ).toHaveValue("0.75");
    await expect(
      page.getByLabel("720p at 30 FPS, Mbps", { exact: true }),
    ).toHaveValue("4");
    await expect(
      page.getByLabel("Camera 720p at 30 FPS, Mbps", { exact: true }),
    ).toHaveValue("0.5");
    // Simulate the dashboard status during an SFU capability loss.
    await page.route(`**${endpoint}`, async (route) => {
      if (route.request().method() !== "GET") return route.continue();
      const response = await route.fetch();
      await route.fulfill({
        response,
        json: { ...(await response.json()), cameraEnforcement: false },
      });
    });
    await page.reload();
    await open();
    await expect(
      page.getByText(/Camera enforcement is unavailable/),
    ).toBeVisible();
    await page.getByLabel("Maximum camera video bitrate (Mbps)").fill("");
    await page.getByRole("button", { name: "Save stream policy" }).click();
    await expect(page.getByText("Stream policy saved.")).toBeVisible();
    expect(await read()).toMatchObject({
      maxBitrateBps: 10_000_000,
      maxCameraBitrateBps: null,
      cameraAutomaticTargets: [
        { resolution: 720, fps: 30, bitrateBps: 500_000 },
      ],
    });
  } finally {
    const restore = await page.request.put(endpoint, {
      headers: { Origin: origin },
      data: {
        maxCameraBitrateBps: null,
        cameraAutomaticTargets: [],
        ...initial,
        revision: (await read()).revision,
      },
    });
    expect(restore.status()).toBe(200);
  }
});

test("dashboard saves caps and the SFU enforces them for a publisher that ignores policy", async ({
  page,
  context,
  request,
  baseURL,
}) => {
  test.setTimeout(120_000);
  const origin = new URL(baseURL!).origin;
  expect(new URL(origin).hostname).toMatch(
    /^(localhost|127\.0\.0\.1|\[::1\])$/,
  );
  const headers = { Origin: origin };
  await page.goto("/");
  await page
    .getByLabel("Password", { exact: true })
    .fill(process.env.ADMIN_TOKEN!);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await page
    .getByRole("button", { name: "Stream quality", exact: true })
    .click();
  const endpoint = "/api/admin/stream-policy";
  const read = async () =>
    (await (await page.request.get(endpoint)).json()).policy as StreamPolicy;
  const initial = await read();
  const sender = await playwrightRequest.newContext({
    baseURL: origin,
    extraHTTPHeaders: headers,
  });
  const receiver = await playwrightRequest.newContext({
    baseURL: origin,
    extraHTTPHeaders: headers,
  });
  const source = await context.newPage();
  const viewer = await context.newPage();
  try {
    await page.getByLabel("Maximum video bitrate (Mbps)").fill("10");
    await page.getByLabel("Maximum resolution").selectOption("1080");
    await page.getByLabel("Maximum frame rate").selectOption("60");
    await page.getByLabel("720p at 30 FPS, Mbps", { exact: true }).fill("4");
    await page.getByLabel("1080p at 60 FPS, Mbps", { exact: true }).fill("6");
    await page.getByRole("button", { name: "Save stream policy" }).click();
    await expect(page.getByText("Stream policy saved.")).toBeVisible();
    expect(await read()).toMatchObject({
      revision: initial.revision + 1,
      maxBitrateBps: 10_000_000,
      maxResolution: 1080,
      maxFps: 60,
      automaticTargets: [
        { resolution: 720, fps: 30, bitrateBps: 4_000_000 },
        { resolution: 1080, fps: 60, bitrateBps: 6_000_000 },
      ],
    });
    const created = await createTestRoom(
      request,
      origin,
      "Policy enforcement acceptance",
    );
    expect(created.status()).toBe(201);
    const { room, invitationUrl } = await created.json();
    const invitation = new URL(invitationUrl).hash.slice("#invite=".length);
    for (const [api, mediaPage, displayName] of [
      [sender, source, "Policy-ignorant publisher"],
      [receiver, viewer, "Policy viewer"],
    ] as const) {
      expect(
        (
          await api.post(`/api/rooms/${room.id}/join`, {
            data: { invitation, displayName },
          })
        ).status(),
      ).toBe(200);
      const media = await api.post(`/api/rooms/${room.id}/native-media-token`);
      expect(media.status()).toBe(200);
      await mediaPage.goto("/api/health");
      await mediaPage.addScriptTag({
        path: resolve("node_modules/livekit-client/dist/livekit-client.umd.js"),
      });
      await mediaPage.evaluate(
        async ({ serverUrl, token }) => {
          const test = window as unknown as TestWindow;
          const sdk = test.LivekitClient;
          const room = new sdk.Room({ dynacast: false, adaptiveStream: false });
          test.policyRoom = room;
          test.published = test.removed = test.localRemoved = 0;
          test.decodedOversize = false;
          room.on(sdk.RoomEvent.TrackPublished, (publication) => {
            ++test.published;
            publication.setSubscribed(true);
          });
          room.on(sdk.RoomEvent.TrackSubscribed, (track) => {
            if (track.kind !== sdk.Track.Kind.Video) return;
            const video = document.createElement("video");
            track.attach(video);
            video.autoplay = video.muted = true;
            video.addEventListener("resize", () => {
              test.decodedOversize ||=
                video.videoWidth > 1920 || video.videoHeight > 1080;
            });
            document.body.append(video);
          });
          room.on(sdk.RoomEvent.TrackUnpublished, () => ++test.removed);
          room.on(
            sdk.RoomEvent.LocalTrackUnpublished,
            () => ++test.localRemoved,
          );
          await room.connect(serverUrl, token, { autoSubscribe: false });
        },
        await media.json(),
      );
    }
    await publish(source, 640, 360, true);
    await expect
      .poll(
        () =>
          viewer
            .locator("video")
            .evaluate((video: HTMLVideoElement) => video.videoWidth),
        { timeout: 15_000 },
      )
      .toBeGreaterThan(0);
    await expect
      .poll(() => outgoingRate(source, "framesEncoded"), { timeout: 10_000 })
      .toBeGreaterThan(20);
    await page.getByLabel("Maximum frame rate").selectOption("15");
    await page.getByRole("button", { name: "Save stream policy" }).click();
    await expect(page.getByText("Stream policy saved.")).toBeVisible();
    await expect.poll(async () => (await read()).maxFps).toBe(15);
    await source.bringToFront();
    await expect
      .poll(
        () => viewer.evaluate(() => (window as unknown as TestWindow).removed),
        { timeout: 22_000 },
      )
      .toBe(1);
    await expect
      .poll(() =>
        source.evaluate(() => (window as unknown as TestWindow).localRemoved),
      )
      .toBe(1);
    await viewer
      .locator("video")
      .evaluateAll((videos) => videos.forEach((video) => video.remove()));

    await page.getByLabel("Maximum frame rate").selectOption("60");
    await page.getByRole("button", { name: "Save stream policy" }).click();
    await expect(page.getByText("Stream policy saved.")).toBeVisible();
    await expect.poll(async () => (await read()).maxFps).toBe(60);
    await publish(source, 2560, 1440);
    await expect
      .poll(() =>
        viewer.evaluate(() => (window as unknown as TestWindow).published),
      )
      .toBe(2);
    await expect
      .poll(
        () => viewer.evaluate(() => (window as unknown as TestWindow).removed),
        { timeout: 16_000 },
      )
      .toBe(2);
    expect(
      await viewer.evaluate(
        () => (window as unknown as TestWindow).decodedOversize,
      ),
    ).toBe(false);

    await publish(source, 1920, 1080, true);
    await expect
      .poll(async () => 8 * (await outgoingRate(source, "bytesSent")), {
        timeout: 15_000,
      })
      .toBeGreaterThan(1_300_000);
    await page.getByLabel("Maximum video bitrate (Mbps)").fill("1");
    await page.getByRole("button", { name: "Save stream policy" }).click();
    await expect(page.getByText("Stream policy saved.")).toBeVisible();
    await expect
      .poll(
        () => viewer.evaluate(() => (window as unknown as TestWindow).removed),
        { timeout: 30_000 },
      )
      .toBe(3);

    await page.getByLabel("Maximum video bitrate (Mbps)").fill("10");
    await page.getByRole("button", { name: "Save stream policy" }).click();
    await expect
      .poll(async () => (await read()).maxBitrateBps)
      .toBe(10_000_000);
    await publish(source, 1280, 720, true, "camera");
    await expect
      .poll(async () => 8 * (await outgoingRate(source, "bytesSent")), {
        timeout: 15_000,
      })
      .toBeGreaterThan(1_000_000);
    await page.getByLabel("Maximum camera video bitrate (Mbps)").fill("0.5");
    await page.getByRole("button", { name: "Save stream policy" }).click();
    await expect
      .poll(async () => (await read()).maxCameraBitrateBps)
      .toBe(500_000);
    await expect
      .poll(
        () => viewer.evaluate(() => (window as unknown as TestWindow).removed),
        { timeout: 30_000 },
      )
      .toBe(4);
  } finally {
    await Promise.all([
      source.close(),
      viewer.close(),
      sender.dispose(),
      receiver.dispose(),
    ]);
    const restore = await page.request.put(endpoint, {
      headers,
      data: {
        maxCameraBitrateBps: null,
        cameraAutomaticTargets: [],
        ...initial,
        revision: (await read()).revision,
      },
    });
    expect(restore.status()).toBe(200);
  }
});

async function outgoingRate(
  page: Page,
  counter: "bytesSent" | "framesEncoded",
) {
  return page.evaluate(async (counter) => {
    const test = window as unknown as TestWindow;
    const sample = async () => {
      let outgoing = { timestamp: 0, value: 0 };
      (await test.policyTrack?.getRTCStatsReport())?.forEach((stat) => {
        if (stat.type === "outbound-rtp" && stat.kind === "video") {
          outgoing = {
            timestamp: stat.timestamp as number,
            value: stat[counter] as number,
          };
        }
      });
      return outgoing;
    };
    const before = await sample();
    await new Promise((done) => setTimeout(done, 1_000));
    const after = await sample();
    return after.timestamp > before.timestamp
      ? (1_000 * (after.value - before.value)) /
          (after.timestamp - before.timestamp)
      : 0;
  }, counter);
}

async function publish(
  page: Page,
  width: number,
  height: number,
  detailed = false,
  kind: "screen" | "camera" = "screen",
) {
  await page.bringToFront();
  await page.evaluate(
    async ({ width, height, detailed, kind }) => {
      const test = window as unknown as TestWindow;
      if (test.policyTrack) {
        await test.policyRoom.localParticipant.unpublishTrack(test.policyTrack);
        clearInterval(test.paintTimer);
      }
      const canvas = document.createElement("canvas");
      canvas.width = width;
      canvas.height = height;
      const drawing = canvas.getContext("2d")!;
      const noise = document.createElement("canvas");
      noise.width = 120;
      noise.height = 68;
      const pixels = noise.getContext("2d")!;
      const image = pixels.createImageData(noise.width, noise.height);
      drawing.imageSmoothingEnabled = false;
      let frame = 0;
      test.paintTimer = window.setInterval(() => {
        if (detailed) {
          crypto.getRandomValues(image.data);
          for (let alpha = 3; alpha < image.data.length; alpha += 4)
            image.data[alpha] = 255;
          pixels.putImageData(image, 0, 0);
          drawing.drawImage(noise, 0, 0, width, height);
        } else {
          drawing.fillStyle = `hsl(${(frame++ * 11) % 360}, 60%, 40%)`;
          drawing.fillRect(0, 0, width, height);
        }
      }, 32);
      const sdk = test.LivekitClient;
      const sourceTrack = canvas.captureStream(30).getVideoTracks()[0]!;
      sourceTrack.contentHint = detailed ? "motion" : "detail";
      const track = new sdk.LocalVideoTrack(sourceTrack);
      test.policyTrack = track;
      await test.policyRoom.localParticipant.publishTrack(track, {
        source:
          kind === "camera"
            ? sdk.Track.Source.Camera
            : sdk.Track.Source.ScreenShare,
        videoCodec: "h264",
        simulcast: false,
        degradationPreference: detailed
          ? "maintain-framerate"
          : "maintain-resolution",
        screenShareEncoding: { maxBitrate: 8_000_000, maxFramerate: 30 },
        videoEncoding: { maxBitrate: 8_000_000, maxFramerate: 30 },
      });
    },
    { width, height, detailed, kind },
  );
}
