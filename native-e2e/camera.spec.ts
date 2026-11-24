import { expect, test } from "@playwright/test";
import { RoomServiceClient, TrackSource, TrackType } from "livekit-server-sdk";
import { createTestRoom } from "../scripts/test-room.js";
import { nativeRequest } from "./fixtures.js";

type Sent = {
  captureStopped: boolean;
  mode: { width: number; height: number; fps: number; bitrate: number };
  metrics: { fps: number; requested_bitrate: number; encoder_bitrate: number };
  previewCaptures: number;
  previewUploads: number;
};
type Received = {
  previewBeforeWatching: boolean;
  samples: {
    phaseSecond: number | null;
    receivedBytes: number;
    streams: { frames: number; pictureSize: number[] | null }[];
  }[];
};

test("an explicitly selected Windows camera publishes video, previews and releases capture", async ({
  request,
  baseURL,
}) => {
  const name = process.env.PODETELAR_TEST_CAMERA;
  test.skip(
    !name || !process.env.PODETELAR_TEST_SFU_PID,
    "Choose a camera for the isolated Windows SFU run.",
  );
  test.setTimeout(90_000);
  const devices = (await nativeRequest("--cameras", {}).result) as {
    id: string;
    name: string;
  }[];
  const selected = devices.filter((camera) => camera.name === name);
  expect(selected).toHaveLength(1);
  const created = await createTestRoom(request, baseURL!, "Native camera");
  expect(created.status()).toBe(201);
  const { room, invitationUrl } = await created.json();
  const media = new RoomServiceClient(
    `http://127.0.0.1:${process.env.PODETELAR_SMOKE_SFU_PORT ?? 17880}`,
    process.env.LIVEKIT_API_KEY!,
    process.env.LIVEKIT_API_SECRET!,
  );
  const configuration = {
    invitationUrl,
    displayName: "Camera publisher",
    cameraId: selected[0]!.id,
    codec: "h264",
    quality: {
      resolution: "p720",
      fps: 24,
      profile: "motion",
      bitrate: { mode: "manual", bps: 500_000 },
    },
  };
  const sender = nativeRequest("--publish-fixture", {
    ...configuration,
    seconds: 23,
    lingerSeconds: 8,
  });
  let viewer: ReturnType<typeof nativeRequest> | undefined;
  let restarted: ReturnType<typeof nativeRequest> | undefined;
  const published = async () => {
    const participants = await media.listParticipants(`podetelar_${room.id}`);
    const presenter = participants.find(
      (p) => p.name === configuration.displayName,
    );
    return presenter?.tracks.map((track) => [track.type, track.source]);
  };
  try {
    await sender.reported;
    await expect
      .poll(published)
      .toEqual([[TrackType.VIDEO, TrackSource.CAMERA]]);
    viewer = nativeRequest("--watch-fixture", {
      invitationUrl,
      seconds: 20,
      waitForPreview: true,
    });
    const watched = (await viewer.result) as Received;
    expect(watched.previewBeforeWatching).toBe(true);
    const decoded = (minimum: number, maximum: number) =>
      watched.samples.some(
        (sample) =>
          sample.phaseSecond !== null &&
          sample.phaseSecond >= minimum &&
          sample.phaseSecond <= maximum &&
          sample.streams.some(
            (stream) =>
              stream.frames > 5 && stream.pictureSize?.join("x") === "1280x720",
          ),
      );
    expect(decoded(0, 4)).toBe(true);
    expect(decoded(10, 19)).toBe(true);
    const before = watched.samples.find((sample) => sample.phaseSecond === 6)!;
    const after = watched.samples.find((sample) => sample.phaseSecond === 7)!;
    expect(before.receivedBytes).toBeGreaterThan(0);
    expect(after.receivedBytes).toBe(before.receivedBytes);
    await expect.poll(published, { timeout: 7500 }).toEqual([]);
    expect(sender.child.exitCode).toBeNull();
    restarted = nativeRequest("--publish-fixture", {
      ...configuration,
      seconds: 3,
      quality: {
        ...configuration.quality,
        fps: 30,
        bitrate: { mode: "manual", bps: 2_000_000 },
      },
    });
    await restarted.reported;
    expect(sender.child.exitCode).toBeNull();
    const [sent, defaults] = (await Promise.all([
      sender.result,
      restarted.result,
    ])) as [Sent, Sent];
    expect(sent.captureStopped).toBe(true);
    expect(sent.mode).toEqual({
      width: 1280,
      height: 720,
      fps: 24,
      bitrate: 500_000,
    });
    expect(sent.metrics.fps).toBeGreaterThan(20);
    expect(sent.metrics.fps).toBeLessThan(25);
    expect(sent.metrics.requested_bitrate).toBe(500_000);
    expect(sent.metrics.encoder_bitrate).toBeGreaterThan(0);
    expect(sent.metrics.encoder_bitrate).toBeLessThanOrEqual(500_000);
    expect(sent.previewCaptures).toBe(1);
    expect(sent.previewUploads).toBe(1);
    expect(defaults.captureStopped).toBe(true);
    expect(defaults.mode).toEqual({
      width: 1280,
      height: 720,
      fps: 30,
      bitrate: 2_000_000,
    });
    expect(defaults.metrics.requested_bitrate).toBe(2_000_000);
    expect(defaults.metrics.encoder_bitrate).toBeGreaterThan(0);
    expect(defaults.metrics.encoder_bitrate).toBeLessThanOrEqual(2_000_000);
  } finally {
    sender.child.kill();
    viewer?.child.kill();
    restarted?.child.kill();
  }
});
