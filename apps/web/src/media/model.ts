import type { Preview, PreviewKind } from "@freedomstream/contracts";
export type StreamKind = PreviewKind;
export type LiveStream = {
  id: string;
  publisherId: string;
  publisherName: string;
  kind: StreamKind;
  codec: string;
  compatible: boolean;
  local: boolean;
  muted: boolean;
  watching: boolean;
  video: MediaStreamTrack | null;
  audio: MediaStreamTrack | null;
  preview: Preview | null;
};
export type LiveParticipant = { id: string; name: string; local: boolean };
export type MediaSnapshot = {
  connection:
    "connecting" | "connected" | "reconnecting" | "disconnected" | "failed";
  participants: LiveParticipant[];
  streams: LiveStream[];
  pending: StreamKind | null;
  error: string;
};
export const emptyMediaSnapshot: MediaSnapshot = {
  connection: "connecting",
  participants: [],
  streams: [],
  pending: null,
  error: "",
};
