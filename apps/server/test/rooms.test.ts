import assert from "node:assert/strict";
import { mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { test } from "node:test";
import type { TestContext } from "node:test";
import { TokenVerifier } from "livekit-server-sdk";
import { buildApp } from "../src/app.js";
import { loadConfig } from "../src/config.js";
import { Rooms, SESSION_TTL_MS } from "../src/rooms.js";

const ORIGIN = "http://localhost:5173";
const ADMIN = "test-admin-key-only-not-a-production-secret";
const MEDIA_SECRET = "test-livekit-secret-only-not-for-production";
const cookieFrom = (response: { headers: Record<string, unknown> }) =>
  String(response.headers["set-cookie"]).split(";")[0]!;

async function fixture(t: TestContext) {
  const directory = mkdtempSync(join(tmpdir(), "freedomstream-"));
  const config = loadConfig({
    PUBLIC_URL: ORIGIN,
    ADMIN_TOKEN: ADMIN,
    LIVEKIT_API_KEY: "test-api-key",
    LIVEKIT_API_SECRET: MEDIA_SECRET,
    DATABASE_PATH: join(directory, "rooms.db"),
  });
  // No static assets in API tests; the Docker smoke check exercises the built app.
  config.webRoot = join(directory, "no-web-build");
  let now = Date.now();
  const mediaDirectory = {
    async members() {
      return [];
    },
    async remove() {},
    async participant() {
      return null;
    },
    async attribute() {},
  };
  let app = await buildApp(config, { now: () => now, mediaDirectory });
  await app.ready();
  t.after(async () => {
    await app.close();
    const resolved = realpathSync(directory);
    if (
      dirname(resolved).toLowerCase() !==
        realpathSync(tmpdir()).toLowerCase() ||
      !basename(resolved).startsWith("freedomstream-")
    )
      throw new Error("Refusing unexpected cleanup path");
    rmSync(resolved, { recursive: true });
  });
  return {
    get app() {
      return app;
    },
    config,
    advance: (milliseconds: number) => {
      now += milliseconds;
    },
    restart: async () => {
      await app.close();
      app = await buildApp(config, { now: () => now, mediaDirectory });
      await app.ready();
    },
    create: async (name = "Friends") => {
      const response = await app.inject({
        method: "POST",
        url: "/api/rooms",
        headers: { origin: ORIGIN, authorization: `Bearer ${ADMIN}` },
        payload: { name, displayName: "Felipe" },
      });
      assert.equal(response.statusCode, 201, response.body);
      const body = response.json();
      return {
        body,
        cookie: cookieFrom(response),
        invitation: new URL(body.invitationUrl).hash.slice("#invite=".length),
        response,
      };
    },
  };
}

test("document/static requests do not spend the API quota, while API throttling stays enforced", async (t) => {
  const f = await fixture(t);
  for (let i = 0; i < 150; i++)
    assert.equal((await f.app.inject("/assets/client.js")).statusCode, 404);
  for (let i = 0; i < 120; i++)
    assert.equal((await f.app.inject("/api/health")).statusCode, 200);
  assert.equal((await f.app.inject("/api/health")).statusCode, 429);
});

test("room creation requires the server key and validates input", async (t) => {
  const f = await fixture(t);
  const noKey = await f.app.inject({
    method: "POST",
    url: "/api/rooms",
    headers: { origin: ORIGIN },
    payload: { name: "Friends", displayName: "Someone" },
  });
  assert.equal(noKey.statusCode, 401);
  const wrongKey = await f.app.inject({
    method: "POST",
    url: "/api/rooms",
    headers: { origin: ORIGIN, authorization: "Bearer wrong" },
    payload: { name: "Friends", displayName: "Someone" },
  });
  assert.equal(wrongKey.statusCode, 401);
  const invalid = await f.app.inject({
    method: "POST",
    url: "/api/rooms",
    headers: { origin: ORIGIN, authorization: `Bearer ${ADMIN}` },
    payload: { name: "   ", displayName: "Someone" },
  });
  assert.equal(invalid.statusCode, 400);
  const { response, body } = await f.create();
  assert.match(body.room.id, /^[A-Za-z0-9_-]{22}$/);
  assert.match(String(response.headers["set-cookie"]), /HttpOnly/);
  assert.match(String(response.headers["set-cookie"]), /SameSite=Strict/);
  assert.match(
    String(response.headers["set-cookie"]),
    new RegExp(`Path=/api/rooms/${body.room.id}`),
  );
  assert.equal(response.headers["cache-control"], "no-store");
  assert.equal(response.headers["referrer-policy"], "no-referrer");
});

test("unauthenticated users cannot discover room details or obtain media access", async (t) => {
  const f = await fixture(t);
  const { body } = await f.create("A private name");
  for (const suffix of ["session", "media-token"]) {
    const response = await f.app.inject({
      method: suffix === "session" ? "GET" : "POST",
      url: `/api/rooms/${body.room.id}/${suffix}`,
      headers: { origin: ORIGIN },
    });
    assert.equal(response.statusCode, 401);
    assert.ok(!response.body.includes("A private name"));
  }
  assert.equal(
    (await f.app.inject({ method: "GET", url: "/api/rooms" })).statusCode,
    404,
  );
});

test("invitations admit only to their room, with indistinguishable unknown/wrong-secret errors", async (t) => {
  const f = await fixture(t);
  const first = await f.create();
  const second = await f.create("Other room");
  const admitted = await f.app.inject({
    method: "POST",
    url: `/api/rooms/${first.body.room.id}/join`,
    headers: { origin: ORIGIN },
    payload: { invitation: first.invitation, displayName: "Friend" },
  });
  assert.equal(admitted.statusCode, 200);
  assert.equal(admitted.json().participant.displayName, "Friend");
  const wrongSecret = await f.app.inject({
    method: "POST",
    url: `/api/rooms/${second.body.room.id}/join`,
    headers: { origin: ORIGIN },
    payload: { invitation: first.invitation, displayName: "Friend" },
  });
  const unknown = await f.app.inject({
    method: "POST",
    url: `/api/rooms/${"x".repeat(22)}/join`,
    headers: { origin: ORIGIN },
    payload: { invitation: first.invitation, displayName: "Friend" },
  });
  assert.equal(wrongSecret.statusCode, 404);
  assert.equal(unknown.statusCode, 404);
  assert.equal(unknown.body, wrongSecret.body);
  const wrongRoom = await f.app.inject({
    method: "GET",
    url: `/api/rooms/${second.body.room.id}/session`,
    headers: { cookie: cookieFrom(admitted) },
  });
  assert.equal(wrongRoom.statusCode, 401);
});

test("room and session authority persist across restart without storing raw secrets", async (t) => {
  const f = await fixture(t);
  const created = await f.create();
  const database = new DatabaseSync(f.config.databasePath, { readOnly: true });
  const room = database.prepare("SELECT invitation_hash FROM rooms").get() as {
    invitation_hash: Uint8Array;
  };
  const session = database.prepare("SELECT token_hash FROM sessions").get() as {
    token_hash: Uint8Array;
  };
  assert.equal(room.invitation_hash.length, 32);
  assert.equal(session.token_hash.length, 32);
  assert.notEqual(
    Buffer.from(room.invitation_hash).toString(),
    created.invitation,
  );
  assert.notEqual(
    Buffer.from(session.token_hash).toString(),
    created.cookie.split("=")[1],
  );
  database.close();
  await f.restart();
  const resumed = await f.app.inject({
    method: "GET",
    url: `/api/rooms/${created.body.room.id}/session`,
    headers: { cookie: created.cookie },
  });
  assert.equal(resumed.statusCode, 200);
  assert.deepEqual(resumed.json(), {
    room: created.body.room,
    participant: created.body.participant,
    expiresAt: created.body.expiresAt,
  });
  const joined = await f.app.inject({
    method: "POST",
    url: `/api/rooms/${created.body.room.id}/join`,
    headers: { origin: ORIGIN },
    payload: {
      invitation: created.invitation,
      displayName: "Returning friend",
    },
  });
  assert.equal(joined.statusCode, 200);
});

test("version-one databases migrate without replacing existing room/session authority", async (t) => {
  const f = await fixture(t);
  const created = await f.create();
  await f.app.close();
  const legacy = new DatabaseSync(f.config.databasePath);
  legacy.exec(
    "DROP TABLE removals; DROP INDEX sessions_identity; ALTER TABLE rooms DROP COLUMN invitation_enabled; PRAGMA user_version = 1;",
  );
  legacy.close();
  await f.restart();
  const resumed = await f.app.inject({
    method: "GET",
    url: `/api/rooms/${created.body.room.id}/session`,
    headers: { cookie: created.cookie },
  });
  assert.equal(resumed.statusCode, 200);
  assert.deepEqual(resumed.json().participant, created.body.participant);
  const changed = await f.app.inject({
    method: "POST",
    url: `/api/rooms/${created.body.room.id}/access`,
    headers: { origin: ORIGIN, authorization: `Bearer ${ADMIN}` },
    payload: { invitation: "disable", revokeSessions: true },
  });
  assert.equal(changed.statusCode, 200);
  assert.equal(
    (
      await f.app.inject({
        method: "GET",
        url: `/api/rooms/${created.body.room.id}/session`,
        headers: { cookie: created.cookie },
      })
    ).statusCode,
    401,
  );
});

test("migration failure rolls back partial DDL and restarts without losing existing authority", async (t) => {
  const f = await fixture(t);
  const created = await f.create();
  await f.app.close();
  let database = new DatabaseSync(f.config.databasePath);
  database.exec(
    "DROP TABLE removals; DROP INDEX sessions_identity; ALTER TABLE rooms DROP COLUMN invitation_enabled; CREATE TABLE removals(blocking INTEGER); PRAGMA user_version = 1;",
  );
  database.close();
  assert.throws(() => new Rooms(f.config.databasePath), /already exists/);
  database = new DatabaseSync(f.config.databasePath);
  assert.equal(
    (database.prepare("PRAGMA user_version").get() as { user_version: number })
      .user_version,
    1,
  );
  assert.equal(
    (
      database.prepare("PRAGMA table_info(rooms)").all() as { name: string }[]
    ).some((column) => column.name === "invitation_enabled"),
    false,
  );
  database.exec("DROP TABLE removals");
  database.close();
  await f.restart();
  const resumed = await f.app.inject({
    url: `/api/rooms/${created.body.room.id}/session`,
    headers: { cookie: created.cookie },
  });
  assert.equal(resumed.statusCode, 200);
  assert.deepEqual(resumed.json().participant, created.body.participant);
});

test("expiry blocks API/media access while the persistent invitation can admit again", async (t) => {
  const f = await fixture(t);
  const created = await f.create();
  f.advance(SESSION_TTL_MS + 1);
  const expired = await f.app.inject({
    method: "POST",
    url: `/api/rooms/${created.body.room.id}/media-token`,
    headers: { origin: ORIGIN, cookie: created.cookie },
  });
  assert.equal(expired.statusCode, 401);
  const rejoined = await f.app.inject({
    method: "POST",
    url: `/api/rooms/${created.body.room.id}/join`,
    headers: { origin: ORIGIN },
    payload: { invitation: created.invitation, displayName: "Friend" },
  });
  assert.equal(rejoined.statusCode, 200);
});

test("leaving revokes the current session and rejoining replaces an old cookie", async (t) => {
  const f = await fixture(t);
  const created = await f.create();
  const rejoined = await f.app.inject({
    method: "POST",
    url: `/api/rooms/${created.body.room.id}/join`,
    headers: { origin: ORIGIN, cookie: created.cookie },
    payload: { invitation: created.invitation, displayName: "Felipe again" },
  });
  assert.equal(rejoined.statusCode, 200);
  assert.equal(
    (
      await f.app.inject({
        method: "GET",
        url: `/api/rooms/${created.body.room.id}/session`,
        headers: { cookie: created.cookie },
      })
    ).statusCode,
    401,
  );
  const cookie = cookieFrom(rejoined);
  const left = await f.app.inject({
    method: "POST",
    url: `/api/rooms/${created.body.room.id}/leave`,
    headers: { origin: ORIGIN, cookie },
  });
  assert.equal(left.statusCode, 204);
  assert.equal(
    (
      await f.app.inject({
        method: "GET",
        url: `/api/rooms/${created.body.room.id}/session`,
        headers: { cookie },
      })
    ).statusCode,
    401,
  );
});

test("media tokens have verified signatures, fixed room/identity and short lifetime", async (t) => {
  const f = await fixture(t);
  const created = await f.create();
  const response = await f.app.inject({
    method: "POST",
    url: `/api/rooms/${created.body.room.id}/native-media-token`,
    headers: { origin: ORIGIN, cookie: created.cookie },
    payload: { room: "someone-elses-room", identity: "forged" },
  });
  assert.equal(response.statusCode, 200);
  const body = response.json();
  const claims = await new TokenVerifier(
    f.config.livekitApiKey,
    MEDIA_SECRET,
  ).verify(body.token);
  assert.equal(claims.sub, created.body.participant.id);
  assert.equal(claims.video?.room, `freedomstream_${created.body.room.id}`);
  assert.equal(claims.video?.roomJoin, true);
  assert.equal(claims.video?.canPublish, true);
  assert.equal(claims.video?.canSubscribe, true);
  assert.equal(claims.video?.canPublishData, false);
  assert.equal(body.expiresInSeconds, 120);
  assert.ok(claims.exp! - claims.nbf! <= 120);
  assert.ok(!response.body.includes(MEDIA_SECRET));
});

test("cross-origin and missing-Origin mutations are denied even with valid authority", async (t) => {
  const f = await fixture(t);
  for (const origin of ["https://untrusted.example", undefined]) {
    const response = await f.app.inject({
      method: "POST",
      url: "/api/rooms",
      headers: {
        authorization: `Bearer ${ADMIN}`,
        ...(origin ? { origin } : {}),
      },
      payload: { name: "Friends", displayName: "Felipe" },
    });
    assert.equal(response.statusCode, 403);
  }
});

test("invitation guessing is rate limited and oversized bodies are rejected", async (t) => {
  const f = await fixture(t);
  const created = await f.create();
  let status = 0;
  for (let attempt = 0; attempt < 11; attempt++) {
    const response = await f.app.inject({
      method: "POST",
      url: `/api/rooms/${created.body.room.id}/join`,
      headers: { origin: ORIGIN },
      payload: { invitation: "x".repeat(43), displayName: "Guess" },
    });
    status = response.statusCode;
  }
  assert.equal(status, 429);
  const large = await f.app.inject({
    method: "POST",
    url: "/api/rooms",
    headers: { origin: ORIGIN, authorization: `Bearer ${ADMIN}` },
    payload: { name: "x".repeat(5000), displayName: "Felipe" },
  });
  assert.equal(large.statusCode, 413);
});

test("HTTPS room sessions use Secure cookies", async (t) => {
  const f = await fixture(t);
  f.config.publicUrl = "https://stream.example";
  await f.restart();
  const response = await f.app.inject({
    method: "POST",
    url: "/api/rooms",
    headers: { origin: f.config.publicUrl, authorization: `Bearer ${ADMIN}` },
    payload: { name: "Private", displayName: "Felipe" },
  });
  assert.equal(response.statusCode, 201);
  assert.match(String(response.headers["set-cookie"]), /; Secure/);
});

test("browser tokens cannot publish or subscribe and browser fetch cannot obtain native grants", async (t) => {
  const f = await fixture(t);
  const created = await f.create();
  const headers = { origin: ORIGIN, cookie: created.cookie };
  const browser = await f.app.inject({
    method: "POST",
    url: `/api/rooms/${created.body.room.id}/media-token`,
    headers,
  });
  assert.equal(browser.statusCode, 200);
  const claims = await new TokenVerifier(
    f.config.livekitApiKey,
    MEDIA_SECRET,
  ).verify(browser.json().token);
  assert.equal(claims.video?.canPublish, false);
  assert.equal(claims.video?.canSubscribe, false);
  assert.equal(claims.video?.canUpdateOwnMetadata, false);
  for (const metadata of [
    { "sec-fetch-site": "same-origin" },
    { "sec-fetch-mode": "cors" },
  ]) {
    const response = await f.app.inject({
      method: "POST",
      url: `/api/rooms/${created.body.room.id}/native-media-token`,
      headers: { ...headers, ...metadata },
    });
    assert.equal(response.statusCode, 403);
  }
});

test("app handoff tickets are private, single-use, expiring and revoked by invitation changes", async (t) => {
  const f = await fixture(t);
  const created = await f.create();
  const url = `/api/rooms/${created.body.room.id}/app-link`;
  const issue = async (invitation = created.invitation) =>
    f.app.inject({
      method: "POST",
      url,
      headers: { origin: ORIGIN },
      payload: { invitation },
    });
  const redeem = async (ticket: string, extra = {}) =>
    f.app.inject({
      method: "POST",
      url: "/api/app-links/redeem",
      headers: { origin: ORIGIN, ...extra },
      payload: { ticket },
    });
  assert.equal(
    (
      await f.app.inject({
        method: "POST",
        url,
        payload: { invitation: created.invitation },
      })
    ).statusCode,
    403,
  );
  assert.equal((await issue("x".repeat(43))).statusCode, 404);
  const issued = await issue();
  assert.equal(issued.statusCode, 200);
  assert.equal(issued.headers["cache-control"], "no-store");
  assert.ok(!issued.body.includes(created.invitation));
  const link = new URL(issued.json().appUrl);
  assert.equal(link.protocol, "freedomstream:");
  assert.equal(link.searchParams.get("server"), ORIGIN);
  const ticket = link.hash.slice("#handoff=".length);
  assert.equal(ticket.length, 43);
  assert.equal(
    (await redeem(ticket, { "sec-fetch-mode": "cors" })).statusCode,
    403,
  );
  const consumed = await redeem(ticket);
  assert.equal(consumed.statusCode, 200);
  assert.equal(consumed.json().invitationUrl, created.body.invitationUrl);
  assert.equal((await redeem(ticket)).statusCode, 404);
  const expiring = new URL((await issue()).json().appUrl).hash.slice(
    "#handoff=".length,
  );
  f.advance(60000);
  assert.equal((await redeem(expiring)).statusCode, 404);
  const revoked = new URL((await issue()).json().appUrl).hash.slice(
    "#handoff=".length,
  );
  await f.app.inject({
    method: "POST",
    url: `/api/rooms/${created.body.room.id}/access`,
    headers: { origin: ORIGIN, authorization: `Bearer ${ADMIN}` },
    payload: { invitation: "rotate", revokeSessions: false },
  });
  assert.equal((await redeem(revoked)).statusCode, 404);
  assert.equal((await issue()).statusCode, 404);
});

test("app handoff memory is bounded, expiry frees capacity and restart discards tickets", async (t) => {
  const f = await fixture(t);
  const created = await f.create();
  const issue = async (index: number) =>
    f.app.inject({
      method: "POST",
      url: `/api/rooms/${created.body.room.id}/app-link`,
      remoteAddress: `127.0.1.${index}`,
      headers: { origin: ORIGIN },
      payload: { invitation: created.invitation },
    });
  for (let i = 1; i <= 128; i++) assert.equal((await issue(i)).statusCode, 200);
  assert.equal((await issue(129)).statusCode, 503);
  f.advance(60001);
  const fresh = await issue(130);
  assert.equal(fresh.statusCode, 200);
  const ticket = new URL(fresh.json().appUrl).hash.slice("#handoff=".length);
  await f.restart();
  assert.equal(
    (
      await f.app.inject({
        method: "POST",
        url: "/api/app-links/redeem",
        headers: { origin: ORIGIN },
        payload: { ticket },
      })
    ).statusCode,
    404,
  );
});
