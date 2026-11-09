import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { mock, test } from "node:test";
import type { TestContext } from "node:test";
import { setImmediate } from "node:timers/promises";
import type { MediaSnapshot } from "../src/media/model.ts";

const Source = {
  Camera: "camera",
  ScreenShare: "screen",
  ScreenShareAudio: "screen-audio",
};
const events = {
  ParticipantConnected: "participantConnected",
  ParticipantDisconnected: "participantDisconnected",
  TrackPublished: "trackPublished",
  TrackSubscribed: "trackSubscribed",
  TrackUnsubscribed: "trackUnsubscribed",
  TrackMuted: "trackMuted",
  TrackUnmuted: "trackUnmuted",
  LocalTrackPublished: "localTrackPublished",
  LocalTrackUnpublished: "localTrackUnpublished",
  ParticipantNameChanged: "participantNameChanged",
  TrackUnpublished: "trackUnpublished",
  ConnectionStateChanged: "connectionStateChanged",
  Reconnected: "reconnected",
  TrackSubscriptionFailed: "trackSubscriptionFailed",
};
const states = {
  Connected: "connected",
  Connecting: "connecting",
  Reconnecting: "reconnecting",
  SignalReconnecting: "signalReconnecting",
};
class Capture {
  readonly mediaStreamTrack = { readyState: "live" };
  constructor(readonly kind = "video") {}
  stop() {
    this.mediaStreamTrack.readyState = "ended";
  }
}
class Publication {
  isDesired = false;
  readonly isMuted = false;
  constructor(
    readonly source: string,
    readonly track: Capture | undefined = undefined,
  ) {}
  setSubscribed(value: boolean) {
    this.isDesired = value;
  }
}
class Peer {
  readonly trackPublications = new Map<string, Publication>();
  constructor(
    readonly identity: string,
    readonly isLocal = false,
  ) {}
  get name() {
    return this.identity;
  }
  get videoTrackPublications() {
    return new Map(
      [...this.trackPublications].filter(
        ([, publication]) => publication.source !== Source.ScreenShareAudio,
      ),
    );
  }
  getTrackPublication(source: string) {
    return this.trackPublications.get(source);
  }
}
class Transport extends EventEmitter {
  static latest: Transport;
  state = states.Connecting;
  readonly remoteParticipants = new Map<string, Peer>();
  readonly localParticipant = Object.assign(new Peer("self", true), {
    publishTrack: async (track: Capture, options: { source: string }) => {
      const publication = new Publication(options.source, track);
      this.localParticipant.trackPublications.set(options.source, publication);
      this.emit(events.LocalTrackPublished, publication);
    },
    unpublishTrack: async (track: Capture) => {
      track.stop();
      for (const [source, publication] of this.localParticipant
        .trackPublications)
        if (publication.track === track) {
          this.localParticipant.trackPublications.delete(source);
          this.emit(events.LocalTrackUnpublished, publication);
        }
    },
  });
  constructor() {
    super();
    Transport.latest = this;
  }
  async connect() {
    this.state = states.Connected;
    this.emit(events.ConnectionStateChanged, this.state);
  }
  async disconnect() {
    this.state = "disconnected";
  }
}
let camera: () => Promise<Capture> = async () => new Capture();
let screen: () => Promise<Capture[]> = async () => [
  new Capture(),
  new Capture("audio"),
];
mock.module("livekit-client", {
  namedExports: {
    Room: Transport,
    RoomEvent: events,
    ConnectionState: states,
    Track: { Source, Kind: { Audio: "audio", Video: "video" } },
    VideoPresets: { h720: { resolution: { width: 1280, height: 720 } } },
    LogLevel: { silent: 0 },
    setLogLevel() {},
    createLocalVideoTrack: () => camera(),
    createLocalScreenTracks: () => screen(),
  },
});
const { MediaRoom } = await import("../src/media/MediaRoom.ts");
const deferred = () => {
  let resolve!: () => void;
  const promise = new Promise<void>((done) => {
    resolve = done;
  });
  return { promise, resolve };
};
async function fixture(t: TestContext) {
  const fetchMock = mock.method(
    globalThis,
    "fetch",
    async () =>
      new Response(
        JSON.stringify({
          serverUrl: "ws://test.invalid",
          token: "test-token",
          expiresInSeconds: 120,
        }),
      ),
  );
  const media = new MediaRoom("test-room");
  const transport = Transport.latest;
  let snapshot: MediaSnapshot | undefined;
  media.subscribe((value) => {
    snapshot = value;
  });
  await media.connect();
  t.after(async () => {
    await media.disconnect();
    fetchMock.mock.restore();
  });
  return { media, transport, snapshot: () => snapshot! };
}

test("leaving stops acquired capture while SDK publication is still pending", async (t) => {
  const capture = new Capture();
  camera = async () => capture;
  const f = await fixture(t);
  const entered = deferred();
  const publication = deferred();
  f.transport.localParticipant.publishTrack = async () => {
    entered.resolve();
    await publication.promise;
  };
  const pending = f.media.toggle("camera");
  await entered.promise;
  await f.media.disconnect();
  assert.equal(capture.mediaStreamTrack.readyState, "ended");
  publication.resolve();
  await pending;
});

test("all acquired screen tracks stop before failed-publication cleanup can settle", async (t) => {
  const tracks = [new Capture(), new Capture("audio")];
  screen = async () => tracks;
  const f = await fixture(t);
  const cleaning = deferred();
  const cleanup = deferred();
  f.transport.localParticipant.publishTrack = async () => {
    throw new Error("Publication failed");
  };
  f.transport.localParticipant.unpublishTrack = async () => {
    cleaning.resolve();
    await cleanup.promise;
  };
  const pending = f.media.toggle("screen");
  await cleaning.promise;
  assert.ok(
    tracks.every((track) => track.mediaStreamTrack.readyState === "ended"),
  );
  cleanup.resolve();
  await pending;
});

test("capture permissions resolving after leave cannot publish or retain a device", async (t) => {
  const granted = deferred();
  const entered = deferred();
  const capture = new Capture();
  camera = async () => {
    entered.resolve();
    await granted.promise;
    return capture;
  };
  const f = await fixture(t);
  const publish = mock.method(f.transport.localParticipant, "publishTrack");
  const pending = f.media.toggle("camera");
  await entered.promise;
  await f.media.disconnect();
  granted.resolve();
  await pending;
  assert.equal(capture.mediaStreamTrack.readyState, "ended");
  assert.equal(publish.mock.callCount(), 0);
});

function addPeer(transport: Transport) {
  const peer = new Peer("friend");
  peer.trackPublications.set(
    Source.ScreenShare,
    new Publication(Source.ScreenShare),
  );
  peer.trackPublications.set(
    Source.ScreenShareAudio,
    new Publication(Source.ScreenShareAudio),
  );
  transport.remoteParticipants.set(peer.identity, peer);
  transport.emit(events.ParticipantConnected, peer);
  return peer;
}
function removePeer(transport: Transport, peer: Peer) {
  transport.remoteParticipants.delete(peer.identity);
  for (const publication of peer.trackPublications.values())
    transport.emit(events.TrackUnpublished, publication, peer);
  transport.emit(events.ParticipantDisconnected, peer);
}

test("full reconnect preserves Watch across SDK teardown events preceding Reconnecting", async (t) => {
  const f = await fixture(t);
  const before = addPeer(f.transport);
  f.media.watch("friend:screen", true);
  assert.equal(before.getTrackPublication(Source.ScreenShare)?.isDesired, true);
  removePeer(f.transport, before);
  f.transport.state = states.Reconnecting;
  f.transport.emit(events.ConnectionStateChanged, states.Reconnecting);
  await setImmediate();
  const resumed = addPeer(f.transport);
  f.transport.state = states.Connected;
  f.transport.emit(events.ConnectionStateChanged, states.Connected);
  f.transport.emit(events.Reconnected);
  await setImmediate();
  assert.equal(
    resumed.getTrackPublication(Source.ScreenShare)?.isDesired,
    true,
  );
  assert.equal(
    resumed.getTrackPublication(Source.ScreenShareAudio)?.isDesired,
    true,
  );
  assert.equal(f.snapshot().streams[0]?.watching, true);
});

test("genuine removals prune Watch and do not auto-subscribe a later publication", async (t) => {
  const f = await fixture(t);
  const before = addPeer(f.transport);
  f.media.watch("friend:screen", true);
  removePeer(f.transport, before);
  await setImmediate();
  const later = addPeer(f.transport);
  assert.equal(later.getTrackPublication(Source.ScreenShare)?.isDesired, false);
  assert.equal(
    later.getTrackPublication(Source.ScreenShareAudio)?.isDesired,
    false,
  );
});

test("recovery prunes a publisher that really is absent from the rejoined room", async (t) => {
  const f = await fixture(t);
  const before = addPeer(f.transport);
  f.media.watch("friend:screen", true);
  removePeer(f.transport, before);
  f.transport.state = states.Reconnecting;
  f.transport.emit(events.ConnectionStateChanged, states.Reconnecting);
  await setImmediate();
  f.transport.state = states.Connected;
  f.transport.emit(events.ConnectionStateChanged, states.Connected);
  f.transport.emit(events.Reconnected);
  await setImmediate();
  const later = addPeer(f.transport);
  assert.equal(later.getTrackPublication(Source.ScreenShare)?.isDesired, false);
});
