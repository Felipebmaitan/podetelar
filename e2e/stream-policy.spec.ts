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
};

test.skip(
  process.env.PODETELAR_TEST_STREAM_POLICY !== "1",
  "Requires an isolated policy-enabled API and SFU.",
);
test.use({ bypassCSP: true });

test("dashboard saves caps and the SFU enforces them for a publisher that ignores policy", async ({
  page,
  context,
  request,
  baseURL,
}) => {
  test.setTimeout(90_000);
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
    await page.getByLabel("720p at 30 FPS, Mbps").fill("4");
    await page.getByLabel("1080p at 60 FPS, Mbps").fill("6");
    await page.getByRole("button", { name: "Save stream policy" }).click();
    await expect(page.getByText("Stream policy saved.")).toBeVisible();
    expect(await read()).toEqual({
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
          test.published = test.removed = 0;
          room.on(sdk.RoomEvent.TrackPublished, (publication) => {
            ++test.published;
            publication.setSubscribed(true);
          });
          room.on(sdk.RoomEvent.TrackSubscribed, (track) => {
            if (track.kind !== sdk.Track.Kind.Video) return;
            const video = track.attach();
            video.autoplay = video.muted = true;
            document.body.append(video);
          });
          room.on(sdk.RoomEvent.TrackUnpublished, () => ++test.removed);
          await room.connect(serverUrl, token, { autoSubscribe: false });
        },
        await media.json(),
      );
    }
    await publish(source, 640, 360);
    await expect
      .poll(
        () =>
          viewer
            .locator("video")
            .evaluate((video: HTMLVideoElement) => video.videoWidth),
        { timeout: 15_000 },
      )
      .toBe(640);
    await page.getByLabel("Maximum frame rate").selectOption("15");
    await page.getByRole("button", { name: "Save stream policy" }).click();
    await expect(page.getByText("Stream policy saved.")).toBeVisible();
    await expect
      .poll(
        () => viewer.evaluate(() => (window as unknown as TestWindow).removed),
        { timeout: 22_000 },
      )
      .toBe(1);
    await viewer
      .locator("video")
      .evaluateAll((videos) => videos.forEach((video) => video.remove()));
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
      await viewer
        .locator("video")
        .evaluateAll((videos: HTMLVideoElement[]) =>
          videos.some((video) => video.videoWidth > 0),
        ),
    ).toBe(false);
  } finally {
    await Promise.all([
      source.close(),
      viewer.close(),
      sender.dispose(),
      receiver.dispose(),
    ]);
    const restore = await page.request.put(endpoint, {
      headers,
      data: { ...initial, revision: (await read()).revision },
    });
    expect(restore.status()).toBe(200);
  }
});

async function publish(page: Page, width: number, height: number) {
  await page.evaluate(
    async ({ width, height }) => {
      const test = window as unknown as TestWindow;
      if (test.policyTrack) {
        await test.policyRoom.localParticipant.unpublishTrack(test.policyTrack);
        clearInterval(test.paintTimer);
      }
      const canvas = document.createElement("canvas");
      canvas.width = width;
      canvas.height = height;
      const drawing = canvas.getContext("2d")!;
      let frame = 0;
      test.paintTimer = window.setInterval(() => {
        drawing.fillStyle = `hsl(${(frame++ * 11) % 360}, 60%, 40%)`;
        drawing.fillRect(0, 0, width, height);
      }, 32);
      const sdk = test.LivekitClient;
      const track = new sdk.LocalVideoTrack(
        canvas.captureStream(30).getVideoTracks()[0]!,
      );
      test.policyTrack = track;
      await test.policyRoom.localParticipant.publishTrack(track, {
        source: sdk.Track.Source.ScreenShare,
        videoCodec: "h264",
        simulcast: false,
        videoEncoding: { maxBitrate: 8_000_000, maxFramerate: 30 },
      });
    },
    { width, height },
  );
}
