# Audio-after-video investigation — 2026-11-12

The user reports sound arriving **after** its matching picture, apparently worse
on their AV1 publication than a friend's H.264 publication. Accumulating drift
versus an initial constant offset is still unknown. These were different senders,
so that observation alone does not isolate a codec.

## Confirmed defect and bounded correction

Native playback requested `wasapi2sink low-latency=true sync=false`, retaining
GstAudioBaseSink's **200,000 microsecond buffer-time default**. Low-latency mode
optimizes the WASAPI device period; it does not remove this separate ring buffer.
The current appsrc/backlog queue metric excludes it. Consequently a small
`playback_queue_ms` was insufficient evidence of low output latency.

A real process-loopback test isolates this stage: generate eleven quiet 20 ms
1 kHz impulses in live stereo PCM, submit 480-sample frames at 48 kHz every 10 ms
through the **production playback pipeline**, and detect the same process's
audio through Windows process-tree loopback. Read only numeric timing aggregates.
No microphone, another application's audio, media recording, server, SDK or video
codec is involved. The qualifier fails if it cannot identify all eleven events.

On this Windows machine, with its existing default output device:

| Production pipeline configuration | Eleven observed submission-to-loopback delays |
| --------------------------------- | --------------------------------------------- |
| Legacy 200 ms sink buffer         | 259.72–260.41 ms                              |
| Corrected 40 ms sink buffer       | 99.33–100.35 ms                               |

All eleven impulses were detected in both runs; the largest observed appsrc
backlog was one 10 ms block (1,920 bytes). Loopback capture itself reported a
20 ms period and about 19–21 ms buffer timestamp age. These measurements include
capture/polling overhead and are **not** physical speaker timing. The approximate
160 ms A/B improvement is an output-path result, not a measurement of total
stream latency or final A/V offset.

An initial standalone C++ reproduction also found about 250–256 ms with the
legacy pipeline, 72–73 ms with a 20 ms sink buffer, and about 90–132 ms with a
40 ms buffer. The smaller candidate was not selected: 40 ms retains a modest
scheduling cushion. Turning `sync=true` on without a shared presentation clock
did not fix this: that trial measured about 254–294 ms. The correction bounds
unnecessary buffering; it does not insert an arbitrary delay into video.

Reproduce from a built native runtime with `--qualify-playback legacy` and
`--qualify-playback`. Each run lasts about 12.4 seconds and emits eleven quiet
clicks through the selected default output. Use a hidden process, wait for its
completion and capture its final JSON; PowerShell may return before a GUI
executable exits when launched directly. The command does not join a room,
change Windows settings, capture other applications, or retain raw audio.

## Full-path source audit

The numeric loopback samples are preserved in
[audio-sync-measurements.json](audio-sync-measurements.json).

| Stage                | Implementation and timing consequences                                                                                                                                                                                                                       | Evidence status                                                                                                                                                                                     |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Video capture        | DXGI/WGC -> two-frame leaky raw entry queue; WGC imagefreeze retains a static image; videorate/conversion/clocksync feed the encoder.                                                                                                                        | Source inspected. WGC retained frames and physical source presentation need event-based measurement.                                                                                                |
| GPU encoding         | NVENC p4/ultra-low-latency/CBR, B frames disabled, zerolatency enabled. AMF uses AV1 low-latency or H.264/HEVC ultra-low-latency. Encoded appsink is bounded to two buffers. AV1 TU parsing differs from H.264 AU parsing.                                   | No configured NVENC B-frame/lookahead explanation for hundreds of milliseconds of audio delay. Actual AMF performance is unqualified here.                                                          |
| Video handoff        | GStreamer PTS plus an epoch enters pre-encoded NativeVideoSource. The pinned SDK's TimestampAligner maps that source timeline to WebRTC time; pass-through preserves RTP/capture timing.                                                                     | Source inspected. The alignment reference is the handoff stage; it does not independently measure source capture age or audio alignment.                                                            |
| Audio capture        | Selected process-tree WASAPI loopbacks, resampling, force-live audiomixer with 30 ms latency and 10 ms output buffers, bounded dropping appsink. Inventory refresh can rebuild capture.                                                                      | Fixed mixing latency exists independently of codec. Capture age is observed; capture timestamps are not carried into the SDK AudioFrame.                                                            |
| Audio handoff/Opus   | NativeAudioSource uses a 50 ms pacing queue; its implementation accepts up to two queue lengths, drains 10 ms per timer callback and backpressures above its threshold. Opus target is 192 kbps, DTX disabled.                                               | Additional queueing is possible. The 50 ms argument is not proof of exactly 50 ms steady latency. No speculative change made.                                                                       |
| Network/SFU          | One video publication plus one Opus publication, selective Watch/Stop, WebRTC congestion/pacing/retransmission and independent receive jitter buffers. The API/nginx signaling path is separate from RTP media.                                              | Local SFU acceptance exercises actual traffic. Current WAN loss/RTT, relay selection and VPS load cannot be inferred from local tests or historical logs.                                           |
| A/V grouping         | The Rust publish option is empty, but pinned LiveKit 1.13.11 fills Stream from TrackSource and supplies sync stream IDs to supported subscribers.                                                                                                            | Empty SDK option alone is **not** evidence of missing screen/audio grouping. Do not apply an untested stream-name patch as a claimed fix.                                                           |
| Video receive/decode | WebRTC receive timing -> native D3D11 decoder, or dav1d software AV1. AV1 temporal delimiters are restored before av1parse. Decoder correlates synthetic GStreamer PTS back to original RTP timestamp; synthetic PTS is a lookup key, not a new media clock. | GPU/software decoder behavior differs and needs matched measurement. Arbitrarily changing these PTS would risk frame correlation.                                                                   |
| Video presentation   | One-frame SDK queue -> conversion -> latest-picture mailbox -> egui texture upload and repaint. Frame timestamps are not used by an application-level audio-master scheduler.                                                                                | Mailbox replaces old pictures instead of building a FIFO. Texture upload and decoded FPS are not display timing.                                                                                    |
| Audio receive/output | SDK NetEQ/audio callbacks -> five-frame native queue -> appsrc/backlog -> conversion/resampling -> WASAPI ring/device. The app timestamps PCM on arrival; original capture/RTP timing is absent from AudioFrame.                                             | Output ring defect reproduced. Native synthetic ADM playout does not account for this external Windows sink tail in its synchronization. No full shared physical presentation clock is implemented. |
| Diagnostics          | Sender capture/encoder/send counters and receiver video jitter/decode counters are available on demand. Fixture reports now include allowlisted numeric audio/video receive counters.                                                                        | No cross-machine clock subtraction; absent/default SDK counters are not evidence of zero impairment. Full physical latency remains unavailable.                                                     |

Relevant sources: `apps/publisher/src/{encoding.rs,audio/mod.rs,publisher.rs,
playback.rs,playback_qualification.rs}`, `apps/publisher/decoder/
freedomstream_decoder.h`, and the verified generated SDK's `video_track.cpp`,
`audio_track.cpp`, `synthetic_audio_device.cpp` and pass-through encoder.
The SDK is pinned in `deploy/native-sdk.json` and `deploy/native-decoder.json`.

Primary reference checks:

- [GStreamer audio-base-sink ring buffer and defaults](https://gstreamer.freedesktop.org/documentation/audio/gstaudiobasesink.html).
- [GStreamer WASAPI2 sink low-latency option](https://gstreamer.freedesktop.org/documentation/wasapi2/wasapi2sink.html).
- [GStreamer clock synchronization](https://gstreamer.freedesktop.org/documentation/application-development/advanced/clocks.html).
- [Pinned SFU track-source stream default](https://github.com/livekit/livekit/blob/v1.13.11/pkg/rtc/participant.go)
  and [subscriber sync stream ID selection](https://github.com/livekit/livekit/blob/v1.13.11/pkg/rtc/subscribedtrack.go).

## Verification and remaining acceptance

In the first matched native 1080p60/6 Mbps run, both receivers used
`FreedomStream Direct3D11`. The final watched-generation cumulative SDK counters
gave mean video decode intervals of 16.39 ms for H.264 and 0.46 ms for AV1;
mean video jitter-buffer residence was 12.84/12.66 ms, and mean audio jitter-buffer
residence was 31.55/31.53 ms respectively. Sender sampling reported about
59.8/59.9 FPS over UDP with roughly 1 ms local RTT. This is one short matched
local fixture, not a general codec benchmark. SDK decode intervals include the
asynchronous decoder/parser callback path, not just GPU execution.

A second matched run reproduced 16.44 ms H.264 versus 0.45 ms AV1 decode
intervals, with approximately 31.52/31.54 ms audio jitter-buffer residence.
The repeat is consistent with a roughly one-frame H.264 pipeline interval at
60 FPS; it does not identify the responsible parser/decoder stage by itself.

Faster AV1 picture delivery could expose more of the same delayed audio tail;
this is an **inference**, not proof of the user's differing AV1/H.264 offsets.
No AV1-specific audio buffering is configured. Both streams use the same Opus
and Windows output path. A codec-only diagnosis or a switch away from LiveKit
is not supported by these observations.

The live installed GUI and its room were not disconnected or replaced. Both
available Windows app-control interfaces failed with `failed to connect native
pipe ... The system cannot find the file specified`; the browser host was also
unavailable. Existing VPS diagnostic files are historical, and the old SSH key
they reference is absent. No current production-room or VPS telemetry is claimed.
The running installed app's `gstaudio-1.0-0.dll` has the same SHA-256 as the
development runtime used here (`96f0fe6ed46aeb9da0adf25cc4fe9736ad358910568e0a05f31de6121bdeb59d`).
This checks the relevant audio runtime bytes, not live process state or all
installed application provenance.

Run native acceptance with `audio-timing.spec.ts`: process-loopback plus matched
1080p60/6 Mbps H.264 and AV1 fixtures carrying process-isolated stereo tone audio
through the real local API/SFU. Stage counters establish delivery/decoding and
audio callback delivery, **not** matching flash/click synchronization or sustained
physical playback continuity. Full media
results, native/Node checks and review are recorded after completion below.

Still required before saying the user's full sync problem is resolved: compare
the synchronized flash/click source against a local playback baseline on the
actual watcher/output, measure initial and sustained offsets, match resolution,
FPS, bitrate, source and sender hardware across codecs, and repeat over the
production route. Exercise reconnect/source changes, multiple watched streams,
AV1 GPU versus CPU, AMD hardware, packet loss and scheduling stalls. The bounded
output-ring correction can remove a demonstrated contributor while residual
capture/audio-device/network timing or long-term drift remains possible.
Different video delays can also make audio lead after unnecessary output
buffering is removed. A smaller ring is not a complete lip-sync controller.

## Completed local checks and source review

- Node 24.11.1 `npm run check`: formatting, TypeScript, API/tool tests and builds
  pass. Native Clippy passes. Native workspace tests pass (76 publisher tests,
  five existing ignored; portable/bootstrap results also pass). Release builds
  complete with the existing pinned-SDK warnings.
- The initial full 13-case real-media run passed 12 cases: matched H.264/AV1
  stage counters, output loopback, AV1 hardware/software decode, HEVC/H.264,
  previews, Stop/rewatch, compatibility fallback, reconnect and aggregate caps.
  Its desktop-audio exclusion case failed spectral limits while unrelated local
  audio applications were included in that fixture's desktop capture. The case
  now uses the existing `backgroundAudioExclusions` fixture helper to isolate
  the test tones. Production capture policy and spectral assertions are unchanged.
- The targeted final four-case rerun passed in 1.6 minutes: paired legacy/current
  output loopback, both matched codec stage tests and the isolated stereo/Discord
  exclusion test. The latency acceptance reads back the configured sink property
  and requires a same-device median improvement above 75 ms, rather than merely
  accepting the old 260 ms delay under a broad absolute ceiling. The final pair
  detected all eleven events each, at about 260 ms and 100 ms respectively.
  The complete 13-case suite was not rerun after these fixture-only refinements.
- Fresh independent GPT-6.1-Sol high-reasoning review approved the bounded source
  correction with no blocking findings. Its two P3 findings were addressed:
  read back/assert the sink setting and pair timing acceptance, and narrow
  the stage-test claims from playback continuity to audio callback delivery.
  The review expressly does not approve a claim of resolved physical A/V sync.

The correction is prepared separately for review and physical acceptance. The
installed app, active production room, public release and VPS were not updated.
