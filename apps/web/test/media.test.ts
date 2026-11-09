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
  ParticipantAttributesChanged: "participantAttributesChanged",
  TrackUnpublished: "trackUnpublished",
  ConnectionStateChanged: "connectionStateChanged",
  Reconnected: "reconnected",
  TrackSubscriptionFailed: "trackSubscriptionFailed",
};
const states = {
  Disconnected: "disconnected",
  Connected: "connected",
  Connecting: "connecting",
  Reconnecting: "reconnecting",
  SignalReconnecting: "signalReconnecting",
};
class BrowserTrack extends EventTarget {
  readyState = "live";
  constructor(readonly kind = "video") {
    super();
  }
  stop() {
    this.readyState = "ended";
  }
}
class Capture {
  constructor(
    readonly kind = "video",
    readonly mediaStreamTrack = new BrowserTrack(kind),
    readonly isUserProvided = true,
  ) {}
  stop() {
    this.mediaStreamTrack.readyState = "ended";
  }
  endNaturally() {
    this.mediaStreamTrack.readyState = "ended";
    this.mediaStreamTrack.dispatchEvent(new Event("ended"));
  }
}
class Publication {
  readonly trackSid: string;
  isDesired = false;
  readonly isMuted = false;
  constructor(
    readonly source: string,
    readonly track: Capture | undefined = undefined,
  ) {
    this.trackSid = `TR_${source}`;
  }
  setSubscribed(value: boolean) {
    this.isDesired = value;
  }
}
class Peer {
  attributes: Record<string, string> = {};
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
    LocalVideoTrack: class extends Capture {
      constructor(
        track: BrowserTrack,
        _constraints: unknown,
        supplied: boolean,
      ) {
        super("video", track, supplied);
      }
    },
    LocalAudioTrack: class extends Capture {
      constructor(
        track: BrowserTrack,
        _constraints: unknown,
        supplied: boolean,
      ) {
        super("audio", track, supplied);
      }
    },
    RoomEvent: events,
    ConnectionState: states,
    Track: { Source, Kind: { Audio: "audio", Video: "video" } },
    LogLevel: { silent: 0 },
    setLogLevel() {},
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
  const stream = (captures: Capture[]) => ({
    getTracks: () => captures.map((capture) => capture.mediaStreamTrack),
    getVideoTracks: () =>
      captures
        .filter((capture) => capture.kind === "video")
        .map((capture) => capture.mediaStreamTrack),
  });
  const navigatorMock = mock.getter(
    globalThis,
    "navigator",
    () =>
      ({
        mediaDevices: {
          getUserMedia: async () => stream([await camera()]),
          getDisplayMedia: async () => stream(await screen()),
        },
      }) as unknown as Navigator,
  );
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
    navigatorMock.mock.restore();
  });
  return { media, transport, snapshot: () => snapshot! };
}

test("camera capture is application-supplied and natural termination releases it", async (t) => {
  const capture = new Capture();
  camera = async () => capture;
  const f = await fixture(t);
  await f.media.toggle("camera");
  assert.equal(
    f.transport.localParticipant.getTrackPublication(Source.Camera)?.track
      ?.isUserProvided,
    true,
  );
  capture.endNaturally();
  await setImmediate();
  assert.equal(f.transport.localParticipant.trackPublications.size, 0);
  assert.equal(capture.mediaStreamTrack.readyState, "ended");
});

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

test("browser ending screen video stops and unpublishes its paired audio", async (t) => {
  const tracks = [new Capture(), new Capture("audio")];
  screen = async () => tracks;
  const f = await fixture(t);
  await f.media.toggle("screen");
  assert.equal(f.transport.localParticipant.trackPublications.size, 2);
  tracks[0]!.endNaturally();
  assert.ok(
    tracks.every((track) => track.mediaStreamTrack.readyState === "ended"),
  );
  await setImmediate();
  assert.equal(f.transport.localParticipant.trackPublications.size, 0);
});

test("browser ending screen video also stops audio with publication still pending", async (t) => {
  const tracks = [new Capture(), new Capture("audio")];
  screen = async () => tracks;
  const f = await fixture(t);
  const entered = deferred();
  const publication = deferred();
  const publish = f.transport.localParticipant.publishTrack;
  f.transport.localParticipant.publishTrack = async (track, options) => {
    if (track.kind === "audio") {
      entered.resolve();
      await publication.promise;
    }
    await publish(track, options);
  };
  const unpublish = f.transport.localParticipant.unpublishTrack;
  f.transport.localParticipant.unpublishTrack = async (track) => {
    if (track.kind === "audio") await publication.promise;
    await unpublish(track);
  };
  const pending = f.media.toggle("screen");
  await entered.promise;
  tracks[0]!.endNaturally();
  assert.equal(tracks[1]!.mediaStreamTrack.readyState, "ended");
  publication.resolve();
  await pending;
  await setImmediate();
  assert.equal(f.transport.localParticipant.trackPublications.size, 0);
});

test("temporary SDK unpublication keeps acquired screen audio alive for republishing", async (t) => {
  const tracks = [new Capture(), new Capture("audio")];
  screen = async () => tracks;
  const f = await fixture(t);
  await f.media.toggle("screen");
  const video = f.transport.localParticipant.getTrackPublication(
    Source.ScreenShare,
  )!;
  f.transport.localParticipant.trackPublications.delete(Source.ScreenShare);
  f.transport.emit(events.LocalTrackUnpublished, video);
  f.transport.state = states.Reconnecting;
  await setImmediate();
  assert.ok(
    tracks.every((track) => track.mediaStreamTrack.readyState === "live"),
  );
  tracks[0]!.endNaturally();
  assert.equal(tracks[1]!.mediaStreamTrack.readyState, "ended");
});

test("failed republication and terminal disconnect release captures absent from SDK maps", async (t) => {
  const tracks = [new Capture(), new Capture("audio")];
  screen = async () => tracks;
  const f = await fixture(t);
  await f.media.toggle("screen");
  for (const publication of f.transport.localParticipant.trackPublications.values())
    f.transport.emit(events.LocalTrackUnpublished, publication);
  f.transport.localParticipant.trackPublications.clear();
  f.transport.state = states.Reconnecting;
  f.transport.emit(events.ConnectionStateChanged, states.Reconnecting);
  assert.ok(
    tracks.every((track) => track.mediaStreamTrack.readyState === "live"),
  );
  f.transport.state = states.Disconnected;
  f.transport.emit(events.ConnectionStateChanged, states.Disconnected);
  assert.ok(
    tracks.every((track) => track.mediaStreamTrack.readyState === "ended"),
  );
  assert.equal(f.snapshot().connection, "disconnected");
});

test("permissions resolving after terminal disconnection cannot start publication", async (t) => {
  const granted = deferred();
  const entered = deferred();
  const tracks = [new Capture(), new Capture("audio")];
  screen = async () => {
    entered.resolve();
    await granted.promise;
    return tracks;
  };
  const f = await fixture(t);
  const publish = mock.method(f.transport.localParticipant, "publishTrack");
  const pending = f.media.toggle("screen");
  await entered.promise;
  f.transport.state = states.Disconnected;
  f.transport.emit(events.ConnectionStateChanged, states.Disconnected);
  granted.resolve();
  await pending;
  assert.ok(
    tracks.every((track) => track.mediaStreamTrack.readyState === "ended"),
  );
  assert.equal(publish.mock.callCount(), 0);
});

test("successful room recovery releases failed camera republication before another share", async (t) => {
  const first = new Capture();
  camera = async () => first;
  const f = await fixture(t);
  await f.media.toggle("camera");
  const publication = f.transport.localParticipant.getTrackPublication(
    Source.Camera,
  )!;
  f.transport.localParticipant.trackPublications.clear();
  f.transport.emit(events.LocalTrackUnpublished, publication);
  f.transport.state = states.Reconnecting;
  f.transport.emit(events.ConnectionStateChanged, states.Reconnecting);
  assert.equal(first.mediaStreamTrack.readyState, "live");
  f.transport.state = states.Connected;
  f.transport.emit(events.ConnectionStateChanged, states.Connected);
  f.transport.emit(events.Reconnected);
  assert.equal(first.mediaStreamTrack.readyState, "ended");
  const second = new Capture();
  camera = async () => second;
  await f.media.toggle("camera");
  assert.equal(first.mediaStreamTrack.readyState, "ended");
  assert.equal(second.mediaStreamTrack.readyState, "live");
});

test("partial screen republication ends the entire video/audio capture group", async (t) => {
  const tracks = [new Capture(), new Capture("audio")];
  screen = async () => tracks;
  const f = await fixture(t);
  await f.media.toggle("screen");
  const audio = f.transport.localParticipant.getTrackPublication(
    Source.ScreenShareAudio,
  )!;
  f.transport.localParticipant.trackPublications.delete(
    Source.ScreenShareAudio,
  );
  f.transport.emit(events.LocalTrackUnpublished, audio);
  f.transport.state = states.Reconnecting;
  f.transport.emit(events.ConnectionStateChanged, states.Reconnecting);
  f.transport.state = states.Connected;
  f.transport.emit(events.ConnectionStateChanged, states.Connected);
  f.transport.emit(events.Reconnected);
  assert.ok(
    tracks.every((track) => track.mediaStreamTrack.readyState === "ended"),
  );
  await setImmediate();
  assert.equal(f.transport.localParticipant.trackPublications.size, 0);
});

test("successful camera republication preserves the application-supplied capture", async (t) => {
  const capture = new Capture();
  camera = async () => capture;
  const f = await fixture(t);
  await f.media.toggle("camera");
  const publication = f.transport.localParticipant.getTrackPublication(
    Source.Camera,
  )!;
  f.transport.localParticipant.trackPublications.clear();
  f.transport.emit(events.LocalTrackUnpublished, publication);
  f.transport.state = states.Reconnecting;
  f.transport.emit(events.ConnectionStateChanged, states.Reconnecting);
  await setImmediate();
  f.transport.localParticipant.trackPublications.set(
    Source.Camera,
    publication,
  );
  f.transport.emit(events.LocalTrackPublished, publication);
  f.transport.state = states.Connected;
  f.transport.emit(events.ConnectionStateChanged, states.Connected);
  f.transport.emit(events.Reconnected);
  await setImmediate();
  assert.equal(capture.mediaStreamTrack.readyState, "live");
  await f.media.toggle("camera");
  assert.equal(capture.mediaStreamTrack.readyState, "ended");
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

test("a viewer without HEVC receive support does not subscribe to its video or paired audio", async (t) => {
  const f = await fixture(t);
  const peer = addPeer(f.transport);
  Object.assign(peer.getTrackPublication(Source.ScreenShare)!, {
    mimeType: "video/H265",
  });
  f.transport.emit(events.TrackPublished);
  f.media.watch("friend:screen", true);
  assert.equal(f.snapshot().streams[0]?.compatible, false);
  assert.equal(f.snapshot().streams[0]?.watching, false);
  assert.equal(peer.getTrackPublication(Source.ScreenShare)?.isDesired, false);
  assert.equal(
    peer.getTrackPublication(Source.ScreenShareAudio)?.isDesired,
    false,
  );
});

test("preview announcements accept only bounded descriptors for the current publication without subscribing", async (t) => {
  const f = await fixture(t);
  const peer = addPeer(f.transport);
  const descriptor = {
    trackSid: "TR_screen",
    revision: "r".repeat(22),
    width: 320,
    height: 180,
    updatedAt: Date.now(),
  };
  const announce = (value: string) => {
    peer.attributes["freedomstream.preview.screen"] = value;
    f.transport.emit(events.ParticipantAttributesChanged);
  };
  announce(JSON.stringify(descriptor));
  assert.deepEqual(f.snapshot().streams[0]?.preview, descriptor);
  assert.equal(peer.getTrackPublication(Source.ScreenShare)?.isDesired, false);
  assert.equal(
    peer.getTrackPublication(Source.ScreenShareAudio)?.isDesired,
    false,
  );
  for (const value of [
    "invalid JSON",
    "x".repeat(513),
    JSON.stringify({ ...descriptor, width: 4096 }),
    JSON.stringify({ ...descriptor, trackSid: "TR_old" }),
  ]) {
    announce(value);
    assert.equal(f.snapshot().streams[0]?.preview, null);
  }
});
