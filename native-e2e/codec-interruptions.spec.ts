import { setTimeout as delay } from "node:timers/promises";
import { expect, test } from "@playwright/test";
import { RoomServiceClient, TrackType } from "livekit-server-sdk";
import { createTestRoom } from "../scripts/test-room.js";
import { nativeRequest } from "./fixtures.js";

type Received = {
  samples: {
    second: number;
    receivedBytes: number;
    streams: { codec: string; watching: boolean; frames: number }[];
  }[];
};

for (const codec of ["av1", "h265"] as const) {
  test(`${codec} interruption preserves hardware capabilities, publication and recovery`, async ({
    request,
    baseURL,
  }, testInfo) => {
    test.skip(
      !process.env.PODETELAR_TEST_SFU_PID,
      "Requires the isolated native smoke SFU.",
    );
    test.setTimeout(65000);
    const created = await createTestRoom(
      request,
      baseURL!,
      "Stable hardware codecs",
    );
    expect(created.status()).toBe(201);
    const { room, invitationUrl } = await created.json();
    const sender = nativeRequest("--publish-fixture", {
      invitationUrl,
      displayName: "Publisher",
      codec,
      seconds: 35,
      quality: {
        resolution: "p1080",
        fps: 60,
        profile: "motion",
        bitrate: { mode: "auto" },
      },
    });
    let viewer: ReturnType<typeof nativeRequest> | undefined;
    const media = new RoomServiceClient(
      `http://127.0.0.1:${process.env.PODETELAR_SMOKE_SFU_PORT ?? 17880}`,
      process.env.LIVEKIT_API_KEY!,
      process.env.LIVEKIT_API_SECRET!,
    );
    const roomName = `podetelar_${room.id}`;
    try {
      await sender.reported;
      viewer = nativeRequest("--watch-fixture", {
        invitationUrl,
        seconds: 32,
        continuousWatch: true,
      });
      const publisher = async () =>
        (await media.listParticipants(roomName)).find(
          (p) => p.name === "Publisher",
        );
      await expect
        .poll(async () =>
          (await publisher())?.tracks.some((t) => t.type === TrackType.VIDEO),
        )
        .toBe(true);
      await delay(5000);
      const current = (await publisher())!;
      const video = current.tracks.find((t) => t.type === TrackType.VIDEO)!;
      const members = await media.listParticipants(roomName);
      const watcher = members.find((p) => p.name === "Native playback check")!;
      const attribute = "podetelar.decode.hardware.v1";
      const before = watcher.attributes[attribute];
      expect(before).toContain(codec === "av1" ? "av1" : "hevc");
      await media.mutePublishedTrack(
        roomName,
        current.identity,
        video.sid,
        true,
      );
      // Cross the removed six-second rejection threshold on both advanced codecs.
      await delay(9000);
      expect(
        (await publisher())?.tracks.find((t) => t.type === TrackType.VIDEO)
          ?.sid,
      ).toBe(video.sid);
      expect(
        (await media.getParticipant(roomName, watcher.identity)).attributes[
          attribute
        ],
      ).toBe(before);
      await media.mutePublishedTrack(
        roomName,
        current.identity,
        video.sid,
        false,
      );
      const [sent, watched] = (await Promise.all([
        sender.result,
        viewer.result,
      ])) as [
        {
          codec: string;
          initialSid: string;
          finalSid: string;
          previewCaptures: number;
        },
        Received,
      ];
      await testInfo.attach("interruption", {
        body: JSON.stringify({ sent, watched }),
        contentType: "application/json",
      });
      expect(sent.codec).toBe(codec);
      expect(sent.finalSid).toBe(sent.initialSid);
      expect(sent.previewCaptures).toBe(1);
      const received = watched.samples.filter((s) =>
        s.streams.some((t) => t.watching),
      );
      expect(
        received.every((s) =>
          s.streams.every((t) => t.codec.toLowerCase() === `video/${codec}`),
        ),
      ).toBe(true);
      const count = (sample: Received["samples"][number]) =>
        Math.max(0, ...sample.streams.map((t) => t.frames));
      const initial = received.find((s) => s.second >= 4)!;
      const paused = received.filter((s) => s.second >= 9 && s.second <= 12);
      const recovered = received.filter(
        (s) => s.second >= 20 && s.second <= 29,
      );
      expect(count(initial)).toBeGreaterThan(5);
      expect(paused.length).toBeGreaterThanOrEqual(2);
      expect(count(paused.at(-1)!)).toBeLessThanOrEqual(count(paused[0]!) + 1);
      expect(count(recovered.at(-1)!)).toBeGreaterThan(
        count(recovered[0]!) + 60,
      );
    } finally {
      sender.child.kill();
      viewer?.child.kill();
    }
  });
}
