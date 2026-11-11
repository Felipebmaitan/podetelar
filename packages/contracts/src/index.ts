import { z } from "zod";

export const THUMBNAIL_INTERVAL_MS = 60_000;
export const PREVIEW_MAX_BYTES = 262_144;
export const PREVIEW_MAX_WIDTH = 960;
export const PREVIEW_MAX_HEIGHT = 540;
export const PREVIEW_TTL_MS = 180_000;
export const previewKindSchema = z.enum(["screen", "camera"]);
export type PreviewKind = z.infer<typeof previewKindSchema>;
export const trackSidSchema = z
  .string()
  .min(1)
  .max(128)
  .regex(/^[A-Za-z0-9_-]+$/);
export const previewSchema = z
  .object({
    trackSid: trackSidSchema,
    revision: z.string().regex(/^[A-Za-z0-9_-]{22}$/),
    width: z.number().int().min(1).max(PREVIEW_MAX_WIDTH),
    height: z.number().int().min(1).max(PREVIEW_MAX_HEIGHT),
    updatedAt: z.number().int().nonnegative(),
  })
  .strict();
export type Preview = z.infer<typeof previewSchema>;
export const previewAttribute = (kind: PreviewKind) =>
  `freedomstream.preview.${kind}`;
export const roomIdSchema = z.string().regex(/^[A-Za-z0-9_-]{22}$/);
export const invitationSchema = z.string().regex(/^[A-Za-z0-9_-]{43}$/);
const nameSchema = z
  .string()
  .trim()
  .min(1)
  .max(60)
  .refine((value) => !/[\p{Cc}\p{Cf}]/u.test(value));

export const adminCreateRoomSchema = z.object({ name: nameSchema }).strict();
export const createRoomSchema = z
  .object({ name: nameSchema, displayName: nameSchema })
  .strict();
export const joinRoomSchema = z
  .object({ invitation: invitationSchema, displayName: nameSchema })
  .strict();
export const accessChangeSchema = z
  .object({
    invitation: z.enum(["rotate", "disable"]),
    revokeSessions: z.boolean(),
  })
  .strict();
export const participantIdSchema = z.string().uuid();
export type AccessChange = z.infer<typeof accessChangeSchema>;
export type RoomAccess = {
  invitationEnabled: boolean;
  sessions: (Participant & { expiresAt: string })[];
  pendingDisconnects: number;
};
export type ChangedAccess = {
  invitationUrl?: string;
  pendingDisconnects: number;
};

export type CreateRoomInput = z.infer<typeof createRoomSchema>;
export type JoinRoomInput = z.infer<typeof joinRoomSchema>;
export type Room = { id: string; name: string; createdAt: string };
export type ManagedRoom = Room & { invitationEnabled: boolean };
export type RoomDirectory = { rooms: ManagedRoom[]; nextOffset: number | null };
export type AdminRoom = ManagedRoom & {
  members: Participant[];
  presence: "active" | "waiting" | "closing" | "unavailable";
};
export type AdminRoomDirectory = {
  rooms: AdminRoom[];
  nextOffset: number | null;
};
export type AdminCreatedRoom = { room: Room; invitationUrl: string };
export type Participant = { id: string; displayName: string };
export type RoomSession = {
  room: Room;
  participant: Participant;
  expiresAt: string;
};
export type CreatedRoom = RoomSession & { invitationUrl: string };
export type MediaAccess = {
  serverUrl: string;
  token: string;
  expiresInSeconds: number;
};
export type ApiErrorBody = { error: string };

export type UsagePoint = {
  at: number;
  durationMs: number;
  uploadedBytes: number;
  downloadedBytes: number;
};
export type UsageReport = {
  from: number;
  to: number;
  bucketMs: number;
  lastSampleAt: number | null;
  available: boolean;
  uploadedBytes: number;
  downloadedBytes: number;
  userCount: number;
  page: number;
  retentionDays: number;
  points: UsagePoint[];
  users: {
    id: string;
    roomId: string;
    displayName: string;
    uploadedBytes: number;
    downloadedBytes: number;
    points: UsagePoint[];
  }[];
};
