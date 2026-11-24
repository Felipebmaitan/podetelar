import { spawn } from "node:child_process";
import { expect, test } from "@playwright/test";
import { RoomServiceClient, TrackSource, TrackType } from "livekit-server-sdk";
import { createTestRoom } from "../scripts/test-room.js";
import {
  nativeExecutable,
  nativeRequest,
  policyAdministrator,
} from "./fixtures.js";

type Sent = {
  captureStopped: boolean;
  mode: { width: number; height: number; fps: number; bitrate: number };
  metrics: { fps: number; requested_bitrate: number; encoder_bitrate: number };
  previewCaptures: number;
  previewUploads: number;
};
type Received = {
  samples: {
    second: number;
    streams: {
      kind: "screen" | "camera";
      codec: string;
      frames: number;
      watching: boolean;
      pictureSize: number[] | null;
    }[];
  }[];
};

async function cameraId(
  isolated = Boolean(process.env.PODETELAR_TEST_SFU_PID),
) {
  const name = process.env.PODETELAR_TEST_CAMERA;
  test.skip(
    !name || !isolated,
    "Choose a camera for the isolated Windows SFU run.",
  );
  const devices = (await nativeRequest("--cameras", {}).result) as {
    id: string;
    name: string;
  }[];
  const selected = devices.filter((camera) => camera.name === name);
  expect(selected).toHaveLength(1);
  return selected[0]!.id;
}

test("an explicitly selected Windows camera publishes and starts playing automatically", async ({
  request,
  baseURL,
}) => {
  test.setTimeout(90_000);
  const id = await cameraId();
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
    cameraId: id,
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
      previewOnly: true,
    });
    const watched = (await viewer.result) as Received;
    expect(
      watched.samples.some(
        (sample) =>
          sample.second < 6 &&
          sample.streams.some(
            (stream) =>
              stream.kind === "camera" &&
              stream.watching &&
              stream.frames > 5 &&
              stream.pictureSize?.join("x") === "1280x720",
          ),
      ),
    ).toBe(true);
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
    // Exposure and driver timing can drop camera input frames. Verify an
    // active stream and enforce the selected ceiling without duplicating frames.
    expect(sent.metrics.fps).toBeGreaterThan(10);
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

test("one participant shares a window and a camera concurrently with independent playback and stop", async ({
  request,
  baseURL,
}) => {
  test.setTimeout(115_000);
  const id = await cameraId();
  const cameraCodec = process.env.PODETELAR_TEST_CAMERA_CODEC;
  const screen = timingSource();
  let viewer: ReturnType<typeof nativeRequest> | undefined;
  let sender: ReturnType<typeof nativeRequest> | undefined;
  try {
    const sourceId = await timingWindow(screen.pid);
    const created = await createTestRoom(
      request,
      baseURL!,
      "Concurrent camera and screen",
    );
    expect(created.status()).toBe(201);
    const { room, invitationUrl } = await created.json();
    const media = new RoomServiceClient(
      `http://127.0.0.1:${process.env.PODETELAR_SMOKE_SFU_PORT ?? 17880}`,
      process.env.LIVEKIT_API_KEY!,
      process.env.LIVEKIT_API_SECRET!,
    );
    const participants = () => media.listParticipants(`podetelar_${room.id}`);
    const name = "Dual publisher";
    viewer = nativeRequest("--watch-fixture", {
      invitationUrl,
      seconds: 37,
      watchSchedule: [
        { displayName: name, second: 0, enabled: false },
        { displayName: name, second: 10, enabled: true },
        { displayName: name, second: 14, enabled: false },
        { displayName: name, second: 17, enabled: true },
      ],
    });
    await expect
      .poll(
        async () =>
          (await participants()).some(
            (participant) => participant.name === "Native playback check",
          ),
        { timeout: 10_000 },
      )
      .toBe(true);
    sender = nativeRequest("--qualify-dual-sharing", {
      invitationUrl,
      sourceId,
      cameraId: id,
      cameraCodec,
    });
    expect(await sender.reported).toEqual({ both: true });
    const tracks = async () =>
      (await participants()).find((p) => p.name === name)?.tracks ?? [];
    const sid = async (source: TrackSource) =>
      (await tracks()).find((t) => t.source === source)?.sid;
    const sources = async () => (await tracks()).map((t) => t.source).sort();
    const both = [
      TrackSource.SCREEN_SHARE,
      TrackSource.SCREEN_SHARE_AUDIO,
      TrackSource.CAMERA,
    ].sort();
    await expect.poll(sources).toEqual(both);
    const screenSid = await sid(TrackSource.SCREEN_SHARE);
    const cameraSid = await sid(TrackSource.CAMERA);
    expect(screenSid).toBeTruthy();
    expect(cameraSid).toBeTruthy();
    expect(await sid(TrackSource.SCREEN_SHARE_AUDIO)).toBeTruthy();
    await expect
      .poll(sources, { timeout: 22_000 })
      .toEqual(
        [TrackSource.SCREEN_SHARE, TrackSource.SCREEN_SHARE_AUDIO].sort(),
      );
    expect(await sid(TrackSource.SCREEN_SHARE)).toBe(screenSid);
    await expect.poll(sources, { timeout: 10_000 }).toEqual(both);
    const nextCameraSid = await sid(TrackSource.CAMERA);
    expect(nextCameraSid).not.toBe(cameraSid);
    await expect
      .poll(sources, { timeout: 10_000 })
      .toEqual([TrackSource.CAMERA]);
    expect(await sid(TrackSource.CAMERA)).toBe(nextCameraSid);
    expect(await sender.result).toEqual({ closed: true });
    const watched = (await viewer.result) as Received;
    if (cameraCodec) {
      expect(
        watched.samples.some((sample) =>
          sample.streams.some(
            (stream) =>
              stream.kind === "camera" &&
              stream.frames > 5 &&
              stream.codec.toLowerCase() === `video/${cameraCodec}`,
          ),
        ),
      ).toBe(true);
    }
    const frameCount = (
      sample: Received["samples"][number],
      kind: "screen" | "camera",
    ) =>
      sample.streams.find((stream) => stream.kind === kind && stream.watching)
        ?.frames ?? 0;
    expect(
      watched.samples.some(
        (sample) =>
          sample.second < 10 &&
          frameCount(sample, "camera") > 5 &&
          frameCount(sample, "screen") === 0,
      ),
    ).toBe(true);
    expect(
      watched.samples.some(
        (sample) =>
          sample.second >= 11 &&
          sample.second < 14 &&
          frameCount(sample, "camera") > 5 &&
          frameCount(sample, "screen") > 5,
      ),
    ).toBe(true);
    expect(
      watched.samples.some(
        (sample) =>
          sample.second >= 15 &&
          sample.second < 17 &&
          frameCount(sample, "camera") > 5 &&
          frameCount(sample, "screen") === 0,
      ),
    ).toBe(true);
  } finally {
    sender?.child.kill();
    viewer?.child.kill();
    screen.kill();
  }
});

function timingSource() {
  return spawn(
    process.env.PODETELAR_NATIVE_TONE_EXE ?? nativeExecutable(),
    ["--av-timing-source", "90"],
    {
      windowsHide: false,
      stdio: "ignore",
    },
  );
}

async function timingWindow(pid: number | undefined) {
  expect(pid).toBeDefined();
  let sourceId: string | undefined;
  await expect
    .poll(
      async () => {
        const sources = (await nativeRequest("--sources", {}).result) as {
          id: string;
        }[];
        sourceId = sources.find(
          (source) =>
            source.id.startsWith("window:") && source.id.endsWith(`:${pid}`),
        )?.id;
        return Boolean(sourceId);
      },
      { timeout: 10_000 },
    )
    .toBe(true);
  return sourceId!;
}

test("live camera Automatic target and sub-1 Mbps cap preserve the concurrent screen", async ({
  request,
  baseURL,
}) => {
  test.setTimeout(85_000);
  const id = await cameraId(process.env.PODETELAR_TEST_STREAM_POLICY === "1");
  const admin = await policyAdministrator(request, baseURL!);
  const screen = timingSource();
  let sender: ReturnType<typeof nativeRequest> | undefined;
  let viewer: ReturnType<typeof nativeRequest> | undefined;
  try {
    const sourceId = await timingWindow(screen.pid);
    const created = await createTestRoom(
      request,
      baseURL!,
      "Camera bitrate policy",
    );
    expect(created.status()).toBe(201);
    const { room, invitationUrl } = await created.json();
    await admin.write({
      ...(await admin.read()),
      maxBitrateBps: 6_000_000,
      maxCameraBitrateBps: null,
      cameraAutomaticTargets: [],
    });
    sender = nativeRequest("--qualify-dual-sharing", {
      invitationUrl,
      sourceId,
      cameraId: id,
      cameraCodec: "h264",
      cameraPolicyBitrates: [800_000, 500_000],
    });
    expect(await sender.reported).toEqual({ both: true });
    const client = new RoomServiceClient(
      process.env.LIVEKIT_INTERNAL_URL!,
      process.env.LIVEKIT_API_KEY!,
      process.env.LIVEKIT_API_SECRET!,
    );
    const sid = async (source: TrackSource) =>
      (await client.listParticipants(`podetelar_${room.id}`))
        .flatMap((participant) => participant.tracks)
        .find((track) => track.source === source)?.sid;
    const cameraSid = await sid(TrackSource.CAMERA);
    const screenSid = await sid(TrackSource.SCREEN_SHARE);
    expect(cameraSid).toBeTruthy();
    expect(screenSid).toBeTruthy();
    viewer = nativeRequest("--watch-fixture", {
      invitationUrl,
      seconds: 19,
      continuousWatch: true,
      watchOncePerPublisher: true,
    });
    let observations = "";
    sender.child.stdout?.on("data", (chunk: Buffer) => {
      observations = (observations + chunk.toString()).slice(-4096);
    });
    await new Promise((resolve) => setTimeout(resolve, 5_000));
    await admin.write({
      ...(await admin.read()),
      cameraAutomaticTargets: [
        { resolution: 720, fps: 30, bitrateBps: 800_000 },
      ],
    });
    await expect
      .poll(() => observations, { timeout: 8_000 })
      .toContain('"cameraBitrate":800000');
    await admin.write({
      ...(await admin.read()),
      maxCameraBitrateBps: 500_000,
    });
    await expect
      .poll(() => observations, { timeout: 8_000 })
      .toContain('"cameraBitrate":500000');
    expect(await sid(TrackSource.CAMERA)).toBe(cameraSid);
    expect(await sid(TrackSource.SCREEN_SHARE)).toBe(screenSid);
    const [sent, watched] = await Promise.all([sender.result, viewer.result]);
    expect(sent).toMatchObject({
      cameraMode: { bitrate: 500_000 },
      screenMode: { fps: 30 },
    });
    expect(
      (watched as Received).samples.filter(({ streams }) =>
        ["camera", "screen"].every((kind) =>
          streams.some(
            (stream) =>
              stream.kind === kind && stream.watching && stream.frames > 5,
          ),
        ),
      ).length,
    ).toBeGreaterThan(2);
  } finally {
    sender?.child.kill();
    viewer?.child.kill();
    screen.kill();
    await admin.restore();
  }
});

test("a live SFU policy update applies to both concurrent video publications", async ({
  request,
  baseURL,
}) => {
  test.setTimeout(115_000);
  const policyEnabled = process.env.PODETELAR_TEST_STREAM_POLICY === "1";
  const id = await cameraId(policyEnabled);
  const admin = await policyAdministrator(request, baseURL!);
  const screen = timingSource();
  let viewer: ReturnType<typeof nativeRequest> | undefined;
  let sender: ReturnType<typeof nativeRequest> | undefined;
  try {
    const sourceId = await timingWindow(screen.pid);
    const created = await createTestRoom(
      request,
      baseURL!,
      "Concurrent camera policy",
    );
    expect(created.status()).toBe(201);
    const { invitationUrl } = await created.json();
    sender = nativeRequest("--qualify-dual-sharing", {
      invitationUrl,
      sourceId,
      cameraId: id,
      checkLivePolicy: true,
    });
    expect(await sender.reported).toEqual({ both: true });
    viewer = nativeRequest("--watch-fixture", {
      invitationUrl,
      seconds: 25,
      continuousWatch: true,
      watchOncePerPublisher: true,
    });
    // Allow initial frames to arrive before measuring playback across the cap change.
    await new Promise((resolve) => setTimeout(resolve, 7_000));
    await admin.write({
      ...(await admin.read()),
      maxBitrateBps: 1_000_000,
      maxResolution: 720,
      maxFps: 24,
    });
    const [sent, watched] = await Promise.all([sender.result, viewer.result]);
    expect(sent).toMatchObject({
      screenMode: { fps: 24, bitrate: 1_000_000 },
      cameraMode: { width: 1280, height: 720, fps: 24, bitrate: 1_000_000 },
    });
    const samples = (watched as Received).samples;
    const played = (height: number) =>
      samples.find(
        ({ streams }) =>
          streams.some(
            (stream) =>
              stream.kind === "camera" &&
              stream.watching &&
              stream.frames > 5 &&
              stream.pictureSize?.[1] === height,
          ) &&
          streams.some(
            (stream) =>
              stream.kind === "screen" &&
              stream.watching &&
              stream.frames > 5 &&
              stream.pictureSize !== null,
          ),
      );
    const before = played(1080);
    const after = played(720);
    expect(before).toBeDefined();
    expect(after).toBeDefined();
    expect(after!.second).toBeGreaterThan(before!.second);
  } finally {
    sender?.child.kill();
    viewer?.child.kill();
    screen.kill();
    await admin.restore();
  }
});
