import { test, expect, type BrowserContext } from "@playwright/test";
import type { Room as LiveRoom } from "livekit-client";
import { readFileSync } from "node:fs";
import { voiceFixture } from "./voice-fixture.js";
import { LivekitDirectory } from "../apps/server/src/media.js";

test("confirmed voice departure removes the real SFU peer while an invitation peer stays connected", async ({
  browser,
}) => {
  test.skip(
    !process.env.PODETELAR_TEST_SFU_URL,
    "Run through native:room-smoke for an isolated real SFU.",
  );
  const fixture = await voiceFixture();
  const { app, config, origin, guild, a, b, user, credential, voice } = fixture;
  const contexts: BrowserContext[] = [];
  try {
    await fixture.listen();
    const headers = { origin, authorization: `Bearer ${credential.token}` };
    const admit = () =>
      app.inject({
        method: "POST",
        url: "/api/auth/voice/join",
        headers,
        payload: {
          guildId: guild,
          channelId: a,
          deviceId: `pc1_${"f".repeat(64)}`,
        },
      });
    const auto = await admit();
    expect(auto.statusCode).toBe(200);
    const room = auto.json().room.id,
      autoCookie = String(auto.headers["set-cookie"]).split(";")[0]!;
    const resolved = await app.inject({
      method: "POST",
      url: `/api/channels/${guild}/${a}/resolve`,
      headers: { origin },
      payload: { invitation: new URL(auto.json().invitationUrl).hash.slice(8) },
    });
    expect(resolved.statusCode).toBe(200);
    const invitation = new URL(resolved.json().invitationUrl).hash.slice(8);
    const guest = await app.inject({
      method: "POST",
      url: `/api/rooms/${room}/join`,
      headers: { origin },
      payload: { invitation, displayName: "Independent fixture" },
    });
    expect(guest.statusCode).toBe(200);
    const guestCookie = String(guest.headers["set-cookie"]).split(";")[0]!;
    const connect = async (cookie: string) => {
      const access = await app.inject({
        method: "POST",
        url: `/api/rooms/${room}/native-media-token`,
        headers: { origin, cookie },
      });
      expect(access.statusCode).toBe(200);
      const context = await browser.newContext({ bypassCSP: true });
      contexts.push(context);
      const page = await context.newPage();
      await page.goto(origin);
      await page.addScriptTag({
        content: readFileSync(
          new URL(
            "../node_modules/livekit-client/dist/livekit-client.umd.js",
            import.meta.url,
          ),
          "utf8",
        ),
      });
      await page.evaluate(async (credentials) => {
        const scope = window as unknown as {
          LivekitClient: { Room: new (options: object) => LiveRoom };
          fixtureRoom: LiveRoom;
        };
        scope.fixtureRoom = new scope.LivekitClient.Room({
          autoSubscribe: false,
        });
        await scope.fixtureRoom.connect(
          credentials.serverUrl,
          credentials.token,
        );
      }, access.json());
      return page;
    };
    const controlled = await connect(autoCookie),
      independent = await connect(guestCookie);
    const connection = (page: typeof controlled) =>
      page.evaluate(
        () =>
          (window as unknown as { fixtureRoom: LiveRoom }).fixtureRoom.state,
      );
    voice.index.disconnected();
    await new Promise((resolve) => setTimeout(resolve, 500));
    expect(await connection(controlled)).toBe("connected");
    voice.index.connected();
    voice.index.apply("VOICE_STATE_UPDATE", {
      guild_id: guild,
      user_id: user,
      channel_id: b,
    });
    const directory = new LivekitDirectory(config);
    await expect
      .poll(async () => (await directory.members(room)).map((p) => p.id), {
        timeout: 10000,
      })
      .toEqual([guest.json().participant.id]);
    await expect
      .poll(() => connection(controlled), { timeout: 5000 })
      .not.toBe("connected");
    expect(await connection(independent)).toBe("connected");
    expect(
      (
        await app.inject({
          method: "POST",
          url: `/api/rooms/${room}/native-media-token`,
          headers: { origin, cookie: autoCookie },
        })
      ).statusCode,
    ).toBe(401);
  } finally {
    await Promise.all(contexts.map((context) => context.close()));
    await fixture.close();
  }
});
