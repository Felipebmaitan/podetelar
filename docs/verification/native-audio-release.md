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

The final thirty-minute simultaneous run **passes** with a deterministic
nonrepeating interval sequence, equal 100-ms markers and both event timelines
retained. All progress, native-rendering, recording-disabled, count-agreement,
pairing and teardown gates pass. Each renderer started once, reported zero output
failures, and reported 42 ms device delay. Output ownership returned to zero.

The stage-offset drift gate compares two 60-event windows (after ten warmup pairs
and at the end), with an absolute limit of 20 ms:

| Codec | Video / audio / paired events | Start median (ms) | End median (ms) | Change (ms) |
| ----- | ----------------------------- | ----------------- | --------------- | ----------- |
| H.264 | 1057 / 1056 / 1056            | 89.002            | 69.820          | -19.182     |
| AV1   | 1056 / 1056 / 1055            | 77.216            | 82.608          | +5.392      |

The interval starts at video mailbox publication and ends at the audio loopback
callback, including loopback capture/scheduling delay. It is not physical lip
sync. The H.264 drift result is close to the gate; this run does not establish a
wide margin across other hardware or networks. Numeric evidence and artifact
digests are in [native-audio-release-measurements.json](native-audio-release-measurements.json).

Node 24 checks, native Clippy/workspace tests (79 publisher tests pass; six test
entry points are ignored by default) and ten adapter/pin tampering checks pass. A fresh
independent GPT-6.1-Sol high review approved source commit `32d74a3`, including the
final packaging correction. A completed-task cleanup bug that bypassed retry
backoff was fixed; a Tokio regression checks retention until the deadline and
removal on unsubscribe. Fresh SDK reconstruction from the pinned upstream
archives reproduces the adapter tree and header pins.

The final mute-transition check retains the fixed 440 Hz leakage limit. It also
compares output channel-difference peak against the audible mono track's measured
pre-gain difference, allowing four S16 quantization steps for loopback conversion
and imposing an absolute 0.001 ceiling. The earlier fixed 0.0005 check failed at
17 LSB while the audible mono control already measured 16 LSB before mixing; no
muted-tone violation occurred. Both transition tests passed after calibration.

The packaging scripts now honor Cargo's configured target directory for the
publisher, native license provenance and installer launcher. This prevents a
successful build in a custom target directory from packaging an older default
directory binary.

Testing the isolated bundle exposed a missing `imagefreeze` plugin, required by
actual window/WGC capture but unused by the synthetic encoder probe. The bundle
now includes its pinned DLL and checks that the factory exists before packaging.
Its LGPL license and PE dependency closure pass qualification; the matching
GStreamer monorepo sources and license notices were already included. Both actual
captured-window codec checks pass from the corrected bundle. The other 19
packaged-media cases passed with the identical publisher and existing runtime
files before this additional plugin; output recovery passed again after correction.

The final package is built from `32d74a37f572e76d9f9d8fead3870e8ee3fc8fc6`.
The long-run executable was built from `e4a26ef`; the later main-branch merge adds
PC-usage recognition without changing the native output, playback, timing source
or decoder adapter code. The merged executable's packaged media checks are
recorded separately from that sustained run.

Isolated startup from a fresh private cache, concurrent extraction, runtime and
bundled-CRT probes, payload/source integrity, cache preservation, explicit
extraction and damage rejection pass for the final portable payload. A local
unsigned installer was built; its embedded payload digest matches the qualified
portable executable. No installer was installed or published. The normal
worktree-based startup harness exceeded Windows' path limit (268 characters);
the same unchanged qualification script passed from a short temporary directory.

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
