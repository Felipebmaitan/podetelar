import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { request as httpRequest } from "node:http";
import { buildApp } from "../src/app.js";
import { loadConfig } from "../src/config.js";

test("split origins keep invitations public, admit natives directly, and transfer only authorized downloads", async (t) => {
  const dir = mkdtempSync(join(tmpdir(), "freedomstream-domains-"));
  const bytes = Buffer.from("verified local download fixture");
  const filename = "freedomstream-windows-x64-0.1.0-aaaaaaaaaaaa.exe";
  writeFileSync(join(dir, filename), bytes);
  writeFileSync(
    join(dir, "publisher.json"),
    JSON.stringify({
      filename,
      version: "0.1.0",
      commit: "a".repeat(40),
      sha256: createHash("sha256").update(bytes).digest("hex"),
      size: bytes.length,
    }),
  );
  const publicUrl = "https://stream.maitan.me";
  const directUrl = "https://stream-direct.maitan.me:8443";
  const config = loadConfig({
    PUBLIC_URL: publicUrl,
    DIRECT_URL: directUrl,
    ADMIN_TOKEN: "test-only-administrator-with-32-characters",
    LIVEKIT_API_KEY: "test-key",
    LIVEKIT_API_SECRET: "test-only-livekit-secret-with-32-characters",
    DATABASE_PATH: join(dir, "rooms.db"),
    PUBLISHER_DIRECTORY: dir,
  });
  let now = Date.now();
  const app = await buildApp(config, {
    now: () => now,
    usagePolling: false,
    mediaDirectory: {
      async members() {
        return [];
      },
      async participant() {
        return null;
      },
      async attribute() {},
      async remove() {},
    },
  });
  t.after(async () => {
    await app.close();
    rmSync(dir, { recursive: true });
  });
  const created = await app.inject({
    method: "POST",
    url: "/api/rooms",
    headers: {
      host: "stream.maitan.me",
      origin: publicUrl,
      authorization: `Bearer ${config.adminToken}`,
    },
    payload: { name: "Friends", displayName: "Website session" },
  });
  assert.equal(created.statusCode, 201);
  const room = created.json().room.id;
  const invitationUrl = new URL(created.json().invitationUrl);
  assert.equal(invitationUrl.origin, publicUrl);
  const invitation = invitationUrl.hash.slice(8);
  const publicCookie = String(created.headers["set-cookie"]).split(";")[0]!;
  const headers = { host: new URL(directUrl).host, origin: directUrl };
  assert.deepEqual((await app.inject({ url: "/api/connection" })).json(), {
    directUrl,
  });
  const handoff = new URL(
    (
      await app.inject({
        method: "POST",
        url: `/api/rooms/${room}/app-link`,
        headers: { origin: publicUrl },
        payload: { invitation },
      })
    ).json().appUrl,
  );
  assert.equal(handoff.searchParams.get("server"), publicUrl);
  const redeemed = await app.inject({
    method: "POST",
    url: "/api/app-links/redeem",
    headers,
    payload: { ticket: handoff.hash.slice(9) },
  });
  assert.equal(new URL(redeemed.json().invitationUrl).origin, publicUrl);
  const joined = await app.inject({
    method: "POST",
    url: `/api/rooms/${room}/join`,
    headers,
    payload: { invitation, displayName: "Native session" },
  });
  assert.equal(joined.statusCode, 200);
  const cookie = String(joined.headers["set-cookie"]).split(";")[0]!;
  assert.ok(!String(joined.headers["set-cookie"]).includes("Domain="));
  const access = await app.inject({
    method: "POST",
    url: `/api/rooms/${room}/native-media-token`,
    headers: { ...headers, cookie },
  });
  assert.equal(
    access.json().serverUrl,
    "wss://stream-direct.maitan.me:8443/media",
  );
  assert.equal(
    (
      await app.inject({
        method: "POST",
        url: `/api/rooms/${room}/join`,
        headers: { origin: "https://evil.test" },
        payload: { invitation, displayName: "Denied" },
      })
    ).statusCode,
    403,
  );
  // Exercise host routing through real HTTP too, rather than only injection.
  const listen = await app.listen({ host: "127.0.0.1", port: 0 });
  const http = (path: string, headers: Record<string, string>) =>
    new Promise<{ status: number; body: string }>((resolve, reject) => {
      const request = httpRequest(
        listen + path,
        { method: "POST", headers },
        (response) => {
          let body = "";
          response.setEncoding("utf8");
          response.on("data", (chunk) => {
            body += chunk;
          });
          response.on("end", () =>
            resolve({ status: response.statusCode!, body }),
          );
        },
      );
      request.on("error", reject);
      request.end();
    });
  const probePath = `/api/rooms/${room}/network-test`;
  const refused = await http(probePath, {
    host: "stream.maitan.me",
    origin: publicUrl,
    cookie: publicCookie,
  });
  assert.equal(refused.status, 421);
  const probe = await http(probePath, { ...headers, cookie });
  assert.equal(probe.status, 200);
  await app.inject({
    method: "DELETE",
    url: probePath,
    headers: {
      ...headers,
      cookie,
      "x-network-probe": JSON.parse(probe.body).probe,
    },
  });
  assert.equal(
    (await app.inject({ method: "POST", url: probePath, headers })).statusCode,
    401,
  );
  const releasePath = `/api/rooms/${room}/publisher`;
  const metadata = await app.inject({
    url: releasePath,
    headers: { cookie: publicCookie },
  });
  let requestId = 0;
  const downloadRequest = () =>
    app.inject({
      url: metadata.json().downloadUrl,
      remoteAddress: `127.0.2.${++requestId}`,
      headers: { host: "stream.maitan.me", cookie: publicCookie },
    });
  const issue = async () => {
    const response = await downloadRequest();
    assert.equal(response.statusCode, 303);
    return new URL(response.headers.location!);
  };
  const download = await issue();
  assert.equal(download.origin, directUrl);
  assert.equal(download.pathname, "/download");
  assert.equal(download.search, "");
  const token = download.hash.slice(8);
  assert.ok(!download.href.includes(invitation));
  const consume = (token: string, extra = {}) =>
    app.inject({
      method: "POST",
      url: "/api/publisher/download",
      headers: {
        ...headers,
        "content-type": "application/x-www-form-urlencoded",
        ...extra,
      },
      payload: new URLSearchParams({ ticket: token }).toString(),
    });
  assert.equal(
    (await consume(token, { host: "stream.maitan.me" })).statusCode,
    421,
  );
  assert.equal(
    (await consume(token, { origin: "https://evil.test" })).statusCode,
    403,
  );
  const downloaded = await consume(token);
  assert.equal(downloaded.statusCode, 200);
  assert.deepEqual(downloaded.rawPayload, bytes);
  assert.equal(
    downloaded.headers["set-cookie"],
    undefined,
    "download never copies a website session cookie to the direct host",
  );
  assert.equal(downloaded.headers["cache-control"], "no-store");
  assert.equal((await consume(token)).statusCode, 401);
  const expiring = (await issue()).hash.slice(8);
  now += 60_000;
  assert.equal((await consume(expiring)).statusCode, 401);
  for (let i = 0; i < 128; i++) await issue();
  assert.equal(
    (await downloadRequest()).statusCode,
    503,
    "pending downloads are bounded across clients",
  );
  now += 60_000;
  const revoked = (await issue()).hash.slice(8);
  await app.inject({
    method: "POST",
    url: `/api/rooms/${room}/leave`,
    headers: { origin: publicUrl, cookie: publicCookie },
  });
  assert.equal((await consume(revoked)).statusCode, 401);
  assert.equal((await app.inject({ url: releasePath })).statusCode, 401);
});
