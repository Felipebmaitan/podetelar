import { createHash } from "node:crypto";
import type { FastifyInstance } from "fastify";
import { z } from "zod";
import type { Rooms } from "./rooms.js";
import { MediaUnavailable, type MediaDirectory } from "./media.js";

const metricNames = [
  "encoded_window_ms",
  "capture_queue_frames",
  "encoder_queue_frames",
  "capture_queue_overruns",
  "capture_fps",
  "encoder_input_fps",
  "requested_bps",
  "effective_bps",
  "encoded_bps",
  "sent_bps",
  "upload_cap_bps",
  "watcher_cap_bps",
  "congestion_bps",
  "requested_fps",
  "encoded_fps",
  "sent_fps",
  "accepted_fps",
  "encoder_reconfigurations",
  "handoff_rejections",
  "source_width",
  "source_height",
  "encoded_width",
  "encoded_height",
  "rtt_ms",
  "loss_fraction",
] as const;
const metrics = z
  .object(
    Object.fromEntries(
      metricNames.map((k) => [
        k,
        z.number().finite().min(0).max(1e12).nullable(),
      ]),
    ) as Record<(typeof metricNames)[number], z.ZodNullable<z.ZodNumber>>,
  )
  .strict();
const sampleSchema = z
  .object({
    window_ms: z.number().int().min(500).max(15000),
    sequence: z.number().int().min(0).max(Number.MAX_SAFE_INTEGER),
    codec: z.enum(["av1", "h265", "h264"]),
    encoder: z.enum([
      "nvd3d11av1enc",
      "amfav1enc",
      "nvd3d11h265enc",
      "amfh265enc",
      "nvd3d11h264enc",
      "amfh264enc",
    ]),
    metrics,
  })
  .strict();
type Sample = z.infer<typeof sampleSchema>;
type Entry = {
  room: string;
  owner: string;
  sid: string;
  viewers: Map<string, number>;
  sample?: Sample;
  sampled?: number;
};
const sidPattern = /^TR_[A-Za-z0-9_-]{1,128}$/;
const ownerPattern = /^[A-Za-z0-9_-]{1,128}$/;
export const DIAGNOSTIC_LEASE_MS = 10000;

/** Latest aggregates only. Never serialized to disk, usage analytics or logs. */
export function streamDiagnosticsRoutes(
  app: FastifyInstance,
  rooms: Rooms,
  directory: MediaDirectory,
  now = Date.now,
) {
  const entries = new Map<string, Entry>();
  const pending = new Set<string>();
  const sweep = () => {
    for (const [key, entry] of entries) {
      for (const [viewer, expiry] of entry.viewers)
        if (expiry <= now() || !rooms.active(entry.room, viewer))
          entry.viewers.delete(viewer);
      if (!entry.viewers.size || !rooms.active(entry.room, entry.owner))
        entries.delete(key);
    }
  };
  const timer = setInterval(sweep, 1000);
  timer.unref();
  app.addHook("onClose", async () => {
    clearInterval(timer);
    entries.clear();
  });
  type Params = { roomId: string; owner: string; sid: string };
  app.route<{ Params: Params }>({
    method: ["GET", "POST", "PUT", "DELETE"],
    url: "/api/rooms/:roomId/stream-diagnostics/:owner/:sid",
    bodyLimit: 4096,
    config: {
      rateLimit: {
        max: 240,
        timeWindow: "1 minute",
        keyGenerator: (request) =>
          createHash("sha256")
            .update(request.cookies.fs_session ?? request.ip)
            .digest("hex"),
      },
    },
    handler: async (request, reply) => {
      sweep();
      const { roomId: room, sid } = request.params;
      let { owner } = request.params;
      const session = () => rooms.session(room, request.cookies.fs_session);
      const member = session();
      if (!member)
        return reply.code(401).send({ error: "Room session required." });
      if (!sidPattern.test(sid) || !ownerPattern.test(owner))
        return reply.code(400).send({ error: "Invalid publication." });
      const viewer = member.participant.id;
      if (owner === "self") owner = viewer;
      const own = viewer === owner;
      if ((request.method === "PUT" || request.method === "GET") && !own)
        return reply.code(403).send({ error: "Publisher required." });
      const key = `${room}/${owner}/${sid}`;
      if (request.method === "DELETE") {
        const entry = entries.get(key);
        if (own) entries.delete(key);
        else entry?.viewers.delete(viewer);
        sweep();
        return reply.code(204).send();
      }
      const publicationKey = `${room}/${owner}`;
      if (pending.has(publicationKey) || pending.size >= 16)
        return reply.code(429).send({ error: "Diagnostic request busy." });
      pending.add(publicationKey);
      try {
        const snapshot = directory.diagnosticParticipants
          ? await directory.diagnosticParticipants(room)
          : null;
        const participant = (identity: string) =>
          snapshot
            ? Promise.resolve(snapshot.get(identity) ?? null)
            : directory.participant(room, identity);
        const publisher = await participant(owner);
        if (!session() || !rooms.active(room, owner))
          return reply.code(403).send({ error: "Session ended." });
        if (!publisher?.tracks.some((t) => t.sid === sid && !t.muted)) {
          entries.delete(key);
          return reply.code(409).send({ error: "Publication ended." });
        }
        if (!own) {
          const watcher = await participant(viewer);
          // The SFU API exposes participant-owned watch intent, not actual RTP subscription state.
          // Require that existing selective-watch contract; never create watch intent here.
          if (
            !session() ||
            !rooms.active(room, owner) ||
            !["h264", "hevc", "av1"].includes(
              watcher?.attributes?.[`freedomstream.watch.${owner}`] ?? "",
            )
          )
            return reply
              .code(403)
              .send({ error: "Watch this publication first." });
        }
        // Revalidate every lease against live SFU watch intent, including publisher polls.
        const existing = entries.get(key);
        if (existing && request.method === "GET")
          for (const viewerId of [...existing.viewers.keys()]) {
            const watcher = await participant(viewerId);
            if (
              !rooms.active(room, viewerId) ||
              !["h264", "hevc", "av1"].includes(
                watcher?.attributes?.[`freedomstream.watch.${owner}`] ?? "",
              )
            )
              existing.viewers.delete(viewerId);
          }
        if (!session() || !rooms.active(room, owner))
          return reply.code(403).send({ error: "Session ended." });
        const stillPublished = await directory.participant(room, owner);
        if (!stillPublished?.tracks.some((t) => t.sid === sid && !t.muted)) {
          entries.delete(key);
          return reply.code(409).send({ error: "Publication ended." });
        }
        if (!session() || !rooms.active(room, owner))
          return reply.code(403).send({ error: "Session ended." });
        // Track SID is the SFU publication generation. Retire old generations.
        for (const [oldKey, entry] of entries)
          if (entry.room === room && entry.owner === owner && entry.sid !== sid)
            entries.delete(oldKey);
        sweep();
        let entry = entries.get(key);
        if (request.method === "GET")
          return {
            demand: entry?.viewers.size ?? 0,
            lease_ms: DIAGNOSTIC_LEASE_MS,
          };
        if (request.method === "PUT") {
          const parsed = sampleSchema.safeParse(request.body);
          if (!parsed.success)
            return reply
              .code(400)
              .send({ error: "Invalid diagnostic aggregate." });
          if (!entry)
            return reply.code(409).send({ error: "No diagnostic demand." });
          if (entry.sampled !== undefined && now() - entry.sampled < 2000)
            return reply.code(429).send({ error: "Sample cadence exceeded." });
          if (entry.sample && parsed.data.sequence <= entry.sample.sequence)
            return reply.code(409).send({ error: "Old sample." });
          entry.sample = parsed.data;
          entry.sampled = now();
          return { accepted: true };
        }
        if (own) return reply.code(403).send({ error: "Viewer required." });
        if (!entry) {
          if (entries.size >= 256)
            return reply
              .code(429)
              .send({ error: "Diagnostic capacity reached." });
          entry = { room, owner, sid, viewers: new Map() };
          entries.set(key, entry);
        }
        if (!entry.viewers.has(viewer) && entry.viewers.size >= 32)
          return reply.code(429).send({ error: "Viewer capacity reached." });
        entry.viewers.set(viewer, now() + DIAGNOSTIC_LEASE_MS);
        const age_ms =
          entry.sampled === undefined
            ? null
            : Math.max(0, now() - entry.sampled);
        return {
          generation: sid,
          age_ms,
          stale: age_ms === null || age_ms > 7000,
          sample: entry.sample ?? null,
        };
      } catch (error) {
        if (error instanceof MediaUnavailable)
          return reply.code(503).send({ error: "Media state unavailable." });
        return reply.code(503).send({ error: "Diagnostics unavailable." });
      } finally {
        pending.delete(publicationKey);
      }
    },
  });
}
