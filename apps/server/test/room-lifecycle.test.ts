import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { Rooms, EMPTY_ROOM_GRACE_MS } from "../src/rooms.js";
import { RoomLifecycle } from "../src/room-lifecycle.js";
import { Access } from "../src/access.js";
import { Previews } from "../src/previews.js";
import { buildApp } from "../src/app.js";
import { loadConfig } from "../src/config.js";

const key = "test-administrator-invitation-key-not-production";
test("an older empty presence response cannot close a same-session signaling reconnect", async (t) => {
  const dir = mkdtempSync(join(tmpdir(), "freedomstream-presence-race-"));
  let now = 0;
  const rooms = new Rooms(join(dir, "rooms.db"), () => now, key);
  t.after(() => {
    rooms.close();
    rmSync(dir, { recursive: true });
  });
  const created = rooms.create({ name: "Friends", displayName: "Ana" });
  const id = created.snapshot.room.id;
  rooms.observePresence(id, false);
  let respond!: (members: { id: string; displayName: string }[]) => void;
  const media = {
    members: () =>
      new Promise<{ id: string; displayName: string }[]>((resolve) => {
        respond = resolve;
      }),
    async remove() {},
    async participant() {
      return null;
    },
    async attribute() {},
  };
  const access = new Access(
    rooms,
    media,
    new Previews(media, () => now),
    () => {},
  );
  const lifecycle = new RoomLifecycle(rooms, media, access);
  now = 58000;
  const oldRead = lifecycle.describePage(rooms.list(0).rooms);
  now = 59000;
  rooms.signalingConnected(id); // Successful SDK reconnect, without a new POST /join.
  now = 60000;
  respond([]); // An old snapshot taken before that reconnect.
  assert.equal((await oldRead).length, 1);
  assert.ok(rooms.session(id, created.token));
  assert.equal(rooms.acceptsInvitation(id, created.invitation), true);
  assert.equal(rooms.lifecycle(id)!.emptySince, 60000);
  const currentRead = lifecycle.describePage(rooms.list(0).rooms);
  respond([{ id: created.snapshot.participant.id, displayName: "Ana" }]);
  assert.equal((await currentRead)[0]!.presence, "active");
  assert.equal(rooms.lifecycle(id)!.emptySince, null);
});
test("background presence cleanup preserves pending rooms, reconnects and outages, then deletes all room authority", async (t) => {
  const dir = mkdtempSync(join(tmpdir(), "freedomstream-lifecycle-"));
  let now = 100000;
  const rooms = new Rooms(join(dir, "rooms.db"), () => now, key);
  let members: { id: string; displayName: string }[] = [];
  let fail = false;
  const removed: string[] = [];
  const media = {
    async members() {
      if (fail) throw new Error("private service detail");
      return members;
    },
    async remove(_room: string, identity: string) {
      removed.push(identity);
    },
    async participant() {
      return null;
    },
    async attribute() {},
  };
  const access = new Access(
    rooms,
    media,
    new Previews(media, () => now),
    () => {},
  );
  const lifecycle = new RoomLifecycle(rooms, media, access);
  t.after(() => {
    rooms.close();
    rmSync(dir, { recursive: true });
  });
  const created = rooms.createManaged("Friends");
  now += EMPTY_ROOM_GRACE_MS * 10;
  await lifecycle.tick(); // No dashboard is involved.
  assert.equal(rooms.list(0).rooms.length, 1);
  const session = rooms.join(created.room.id, created.invitation, "Felipe")!;
  members = [
    { id: session.snapshot.participant.id, displayName: "Felipe" },
    { id: "stale", displayName: "Stale" },
  ];
  let page = await lifecycle.describePage(rooms.list(0).rooms);
  assert.deepEqual(page[0]!.members, [members[0]]);
  assert.equal(page[0]!.presence, "active");
  members = [];
  await lifecycle.tick();
  now += EMPTY_ROOM_GRACE_MS - 1;
  await lifecycle.tick();
  assert.ok(rooms.session(created.room.id, session.token));
  members = [{ id: session.snapshot.participant.id, displayName: "Felipe" }];
  await lifecycle.tick();
  now += EMPTY_ROOM_GRACE_MS * 2;
  await lifecycle.tick();
  assert.equal(
    rooms.list(0).rooms.length,
    1,
    "connected idle members keep the room open",
  );
  members = [];
  await lifecycle.tick();
  now += EMPTY_ROOM_GRACE_MS;
  fail = true;
  await lifecycle.tick();
  assert.equal(rooms.list(0).rooms.length, 1, "outage cannot close a room");
  fail = false;
  await lifecycle.tick(); // Begin a fresh continuous observation after outage.
  now += EMPTY_ROOM_GRACE_MS - 1;
  await lifecycle.tick();
  assert.equal(rooms.list(0).rooms.length, 1);
  now++;
  await lifecycle.tick();
  assert.equal(rooms.list(0).rooms.length, 0);
  assert.equal(rooms.session(created.room.id, session.token), null);
  assert.equal(
    rooms.acceptsInvitation(created.room.id, created.invitation),
    false,
  );
  assert.equal(
    rooms.active(created.room.id, session.snapshot.participant.id),
    false,
  );
  assert.ok(removed.includes(session.snapshot.participant.id));
  assert.equal(rooms.closeRoom(created.room.id), false);
});

test("encrypted invitations survive restart, bind to a room and follow rotation; legacy and changed keys need explicit replacement", (t) => {
  const dir = mkdtempSync(join(tmpdir(), "freedomstream-invite-cipher-"));
  const path = join(dir, "rooms.db");
  let rooms = new Rooms(path, Date.now, key);
  t.after(() => {
    rooms.close();
    rmSync(dir, { recursive: true });
  });
  const one = rooms.createManaged("One");
  const two = rooms.createManaged("Two");
  assert.equal(rooms.invitation(one.room.id), one.invitation);
  const db = new DatabaseSync(path);
  const cipher = db
    .prepare("SELECT invitation_cipher FROM room_lifecycle WHERE room_id = ?")
    .get(one.room.id)!.invitation_cipher as Uint8Array;
  assert.equal(
    Buffer.from(cipher).includes(Buffer.from(one.invitation)),
    false,
  );
  db.prepare(
    "UPDATE room_lifecycle SET invitation_cipher = ? WHERE room_id = ?",
  ).run(cipher, two.room.id);
  assert.equal(
    rooms.invitation(two.room.id),
    null,
    "cannot transplant ciphertext to another room",
  );
  const rotated = rooms.change(one.room.id, {
    invitation: "rotate",
    revokeSessions: false,
  })!;
  assert.notEqual(rotated.invitation, one.invitation);
  assert.equal(rooms.acceptsInvitation(one.room.id, one.invitation), false);
  assert.equal(rooms.invitation(one.room.id), rotated.invitation);
  rooms.close();
  rooms = new Rooms(path, Date.now, key);
  assert.equal(rooms.invitation(one.room.id), rotated.invitation);
  rooms.close();
  rooms = new Rooms(path, Date.now, "another-administrator-key-not-production");
  assert.equal(rooms.invitation(one.room.id), null);
  assert.equal(rooms.acceptsInvitation(one.room.id, rotated.invitation!), true);
  const replaced = rooms.change(one.room.id, {
    invitation: "rotate",
    revokeSessions: false,
  })!;
  assert.equal(rooms.invitation(one.room.id), replaced.invitation);
  rooms.change(one.room.id, { invitation: "disable", revokeSessions: false });
  assert.equal(rooms.invitation(one.room.id), null);
  db.close();
});

test("restart resets an unobserved empty interval and schema 3 migration preserves authority", (t) => {
  const dir = mkdtempSync(join(tmpdir(), "freedomstream-lifecycle-migration-"));
  const path = join(dir, "rooms.db");
  let now = 100000;
  let rooms = new Rooms(path, () => now);
  t.after(() => {
    rooms.close();
    rmSync(dir, { recursive: true });
  });
  const created = rooms.create({ name: "Legacy", displayName: "Friend" });
  rooms.observePresence(created.snapshot.room.id, false);
  rooms.close();
  const db = new DatabaseSync(path);
  db.exec("DROP TABLE room_lifecycle; PRAGMA user_version = 3");
  db.close();
  rooms = new Rooms(path, () => now, key);
  assert.ok(rooms.session(created.snapshot.room.id, created.token));
  assert.equal(rooms.invitation(created.snapshot.room.id), null);
  assert.equal(rooms.lifecycle(created.snapshot.room.id)!.joined, true);
  rooms.observePresence(created.snapshot.room.id, false);
  rooms.close();
  now += EMPTY_ROOM_GRACE_MS * 5;
  rooms = new Rooms(path, () => now, key);
  assert.equal(rooms.observePresence(created.snapshot.room.id, false), false);
  now += EMPTY_ROOM_GRACE_MS;
  assert.equal(rooms.observePresence(created.snapshot.room.id, false), true);
});

test("admin invitation and close endpoints require admin and origin, retain invites on reload and revoke room access", async (t) => {
  const dir = mkdtempSync(join(tmpdir(), "freedomstream-admin-actions-"));
  const origin = "http://localhost:5173";
  const config = loadConfig({
    PUBLIC_URL: origin,
    ADMIN_TOKEN: key,
    LIVEKIT_API_KEY: "test-key",
    LIVEKIT_API_SECRET: key,
    DATABASE_PATH: join(dir, "rooms.db"),
  });
  config.webRoot = join(dir, "no-web");
  let removalFails = true;
  const app = await buildApp(config, {
    usagePolling: false,
    mediaDirectory: {
      async members() {
        return [];
      },
      async remove() {
        if (removalFails) throw new Error("private SFU detail");
      },
      async participant() {
        return null;
      },
      async attribute() {},
    },
  });
  t.after(async () => {
    await app.close();
    rmSync(dir, { recursive: true });
  });
  const login = await app.inject({
    method: "POST",
    url: "/api/admin/login",
    headers: { origin },
    payload: { password: key },
  });
  const cookie = String(login.headers["set-cookie"]).split(";")[0]!;
  const created = (
    await app.inject({
      method: "POST",
      url: "/api/admin/rooms",
      headers: { origin, cookie },
      payload: { name: "Friends" },
    })
  ).json();
  const id = created.room.id;
  const invitation = new URL(created.invitationUrl).hash.slice(8);
  const joined = await app.inject({
    method: "POST",
    url: `/api/rooms/${id}/join`,
    headers: { origin },
    payload: { invitation, displayName: "Friend" },
  });
  const memberCookie = String(joined.headers["set-cookie"]).split(";")[0]!;
  for (const [method, url] of [
    ["POST", `/api/admin/rooms/${id}/invitation`],
    ["POST", `/api/admin/rooms/${id}/invitation/rotate`],
    ["DELETE", `/api/admin/rooms/${id}`],
  ] as const) {
    for (const credentials of [
      {},
      { cookie: memberCookie },
      { authorization: `Bearer ${key}` },
    ])
      assert.equal(
        (await app.inject({ method, url, headers: { origin, ...credentials } }))
          .statusCode,
        401,
      );
    for (const badOrigin of [undefined, "https://evil.example"])
      assert.equal(
        (
          await app.inject({
            method,
            url,
            headers: { cookie, ...(badOrigin ? { origin: badOrigin } : {}) },
          })
        ).statusCode,
        403,
      );
  }
  for (let i = 0; i < 2; i++) {
    const retrieved = await app.inject({
      method: "POST",
      url: `/api/admin/rooms/${id}/invitation`,
      headers: { origin, cookie },
    });
    assert.equal(retrieved.json().invitationUrl, created.invitationUrl);
    assert.equal(retrieved.headers["cache-control"], "no-store");
  }
  const changed = await app.inject({
    method: "POST",
    url: `/api/admin/rooms/${id}/invitation/rotate`,
    headers: { origin, cookie },
  });
  assert.equal(changed.statusCode, 200);
  assert.notEqual(changed.json().invitationUrl, created.invitationUrl);
  assert.equal(
    (
      await app.inject({
        url: `/api/rooms/${id}/session`,
        headers: { cookie: memberCookie },
      })
    ).statusCode,
    200,
  );
  const closed = await app.inject({
    method: "DELETE",
    url: `/api/admin/rooms/${id}`,
    headers: { origin, cookie },
  });
  assert.equal(closed.statusCode, 202);
  assert.ok(closed.json().pendingDisconnects > 0);
  assert.deepEqual(
    (await app.inject({ url: "/api/admin/rooms", headers: { cookie } })).json()
      .rooms,
    [],
  );
  assert.equal(
    (
      await app.inject({
        method: "POST",
        url: `/api/admin/rooms/${id}/invitation`,
        headers: { origin, cookie },
      })
    ).statusCode,
    404,
  );
  assert.equal(
    (
      await app.inject({
        url: `/api/rooms/${id}/session`,
        headers: { cookie: memberCookie },
      })
    ).statusCode,
    401,
  );
  assert.equal(
    (
      await app.inject({
        method: "POST",
        url: `/api/rooms/${id}/native-media-token`,
        headers: { origin, cookie: memberCookie },
      })
    ).statusCode,
    401,
  );
  const newInvitation = new URL(changed.json().invitationUrl).hash.slice(8);
  assert.equal(
    (
      await app.inject({
        method: "POST",
        url: `/api/rooms/${id}/join`,
        headers: { origin },
        payload: { invitation: newInvitation, displayName: "Friend" },
      })
    ).statusCode,
    404,
  );
  removalFails = false;
});
