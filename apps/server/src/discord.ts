import { createPublicKey, verify } from "node:crypto";
import type { FastifyInstance } from "fastify";
import { z } from "zod";
import type { Config } from "./config.js";
import type { Rooms } from "./rooms.js";

const snowflake = z.string().regex(/^\d{17,20}$/);
const interaction = z.object({
  id: snowflake,
  application_id: snowflake,
  type: z.number().int(),
  guild_id: snowflake.optional(),
  member: z.object({ user: z.object({ id: snowflake }) }).optional(),
  data: z
    .object({
      name: z.enum(["stream", "telar"]),
      type: z.literal(1),
      options: z.array(z.unknown()).max(0).optional(),
    })
    .optional(),
});
const message = (content: string, ephemeral = true) => ({
  type: 4,
  data: { content, flags: ephemeral ? 64 : 4, allowed_mentions: { parse: [] } },
});

/** Discord signatures authenticate raw bytes; this scope alone bypasses browser Origin checks. */
export async function discordRoutes(
  app: FastifyInstance,
  config: Config,
  rooms: Rooms,
  now = Date.now,
) {
  if (!config.discord) return;
  const discord = config.discord;
  const key = createPublicKey({
    key: Buffer.concat([
      Buffer.from("302a300506032b6570032100", "hex"),
      Buffer.from(discord.publicKey, "hex"),
    ]),
    format: "der",
    type: "spki",
  });
  await app.register(async (scope) => {
    scope.removeContentTypeParser("application/json");
    scope.addContentTypeParser(
      "application/json",
      { parseAs: "buffer", bodyLimit: 32768 },
      (_request, body, done) => done(null, body),
    );
    scope.post(
      "/api/discord/interactions",
      { bodyLimit: 32768 },
      async (request, reply) => {
        const signature = request.headers["x-signature-ed25519"];
        const timestamp = request.headers["x-signature-timestamp"];
        const body = request.body;
        if (
          typeof signature !== "string" ||
          !/^[a-fA-F0-9]{128}$/.test(signature) ||
          typeof timestamp !== "string" ||
          !/^\d{10}$/.test(timestamp) ||
          Math.abs(now() - Number(timestamp) * 1000) > 5 * 60_000 ||
          !Buffer.isBuffer(body) ||
          !verify(
            null,
            Buffer.concat([Buffer.from(timestamp), body]),
            key,
            Buffer.from(signature, "hex"),
          )
        )
          return reply.code(401).send({ error: "Invalid Discord signature." });
        let raw: unknown;
        try {
          raw = JSON.parse(body.toString("utf8"));
        } catch {
          return reply.code(400).send({ error: "Invalid interaction." });
        }
        const parsed = interaction.safeParse(raw);
        if (
          !parsed.success ||
          parsed.data.application_id !== discord.applicationId
        )
          return reply.code(400).send({ error: "Invalid interaction." });
        const input = parsed.data;
        if (input.type === 1) return { type: 1 };
        if (input.type !== 2 || !input.data)
          return reply.code(400).send({ error: "Unsupported interaction." });
        if (
          !input.guild_id ||
          !discord.guildIds.includes(input.guild_id) ||
          !input.member
        )
          return message(
            "Room creation is unavailable in this Discord server.",
          );
        const created = rooms.createFromDiscord(
          input.id,
          input.guild_id,
          input.member.user.id,
        );
        if (created === "duplicate")
          return message(
            "This command was already handled. Use /stream or /telar for a new room.",
          );
        if (created === "limited")
          return message("Too many rooms created. Try again in a minute.");
        // Angle brackets and SUPPRESS_EMBEDS prevent link unfurling. The secret is fragment-only.
        return message(
          `Join a new PodeTelar room: <${config.publicUrl}/r/${created.room.id}#invite=${created.invitation}>\nAnyone with this link can join. Share it only with friends.`,
          false,
        );
      },
    );
  });
}
