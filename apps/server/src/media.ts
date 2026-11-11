import {
  RoomServiceClient,
  TrackSource,
  TrackType,
  TwirpError,
} from "livekit-server-sdk";
import type { PreviewKind } from "@freedomstream/contracts";
import type { Config } from "./config.js";

export const mediaRoomName = (roomId: string) => `freedomstream_${roomId}`;
export type MediaParticipant = {
  attributes?: Record<string, string>;
  tracks: { sid: string; kind: PreviewKind; muted: boolean }[];
};
export interface MediaDirectory {
  diagnosticParticipants?(
    roomId: string,
  ): Promise<Map<string, MediaParticipant>>;
  members(roomId: string): Promise<{ id: string; displayName: string }[]>;
  remove(roomId: string, identity: string): Promise<void>;
  participant(
    roomId: string,
    identity: string,
  ): Promise<MediaParticipant | null>;
  attribute(
    roomId: string,
    identity: string,
    key: string,
    value: string,
  ): Promise<void>;
}
export class MediaUnavailable extends Error {
  constructor() {
    super("Private preview service is temporarily unavailable.");
  }
}

/** Owns administrative SFU access; SDK errors/URLs never escape this module. */
export class LivekitDirectory implements MediaDirectory {
  private readonly client: RoomServiceClient;
  constructor(config: Config) {
    this.client = new RoomServiceClient(
      config.livekitInternalUrl,
      config.livekitApiKey,
      config.livekitApiSecret,
      { requestTimeout: 3, failover: false },
    );
  }
  async diagnosticParticipants(
    roomId: string,
  ): Promise<Map<string, MediaParticipant>> {
    try {
      const participants = await this.client.listParticipants(
        mediaRoomName(roomId),
      );
      return new Map(
        participants
          .slice(0, 100)
          .map((p) => [p.identity, diagnosticParticipant(p)]),
      );
    } catch {
      throw new MediaUnavailable();
    }
  }
  async members(
    roomId: string,
  ): Promise<{ id: string; displayName: string }[]> {
    try {
      return (await this.client.listParticipants(mediaRoomName(roomId)))
        .slice(0, 100)
        .map((participant) => ({
          id: participant.identity,
          displayName: participant.name.slice(0, 60) || "Participant",
        }));
    } catch (error) {
      if (error instanceof TwirpError && error.code === "not_found") return [];
      throw new MediaUnavailable();
    }
  }
  async remove(roomId: string, identity: string): Promise<void> {
    try {
      await this.client.removeParticipant(mediaRoomName(roomId), identity);
    } catch (error) {
      if (error instanceof TwirpError && error.code === "not_found") return;
      throw new MediaUnavailable();
    }
  }
  async participant(
    roomId: string,
    identity: string,
  ): Promise<MediaParticipant | null> {
    try {
      const participant = await this.client.getParticipant(
        mediaRoomName(roomId),
        identity,
      );
      return diagnosticParticipant(participant);
    } catch (error) {
      if (error instanceof TwirpError && error.code === "not_found")
        return null;
      throw new MediaUnavailable();
    }
  }
  async attribute(
    roomId: string,
    identity: string,
    key: string,
    value: string,
  ): Promise<void> {
    try {
      await this.client.updateParticipant(mediaRoomName(roomId), identity, {
        attributes: { [key]: value },
      });
    } catch (error) {
      throw new MediaUnavailable();
    }
  }
}

function diagnosticParticipant(participant: {
  attributes: Record<string, string>;
  tracks: {
    sid: string;
    type: TrackType;
    source: TrackSource;
    muted: boolean;
  }[];
}): MediaParticipant {
  return {
    attributes: participant.attributes,
    tracks: participant.tracks.flatMap((track) => {
      if (
        track.type !== TrackType.VIDEO ||
        ![TrackSource.CAMERA, TrackSource.SCREEN_SHARE].includes(track.source)
      )
        return [];
      return [
        {
          sid: track.sid,
          kind:
            track.source === TrackSource.CAMERA
              ? ("camera" as const)
              : ("screen" as const),
          muted: track.muted,
        },
      ];
    }),
  };
}
