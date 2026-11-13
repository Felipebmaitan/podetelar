# Receiver A/V synchronization feasibility

Research date: 2026-11-12. Scope: read-only investigation for PR #40,
`fix/native-audio-playback-latency`; no implementation or runtime qualification.
Read AGENTS.md, CONTEXT.md, the implementation plan and the existing audio-sync
investigation. This report is the only file created by this research task.
Application line references were inspected in the working tree based on
`e215978b4f1385f84cc31d458f133104b6413424`; the main agent is concurrently extending
qualification, so resolve the named functions again before editing.

Subsequent implementation and actual pilot acceptance are recorded separately in
[audio-sync-investigation.md](../verification/audio-sync-investigation.md#approved-native-output-pilot).
This research snapshot does not establish the implemented pilot's runtime results.

## Decision

Parent implementation note: the user's agreed reference limits remain **15 ms
audio leading / 45 ms audio lagging**, with a tighter normal-operation audio-lag
target around 20 ms. The exploratory targets later in this report do not replace
those limits. Feasibility approval is not implementation or physical acceptance.

**Feasible to pilot now, with SDK adapters: use WebRTC's real Windows output ADM
without enabling recording, and bind per-remote-track gain.** Keep LiveKit, the
current encoders/decoders, explicit subscriptions and process-isolated capture.
This removes an external playback tail from outside WebRTC's device-delay
accounting and lets its existing synchronization operate against the output it
actually drives. This is an architecture proposal, not implemented or verified
behavior. No new transport or SFU is justified by the evidence below.

**Not feasible as an application-only configuration switch.** The supplied
`PlatformAudio::new()` enables recording; using it and stopping recording
afterward does not satisfy a never-acquire-microphone contract. The pinned Rust
track APIs also lack per-track volume. Both require deliberate adapter work.
Native playout cannot reconstruct capture times already discarded by the sender
and cannot measure the monitor or speaker's physical presentation delay.

**A complete synchronization claim remains blocked on acceptance, not on a known
fundamental platform limitation.** First measure a matched audiovisual source,
initial offset and sustained drift, and establish that each stream has its own
working sync group. Sender capture-time preservation may be the next required
change. Hardware, production WAN and physical output acceptance remain open.

The existing correction changes the GStreamer output ring from 200 ms to 40 ms.
Its measured approximately 260 ms to 100 ms PCM-submission-to-process-loopback
improvement is valuable, but is neither glass-to-glass latency nor lip sync.
See the measurements and limitations in
[audio-sync-investigation.md](C:/Users/Felipe/Documents/Coding/freedomstream/docs/verification/audio-sync-investigation.md:10).
This research performed no additional media measurements.

## What Discord discloses

Discord's March 5, 2024 engineering article says: “Audio and video are sent over
separate RTP packets and the receiver synchronizes them before playback.” It also
distinguishes Go Live synchronization from voice-chat synchronization. The article
does not disclose its clock selection, jitter-buffer controller, device-delay
feedback, timestamp representation or rendering implementation. It supports the
goal of receiver-coordinated playback, not a claim that Discord uses one of the
specific designs proposed here.
[Discord: How It All Goes Live](https://discord.com/blog/how-it-all-goes-live-an-overview-of-discords-streaming-technology).

RTP audio and video timestamps have independent origins and rates; raw values
cannot be subtracted. RTCP sender reports associate each RTP timeline with NTP,
and source association identifies which streams belong together. That provides a
common media-time relationship without requiring the viewer's wall clock to
equal the publisher's. Mapping it onto a local playback clock and accounting for
output latency is still necessary.
[RFC 3550 §§5.1, 6.4.1 and 6.5.1](https://www.rfc-editor.org/rfc/rfc3550.html).

## What the pinned implementation actually exposes

The application patches LiveKit **0.10.1** and webrtc-sys **0.3.53** in generated
local trees. Cargo resolves **libwebrtc 0.4.0** from the registry. Its native build
helper is webrtc-sys-build **0.3.21**, whose `WEBRTC_TAG` is `webrtc-a545de4`.
The inspected downloaded Windows headers use that tag. WebRTC source links below
refer to that base revision; the bundled SDK also carries its build patches,
including external audio-source handling. These are source/header observations,
not a fresh verification of linked binary provenance.
[native SDK pin](C:/Users/Felipe/Documents/Coding/freedomstream/deploy/native-sdk.json:1),
[native decoder/bridge pin](C:/Users/Felipe/Documents/Coding/freedomstream/deploy/native-decoder.json:1),
[Cargo patches](C:/Users/Felipe/Documents/Coding/freedomstream/Cargo.toml:6),
[libwebrtc lock entry](C:/Users/Felipe/Documents/Coding/freedomstream/Cargo.lock:2157),
[native build tag](C:/Users/Felipe/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/webrtc-sys-build-0.3.21/src/lib.rs:31).

### Audio receive metadata and clocks

- `libwebrtc::AudioFrame` contains PCM, sample rate, channel count and samples per
  channel only. `NativeAudioStream` constructs it from the sink callback without
  RTP, NTP, capture time, sequence or presentation deadline. Its bounded queue
  drops old blocks when full. The application requests five frames, copies those
  samples into appsrc and timestamps them on arrival. It has no media-time or
  hardware-playhead association at this boundary.
  [AudioFrame fields](C:/Users/Felipe/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/libwebrtc-0.4.0/src/audio_frame.rs:18),
  [stream callback and queue](C:/Users/Felipe/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/libwebrtc-0.4.0/src/native/audio_stream.rs:90),
  [application audio pump](C:/Users/Felipe/Documents/Coding/freedomstream/apps/publisher/src/playback.rs:835),
  [arrival-timestamped sink pipeline](C:/Users/Felipe/Documents/Coding/freedomstream/apps/publisher/src/playback.rs:915).
- This is deeper than an omitted Rust field. The SDK's `NativeAudioSink`
  implements the five-argument `OnData`; WebRTC's remote source explicitly passes
  an absent absolute capture timestamp even to the newer overload. Merely
  forwarding that overload would still produce no remote capture time.
  [SDK callback](C:/Users/Felipe/Documents/Coding/freedomstream/.local/native-sdk/webrtc-sys/src/audio_track.cpp:93),
  [remote source at a545de4, lines 163–172](https://github.com/webrtc-sdk/webrtc/blob/a545de4/pc/remote_audio_source.cc#L163).
- A deeper receive hook can recover the first sample's RTP timestamp: the
  internal raw `AudioSinkInterface::Data` already carries it. Reaching Rust
  requires preserving it through `RemoteAudioSource` and the SDK bridge, or
  providing another receiver hook. Sender-report mapping, concealment,
  resampling and discontinuities still need explicit semantics; the existing
  callback does not provide all of these.
  [internal raw sink fields](https://github.com/webrtc-sdk/webrtc/blob/a545de4/api/call/audio_sink.h#L22).

### Synthetic ADM versus actual Windows output

The factory installs an `AdmProxy`. Its default mode constructs only the
synthetic device, which requests 10 ms chunks on a timer; it does not play those
chunks through Windows. `SyntheticAudioDevice::PlayoutDelay` returns success
without writing the output parameter. It therefore provides no estimate of the
external GStreamer/WASAPI tail; do not describe this as measured zero delay.
[factory wiring](C:/Users/Felipe/Documents/Coding/freedomstream/.local/native-sdk/webrtc-sys/src/peer_connection_factory.cpp:179),
[lazy platform device](C:/Users/Felipe/Documents/Coding/freedomstream/.local/native-sdk/webrtc-sys/src/adm_proxy.cpp:33),
[synthetic timer](C:/Users/Felipe/Documents/Coding/freedomstream/.local/native-sdk/webrtc-sys/src/synthetic_audio_device.cpp:45),
[synthetic delay method](C:/Users/Felipe/Documents/Coding/freedomstream/.local/native-sdk/webrtc-sys/src/synthetic_audio_device.cpp:291).

The proxy already has independent recording and playout flags, both initially
false. With a platform reference and playout enabled it stops the synthetic
pump, starts platform playout, and delegates `PlayoutDelay` to that device.
Recording has a separate guard. This makes output-only operation a credible
adapter change rather than a new ADM implementation.
[default flags](C:/Users/Felipe/Documents/Coding/freedomstream/.local/native-sdk/webrtc-sys/include/livekit/adm_proxy.h:295),
[mode switching](C:/Users/Felipe/Documents/Coding/freedomstream/.local/native-sdk/webrtc-sys/src/adm_proxy.cpp:243),
[guarded recording](C:/Users/Felipe/Documents/Coding/freedomstream/.local/native-sdk/webrtc-sys/src/adm_proxy.cpp:504),
[delay delegation](C:/Users/Felipe/Documents/Coding/freedomstream/.local/native-sdk/webrtc-sys/src/adm_proxy.cpp:819).

The real Windows ADM calculates pending output time from samples written and
`IAudioClock` position/frequency. Its `PlayoutDelay` returns that estimate. WebRTC
receive code incorporates ADM delay when estimating audio playout and combines
it with NetEQ delay for synchronization. This is precisely the feedback missing
from the current external sink. It is still an endpoint estimate, not a
measurement of acoustic arrival or display scanout.
[Windows ADM, lines 2598–2601 and 2860–2867](https://github.com/webrtc-sdk/webrtc/blob/a545de4/modules/audio_device/win/audio_device_core_win.cc#L2598),
[receive delay and playout timestamp, lines 1057–1183](https://github.com/webrtc-sdk/webrtc/blob/a545de4/audio/channel_receive.cc#L1057),
[Microsoft IAudioClock::GetPosition](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudioclock-getposition).

However, `PlatformAudio::new()` acquires the platform ADM and explicitly enables
recording and playout. It then applies audio-processing defaults whose AEC, noise
suppression and AGC are true. It is unsuitable unchanged. These are enablement
paths; this inspection did not open a microphone to test them. An output-only
constructor must never enable recording, expose a device-source publishing path,
or apply speech-processing defaults to game/music capture.
[constructor, lines 443–504](C:/Users/Felipe/Documents/Coding/freedomstream/.local/native-sdk/livekit/src/platform_audio/mod.rs:443),
[processing defaults](C:/Users/Felipe/Documents/Coding/freedomstream/.local/native-sdk/livekit/src/platform_audio/processing.rs:115),
[internal recording/playout controls](C:/Users/Felipe/Documents/Coding/freedomstream/.local/native-sdk/livekit/src/rtc_engine/lk_runtime.rs:256).

Do not assume that the recording flag guards every input-related method: the
proxy currently replays cached input-device/stereo selections during acquisition,
and its `InitMicrophone` delegates without that flag. The output-only adapter
should suppress input selection/replay and microphone initialization as well as
recording, with observable enforcement. Windows ADM `Init()` enumerates endpoint
metadata for both directions; that enumeration is distinct from opening an
input audio client or capturing samples. If the contract were expanded to ban
even enumeration, the stock Windows ADM would need deeper changes.
[cached input replay](C:/Users/Felipe/Documents/Coding/freedomstream/.local/native-sdk/webrtc-sys/src/adm_proxy.cpp:152),
[microphone initialization delegate](C:/Users/Felipe/Documents/Coding/freedomstream/.local/native-sdk/webrtc-sys/src/adm_proxy.cpp:619),
[Windows initialization, lines 583–598](https://github.com/webrtc-sdk/webrtc/blob/a545de4/modules/audio_device/win/audio_device_core_win.cc#L583).

### Per-track volume, mute and actual subscriptions

The LiveKit remote audio track exposes enable/disable and `rtc_track()`, but no
gain. The underlying Rust `RtcAudioTrack` likewise has no volume method; its
public `sys_handle()` permits an intentional lower-level bridge. The app's
current gain belongs to a separate GStreamer `volume` element for every watched
audio track; the UI uses 0–100%, and mute remembers/restores that stream's volume.
[remote track](C:/Users/Felipe/Documents/Coding/freedomstream/.local/native-sdk/livekit/src/room/track/remote_audio_track.rs:70),
[Rust native track](C:/Users/Felipe/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/libwebrtc-0.4.0/src/native/audio_track.rs:22),
[current gain application](C:/Users/Felipe/Documents/Coding/freedomstream/apps/publisher/src/playback.rs:898),
[UI mute/volume](C:/Users/Felipe/Documents/Coding/freedomstream/apps/publisher/src/ui.rs:1295).

WebRTC already has a receive gain path: remote source volume notifies its audio
receiver, which updates the SSRC-specific output volume on the worker thread.
Bind that path, marshaling onto the correct signaling/worker sequences; do not
change the Windows endpoint or master speaker volume. Disabled tracks and
cached gains interact, so gain-zero mute with unchanged subscriptions is the
simplest semantic match to the existing UI. The raw track callback is upstream
of output scaling: its samples cannot prove audible mute or applied gain.
[source gain notification](https://github.com/webrtc-sdk/webrtc/blob/a545de4/pc/remote_audio_source.cc#L117),
[receiver gain and enabled state](https://github.com/webrtc-sdk/webrtc/blob/a545de4/pc/audio_rtp_receiver.cc#L127),
[raw callback before scaling](https://github.com/webrtc-sdk/webrtc/blob/a545de4/audio/channel_receive.cc#L453).

Native playout will render received audio without needing an application PCM
pump. Consequently callback removal is insufficient to stop sound or traffic.
Preserve `auto_subscribe=false`, and apply Watch/Stop to both Screenshare and
ScreenshareAudio publications. Establish the remembered gain before audible
playout of a newly subscribed/replaced track; reject a pilot that briefly plays a
muted stream at unity on reconnect.
[join options](C:/Users/Felipe/Documents/Coding/freedomstream/apps/publisher/src/connection.rs:13),
[subscription helper](C:/Users/Felipe/Documents/Coding/freedomstream/apps/publisher/src/playback.rs:122),
[paired subscription selection](C:/Users/Felipe/Documents/Coding/freedomstream/apps/publisher/src/playback.rs:580).

### Video media time, render deadlines and grouping

The C++/webrtc-sys video frame bridge exposes `timestamp_us`, RTP timestamp and
`ntp_time_ms`. `libwebrtc`'s higher-level native stream retains `timestamp_us` but
drops ordinary RTP and NTP values; optional packet-trailer metadata is a separate
feature and cannot be presumed negotiated or equivalent to an audio clock.
[bridge fields](C:/Users/Felipe/Documents/Coding/freedomstream/.local/native-sdk/webrtc-sys/src/video_frame.rs:39),
[higher-level callback](C:/Users/Felipe/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/libwebrtc-0.4.0/src/native/video_stream.rs:128).

Importantly, in this receive path WebRTC restores NTP from its frame information
and sets decoded `timestamp_us` to the planned **local render time**. It is not
the publisher's original UNIX capture time. The custom decoder's synthetic
GStreamer PTS is only a lookup key used to restore RTP correlation; WebRTC's
generic callback then restores timing metadata. Changing those synthetic PTS to
look like wall time risks breaking correlation rather than repairing sync.
[generic decoder metadata/deadline restoration, lines 147 and 239–247](https://github.com/webrtc-sdk/webrtc/blob/a545de4/modules/video_coding/generic_decoder.cc#L147),
[custom decoder correlation](C:/Users/Felipe/Documents/Coding/freedomstream/apps/publisher/decoder/freedomstream_decoder.h:66).

WebRTC's default prerender smoothing can schedule decoded frames before the
application callback; native ADM is not a design that necessarily requires a
second video delay queue. Confirm the effective configuration and callback
deadline timing in the pilot. The app subsequently converts and replaces a
latest-picture mailbox without using frame time to schedule egui submission;
that additional conversion/UI/display tail remains outside physical output
feedback. If smoothing is disabled, preserving and honoring the render deadline
becomes required. Avoid two controllers delaying the same frame.
[smoothing default](https://github.com/webrtc-sdk/webrtc/blob/a545de4/media/base/media_config.h#L47),
[receive renderer selection](https://github.com/webrtc-sdk/webrtc/blob/a545de4/video/video_receive_stream2.cc#L380),
[scheduled renderer queue](https://github.com/webrtc-sdk/webrtc/blob/a545de4/video/render/incoming_video_stream.cc#L47),
[current video pump](C:/Users/Felipe/Documents/Coding/freedomstream/apps/publisher/src/playback.rs:632).

WebRTC's synchronizer maps RTP through sender reports, compares current delays
and adjusts audio/video minimum playout delays. It cannot correct an external
audio tail it does not know about. The pinned LiveKit SFU fills an empty stream
from track source and, when supported/enabled, packs publisher identity plus
stream into the subscriber sync stream ID. An empty Rust `stream` option alone
is not evidence of missing grouping. Verify the effective subscriber association
before changing server configuration or inventing a global stream name.
[WebRTC synchronization](https://github.com/webrtc-sdk/webrtc/blob/a545de4/video/rtp_streams_synchronizer2.cc#L100),
[LiveKit source default, lines 3171–3173](https://github.com/livekit/livekit/blob/v1.13.11/pkg/rtc/participant.go#L3171),
[LiveKit subscriber stream ID, lines 124–126](https://github.com/livekit/livekit/blob/v1.13.11/pkg/rtc/subscribedtrack.go#L124).

Multiple watched streams can share one physical audio mixer/device while each
publisher's audio/video pair has a distinct sync group. The pinned WebRTC code
only synchronizes the first video with an audio stream **within one group**;
placing every publisher in the same group would therefore be wrong. Its current
mixer iterates all registered sources rather than selecting only three talkers.
Mix headroom, clipping/limiting and stereo fidelity still need measurements with
several simultaneous game/music streams; source support does not establish
acceptable CPU/game performance.
[one pair per group](https://github.com/webrtc-sdk/webrtc/blob/a545de4/call/call.cc#L1340),
[mixer source iteration](https://github.com/webrtc-sdk/webrtc/blob/a545de4/modules/audio_mixer/audio_mixer_impl.cc#L132).

## Architecture comparison

| Option | Required work | Feasibility and principal limit |
| --- | --- | --- |
| Output-only native WebRTC ADM plus per-track gain | Safe lifetime/policy API, gain bridge, subscription/generation integration and output instrumentation. Retain current capture and video path initially. | Smallest pilot. Uses existing NetEQ, grouping and device-delay feedback. Sender timing and UI/display tails remain. |
| Shared GStreamer audio clock and presentation pipeline | Recover audio/video media time, map each publisher to receiver running time, shared clock/base-time/segments, bounded queues, mixing and egui presentation integration. | Possible, larger change. A common clock alone cannot reconstruct discarded timestamps or synchronize latest-image rendering. Current decoder pipelines are independent codec adapters, not a shared A/V player. |
| Custom audio-master scheduler | Timestamp-preserving receive hooks and sender capture mapping; device sample-position/QPC correlation; per-stream media-to-device mapping, drift control/resampling, deadlines, late-frame/drop/reset policy. | Possible in principle, most engineering and validation. Current `AudioFrame` cannot support it. Reimplements functions WebRTC/GStreamer already provide. |

For the GStreamer option, explicitly timestamp buffers in the selected
running-time domain and use synchronized sinks/presentation. A video appsink
callback followed by an unscheduled egui texture is still outside that schedule.
One receiver pipeline can mix per-watch audio branches, while each publisher
keeps its own media-time mapping; unrelated publishers' NTP epochs must not be
treated as a demand to play all publishers at one shared capture instant.
Handle clock loss, dynamic pad teardown and latency recalculation. The actual
selected/provided clock must be observed rather than assumed from plugin names.
[GStreamer clocks](https://gstreamer.freedesktop.org/documentation/application-development/advanced/clocks.html),
[synchronization design](https://gstreamer.freedesktop.org/documentation/additional/design/synchronisation.html),
[appsrc integration](https://gstreamer.freedesktop.org/documentation/app/appsrc.html).

For a custom scheduler, use a hardware-backed played-sample position correlated
with receiver monotonic time; a count of samples submitted is not a played-sample
clock. Track each stream before mixing, including silence/concealment, and retain
bounded future frames rather than only the newest picture. Otherwise an early
newer picture can replace the frame that is due now. Reset mappings on device,
SSRC, source, codec and reconnect generations. Missing timing requires an
explicit degraded mode, not a fabricated timestamp or guessed fixed video delay.
[Microsoft clock position and frequency semantics](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudioclock-getposition),
[existing generation mailbox protection](C:/Users/Felipe/Documents/Coding/freedomstream/apps/publisher/src/playback.rs:182).

## Smallest useful pilot and reproducible setup changes

These are proposed changes for a separately approved implementation, not work
performed by this research task.

1. Add an output-only platform-audio handle to the pinned LiveKit adapter. Keep
   recording disabled before acquisition and throughout the handle's life;
   prevent a second constructor/shared handle from escalating it to recording.
   Do not obtain `RtcAudioSource::Device`, enumerate/select input devices, or
   apply `AudioProcessingOptions::default()`. Continue publishing only the
   existing external NativeAudioSource. Instrument native recording entry points
   to verify that no input is initialized/started during watch, publish, device
   changes or reconnect. Preserve the external-source build patch that prevents
   device capture from being mixed into injected audio.
   [external-source patch](C:/Users/Felipe/Documents/Coding/freedomstream/.local/native-sdk/webrtc-sys/libwebrtc/patches/external_audio_source.patch:1).
2. Add a typed webrtc-sys gain bridge to the remote source's volume path, with
   thread-safe lifetime and finite 0–1 gain validation matching the current UI.
   Prefer a small libwebrtc method and LiveKit forwarding method for a durable
   API. A deliberate direct webrtc-sys dependency using the existing
   `RtcAudioTrack::sys_handle()` could avoid a libwebrtc fork for this limited
   pilot; it must be explicit and pinned. No such Rust volume API exists today.
3. Enable exactly one platform output owner for the receiver/runtime. Remove or
   bypass GStreamer audio output for pilot tracks to prevent double playback.
   Diagnostic NativeAudioStream taps may remain bounded, but are observations
   before final gain/device rendering, not a second output path. Apply each
   stream's gain at initial subscription and replacement before audible output.
4. Keep normal WebRTC sync/grouping and smoothing initially. Log only numeric
   timing/validity aggregates and local generation identifiers: selected output
   mode, ADM delay availability, callback/deadline deltas, group pair counts,
   jitter/decode/underrun/discontinuity data. Do not log SDP, invitations, tokens,
   credentials or media payloads. Expose RTP/NTP/render metadata through a typed
   libwebrtc adapter if needed for diagnosis; do not interpret absent estimates
   as zero. Compare measured residuals before adding another scheduler.
5. Preserve playback ownership explicitly. Last Stop/leave/shutdown must release
   tracks, taps, pending callbacks and the output handle after unsubscribe; Stop
   for one stream must not stop the shared output for other watches. On
   disconnect suspend audible state, then reattach saved per-watch gains to new
   track generations. Observe actual completion rather than treating a task
   abort request as proof that native teardown finished.
   [current abort-on-drop](C:/Users/Felipe/Documents/Coding/freedomstream/apps/publisher/src/playback.rs:200),
   [current pipeline NULL teardown](C:/Users/Felipe/Documents/Coding/freedomstream/apps/publisher/src/playback.rs:830),
   [platform release behavior](C:/Users/Felipe/Documents/Coding/freedomstream/.local/native-sdk/webrtc-sys/src/adm_proxy.cpp:203).

   The current proxy retains the platform module object until factory destruction
   even after its last platform reference is released. Verify stopped device I/O
   separately from destruction of that intentionally retained module.

Implement adapters in version-controlled setup inputs. Extend
[setup-native-sdk.mjs](C:/Users/Felipe/Documents/Coding/freedomstream/scripts/setup-native-sdk.mjs:26)
and
[setup-native-decoder.mjs](C:/Users/Felipe/Documents/Coding/freedomstream/scripts/setup-native-decoder.mjs:104),
and update both pin manifests' affected source/adapter/tree hashes. If changing
libwebrtc's Rust API, add a separately hashed generated libwebrtc source adapter,
Cargo patch/workspace exclusion and lockfile changes. Never edit the Cargo
registry or leave hand changes only in `.local/native-sdk`.

The minimal output/gain wrapper pilot should be able to reuse the current native
WebRTC archive: it uses already-present ADM and remote-volume APIs. Confirm
symbols/ABI on a clean build. Preserving deeper raw audio metadata or changing
WebRTC's internals requires a newly built, checksummed native archive with matching
headers, source revision and patch set; updating only Rust wrappers would not
apply those internal changes. Re-run clean setup, native checks, real media,
license/provenance qualification and an independently reviewed separate PR before
shipping any pilot. No SDK upgrade is intrinsically necessary for the initial
experiment; a broad upgrade would complicate attribution.

## Capture-time risks that native output alone cannot fix

Video currently uses a UNIX epoch plus GStreamer PTS and feeds that into the SDK
TimestampAligner. The aligner translates against WebRTC handoff time. Audio has
its own capture/mixer pipeline; its PTS is used for capture-age diagnostics, then
discarded when constructing AudioFrame. A 50 ms NativeAudioSource argument sets
pacing/backpressure policy, not a measured fixed delay. Its timer queues PCM and
calls the timestamp-free sink overload in 10 ms chunks, inserting silence on
underrun. Those mechanics can change the relationship between source events and
the audio media timeline.
[video epoch](C:/Users/Felipe/Documents/Coding/freedomstream/apps/publisher/src/encoding.rs:274),
[video PTS handoff](C:/Users/Felipe/Documents/Coding/freedomstream/apps/publisher/src/encoding.rs:468),
[SDK timestamp alignment](C:/Users/Felipe/Documents/Coding/freedomstream/.local/native-sdk/webrtc-sys/src/video_track.cpp:172),
[capture-age versus timestamp-free audio](C:/Users/Felipe/Documents/Coding/freedomstream/apps/publisher/src/audio/mod.rs:256),
[source queue and silence](C:/Users/Felipe/Documents/Coding/freedomstream/.local/native-sdk/webrtc-sys/src/audio_track.cpp:149).

If the pilot leaves a repeatable source offset or drifting relation, the next
bounded investigation is a common sender monotonic capture mapping: map video
capture PTS and audio first-sample PTS into the same domain, preserve it through
queues/resampling, and propagate it into WebRTC's supported capture-time path.
The native API has a timestamp-bearing audio overload, but the present source
does not use it. Timestamped audio needs per-block metadata, sample-offset
adjustment during 10 ms slicing, explicit dropped-block/inserted-silence semantics
and correct conversions; adding one timestamp to a whole mutable PCM vector is
insufficient. Confirm how the encoder maps capture time to RTP/RTCP on the wire.
[timestamp-bearing native audio contract](https://github.com/webrtc-sdk/webrtc/blob/a545de4/api/media_stream_interface.h#L213).

Separate capture pipelines can have different base times, startup latencies and
device-rate drift. Source/mixer inventory rebuilds, static-image retention,
application audio generation ahead of display, codec parser/decode delays and
device changes can introduce discontinuities. Preserve actual source age rather
than timestamping at encode completion. Do not subtract unsynchronized sender
and receiver wall clocks or assume packet-trailer video timestamps fix audio.
This sender work is a follow-up proposal, not established as the cause of the
reported AV1/H.264 difference.
[capture mixer and per-process branches](C:/Users/Felipe/Documents/Coding/freedomstream/apps/publisher/src/audio/mod.rs:98),
[video cadence pipeline](C:/Users/Felipe/Documents/Coding/freedomstream/apps/publisher/src/encoding.rs:148).

## Contracts and distribution

Keep selected application process-tree loopback and the existing fail-closed
Discord/self-playback exclusions. Native output must stay in the excluded app
process tree; a future helper would need an explicitly protected identity. Do not
replace this policy with endpoint-wide loopback or microphone AEC. Watch/Stop
continues to change real subscriptions; gain-zero mute does not stop network
traffic. Preview capture remains once at start and at most once per minute.
[protected process policy](C:/Users/Felipe/Documents/Coding/freedomstream/apps/publisher/src/audio/policy.rs:46),
[process include-loopback pipeline](C:/Users/Felipe/Documents/Coding/freedomstream/apps/publisher/src/audio/mod.rs:127),
[subscription contract](C:/Users/Felipe/Documents/Coding/freedomstream/CONTEXT.md:27).

The application can retain MIT licensing while its dependencies retain theirs.
LiveKit Rust bridge sources are Apache-2.0; WebRTC's base is BSD-style with
additional third-party terms; GStreamer uses LGPL with plugin-specific licensing
and separate codec/distribution considerations. Preserve upstream license,
NOTICE and modified-source notices as applicable. Use independently written
adapters and these permitted APIs; do not copy GPL libobs/VLC/mpv synchronization
code. A new bridge/archive/plugin set must pass the existing distribution
qualification rather than being described as wholly MIT licensed.
[existing engine license decision](C:/Users/Felipe/Documents/Coding/freedomstream/docs/adr/0001-native-media-engine.md:3),
[Apache-2.0 redistribution terms](https://www.apache.org/licenses/LICENSE-2.0),
[WebRTC license](https://github.com/webrtc-sdk/webrtc/blob/a545de4/LICENSE),
[GStreamer licensing advisory](https://gstreamer.freedesktop.org/documentation/application-development/appendix/licensing.html),
[portable plugin qualification](C:/Users/Felipe/Documents/Coding/freedomstream/scripts/portable-runtime.mjs:263).

## Acceptance that would answer the actual question

The current process-loopback test isolates audio output; codec stage counters
prove media receipt/decoding, not audiovisual event correspondence. Keep that
distinction even if a new synthetic marker test runs successfully.
[existing timing acceptance](C:/Users/Felipe/Documents/Coding/freedomstream/native-e2e/audio-timing.spec.ts:6).

1. **Matched full source fixture.** Generate identifiable flash/click event pairs
   from one source timeline, with nonrepeating event IDs and stereo identifiers.
   Capture the real source window and its process audio through production
   capture, encode, local API/SFU, native decode and production output. Use the
   same source, sender, resolution/FPS/bitrate and output for H.264, AV1 hardware,
   AV1 software and HEVC where supported. Compare current bounded GStreamer output
   against the native ADM pilot; retain source and local-playback baselines.
2. **Observe output, not only callbacks.** A receiver compositor/screen-capture
   observation plus process-loopback audio on a shared receiver monotonic/QPC
   timeline can establish digital A/V event offset. A callback marker observed
   before texture upload is only submission timing. Physical acceptance needs
   external screen/audio instrumentation, such as a high-frame-rate camera with
   recorded speaker output or a photodiode/audio measurement sharing one clock.
   External test equipment does not authorize the application to acquire a
   microphone. Record baseline instrument/display/audio latency and uncertainty.
3. **Report startup and drift separately.** Define signed offset as audio event
   time minus video event time: positive means audio late. Report valid event
   counts, missing/duplicate events, median, p95 absolute skew, worst sustained
   excursion, convergence time and ms/min drift over at least 30 minutes. Separate
   startup/recovery from steady state. Proposed pilot targets, to agree before
   implementation: steady-state median absolute skew at most 30 ms, p95 at most
   50 ms, no persistent excursion beyond 100 ms, no monotonic accumulating drift,
   and no improvement obtained by allowing unbounded overall latency. These are
   project acceptance proposals, not Discord guarantees or normative limits.
4. **Exercise independent watches.** Two then four/five publishers with different
   event sequences; mute/0/25/50/100% gain, unmute restoration, focus/grid/clipped
   tiles, Stop/rewatch, leaving, source/audio policy changes, codec fallback and
   reconnect. Measure each pair's skew and aggregate output spectra. Stop must
   stop its incoming audio/video bytes after bounded in-flight teardown, remove
   its sound and callbacks, and leave other watched streams intact. A muted
   stream must stay silent even during track replacement.
5. **Verify privacy, exclusion and lifecycle.** Instrument input-open/start
   attempts throughout the pilot, including while publishing external audio and
   switching/default-removing output devices. Require no microphone acquisition;
   verify selected application stereo survives and Discord/self-playback does
   not leak or feed back while watching others. Check all stop/leave/close paths,
   task completion, platform references, device handles and stale-generation
   callbacks; do not infer native cleanup from a Rust task count alone.
6. **Hardware and WAN gates.** Test NVIDIA and AMD independently, CPU/game/GPU
   contention and several simultaneous software decoders; wired/USB/HDMI and
   other supported real output devices, default-device changes and stalls.
   Bluetooth delay requires separate characterization. Repeat on the production
   route with controlled loss, jitter, RTT, bandwidth restriction and forced
   TURN/TCP/TLS fallback. Measure recovery and resource use without adding delay
   indefinitely. Local source reading/builds/mocks cannot close these gates;
   neither native HEVC nor 4K120 qualification is implied by this proposal.

Proceeding with the small native-output pilot is technically justified. Shipping
it as a general A/V fix, or selecting a larger shared-clock architecture, should
follow these measurements and the user's implementation choice. The research
does not implement, deploy, merge or waive any acceptance gate.
