import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { buildApp } from "../src/app.js";
import { loadConfig } from "../src/config.js";

const origin = "http://localhost:5173";
const admin = "test-invitation-administrator-not-production";
test("invitation preview requires a current invitation and reveals only connected active members without admitting browsers", async (t) => {
  const dir = mkdtempSync(join(tmpdir(), "freedomstream-invitation-"));
  const config = loadConfig({
    PUBLIC_URL: origin,
    ADMIN_TOKEN: admin,
    LIVEKIT_API_KEY: "test-key",
    LIVEKIT_API_SECRET: "test-media-secret-not-for-production",
    DATABASE_PATH: join(dir, "rooms.db"),
  });
  config.webRoot = join(dir, "no-assets");
  let members: { id: string; displayName: string }[] = [];
  let fail = false;
  let beforeMembers = async () => {};
  const app = await buildApp(config, {
    usagePolling: false,
    mediaDirectory: {
      async remove() {},
      async participant() {
        return null;
      },
      async attribute() {},
      async members() {
        await beforeMembers();
        if (fail) throw new Error("private SDK detail");
        return members;
      },
    },
  });
  t.after(async () => {
    await app.close();
    rmSync(dir, { recursive: true });
  });
  const created = await app.inject({
    method: "POST",
    url: "/api/rooms",
    headers: { origin, authorization: `Bearer ${admin}` },
    payload: { name: "Private", displayName: "Online friend" },
  });
  assert.equal(created.statusCode, 201);
  const body = created.json();
  const invitation = new URL(body.invitationUrl).hash.slice(8);
  const url = `/api/rooms/${body.room.id}/invitation-preview`;
  members = [
    { id: body.participant.id, displayName: "Online friend" },
    { id: "stale", displayName: "Stale connection" },
  ];
  for (const payload of [{}, { invitation: "x".repeat(43) }])
    assert.equal(
      (await app.inject({ method: "POST", url, headers: { origin }, payload }))
        .statusCode,
      404,
    );
  assert.equal(
    (
      await app.inject({
        method: "POST",
        url,
        headers: { origin: "https://other.example" },
        payload: { invitation },
      })
    ).statusCode,
    403,
  );
  const preview = () =>
    app.inject({
      method: "POST",
      url,
      headers: { origin },
      payload: { invitation },
    });
  const response = await preview();
  assert.equal(response.statusCode, 200);
  assert.deepEqual(response.json(), { members: [members[0]] });
  assert.equal(response.headers["set-cookie"], undefined);
  assert.equal(
    (await app.inject({ url: `/api/rooms/${body.room.id}/session` }))
      .statusCode,
    401,
  );
  members = [];
  assert.deepEqual((await preview()).json(), { members: [] });
  fail = true;
  const unavailable = await preview();
  assert.equal(unavailable.statusCode, 503);
  assert.ok(!unavailable.body.includes("SDK"));
  fail = false;
  beforeMembers = async () => {
    const changed = await app.inject({
      method: "POST",
      url: `/api/rooms/${body.room.id}/access`,
      headers: { origin, authorization: `Bearer ${admin}` },
      payload: { invitation: "rotate", revokeSessions: false },
    });
    assert.equal(changed.statusCode, 200);
  };
  assert.equal(
    (await preview()).statusCode,
    404,
    "rotation during member lookup denies disclosure",
  );
});
