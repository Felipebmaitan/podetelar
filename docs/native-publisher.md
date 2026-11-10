# Native Windows publisher

The native Windows app publishes and watches streams in existing LiveKit rooms.
GStreamer captures D3D11 textures and uses NVIDIA NVENC or AMD AMF. A pinned
WebRTC decoder adapter provides native H.264/HEVC playback through GStreamer;
audio uses WASAPI. LiveKit forwards compressed media without server transcoding.
There is no embedded/external browser or CPU encoder fallback. The website handles
invitations, access management and portable app downloads.

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
changes are rejected. See the [stereo SDK decision](adr/0003-native-stereo-publication-feature.md)
and `deploy/native-decoder.json`. Friends use the [self-contained portable EXE](distribution.md);
the raw Cargo executable still requires its runtime. Both production executables
use the GUI subsystem. Helper processes run hidden.

Paste an invitation and name once. After a successful join these are encrypted
for your Windows user; reopening automatically joins the saved room. Sharing and
watching remain explicit. Room options → Forget saved room removes the saved profile
and leaves. See [saved access and firewall behavior](distribution.md#saved-room-and-launch-behavior).

The initial 1280×720 viewport has sharing controls on the left and larger native
stream tiles on the right. Watch sits in the center of each still preview. Stop
watching actually unsubscribes video and audio; focus/hiding does not. Each stream
has independent volume. Preview capture occurs initially and at most once/minute.

Source resolution defaults to the monitor/window bounded to 4K, at 60 FPS. The
visible slider offers 15–120 FPS; codec/bitrate live in Advanced. Auto starts around
6 Mbps HEVC or 8 Mbps H.264 for 1080p60 motion, with a 35 Mbps ceiling. Manual
1–100 Mbps remains available. A shared watching allowance and receiver feedback
can reduce the one publication for all watchers without rewriting resolution/FPS.
See [quality behavior](quality.md) and [bitrate research](bitrate-guidance.md).

Audio follows the source: a selected window's process tree or the approved desktop
application mix. Discord and native playback are excluded, and the microphone is
never acquired. See [automatic source audio](audio.md). Monitor capture uses DXGI;
windows use Windows Graphics Capture. No injection into games is implemented.

An incompatible/stalled HEVC watcher requests a whole-publication H.264 fallback;
its Watch intent survives replacement. This is one shared encoding, not a second
encoder or server transcode. Native receive path tests are required alongside
capability probing. SDR only; HDR fidelity and physical 4K120 remain unqualified.
AMD factories being present in the runtime does not constitute AMD qualification.

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
`FREEDOMSTREAM_TEST_NODE_IP` if needed. A working default playback device is needed
for audio checks. Native invitation authority travels through bounded stdin only.

Set `FREEDOMSTREAM_NATIVE_EXE` to test another built/portable executable. For
portable audio fixtures, also set `FREEDOMSTREAM_NATIVE_TONE_EXE` to its extracted
publisher executable, so Game.exe/Discord.exe fixture process names remain real.
The old browser media suites have been replaced with native room acceptance;
historical browser/TURN evidence is retained in the phase verification documents.
Native TURN/TLS, larger groups, real games and A/V latency need new qualification.
