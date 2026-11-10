import { expect, test } from "@playwright/test";
import { AccessToken, RoomServiceClient } from "livekit-server-sdk";
import { Encryption_Type, ParticipantInfo } from "@livekit/protocol";
import { setTimeout as delay } from "node:timers/promises";
import { createTestRoom } from "../scripts/test-room.js";
import { nativeRequest } from "./fixtures.js";
import { signalingFaults } from "./signaling-faults.js";

const quality = {
  resolution: "p1080",
  fps: 60,
  profile: "motion",
  bitrate: { mode: "manual", bps: 35_000_000 },
};
async function protect(invitationUrl: string) {
  return (
    (await nativeRequest("--protect-invitation", { invitationUrl }).result) as {
      invitationUrl: string;
    }
  ).invitationUrl;
}
type Received = {
  samples: {
    receivedBytes: number;
    streams: {
      frames: number;
      watching: boolean;
      pictureSize: number[] | null;
      audio: { frames: number };
    }[];
  }[];
};

test("malicious signaling cannot downgrade a protected sender before activation", async ({
  request,
  baseURL,
}) => {
  test.setTimeout(60_000);
  const created = await createTestRoom(
    request,
    baseURL!,
    "Outgoing downgrade fault",
  );
  const room = await created.json();
  const invitationUrl = await protect(room.invitationUrl);
  let changed = false;
  const proxy = await signalingFaults(baseURL!, (response) => {
    if (
      response.message.case === "trackPublished" &&
      response.message.value.track
    ) {
      response.message.value.track.encryption = Encryption_Type.NONE;
      changed = true;
    }
  });
  const sender = nativeRequest("--publish-fixture", {
    invitationUrl,
    displayName: "Protected publisher",
    codec: "h264",
    seconds: 12,
    quality,
    mediaServerUrl: proxy.url,
  });
  try {
    await expect(sender.result).rejects.toThrow();
    expect(changed).toBe(true);
  } finally {
    sender.child.kill();
    await proxy.close();
  }
});

test("replayed descriptor and None-to-GCM metadata cannot authorize plaintext playback", async ({
  request,
  baseURL,
}) => {
  test.setTimeout(110_000);
  const created = await createTestRoom(
    request,
    baseURL!,
    "Incoming downgrade fault",
  );
  const room = await created.json();
  const invitationUrl = await protect(room.invitationUrl);
  const sfUrl =
    process.env.FREEDOMSTREAM_TEST_SFU_URL ?? "http://127.0.0.1:17880";
  const admin = new RoomServiceClient(
    sfUrl,
    process.env.LIVEKIT_API_KEY!,
    process.env.LIVEKIT_API_SECRET!,
  );
  const mediaRoom = `freedomstream_${room.room.id}`;
  const original = nativeRequest("--watch-fixture", {
    invitationUrl,
    seconds: 10,
  });
  let descriptor = "",
    identity = "";
  await expect
    .poll(
      async () => {
        const participant = (await admin.listParticipants(mediaRoom)).find(
          (p) => Boolean(p.attributes["freedomstream.e2ee.v1"]),
        );
        if (!participant) return false;
        descriptor = participant.attributes["freedomstream.e2ee.v1"]!;
        identity = participant.identity;
        return true;
      },
      { timeout: 8000 },
    )
    .toBe(true);
  original.child.kill();
  await original.result.catch(() => {});
  await admin.removeParticipant(mediaRoom, identity);
  await expect
    .poll(
      async () =>
        (await admin.listParticipants(mediaRoom)).some(
          (p) => p.identity === identity,
        ),
      { timeout: 8000 },
    )
    .toBe(false);
  // Backend signing authority and a public descriptor do not include the root.
  const token = new AccessToken(
    process.env.LIVEKIT_API_KEY!,
    process.env.LIVEKIT_API_SECRET!,
    { identity, ttl: 120 },
  );
  token.addGrant({
    roomJoin: true,
    room: mediaRoom,
    canPublish: true,
    canSubscribe: true,
  });
  const attacker = nativeRequest("--publish-fixture", {
    invitationUrl: room.invitationUrl,
    displayName: "Untrusted source",
    codec: "h264",
    seconds: 29,
    quality,
    mediaServerUrl: sfUrl.replace("http:", "ws:"),
    mediaToken: await token.toJwt(),
  });
  await attacker.reported;
  await admin.updateParticipant(mediaRoom, identity, {
    attributes: { "freedomstream.e2ee.v1": descriptor },
  });
  const proxy = await signalingFaults(baseURL!);
  const receiver = nativeRequest("--watch-fixture", {
    invitationUrl,
    seconds: 23,
    mediaServerUrl: proxy.url,
  });
  try {
    await expect
      .poll(() => Boolean(proxy.identity()), { timeout: 8000 })
      .toBe(true);
    const sender = await admin.getParticipant(mediaRoom, identity);
    expect(
      sender.tracks.length > 0 &&
        sender.tracks.every((t) => t.encryption === Encryption_Type.NONE),
    ).toBe(true);
    await admin.updateSubscriptions(
      mediaRoom,
      proxy.identity(),
      sender.tracks.map((t) => t.sid),
      true,
    );
    await delay(2500);
    const forged = ParticipantInfo.fromBinary(sender.toBinary());
    for (const track of forged.tracks) {
      track.encryption = Encryption_Type.GCM;
      track.packetTrailerFeatures = [1];
    }
    proxy.participant(forged);
    const watched = (await receiver.result) as Received;
    expect(
      watched.samples.some((s) => s.streams.some((t) => t.frames > 0)),
    ).toBe(false);
    expect(watched.samples.some((s) => s.receivedBytes > 1000)).toBe(true);
    expect(watched.samples.some((s) => s.streams.some((t) => t.watching))).toBe(
      true,
    );
    expect(
      watched.samples.every((s) =>
        s.streams.every((t) => t.pictureSize === null && t.audio.frames === 0),
      ),
    ).toBe(true);
    await attacker.result;
  } finally {
    attacker.child.kill();
    receiver.child.kill();
    await proxy.close();
  }
});
