import assert from "node:assert/strict";
import { test, type TestContext } from "node:test";
import { mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import sharp from "sharp";
import { buildApp } from "../src/app.js";
import { loadConfig } from "../src/config.js";
import { AVATAR_MAX_BYTES } from "@podetelar/contracts";
import { GuestProfiles } from "../src/profiles.js";
import { Rooms } from "../src/rooms.js";

const origin = "http://localhost:5173",
  password = "profile-test-admin-not-a-production-secret";
const device = (n: number) => `pc1_${n.toString(16).padStart(64, "0")}`;
async function fixture(t: TestContext) {
  const dir = mkdtempSync(join(tmpdir(), "podetelar-profiles-"));
  const config = loadConfig({
    PUBLIC_URL: origin,
    ADMIN_TOKEN: password,
    LIVEKIT_API_KEY: "profile-test-key",
    LIVEKIT_API_SECRET: "profile-test-media-not-a-production-secret",
    DATABASE_PATH: join(dir, "rooms.db"),
  });
  config.webRoot = join(dir, "no-web");
  const names = new Map<string, string>();
  let failNames = false;
  const build = () =>
    buildApp(config, {
      usagePolling: false,
      mediaDirectory: {
        async members() {
          return [];
        },
        async remove() {},
        async participant() {
          return null;
        },
        async attribute() {},
        async rename(_room, id, name) {
          if (failNames) throw Error("Unavailable");
          names.set(id, name);
        },
      },
    });
  let app = await build();
  t.after(async () => {
    await app.close();
    const path = realpathSync(dir);
    assert.equal(dirname(path), realpathSync(tmpdir()));
    assert.ok(basename(path).startsWith("podetelar-profiles-"));
    rmSync(path, { recursive: true });
  });
  const create = async (n: number) => {
    const result = await app.inject({
      method: "POST",
      url: "/api/rooms",
      headers: { origin, authorization: `Bearer ${password}` },
      payload: {
        name: "Profiles",
        displayName: `Guest ${n}`,
        deviceId: device(n),
      },
    });
    assert.equal(result.statusCode, 201, result.body);
    return {
      room: result.json().room.id as string,
      id: result.json().participant.id as string,
      invitation: new URL(result.json().invitationUrl).hash.slice(8),
      cookie: String(result.headers["set-cookie"]).split(";")[0]!,
    };
  };
  const admin = async () =>
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
  return {
    get app() {
      return app;
    },
    config,
    names,
    failNames(value: boolean) {
      failNames = value;
    },
    create,
    admin,
    async restart() {
      await app.close();
      app = await build();
    },
  };
}
test("guest pictures persist per claimed PC, keep IDs private, and enforce room/admin authority", async (t) => {
  const f = await fixture(t),
    first = await f.create(1),
    other = await f.create(2);
  const pixels = await sharp({
    create: { width: 5, height: 7, channels: 4, background: "red" },
  })
    .png()
    .toBuffer();
  const path = `/api/rooms/${first.room}/profile/avatar`;
  assert.equal(
    (
      await f.app.inject({
        method: "PUT",
        url: path,
        headers: { origin, "content-type": "application/octet-stream" },
        payload: pixels,
      })
    ).statusCode,
    401,
  );
  assert.equal(
    (
      await f.app.inject({
        method: "PUT",
        url: path,
        headers: {
          origin: "https://evil.example",
          cookie: first.cookie,
          "content-type": "application/octet-stream",
        },
        payload: pixels,
      })
    ).statusCode,
    403,
  );
  const uploaded = await f.app.inject({
    method: "PUT",
    url: path,
    headers: {
      origin,
      cookie: first.cookie,
      "content-type": "application/octet-stream",
    },
    payload: pixels,
  });
  assert.equal(uploaded.statusCode, 200, uploaded.body);
  assert.ok(!uploaded.body.includes("pc1_"));
  const revision = uploaded.json().profiles[0].avatarRevision;
  const imageUrl = `/api/rooms/${first.room}/profiles/${first.id}/avatar/${revision}`;
  const image = await f.app.inject({
    url: imageUrl,
    headers: { cookie: first.cookie },
  });
  assert.equal(image.statusCode, 200);
  assert.equal((await sharp(image.rawPayload).metadata()).width, 128);
  assert.equal(
    (await f.app.inject({ url: imageUrl, headers: { cookie: other.cookie } }))
      .statusCode,
    401,
  );
  assert.equal(
    (
      await f.app.inject({
        url: "/api/admin/profiles",
        headers: { cookie: first.cookie },
      })
    ).statusCode,
    401,
  );
  await f.restart();
  assert.equal(
    (await f.app.inject({ url: imageUrl, headers: { cookie: first.cookie } }))
      .statusCode,
    200,
  );
  const repeated = await f.create(1);
  const listed = (
    await f.app.inject({
      url: `/api/rooms/${repeated.room}/profiles`,
      headers: { cookie: repeated.cookie },
    })
  ).json();
  assert.equal(
    listed.profiles[0].avatarRevision,
    revision,
    "same claimed PC reuses the avatar across rooms",
  );
  const admin = await f.admin();
  const gallery = (
    await f.app.inject({
      url: "/api/admin/profiles",
      headers: { cookie: admin },
    })
  ).json();
  const profile = gallery.profiles.find(
    (p: { deviceId: string }) => p.deviceId === device(1),
  );
  assert.equal(profile.avatarRevision, revision);
  assert.equal(
    (
      await f.app.inject({
        method: "POST",
        url: `/api/admin/profiles/${profile.id}`,
        headers: { origin, cookie: admin },
        payload: { action: "delete" },
      })
    ).statusCode,
    204,
  );
  assert.equal(
    (await f.app.inject({ url: imageUrl, headers: { cookie: first.cookie } }))
      .statusCode,
    404,
  );
  assert.equal(
    (
      await f.app.inject({
        url: `/api/rooms/${repeated.room}/profiles`,
        headers: { cookie: repeated.cookie },
      })
    ).json().profiles[0].avatarRevision,
    null,
  );
  await f.app.inject({
    method: "POST",
    url: `/api/admin/profiles/${profile.id}`,
    headers: { origin, cookie: admin },
    payload: { action: "block" },
  });
  assert.equal(
    (
      await f.app.inject({
        method: "PUT",
        url: path,
        headers: {
          origin,
          cookie: first.cookie,
          "content-type": "application/octet-stream",
        },
        payload: pixels,
      })
    ).statusCode,
    403,
  );
  await f.restart();
  const admin2 = await f.admin();
  assert.equal(
    (
      await f.app.inject({
        url: "/api/admin/profiles",
        headers: { cookie: admin2 },
      })
    )
      .json()
      .profiles.find((p: { id: string }) => p.id === profile.id).uploadsBlocked,
    true,
  );
});
test("animated GIFs retain multiple frames; oversized, malformed, vector and excessive pixel inputs are refused", async (t) => {
  const f = await fixture(t),
    guest = await f.create(3),
    url = `/api/rooms/${guest.room}/profile/avatar`;
  const headers = {
    origin,
    cookie: guest.cookie,
    "content-type": "application/octet-stream",
  };
  const pixels = Buffer.from([255, 0, 0, 255, 0, 255, 0, 255]);
  const gif = await sharp(pixels, {
    raw: { width: 1, height: 2, channels: 4, pageHeight: 1 },
  })
    .gif({ delay: [100, 200], loop: 0 })
    .toBuffer();
  const result = await f.app.inject({
    method: "PUT",
    url,
    headers,
    payload: gif,
  });
  assert.equal(result.statusCode, 200, result.body);
  const revision = result.json().profiles[0].avatarRevision;
  const image = await f.app.inject({
    url: `/api/rooms/${guest.room}/profiles/${guest.id}/avatar/${revision}`,
    headers: { cookie: guest.cookie },
  });
  const meta = await sharp(image.rawPayload, { animated: true }).metadata();
  assert.equal(meta.pages, 2);
  assert.deepEqual(meta.delay, [100, 200]);
  for (const payload of [
    Buffer.from("GIF89a"),
    Buffer.from('<svg xmlns="http://www.w3.org/2000/svg"/>'),
    await sharp({
      create: { width: 4097, height: 1, channels: 3, background: "red" },
    })
      .png()
      .toBuffer(),
  ])
    assert.equal(
      (await f.app.inject({ method: "PUT", url, headers, payload })).statusCode,
      400,
    );
  assert.equal(
    (
      await f.app.inject({
        method: "PUT",
        url,
        headers,
        payload: Buffer.alloc(AVATAR_MAX_BYTES + 1),
      })
    ).statusCode,
    413,
  );
});
test("renaming updates session authority without replacing identity, retries media name changes, and validates input", async (t) => {
  const f = await fixture(t),
    guest = await f.create(4),
    url = `/api/rooms/${guest.room}/profile`;
  f.failNames(true);
  const renamed = await f.app.inject({
    method: "PATCH",
    url,
    headers: { origin, cookie: guest.cookie },
    payload: { displayName: "New name" },
  });
  assert.equal(renamed.statusCode, 200);
  const session = (
    await f.app.inject({
      url: `/api/rooms/${guest.room}/session`,
      headers: { cookie: guest.cookie },
    })
  ).json();
  assert.equal(session.participant.id, guest.id);
  assert.equal(session.participant.displayName, "New name");
  f.failNames(false);
  await f.app.inject({
    method: "PATCH",
    url,
    headers: { origin, cookie: guest.cookie },
    payload: { displayName: "Newest name" },
  });
  assert.equal(f.names.get(guest.id), "Newest name");
  for (const displayName of ["", "x".repeat(61), "bad\nname"]) {
    assert.equal(
      (
        await f.app.inject({
          method: "PATCH",
          url,
          headers: { origin, cookie: guest.cookie },
          payload: { displayName },
        })
      ).statusCode,
      400,
    );
  }
  await f.app.inject({
    method: "POST",
    url: `/api/rooms/${guest.room}/leave`,
    headers: { origin, cookie: guest.cookie },
  });
  assert.equal(
    (
      await f.app.inject({
        method: "PATCH",
        url,
        headers: { origin, cookie: guest.cookie },
        payload: { displayName: "Denied" },
      })
    ).statusCode,
    401,
  );
});

test("administrator deletion wins over an upload that is still decoding", async (t) => {
  const f = await fixture(t),
    guest = await f.create(8),
    store = new GuestProfiles(f.config.databasePath);
  try {
    const bytes = await sharp({
      create: { width: 100, height: 100, channels: 3, background: "blue" },
    })
      .png()
      .toBuffer();
    const profile = store.directory(0).profiles[0]!;
    const pending = store.upload(guest.room, guest.id, bytes);
    store.moderate(profile.id, "delete");
    assert.equal(await pending, "changed");
    assert.equal(store.directory(0).profiles[0]!.avatarRevision, null);
  } finally {
    await store.close();
  }
});

test("failed SFU renames yield retry slots to later participants", async (t) => {
  const f = await fixture(t),
    first = await f.create(11),
    rooms = new Rooms(f.config.databasePath);
  let now = Date.now();
  const store = new GuestProfiles(f.config.databasePath, () => now);
  try {
    const ids = Array.from(
      { length: 9 },
      (_, index) =>
        rooms.join(first.room, first.invitation, `Original ${index}`)!.snapshot
          .participant.id,
    );
    ids.forEach((id, index) =>
      store.rename(first.room, id, `Updated ${index}`),
    );
    const attempts: string[] = [];
    let failure = true;
    const media = {
      async members() {
        return [];
      },
      async remove() {},
      async participant() {
        return null;
      },
      async attribute() {},
      async rename(_room: string, id: string) {
        attempts.push(id);
        if (failure && ids.indexOf(id) < 8) throw Error("Offline participant");
      },
    };
    await store.syncNames(media);
    await store.syncNames(media);
    assert.equal(attempts.length, 9);
    assert.equal(attempts[8], ids[8]);
    failure = false;
    now += 5001;
    await store.syncNames(media);
    assert.equal(attempts.length, 17);
    await store.syncNames(media);
    assert.equal(
      attempts.length,
      17,
      "successfully synchronized updates are removed",
    );
  } finally {
    await store.close();
    rooms.close();
  }
});

test("authenticated profile polling has per-session budgets independent of other room requests", async (t) => {
  const f = await fixture(t),
    guest = await f.create(12);
  for (let index = 0; index < 120; index++)
    assert.equal(
      (
        await f.app.inject({
          url: `/api/rooms/${guest.room}/profiles`,
          headers: { cookie: guest.cookie },
        })
      ).statusCode,
      200,
    );
  assert.equal(
    (
      await f.app.inject({
        url: `/api/rooms/${guest.room}/profiles`,
        headers: { cookie: guest.cookie },
      })
    ).statusCode,
    429,
  );
  assert.equal(
    (
      await f.app.inject({
        url: `/api/rooms/${guest.room}/session`,
        headers: { cookie: guest.cookie },
      })
    ).statusCode,
    200,
  );
  const other = await f.app.inject({
    method: "POST",
    url: `/api/rooms/${guest.room}/join`,
    headers: { origin },
    payload: {
      invitation: guest.invitation,
      displayName: "Other PC",
      deviceId: device(13),
    },
  });
  assert.equal(other.statusCode, 200);
  const cookie = String(other.headers["set-cookie"]).split(";")[0]!;
  assert.equal(
    (
      await f.app.inject({
        url: `/api/rooms/${guest.room}/profiles`,
        headers: { cookie },
      })
    ).statusCode,
    200,
  );
});
