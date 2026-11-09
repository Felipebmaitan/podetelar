import {
  ConnectionState,
  LocalAudioTrack,
  LocalVideoTrack,
  LogLevel,
  Room,
  RoomEvent,
  Track,
  setLogLevel,
} from "livekit-client";
import type { LocalTrack, Participant } from "livekit-client";
import type { MediaAccess } from "@freedomstream/contracts";
import { previewAttribute, previewSchema } from "@freedomstream/contracts";
import { ApiError, request } from "../api";
import type {
  LiveStream,
  MediaSnapshot,
  StreamKind,
  PlaybackBuffer,
  ConnectionRoute,
} from "./model";
import { BrowserPreviews } from "./BrowserPreviews";
import { reconnectPolicy } from "./recovery";
import { networkSummary } from "./network";

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
  private readonly receivesHevc =
    globalThis.RTCRtpReceiver?.getCapabilities("video")?.codecs.some(
      (codec) => codec.mimeType.toLowerCase() === "video/h265",
    ) ?? false;
  private readonly room = new Room({
    adaptiveStream: false,
    dynacast: false,
    publishDefaults: { videoCodec: "vp8", simulcast: false },
    reconnectPolicy,
  });
  private readonly watching = new Set<string>();
  private readonly captures = new Set<LocalTrack>();
  private readonly captureEndings = new Map<LocalTrack, () => void>();
  private readonly listeners = new Set<(snapshot: MediaSnapshot) => void>();
  private readonly abort = new AbortController();
  private closed = false;
  private connection: MediaSnapshot["connection"] = "connecting";
  private pending: StreamKind | null = null;
  private error = "";
  private reconciliationQueued = false;
  private readonly previews: BrowserPreviews;
  private statsTimer: ReturnType<typeof setInterval> | null = null;
  private sampling = false;
  private playbackBuffer: PlaybackBuffer = "smooth";
  private readonly delivery = new Map<
    string,
    {
      sid: string;
      at: number;
      bytes: number;
      frames: number;
      info: LiveStream["delivery"];
    }
  >();

  constructor(
    private readonly roomId: string,
    private readonly route: ConnectionRoute = "auto",
  ) {
    this.previews = new BrowserPreviews(roomId);
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
      RoomEvent.ParticipantAttributesChanged,
    ])
      this.room.on(event, refresh);
    this.room.on(RoomEvent.TrackSubscribed, (track) => {
      track?.setPlayoutDelay?.(this.playbackBuffer === "smooth" ? 0.3 : 0);
    });
    this.room.on(RoomEvent.LocalTrackUnpublished, () => {
      this.reconcileRemovals();
      this.refresh();
    });
    this.room.on(RoomEvent.TrackUnpublished, () => {
      this.reconcileRemovals();
      this.refresh();
    });
    this.room.on(RoomEvent.ParticipantDisconnected, () => {
      this.reconcileRemovals();
      this.refresh();
    });
    this.room.on(RoomEvent.Reconnected, () => {
      this.reconcileCaptures();
      this.reconcileRemovals();
    });
    this.room.on(RoomEvent.ConnectionStateChanged, (state) => {
      if (state === ConnectionState.Disconnected)
        this.stopCaptures([...this.captures]);
      if (state !== ConnectionState.Connected) this.delivery.clear();
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
        maxRetries: 0,
        websocketTimeout: 10_000,
        peerConnectionTimeout: 15_000,
        rtcConfig: {
          iceTransportPolicy: this.route === "relay" ? "relay" : "all",
        },
      });
      if (this.closed) {
        await this.room.disconnect();
        return;
      }
      this.connection = "connected";
      if (typeof window !== "undefined" && this.statsTimer === null)
        this.statsTimer = setInterval(() => void this.sampleDelivery(), 1000);
      this.refresh();
    } catch (caught) {
      if (this.closed) return;
      this.stopCaptures([...this.captures]);
      this.connection = "failed";
      this.error =
        caught instanceof ApiError && caught.status === 401
          ? "Your room access ended. Open a current invitation to join again."
          : "Could not connect to live sharing. Check your connection and try reconnecting.";
      this.refresh();
    }
  }

  watch(id: string, enabled: boolean): void {
    if (!enabled) this.delivery.delete(id);
    const stream = [
      this.room.localParticipant,
      ...this.room.remoteParticipants.values(),
    ]
      .flatMap((participant) => this.streams(participant))
      .find((stream) => stream.id === id);
    if (enabled && stream && !stream.compatible) {
      this.error =
        "This browser cannot receive HEVC. Ask the publisher to choose H.264.";
      this.refresh();
      return;
    }
    if (enabled) this.watching.add(id);
    else this.watching.delete(id);
    this.error = "";
    this.refresh();
  }

  setPlaybackBuffer(value: PlaybackBuffer): void {
    this.playbackBuffer = value;
    for (const participant of this.room.remoteParticipants.values())
      for (const publication of participant.trackPublications.values())
        publication.track?.setPlayoutDelay?.(value === "smooth" ? 0.3 : 0);
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
        const tracks = sources.flatMap(
          (current) =>
            this.room.localParticipant.getTrackPublication(current)?.track ??
            [],
        );
        this.watching.delete(
          streamId(this.room.localParticipant.identity, kind),
        );
        this.stopCaptures(tracks);
        await Promise.all(
          tracks.map((track) =>
            this.room.localParticipant.unpublishTrack(track, true),
          ),
        );
        return;
      }
      captured.push(...(await this.capture(kind)));
      // Own captures before handing them to asynchronous SDK publication.
      this.ownCaptures(captured);
      for (const track of captured) {
        if (
          this.closed ||
          this.connection !== "connected" ||
          track.mediaStreamTrack.readyState === "ended"
        )
          throw new Error("Capture cancelled");
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
        if (track.kind === Track.Kind.Video)
          this.previews.start(
            track.mediaStreamTrack,
            kind,
            () =>
              [...this.room.localParticipant.trackPublications.values()].find(
                (publication) => publication.track === track,
              )?.trackSid,
          );
      }
    } catch {
      this.stopCaptures(captured);
      await Promise.allSettled(
        captured.map((track) =>
          this.room.localParticipant.unpublishTrack(track, true),
        ),
      );
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
    if (this.statsTimer !== null) clearInterval(this.statsTimer);
    this.delivery.clear();
    this.stopCaptures([...this.captures]);
    this.listeners.clear();
    this.watching.clear();
    await this.room.disconnect(true);
    this.room.removeAllListeners();
  }

  private stopCaptures(tracks: LocalTrack[]): void {
    for (const track of tracks) {
      this.previews.stop(track.mediaStreamTrack);
      const ended = this.captureEndings.get(track);
      if (ended) {
        track.mediaStreamTrack.removeEventListener("ended", ended);
        this.captureEndings.delete(track);
      }
      track.stop();
      this.captures.delete(track);
    }
  }

  private async capture(kind: StreamKind): Promise<LocalTrack[]> {
    const screenOptions: DisplayMediaStreamOptions & {
      selfBrowserSurface: "exclude";
    } = {
      video: { width: 1920, height: 1080, frameRate: 30 },
      audio: {
        autoGainControl: false,
        echoCancellation: false,
        noiseSuppression: false,
      },
      selfBrowserSurface: "exclude",
    };
    const stream =
      kind === "camera"
        ? await navigator.mediaDevices.getUserMedia({
            video: { width: 1280, height: 720 },
            audio: false,
          })
        : await navigator.mediaDevices.getDisplayMedia(screenOptions);
    const tracks: LocalTrack[] = [];
    try {
      if (this.closed || this.connection !== "connected")
        throw new Error("Capture cancelled");
      if (!stream.getVideoTracks().length) throw new Error("No video capture");
      for (const track of stream.getTracks()) {
        if (track.kind === "video") {
          if (kind === "screen") track.contentHint = "motion";
          tracks.push(new LocalVideoTrack(track, undefined, true));
        } else {
          // SDK 2.23 redeclares a protected optional audio processor with an
          // explicit undefined, conflicting with exactOptionalPropertyTypes.
          tracks.push(
            new LocalAudioTrack(track, undefined, true) as LocalTrack,
          );
        }
      }
      return tracks;
    } catch (error) {
      this.stopCaptures(tracks);
      for (const track of stream.getTracks()) track.stop();
      throw error;
    }
  }

  private ownCaptures(tracks: LocalTrack[]): void {
    for (const track of tracks) this.captures.add(track);
    const video = tracks.find((track) => track.kind === Track.Kind.Video);
    if (!video) return;
    // All capture/reacquisition is application-owned. Native video termination
    // releases paired audio even while publication is pending. SDK reconnect
    // unpublication does not emit a native ended event and retains the captures.
    const ended = () => {
      this.stopCaptures(tracks);
      void Promise.allSettled(
        tracks.map((track) =>
          this.room.localParticipant.unpublishTrack(track, true),
        ),
      ).then(() => {
        this.reconcileRemovals();
        this.refresh();
      });
    };
    for (const track of tracks) this.captureEndings.set(track, ended);
    video.mediaStreamTrack.addEventListener("ended", ended, { once: true });
    if (video.mediaStreamTrack.readyState === "ended") ended();
  }

  private reconcileCaptures(): void {
    if (this.closed || this.room.state !== ConnectionState.Connected) return;
    const published = new Set(
      [...this.room.localParticipant.trackPublications.values()].map(
        (publication) => publication.track,
      ),
    );
    // Room recovery can succeed even when track republication failed. A missing
    // member ends its whole capture group so no invisible device/audio remains.
    for (const track of this.captures) {
      if (published.has(track)) continue;
      this.captureEndings.get(track)?.();
      this.error = "Sharing stopped during reconnect. Share again to retry.";
    }
    this.refresh();
  }

  private reconcileRemovals(): void {
    if (this.closed || this.reconciliationQueued) return;
    this.reconciliationQueued = true;
    // The pinned SDK emits removal events before entering Reconnecting. Defer until
    // that synchronous event batch ends, then reconcile only a stable connected room.
    queueMicrotask(() => {
      this.reconciliationQueued = false;
      if (this.closed || this.room.state !== ConnectionState.Connected) return;
      const present = new Set(
        [
          this.room.localParticipant,
          ...this.room.remoteParticipants.values(),
        ].flatMap((participant) =>
          this.streams(participant).map((stream) => stream.id),
        ),
      );
      for (const id of this.watching)
        if (!present.has(id)) {
          this.watching.delete(id);
          this.delivery.delete(id);
        }
      this.refresh();
    });
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
        if (kind && !this.compatible(publication.mimeType)) {
          const id = streamId(participant.identity, kind);
          this.watching.delete(id);
          this.delivery.delete(id);
        }
        const desired = kind
          ? this.watching.has(streamId(participant.identity, kind))
          : publication.source === Track.Source.ScreenShareAudio &&
            this.compatible(
              participant.getTrackPublication(Track.Source.ScreenShare)
                ?.mimeType,
            ) &&
            this.watching.has(streamId(participant.identity, "screen"));
        if (publication.isDesired !== desired) {
          if (!desired) publication.track?.setPlayoutDelay?.(0);
          publication.setSubscribed(desired);
        }
      }
    }
    const snapshot: MediaSnapshot = {
      connection: this.connection,
      pending: this.pending,
      error: this.error,
      playbackBuffer: this.playbackBuffer,
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

  private compatible(mimeType: string | undefined): boolean {
    return mimeType?.toLowerCase() !== "video/h265" || this.receivesHevc;
  }

  private async sampleDelivery(): Promise<void> {
    if (this.closed || this.connection !== "connected" || this.sampling) return;
    this.sampling = true;
    try {
      await Promise.all(
        [...this.room.remoteParticipants.values()].flatMap((participant) =>
          [...participant.videoTrackPublications.values()].map(
            async (publication) => {
              const kind = kindOf(publication.source);
              if (!kind) return;
              const id = streamId(participant.identity, kind);
              const track = publication.track;
              if (!track || !this.watching.has(id)) {
                this.delivery.delete(id);
                return;
              }
              try {
                const stats = await track.getRTCStatsReport();
                if (
                  this.closed ||
                  !this.watching.has(id) ||
                  this.room.remoteParticipants
                    .get(participant.identity)
                    ?.getTrackPublication(publication.source)?.trackSid !==
                    publication.trackSid
                )
                  return;
                let found = false;
                stats?.forEach((stat) => {
                  if (stat.type !== "inbound-rtp" || stat.kind !== "video")
                    return;
                  found = true;
                  const at = performance.now();
                  const bytes = Number(stat.bytesReceived ?? 0);
                  const frames = Number(stat.framesDecoded ?? 0);
                  const before = this.delivery.get(id);
                  const valid =
                    before?.sid === publication.trackSid &&
                    bytes >= before.bytes &&
                    frames >= before.frames;
                  const seconds = valid
                    ? Math.max((at - before.at) / 1000, 0.001)
                    : 0;
                  this.delivery.set(id, {
                    sid: publication.trackSid,
                    at,
                    bytes,
                    frames,
                    info: seconds
                      ? {
                          width: Number(stat.frameWidth ?? 0),
                          height: Number(stat.frameHeight ?? 0),
                          fps: (frames - before!.frames) / seconds,
                          bitrate: ((bytes - before!.bytes) * 8) / seconds,
                          network: networkSummary(stats),
                        }
                      : null,
                  });
                });
                if (!found) this.delivery.delete(id);
              } catch {
                this.delivery.delete(id);
              }
            },
          ),
        ),
      );
      this.refresh();
    } finally {
      this.sampling = false;
    }
  }

  private streams(participant: Participant): LiveStream[] {
    return [...participant.videoTrackPublications.values()].flatMap(
      (publication) => {
        const kind = kindOf(publication.source);
        if (!kind) return [];
        const id = streamId(participant.identity, kind);
        const watching = this.watching.has(id);
        const rawPreview = participant.attributes?.[previewAttribute(kind)];
        const parsedPreview = (() => {
          if (!rawPreview || rawPreview.length > 512) return null;
          try {
            const parsed = previewSchema.safeParse(JSON.parse(rawPreview));
            return parsed.success &&
              parsed.data.trackSid === publication.trackSid
              ? parsed.data
              : null;
          } catch {
            return null;
          }
        })();
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
            codec:
              publication.mimeType?.replace(/^video\//i, "").toUpperCase() ??
              "",
            compatible: this.compatible(publication.mimeType),
            local: participant.isLocal,
            muted: publication.isMuted,
            watching,
            preview: parsedPreview,
            delivery:
              watching && this.delivery.get(id)?.sid === publication.trackSid
                ? (this.delivery.get(id)?.info ?? null)
                : null,
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
