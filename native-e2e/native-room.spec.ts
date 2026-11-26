import { rmSync, writeFileSync } from "node:fs";
import { setTimeout as delay } from "node:timers/promises";
import { expect, test } from "@playwright/test";
import { createTestRoom } from "../scripts/test-room.js";
import {
  backgroundAudioExclusions,
  nativeRequest,
  toneFixtures,
} from "./fixtures.js";

type Stream = {
  codec: string;
  frames: number;
  watching: boolean;
  pictureSize: number[] | null;
  audio: { frames: number; tones: number[]; channel_difference_peak: number };
};
type Received = {
  closed: boolean;
  startedAtMs: number;
  previewBeforeWatching: boolean;
  samples: {
    second: number;
    phaseSecond: number | null;
    recoveries: number;
    connected: boolean;
    watching: boolean;
    receivedBytes: number;
    mediaTiming: { kind: string; bytes: number }[];
    streams: Stream[];
  }[];
};
type Sent = {
  codec: string;
  initialSid: string;
  finalSid: string;
  captureStopped: boolean;
  mode: { width: number; height: number; fps: number };
  previewUploads: number;
  previewCaptures: number;
};
const quality = {
  resolution: "p1080",
  fps: 60,
  profile: "motion",
  bitrate: { mode: "auto" },
};

for (const codec of ["h265", "h264"] as const) {
  test(`native ${codec} decode, previews, explicit unsubscribe and rewatch`, async ({
    request,
    baseURL,
  }, testInfo) => {
    test.setTimeout(90_000);
    const created = await createTestRoom(request, baseURL!, `Native ${codec}`);
    expect(created.status()).toBe(201);
    const { invitationUrl } = await created.json();
    const sender = nativeRequest("--publish-fixture", {
      invitationUrl,
      displayName: "Publisher",
      codec,
      seconds: 26,
      quality,
    });
    await sender.reported;
    const receiver = nativeRequest("--watch-fixture", {
      invitationUrl,
      seconds: 24,
      waitForPreview: true,
    });
    try {
      const [sent, watched] = await Promise.all([
        sender.result as Promise<Sent>,
        receiver.result as Promise<Received>,
      ]);
      writeFileSync(
        testInfo.outputPath("native-report.json"),
        JSON.stringify({ sent, watched }),
      );
      await testInfo.attach("native-aggregate-report", {
        body: JSON.stringify({ sent, watched }),
        contentType: "application/json",
      });
      expect(sent.captureStopped).toBe(true);
      expect(sent.previewCaptures).toBe(1);
      expect(watched.closed).toBe(true);
      expect(
        watched.samples.some(
          (s) =>
            s.phaseSecond !== null &&
            s.phaseSecond < 5 &&
            s.streams.some((t) => t.frames > 5),
        ),
      ).toBe(true);
      expect(
        watched.samples.some(
          (s) =>
            s.phaseSecond !== null &&
            s.phaseSecond >= 10 &&
            s.streams.some((t) => t.frames > 5),
        ),
      ).toBe(true);
      const before = watched.samples.find((s) => s.phaseSecond === 6)!;
      const after = watched.samples.find((s) => s.phaseSecond === 7)!;
      expect(before.streams.every((t) => !t.watching)).toBe(true);
      expect(before.receivedBytes).toBeGreaterThan(0);
      expect(after.receivedBytes).toBe(before.receivedBytes);
      expect(watched.previewBeforeWatching).toBe(true);
      expect(
        after.streams.some(
          (t) => t.pictureSize?.[0] === 960 && t.pictureSize?.[1] === 540,
        ),
      ).toBe(true);
    } finally {
      sender.child.kill();
      receiver.child.kill();
    }
  });
}

test("an H.264-only watcher triggers publication fallback and keeps watching", async ({
  request,
  baseURL,
}) => {
  test.setTimeout(90_000);
  const created = await createTestRoom(request, baseURL!, "Native fallback");
  const { invitationUrl } = await created.json();
  const sender = nativeRequest("--publish-fixture", {
    invitationUrl,
    displayName: "Publisher",
    codec: "h265",
    seconds: 18,
    quality,
  });
  await sender.reported;
  const receiver = nativeRequest("--watch-fixture", {
    invitationUrl,
    seconds: 16,
    h264Only: true,
  });
  try {
    const [sent, watched] = await Promise.all([
      sender.result as Promise<Sent>,
      receiver.result as Promise<Received>,
    ]);
    // Departure alone does not invalidate the compatible live codec.
    expect(sent.codec).toBe("h264");
    expect(sent.finalSid).not.toBe(sent.initialSid);
    expect(
      watched.samples.some(
        (s) =>
          s.second < 7 &&
          s.streams.some(
            (t) => t.codec.toLowerCase() === "video/h264" && t.frames > 5,
          ),
      ),
    ).toBe(true);
  } finally {
    sender.child.kill();
    receiver.child.kill();
  }
});

test("native playback receives stereo game audio while rejecting Discord", async ({
  request,
  baseURL,
}, testInfo) => {
  test.setTimeout(90_000);
  const created = await createTestRoom(request, baseURL!, "Native stereo");
  const { invitationUrl } = await created.json();
  const tones = toneFixtures(testInfo.outputPath("tones"));
  const exclusions = await backgroundAudioExclusions();
  const sender = nativeRequest("--publish-fixture", {
    invitationUrl,
    displayName: "Publisher",
    codec: "h264",
    seconds: 18,
    quality,
    audio: { mode: "desktop" },
    exclusions,
  });
  await sender.reported;
  const receiver = nativeRequest("--watch-fixture", {
    invitationUrl,
    seconds: 16,
  });
  tones.game();
  tones.discord();
  try {
    const [, watched] = await Promise.all([
      sender.result,
      receiver.result as Promise<Received>,
    ]);
    const audio = watched.samples
      .flatMap((s) => s.streams)
      .map((t) => t.audio)
      .filter((a) => a.frames > 20_000);
    await testInfo.attach("native-audio-metrics", {
      body: JSON.stringify(audio),
      contentType: "application/json",
    });
    writeFileSync(
      testInfo.outputPath("native-audio-metrics.json"),
      JSON.stringify(audio),
    );
    expect(audio.length).toBeGreaterThan(0);
    expect(
      audio.some(
        (a) => a.tones[0]! > 0.005 && a.channel_difference_peak > 0.005,
      ),
    ).toBe(true);
    expect(audio.every((a) => a.tones[1]! < 0.003)).toBe(true);
    // The separate native receiver must not be recaptured into the sender's
    // desktop mix: feedback amplified this 0.02 tone well beyond 0.03.
    expect(audio.every((a) => a.tones[0]! < 0.03)).toBe(true);
  } finally {
    sender.child.kill();
    receiver.child.kill();
    tones.stop();
  }
});

test("native watcher recovers after a full reconnect", async ({
  request,
  baseURL,
}) => {
  test.setTimeout(90_000);
  const created = await createTestRoom(request, baseURL!, "Native recovery");
  const { invitationUrl } = await created.json();
  const sender = nativeRequest("--publish-fixture", {
    invitationUrl,
    displayName: "Publisher",
    codec: "h264",
    seconds: 22,
    quality,
  });
  await sender.reported;
  const receiver = nativeRequest("--watch-fixture", {
    invitationUrl,
    seconds: 20,
    reconnectAfterSeconds: 12,
  });
  try {
    const [, watched] = await Promise.all([
      sender.result,
      receiver.result as Promise<Received>,
    ]);
    const post = watched.samples.filter(
      (s) => s.second >= 16 && s.streams.some((t) => t.watching),
    );
    expect(post.length).toBeGreaterThanOrEqual(2);
    const first = post[0]!;
    const last = post.at(-1)!;
    expect(last.streams[0]!.frames - first.streams[0]!.frames).toBeGreaterThan(
      5,
    );
    expect(last.receivedBytes).toBeGreaterThan(first.receivedBytes);
  } finally {
    sender.child.kill();
    receiver.child.kill();
  }
});

test("native streams recover automatically after the UDP path is interrupted", async ({
  request,
  baseURL,
}, testInfo) => {
  const outage = process.env.PODETELAR_TEST_MEDIA_OUTAGE;
  test.skip(!outage, "Run native:room-smoke with --media-outage.");
  test.setTimeout(90_000);
  const created = await createTestRoom(
    request,
    baseURL!,
    "Native UDP recovery",
  );
  expect(created.status()).toBe(201);
  const { invitationUrl } = await created.json();
  const tones = toneFixtures(testInfo.outputPath("tones"));
  tones.game();
  const sender = nativeRequest("--publish-fixture", {
    invitationUrl,
    displayName: "Publisher",
    codec: "h264",
    seconds: 35,
    quality,
    audio: { mode: "application", application: tones.application },
  });
  try {
    await sender.reported;
    const receiver = nativeRequest("--watch-fixture", {
      invitationUrl,
      seconds: 31,
      continuousWatch: true,
      watchOncePerPublisher: true,
    });
    try {
      const interrupt = async () => {
        await delay(8_000);
        writeFileSync(outage!, "");
        const blockedAt = Date.now();
        try {
          await delay(12_000);
        } finally {
          rmSync(outage!, { force: true });
        }
        return { blockedAt, restoredAt: Date.now() };
      };
      const [sent, watched, { blockedAt, restoredAt }] = await Promise.all([
        sender.result as Promise<Sent>,
        receiver.result as Promise<Received>,
        interrupt(),
      ]);
      const time = (second: number) => watched.startedAtMs + second * 1000;
      const before = watched.samples
        .filter((sample) => time(sample.second + 1) <= blockedAt)
        .at(-1);
      const recovering = watched.samples.find(
        (sample) => sample.recoveries > 0,
      );
      const after = watched.samples.filter(
        (sample) =>
          time(sample.second) >= restoredAt &&
          sample.connected &&
          sample.recoveries >= 2 &&
          sample.streams[0]?.watching,
      );
      // The decoder can emit concealment frames during packet loss. Require
      // fresh audio RTP as well as advancing native audio and video counters.
      const audioBytes = (sample: Received["samples"][number]) =>
        sample.mediaTiming
          .filter((rtp) => rtp.kind === "audio")
          .reduce((bytes, rtp) => bytes + rtp.bytes, 0);
      const firstAfter = after[0];
      const resumed =
        firstAfter &&
        after.find(
          (sample) =>
            sample.streams[0]!.frames > firstAfter.streams[0]!.frames + 5 &&
            sample.streams[0]!.audio.frames >
              firstAfter.streams[0]!.audio.frames &&
            audioBytes(sample) > audioBytes(firstAfter),
        );
      await testInfo.attach("recovery-timeline", {
        body: JSON.stringify({
          blockedAtSecond: (blockedAt - watched.startedAtMs) / 1000,
          restoredAtSecond: (restoredAt - watched.startedAtMs) / 1000,
          recoveringSecond: recovering?.second,
          resumedSecond: resumed?.second,
          samples: watched.samples.map((sample) => ({
            second: sample.second,
            recoveries: sample.recoveries,
            connected: sample.connected,
            videoFrames: sample.streams[0]?.frames,
            audioFrames: sample.streams[0]?.audio.frames,
            audioBytes: audioBytes(sample),
          })),
        }),
        contentType: "application/json",
      });
      expect(sent.previewCaptures).toBe(1);
      expect(before?.recoveries).toBe(0);
      expect(before?.streams[0]?.frames).toBeGreaterThan(5);
      expect(before?.streams[0]?.audio.frames).toBeGreaterThan(0);
      expect(time(recovering?.second ?? Infinity)).toBeGreaterThanOrEqual(
        blockedAt - 1000,
      );
      expect(time((recovering?.second ?? Infinity) + 1)).toBeLessThanOrEqual(
        restoredAt,
      );
      expect(time((resumed?.second ?? Infinity) + 1)).toBeLessThanOrEqual(
        restoredAt + 8000,
      );
    } finally {
      receiver.child.kill();
    }
  } finally {
    sender.child.kill();
    tones.stop();
  }
});

test("an unwatched preview returns after reconnect without another publisher capture", async ({
  request,
  baseURL,
}) => {
  test.setTimeout(90_000);
  const created = await createTestRoom(request, baseURL!, "Preview recovery");
  const { invitationUrl } = await created.json();
  const sender = nativeRequest("--publish-fixture", {
    invitationUrl,
    displayName: "Publisher",
    codec: "h264",
    seconds: 18,
    quality,
  });
  await sender.reported;
  const receiver = nativeRequest("--watch-fixture", {
    invitationUrl,
    seconds: 16,
    previewOnly: true,
    consumePictures: true,
    reconnectAfterSeconds: 7,
  });
  try {
    const [sent, watched] = await Promise.all([
      sender.result as Promise<Sent>,
      receiver.result as Promise<Received>,
    ]);
    const preview = (s: Received["samples"][number]) =>
      s.streams.some((t) => t.pictureSize?.[0] === 960);
    expect(watched.samples.some((s) => s.second < 7 && preview(s))).toBe(true);
    expect(
      watched.samples.some(
        (s) => s.second >= 7 && s.recoveries > 0 && s.connected && preview(s),
      ),
    ).toBe(true);
    expect(watched.samples.every((s) => s.receivedBytes === 0)).toBe(true);
    expect(sent.previewCaptures).toBe(1);
  } finally {
    sender.child.kill();
    receiver.child.kill();
  }
});

test("a watcher's aggregate connection allowance caps the shared encoder", async ({
  request,
  baseURL,
}) => {
  test.setTimeout(90_000);
  const created = await createTestRoom(request, baseURL!, "Watching allowance");
  const { invitationUrl } = await created.json();
  const sender = nativeRequest("--publish-fixture", {
    invitationUrl,
    displayName: "Publisher",
    codec: "h264",
    seconds: 18,
    quality,
  });
  await sender.reported;
  const receiver = nativeRequest("--watch-fixture", {
    invitationUrl,
    seconds: 16,
    downloadLimitMbps: 5,
  });
  try {
    const [sent, watched] = await Promise.all([
      sender.result as Promise<Sent & { metrics: { encoder_bitrate: number } }>,
      receiver.result as Promise<Received>,
    ]);
    expect(sent.mode.width).toBe(1920);
    expect(sent.mode.fps).toBe(60);
    expect(sent.metrics.encoder_bitrate).toBeLessThanOrEqual(3_494_000);
    expect(
      watched.samples.some(
        (s) => s.second >= 12 && s.streams.some((t) => t.frames > 5),
      ),
    ).toBe(true);
  } finally {
    sender.child.kill();
    receiver.child.kill();
  }
});
