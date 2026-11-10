import { expect, test } from "@playwright/test";
import { RoomServiceClient } from "livekit-server-sdk";
import { setTimeout as delay } from "node:timers/promises";
import { createTestRoom } from "../scripts/test-room.js";
import { nativeRequest, toneFixtures } from "./fixtures.js";

type Watched = {
  closed: boolean;
  previewBeforeWatching: boolean;
  samples: {
    second: number;
    phaseSecond: number | null;
    receivedBytes: number;
    streams: {
      frames: number;
      watching: boolean;
      codec: string;
      pictureSize: number[] | null;
      audio: { frames: number; tones: number[] };
    }[];
  }[];
};
type Sent = { captureStopped: boolean; previewUploads: number; codec: string };
const quality = {
  resolution: "p1080",
  fps: 60,
  profile: "motion",
  bitrate: { mode: "manual", bps: 35_000_000 },
};
async function protect(invitationUrl: string): Promise<string> {
  const result = (await nativeRequest("--protect-invitation", {
    invitationUrl,
  }).result) as { invitationUrl: string };
  // Never include invitation material in failed assertions or test attachments.
  expect(result.invitationUrl.startsWith("FSPRIVATE1:")).toBe(true);
  return result.invitationUrl;
}
function hasVideo(result: Watched) {
  return result.samples.some((s) => s.streams.some((t) => t.frames > 5));
}
function mediaAdmin() {
  return new RoomServiceClient(
    process.env.FREEDOMSTREAM_TEST_SFU_URL ?? "http://127.0.0.1:17880",
    process.env.LIVEKIT_API_KEY!,
    process.env.LIVEKIT_API_SECRET!,
  );
}
function noContent(result: Watched) {
  expect(result.closed).toBe(true);
  expect(hasVideo(result)).toBe(false);
  expect(result.previewBeforeWatching).toBe(false);
  expect(
    result.samples.every((s) =>
      s.streams.every((t) => t.pictureSize === null && t.audio.frames === 0),
    ),
  ).toBe(true);
}

test("protected AV1 to H.264 compatibility fallback retains encryption and watch intent", async ({
  request,
  baseURL,
}) => {
  test.setTimeout(90_000);
  const created = await createTestRoom(
    request,
    baseURL!,
    "Protected compatibility fallback",
  );
  const room = await created.json();
  const invitationUrl = await protect(room.invitationUrl);
  const sender = nativeRequest("--publish-fixture", {
    invitationUrl,
    displayName: "Protected publisher",
    codec: "av1",
    seconds: 23,
    quality,
  });
  await sender.reported;
  const receiver = nativeRequest("--watch-fixture", {
    invitationUrl,
    seconds: 19,
    h264Only: true,
  });
  try {
    const [sent, watched] = await Promise.all([
      sender.result as Promise<Sent>,
      receiver.result as Promise<Watched>,
    ]);
    expect(sent.codec).toBe("h264");
    expect(
      watched.samples.some((s) =>
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

test("removing an authenticated descriptor stops an existing protected subscription", async ({
  request,
  baseURL,
}) => {
  test.setTimeout(90_000);
  const created = await createTestRoom(
    request,
    baseURL!,
    "Protected descriptor removal",
  );
  const room = await created.json();
  const invitationUrl = await protect(room.invitationUrl);
  const sender = nativeRequest("--publish-fixture", {
    invitationUrl,
    displayName: "Protected publisher",
    codec: "h264",
    seconds: 25,
    quality,
  });
  await sender.reported;
  const receiver = nativeRequest("--watch-fixture", {
    invitationUrl,
    seconds: 21,
  });
  try {
    await delay(5000);
    const admin = mediaAdmin();
    const participants = await admin.listParticipants(
      `freedomstream_${room.room.id}`,
    );
    const publisher = participants.find(
      (p) =>
        Boolean(p.attributes["freedomstream.e2ee.v1"]) && p.tracks.length > 0,
    );
    expect(Boolean(publisher)).toBe(true);
    await admin.updateParticipant(
      `freedomstream_${room.room.id}`,
      publisher!.identity,
      { attributes: { "freedomstream.e2ee.v1": "" } },
    );
    const watched = (await receiver.result) as Watched;
    expect(hasVideo(watched)).toBe(true);
    const settled = watched.samples.filter((s) => s.second >= 12);
    expect(settled.length).toBeGreaterThan(3);
    expect(settled.every((s) => s.streams.length === 0)).toBe(true);
    expect(settled.at(-1)!.receivedBytes).toBe(settled[0]!.receivedBytes);
    await sender.result;
  } finally {
    sender.child.kill();
    receiver.child.kill();
  }
});

for (const codec of ["h264", "h265", "av1"] as const) {
  test(`protected ${codec}: native video, encrypted preview, stop and rewatch`, async ({
    request,
    baseURL,
  }) => {
    test.setTimeout(100_000);
    const created = await createTestRoom(
      request,
      baseURL!,
      `Protected ${codec}`,
    );
    expect(created.status()).toBe(201);
    const room = await created.json();
    const invitationUrl = await protect(room.invitationUrl);
    const sender = nativeRequest("--publish-fixture", {
      invitationUrl,
      displayName: "Protected publisher",
      codec,
      seconds: 28,
      quality,
    });
    await sender.reported;
    const receiver = nativeRequest("--watch-fixture", {
      invitationUrl,
      seconds: 25,
      waitForPreview: true,
    });
    try {
      if (codec === "h264") {
        const admin = mediaAdmin();
        let previewPath: string | undefined;
        await expect
          .poll(
            async () => {
              const participants = await admin.listParticipants(
                `freedomstream_${room.room.id}`,
              );
              const publisher = participants.find((p) =>
                Boolean(
                  p.attributes["freedomstream.e2ee.v1"] &&
                  p.attributes["freedomstream.preview.screen"],
                ),
              );
              if (!publisher) return false;
              const descriptor = JSON.parse(
                publisher.attributes["freedomstream.preview.screen"]!,
              );
              previewPath = `/api/rooms/${room.room.id}/previews/${publisher.identity}/screen/${descriptor.revision}`;
              return true;
            },
            { timeout: 8000 },
          )
          .toBe(true);
        const preview = await request.get(previewPath!);
        expect(preview.status()).toBe(200);
        expect(preview.headers()["content-type"]).toBe(
          "application/vnd.freedomstream.preview-v1",
        );
        const opaque = await preview.body();
        expect(opaque.subarray(0, 4).toString("ascii")).toBe("FSP1");
        expect(opaque.length).toBeLessThanOrEqual(266_240);
      }
      const [sent, watched] = await Promise.all([
        sender.result as Promise<Sent>,
        receiver.result as Promise<Watched>,
      ]);
      expect(sent.captureStopped).toBe(true);
      expect(sent.previewUploads).toBe(1);
      expect(watched.previewBeforeWatching).toBe(true);
      expect(hasVideo(watched)).toBe(true);
      expect(
        watched.samples.some(
          (s) =>
            s.phaseSecond !== null &&
            s.phaseSecond >= 10 &&
            s.streams.some((t) => t.frames > 5),
        ),
      ).toBe(true);
      const stopped = watched.samples.find((s) => s.phaseSecond === 6);
      const stillStopped = watched.samples.find((s) => s.phaseSecond === 7);
      expect(Boolean(stopped && stillStopped)).toBe(true);
      expect(stopped!.streams.every((t) => !t.watching)).toBe(true);
      expect(stillStopped!.receivedBytes).toBe(stopped!.receivedBytes);
    } finally {
      sender.child.kill();
      receiver.child.kill();
    }
  });
}

test("valid server credentials without the private root cannot recover protected content", async ({
  request,
  baseURL,
}) => {
  test.setTimeout(100_000);
  const created = await createTestRoom(
    request,
    baseURL!,
    "Protected wrong root",
  );
  const room = await created.json();
  const invitationUrl = await protect(room.invitationUrl);
  const wrongInvitation = await protect(room.invitationUrl);
  const sender = nativeRequest("--publish-fixture", {
    invitationUrl,
    displayName: "Protected publisher",
    codec: "h264",
    seconds: 22,
    quality,
  });
  await sender.reported;
  const wrong = nativeRequest("--watch-fixture", {
    invitationUrl: wrongInvitation,
    seconds: 18,
    waitForPreview: true,
  });
  const missing = nativeRequest("--watch-fixture", {
    invitationUrl: room.invitationUrl,
    seconds: 18,
    waitForPreview: true,
  });
  try {
    const [, wrongResult, missingResult] = await Promise.all([
      sender.result,
      wrong.result as Promise<Watched>,
      missing.result as Promise<Watched>,
    ]);
    noContent(wrongResult);
    noContent(missingResult);
  } finally {
    sender.child.kill();
    wrong.child.kill();
    missing.child.kill();
  }
});

test("protected receiver rejects a legacy plaintext publisher and its JPEG preview", async ({
  request,
  baseURL,
}) => {
  test.setTimeout(90_000);
  const created = await createTestRoom(
    request,
    baseURL!,
    "Protected downgrade",
  );
  const room = await created.json();
  const invitationUrl = await protect(room.invitationUrl);
  const sender = nativeRequest("--publish-fixture", {
    invitationUrl: room.invitationUrl,
    displayName: "Plaintext publisher",
    codec: "h264",
    seconds: 18,
    quality,
  });
  await sender.reported;
  const receiver = nativeRequest("--watch-fixture", {
    invitationUrl,
    seconds: 14,
    waitForPreview: true,
  });
  try {
    const [, watched] = await Promise.all([
      sender.result,
      receiver.result as Promise<Watched>,
    ]);
    noContent(watched);
    expect(watched.samples.every((s) => s.receivedBytes === 0)).toBe(true);
  } finally {
    sender.child.kill();
    receiver.child.kill();
  }
});

test("server-tampered sender descriptor cannot grant a protected subscription", async ({
  request,
  baseURL,
}) => {
  test.setTimeout(90_000);
  const created = await createTestRoom(
    request,
    baseURL!,
    "Protected descriptor tampering",
  );
  const room = await created.json();
  const invitationUrl = await protect(room.invitationUrl);
  const sender = nativeRequest("--publish-fixture", {
    invitationUrl,
    displayName: "Protected publisher",
    codec: "h264",
    seconds: 20,
    quality,
  });
  await sender.reported;
  const admin = mediaAdmin();
  const participants = await admin.listParticipants(
    `freedomstream_${room.room.id}`,
  );
  const publisher = participants.find(
    (p) =>
      Boolean(p.attributes["freedomstream.e2ee.v1"]) && p.tracks.length > 0,
  );
  expect(Boolean(publisher)).toBe(true);
  await admin.updateParticipant(
    `freedomstream_${room.room.id}`,
    publisher!.identity,
    { attributes: { "freedomstream.e2ee.v1": '{"version":0}' } },
  );
  const receiver = nativeRequest("--watch-fixture", {
    invitationUrl,
    seconds: 15,
    waitForPreview: true,
  });
  try {
    noContent((await receiver.result) as Watched);
    await sender.result;
  } finally {
    sender.child.kill();
    receiver.child.kill();
  }
});

test("protected native stereo audio reaches only a correct-root viewer", async ({
  request,
  baseURL,
}, testInfo) => {
  test.setTimeout(90_000);
  const created = await createTestRoom(request, baseURL!, "Protected audio");
  const room = await created.json();
  const invitationUrl = await protect(room.invitationUrl);
  const tones = toneFixtures(testInfo.outputPath("tones"));
  const sender = nativeRequest("--publish-fixture", {
    invitationUrl,
    displayName: "Protected audio publisher",
    codec: "h264",
    seconds: 22,
    quality,
    audio: { mode: "desktop" },
  });
  await sender.reported;
  tones.game();
  tones.discord();
  const receiver = nativeRequest("--watch-fixture", {
    invitationUrl,
    seconds: 18,
  });
  try {
    const [, watched] = await Promise.all([
      sender.result,
      receiver.result as Promise<Watched>,
    ]);
    expect(hasVideo(watched)).toBe(true);
    const audio = watched.samples
      .flatMap((s) => s.streams)
      .map((t) => t.audio)
      .filter((a) => a.frames > 20_000);
    expect(audio.some((a) => a.tones[0]! > 0.005)).toBe(true);
    expect(audio.every((a) => a.tones[1]! < 0.003)).toBe(true);
  } finally {
    sender.child.kill();
    receiver.child.kill();
    tones.stop();
  }
});
