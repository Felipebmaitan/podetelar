import { createHash, randomBytes } from "node:crypto";
import type { FastifyInstance, FastifyRequest } from "fastify";
import type { Config } from "./config.js";
import {
  adminCreateRoomSchema,
  roomIdSchema,
  streamPolicySchema,
} from "@podetelar/contracts";
import { Rooms, secretsEqual } from "./rooms.js";
import type { Usage } from "./usage.js";
import type { Access } from "./access.js";
import type { RoomLifecycle } from "./room-lifecycle.js";
import type { GuestProfiles } from "./profiles.js";
import { z } from "zod";
import type { DiscordVoice } from "./discord-voice.js";
import { channelParams } from "./channels.js";
import type { Accounts } from "./accounts.js";
import type { Identities } from "./identities.js";
import type { VoiceFollowing } from "./voice-following.js";
import { hasStreamCaps, type StreamPolicies } from "./stream-policy.js";

const COOKIE = "podetelar_admin";
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
  streamPolicies: StreamPolicies,
  now = Date.now,
  profiles?: GuestProfiles,
  discordVoice?: DiscordVoice,
  accounts?: Accounts,
  identities?: Identities,
  following?: VoiceFollowing,
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
  await app.register(async (scope) => {
    scope.addHook("preHandler", async (request, reply) => {
      if (!authenticated(request))
        return reply.code(401).send({ error: "Sign in as administrator." });
    });
    scope.get("/api/admin/stream-policy", async () => ({
      policy: streamPolicies.current(),
      enforcement: Boolean(config.streamPolicyEnforcement),
      cameraEnforcement:
        Boolean(config.streamPolicyEnforcement) &&
        streamPolicies.cameraEnforcementReady(),
    }));
    scope.put("/api/admin/stream-policy", async (request, reply) => {
      const policy = streamPolicySchema.safeParse(request.body);
      if (!policy.success)
        return reply.code(400).send({
          error: "Choose valid stream limits and bitrate targets (1–100 Mbps).",
        });
      const current = streamPolicies.current();
      const next = { ...current, ...policy.data };
      const changesCameraCap =
        (next.maxCameraBitrateBps ?? null) !==
        (current.maxCameraBitrateBps ?? null);
      const changesCaps =
        policy.data.maxBitrateBps !== current.maxBitrateBps ||
        policy.data.maxResolution !== current.maxResolution ||
        policy.data.maxFps !== current.maxFps;
      const requiresEnforcement =
        (changesCaps || changesCameraCap) && hasStreamCaps(next);
      if (requiresEnforcement && !config.streamPolicyEnforcement)
        return reply.code(409).send({
          error:
            "Configure a policy-enabled media server before setting limits.",
        });
      if (requiresEnforcement && !streamPolicies.enforcementReady())
        return reply.code(409).send({
          error:
            "Wait for the media server to synchronize stream policy, then try again.",
        });
      if (
        changesCameraCap &&
        next.maxCameraBitrateBps != null &&
        !streamPolicies.cameraEnforcementReady()
      )
        return reply.code(409).send({
          error:
            "The media server must support camera bitrate limits before you can set one.",
        });
      const saved = streamPolicies.save(next);
      if (!saved)
        return reply.code(409).send({
          error: "The policy changed. Reload this section before editing.",
        });
      return {
        policy: saved,
        enforcement: Boolean(config.streamPolicyEnforcement),
        cameraEnforcement:
          Boolean(config.streamPolicyEnforcement) &&
          streamPolicies.cameraEnforcementReady(),
      };
    });
  });
  if (accounts)
    await app.register(async (scope) => {
      scope.addHook("preHandler", async (request, reply) => {
        if (!authenticated(request))
          return reply.code(401).send({ error: "Sign in as administrator." });
      });
      scope.get<{ Querystring: { offset?: string } }>(
        "/api/admin/accounts",
        async (request, reply) => {
          const offset = request.query.offset ?? "0";
          if (!/^(0|[1-9][0-9]{0,8})$/.test(offset))
            return reply
              .code(400)
              .send({ error: "Invalid account directory offset." });
          return accounts.directory(Number(offset));
        },
      );
      scope.get<{ Params: { userId: string } }>(
        "/api/admin/accounts/:userId/sessions",
        async (request, reply) => {
          if (!/^\d{17,20}$/.test(request.params.userId))
            return reply.code(400).send({ error: "Invalid account." });
          return { sessions: accounts.sessions(request.params.userId) };
        },
      );
      scope.post<{ Params: { userId: string } }>(
        "/api/admin/accounts/:userId",
        async (request, reply) => {
          const input = z
            .object({
              action: z.enum([
                "disable",
                "enable",
                "revoke",
                "delete",
                "hide-avatar",
                "show-avatar",
                "revoke-session",
                "refresh-profile",
              ]),
              sessionId: z.uuid().optional(),
            })
            .strict()
            .safeParse(request.body);
          if (!/^\d{17,20}$/.test(request.params.userId) || !input.success)
            return reply
              .code(400)
              .send({ error: "Choose a valid account action." });
          if (input.data.action === "refresh-profile") {
            await identities?.refresh(request.params.userId);
            return reply.code(204).send();
          }
          const changed =
            input.data.action === "revoke-session"
              ? Boolean(
                  input.data.sessionId &&
                  accounts.revokeSession(
                    request.params.userId,
                    input.data.sessionId,
                  ),
                )
              : accounts.manage(request.params.userId, input.data.action);
          if (!changed)
            return reply
              .code(404)
              .send({ error: "Account or login unavailable." });
          return reply.code(204).send();
        },
      );
    });
  if (discordVoice)
    await app.register(async (scope) => {
      scope.addHook("preHandler", async (request, reply) => {
        if (!authenticated(request))
          return reply.code(401).send({ error: "Sign in as administrator." });
      });
      const installation = config.discord
        ? {
            url: `https://discord.com/oauth2/authorize?${new URLSearchParams({
              client_id: config.discord.applicationId,
              integration_type: "0",
              scope: "bot applications.commands",
              // View Channels and Send Messages for room announcements.
              permissions: "3072",
            })}`,
            guildsRestricted: !config.discord.guildIds.includes("*"),
          }
        : null;
      scope.get("/api/admin/discord", async () => ({
        ...discordVoice.status(),
        installation,
      }));
      if (following) {
        scope.get("/api/admin/discord/following", async () =>
          following.directory(),
        );
        scope.post("/api/admin/discord/following", async (request, reply) => {
          const input = z
            .object({
              action: z.enum(["enable", "disable", "block", "allow"]),
              userId: z
                .string()
                .regex(/^\d{17,20}$/)
                .optional(),
            })
            .strict()
            .safeParse(request.body);
          if (
            !input.success ||
            (["block", "allow"].includes(input.data.action) &&
              !input.data.userId)
          )
            return reply
              .code(400)
              .send({ error: "Choose an automatic-joining action." });
          following.manage(input.data.action, input.data.userId);
          await access.settle();
          return following.directory();
        });
      }
      scope.get("/api/admin/discord/channels", async () => ({
        channels: rooms.channelDirectory(),
      }));
      scope.post<{ Params: { guildId: string; channelId: string } }>(
        "/api/admin/discord/channels/:guildId/:channelId",
        async (request, reply) => {
          const params = channelParams.safeParse(request.params),
            body = z
              .object({
                action: z.enum([
                  "rotate",
                  "disable",
                  "enable",
                  "pause",
                  "resume",
                  "invitation",
                  "repair",
                ]),
              })
              .strict()
              .safeParse(request.body);
          if (!params.success || !body.success)
            return reply
              .code(400)
              .send({ error: "Choose a valid channel action." });
          const { guildId, channelId } = params.data,
            channel = discordVoice.index.channel(guildId, channelId);
          if (channel)
            rooms.channelInvitation(guildId, channelId, channel.channelName);
          if (body.data.action === "invitation") {
            if (!channel)
              return reply
                .code(503)
                .send({ error: "Discord channel unavailable." });
            const secret = rooms.channelInvitation(
              guildId,
              channelId,
              channel.channelName,
            );
            if (!secret)
              return reply.code(409).send({
                error: "Rotate the channel invitation to recover access.",
              });
            return {
              invitationUrl: `${config.publicUrl}/c/${guildId}/${channelId}#invite=${secret}`,
            };
          }
          if (body.data.action === "repair") {
            rooms.announcementResult(guildId, channelId, null, null, false);
            return reply.code(204).send();
          }
          if (!rooms.manageChannel(guildId, channelId, body.data.action))
            return reply.code(404).send({ error: "Channel unavailable." });
          following?.reconcile();
          await access.settle();
          return reply.code(204).send();
        },
      );
      scope.post(
        "/api/admin/discord/reconnect",
        { config: { rateLimit: { max: 2, timeWindow: "1 minute" } } },
        async () => {
          void discordVoice.reconnect();
          return { reconnecting: true };
        },
      );
    });
  if (profiles) {
    await app.register(async (scope) => {
      scope.addHook("preHandler", async (request, reply) => {
        if (!authenticated(request))
          return reply.code(401).send({ error: "Sign in as administrator." });
      });
      scope.get<{ Querystring: { offset?: string } }>(
        "/api/admin/profiles",
        async (request, reply) => {
          const offset = request.query.offset ?? "0";
          if (!/^(0|[1-9][0-9]{0,8})$/.test(offset))
            return reply
              .code(400)
              .send({ error: "Invalid profile directory offset." });
          return profiles.directory(Number(offset));
        },
      );
      scope.post<{ Params: { profileId: string } }>(
        "/api/admin/profiles/:profileId",
        async (request, reply) => {
          const action = z
            .object({ action: z.enum(["delete", "block", "allow"]) })
            .strict()
            .safeParse(request.body);
          if (
            !z.uuid().safeParse(request.params.profileId).success ||
            !action.success
          )
            return reply
              .code(400)
              .send({ error: "Choose a valid profile action." });
          if (!profiles.moderate(request.params.profileId, action.data.action))
            return reply.code(404).send({ error: "Profile unavailable." });
          return reply.code(204).send();
        },
      );
      scope.get<{ Params: { profileId: string; revision: string } }>(
        "/api/admin/profiles/:profileId/avatar/:revision",
        async (request, reply) => {
          const { profileId, revision } = request.params;
          if (
            !z.uuid().safeParse(profileId).success ||
            !z.uuid().safeParse(revision).success
          )
            return reply
              .code(404)
              .send({ error: "Profile picture unavailable." });
          const image = profiles.adminAvatar(profileId, revision);
          if (!image)
            return reply
              .code(404)
              .send({ error: "Profile picture unavailable." });
          return reply.type(image.type).send(image.bytes);
        },
      );
    });
  }
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
