import assert from "node:assert/strict";
import { once } from "node:events";
import { test } from "node:test";
import Fastify from "fastify";
import WebSocket, { WebSocketServer } from "ws";
import { AccessToken } from "livekit-server-sdk";
import { loadConfig } from "../src/config.js";
import { Rooms } from "../src/rooms.js";
import { Signaling } from "../src/signaling.js";

test(
  "real signaling upgrades normalize browser/native authority, preserve binary messages and close revoked peers",
  { timeout: 10_000 },
  async (t) => {
    const upstream = new WebSocketServer({ host: "127.0.0.1", port: 0 });
    await once(upstream, "listening");
    const address = upstream.address();
    assert.ok(address && typeof address !== "string");
    const config = loadConfig({
      ADMIN_TOKEN: "test-admin-key-not-for-production-use",
      LIVEKIT_API_KEY: "test-key",
      LIVEKIT_API_SECRET: "test-sfu-secret-not-for-production-use",
      LIVEKIT_INTERNAL_URL: `http://127.0.0.1:${address.port}`,
    });
    const rooms = new Rooms(":memory:");
    const session = rooms.create({ name: "Private", displayName: "Publisher" });
    const gateway = new Signaling(rooms);
    const app = Fastify();
    await gateway.register(app, config);
    const url = (await app.listen({ host: "127.0.0.1", port: 0 })).replace(
      /^http/,
      "ws",
    );
    t.after(async () => {
      gateway.close();
      await app.close();
      await new Promise<void>((resolve) => upstream.close(() => resolve()));
      rooms.close();
    });
    let upstreamCalls = 0;
    upstream.on("connection", (peer, request) => {
      upstreamCalls++;
      assert.equal(
        Boolean(request.headers.authorization?.startsWith("Bearer ")),
        true,
      );
      assert.equal(
        new URL(request.url!, "http://upstream").searchParams.has(
          "access_token",
        ),
        false,
      );
      peer.on("message", (data, binary) => peer.send(data, { binary }));
    });
    const access = new AccessToken(
      config.livekitApiKey,
      config.livekitApiSecret,
      { identity: session.snapshot.participant.id, ttl: 120 },
    );
    access.addGrant({
      roomJoin: true,
      room: `freedomstream_${session.snapshot.room.id}`,
    });
    const token = await access.toJwt();
    const browser = new WebSocket(
      `${url}/media/rtc/v1?access_token=${encodeURIComponent(token)}`,
      { headers: { Origin: config.publicUrl } },
    );
    const native = new WebSocket(`${url}/media/rtc/v1`, {
      headers: { Authorization: `Bearer ${token}` },
    });
    browser.on("error", () => {});
    native.on("error", () => {});
    await Promise.all([once(browser, "open"), once(native, "open")]);
    const echoed = once(browser, "message");
    browser.send(Buffer.from([1, 2, 3]));
    const [data, binary] = await echoed;
    const nativeEcho = once(native, "message");
    native.send(Buffer.from([4, 5, 6]));
    await nativeEcho;
    assert.deepEqual([...data], [1, 2, 3]);
    assert.equal(binary, true);
    assert.equal(upstreamCalls, 2);
    const closed = Promise.all([once(browser, "close"), once(native, "close")]);
    rooms.revoke(session.snapshot.room.id, session.snapshot.participant.id);
    gateway.closeRevoked();
    await closed;
    const status = await new Promise<number>((resolve) => {
      const replay = new WebSocket(
        `${url}/media/rtc/v1?access_token=${encodeURIComponent(token)}`,
        { handshakeTimeout: 2000 },
      );
      replay.on("error", () => {});
      replay.on("unexpected-response", (_request, response) => {
        response.resume();
        resolve(response.statusCode ?? 0);
        replay.terminate();
      });
      replay.on("open", () => {
        replay.terminate();
        resolve(101);
      });
    });
    assert.equal(status, 401);
    assert.equal(upstreamCalls, 2);
  },
);
