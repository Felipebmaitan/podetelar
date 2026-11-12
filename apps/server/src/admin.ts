import { createHash, randomBytes } from "node:crypto";
import type { FastifyInstance, FastifyRequest } from "fastify";
import type { Config } from "./config.js";
import { adminCreateRoomSchema, roomIdSchema } from "@freedomstream/contracts";
import { Rooms, secretsEqual } from "./rooms.js";
import type { Usage } from "./usage.js";
import type { Access } from "./access.js";
import type { RoomLifecycle } from "./room-lifecycle.js";

const COOKIE = "fs_admin";
const TTL = 8 * 60 * 60 * 1000;
const hash = (value: string) =>
  createHash("sha256").update(value).digest("hex");

/** One administrator, opaque expiring cookies. Restart and credential rotation revoke access. */
export async function adminRoutes(
  app: FastifyInstance,
  config: Config,
  usage: Usage,
  rooms: Rooms,
  access: Access,
  lifecycle: RoomLifecycle,
  now = Date.now,
) {
  const sessions = new Map<string, number>();
  const cookie = {
    httpOnly: true,
    secure: config.publicUrl.startsWith("https:"),
    sameSite: "strict" as const,
    path: "/api/admin",
  };
  const authenticated = (request: FastifyRequest) => {
    const token = request.cookies[COOKIE];
    return Boolean(
      token &&
      /^[A-Za-z0-9_-]{43}$/.test(token) &&
      (sessions.get(hash(token)) ?? 0) > now(),
    );
  };
  app.post(
    "/api/admin/login",
    { config: { rateLimit: { max: 5, timeWindow: "1 minute" } } },
    async (request, reply) => {
      const body = request.body as { password?: unknown } | null;
      if (
        !body ||
        typeof body.password !== "string" ||
        body.password.length > 256 ||
        !secretsEqual(body.password, config.adminToken)
      )
        return reply
          .code(401)
          .send({ error: "Incorrect administrator password." });
      for (const [key, expiry] of sessions)
        if (expiry <= now()) sessions.delete(key);
      if (sessions.size >= 16) sessions.delete(sessions.keys().next().value!);
      const previous = request.cookies[COOKIE];
      if (previous) sessions.delete(hash(previous));
      const token = randomBytes(32).toString("base64url");
      sessions.set(hash(token), now() + TTL);
      reply.setCookie(COOKIE, token, { ...cookie, maxAge: TTL / 1000 });
      return { administrator: true };
    },
  );
  app.get("/api/admin/session", async (request, reply) => {
    if (!authenticated(request))
      return reply.code(401).send({ error: "Sign in as administrator." });
    return { administrator: true };
  });
  app.post("/api/admin/logout", async (request, reply) => {
    const token = request.cookies[COOKIE];
    if (token) sessions.delete(hash(token));
    reply.clearCookie(COOKIE, cookie);
    return reply.code(204).send();
  });
  app.get<{ Querystring: { offset?: string } }>(
    "/api/admin/rooms",
    async (request, reply) => {
      if (!authenticated(request))
        return reply.code(401).send({ error: "Sign in as administrator." });
      const offset = request.query.offset ?? "0";
      if (!/^(0|[1-9][0-9]{0,8})$/.test(offset))
        return reply
          .code(400)
          .send({ error: "Invalid room directory offset." });
      const page = rooms.list(Number(offset));
      const result = {
        ...page,
        rooms: await lifecycle.describePage(page.rooms),
      };
      if (!authenticated(request))
        return reply.code(401).send({ error: "Sign in as administrator." });
      return result;
    },
  );
  app.post<{ Params: { roomId: string } }>(
    "/api/admin/rooms/:roomId/invitation",
    async (request, reply) => {
      if (!authenticated(request))
        return reply.code(401).send({ error: "Sign in as administrator." });
      const id = roomIdSchema.safeParse(request.params.roomId);
      if (!id.success || !rooms.lifecycle(id.data))
        return reply.code(404).send({ error: "Room unavailable." });
      const invitation = rooms.invitation(id.data);
      if (!invitation)
        return reply.code(409).send({
          error:
            "This older invitation cannot be recovered. Create a new invite to replace it.",
        });
      return {
        invitationUrl: `${config.publicUrl}/r/${id.data}#invite=${invitation}`,
      };
    },
  );
  app.post<{ Params: { roomId: string } }>(
    "/api/admin/rooms/:roomId/invitation/rotate",
    async (request, reply) => {
      if (!authenticated(request))
        return reply.code(401).send({ error: "Sign in as administrator." });
      const id = roomIdSchema.safeParse(request.params.roomId);
      if (!id.success)
        return reply.code(404).send({ error: "Room unavailable." });
      const changed = rooms.change(id.data, {
        invitation: "rotate",
        revokeSessions: false,
      });
      if (!changed) return reply.code(404).send({ error: "Room unavailable." });
      return {
        invitationUrl: `${config.publicUrl}/r/${id.data}#invite=${changed.invitation}`,
      };
    },
  );
  app.delete<{ Params: { roomId: string } }>(
    "/api/admin/rooms/:roomId",
    async (request, reply) => {
      if (!authenticated(request))
        return reply.code(401).send({ error: "Sign in as administrator." });
      const id = roomIdSchema.safeParse(request.params.roomId);
      if (!id.success || !rooms.closeRoom(id.data))
        return reply.code(404).send({ error: "Room unavailable." });
      void access.settle();
      return reply
        .code(rooms.pending(id.data) ? 202 : 200)
        .send({ pendingDisconnects: rooms.pending(id.data) });
    },
  );
  app.post(
    "/api/admin/rooms",
    { config: { rateLimit: { max: 5, timeWindow: "1 minute" } } },
    async (request, reply) => {
      if (!authenticated(request))
        return reply.code(401).send({ error: "Sign in as administrator." });
      const input = adminCreateRoomSchema.safeParse(request.body);
      if (!input.success)
        return reply
          .code(400)
          .send({ error: "Enter a room name of up to 60 characters." });
      const created = rooms.createManaged(input.data.name);
      return reply.code(201).send({
        room: created.room,
        invitationUrl: `${config.publicUrl}/r/${created.room.id}#invite=${created.invitation}`,
      });
    },
  );
  app.get<{ Querystring: { from?: string; to?: string; page?: string } }>(
    "/api/admin/usage",
    async (request, reply) => {
      if (!authenticated(request))
        return reply.code(401).send({ error: "Sign in as administrator." });
      const from = Number(request.query.from),
        to = Number(request.query.to),
        page = Number(request.query.page ?? 0);
      if (
        !Number.isSafeInteger(from) ||
        !Number.isSafeInteger(to) ||
        from < 0 ||
        to <= from ||
        to - from > 366 * 86400000 ||
        to > now() + 86400000 ||
        !Number.isSafeInteger(page) ||
        page < 0 ||
        page > 100000
      )
        return reply
          .code(400)
          .send({ error: "Choose a valid period of up to 366 days." });
      return reply
        .type("application/json; charset=utf-8")
        .send(usage.reportJson(from, to, page));
    },
  );
}
