import assert from "node:assert/strict";
import { mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import { test, type TestContext } from "node:test";
import { buildApp } from "../src/app.js";
import { loadConfig } from "../src/config.js";
import type { MediaDirectory, MediaParticipant } from "../src/media.js";
const ORIGIN = "http://localhost:5173";
const ADMIN = "test-admin-key-not-a-production-secret";
class Directory implements MediaDirectory {
  diagnosticParticipants?: (
    room: string,
  ) => Promise<Map<string, MediaParticipant>>;
  participants = new Map<string, MediaParticipant>();
  async members() {
    return [];
  }
  async remove() {}
  async attribute() {}
  beforeLookup: (() => Promise<void>) | undefined;
  async participant(_room: string, owner: string) {
    await this.beforeLookup?.();
    return this.participants.get(owner) ?? null;
  }
  publish(owner: string, sid = "TR_screen") {
    this.participants.set(owner, {
      tracks: [{ sid, kind: "screen", muted: false }],
    });
  }
}
async function fixture(t: TestContext) {
  const directory = mkdtempSync(join(tmpdir(), "freedomstream-stats-"));
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
    assert.ok(basename(path).startsWith("freedomstream-stats-"));
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
  const viewer = async () => {
    const response = await app.inject({
      method: "POST",
      url: `/api/rooms/${owner.room}/join`,
      headers: { origin: ORIGIN },
      payload: { invitation: owner.invitation, displayName: "Viewer" },
    });
    assert.equal(response.statusCode, 200);
    const id = response.json().participant.id as string;
    media.participants.set(id, {
      tracks: [],
      attributes: { [`freedomstream.watch.${owner.owner}`]: "av1" },
    });
    return {
      id,
      cookie: String(response.headers["set-cookie"]).split(";")[0]!,
    };
  };
  const url = `/api/rooms/${owner.room}/stream-diagnostics/${owner.owner}/TR_screen`;
  const request = (
    method: "GET" | "POST" | "PUT" | "DELETE",
    cookie = owner.cookie,
    payload?: unknown,
    path = url,
  ) =>
    app.inject({
      method,
      url: path,
      headers: { origin: ORIGIN, cookie },
      ...(payload ? { payload } : {}),
    });
  return {
    app,
    media,
    owner,
    viewer,
    request,
    url,
    advance: (ms: number) => {
      now += ms;
    },
  };
}
test("diagnostic demand is off by default, reference counted, expires and resets on publication replacement", async (t) => {
  const f = await fixture(t);
  assert.equal((await f.request("GET")).json().demand, 0);
  const a = await f.viewer(),
    b = await f.viewer();
  assert.equal((await f.request("POST", a.cookie)).statusCode, 200);
  assert.equal((await f.request("POST", b.cookie)).statusCode, 200);
  assert.equal((await f.request("GET")).json().demand, 2);
  await f.request("DELETE", a.cookie);
  assert.equal((await f.request("GET")).json().demand, 1);
  f.advance(10001);
  assert.equal((await f.request("GET")).json().demand, 0);
  await f.request("POST", b.cookie);
  f.media.publish(f.owner.owner, "TR_replacement");
  assert.equal((await f.request("GET")).statusCode, 409);
  assert.equal((await f.request("POST", b.cookie)).statusCode, 409);
});
test("diagnostic endpoints deny anonymous, wrong owner, unwatched and cross-room sessions", async (t) => {
  const f = await fixture(t);
  const a = await f.viewer();
  assert.equal((await f.request("POST", "")).statusCode, 401);
  assert.equal((await f.request("GET", a.cookie)).statusCode, 403);
  assert.equal((await f.request("PUT", a.cookie, {})).statusCode, 403);
  f.media.participants.get(a.id)!.attributes = {};
  assert.equal((await f.request("POST", a.cookie)).statusCode, 403);
  const other = await f.app.inject({
    method: "POST",
    url: "/api/rooms",
    headers: { origin: ORIGIN, authorization: `Bearer ${ADMIN}` },
    payload: { name: "Other", displayName: "Other" },
  });
  const cookie = String(other.headers["set-cookie"]).split(";")[0]!;
  assert.equal((await f.request("POST", cookie)).statusCode, 401);
});
test("valid aggregates are bounded, memory only, stale explicitly and erased with demand", async (t) => {
  const f = await fixture(t);
  const a = await f.viewer();
  await f.request("POST", a.cookie);
  const names = [
    "encoded_window_ms",
    "capture_queue_frames",
    "encoder_queue_frames",
    "capture_queue_overruns",
    "capture_fps",
    "encoder_input_fps",
    "requested_bps",
    "effective_bps",
    "encoded_bps",
    "sent_bps",
    "upload_cap_bps",
    "watcher_cap_bps",
    "congestion_bps",
    "requested_fps",
    "encoded_fps",
    "sent_fps",
    "accepted_fps",
    "encoder_reconfigurations",
    "handoff_rejections",
    "source_width",
    "source_height",
    "encoded_width",
    "encoded_height",
    "rtt_ms",
    "loss_fraction",
  ];
  const sample = {
    window_ms: 3000,
    sequence: 1,
    codec: "av1",
    encoder: "nvd3d11av1enc",
    metrics: Object.fromEntries(names.map((k) => [k, null])),
  };
  assert.equal((await f.request("PUT", undefined, sample)).statusCode, 200);
  assert.equal((await f.request("PUT", undefined, sample)).statusCode, 429);
  const result = (await f.request("POST", a.cookie)).json();
  assert.equal(result.stale, false);
  assert.equal(result.sample.metrics.capture_fps, null);
  f.advance(8000);
  assert.equal((await f.request("POST", a.cookie)).json().stale, true);
  assert.equal(
    (
      await f.request("PUT", undefined, {
        ...sample,
        sequence: 2,
        secret: "forbidden",
      })
    ).statusCode,
    400,
  );
  assert.equal(
    (
      await f.request("PUT", undefined, {
        ...sample,
        sequence: 2,
        metrics: { ...sample.metrics, capture_fps: -1 },
      })
    ).statusCode,
    400,
  );
  assert.equal(
    (await f.request("PUT", undefined, { data: "a".repeat(5000) })).statusCode,
    413,
  );
  await f.request("DELETE", a.cookie);
  assert.equal((await f.request("POST", a.cookie)).json().sample, null);
  f.media.participants.get(a.id)!.attributes = {};
  assert.equal((await f.request("GET")).json().demand, 0);
});

test("in-flight publication replacement cannot install stale demand", async (t) => {
  const f = await fixture(t);
  const a = await f.viewer();
  let release!: () => void;
  let entered!: () => void;
  const held = new Promise<void>((r) => {
    release = r;
  });
  const ready = new Promise<void>((r) => {
    entered = r;
  });
  let calls = 0;
  f.media.beforeLookup = async () => {
    if (++calls === 2) {
      entered();
      await held;
    }
  };
  const pending = f.request("POST", a.cookie);
  void pending.then(() => {});
  await ready;
  f.media.publish(f.owner.owner, "TR_new");
  const newUrl = f.url.replace("TR_screen", "TR_new");
  assert.equal(
    (await f.request("POST", a.cookie, undefined, newUrl)).statusCode,
    429,
  );
  release();
  assert.equal((await pending).statusCode, 409);
  f.media.beforeLookup = undefined;
  assert.equal(
    (await f.request("POST", a.cookie, undefined, newUrl)).statusCode,
    200,
  );
  assert.equal((await f.request("POST", a.cookie)).statusCode, 409);
  assert.equal(
    (await f.request("GET", undefined, undefined, newUrl)).json().demand,
    1,
  );
});

test("session departure revokes demand and prevents lease renewal", async (t) => {
  const f = await fixture(t);
  const a = await f.viewer();
  await f.request("POST", a.cookie);
  const leave = await f.app.inject({
    method: "POST",
    url: `/api/rooms/${f.owner.room}/leave`,
    headers: { origin: ORIGIN, cookie: a.cookie },
  });
  assert.equal(leave.statusCode, 204);
  assert.equal((await f.request("POST", a.cookie)).statusCode, 401);
  assert.equal((await f.request("GET")).json().demand, 0);
});

test("production snapshot validation shares bounded participant facts", async (t) => {
  const f = await fixture(t);
  const a = await f.viewer(),
    b = await f.viewer();
  let snapshots = 0;
  let individual = 0;
  f.media.diagnosticParticipants = async () => {
    snapshots++;
    return new Map(f.media.participants);
  };
  f.media.beforeLookup = async () => {
    individual++;
  };
  await f.request("POST", a.cookie);
  await f.request("POST", b.cookie);
  snapshots = 0;
  individual = 0;
  assert.equal((await f.request("GET")).json().demand, 2);
  assert.equal(snapshots, 1);
  assert.equal(individual, 1);
  f.media.participants.get(a.id)!.attributes = {};
  assert.equal((await f.request("GET")).json().demand, 1);
});
