import assert from "node:assert/strict";
import { test } from "node:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve, dirname, basename } from "node:path";
import { request } from "node:http";
import { buildApp } from "../src/app.js";
import { loadConfig } from "../src/config.js";
import { PROBE_BPS, PROBE_BYTES } from "../src/network-test.js";

test("real connection transfers authenticate early, share one lease, pace download and enforce cooldown", async () => {
  const root = mkdtempSync(join(tmpdir(), "freedomstream-network-"));
  const origin = "http://localhost:5173";
  const config = loadConfig({
    PUBLIC_URL: origin,
    ADMIN_TOKEN: "test-administrator-key-with-32-characters",
    LIVEKIT_API_KEY: "test-key",
    LIVEKIT_API_SECRET: "test-livekit-secret-with-32-characters",
    DATABASE_PATH: join(root, "rooms.db"),
  });
  const app = await buildApp(config, {
    mediaDirectory: {
      async participant() {
        return null;
      },
      async attribute() {},
      async remove() {},
    },
  });
  try {
    const address = await app.listen({ host: "127.0.0.1", port: 0 });
    const created = await app.inject({
      method: "POST",
      url: "/api/rooms",
      headers: { origin, authorization: `Bearer ${config.adminToken}` },
      payload: { name: "Probe test", displayName: "First" },
    });
    assert.equal(created.statusCode, 201);
    const id = created.json().room.id;
    const cookie = String(created.headers["set-cookie"]).split(";")[0]!;
    const other = await app.inject({
      method: "POST",
      url: `/api/rooms/${id}/join`,
      headers: { origin },
      payload: {
        invitation: new URL(created.json().invitationUrl).hash.slice(8),
        displayName: "Second",
      },
    });
    const second = String(other.headers["set-cookie"]).split(";")[0]!;
    const path = `/api/rooms/${id}/network-test`;
    // An unauthenticated streaming upload must be rejected before its body ends.
    const early = await new Promise<number>((resolve, reject) => {
      const req = request(
        address + path + "/upload",
        {
          method: "POST",
          headers: {
            origin,
            "content-type": "application/octet-stream",
            "content-length": PROBE_BYTES,
          },
        },
        (res) => {
          res.resume();
          res.on("end", () => {
            resolve(res.statusCode!);
            req.destroy();
          });
        },
      );
      req.on("error", () => reject(Error("Local early-auth check failed.")));
      req.setTimeout(2000, () => req.destroy());
      req.write(Buffer.alloc(1024));
    });
    assert.equal(early, 401);
    const begin = await app.inject({
      method: "POST",
      url: path,
      headers: { origin, cookie, "x-network-test-max-bps": String(PROBE_BPS) },
    });
    assert.equal(begin.statusCode, 200);
    assert.equal(begin.json().payloadCapBps, 200_000_000);
    assert.equal(begin.json().byteLimit, 48 * 1024 * 1024);
    const probe = begin.json().probe;
    assert.equal(
      (
        await app.inject({
          method: "POST",
          url: path,
          headers: { origin, cookie: second },
        })
      ).statusCode,
      409,
    );
    const start = performance.now();
    const down = await fetch(address + path + "/download", {
      headers: { cookie, "x-network-probe": probe },
    }).catch(() => {
      throw Error("Local download failed.");
    });
    assert.equal(down.status, 200);
    const bytes = (await down.arrayBuffer()).byteLength;
    assert.ok(bytes >= 65536 && bytes <= PROBE_BYTES);
    assert.ok(performance.now() - start >= (bytes * 8000) / PROBE_BPS - 100);
    const up = await fetch(address + path + "/upload", {
      method: "POST",
      headers: {
        origin,
        cookie,
        "x-network-probe": probe,
        "content-type": "application/octet-stream",
      },
      body: Buffer.alloc(1024 * 1024),
    }).catch(() => {
      throw Error("Local upload failed.");
    });
    assert.equal(up.status, 200);
    const result = await up.json();
    assert.equal(result.bytes, 1024 * 1024);
    assert.ok(result.bps > 0 && result.bps <= PROBE_BPS);
    assert.equal(
      (
        await app.inject({
          method: "POST",
          url: path,
          headers: { origin, cookie },
        })
      ).statusCode,
      429,
    );
    const next = await app.inject({
      method: "POST",
      url: path,
      headers: { origin, cookie: second },
    });
    assert.equal(next.statusCode, 200);
    assert.equal(next.json().payloadCapBps, 100_000_000);
    assert.equal(next.json().byteLimit, 24 * 1024 * 1024);
    const cancelled = await app.inject({
      method: "DELETE",
      url: path,
      headers: { origin, cookie: second, "x-network-probe": next.json().probe },
    });
    assert.equal(cancelled.statusCode, 204);
    // A stale lease cannot be used even by another authenticated participant.
    assert.equal(
      (
        await app.inject({
          method: "GET",
          url: path + "/download",
          headers: { cookie: second, "x-network-probe": probe },
        })
      ).statusCode,
      409,
    );
    const newMember = async (name: string) => {
      const joined = await app.inject({
        method: "POST",
        url: `/api/rooms/${id}/join`,
        headers: { origin },
        payload: {
          invitation: new URL(created.json().invitationUrl).hash.slice(8),
          displayName: name,
        },
      });
      assert.equal(joined.statusCode, 200);
      return String(joined.headers["set-cookie"]).split(";")[0]!;
    };
    const third = await newMember("Third");
    const thirdLease = await app.inject({
      method: "POST",
      url: path,
      headers: { origin, cookie: third },
    });
    assert.equal(thirdLease.statusCode, 200);
    const controller = new AbortController();
    const stream = await fetch(address + path + "/download", {
      headers: { cookie: third, "x-network-probe": thirdLease.json().probe },
      signal: controller.signal,
    });
    const reader = stream.body!.getReader();
    assert.ok((await reader.read()).value!.length > 0);
    controller.abort();
    await reader.cancel().catch(() => {});
    const fourth = await newMember("Fourth");
    let released = false;
    for (let retry = 0; retry < 30; retry++) {
      const next = await app.inject({
        method: "POST",
        url: path,
        headers: { origin, cookie: fourth },
      });
      if (next.statusCode === 200) {
        released = true;
        await app.inject({
          method: "DELETE",
          url: path,
          headers: {
            origin,
            cookie: fourth,
            "x-network-probe": next.json().probe,
          },
        });
        break;
      }
      assert.equal(next.statusCode, 409);
      await new Promise((r) => setTimeout(r, 50));
    }
    assert.ok(released, "Aborted transfer did not promptly release its lease");
    const fifth = await newMember("Fifth");
    const fifthLease = await app.inject({
      method: "POST",
      url: path,
      headers: { origin, cookie: fifth },
    });
    assert.equal(fifthLease.statusCode, 200);
    const fifthToken = fifthLease.json().probe;
    const readyForUpload = await fetch(address + path + "/download", {
      headers: { cookie: fifth, "x-network-probe": fifthToken },
    });
    await readyForUpload.arrayBuffer();
    const stalledAt = performance.now();
    // A slow writer cannot reserve the global test slot indefinitely.
    await new Promise<void>((resolve, reject) => {
      const req = request(
        address + path + "/upload",
        {
          method: "POST",
          headers: {
            origin,
            cookie: fifth,
            "x-network-probe": fifthToken,
            "content-type": "application/octet-stream",
            "content-length": PROBE_BYTES,
          },
        },
        (res) => res.resume(),
      );
      const limit = setTimeout(() => {
        req.destroy();
        reject(Error("Upload deadline was not enforced."));
      }, 8000);
      req.on("error", () => {});
      req.on("close", () => {
        clearTimeout(limit);
        resolve();
      });
      req.write(Buffer.alloc(1024));
    });
    assert.ok(performance.now() - stalledAt >= 5000);
    const sixth = await newMember("Sixth");
    const sixthLease = await app.inject({
      method: "POST",
      url: path,
      headers: { origin, cookie: sixth },
    });
    assert.equal(sixthLease.statusCode, 200);
    assert.equal(
      (
        await app.inject({
          method: "POST",
          url: `/api/rooms/${id}/leave`,
          headers: { origin, cookie: sixth },
        })
      ).statusCode,
      204,
    );
    assert.equal(
      (
        await app.inject({
          method: "GET",
          url: path + "/download",
          headers: {
            cookie: sixth,
            "x-network-probe": sixthLease.json().probe,
          },
        })
      ).statusCode,
      401,
    );
    const seventh = await newMember("Seventh");
    assert.equal(
      (
        await app.inject({
          method: "POST",
          url: path,
          headers: { origin, cookie: seventh },
        })
      ).statusCode,
      200,
    );
  } finally {
    app.server.closeAllConnections();
    await app.close();
    assert.equal(dirname(resolve(root)), resolve(tmpdir()));
    assert.ok(basename(root).startsWith("freedomstream-network-"));
    rmSync(root, { recursive: true, force: true });
  }
});
