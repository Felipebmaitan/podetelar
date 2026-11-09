import { z } from "zod";

export const THUMBNAIL_INTERVAL_MS = 60_000;
export const roomIdSchema = z.string().regex(/^[A-Za-z0-9_-]{22}$/);
export const invitationSchema = z.string().regex(/^[A-Za-z0-9_-]{43}$/);
const nameSchema = z
  .string()
  .trim()
  .min(1)
  .max(60)
  .refine((value) => !/[\p{Cc}\p{Cf}]/u.test(value));

export const createRoomSchema = z
  .object({ name: nameSchema, displayName: nameSchema })
  .strict();
export const joinRoomSchema = z
  .object({ invitation: invitationSchema, displayName: nameSchema })
  .strict();

export type CreateRoomInput = z.infer<typeof createRoomSchema>;
export type JoinRoomInput = z.infer<typeof joinRoomSchema>;
export type Room = { id: string; name: string; createdAt: string };
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
