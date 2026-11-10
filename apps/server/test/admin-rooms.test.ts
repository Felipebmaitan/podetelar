import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { buildApp } from "../src/app.js";
import { loadConfig } from "../src/config.js";
import { Rooms } from "../src/rooms.js";

const password = "test-room-administrator-not-for-production";
const origin = "https://admin.example";
test("admin room directory and creation require current admin authority and persist private invitations", async () => {
  const dir = mkdtempSync(join(tmpdir(), "freedomstream-admin-rooms-"));
  const config = loadConfig({
    PUBLIC_URL: origin,
    ADMIN_TOKEN: password,
    LIVEKIT_API_KEY: "test-key",
    LIVEKIT_API_SECRET: "test-media-secret-not-used-for-production",
    DATABASE_PATH: join(dir, "rooms.db"),
  });
  config.webRoot = join(dir, "no-assets");
  let now = Date.now();
  let app = await buildApp(config, { now: () => now, usagePolling: false });
  const login = async () =>
    String(
      (
        await app.inject({
          method: "POST",
          url: "/api/admin/login",
          headers: { origin },
          payload: { password },
        })
      ).headers["set-cookie"],
    ).split(";")[0]!;
  const url = "/api/admin/rooms";
  try {
    const cookie = await login();
    const created = await app.inject({
      method: "POST",
      url,
      headers: { origin, cookie },
      payload: { name: " Friends " },
    });
    assert.equal(created.statusCode, 201);
    assert.equal(
      created.headers["set-cookie"],
      undefined,
      "creation must not admit admin to room",
    );
    const body = created.json();
    assert.equal(body.room.name, "Friends");
    const secret = new URL(body.invitationUrl).hash.slice("#invite=".length);
    assert.equal(secret.length, 43);
    const joined = await app.inject({
      method: "POST",
      url: `/api/rooms/${body.room.id}/join`,
      headers: { origin },
      payload: { invitation: secret, displayName: "Friend" },
    });
    assert.equal(joined.statusCode, 200);
    const participantCookie = String(joined.headers["set-cookie"]).split(
      ";",
    )[0]!;
    for (const headers of [
      {},
      { cookie: participantCookie },
      { authorization: `Bearer ${password}` },
    ]) {
      assert.equal((await app.inject({ url, headers })).statusCode, 401);
      assert.equal(
        (
          await app.inject({
            method: "POST",
            url,
            headers: { ...headers, origin },
            payload: { name: "Forbidden" },
          })
        ).statusCode,
        401,
      );
    }
    const invalid = await app.inject({
      method: "POST",
      url,
      headers: { origin, cookie },
      payload: { name: " " },
    });
    assert.equal(invalid.statusCode, 400);
    for (const badOrigin of [undefined, "https://evil.example"]) {
      assert.equal(
        (
          await app.inject({
            method: "POST",
            url,
            remoteAddress: "127.0.0.2",
            headers: { cookie, ...(badOrigin ? { origin: badOrigin } : {}) },
            payload: { name: "Forbidden" },
          })
        ).statusCode,
        403,
      );
    }
    const database = new DatabaseSync(config.databasePath, { readOnly: true });
    assert.equal(
      (
        database.prepare("SELECT COUNT(*) AS n FROM sessions").get() as {
          n: number;
        }
      ).n,
      1,
    );
    assert.equal(
      (
        database.prepare("SELECT invitation_hash FROM rooms").get() as {
          invitation_hash: Uint8Array;
        }
      ).invitation_hash.length,
      32,
    );
    database.close();
    const rooms = new Rooms(config.databasePath, () => now);
    try {
      for (let i = 0; i < 50; i++) rooms.createManaged(`Room ${i}`);
    } finally {
      rooms.close();
    }
    const listed = await app.inject({ url, headers: { cookie } });
    assert.equal(listed.statusCode, 200);
    assert.equal(listed.headers["cache-control"], "no-store");
    assert.equal(listed.json().rooms.length, 50);
    assert.equal(listed.json().nextOffset, 50);
    const last = (
      await app.inject({ url: `${url}?offset=50`, headers: { cookie } })
    ).json();
    assert.equal(last.rooms.length, 1);
    assert.equal(last.nextOffset, null);
    const all = [...listed.json().rooms, ...last.rooms];
    assert.equal(new Set(all.map((room) => room.id)).size, 51);
    assert.deepEqual(
      all.find((room) => room.id === body.room.id),
      { ...body.room, invitationEnabled: true },
    );
    assert.ok(!JSON.stringify(all).includes(secret));
    assert.deepEqual(Object.keys(all[0]).sort(), [
      "createdAt",
      "id",
      "invitationEnabled",
      "name",
    ]);
    for (const offset of ["-1", "1.2", "abc", "9999999999"])
      assert.equal(
        (
          await app.inject({
            url: `${url}?offset=${offset}`,
            headers: { cookie },
          })
        ).statusCode,
        400,
      );
    await app.inject({
      method: "POST",
      url: "/api/admin/logout",
      headers: { origin, cookie },
    });
    assert.equal(
      (await app.inject({ url, headers: { cookie } })).statusCode,
      401,
    );
    now += 60001;
    assert.equal(
      (
        await app.inject({
          method: "POST",
          url,
          remoteAddress: "127.0.0.3",
          headers: { origin, cookie },
          payload: { name: "Forbidden" },
        })
      ).statusCode,
      401,
    );
    const second = await login();
    now += 8 * 3600000 + 1;
    assert.equal(
      (await app.inject({ url, headers: { cookie: second } })).statusCode,
      401,
    );
    assert.equal(
      (
        await app.inject({
          method: "POST",
          url,
          remoteAddress: "127.0.0.3",
          headers: { origin, cookie: second },
          payload: { name: "Forbidden" },
        })
      ).statusCode,
      401,
    );
    const third = await login();
    await app.close();
    app = await buildApp(config, { now: () => now, usagePolling: false });
    assert.equal(
      (await app.inject({ url, headers: { cookie: third } })).statusCode,
      401,
    );
    const fourth = await login();
    const persisted = (
      await app.inject({ url, headers: { cookie: fourth } })
    ).json();
    assert.equal(persisted.rooms.length, 50);
    assert.equal(
      (
        await app.inject({
          method: "POST",
          url: `/api/rooms/${body.room.id}/join`,
          headers: { origin },
          payload: { invitation: secret, displayName: "Returning friend" },
        })
      ).statusCode,
      200,
    );
  } finally {
    await app.close();
    const resolved = realpathSync(dir);
    assert.equal(dirname(resolved), realpathSync(tmpdir()));
    assert.ok(basename(resolved).startsWith("freedomstream-admin-rooms-"));
    rmSync(resolved, { recursive: true });
  }
});
