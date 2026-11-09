import type { FastifyInstance, FastifyRequest } from "fastify";
import {
  accessChangeSchema,
  participantIdSchema,
  roomIdSchema,
} from "@freedomstream/contracts";
import type { Config } from "./config.js";
import { Rooms, secretsEqual } from "./rooms.js";
import type { MediaDirectory } from "./media.js";
import type { Previews } from "./previews.js";

/** Durable revocation owns retries; gateway admission never waits on an SFU RPC. */
export class Access {
  private work: Promise<void> | null = null;
  constructor(
    private rooms: Rooms,
    private media: MediaDirectory,
    private previews: Previews,
    private closeRevoked: () => void,
  ) {}
  settle(): Promise<void> {
    this.rooms.expire();
    this.closeRevoked();
    if (this.work) return this.work;
    this.work = this.drain().finally(() => {
      this.work = null;
    });
    return this.work;
  }
  private async drain(): Promise<void> {
    const pending = this.rooms.removals();
    for (let i = 0; i < pending.length; i += 8) {
      await Promise.all(
        pending.slice(i, i + 8).map(async (row) => {
          this.previews.removeOwner(row.room, row.identity);
          try {
            await this.media.remove(row.room, row.identity);
            this.rooms.acknowledge(row.room, row.identity);
            if (this.rooms.time() >= row.retryUntil)
              this.rooms.removed(row.room, row.identity);
          } catch {
            /* Persisted for the next bounded sweep; no raw SDK errors. */
          }
        }),
      );
    }
  }
}

export function administrator(
  request: FastifyRequest,
  config: Config,
): boolean {
  const header = request.headers.authorization;
  return Boolean(
    header?.startsWith("Bearer ") &&
    secretsEqual(header.slice(7), config.adminToken),
  );
}

export async function accessRoutes(
  app: FastifyInstance,
  config: Config,
  rooms: Rooms,
  access: Access,
) {
  await app.register(async (scope) => {
    scope.addHook("onRequest", async (request, reply) => {
      if (!administrator(request, config))
        return reply
          .code(401)
          .send({ error: "Room management requires the server access key." });
    });
    scope.get<{ Params: { roomId: string } }>(
      "/api/rooms/:roomId/access",
      async (request, reply) => {
        const state = rooms.access(request.params.roomId);
        if (!state) return reply.code(404).send({ error: "Room unavailable." });
        return {
          ...state,
          pendingDisconnects: rooms.pending(request.params.roomId),
        };
      },
    );
    scope.post<{ Params: { roomId: string } }>(
      "/api/rooms/:roomId/access",
      async (request, reply) => {
        const input = accessChangeSchema.safeParse(request.body);
        const id = roomIdSchema.safeParse(request.params.roomId);
        if (!input.success || !id.success)
          return reply
            .code(400)
            .send({ error: "Choose invitation and current-session behavior." });
        const changed = rooms.change(id.data, input.data);
        if (!changed)
          return reply.code(404).send({ error: "Room unavailable." });
        await access.settle();
        const pendingDisconnects = rooms.pending(id.data);
        return reply.code(pendingDisconnects ? 202 : 200).send({
          invitationUrl: changed.invitation
            ? `${config.publicUrl}/r/${id.data}#invite=${changed.invitation}`
            : undefined,
          pendingDisconnects,
        });
      },
    );
    scope.delete<{ Params: { roomId: string; identity: string } }>(
      "/api/rooms/:roomId/access/sessions/:identity",
      async (request, reply) => {
        const { roomId, identity } = request.params;
        if (
          !roomIdSchema.safeParse(roomId).success ||
          !participantIdSchema.safeParse(identity).success ||
          !rooms.access(roomId)
        )
          return reply.code(404).send({ error: "Session unavailable." });
        rooms.revoke(roomId, identity);
        await access.settle();
        return reply
          .code(rooms.pending(roomId) ? 202 : 200)
          .send({ pendingDisconnects: rooms.pending(roomId) });
      },
    );
  });
}
