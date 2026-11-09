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
  tracks: { sid: string; kind: PreviewKind; muted: boolean }[];
};
export interface MediaDirectory {
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
  async participant(
    roomId: string,
    identity: string,
  ): Promise<MediaParticipant | null> {
    try {
      const participant = await this.client.getParticipant(
        mediaRoomName(roomId),
        identity,
      );
      return {
        tracks: participant.tracks.flatMap((track) => {
          if (
            track.type !== TrackType.VIDEO ||
            ![TrackSource.CAMERA, TrackSource.SCREEN_SHARE].includes(
              track.source,
            )
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
