import assert from "node:assert/strict";
import { mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import { test, type TestContext } from "node:test";
import { AccessToken } from "livekit-server-sdk";
import { buildApp } from "../src/app.js";
import { loadConfig } from "../src/config.js";
import { Rooms, SESSION_TTL_MS } from "../src/rooms.js";
import { MediaUnavailable } from "../src/media.js";

const origin = "http://localhost:5173";
const key = "test-server-access-key-not-for-production";
const secret = "test-media-signing-secret-not-for-production";
async function fixture(t: TestContext) {
  const path = mkdtempSync(join(tmpdir(), "freedomstream-access-"));
  const config = loadConfig({
    PUBLIC_URL: origin,
    ADMIN_TOKEN: key,
    LIVEKIT_API_KEY: "test-key",
    LIVEKIT_API_SECRET: secret,
    DATABASE_PATH: join(path, "rooms.db"),
  });
  config.webRoot = join(path, "no-assets");
  let now = Date.now();
  let unavailable = false;
  const removed: string[] = [];
  const mediaDirectory = {
    async participant() {
      return null;
    },
    async attribute() {},
    async remove(_room: string, identity: string) {
      if (unavailable) throw new MediaUnavailable();
      removed.push(identity);
    },
  };
  let app = await buildApp(config, { now: () => now, mediaDirectory });
  t.after(async () => {
    await app.close();
    const resolved = realpathSync(path);
    assert.equal(
      dirname(resolved).toLowerCase(),
      realpathSync(tmpdir()).toLowerCase(),
    );
    assert.ok(basename(resolved).startsWith("freedomstream-access-"));
    rmSync(resolved, { recursive: true });
  });
  const admin = { origin, authorization: `Bearer ${key}` };
  const created = await app.inject({
    method: "POST",
    url: "/api/rooms",
    headers: admin,
    payload: { name: "Friends", displayName: "Operator" },
  });
  assert.equal(created.statusCode, 201);
  const room = created.json().room.id as string;
  const invitation = new URL(created.json().invitationUrl).hash.slice(8);
  const joinFriend = async (invite = invitation) => {
    const response = await app.inject({
      method: "POST",
      url: `/api/rooms/${room}/join`,
      headers: { origin },
      payload: { invitation: invite, displayName: "Friend" },
    });
    return {
      response,
      cookie: String(response.headers["set-cookie"]).split(";")[0]!,
      identity: response.json().participant?.id as string,
    };
  };
  return {
    get app() {
      return app;
    },
    room,
    admin,
    invitation,
    removed,
    joinFriend,
    advance: (ms: number) => {
      now += ms;
    },
    unavailable: (value: boolean) => {
      unavailable = value;
    },
    restart: async () => {
      await app.close();
      app = await buildApp(config, { now: () => now, mediaDirectory });
    },
  };
}

test("rotation preserves sessions by choice and disables the old invitation durably", async (t) => {
  const f = await fixture(t);
  const friend = await f.joinFriend();
  const denied = await f.app.inject({
    method: "POST",
    url: `/api/rooms/${f.room}/access`,
    headers: { origin, cookie: friend.cookie },
    payload: { invitation: "rotate", revokeSessions: true },
  });
  assert.equal(denied.statusCode, 401);
  const changed = await f.app.inject({
    method: "POST",
    url: `/api/rooms/${f.room}/access`,
    headers: f.admin,
    payload: { invitation: "rotate", revokeSessions: false },
  });
  assert.equal(changed.statusCode, 200);
  const next = new URL(changed.json().invitationUrl).hash.slice(8);
  assert.notEqual(next, f.invitation);
  await f.restart();
  assert.equal((await f.joinFriend()).response.statusCode, 404);
  assert.equal((await f.joinFriend(next)).response.statusCode, 200);
  assert.equal(
    (
      await f.app.inject({
        method: "GET",
        url: `/api/rooms/${f.room}/session`,
        headers: { cookie: friend.cookie },
      })
    ).statusCode,
    200,
  );
  const disabled = await f.app.inject({
    method: "POST",
    url: `/api/rooms/${f.room}/access`,
    headers: f.admin,
    payload: { invitation: "disable", revokeSessions: true },
  });
  assert.equal(disabled.statusCode, 200);
  assert.ok(f.removed.includes(friend.identity));
  for (const [method, suffix] of [
    ["GET", "session"],
    ["POST", "media-token"],
    ["GET", `previews/${friend.identity}/screen/${"a".repeat(22)}`],
  ] as const)
    assert.equal(
      (
        await f.app.inject({
          method,
          url: `/api/rooms/${f.room}/${suffix}`,
          headers: { origin, cookie: friend.cookie },
        })
      ).statusCode,
      401,
    );
  assert.equal((await f.joinFriend(next)).response.statusCode, 404);
});

test("revoked token replay and refreshed tokens are denied; an unrelated session stays admitted", async (t) => {
  const f = await fixture(t);
  const friend = await f.joinFriend();
  const other = await f.joinFriend();
  const access = await f.app.inject({
    method: "POST",
    url: `/api/rooms/${f.room}/media-token`,
    headers: { origin, cookie: friend.cookie },
  });
  assert.equal(access.statusCode, 200);
  const validate = (token: string) =>
    f.app.inject({
      method: "GET",
      url: `/media/rtc/v1/validate?access_token=${encodeURIComponent(token)}`,
    });
  assert.equal((await validate(access.json().token)).statusCode, 200);
  const revoked = await f.app.inject({
    method: "DELETE",
    url: `/api/rooms/${f.room}/access/sessions/${friend.identity}`,
    headers: f.admin,
  });
  assert.equal(revoked.statusCode, 200);
  assert.equal((await validate(access.json().token)).statusCode, 401);
  const refreshed = new AccessToken("test-key", secret, {
    identity: friend.identity,
    ttl: 3600,
  });
  refreshed.addGrant({ roomJoin: true, room: `freedomstream_${f.room}` });
  assert.equal((await validate(await refreshed.toJwt())).statusCode, 401);
  const remaining = await f.app.inject({
    method: "POST",
    url: `/api/rooms/${f.room}/media-token`,
    headers: { origin, cookie: other.cookie },
  });
  assert.equal((await validate(remaining.json().token)).statusCode, 200);
  for (const path of [
    "/media/twirp/livekit.RoomService/ListRooms",
    "/media/rtc/v1/../twirp/admin",
  ])
    assert.equal(
      (await f.app.inject({ method: "GET", url: path })).statusCode,
      404,
    );
});

test("revocation fails closed immediately and retries media enforcement after restart", async (t) => {
  const f = await fixture(t);
  const friend = await f.joinFriend();
  f.unavailable(true);
  const response = await f.app.inject({
    method: "DELETE",
    url: `/api/rooms/${f.room}/access/sessions/${friend.identity}`,
    headers: f.admin,
  });
  assert.equal(response.statusCode, 202);
  assert.ok(response.json().pendingDisconnects > 0);
  assert.equal(
    (
      await f.app.inject({
        method: "POST",
        url: `/api/rooms/${f.room}/media-token`,
        headers: { origin, cookie: friend.cookie },
      })
    ).statusCode,
    401,
  );
  await f.restart();
  f.unavailable(false);
  await f.app.close();
  assert.ok(f.removed.includes(friend.identity));
});

test("expired sessions queue media removal instead of silently pruning authority", () => {
  let now = Date.now();
  const rooms = new Rooms(":memory:", () => now);
  try {
    const issued = rooms.create({ name: "Friends", displayName: "Publisher" });
    now += SESSION_TTL_MS;
    assert.equal(
      rooms.active(issued.snapshot.room.id, issued.snapshot.participant.id),
      false,
    );
    rooms.expire();
    assert.equal(rooms.removals()[0]?.identity, issued.snapshot.participant.id);
    assert.equal(rooms.pending(issued.snapshot.room.id), 1);
  } finally {
    rooms.close();
  }
});
