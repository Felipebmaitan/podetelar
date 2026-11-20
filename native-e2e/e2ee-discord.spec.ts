import { expect, test } from "@playwright/test";
import { LivekitDirectory } from "../apps/server/src/media.js";
import { nativeRequest } from "./fixtures.js";
import { voiceFixture } from "./voice-fixture.js";

async function privateVoiceFixture() {
  const fixture = await voiceFixture();
  const { app, config, origin, guild, a, credential } = fixture;
  const privateMaterial: string[] = [];
  let leaked = false;
  let mediaTokenRequests = 0;
  // Inspect in memory; never retain or attach request contents or secrets.
  app.addHook("preValidation", async (request) => {
    const text = [
      request.url,
      JSON.stringify(request.headers),
      Buffer.isBuffer(request.body) ? "" : JSON.stringify(request.body),
    ].join(" ");
    leaked ||= privateMaterial.some((secret) => text.includes(secret));
    if (request.url.endsWith("/native-media-token")) mediaTokenRequests++;
  });
  const location = (channelId: string) => ({
    guildId: guild,
    channelId,
    guildName: "Fixture guild",
    channelName: "Fixture channel",
    enabled: true,
  });
  const nativeVoice = (token: string, channelId: string) => ({
    credential: { server: origin, token },
    location: location(channelId),
  });
  const processes: ReturnType<typeof nativeRequest>[] = [];
  const launch = (arg: string, input: unknown) => {
    const child = nativeRequest(arg, input);
    processes.push(child);
    return child;
  };
  const disableGuests = async () => {
    const login = await app.inject({
      method: "POST",
      url: "/api/admin/login",
      headers: { origin },
      payload: { password: config.adminToken },
    });
    expect(login.statusCode).toBe(200);
    const disabled = await app.inject({
      method: "POST",
      url: `/api/admin/discord/channels/${guild}/${a}`,
      headers: {
        origin,
        cookie: String(login.headers["set-cookie"]).split(";")[0]!,
      },
      payload: { action: "disable" },
    });
    expect(disabled.statusCode).toBe(204);
  };
  return {
    ...fixture,
    nativeVoice,
    organizer: nativeVoice(credential.token, a),
    launch,
    disableGuests,
    privateMaterial,
    leaked: () => leaked,
    mediaTokenRequests: () => mediaTokenRequests,
    close: async () => {
      for (const child of processes) child.child.kill();
      await Promise.allSettled(processes.map((p) => p.result));
      await fixture.close();
    },
  };
}

test("a native voice organizer shares protected media and moves without carrying the room key", async () => {
  test.skip(
    !process.env.PODETELAR_TEST_SFU_URL,
    "Requires the isolated SFU and a native qualification build.",
  );
  test.setTimeout(90_000);
  const fixture = await privateVoiceFixture();
  const {
    config,
    voice,
    guild,
    b,
    user,
    credential,
    organizer: discord,
    launch,
  } = fixture;
  const directory = new LivekitDirectory(config);
  try {
    await fixture.listen();
    const made = (await launch("--protect-discord-fixture", discord)
      .result) as { invitationUrl: string };
    const invitationUrl = made.invitationUrl;
    expect(invitationUrl.startsWith("FSPRIVATE1:")).toBe(true);
    const envelope = JSON.parse(
      Buffer.from(
        invitationUrl.slice("FSPRIVATE1:".length),
        "base64url",
      ).toString(),
    ) as { root: string; invitation: string };
    expect(envelope.root.length).toBe(43);
    fixture.privateMaterial.push(invitationUrl, envelope.root);
    const room = new URL(envelope.invitation).pathname.split("/").at(-1)!;
    const publication = {
      invitationUrl,
      displayName: "Voice fixture",
      codec: "h264",
      seconds: 50,
      quality: {
        resolution: "p1080",
        fps: 60,
        profile: "motion",
        bitrate: { mode: "auto" },
      },
    };
    const sender = launch("--publish-fixture", { ...publication, discord });
    await sender.reported;
    const guest = launch("--watch-fixture", {
      invitationUrl,
      seconds: 28,
      waitForPreview: true,
      continuousWatch: true,
      reportPlayback: true,
    });
    // Wait for a watch request before closing guest admission. Existing guest
    // sessions are independent of the organizer's Discord membership.
    await expect
      .poll(
        async () => {
          const members = await directory.diagnosticParticipants(room);
          return [...members.values()].some(
            (p) =>
              !!p.attributes?.["freedomstream.e2ee.v1"] &&
              Object.keys(p.attributes).some((a) =>
                a.startsWith("podetelar.watch."),
              ),
          );
        },
        { timeout: 15000 },
      )
      .toBe(true);
    expect(((await guest.reported) as { decoded?: boolean }).decoded).toBe(
      true,
    );
    const joined = await directory.members(room);
    const guestId = joined.find(
      (m) => m.displayName === "Native playback check",
    )?.id;
    expect(Boolean(guestId)).toBe(true);
    await fixture.disableGuests();
    const issuedBeforeDenial = fixture.mediaTokenRequests();
    expect(issuedBeforeDenial).toBeGreaterThanOrEqual(2);
    await expect(
      launch("--watch-fixture", { invitationUrl, seconds: 10 }).result,
    ).rejects.toThrow();
    voice.index.apply("VOICE_STATE_UPDATE", {
      guild_id: guild,
      user_id: user,
      channel_id: b,
    });
    await expect(
      launch("--publish-fixture", {
        ...publication,
        discord: {
          ...discord,
          location: fixture.nativeVoice(credential.token, b).location,
        },
      }).result,
    ).rejects.toThrow();
    expect(fixture.mediaTokenRequests()).toBe(issuedBeforeDenial);
    await expect
      .poll(async () => (await directory.members(room)).map((p) => p.id), {
        timeout: 6000,
      })
      .toEqual([guestId]);
    const result = (await guest.result) as {
      previewBeforeWatching: boolean;
      samples: { connected: boolean; streams: { frames: number }[] }[];
    };
    expect(result.previewBeforeWatching).toBe(true);
    expect(
      result.samples.some((s) => s.streams.some((t) => t.frames > 5)),
    ).toBe(true);
    expect(result.samples.at(-1)?.connected).toBe(true);
    expect(fixture.leaked()).toBe(false);
  } finally {
    await fixture.close();
  }
});
test("voice-only encryption joins two signed-in channel members when guest access is disabled", async () => {
  test.skip(
    !process.env.PODETELAR_TEST_SFU_URL,
    "Requires the isolated real SFU.",
  );
  test.setTimeout(90_000);
  const fixture = await privateVoiceFixture();
  const { app, origin, guild, a, b, voice, organizer, launch } = fixture;
  const listenerId = "423456789012345678";
  const listener = fixture.issueLogin(listenerId, "Second voice member");
  const absent = fixture.issueLogin("523456789012345678", "Absent member");
  const move = (userId: string, channelId: string) =>
    voice.index.apply("VOICE_STATE_UPDATE", {
      guild_id: guild,
      user_id: userId,
      channel_id: channelId,
    });
  try {
    await fixture.listen();
    await fixture.disableGuests();
    move(listenerId, a);
    const issued = await app.inject({
      method: "POST",
      url: "/api/auth/voice/join",
      headers: { origin, authorization: `Bearer ${fixture.credential.token}` },
      payload: {
        guildId: guild,
        channelId: a,
        deviceId: `pc1_${"e".repeat(64)}`,
      },
    });
    expect(issued.statusCode).toBe(200);
    expect(issued.json().roomInvitationUrl).toBeNull();
    expect(issued.json().invitationUrl).toBeNull();
    const made = (await launch("--protect-discord-fixture", organizer)
      .result) as { invitationUrl: string };
    const invitationUrl = made.invitationUrl;
    expect(invitationUrl.startsWith("PTVOICE1:")).toBe(true);
    const envelope = JSON.parse(
      Buffer.from(
        invitationUrl.slice("PTVOICE1:".length),
        "base64url",
      ).toString(),
    );
    expect(
      typeof envelope.root === "string" && envelope.root.length === 43,
    ).toBe(true);
    expect(Object.hasOwn(envelope, "invitation")).toBe(false);
    fixture.privateMaterial.push(invitationUrl, envelope.root);
    const sender = launch("--publish-fixture", {
      invitationUrl,
      discord: organizer,
      displayName: "Voice organizer",
      codec: "h264",
      seconds: 48,
      quality: {
        resolution: "p1080",
        fps: 60,
        profile: "motion",
        bitrate: { mode: "auto" },
      },
    });
    await sender.reported;
    const watch = {
      invitationUrl,
      seconds: 15,
      continuousWatch: true,
      waitForPreview: true,
    };
    const member = launch("--watch-fixture", {
      ...watch,
      discord: fixture.nativeVoice(listener.token, a),
    });
    const viewed = (await member.result) as {
      previewBeforeWatching: boolean;
      samples: { streams: { frames: number }[] }[];
    };
    expect(viewed.previewBeforeWatching).toBe(true);
    expect(
      viewed.samples.some((s) => s.streams.some((t) => t.frames > 5)),
    ).toBe(true);
    const mediaBeforeDenied = fixture.mediaTokenRequests();
    expect(mediaBeforeDenied).toBeGreaterThanOrEqual(2);
    // The code alone cannot admit an anonymous guest or a logged-in nonmember.
    await expect(launch("--watch-fixture", watch).result).rejects.toThrow();
    await expect(
      launch("--watch-fixture", {
        ...watch,
        discord: fixture.nativeVoice(absent.token, a),
      }).result,
    ).rejects.toThrow();
    move(listenerId, b);
    await expect(
      launch("--watch-fixture", {
        ...watch,
        discord: fixture.nativeVoice(listener.token, b),
      }).result,
    ).rejects.toThrow();
    expect(fixture.mediaTokenRequests()).toBe(mediaBeforeDenied);
    expect(fixture.leaked()).toBe(false);
  } finally {
    await fixture.close();
  }
});
