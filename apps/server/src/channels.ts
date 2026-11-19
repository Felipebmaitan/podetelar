import { createHash } from "node:crypto";
import type { FastifyInstance } from "fastify";
import { z } from "zod";
import type { Rooms } from "./rooms.js";
import type { DiscordVoice } from "./discord-voice.js";
import type { Config } from "./config.js";
const id = z.string().regex(/^\d{17,20}$/);
export const channelParams = z.object({ guildId: id, channelId: id });
export function channelRoutes(
  app: FastifyInstance,
  config: Config,
  rooms: Rooms,
  voice: DiscordVoice,
) {
  app.post<{ Params: { guildId: string; channelId: string } }>(
    "/api/channels/:guildId/:channelId/resolve",
    { config: { rateLimit: { max: 5, timeWindow: "1 minute" } } },
    async (request, reply) => {
      const params = channelParams.safeParse(request.params),
        body = z
          .object({ invitation: z.string().regex(/^[A-Za-z0-9_-]{43}$/) })
          .strict()
          .safeParse(request.body);
      if (!params.success || !body.success)
        return reply
          .code(404)
          .send({ error: "Channel invitation unavailable." });
      const channel = voice.index.channel(
        params.data.guildId,
        params.data.channelId,
      );
      const cachedName = rooms.channelName(
        params.data.guildId,
        params.data.channelId,
      );
      if (!channel && !cachedName)
        return reply.code(503).send({
          error:
            "Discord channel is temporarily unavailable. Try again when the bot reconnects.",
        });
      const resolved = rooms.resolveChannel(
        params.data.guildId,
        params.data.channelId,
        channel?.channelName ?? cachedName!,
        body.data.invitation,
      );
      if (!resolved)
        return reply
          .code(404)
          .send({ error: "Channel invitation unavailable." });
      return {
        invitationUrl: `${config.publicUrl}/r/${resolved.room.id}#invite=${resolved.invitation}`,
      };
    },
  );
}

export class ChannelAnnouncements {
  private pending: Promise<void> | null = null;
  private next = new Map<string, number>();
  constructor(
    private config: Config,
    private rooms: Rooms,
    private voice: DiscordVoice,
  ) {}
  tick() {
    if (this.pending) return this.pending;
    this.pending = (async () => {
      if (!this.voice.status().online) return;
      const jobs = this.rooms.announcementJobs();
      let attempted = 0;
      for (const job of jobs) {
        const key = `${job.guildId}:${job.channelId}`;
        if (
          (this.next.get(key) ?? 0) > Date.now() ||
          !this.voice.index.channel(job.guildId, job.channelId)
        )
          continue;
        const secret = this.rooms.channelInvitation(
          job.guildId,
          job.channelId,
          job.name,
        );
        if (!secret) continue;
        const state = createHash("sha256")
          .update(`${job.roomId}:${job.guestEnabled}:${secret}`)
          .digest("hex");
        if (state === job.state) continue;
        if (attempted++ >= 4) break;
        this.next.set(key, Date.now() + 30000);
        const url = `${this.config.publicUrl}/c/${job.guildId}/${job.channelId}#invite=${secret}`;
        const message = await this.voice.announce(
          job.channelId,
          job.messageId,
          {
            content: job.roomId
              ? "PodeTelar is ready for this voice channel. Sharing and watching are your choice."
              : "PodeTelar is available for this voice channel. Open the invitation to start a room.",
            allowed_mentions: { parse: [] },
            flags: 4,
            components: job.guestEnabled
              ? [
                  {
                    type: 1,
                    components: [
                      { type: 2, style: 5, label: "Join independently", url },
                    ],
                  },
                ]
              : [],
          },
        );
        this.rooms.announcementResult(
          job.guildId,
          job.channelId,
          message,
          message ? state : null,
          !message,
        );
      }
    })().finally(() => {
      this.pending = null;
    });
    return this.pending;
  }
  async close() {
    await this.pending;
  }
}
