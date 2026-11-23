import assert from "node:assert/strict";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { defaultStreamPolicy } from "@podetelar/contracts";
import { buildApp } from "../src/app.js";
import { loadConfig } from "../src/config.js";

test("admins set a durable stream policy, and only admitted publishers and the SFU read it", async () => {
  const directory = mkdtempSync(join(tmpdir(), "podetelar-stream-policy-"));
  const origin = "https://stream.example";
  const password = "test-admin-stream-policy-password";
  const mediaSecret = "test-media-stream-policy-secret-long";
  const config = loadConfig({
    PUBLIC_URL: origin,
    ADMIN_TOKEN: password,
    DATABASE_PATH: join(directory, "rooms.db"),
    LIVEKIT_API_KEY: "test-app",
    LIVEKIT_API_SECRET: mediaSecret,
    STREAM_POLICY_ENFORCEMENT: "1",
  });
  const options = { usagePolling: false };
  let app = await buildApp(config, options);
  const adminUrl = "/api/admin/stream-policy";
  const sfuUrl = "/api/internal/sfu/stream-policy";
  const sfuHeaders = { authorization: `Bearer ${mediaSecret}` };
  const policy = {
    ...defaultStreamPolicy,
    maxBitrateBps: 10_000_000,
    maxResolution: 1080,
    maxFps: 60,
    automaticTargets: [
      { resolution: 720, fps: 30, bitrateBps: 4_000_000 },
      { resolution: 1080, fps: 60, bitrateBps: 6_000_000 },
    ],
  };
  try {
    assert.equal((await app.inject(adminUrl)).statusCode, 401);
    assert.equal((await app.inject(sfuUrl)).statusCode, 401);
    const login = await app.inject({
      method: "POST",
      url: "/api/admin/login",
      headers: { origin },
      payload: { password },
    });
    const admin = String(login.headers["set-cookie"]).split(";")[0]!;
    const adminHeaders = { origin, cookie: admin };
    assert.deepEqual(
      (await app.inject({ url: adminUrl, headers: adminHeaders })).json()
        .policy,
      defaultStreamPolicy,
    );
    const save = (payload: object, headers = adminHeaders) =>
      app.inject({ method: "PUT", url: adminUrl, headers, payload });
    assert.equal(
      (await save(policy, { ...adminHeaders, origin: "https://other.example" }))
        .statusCode,
      403,
    );
    assert.equal(
      (await save(policy)).statusCode,
      409,
      "caps require recent SFU synchronization",
    );
    assert.equal(
      (await app.inject({ url: sfuUrl, headers: sfuHeaders })).statusCode,
      200,
    );
    assert.equal((await save({ ...policy, maxFps: 61 })).statusCode, 400);
    assert.equal(
      (
        await save({
          ...policy,
          automaticTargets: [
            ...policy.automaticTargets,
            policy.automaticTargets[0],
          ],
        })
      ).statusCode,
      400,
    );
    const created = await app.inject({
      method: "POST",
      url: "/api/admin/rooms",
      headers: adminHeaders,
      payload: { name: "Policy acceptance" },
    });
    const { room, invitationUrl } = created.json();
    const session = await app.inject({
      method: "POST",
      url: `/api/rooms/${room.id}/join`,
      headers: { origin },
      payload: {
        displayName: "Publisher",
        invitation: new URL(invitationUrl).hash.slice("#invite=".length),
      },
    });
    const participant = String(session.headers["set-cookie"]).split(";")[0]!;
    const sessionUrl = `/api/rooms/${room.id}/stream-policy`;
    assert.equal((await app.inject(sessionUrl)).statusCode, 401);
    assert.equal(
      (await save(policy, { origin, cookie: participant })).statusCode,
      401,
    );
    const result = await save(policy);
    assert.equal(result.statusCode, 200);
    const expected = { ...policy, revision: 1 };
    assert.deepEqual(result.json().policy, expected);
    assert.equal(
      (await save(policy)).statusCode,
      409,
      "stale dashboard writes cannot overwrite policy",
    );
    const publisher = await app.inject({
      url: sessionUrl,
      headers: { cookie: participant },
    });
    assert.equal(publisher.headers["cache-control"], "no-store");
    assert.deepEqual(publisher.json(), expected);
    const sfu = await app.inject({ url: sfuUrl, headers: sfuHeaders });
    assert.deepEqual(sfu.json(), {
      revision: 1,
      maxBitrateBps: 10_000_000,
      maxResolution: 1080,
      maxFps: 60,
    });
    await app.close();
    app = await buildApp(config, options);
    const mediaUrl = `/api/rooms/${room.id}/native-media-token`;
    const mediaRequest = {
      method: "POST" as const,
      url: mediaUrl,
      headers: { origin, cookie: participant },
    };
    assert.equal((await app.inject(mediaRequest)).statusCode, 503);
    assert.equal(
      (await app.inject({ url: sfuUrl, headers: sfuHeaders })).statusCode,
      200,
    );
    assert.equal((await app.inject(mediaRequest)).statusCode, 200);
    assert.deepEqual(
      (
        await app.inject({ url: sessionUrl, headers: { cookie: participant } })
      ).json(),
      expected,
    );
    await app.close();
    app = await buildApp(
      { ...config, streamPolicyEnforcement: false },
      options,
    );
    const secondLogin = await app.inject({
      method: "POST",
      url: "/api/admin/login",
      headers: { origin },
      payload: { password },
    });
    const secondAdmin = String(secondLogin.headers["set-cookie"]).split(
      ";",
    )[0]!;
    const headers = { origin, cookie: secondAdmin };
    assert.equal(
      (await save({ ...expected, maxFps: 30 }, headers)).statusCode,
      409,
    );
    const defaultsOnly = await save(
      { ...expected, automaticTargets: [] },
      headers,
    );
    assert.equal(defaultsOnly.statusCode, 200);
    assert.deepEqual(defaultsOnly.json().policy, {
      ...expected,
      revision: 2,
      automaticTargets: [],
    });
  } finally {
    await app.close();
    rmSync(directory, { recursive: true, force: true });
  }
});
