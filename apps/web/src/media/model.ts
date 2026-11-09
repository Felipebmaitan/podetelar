export type StreamKind = "screen" | "camera";
export type LiveStream = {
  id: string;
  publisherId: string;
  publisherName: string;
  kind: StreamKind;
  local: boolean;
  muted: boolean;
  watching: boolean;
  video: MediaStreamTrack | null;
  audio: MediaStreamTrack | null;
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
