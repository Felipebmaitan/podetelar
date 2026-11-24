# Native Windows publisher

The native Windows app publishes and watches streams in existing LiveKit rooms.
GStreamer captures D3D11 textures and uses NVIDIA NVENC or AMD AMF. A pinned
WebRTC decoder adapter provides native H.264/HEVC/AV1 playback through GStreamer.
Only available hardware decoders authorize reception; software AV1 availability
does not count as support.
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
enter your nickname. Each invitation selects its backend; the last successful guest
nickname is remembered locally. Discord sign-in defaults to the hosted backend and
uses the system browser. The selected login backend is remembered separately;
its credential uses OS credential storage scoped to that exact public origin.
The theme follows the operating system by default, using Black for dark
appearance and Light for light appearance. Choose a fixed theme in Settings or
Advanced settings. Theme and interface language are saved locally in
\`appearance.json\` under the PodeTelar application data folder. Room invitations
are kept in memory. See [Discord accounts](discord-accounts.md).
Guest pictures and live nickname editing are described in [guest profiles](guest-profiles.md).
Later links may reuse the nickname
and join when idle while the app remains open. An active room requires confirmation
before switching. Sharing and watching remain explicit. See
[launch and firewall behavior](distribution.md#invitations-and-launch-behavior).

The room dock groups screen and camera sharing separately. Open the arrow on
**Share screen** to choose a capture source or adjust stream quality. While
sharing, use the same arrow to apply changes. The camera menu has separate
quality settings. Open **Camera settings** and select a Windows camera, then click
**Share camera**. The selected device is remembered locally. If it is disconnected,
refresh the list and reconnect it or select another device explicitly. Windows camera
discovery includes OBS Virtual Camera when its driver is registered and usable.
Camera streams publish only video. Each participant can share one source at a time.
Use **Stop camera** before changing camera settings or starting screen sharing.
Watching cameras requires a native app version with camera playback support.
Camera capture is currently Windows-only. The stage toolbar offers Grid, Spotlight and Focus.
Spotlight keeps one stream large and the others in a scrollable strip. Highlighting a stream changes the layout.
Watch subscribes to video and audio, and Stop watching unsubscribes. Each stream
has independent volume. Switching between a participant's camera and screen
requires selecting Watch again. When only you are live, the stage shows your most recent
preview. While you share and watch someone else, you can hide or restore the small
local preview beside the streams. Preview capture occurs initially and at most
once per minute.

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
and every connected member's explicitly advertised hardware decoders. The server
includes these capabilities in the initial native media token so a joining member
does not briefly appear H.264-only. Missing/older capabilities remain pending;
starting a stream requires every member to report hardware support. A live stream
keeps its current codec until explicit capabilities exclude it or the streamer
applies settings. Interruptions and departures alone do not change codecs.
This remains one shared encoding,
with Watch intent and preview cadence preserved across replacements. SDR only;
HDR fidelity and physical 4K120 remain unqualified.

AV1 publication requires a runtime-probed hardware encoder. Receivers without a
registered AV1 GPU decoder constrain the room to another common hardware codec.
The SDK's bundled software decoder is not used by the app's subscription policy.
The presence of
AMD factories in the runtime does not establish AMD hardware qualification.

## Verification tools

```powershell
npm run native:qualify
npm run native:qualify -- monitor
npm run native:qualify -- monitor-wgc
```

The first command encodes a GPU-generated pattern; the other commands capture the local monitor through each backend. They discard all buffers locally, upload nothing and write no video/images. Each detected encoder must produce 120 frames and release its pipeline. `npm run native:run -- --sources` explicitly lists window titles/source IDs; do not paste that output into public logs. Pass a chosen source ID to `native:qualify` to check a window.

`npm run native:run -- --cameras` lists camera names and local device IDs.
Set `PODETELAR_TEST_CAMERA` to the exact name of a camera with a 720p-compatible
mode and run `npm run native:room-smoke -- camera.spec.ts`. This opt-in test
publishes the chosen camera to the isolated local SFU. Use a test scene or aim
the camera at a test target before running it. Run it separately for each device.

For native acceptance, build the app/server, then run `npm run native:room-smoke`
under Node 24 on Windows. It downloads a checksum-pinned official Windows LiveKit
v1.13.11 test server and runs the current API and native clients on isolated local
ports (3010 and 17880–17882). It does not change Docker, Windows settings or the
VPS. The selected local adapter must be reachable; override its discovery with
`PODETELAR_TEST_NODE_IP` if needed. A working default playback device is needed
for audio checks. Native invitation authority travels through bounded stdin only.

The AV1 acceptance suite (`native:room-smoke -- av1.spec.ts`) checks actual GPU
decoding, exclusion of software-only AV1 support, Stop/rewatch traffic, reconnect
frame advancement and H.264 replacement. `PODETELAR_AV1_DECODER=software` masks
AV1 hardware availability for qualification; the app must then use another common
hardware codec, never dav1d. Unset it for normal selection.
These tests require an AV1 hardware encoder, and the GPU case requires a decoder.
`codec-adaptation.spec.ts` checks room-wide hardware restrictions and stable codec
selection after departures. `codec-interruptions.spec.ts` uses an isolated SFU's
explicit test-only mute/unmute permission to verify real AV1/HEVC recovery after
nine seconds without frames, preserving the publication and capability set.

Set `PODETELAR_NATIVE_EXE` to test another built/portable executable. For
portable audio fixtures, also set `PODETELAR_NATIVE_TONE_EXE` to its extracted
publisher executable, so Game.exe/Discord.exe fixture process names remain real.
Native TURN/TLS, larger groups, real games and physical A/V latency require
separate qualification; local SFU checks do not establish those properties.
