import { existsSync } from "node:fs";
import Fastify, { LogController } from "fastify";
import cookie from "@fastify/cookie";
import rateLimit from "@fastify/rate-limit";
import fastifyStatic from "@fastify/static";
import { AccessToken } from "livekit-server-sdk";
import {
  createRoomSchema,
  joinRoomSchema,
  roomIdSchema,
} from "@freedomstream/contracts";
import type { Config } from "./config.js";
import { Rooms, secretsEqual, SESSION_TTL_MS } from "./rooms.js";
import {
  LivekitDirectory,
  mediaRoomName,
  type MediaDirectory,
} from "./media.js";
import { Previews, previewRoutes } from "./previews.js";

const COOKIE = "fs_session";
const MEDIA_TOKEN_TTL_SECONDS = 120;

export async function buildApp(
  config: Config,
  options: {
    now?: () => number;
    logger?: boolean;
    mediaDirectory?: MediaDirectory;
  } = {},
) {
  const app = Fastify({
    logger: options.logger ?? false,
    logController: new LogController({ disableRequestLogging: true }),
    bodyLimit: 4096,
  });
  const rooms = new Rooms(config.databasePath, options.now);
  const previews = new Previews(
    options.mediaDirectory ?? new LivekitDirectory(config),
    options.now,
  );
  const previewSweep = setInterval(() => previews.sweep(), 30_000);
  previewSweep.unref();
  app.addHook("onClose", async () => {
    clearInterval(previewSweep);
    previews.clear();
    rooms.close();
  });
  await app.register(cookie);
  await app.register(rateLimit, { max: 120, timeWindow: "1 minute" });
  const cookieOptions = (id: string) => ({
    httpOnly: true,
    secure: config.publicUrl.startsWith("https:"),
    sameSite: "strict" as const,
    path: `/api/rooms/${id}`,
    maxAge: SESSION_TTL_MS / 1000,
  });
  const mediaOrigin = new URL(config.livekitUrl).origin;
  const mediaHttpOrigin = mediaOrigin.replace(/^ws/, "http");

  app.addHook("onRequest", async (request, reply) => {
    reply.header("Cache-Control", "no-store");
    reply.header("Referrer-Policy", "no-referrer");
    reply.header("X-Content-Type-Options", "nosniff");
    reply.header("X-Frame-Options", "DENY");
    reply.header(
      "Content-Security-Policy",
      `default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data: blob:; media-src 'self' blob:; connect-src 'self' ${mediaOrigin} ${mediaHttpOrigin}; base-uri 'none'; frame-ancestors 'none'; form-action 'self'`,
    );
    if (
      !["GET", "HEAD", "OPTIONS"].includes(request.method) &&
      request.headers.origin !== config.publicUrl
    ) {
      return reply.code(403).send({ error: "Request origin is not allowed." });
    }
  });

  app.setErrorHandler((error, request, reply) => {
    const code =
      error instanceof Error &&
      "statusCode" in error &&
      typeof error.statusCode === "number"
        ? error.statusCode
        : 500;
    const status = code >= 400 && code < 500 ? code : 500;
    if (status === 500) request.log.error({ err: error }, "Request failed");
    return reply.code(status).send({
      error:
        status === 429
          ? "Too many attempts. Please try again shortly."
          : status === 500
            ? "Something went wrong. Please try again."
            : "The request could not be accepted.",
    });
  });

  app.get("/api/health", async () => ({ status: "ok", phase: 5 }));

  app.post(
    "/api/rooms",
    { config: { rateLimit: { max: 5, timeWindow: "1 minute" } } },
    async (request, reply) => {
      const authorization = request.headers.authorization;
      if (
        !authorization?.startsWith("Bearer ") ||
        !secretsEqual(authorization.slice(7), config.adminToken)
      )
        return reply
          .code(401)
          .send({ error: "Room creation requires the server access key." });
      const input = createRoomSchema.safeParse(request.body);
      if (!input.success)
        return reply.code(400).send({
          error:
            "Enter a room name and display name, each up to 60 characters.",
        });
      const issued = rooms.create(input.data);
      reply.setCookie(
        COOKIE,
        issued.token,
        cookieOptions(issued.snapshot.room.id),
      );
      return reply.code(201).send({
        ...issued.snapshot,
        invitationUrl: `${config.publicUrl}/r/${issued.snapshot.room.id}#invite=${issued.invitation}`,
      });
    },
  );

  app.post<{ Params: { roomId: string } }>(
    "/api/rooms/:roomId/join",
    { config: { rateLimit: { max: 10, timeWindow: "1 minute" } } },
    async (request, reply) => {
      const id = roomIdSchema.safeParse(request.params.roomId);
      const input = joinRoomSchema.safeParse(request.body);
      if (!id.success || !input.success)
        return reply.code(400).send({
          error: "Enter a display name and open a valid room invitation.",
        });
      const issued = rooms.join(
        id.data,
        input.data.invitation,
        input.data.displayName,
      );
      if (!issued)
        return reply
          .code(404)
          .send({ error: "This invitation could not be accepted." });
      // Rejoining from this browser replaces its previous session for this room.
      const previous = rooms.session(id.data, request.cookies[COOKIE]);
      if (previous) previews.removeOwner(id.data, previous.participant.id);
      rooms.leave(id.data, request.cookies[COOKIE]);
      reply.setCookie(COOKIE, issued.token, cookieOptions(id.data));
      return issued.snapshot;
    },
  );

  app.get<{ Params: { roomId: string } }>(
    "/api/rooms/:roomId/session",
    async (request, reply) => {
      const session = rooms.session(
        request.params.roomId,
        request.cookies[COOKIE],
      );
      if (!session)
        return reply
          .code(401)
          .send({ error: "Open your room invitation to join." });
      return session;
    },
  );

  app.post<{ Params: { roomId: string } }>(
    "/api/rooms/:roomId/leave",
    async (request, reply) => {
      if (!roomIdSchema.safeParse(request.params.roomId).success)
        return reply.code(404).send({ error: "Not found." });
      const member = rooms.session(
        request.params.roomId,
        request.cookies[COOKIE],
      );
      if (member)
        previews.removeOwner(request.params.roomId, member.participant.id);
      rooms.leave(request.params.roomId, request.cookies[COOKIE]);
      reply.clearCookie(COOKIE, {
        path: `/api/rooms/${request.params.roomId}`,
      });
      return reply.code(204).send();
    },
  );

  app.post<{ Params: { roomId: string } }>(
    "/api/rooms/:roomId/media-token",
    async (request, reply) => {
      const session = rooms.session(
        request.params.roomId,
        request.cookies[COOKIE],
      );
      if (!session)
        return reply
          .code(401)
          .send({ error: "Open your room invitation to join." });
      const access = new AccessToken(
        config.livekitApiKey,
        config.livekitApiSecret,
        {
          identity: session.participant.id,
          name: session.participant.displayName,
          ttl: MEDIA_TOKEN_TTL_SECONDS,
        },
      );
      access.addGrant({
        roomJoin: true,
        room: mediaRoomName(session.room.id),
        canPublish: true,
        canSubscribe: true,
        canPublishData: false,
      });
      return {
        serverUrl: config.livekitUrl,
        token: await access.toJwt(),
        expiresInSeconds: MEDIA_TOKEN_TTL_SECONDS,
      };
    },
  );

  await previewRoutes(app, rooms, previews);

  if (existsSync(config.webRoot)) {
    await app.register(fastifyStatic, {
      root: config.webRoot,
      wildcard: false,
    });
  }
  app.setNotFoundHandler((request, reply) => {
    if (
      request.method === "GET" &&
      /^\/r\/[A-Za-z0-9_-]{22}(?:\?.*)?$/.test(request.url) &&
      existsSync(config.webRoot)
    )
      return reply.sendFile("index.html");
    return reply.code(404).send({ error: "Not found." });
  });
  return app;
}
