import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { readFileSync, mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import { test, type TestContext } from "node:test";
import {
  PREVIEW_TTL_MS,
  PROTECTED_PREVIEW_MAX_BYTES,
  PROTECTED_PREVIEW_TYPE,
  THUMBNAIL_INTERVAL_MS,
  type Preview,
} from "@freedomstream/contracts";
import { buildApp } from "../src/app.js";
import { loadConfig } from "../src/config.js";
import { jpegDimensions } from "../src/jpeg.js";
import {
  MediaUnavailable,
  type MediaDirectory,
  type MediaParticipant,
} from "../src/media.js";
import { Previews } from "../src/previews.js";

// Generated black test pattern, not a captured desktop/camera frame.
const jpeg = readFileSync(new URL("fixtures/preview.jpg", import.meta.url));
const ORIGIN = "http://localhost:5173";
const ADMIN = "test-admin-key-not-a-production-secret";
class Directory implements MediaDirectory {
  beforeRemove: () => Promise<void> = async () => {};
  async remove() {
    await this.beforeRemove();
  }
  participants = new Map<string, MediaParticipant>();
  attributes = new Map<string, string>();
  failure = false;
  async participant(_room: string, owner: string) {
    if (this.failure) throw new MediaUnavailable();
    return this.participants.get(owner) ?? null;
  }
  async attribute(_room: string, owner: string, key: string, value: string) {
    if (this.failure) throw new MediaUnavailable();
    this.attributes.set(`${owner}:${key}`, value);
  }
  publish(owner: string, sid = "TR_screen") {
    this.participants.set(owner, {
      tracks: [{ sid, kind: "screen", muted: false }],
    });
  }
}
async function fixture(t: TestContext) {
  const directory = mkdtempSync(join(tmpdir(), "freedomstream-preview-"));
  let now = Date.now();
  const media = new Directory();
  const config = loadConfig({
    PUBLIC_URL: ORIGIN,
    ADMIN_TOKEN: ADMIN,
    LIVEKIT_API_KEY: "test-api-key",
    LIVEKIT_API_SECRET: "test-media-key-not-a-production-secret",
    DATABASE_PATH: join(directory, "rooms.db"),
  });
  config.webRoot = join(directory, "no-web-build");
  const app = await buildApp(config, { now: () => now, mediaDirectory: media });
  await app.ready();
  t.after(async () => {
    await app.close();
    const path = realpathSync(directory);
    assert.equal(
      dirname(path).toLowerCase(),
      realpathSync(tmpdir()).toLowerCase(),
    );
    assert.ok(basename(path).startsWith("freedomstream-preview-"));
    rmSync(path, { recursive: true });
  });
  const create = async () => {
    const response = await app.inject({
      method: "POST",
      url: "/api/rooms",
      headers: { origin: ORIGIN, authorization: `Bearer ${ADMIN}` },
      payload: { name: "Preview room", displayName: "Publisher" },
    });
    assert.equal(response.statusCode, 201);
    const body = response.json();
    return {
      room: body.room.id as string,
      owner: body.participant.id as string,
      cookie: String(response.headers["set-cookie"]).split(";")[0]!,
      invitation: new URL(body.invitationUrl).hash.slice(8),
    };
  };
  const owner = await create();
  media.publish(owner.owner);
  const headers = {
    origin: ORIGIN,
    cookie: owner.cookie,
    "content-type": "image/jpeg",
    "x-track-sid": "TR_screen",
  };
  const upload = (overrides: Record<string, string> = {}, payload = jpeg) =>
    app.inject({
      method: "POST",
      url: `/api/rooms/${owner.room}/previews/screen`,
      headers: { ...headers, ...overrides },
      payload,
    });
  const imageUrl = (preview: Preview) =>
    `/api/rooms/${owner.room}/previews/${owner.owner}/screen/${preview.revision}`;
  return {
    app,
    media,
    owner,
    headers,
    upload,
    imageUrl,
    create,
    advance: (ms: number) => {
      now += ms;
    },
  };
}

test("revoked publisher previews are denied while another media removal is still in flight", async (t) => {
  const f = await fixture(t);
  const uploaded = await f.upload();
  assert.equal(uploaded.statusCode, 200);
  const url = f.imageUrl(uploaded.json());
  const join = async (displayName: string) => {
    const response = await f.app.inject({
      method: "POST",
      url: `/api/rooms/${f.owner.room}/join`,
      headers: { origin: ORIGIN },
      payload: { invitation: f.owner.invitation, displayName },
    });
    assert.equal(response.statusCode, 200);
    return {
      id: response.json().participant.id,
      cookie: String(response.headers["set-cookie"]).split(";")[0]!,
    };
  };
  const blocker = await join("Blocking removal"),
    viewer = await join("Viewer");
  let release!: () => void;
  let started!: () => void;
  const blocked = new Promise<void>((resolve) => {
    release = resolve;
  });
  const entered = new Promise<void>((resolve) => {
    started = resolve;
  });
  f.media.beforeRemove = async () => {
    started();
    await blocked;
  };
  const headers = { origin: ORIGIN, authorization: `Bearer ${ADMIN}` };
  const first = f.app
    .inject({
      method: "DELETE",
      url: `/api/rooms/${f.owner.room}/access/sessions/${blocker.id}`,
      headers,
    })
    .then((response) => response);
  await entered;
  const second = f.app
    .inject({
      method: "DELETE",
      url: `/api/rooms/${f.owner.room}/access/sessions/${f.owner.owner}`,
      headers,
    })
    .then((response) => response);
  try {
    await new Promise<void>((resolve) => setImmediate(resolve));
    assert.equal(
      (
        await f.app.inject({
          url: `/api/rooms/${f.owner.room}/session`,
          headers: { cookie: f.owner.cookie },
        })
      ).statusCode,
      401,
    );
    assert.equal(
      (await f.app.inject({ url, headers: { cookie: viewer.cookie } }))
        .statusCode,
      404,
    );
  } finally {
    release();
    await Promise.all([first, second]);
  }
});

test("JPEG validation bounds dimensions and rejects multi-image, truncated, progressive and malformed headers", () => {
  assert.deepEqual(jpegDimensions(jpeg), { width: 320, height: 180 });
  assert.equal(jpegDimensions(Buffer.concat([jpeg, jpeg])), null);
  assert.equal(jpegDimensions(jpeg.subarray(0, -2)), null);
  assert.equal(jpegDimensions(Buffer.alloc(262_145)), null);
  const sof = jpeg.indexOf(Buffer.from([0xff, 0xc0]));
  assert.ok(sof > 0);
  const huge = Buffer.from(jpeg);
  huge.writeUInt16BE(4096, sof + 7);
  assert.equal(jpegDimensions(huge), null);
  const progressive = Buffer.from(jpeg);
  progressive[sof + 1] = 0xc2;
  assert.equal(jpegDimensions(progressive), null);
  for (let size = 0; size < jpeg.length; size += 7)
    assert.equal(jpegDimensions(jpeg.subarray(0, size)), null);
});

test("protected previews remain opaque with admission, publication proof and exact MIME", async (t) => {
  const f = await fixture(t);
  // Structural envelope fixture only: the server cannot authenticate ciphertext.
  const envelope = Buffer.concat([Buffer.from("FSP1"), Buffer.alloc(48, 0xa5)]);
  const headers = { "content-type": PROTECTED_PREVIEW_TYPE };
  assert.equal(
    (await f.upload({ ...headers, cookie: "" }, envelope)).statusCode,
    401,
  );
  assert.equal(
    (await f.upload({ ...headers, "x-track-sid": "TR_other" }, envelope))
      .statusCode,
    404,
  );
  assert.equal((await f.upload(headers, jpeg)).statusCode, 400);
  assert.equal((await f.upload(headers, Buffer.from("FSP1"))).statusCode, 400);
  assert.equal(
    (await f.upload(headers, Buffer.alloc(PROTECTED_PREVIEW_MAX_BYTES + 1)))
      .statusCode,
    413,
  );
  assert.equal((await f.upload({}, envelope)).statusCode, 400);
  const uploaded = await f.upload(headers, envelope);
  assert.equal(uploaded.statusCode, 200, uploaded.body);
  const url = f.imageUrl(uploaded.json());
  assert.equal((await f.app.inject(url)).statusCode, 401);
  const downloaded = await f.app.inject({
    url,
    headers: { cookie: f.owner.cookie },
  });
  assert.equal(downloaded.statusCode, 200);
  assert.equal(downloaded.headers["content-type"], PROTECTED_PREVIEW_TYPE);
  assert.equal(downloaded.headers["cache-control"], "no-store");
  assert.deepEqual(downloaded.rawPayload, envelope);
  assert.equal((await f.upload()).statusCode, 429);
  f.advance(THUMBNAIL_INTERVAL_MS);
  assert.equal((await f.upload()).statusCode, 200);
  assert.equal((await f.upload(headers, envelope)).statusCode, 429);
});

test("protected envelopes retain byte limits, expiry and stopped-stream cleanup", async (t) => {
  const f = await fixture(t);
  const envelope = Buffer.alloc(PROTECTED_PREVIEW_MAX_BYTES, 0xa5);
  envelope.write("FSP1");
  const uploaded = await f.upload(
    { "content-type": PROTECTED_PREVIEW_TYPE },
    envelope,
  );
  assert.equal(uploaded.statusCode, 200, uploaded.body);
  const url = f.imageUrl(uploaded.json());
  const read = () => f.app.inject({ url, headers: { cookie: f.owner.cookie } });
  assert.equal((await read()).rawPayload.length, PROTECTED_PREVIEW_MAX_BYTES);
  await f.app.inject({
    method: "DELETE",
    url: `/api/rooms/${f.owner.room}/previews/screen`,
    headers: f.headers,
  });
  assert.equal((await read()).statusCode, 404);
  assert.equal(
    (await f.upload({ "content-type": PROTECTED_PREVIEW_TYPE }, envelope))
      .statusCode,
    429,
  );
  f.advance(PREVIEW_TTL_MS);
  assert.equal(
    (await f.upload({ "content-type": PROTECTED_PREVIEW_TYPE }, envelope))
      .statusCode,
    200,
  );
});

test("private previews require admission, same origin, and the publisher's live unmuted track", async (t) => {
  const f = await fixture(t);
  assert.equal((await f.upload({ cookie: "" })).statusCode, 401);
  assert.equal(
    (await f.upload({ origin: "https://other.example" })).statusCode,
    403,
  );
  assert.equal(
    (await f.upload({ "x-track-sid": "TR_someone_else" })).statusCode,
    404,
  );
  f.media.participants.get(f.owner.owner)!.tracks[0]!.muted = true;
  assert.equal((await f.upload()).statusCode, 404);
  f.media.publish(f.owner.owner);
  const upload = await f.upload();
  assert.equal(upload.statusCode, 200, upload.body);
  const url = f.imageUrl(upload.json());
  assert.equal((await f.app.inject(url)).statusCode, 401);
  const other = await f.create();
  assert.equal(
    (await f.app.inject({ url, headers: { cookie: other.cookie } })).statusCode,
    401,
  );
  const image = await f.app.inject({
    url,
    headers: { cookie: f.owner.cookie },
  });
  assert.equal(image.statusCode, 200);
  assert.equal(image.headers["content-type"], "image/jpeg");
  assert.equal(image.headers["cache-control"], "no-store");
  assert.deepEqual(image.rawPayload, jpeg);
  f.media.participants.clear();
  assert.equal(
    (await f.app.inject({ url, headers: { cookie: f.owner.cookie } }))
      .statusCode,
    404,
  );
});

test("only the latest preview is retained; minute cadence, expiry, late join and Stop are enforced", async (t) => {
  const f = await fixture(t);
  const first = await f.upload();
  assert.equal(first.statusCode, 200);
  const old = f.imageUrl(first.json());
  const early = await f.upload();
  assert.equal(early.statusCode, 429);
  assert.equal(early.headers["retry-after"], "60");
  // A late participant reads the existing image without a new publisher capture.
  const joined = await f.app.inject({
    method: "POST",
    url: `/api/rooms/${f.owner.room}/join`,
    headers: { origin: ORIGIN },
    payload: { displayName: "Late friend", invitation: f.owner.invitation },
  });
  assert.equal(joined.statusCode, 200);
  assert.equal(
    (
      await f.app.inject({
        url: old,
        headers: {
          cookie: String(joined.headers["set-cookie"]).split(";")[0]!,
        },
      })
    ).statusCode,
    200,
  );
  f.advance(THUMBNAIL_INTERVAL_MS);
  const second = await f.upload();
  assert.equal(second.statusCode, 200);
  assert.equal(
    (await f.app.inject({ url: old, headers: { cookie: f.owner.cookie } }))
      .statusCode,
    404,
  );
  const url = f.imageUrl(second.json());
  // An old Stop cannot erase a replacement publication's image.
  f.media.publish(f.owner.owner, "TR_new");
  const replacement = await f.upload({ "x-track-sid": "TR_new" });
  assert.equal(replacement.statusCode, 200);
  await f.app.inject({
    method: "DELETE",
    url: `/api/rooms/${f.owner.room}/previews/screen`,
    headers: f.headers,
  });
  const latest = f.imageUrl(replacement.json());
  assert.equal(
    (await f.app.inject({ url: latest, headers: { cookie: f.owner.cookie } }))
      .statusCode,
    200,
  );
  assert.equal(
    (await f.app.inject({ url, headers: { cookie: f.owner.cookie } }))
      .statusCode,
    404,
  );
  f.advance(PREVIEW_TTL_MS);
  assert.equal(
    (await f.app.inject({ url: latest, headers: { cookie: f.owner.cookie } }))
      .statusCode,
    404,
  );
  const last = await f.upload({ "x-track-sid": "TR_new" });
  assert.equal(last.statusCode, 200);
  await f.app.inject({
    method: "POST",
    url: `/api/rooms/${f.owner.room}/leave`,
    headers: { origin: ORIGIN, cookie: f.owner.cookie },
  });
  assert.equal((await f.upload()).statusCode, 401);
});

test("invalid/oversized images and unavailable SFU do not announce a preview", async (t) => {
  const f = await fixture(t);
  assert.equal(
    (await f.upload({}, Buffer.from("not an image"))).statusCode,
    400,
  );
  assert.equal((await f.upload({}, Buffer.alloc(262_145))).statusCode, 413);
  f.media.failure = true;
  assert.equal((await f.upload()).statusCode, 503);
  assert.equal(f.media.attributes.size, 0);
});

test("Stop/Leave during an in-flight announcement cannot resurrect image bytes", async () => {
  const media = new Directory();
  const owner = randomUUID();
  media.publish(owner);
  let announce!: () => void;
  let began!: () => void;
  const started = new Promise<void>((resolve) => {
    began = resolve;
  });
  media.attribute = async () => {
    began();
    await new Promise<void>((resolve) => {
      announce = resolve;
    });
  };
  const previews = new Previews(media);
  const pending = previews.put(
    "room",
    owner,
    "screen",
    "TR_screen",
    jpeg,
    () => true,
  );
  await started;
  await assert.rejects(
    previews.put("room", owner, "screen", "TR_screen", jpeg, () => true),
    /already in progress/,
  );
  await previews.remove("room", owner, "screen", "TR_screen");
  announce();
  await assert.rejects(pending, /invitation/);
});

test("ephemeral storage is bounded and expired images release capacity", async () => {
  let now = Date.now();
  const media = new Directory();
  const previews = new Previews(media, () => now);
  for (let index = 0; index < 256; index++) {
    const owner = randomUUID();
    media.publish(owner);
    await previews.put("room", owner, "screen", "TR_screen", jpeg, () => true);
  }
  const owner = randomUUID();
  media.publish(owner);
  await assert.rejects(
    previews.put("room", owner, "screen", "TR_screen", jpeg, () => true),
    /storage is busy/,
  );
  now += PREVIEW_TTL_MS;
  await previews.put("room", owner, "screen", "TR_screen", jpeg, () => true);
});

test("deleting an active image cannot reset cadence; expiry during an SFU lookup denies delivery", async () => {
  let now = Date.now();
  const media = new Directory();
  const owner = randomUUID();
  media.publish(owner);
  const previews = new Previews(media, () => now);
  await previews.put("room", owner, "screen", "TR_screen", jpeg, () => true);
  await previews.remove("room", owner, "screen", "TR_screen");
  await assert.rejects(
    previews.put("room", owner, "screen", "TR_screen", jpeg, () => true),
    /once a minute/,
  );
  now += THUMBNAIL_INTERVAL_MS;
  const latest = await previews.put(
    "room",
    owner,
    "screen",
    "TR_screen",
    jpeg,
    () => true,
  );
  let finish!: () => void;
  let began!: () => void;
  const started = new Promise<void>((resolve) => {
    began = resolve;
  });
  media.participant = async () => {
    began();
    await new Promise<void>((resolve) => {
      finish = resolve;
    });
    return { tracks: [{ sid: "TR_screen", kind: "screen", muted: false }] };
  };
  const pending = previews.get("room", owner, "screen", latest.revision);
  await started;
  now += PREVIEW_TTL_MS;
  finish();
  assert.equal(await pending, null);
});
