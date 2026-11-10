import { randomBytes } from "node:crypto";
import type { FastifyInstance } from "fastify";
import {
  PREVIEW_MAX_BYTES,
  PROTECTED_PREVIEW_MAX_BYTES,
  PROTECTED_PREVIEW_TYPE,
  PREVIEW_TTL_MS,
  THUMBNAIL_INTERVAL_MS,
  previewAttribute,
  previewKindSchema,
  roomIdSchema,
  trackSidSchema,
  type Preview,
  type PreviewKind,
} from "@freedomstream/contracts";
import { jpegDimensions } from "./jpeg.js";
import { MediaUnavailable, type MediaDirectory } from "./media.js";
import type { Rooms } from "./rooms.js";

const CAPACITY = 256; // Bounded to 65 MiB including protected envelopes.
const MAX_PENDING = 16;
const ownerPattern =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
type Entry = { preview: Preview; bytes: Buffer | null; contentType: string };
type Job = { trackSid: string; cancelled: boolean };
const keyOf = (room: string, owner: string, kind: PreviewKind) =>
  `${room}:${owner}:${kind}`;

class PreviewError extends Error {
  constructor(
    readonly status: number,
    message: string,
    readonly retryAfter?: number,
  ) {
    super(message);
  }
}

/** Owns bounded, ephemeral image bytes and SFU publication proof. No image decoder. */
export class Previews {
  private readonly entries = new Map<string, Entry>();
  private readonly pending = new Map<string, Job>();
  constructor(
    private readonly media: MediaDirectory,
    private readonly now: () => number = Date.now,
  ) {}

  sweep(): void {
    for (const [key, entry] of this.entries)
      if (this.now() - entry.preview.updatedAt >= PREVIEW_TTL_MS)
        this.entries.delete(key);
  }
  clear(): void {
    this.entries.clear();
    for (const job of this.pending.values()) job.cancelled = true;
  }
  removeOwner(room: string, owner: string): void {
    for (const kind of ["screen", "camera"] as const) {
      const key = keyOf(room, owner, kind);
      this.entries.delete(key);
      const job = this.pending.get(key);
      if (job) job.cancelled = true;
    }
  }
  private async published(
    room: string,
    owner: string,
    kind: PreviewKind,
    sid: string,
  ): Promise<boolean> {
    const participant = await this.media.participant(room, owner);
    return (
      participant?.tracks.some(
        (track) => track.sid === sid && track.kind === kind && !track.muted,
      ) ?? false
    );
  }
  async put(
    room: string,
    owner: string,
    kind: PreviewKind,
    trackSid: string,
    bytes: Buffer,
    admitted: () => boolean,
    protectedContent = false,
  ): Promise<Preview> {
    // This service has no encryption keys. Envelope checks are structural only;
    // protected clients authenticate, bound dimensions and decode after decryption.
    const dimensions = protectedContent
      ? bytes.length >= 32 &&
        bytes.length <= PROTECTED_PREVIEW_MAX_BYTES &&
        bytes.subarray(0, 4).equals(Buffer.from("FSP1"))
        ? { width: 960, height: 540 }
        : null
      : jpegDimensions(bytes);
    if (!dimensions)
      throw new PreviewError(
        400,
        protectedContent
          ? "Use a bounded version 1 protected preview envelope."
          : "Use a baseline JPEG no larger than 960 × 540 and 256 KiB.",
      );
    this.sweep();
    const key = keyOf(room, owner, kind);
    const previous = this.entries.get(key);
    if (this.pending.has(key))
      throw new PreviewError(409, "A preview update is already in progress.");
    if (
      previous?.preview.trackSid === trackSid &&
      this.now() - previous.preview.updatedAt < THUMBNAIL_INTERVAL_MS
    )
      throw new PreviewError(
        429,
        "Previews update at most once a minute.",
        Math.ceil(
          (previous.preview.updatedAt + THUMBNAIL_INTERVAL_MS - this.now()) /
            1000,
        ),
      );
    if (
      this.pending.size >= MAX_PENDING ||
      (!previous && this.entries.size + this.pending.size >= CAPACITY)
    )
      throw new PreviewError(
        503,
        "Preview storage is busy. Try again shortly.",
        5,
      );
    const job: Job = { trackSid, cancelled: false };
    this.pending.set(key, job);
    try {
      if (!(await this.published(room, owner, kind, trackSid)))
        throw new PreviewError(404, "This stream is no longer available.");
      if (job.cancelled || !admitted())
        throw new PreviewError(401, "Open your room invitation to join.");
      const preview: Preview = {
        ...dimensions,
        trackSid,
        revision: randomBytes(16).toString("base64url"),
        updatedAt: this.now(),
      };
      await this.media.attribute(
        room,
        owner,
        previewAttribute(kind),
        JSON.stringify(preview),
      );
      // Stop/Leave may run while the administrative RPC is in flight. Never
      // resurrect its bytes or let an old completion overwrite a new stream.
      if (job.cancelled || !admitted())
        throw new PreviewError(401, "Open your room invitation to join.");
      this.entries.set(key, {
        preview,
        bytes,
        contentType: protectedContent ? PROTECTED_PREVIEW_TYPE : "image/jpeg",
      });
      return preview;
    } finally {
      if (this.pending.get(key) === job) this.pending.delete(key);
    }
  }
  async get(
    room: string,
    owner: string,
    kind: PreviewKind,
    revision: string,
  ): Promise<{ preview: Preview; bytes: Buffer; contentType: string } | null> {
    this.sweep();
    const key = keyOf(room, owner, kind);
    const entry = this.entries.get(key);
    if (!entry?.bytes || entry.preview.revision !== revision) return null;
    if (!(await this.published(room, owner, kind, entry.preview.trackSid))) {
      if (this.entries.get(key) === entry) this.entries.delete(key);
      return null;
    }
    this.sweep();
    return this.entries.get(key) === entry && entry.bytes
      ? {
          preview: entry.preview,
          bytes: entry.bytes,
          contentType: entry.contentType,
        }
      : null;
  }
  async remove(
    room: string,
    owner: string,
    kind: PreviewKind,
    trackSid: string,
  ): Promise<void> {
    const key = keyOf(room, owner, kind);
    const job = this.pending.get(key);
    if (job?.trackSid === trackSid) job.cancelled = true;
    const entry = this.entries.get(key);
    if (entry?.preview.trackSid !== trackSid) return;
    // Retain only the bounded cadence descriptor until expiry, so DELETE cannot
    // reset the same publication's minute limit. Its image bytes are released.
    entry.bytes = null;
    // A new publication can upload as soon as this call yields. Do not clear
    // attributes here: stale descriptors are harmless and checked against SID
    // and live publication; a late clear could erase the new descriptor.
  }
}

export async function previewRoutes(
  app: FastifyInstance,
  rooms: Rooms,
  previews: Previews,
): Promise<void> {
  app.addContentTypeParser(
    "image/jpeg",
    { parseAs: "buffer", bodyLimit: PREVIEW_MAX_BYTES },
    (_request, body, done) => done(null, body),
  );
  app.addContentTypeParser(
    PROTECTED_PREVIEW_TYPE,
    { parseAs: "buffer", bodyLimit: PROTECTED_PREVIEW_MAX_BYTES },
    (_request, body, done) => done(null, body),
  );
  type Own = { roomId: string; kind: string };
  const session = (id: string, cookie: string | undefined) =>
    rooms.session(id, cookie);
  const errorResponse = (error: unknown) => {
    if (error instanceof PreviewError) return error;
    if (error instanceof MediaUnavailable)
      return new PreviewError(503, error.message, 3);
    throw error;
  };
  app.post<{ Params: Own }>(
    "/api/rooms/:roomId/previews/:kind",
    { bodyLimit: PROTECTED_PREVIEW_MAX_BYTES },
    async (request, reply) => {
      const { roomId } = request.params;
      const member = session(roomId, request.cookies.fs_session);
      if (!member)
        return reply
          .code(401)
          .send({ error: "Open your room invitation to join." });
      const kind = previewKindSchema.safeParse(request.params.kind);
      const sid = trackSidSchema.safeParse(request.headers["x-track-sid"]);
      if (
        !kind.success ||
        !sid.success ||
        !Buffer.isBuffer(request.body) ||
        !["image/jpeg", PROTECTED_PREVIEW_TYPE].includes(
          String(request.headers["content-type"]),
        )
      )
        return reply.code(400).send({
          error: "Choose a stream and send a supported preview format.",
        });
      const protectedContent =
        request.headers["content-type"] === PROTECTED_PREVIEW_TYPE;
      if (
        request.body.length >
        (protectedContent ? PROTECTED_PREVIEW_MAX_BYTES : PREVIEW_MAX_BYTES)
      )
        return reply
          .code(413)
          .send({ error: "Preview exceeds its size limit." });
      try {
        return await previews.put(
          roomId,
          member.participant.id,
          kind.data,
          sid.data,
          request.body,
          () => Boolean(session(roomId, request.cookies.fs_session)),
          protectedContent,
        );
      } catch (error) {
        const failure = errorResponse(error);
        if (failure.retryAfter) reply.header("Retry-After", failure.retryAfter);
        return reply.code(failure.status).send({ error: failure.message });
      }
    },
  );
  app.delete<{ Params: Own }>(
    "/api/rooms/:roomId/previews/:kind",
    async (request, reply) => {
      const { roomId } = request.params;
      const member = session(roomId, request.cookies.fs_session);
      if (!member)
        return reply
          .code(401)
          .send({ error: "Open your room invitation to join." });
      const kind = previewKindSchema.safeParse(request.params.kind);
      const sid = trackSidSchema.safeParse(request.headers["x-track-sid"]);
      if (!kind.success || !sid.success)
        return reply.code(400).send({ error: "Choose a stream." });
      await previews.remove(roomId, member.participant.id, kind.data, sid.data);
      return reply.code(204).send();
    },
  );
  app.get<{ Params: Own & { owner: string; revision: string } }>(
    "/api/rooms/:roomId/previews/:owner/:kind/:revision",
    async (request, reply) => {
      const { roomId, owner, revision } = request.params;
      if (!session(roomId, request.cookies.fs_session))
        return reply
          .code(401)
          .send({ error: "Open your room invitation to join." });
      const kind = previewKindSchema.safeParse(request.params.kind);
      if (
        !roomIdSchema.safeParse(roomId).success ||
        !kind.success ||
        !ownerPattern.test(owner) ||
        !/^[A-Za-z0-9_-]{22}$/.test(revision)
      )
        return reply.code(404).send({ error: "Preview unavailable." });
      try {
        if (!rooms.active(roomId, owner))
          return reply.code(404).send({ error: "Preview unavailable." });
        const entry = await previews.get(roomId, owner, kind.data, revision);
        if (!entry)
          return reply.code(404).send({ error: "Preview unavailable." });
        // Session expiry/rotation may happen while the live-publication check waits.
        if (!session(roomId, request.cookies.fs_session))
          return reply
            .code(401)
            .send({ error: "Open your room invitation to join." });
        if (!rooms.active(roomId, owner))
          return reply.code(404).send({ error: "Preview unavailable." });
        return reply.type(entry.contentType).send(entry.bytes);
      } catch (error) {
        const failure = errorResponse(error);
        return reply.code(failure.status).send({ error: failure.message });
      }
    },
  );
}
