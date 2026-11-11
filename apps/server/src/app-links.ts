import { createHash, randomBytes } from "node:crypto";
import type { FastifyInstance } from "fastify";
import { invitationSchema, roomIdSchema } from "@freedomstream/contracts";
import type { Config } from "./config.js";
import type { Rooms } from "./rooms.js";
import type { MediaDirectory } from "./media.js";

const hash = (value: string) =>
  createHash("sha256").update(value).digest("hex");
const TTL = 60_000;
/** Short-lived memory only. Never put permanent invitations in activation arguments. */
export function appLinkRoutes(
  app: FastifyInstance,
  config: Config,
  rooms: Rooms,
  directory: MediaDirectory,
  now = Date.now,
) {
  const tickets = new Map<
    string,
    { roomId: string; invitation: string; expires: number }
  >();
  const sweep = () => {
    for (const [key, ticket] of tickets)
      if (ticket.expires <= now()) tickets.delete(key);
  };
  const timer = setInterval(sweep, 15000);
  timer.unref();
  app.addHook("onClose", async () => {
    clearInterval(timer);
    tickets.clear();
  });
  app.post<{ Params: { roomId: string } }>(
    "/api/rooms/:roomId/invitation-preview",
    { config: { rateLimit: { max: 12, timeWindow: "1 minute" } } },
    async (request, reply) => {
      const id = roomIdSchema.safeParse(request.params.roomId);
      const invitation = invitationSchema.safeParse(
        (request.body as { invitation?: unknown } | null)?.invitation,
      );
      if (
        !id.success ||
        !invitation.success ||
        !rooms.acceptsInvitation(id.data, invitation.data)
      )
        return reply
          .code(404)
          .send({ error: "This invitation could not be accepted." });
      let members;
      try {
        members = await directory.members(id.data);
      } catch {
        return reply
          .code(503)
          .send({ error: "Could not load room members. Try again shortly." });
      }
      // Rotation or disabling during the SFU request must also deny disclosure.
      if (!rooms.acceptsInvitation(id.data, invitation.data))
        return reply
          .code(404)
          .send({ error: "This invitation could not be accepted." });
      return {
        members: members.filter((member) => rooms.active(id.data, member.id)),
      };
    },
  );
  app.post<{ Params: { roomId: string } }>(
    "/api/rooms/:roomId/app-link",
    { config: { rateLimit: { max: 5, timeWindow: "1 minute" } } },
    async (request, reply) => {
      const id = roomIdSchema.safeParse(request.params.roomId);
      const invitation = invitationSchema.safeParse(
        (request.body as { invitation?: unknown } | null)?.invitation,
      );
      if (
        !id.success ||
        !invitation.success ||
        !rooms.acceptsInvitation(id.data, invitation.data)
      )
        return reply
          .code(404)
          .send({ error: "This invitation could not be accepted." });
      sweep();
      if (tickets.size >= 128)
        return reply
          .code(503)
          .send({ error: "App handoff is busy. Try again shortly." });
      const ticket = randomBytes(32).toString("base64url");
      tickets.set(hash(ticket), {
        roomId: id.data,
        invitation: invitation.data,
        expires: now() + TTL,
      });
      return {
        appUrl: `freedomstream://join?server=${encodeURIComponent(config.publicUrl)}#handoff=${ticket}`,
      };
    },
  );
  app.post(
    "/api/app-links/redeem",
    { config: { rateLimit: { max: 10, timeWindow: "1 minute" } } },
    async (request, reply) => {
      // Browser fetch must never redeem a ticket intended for the desktop app.
      if (
        request.headers["sec-fetch-mode"] ||
        request.headers["sec-fetch-site"]
      )
        return reply
          .code(403)
          .send({ error: "Open this invitation in FreedomStream." });
      const input = invitationSchema.safeParse(
        (request.body as { ticket?: unknown } | null)?.ticket,
      );
      if (!input.success)
        return reply.code(404).send({
          error: "App invitation expired. Open it from your browser again.",
        });
      const key = hash(input.data);
      const ticket = tickets.get(key);
      tickets.delete(key); // Single use, including revoked/expired tickets.
      sweep();
      if (
        !ticket ||
        ticket.expires <= now() ||
        !rooms.acceptsInvitation(ticket.roomId, ticket.invitation)
      )
        return reply.code(404).send({
          error: "App invitation expired. Open it from your browser again.",
        });
      return {
        invitationUrl: `${config.publicUrl}/r/${ticket.roomId}#invite=${ticket.invitation}`,
      };
    },
  );
}
