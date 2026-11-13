import { setTimeout as delay } from "node:timers/promises";
import { expect, test } from "@playwright/test";
import { createTestRoom } from "../scripts/test-room.js";
import { nativeRequest } from "./fixtures.js";

type Received = {
  samples: {
    second: number;
    receivedBytes: number;
    streams: { codec: string; watching: boolean; frames: number }[];
  }[];
};

test("room members drive AV1 → HEVC → H.264 → HEVC → AV1 without subscribing", async ({
  request,
  baseURL,
}, testInfo) => {
  test.setTimeout(100_000);
  const created = await createTestRoom(request, baseURL!, "Ranked codecs");
  expect(created.status()).toBe(201);
  const { invitationUrl } = await created.json();
  const sender = nativeRequest("--publish-fixture", {
    invitationUrl,
    displayName: "Publisher",
    codec: "av1",
    seconds: 50,
    quality: {
      resolution: "p1080",
      fps: 60,
      profile: "motion",
      bitrate: { mode: "auto" },
    },
  });
  const children = [sender.child];
  try {
    await sender.reported;
    const observer = nativeRequest("--watch-fixture", {
      invitationUrl,
      seconds: 47,
      continuousWatch: true,
    });
    children.push(observer.child);
    await delay(5000);
    const hevc = nativeRequest("--watch-fixture", {
      invitationUrl,
      seconds: 28,
      hevcOnly: true,
      previewOnly: true,
    });
    children.push(hevc.child);
    await delay(8000);
    const h264 = nativeRequest("--watch-fixture", {
      invitationUrl,
      seconds: 10,
      h264Only: true,
      previewOnly: true,
    });
    children.push(h264.child);
    const [sent, watched, hevcMember, h264Member] = await Promise.all([
      sender.result as Promise<{
        codec: string;
        mode: { width: number; height: number; fps: number };
        previewCaptures: number;
      }>,
      observer.result as Promise<Received>,
      hevc.result as Promise<Received>,
      h264.result as Promise<Received>,
    ]);
    await testInfo.attach("codec-adaptation", {
      body: JSON.stringify({ sent, watched, hevcMember, h264Member }),
      contentType: "application/json",
    });
    const runs: string[] = [];
    let previous: { codec: string; frames: number } | undefined;
    for (const sample of watched.samples) {
      const stream = sample.streams.find((stream) => stream.watching);
      const codec = stream?.codec.toLowerCase();
      // Watch.frames survives replacement: require fresh frame growth within
      // each codec run instead of crediting frames from an earlier decoder.
      if (
        stream &&
        codec &&
        codec === previous?.codec &&
        stream.frames > previous.frames + 5 &&
        codec !== runs.at(-1)
      )
        runs.push(codec);
      previous = stream && codec ? { codec, frames: stream.frames } : undefined;
      expect(sample.streams.length).toBeLessThanOrEqual(1);
    }
    expect(runs).toEqual([
      "video/av1",
      "video/h265",
      "video/h264",
      "video/h265",
      "video/av1",
    ]);
    for (const member of [hevcMember, h264Member]) {
      expect(
        member.samples.every(
          (sample) =>
            sample.receivedBytes === 0 &&
            sample.streams.every((stream) => !stream.watching),
        ),
      ).toBe(true);
    }
    expect(sent.codec).toBe("av1");
    expect(sent.mode).toMatchObject({ width: 1920, height: 1080, fps: 60 });
    // All automatic replacements share the initial preview capture clock.
    expect(sent.previewCaptures).toBe(1);
  } finally {
    for (const child of children) child.kill();
  }
});
