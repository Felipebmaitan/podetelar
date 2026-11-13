# Standard native audio — release qualification

Native WebRTC output is the normal playback path. No user option, environment
feature switch or alternate stream-audio backend is required. GStreamer remains
in capture/codec pipelines and explicit historical comparison fixtures.

The existing LiveKit transport and codecs remain. Each publisher has its own
negotiated A/V group, and the native Windows ADM supplies device-delay feedback.
New mixer sources start silent; a playback task owns an immutable source
generation and per-stream gain. Recording/microphone initialization stays guarded.

## Output recovery

Every 500 ms the shared output owner checks the Windows default render endpoint.
A stalled mixer is detected after three seconds. Missing output pauses physical
rendering while synthetic WebRTC pumping keeps received audio current; output
is retried without room rejoin. Endpoint changes stop/reinitialize only this
process's renderer. Windows default settings and other applications are not changed.

Device restarts preserve mixer gains and generation ownership. Stale sources are
discarded before acquiring a device, completed playback tasks are retried with a
one-second minimum interval, and each replacement task owns its own status
channel. An old task cannot leave an error on a successful replacement.

Actual Windows process-loopback recovery qualification passed:

- Three render endpoints were available.
- Output stopped for three seconds and resumed automatically.
- The renderer selected a different real endpoint and returned to the default,
  recording two route changes and four output starts.
- A muted stream stayed silent after recovery and during switching.
- Audio returned after unmute, and the output owner count returned to zero.

The absence interval is a controlled renderer suspension through the same recovery
path. It is not a claim that a cable was physically unplugged. Actual endpoint
selection/reinitialization and process-loopback output were exercised.

## Validation

The 21-case real-media suite passes without a feature flag while two additional
long-running publishers/watchers are active. It covers captured H.264/AV1 timing,
GPU/software AV1, HEVC/H.264, stereo/exclusion, per-stream gain/mute, late muted
subscriptions, replacement, Stop/rewatch, reconnect, fallback, aggregate caps,
diagnostics and output recovery.

An initial thirty-minute simultaneous H.264/AV1 run had continuous rendering,
zero output failures and stage-offset drift of +12.66 / -4.55 ms. Its unequal
marker counts (1095/1080 and 1096/1080 video/audio) prevent using it as the final
timing acceptance. That fixture used repeating intervals and shorter audio than
video markers, and did not retain the audio event timeline.

The final candidate is rerunning with a deterministic nonrepeating interval
sequence, equal 100-ms markers and both event timelines retained. It checks
advancing audio/video, native rendering, recording-disabled state, count agreement,
event pairing and change in median stage offset between the first and last
approximately-minute windows. The drift gate is 20 ms. Final numeric results will
be recorded here.

Node 24 checks, native Clippy/workspace tests and ten adapter/pin tampering checks
pass. A fresh independent GPT-6.1-Sol high review approved the runtime source after
fixing a completed-task cleanup bug that had bypassed its retry backoff. A Tokio
regression checks retention until the deadline and removal on unsubscribe.
Final rebuilt-artifact checks are recorded after completion.

The packaging scripts now honor Cargo's configured target directory for the
publisher, native license provenance and installer launcher. This prevents a
successful build in a custom target directory from packaging an older default
directory binary.

Reproduce the sustained test with `npm run native:soak` after setting
`FREEDOMSTREAM_NATIVE_EXE` to the candidate executable. This variable selects a
test artifact; it does not enable a playback feature. The harness isolates its
ports and output directory from regular media/browser tests.

## Rollout and measurement boundaries

The server needs `room.sync_streams: true`. It is present in local and generated
production configuration. The existing production change detector includes the
configuration generator, so this change must travel with the backend rollout.
Current VPS configuration and WAN playback are not inferred from local tests.

Mailbox/process-loopback measurements exercise actual capture, codecs, transport
and Windows rendering, but they are not physical display/speaker measurements.
Do not claim zero physical offset or universal 15 ms-leading / 45 ms-lagging
compliance from these results. The default release behavior and its tested
stability are separate from such an absolute latency claim.
