# Native Windows publisher

The native Windows app publishes and watches streams in existing LiveKit rooms.
GStreamer captures D3D11 textures and uses NVIDIA NVENC or AMD AMF. A pinned
WebRTC decoder adapter provides native H.264/HEVC/AV1 playback through GStreamer.
AV1 receivers without a registered GPU decoder use the SDK dav1d CPU decoder.
Audio uses WASAPI. LiveKit forwards compressed media without server transcoding.
There is no embedded/external browser or CPU encoder fallback. The website handles
invitations, access management and app downloads.

## Development

Use Windows 11 (process-loopback audio requires build 20348+), current NVIDIA/AMD
drivers, Rust 1.95.0, Node 24 and Visual Studio C++ build tools.

```powershell
npm run native:setup
npm run native:check
npm run native:test
npm run native:probe
npm run native:run
```

Setup verifies and installs the pinned GStreamer 1.28.8 development runtime into
ignored `.local/gstreamer`, and prepares checksum-verified SDK source patches for
native stereo and the D3D11 decoder bridge in `.local/native-sdk`. Generated source
changes are rejected. See [SDK maintenance](development.md#native-dependency-maintenance)
and `deploy/native-decoder.json`. Friends use [Setup](distribution.md);
the raw Cargo executable still requires its runtime. Both production executables
use the GUI subsystem. Helper processes run hidden.

Open a room invitation from `/stream` or `/telar` in Discord, or paste a link and
enter your nickname. Each invitation selects its backend; no server, nickname,
room or language preference is restored or saved. Later links may reuse the nickname
and join when idle while the app remains open. An active room requires confirmation
before switching. Sharing and watching remain explicit. See
[launch and firewall behavior](distribution.md#invitations-and-launch-behavior).

The initial 1280×720 viewport has sharing controls on the left and larger native
stream tiles on the right. Watch sits in the center of each still preview. Stop
watching actually unsubscribes video and audio; focus/hiding does not. Each stream
has independent volume. Preview capture occurs initially and at most once/minute.

Source resolution defaults to the monitor/window bounded to 4K, at 60 FPS. The
visible slider offers 15–120 FPS; codec/bitrate live in Advanced. Auto starts around
6 Mbps AV1/HEVC or 8 Mbps H.264 for 1080p60 motion, with a 35 Mbps ceiling. Manual
1–100 Mbps remains available. A shared watching allowance and receiver feedback
can reduce the one publication for all watchers without rewriting resolution/FPS.
See [quality behavior](quality.md) and [bitrate guidance](bitrate-guidance.md).

Audio follows the source: a selected window's process tree or the approved desktop
application mix. Discord and native playback are excluded, and the microphone is
never acquired. See [automatic source audio](audio.md). Monitor capture uses DXGI;
windows use Windows Graphics Capture. No injection into games is implemented.

Rooms rank available codecs AV1 → HEVC → H.264 within the sender's preference
and every connected member's decoder capabilities. Compatible rooms can return
to the preferred codec when restrictions leave. This remains one shared encoding,
with Watch intent and preview cadence preserved across replacements. SDR only;
HDR fidelity and physical 4K120 remain unqualified.

AV1 publication requires a runtime-probed hardware encoder. Receivers without a
registered GPU decoder can use the SDK's dav1d CPU decoder; that fallback does not
promise low CPU use while gaming or watching multiple streams. The presence of
AMD factories in the runtime does not establish AMD hardware qualification.

## Verification tools

```powershell
npm run native:qualify
npm run native:qualify -- monitor
npm run native:qualify -- monitor-wgc
```

The first command encodes a GPU-generated pattern; the other commands capture the local monitor through each backend. They discard all buffers locally, upload nothing and write no video/images. Each detected encoder must produce 120 frames and release its pipeline. `npm run native:run -- --sources` explicitly lists window titles/source IDs; do not paste that output into public logs. Pass a chosen source ID to `native:qualify` to check a window.

For native acceptance, build the app/server, then run `npm run native:room-smoke`
under Node 24 on Windows. It downloads a checksum-pinned official Windows LiveKit
v1.13.11 test server and runs the current API and native clients on isolated local
ports (3010 and 17880–17882). It does not change Docker, Windows settings or the
VPS. The selected local adapter must be reachable; override its discovery with
`PODETELAR_TEST_NODE_IP` if needed. A working default playback device is needed
for audio checks. Native invitation authority travels through bounded stdin only.

The AV1 acceptance suite (`native:room-smoke -- av1.spec.ts`) checks actual GPU
and forced dav1d decoder implementations, Stop/rewatch traffic, reconnect frame
advancement and H.264 replacement. `PODETELAR_AV1_DECODER=software` bypasses
the custom AV1 GPU factory for CPU qualification; unset it for normal selection.
These tests require an AV1 hardware encoder, and the GPU case requires a decoder.

Set `PODETELAR_NATIVE_EXE` to test another built/portable executable. For
portable audio fixtures, also set `PODETELAR_NATIVE_TONE_EXE` to its extracted
publisher executable, so Game.exe/Discord.exe fixture process names remain real.
Native TURN/TLS, larger groups, real games and physical A/V latency require
separate qualification; local SFU checks do not establish those properties.
