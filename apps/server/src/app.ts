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
  HARDWARE_DECODERS_ATTRIBUTE,
  nativeMediaCapabilitiesSchema,
  deviceIdSchema,
} from "@podetelar/contracts";
import type { Config } from "./config.js";
import { Rooms, SESSION_TTL_MS } from "./rooms.js";
import {
  LivekitDirectory,
  mediaRoomName,
  type MediaDirectory,
} from "./media.js";
import { Previews, previewRoutes } from "./previews.js";
import {
  Access,
  accessRoutes,
  administrator,
  ACCESS_SWEEP_MS,
} from "./access.js";
import { Signaling } from "./signaling.js";
import { loadPublisher, publisherRoutes } from "./distribution.js";
import { networkTestRoutes } from "./network-test.js";
import { adminRoutes } from "./admin.js";
import { Usage } from "./usage.js";
import { streamDiagnosticsRoutes } from "./stream-diagnostics.js";
import { appLinkRoutes } from "./app-links.js";
import { RoomLifecycle } from "./room-lifecycle.js";

import { discordRoutes } from "./discord.js";
import { GuestProfiles } from "./profiles.js";
import { guestProfileRoutes } from "./profile-routes.js";
import { DiscordVoice } from "./discord-voice.js";
import { channelRoutes, ChannelAnnouncements } from "./channels.js";
import { Accounts, accountBearer } from "./accounts.js";
import { Identities } from "./identities.js";
import { discordAuthRoutes } from "./auth.js";
import { VoiceFollowing } from "./voice-following.js";
import { z } from "zod";

const COOKIE = "podetelar_session";
const MEDIA_TOKEN_TTL_SECONDS = 120;

export async function buildApp(
  config: Config,
  options: {
    now?: () => number;
    logger?: boolean;
    mediaDirectory?: MediaDirectory;
    usagePolling?: boolean;
    discordVoice?: DiscordVoice;
    discordFetch?: typeof fetch;
  } = {},
) {
  const publisher = await loadPublisher(config.publisherDirectory);
  const app = Fastify({
    logger: options.logger ?? false,
    logController: new LogController({ disableRequestLogging: true }),
    bodyLimit: 4096,
    trustProxy: config.trustedProxy ?? false,
  });
  const rooms = new Rooms(config.databasePath, options.now, config.adminToken);
  const profiles = new GuestProfiles(config.databasePath, options.now);
  const accounts = new Accounts(
    config.databasePath,
    config.publicUrl,
    options.now,
  );
  const discordVoice = options.discordVoice ?? new DiscordVoice(config.discord);
  const announcements = new ChannelAnnouncements(config, rooms, discordVoice);
  const identities = new Identities(
    config.databasePath,
    rooms,
    accounts,
    profiles,
    discordVoice,
    options.now,
  );
  if (!options.discordVoice) void discordVoice.start();
  const usage = new Usage(
    config.databasePath,
    config.usageMetricsUrl ?? "http://127.0.0.1:6789/metrics",
    options.now,
  );
  if (options.usagePolling !== false) usage.start();
  const directory = options.mediaDirectory ?? new LivekitDirectory(config);
  const previews = new Previews(directory, options.now);
  const signaling = new Signaling(rooms);
  const access = new Access(rooms, directory, previews, () =>
    signaling.closeRevoked(),
  );
  const following = new VoiceFollowing(
    config.databasePath,
    rooms,
    accounts,
    discordVoice,
    access,
  );
  app.addHook("preHandler", async () => {
    following.reconcile();
  });
  const lifecycle = new RoomLifecycle(rooms, directory, access);
  const lifecycleSweep = setInterval(() => {
    void lifecycle.tick();
    void announcements.tick();
  }, 5000);
  lifecycleSweep.unref();
  const previewSweep = setInterval(() => {
    following.reconcile();
    previews.sweep();
    void profiles.syncNames(directory);
    void identities.refresh();
    void access.settle();
  }, ACCESS_SWEEP_MS);
  previewSweep.unref();
  app.addHook("preClose", async () => {
    await discordVoice.close();
    clearInterval(lifecycleSweep);
    clearInterval(previewSweep);
    signaling.close();
  });
  app.addHook("onClose", async () => {
    clearInterval(lifecycleSweep);
    await lifecycle.settle();
    await announcements.close();
    clearInterval(previewSweep);
    signaling.close();
    await access.settle();
    previews.clear();
    await usage.close();
    await identities.close();
    following.close();
    await profiles.close();
    accounts.close();
    rooms.close();
  });
  await app.register(cookie);
  await app.register(rateLimit, {
    max: 120,
    timeWindow: "1 minute",
    // Documents/chunks must remain loadable when an API client is throttled.
    allowList: (request) =>
      !request.url.startsWith("/api/") && !request.url.startsWith("/media/"),
  });
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
    // A same-origin form POST on the direct download page must retain Origin;
    // Chromium sends Origin: null with no-referrer for this navigation. The
    // ticket is removed from the fragment and never occurs in a Referer URL.
    reply.header(
      "Referrer-Policy",
      request.url === "/download" ? "same-origin" : "no-referrer",
    );
    reply.header("X-Content-Type-Options", "nosniff");
    reply.header("X-Frame-Options", "DENY");
    reply.header(
      "Content-Security-Policy",
      `default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data: blob:; media-src 'self' blob:; connect-src 'self' ${mediaOrigin} ${mediaHttpOrigin}; base-uri 'none'; frame-ancestors 'none'; form-action 'self'`,
    );
    if (
      !["GET", "HEAD", "OPTIONS"].includes(request.method) &&
      !(
        config.discord &&
        request.method === "POST" &&
        request.url === "/api/discord/interactions"
      ) &&
      request.headers.origin !== config.publicUrl &&
      request.headers.origin !== config.directUrl
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

  await discordRoutes(app, config, rooms, options.now, discordVoice);
  await discordAuthRoutes(app, config, accounts, options.discordFetch);
  channelRoutes(app, config, rooms, discordVoice);
  app.get("/api/health", async () => ({ status: "ok", phase: 8 }));
  // No credentials or cookies are needed to discover the operator's direct origin.
  app.get("/api/connection", async () => ({ directUrl: config.directUrl }));
  app.addHook("onRequest", async (request, reply) => {
    if (config.directUrl === config.publicUrl) return;
    // Do not redirect authenticated probes: cookies must be issued on the direct
    // origin at admission. Also keep old clients from testing the CDN route.
    const path = request.url.split("?")[0]!;
    if (
      (/^\/api\/rooms\/[^/]+\/network-test(?:\/|$)/.test(path) ||
        path === "/api/publisher/download") &&
      request.headers.host?.toLowerCase() !== new URL(config.directUrl).host
    ) {
      return reply
        .code(421)
        .send({ error: "Use the direct server connection for this request." });
    }
  });
  await adminRoutes(
    app,
    config,
    usage,
    rooms,
    access,
    lifecycle,
    options.now,
    profiles,
    discordVoice,
    accounts,
    identities,
    following,
  );
  await guestProfileRoutes(
    app,
    rooms,
    profiles,
    directory,
    identities,
    accounts,
  );
  appLinkRoutes(app, config, rooms, directory, options.now);
  streamDiagnosticsRoutes(app, rooms, directory, options.now);
  app.get("/api/diagnostics", async (request, reply) => {
    if (!administrator(request, config))
      return reply.code(401).send({ error: "Server access key required." });
    return { signaling: signaling.diagnostics() };
  });

  app.post(
    "/api/rooms",
    { config: { rateLimit: { max: 5, timeWindow: "1 minute" } } },
    async (request, reply) => {
      if (!administrator(request, config))
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
      profiles.bind(
        issued.snapshot.room.id,
        issued.snapshot.participant.id,
        issued.snapshot.participant.displayName,
        input.data.deviceId,
      );
      usage.remember(
        issued.snapshot.room.id,
        issued.snapshot.participant.id,
        issued.snapshot.participant.displayName,
        input.data.deviceId,
      );
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
      const accountToken = accountBearer(request.headers.authorization),
        account = accounts.authenticate(accountToken);
      if (request.headers.authorization && !account)
        return reply.code(401).send({
          error: "Discord login expired. Sign in again or continue as a guest.",
        });
      const display = account
        ? await identities.display(id.data, account)
        : null;
      const issued = rooms.join(
        id.data,
        input.data.invitation,
        display?.displayName ?? input.data.displayName,
      );
      if (!issued)
        return reply
          .code(404)
          .send({ error: "This invitation could not be accepted." });
      profiles.bind(
        id.data,
        issued.snapshot.participant.id,
        issued.snapshot.participant.displayName,
        input.data.deviceId,
      );
      if (account)
        await identities.bind(
          id.data,
          issued.snapshot.participant.id,
          account,
          display!,
        );
      // Rejoining from this browser replaces its previous session for this room.
      usage.remember(
        issued.snapshot.room.id,
        issued.snapshot.participant.id,
        issued.snapshot.participant.displayName,
        input.data.deviceId,
      );
      const previous = rooms.session(id.data, request.cookies[COOKIE]);
      if (previous) previews.removeOwner(id.data, previous.participant.id);
      rooms.leave(id.data, request.cookies[COOKIE]);
      await access.settle();
      reply.setCookie(COOKIE, issued.token, cookieOptions(id.data));
      return issued.snapshot;
    },
  );

  const voiceBudget = {
    max: 60,
    timeWindow: "1 minute",
    keyGenerator: (request: import("fastify").FastifyRequest) =>
      accounts.authenticate(accountBearer(request.headers.authorization))
        ?.sessionId ?? request.ip,
  };
  app.get(
    "/api/auth/voice",
    { config: { rateLimit: voiceBudget } },
    async (request, reply) => {
      const account = accounts.authenticate(
        accountBearer(request.headers.authorization),
      );
      if (!account)
        return reply.code(401).send({ error: "Sign in with Discord." });
      const lookup = discordVoice.index.lookup(account.userId);
      return {
        ...lookup,
        enabled: following.enabled(account.userId),
        locations: lookup.locations.map((c) => ({
          ...c,
          enabled: following.channelEnabled(c.guildId, c.channelId),
        })),
      };
    },
  );
  app.post(
    "/api/auth/voice/join",
    { config: { rateLimit: { ...voiceBudget, max: 10 } } },
    async (request, reply) => {
      const account = accounts.authenticate(
        accountBearer(request.headers.authorization),
      );
      if (!account)
        return reply.code(401).send({ error: "Sign in with Discord." });
      const input = z
        .object({
          guildId: z.string().regex(/^\d{17,20}$/),
          channelId: z.string().regex(/^\d{17,20}$/),
          deviceId: deviceIdSchema,
        })
        .strict()
        .safeParse(request.body);
      if (!input.success)
        return reply.code(400).send({ error: "Choose a voice channel." });
      const { guildId, channelId, deviceId } = input.data;
      if (!following.eligible(account.userId, guildId, channelId))
        return reply
          .code(409)
          .send({ error: "Join an available Discord voice channel first." });
      const channel = discordVoice.index.channel(guildId, channelId)!;
      const resolved = rooms.resolveChannel(
        guildId,
        channelId,
        channel.channelName,
      );
      if (!resolved)
        return reply
          .code(409)
          .send({ error: "Automatic access to this room is disabled." });
      const display = await identities.display(resolved.room.id, account);
      if (
        !accounts.bySession(account.sessionId) ||
        !following.eligible(account.userId, guildId, channelId)
      )
        return reply
          .code(409)
          .send({ error: "Your Discord voice state changed. Try again." });
      const issued = rooms.join(
        resolved.room.id,
        resolved.invitation,
        display.displayName,
      );
      if (!issued)
        return reply.code(409).send({ error: "This room is unavailable." });
      const room = issued.snapshot.room.id,
        participant = issued.snapshot.participant.id;
      profiles.bind(room, participant, display.displayName, deviceId);
      await identities.bind(room, participant, account, display);
      if (
        !following.attach({
          roomId: room,
          participantId: participant,
          userId: account.userId,
          loginId: account.sessionId,
          guildId,
          channelId,
        })
      ) {
        rooms.revoke(room, participant);
        await access.settle();
        return reply
          .code(409)
          .send({ error: "Your Discord voice state changed. Try again." });
      }
      usage.remember(room, participant, display.displayName, deviceId);
      reply.setCookie(COOKIE, issued.token, cookieOptions(room));
      const secret = rooms.channelGuestEnabled(guildId, channelId)
        ? rooms.channelInvitation(guildId, channelId, channel.channelName)
        : null;
      return {
        ...issued.snapshot,
        invitationUrl: secret
          ? `${config.publicUrl}/c/${guildId}/${channelId}#invite=${secret}`
          : null,
      };
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
      await access.settle();
      reply.clearCookie(COOKIE, {
        path: `/api/rooms/${request.params.roomId}`,
      });
      return reply.code(204).send();
    },
  );

  for (const native of [false, true])
    app.post<{ Params: { roomId: string } }>(
      native
        ? "/api/rooms/:roomId/native-media-token"
        : "/api/rooms/:roomId/media-token",
      async (request, reply) => {
        // Fetch Metadata headers are browser-controlled and cannot be removed by
        // page JavaScript. This is a supported-client policy, not binary attestation.
        if (
          native &&
          (request.headers["sec-fetch-mode"] ||
            request.headers["sec-fetch-site"])
        )
          return reply.code(403).send({
            error: "Use the PodeTelar desktop app to share and watch.",
          });
        const session = rooms.session(
          request.params.roomId,
          request.cookies[COOKIE],
        );
        if (!session)
          return reply
            .code(401)
            .send({ error: "Open your room invitation to join." });
        const capabilities =
          native && request.body !== undefined
            ? nativeMediaCapabilitiesSchema.safeParse(request.body)
            : undefined;
        if (capabilities && !capabilities.success)
          return reply
            .code(400)
            .send({ error: "Invalid hardware decoder capabilities." });
        const access = new AccessToken(
          config.livekitApiKey,
          config.livekitApiSecret,
          {
            identity: session.participant.id,
            name: session.participant.displayName,
            ttl: MEDIA_TOKEN_TTL_SECONDS,
            // Initial participant attributes close the connect/announcement
            // race. Missing legacy capabilities remain unknown, not H.264-only.
            ...(capabilities?.success
              ? {
                  attributes: {
                    [HARDWARE_DECODERS_ATTRIBUTE]:
                      capabilities.data.hardwareDecoders.join(",") || "none",
                  },
                }
              : {}),
          },
        );
        access.addGrant({
          roomJoin: true,
          room: mediaRoomName(session.room.id),
          canPublish: native,
          canSubscribe: native,
          canUpdateOwnMetadata: native,
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
  await networkTestRoutes(app, rooms);
  await publisherRoutes(app, publisher, rooms, config, options.now);
  await accessRoutes(app, config, rooms, access);
  await signaling.register(app, config);

  if (existsSync(config.webRoot)) {
    await app.register(fastifyStatic, {
      root: config.webRoot,
      wildcard: false,
    });
  }
  app.setNotFoundHandler((request, reply) => {
    if (
      request.method === "GET" &&
      (/^\/(?:r|manage)\/[A-Za-z0-9_-]{22}(?:\?.*)?$/.test(request.url) ||
        request.url === "/download") &&
      existsSync(config.webRoot)
    )
      return reply.sendFile("index.html");
    return reply.code(404).send({ error: "Not found." });
  });
  return app;
}
