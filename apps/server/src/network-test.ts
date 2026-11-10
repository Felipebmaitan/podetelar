import { randomBytes } from "node:crypto";
import { Readable } from "node:stream";
import { setTimeout as delay } from "node:timers/promises";
import type { FastifyInstance } from "fastify";
import type { Rooms } from "./rooms.js";

// One shared lease: simultaneous joins cannot each consume 100 Mbps.
export const PROBE_BPS = 100_000_000;
export const PROBE_BYTES = 24 * 1024 * 1024;
const DURATION_MS = 2_000;
type Own = { roomId: string };
type Lease = {
  token: string;
  owner: string;
  room: string;
  phase: string;
  until: number;
  cookie: string;
};
export async function networkTestRoutes(app: FastifyInstance, rooms: Rooms) {
  let lease: Lease | undefined;
  const sessions = new Map<string, number>();
  const ips = new Map<string, { start: number; count: number }>();
  const member = (room: string, cookie?: string) => rooms.session(room, cookie);
  await app.register(async (scope) => {
    // Authentication runs before an upload body is read or buffered.
    scope.addHook<{ Params: Own }>("onRequest", async (request, reply) => {
      if (!member(request.params.roomId, request.cookies.fs_session))
        return reply.code(401).send({ error: "Room access required." });
      reply.header("Cache-Control", "no-store");
    });
    scope.post<{ Params: Own }>(
      "/api/rooms/:roomId/network-test",
      async (request, reply) => {
        const now = Date.now();
        if (lease && !member(lease.room, lease.cookie)) lease = undefined;
        for (const [key, last] of sessions)
          if (now - last > 60_000) sessions.delete(key);
        for (const [key, value] of ips)
          if (now - value.start > 60_000) ips.delete(key);
        if (lease && lease.until > now)
          return reply
            .code(409)
            .header("Retry-After", "2")
            .send({ error: "Connection test busy." });
        const owner = member(request.params.roomId, request.cookies.fs_session)!
          .participant.id;
        const ip = ips.get(request.ip);
        if (
          (sessions.get(owner) ?? 0) + 30_000 > now ||
          (ip?.count ?? 0) >= 8 ||
          sessions.size >= 256 ||
          ips.size >= 256
        )
          return reply
            .code(429)
            .header("Retry-After", "30")
            .send({ error: "Wait before testing again." });
        sessions.set(owner, now);
        ips.set(request.ip, {
          start: ip?.start ?? now,
          count: (ip?.count ?? 0) + 1,
        });
        lease = {
          token: randomBytes(24).toString("hex"),
          owner,
          room: request.params.roomId,
          phase: "download",
          until: now + 12_000,
          cookie: request.cookies.fs_session!,
        };
        return {
          probe: lease.token,
          payloadCapBps: PROBE_BPS,
          byteLimit: PROBE_BYTES,
        };
      },
    );
    const valid = (
      room: string,
      cookie: string | undefined,
      token: unknown,
      phase: string,
    ) =>
      lease &&
      lease.until > Date.now() &&
      lease.room === room &&
      lease.token === token &&
      lease.phase === phase &&
      member(room, cookie)?.participant.id === lease.owner
        ? lease
        : undefined;
    scope.get<{ Params: Own }>(
      "/api/rooms/:roomId/network-test/download",
      async (request, reply) => {
        const current = valid(
          request.params.roomId,
          request.cookies.fs_session,
          request.headers["x-network-probe"],
          "download",
        );
        if (!current)
          return reply.code(409).send({ error: "Connection test expired." });
        current.phase = "sending";
        const block = randomBytes(64 * 1024);
        reply
          .type("application/octet-stream")
          .header("Content-Encoding", "identity");
        const deadline = setTimeout(() => reply.raw.destroy(), 6_000);
        deadline.unref();
        reply.raw.once("close", () => clearTimeout(deadline));
        async function* data() {
          const start = performance.now();
          let sent = 0,
            finished = false;
          try {
            while (
              sent < PROBE_BYTES &&
              performance.now() - start < DURATION_MS
            ) {
              if (
                lease !== current ||
                !member(request.params.roomId, request.cookies.fs_session)
              )
                return;
              await delay(
                Math.max(
                  0,
                  (sent * 8_000) / PROBE_BPS - (performance.now() - start),
                ),
              );
              yield block;
              sent += block.length;
            }
            finished = true;
          } finally {
            if (lease === current) {
              if (finished && current) current.phase = "upload";
              else lease = undefined;
            }
          }
        }
        return reply.send(Readable.from(data()));
      },
    );
    scope.addContentTypeParser(
      "application/octet-stream",
      (_request, payload, done) => done(null, payload),
    );
    scope.post<{ Params: Own }>(
      "/api/rooms/:roomId/network-test/upload",
      async (request, reply) => {
        const current = valid(
          request.params.roomId,
          request.cookies.fs_session,
          request.headers["x-network-probe"],
          "upload",
        );
        if (!current)
          return reply.code(409).send({ error: "Connection test expired." });
        current.phase = "receiving";
        const deadline = setTimeout(() => request.raw.destroy(), 6_000);
        deadline.unref();
        reply.raw.once("close", () => clearTimeout(deadline));
        const start = performance.now();
        let bytes = 0;
        try {
          for await (const chunk of request.body as Readable) {
            bytes += Buffer.byteLength(chunk);
            if (bytes > PROBE_BYTES || performance.now() - start > 4_000)
              return reply
                .code(413)
                .send({ error: "Connection test limit exceeded." });
            if (
              lease !== current ||
              !member(request.params.roomId, request.cookies.fs_session)
            )
              return reply.code(401).send({ error: "Room access ended." });
          }
          const seconds = (performance.now() - start) / 1_000;
          return {
            bytes,
            seconds,
            bps: Math.min(
              PROBE_BPS,
              Math.floor((bytes * 8) / Math.max(seconds, 0.001)),
            ),
          };
        } finally {
          if (lease === current) lease = undefined;
        }
      },
    );
    scope.delete<{ Params: Own }>(
      "/api/rooms/:roomId/network-test",
      async (request, reply) => {
        if (
          lease &&
          lease.room === request.params.roomId &&
          lease.token === request.headers["x-network-probe"] &&
          member(lease.room, request.cookies.fs_session)?.participant.id ===
            lease.owner
        )
          lease = undefined;
        return reply.code(204).send();
      },
    );
  });
}
