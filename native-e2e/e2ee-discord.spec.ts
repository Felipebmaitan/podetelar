import { expect, test } from "@playwright/test";
import { setTimeout as delay } from "node:timers/promises";
import { LivekitDirectory } from "../apps/server/src/media.js";
import { nativeRequest } from "./fixtures.js";
import { voiceFixture } from "./voice-fixture.js";

test("a native voice organizer shares protected media and moves without carrying the room key", async () => {
  test.skip(
    !process.env.PODETELAR_TEST_SFU_URL,
    "Requires the isolated SFU and a native qualification build.",
  );
  test.setTimeout(90_000);
  const fixture = await voiceFixture();
  const { app, config, voice, origin, guild, a, b, user, credential } = fixture;
  const directory = new LivekitDirectory(config);
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
  const discord = {
    credential: { server: origin, token: credential.token },
    location: location(a),
  };
  const processes: ReturnType<typeof nativeRequest>[] = [];
  const launch = (arg: string, input: unknown) => {
    const child = nativeRequest(arg, input);
    processes.push(child);
    return child;
  };
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
    privateMaterial.push(invitationUrl, envelope.root);
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
    await delay(2500);
    const joined = await directory.members(room);
    const guestId = joined.find(
      (m) => m.displayName === "Native playback check",
    )?.id;
    expect(Boolean(guestId)).toBe(true);
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
    const issuedBeforeDenial = mediaTokenRequests;
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
        discord: { ...discord, location: location(b) },
      }).result,
    ).rejects.toThrow();
    expect(mediaTokenRequests).toBe(issuedBeforeDenial);
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
    expect(leaked).toBe(false);
  } finally {
    for (const child of processes) child.child.kill();
    await Promise.allSettled(processes.map((p) => p.result));
    await fixture.close();
  }
});
