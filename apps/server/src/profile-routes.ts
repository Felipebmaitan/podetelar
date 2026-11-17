import { createHash } from "node:crypto";
import type { FastifyInstance, FastifyRequest } from "fastify";
import {
  AVATAR_MAX_BYTES,
  guestNameSchema,
  participantIdSchema,
  roomIdSchema,
} from "@podetelar/contracts";
import type { Rooms } from "./rooms.js";
import type { GuestProfiles } from "./profiles.js";
import type { MediaDirectory } from "./media.js";

export async function guestProfileRoutes(
  app: FastifyInstance,
  rooms: Rooms,
  profiles: GuestProfiles,
  directory: MediaDirectory,
) {
  const keyGenerator = (request: FastifyRequest) => {
    const room = (request.params as { roomId: string }).roomId;
    const token = request.cookies.podetelar_session;
    return token && rooms.session(room, token)
      ? createHash("sha256").update(token).digest("hex")
      : `anonymous:${request.ip}`;
  };
  const profileLimit = { max: 120, timeWindow: "1 minute", keyGenerator };
  await app.register(async (scope) => {
    scope.addContentTypeParser(
      "application/octet-stream",
      { parseAs: "buffer", bodyLimit: AVATAR_MAX_BYTES },
      (_request, body, done) => done(null, body),
    );
    scope.addHook("preHandler", async (request, reply) => {
      const room = (request.params as { roomId: string }).roomId;
      if (
        !roomIdSchema.safeParse(room).success ||
        !rooms.session(room, request.cookies.podetelar_session)
      )
        return reply
          .code(401)
          .send({ error: "Join a room to manage your profile." });
    });
    scope.get<{ Params: { roomId: string } }>(
      "/api/rooms/:roomId/profiles",
      { config: { rateLimit: profileLimit } },
      async (request) => ({ profiles: profiles.list(request.params.roomId) }),
    );
    scope.patch<{ Params: { roomId: string } }>(
      "/api/rooms/:roomId/profile",
      { config: { rateLimit: profileLimit } },
      async (request, reply) => {
        const input = guestNameSchema.safeParse(request.body);
        if (!input.success)
          return reply
            .code(400)
            .send({ error: "Enter a name of up to 60 characters." });
        const room = request.params.roomId,
          participant = rooms.session(room, request.cookies.podetelar_session)!
            .participant.id;
        profiles.rename(room, participant, input.data.displayName);
        await profiles.syncNames(directory);
        return { displayName: input.data.displayName };
      },
    );
    scope.put<{ Params: { roomId: string } }>(
      "/api/rooms/:roomId/profile/avatar",
      {
        bodyLimit: AVATAR_MAX_BYTES,
        config: { rateLimit: { ...profileLimit, max: 6 } },
      },
      async (request, reply) => {
        if (
          request.headers["content-type"] !== "application/octet-stream" ||
          !Buffer.isBuffer(request.body)
        )
          return reply.code(400).send({
            error: "Select a PNG, JPEG, WebP or GIF image, up to 3 MB.",
          });
        const room = request.params.roomId,
          participant = rooms.session(room, request.cookies.podetelar_session)!
            .participant.id;
        const result = await profiles.upload(room, participant, request.body);
        if (result !== "ok")
          return reply
            .code(
              result === "blocked"
                ? 403
                : result === "unavailable"
                  ? 409
                  : result === "busy" || result === "full"
                    ? 503
                    : result === "changed"
                      ? 409
                      : 400,
            )
            .send({
              error:
                result === "blocked"
                  ? "Profile picture uploads are disabled for this PC."
                  : result === "changed"
                    ? "The profile changed. Refresh before uploading again."
                    : result === "busy" || result === "full"
                      ? "Profile picture storage is temporarily unavailable."
                      : "Select a valid PNG, JPEG, WebP or GIF image, up to 3 MB, 200 frames and 16 million decoded pixels.",
            });
        return { profiles: profiles.list(room) };
      },
    );
    scope.delete<{ Params: { roomId: string } }>(
      "/api/rooms/:roomId/profile/avatar",
      { config: { rateLimit: profileLimit } },
      async (request, reply) => {
        const room = request.params.roomId,
          participant = rooms.session(room, request.cookies.podetelar_session)!
            .participant.id;
        profiles.remove(room, participant);
        return reply.code(204).send();
      },
    );
    scope.get<{
      Params: { roomId: string; participantId: string; revision: string };
    }>(
      "/api/rooms/:roomId/profiles/:participantId/avatar/:revision",
      { config: { rateLimit: profileLimit } },
      async (request, reply) => {
        const { roomId, participantId, revision } = request.params;
        if (
          !participantIdSchema.safeParse(participantId).success ||
          !participantIdSchema.safeParse(revision).success
        )
          return reply
            .code(404)
            .send({ error: "Profile picture unavailable." });
        const image = profiles.avatar(roomId, participantId, revision);
        if (!image)
          return reply
            .code(404)
            .send({ error: "Profile picture unavailable." });
        return reply.type(image.type).send(image.bytes);
      },
    );
  });
}
