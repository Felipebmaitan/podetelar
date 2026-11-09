import {
  ConnectionState,
  createLocalScreenTracks,
  createLocalVideoTrack,
  LogLevel,
  Room,
  RoomEvent,
  Track,
  VideoPresets,
  setLogLevel,
} from "livekit-client";
import type { LocalTrack, Participant } from "livekit-client";
import type { MediaAccess } from "@freedomstream/contracts";
import { request } from "../api";
import type { LiveStream, MediaSnapshot, StreamKind } from "./model";

// SDK logs can contain signaling URLs. Surface bounded messages through our UI instead.
setLogLevel(LogLevel.silent);

const kindOf = (source: Track.Source): StreamKind | null =>
  source === Track.Source.ScreenShare
    ? "screen"
    : source === Track.Source.Camera
      ? "camera"
      : null;
const streamId = (identity: string, kind: StreamKind) => `${identity}:${kind}`;

/** Owns capture, signaling and subscription lifetimes. UI receives browser tracks, not SDK objects. */
export class MediaRoom {
  private readonly room = new Room({
    adaptiveStream: false,
    dynacast: false,
    publishDefaults: { videoCodec: "vp8", simulcast: false },
  });
  private readonly watching = new Set<string>();
  private readonly listeners = new Set<(snapshot: MediaSnapshot) => void>();
  private readonly abort = new AbortController();
  private closed = false;
  private connection: MediaSnapshot["connection"] = "connecting";
  private pending: StreamKind | null = null;
  private error = "";

  constructor(private readonly roomId: string) {
    const refresh = () => this.refresh();
    for (const event of [
      RoomEvent.ParticipantConnected,
      RoomEvent.TrackPublished,
      RoomEvent.TrackSubscribed,
      RoomEvent.TrackUnsubscribed,
      RoomEvent.TrackMuted,
      RoomEvent.TrackUnmuted,
      RoomEvent.LocalTrackPublished,
      RoomEvent.ParticipantNameChanged,
    ])
      this.room.on(event, refresh);
    this.room.on(RoomEvent.LocalTrackUnpublished, (publication) => {
      const kind = kindOf(publication.source);
      if (kind)
        this.watching.delete(
          streamId(this.room.localParticipant.identity, kind),
        );
      this.refresh();
    });
    this.room.on(RoomEvent.TrackUnpublished, (publication, participant) => {
      const kind = kindOf(publication.source);
      if (kind) this.watching.delete(streamId(participant.identity, kind));
      this.refresh();
    });
    this.room.on(RoomEvent.ParticipantDisconnected, (participant) => {
      this.watching.delete(streamId(participant.identity, "screen"));
      this.watching.delete(streamId(participant.identity, "camera"));
      this.refresh();
    });
    this.room.on(RoomEvent.ConnectionStateChanged, (state) => {
      this.connection =
        state === ConnectionState.Connected
          ? "connected"
          : state === ConnectionState.Reconnecting ||
              state === ConnectionState.SignalReconnecting
            ? "reconnecting"
            : state === ConnectionState.Connecting
              ? "connecting"
              : "disconnected";
      this.refresh();
    });
    this.room.on(RoomEvent.TrackSubscriptionFailed, () => {
      this.error =
        "That stream could not be received. Stop watching and try again.";
      this.refresh();
    });
  }

  subscribe(listener: (snapshot: MediaSnapshot) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  async connect(): Promise<void> {
    try {
      const access = await request<MediaAccess>(
        `/api/rooms/${this.roomId}/media-token`,
        { method: "POST", signal: this.abort.signal },
      );
      if (this.closed) return;
      await this.room.connect(access.serverUrl, access.token, {
        autoSubscribe: false,
      });
      if (this.closed) {
        await this.room.disconnect();
        return;
      }
      this.connection = "connected";
      this.refresh();
    } catch {
      if (this.closed) return;
      this.connection = "failed";
      this.error =
        "Could not connect to live sharing. Check your connection and try reconnecting.";
      this.refresh();
    }
  }

  watch(id: string, enabled: boolean): void {
    if (enabled) this.watching.add(id);
    else this.watching.delete(id);
    this.error = "";
    this.refresh();
  }

  async toggle(kind: StreamKind): Promise<void> {
    if (this.closed || this.pending || this.connection !== "connected") return;
    this.pending = kind;
    this.error = "";
    this.refresh();
    const captured: LocalTrack[] = [];
    try {
      const source =
        kind === "screen" ? Track.Source.ScreenShare : Track.Source.Camera;
      const existing = this.room.localParticipant.getTrackPublication(source);
      if (existing?.track) {
        const sources =
          kind === "screen"
            ? [Track.Source.ScreenShare, Track.Source.ScreenShareAudio]
            : [Track.Source.Camera];
        for (const current of sources) {
          const track =
            this.room.localParticipant.getTrackPublication(current)?.track;
          if (track)
            await this.room.localParticipant.unpublishTrack(track, true);
        }
        return;
      }
      if (kind === "camera") {
        captured.push(
          await createLocalVideoTrack({
            resolution: VideoPresets.h720.resolution,
          }),
        );
      } else {
        captured.push(
          ...(await createLocalScreenTracks({
            audio: {
              autoGainControl: false,
              echoCancellation: false,
              noiseSuppression: false,
            },
            resolution: { width: 1920, height: 1080, frameRate: 30 },
            selfBrowserSurface: "exclude",
            contentHint: "motion",
          })),
        );
      }
      for (const track of captured) {
        if (this.closed) throw new Error("Capture cancelled");
        await this.room.localParticipant.publishTrack(track, {
          source:
            kind === "camera"
              ? Track.Source.Camera
              : track.kind === Track.Kind.Audio
                ? Track.Source.ScreenShareAudio
                : Track.Source.ScreenShare,
          videoCodec: "vp8",
          simulcast: false,
        });
      }
    } catch {
      for (const track of captured) {
        await this.room.localParticipant
          .unpublishTrack(track, true)
          .catch(() => {});
        track.stop();
      }
      if (!this.closed)
        this.error =
          kind === "screen"
            ? "Screen sharing did not start. Choose a source and allow capture to try again."
            : "Camera sharing did not start. Check camera permissions and try again.";
    } finally {
      this.pending = null;
      this.refresh();
    }
  }

  async disconnect(): Promise<void> {
    this.closed = true;
    this.abort.abort();
    this.listeners.clear();
    this.watching.clear();
    await this.room.disconnect(true);
    this.room.removeAllListeners();
  }

  private refresh(): void {
    if (this.closed) return;
    const participants = [
      this.room.localParticipant,
      ...this.room.remoteParticipants.values(),
    ];
    for (const participant of this.room.remoteParticipants.values()) {
      for (const publication of participant.trackPublications.values()) {
        const kind = kindOf(publication.source);
        const desired = kind
          ? this.watching.has(streamId(participant.identity, kind))
          : publication.source === Track.Source.ScreenShareAudio &&
            this.watching.has(streamId(participant.identity, "screen"));
        if (publication.isDesired !== desired)
          publication.setSubscribed(desired);
      }
    }
    const snapshot: MediaSnapshot = {
      connection: this.connection,
      pending: this.pending,
      error: this.error,
      participants: participants
        .filter((participant) => participant.identity)
        .map((participant) => ({
          id: participant.identity,
          name: participant.name || "Friend",
          local: participant.isLocal,
        })),
      streams: participants.flatMap((participant) => this.streams(participant)),
    };
    for (const listener of this.listeners) listener(snapshot);
  }

  private streams(participant: Participant): LiveStream[] {
    return [...participant.videoTrackPublications.values()].flatMap(
      (publication) => {
        const kind = kindOf(publication.source);
        if (!kind) return [];
        const id = streamId(participant.identity, kind);
        const watching = this.watching.has(id);
        const audio =
          kind === "screen"
            ? participant.getTrackPublication(Track.Source.ScreenShareAudio)
                ?.track
            : undefined;
        return [
          {
            id,
            publisherId: participant.identity,
            publisherName: participant.name || "Friend",
            kind,
            local: participant.isLocal,
            muted: publication.isMuted,
            watching,
            video: watching
              ? (publication.track?.mediaStreamTrack ?? null)
              : null,
            audio:
              watching && !participant.isLocal
                ? (audio?.mediaStreamTrack ?? null)
                : null,
          },
        ];
      },
    );
  }
}
