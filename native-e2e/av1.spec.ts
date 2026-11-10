import { writeFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { createTestRoom } from "../scripts/test-room.js";
import { nativeRequest } from "./fixtures.js";

type Stream = {
  codec: string;
  frames: number;
  watching: boolean;
  pictureSize: number[] | null;
  audio: { frames: number; tones: number[]; channel_difference_peak: number };
};
type Received = {
  closed: boolean;
  previewBeforeWatching: boolean;
  samples: {
    second: number;
    phaseSecond: number | null;
    recoveries: number;
    connected: boolean;
    watching: boolean;
    receivedBytes: number;
    decoders: { implementation: string; hardware: boolean }[];
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
};
const quality = {
  resolution: "p1080",
  fps: 60,
  profile: "motion",
  bitrate: { mode: "auto" },
};

for (const software of [false, true]) {
  const codec = "av1";
  test(`native ${codec} ${software ? "CPU" : "GPU"} decode, previews, explicit unsubscribe and rewatch`, async ({
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
    const previous = process.env.FREEDOMSTREAM_AV1_DECODER;
    process.env.FREEDOMSTREAM_AV1_DECODER = software ? "software" : "hardware";
    const receiver = nativeRequest("--watch-fixture", {
      invitationUrl,
      seconds: 24,
      waitForPreview: true,
      reconnectAfterSeconds: 17,
    });
    if (previous === undefined) delete process.env.FREEDOMSTREAM_AV1_DECODER;
    else process.env.FREEDOMSTREAM_AV1_DECODER = previous;
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
      expect(sent.codec).toBe("av1");
      expect(
        watched.samples.some((s) =>
          s.decoders.some((d) =>
            software
              ? /dav1d/i.test(d.implementation) && !d.hardware
              : d.implementation === "FreedomStream Direct3D11" && d.hardware,
          ),
        ),
      ).toBe(true);
      expect(
        watched.samples.some((s) =>
          s.streams.some(
            (t) => t.codec.toLowerCase() === "video/av1" && t.frames > 5,
          ),
        ),
      ).toBe(true);
      expect(sent.captureStopped).toBe(true);
      expect(sent.previewUploads).toBe(1);
      expect(watched.closed).toBe(true);
      const beforeReconnect = watched.samples
        .filter((s) => s.second < 17)
        .at(-1)!;
      const recovered = watched.samples.filter(
        (s) => s.second >= 19 && s.recoveries > 0 && s.connected,
      );
      expect(recovered.length).toBeGreaterThanOrEqual(2);
      const baseline = Math.max(
        ...beforeReconnect.streams.map((t) => t.frames),
      );
      const first = Math.max(...recovered[0]!.streams.map((t) => t.frames));
      const last = Math.max(...recovered.at(-1)!.streams.map((t) => t.frames));
      expect(first).toBeGreaterThan(baseline);
      expect(last).toBeGreaterThan(first);
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

test("an H.264-only watcher triggers AV1 publication fallback and keeps watching", async ({
  request,
  baseURL,
}) => {
  test.setTimeout(90_000);
  const created = await createTestRoom(request, baseURL!, "Native fallback");
  const { invitationUrl } = await created.json();
  const sender = nativeRequest("--publish-fixture", {
    invitationUrl,
    displayName: "Publisher",
    codec: "av1",
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
