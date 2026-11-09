# Native Windows publisher

The Rust publisher sends already encoded video into the existing LiveKit room. GStreamer captures and converts D3D11 textures and uses NVIDIA NVENC or AMD AMF. LiveKit handles transport and forwards compressed media; the VPS does not decode/re-encode it. There is no CPU encoder fallback. Browser camera publication remains available.

## Development

Use Windows 10 1903 or newer (Windows 11 recommended), current NVIDIA/AMD drivers, Rust 1.95.0 and Visual Studio C++ build tools. The toolchain file pins Rust. From the repository root:

```powershell
npm run native:setup
npm run native:check
npm run native:test
npm run native:probe
npm run native:run
```

Setup downloads the pinned official GStreamer 1.28.8 development installer, verifies SHA-256, and installs it for the current user into ignored `.local/gstreamer`. No administrator install is required. Set `FREEDOMSTREAM_GSTREAMER` to an equivalent development installation to use an existing runtime. The helper sets its DLL and pkg-config paths. Runtime redistribution and an installer are Phase 8; copying the executable alone is insufficient.

Setup also prepares a checksum-verified LiveKit SDK source package in `.local/native-sdk/livekit`, with a narrow fix for announcing native stereo. Run setup before any Cargo command on a fresh checkout. The source transformation and complete source-tree pin are reviewed repository files; altered generated source is rejected. See the [SDK maintenance decision](adr/0003-native-stereo-publication-feature.md). The SDK retains Apache-2.0 licensing and is not copied into the tracked application source.

Create a room in the browser, copy its invitation, paste it into the publisher, and join with a display name. Select a monitor/window and an available hardware encoder. Start/Stop controls capture; Leave and closing the publisher also cancel capture. Open the browser room separately for watching friends. Monitor capture uses DXGI; window capture uses Windows Graphics Capture. The latter retains the latest GPU frame for static scenes and validates that the original window/process still exists. A windowed/borderless game can be selected as a window; no game injection is implemented.

Video is source-bounded and aspect-preserving through a 4K envelope and 120 fps, with Auto or manual 3–100 Mbps bitrate. Auto follows resolution/FPS/codec/content and has a 35 Mbps group-use ceiling. Apply settings changes bitrate/profile live; source/codec/resolution/FPS/audio changes validate first, then briefly restart capture/publication. Friends choose Watch again for that new publication. Congestion lowers the effective encoder target without rewriting requested geometry/FPS. Native measurements distinguish encoder output from actual outgoing RTP; the browser displays received dimensions/decoded FPS/Mbps. See [quality behavior and limits](quality.md).

Choose a selected application or desktop application mix under Stream audio; audio starts off. Application audio requires Windows build 20348 or newer (Windows 11 for normal desktop editions). Use Watch friends for the isolated Chrome viewing window, which stays out of captured audio. See [audio behavior and exclusions](audio.md). Private still previews are automatic at stream start and at most once a minute; live bitrate changes keep that clock, and HTTP delivery never waits in the video pump. See [preview behavior](previews.md). Microphone capture remains off.

`npm run native:qualify -- monitor 120` exercises local monitor capture/encoding at the chosen supported FPS and discards its frames. Without an FPS argument the local qualification baseline remains at most 1080p60/12 Mbps. Neither local output FPS nor a UI mode option alone certifies a receiver or unique game frames. See the [Phase 6 verification record](verification/phase-6.md).

HEVC requires a receiver that advertises WebRTC H.265. The browser disables Watch for an unsupported codec and never subscribes its associated audio. Choose H.264 explicitly in the publisher for those viewers. No server transcode is substituted silently. Available factories depend on the actual GPU/driver/runtime; discovering AMF code is not AMD qualification.

## Verification tools

```powershell
npm run native:qualify
npm run native:qualify -- monitor
npm run native:qualify -- monitor-wgc
```

The first command encodes a GPU-generated pattern; the other commands capture the local monitor through each backend. They discard all buffers locally, upload nothing and write no video/images. Each detected encoder must produce 120 frames and release its pipeline. `npm run native:run -- --sources` explicitly lists window titles/source IDs; do not paste that output into public logs. Pass a chosen source ID to `native:qualify` to check a window.

For the automated native-to-browser test, run the app and a reachable SFU, install Chrome, and set `FREEDOMSTREAM_NATIVE_EXE` to the built executable. Add GStreamer's `bin` directory to PATH, then `npm run native:e2e`. It supplies invitation authority through bounded standard input, never command-line arguments. It checks hardware HEVC and H.264 decode, late Watch, stopped RTP traffic, rewatch and native cleanup. A synthetic test is not a game benchmark.

The loopback-only Docker development SFU passes browser tests, but the current Rust WebRTC build timed out establishing ICE through Windows Docker Desktop's loopback forwarding. Native qualification therefore used a separate SFU on the VPS with public media candidates and authenticated TLS signaling. Do not expose the local Compose stack to work around this; production networking belongs to Phases 7/8.

## Boundaries and dependencies

The GUI never handles per-frame callbacks. A bounded worker channel owns admission and publication, and a blocking pump owns the GStreamer pipeline. Stop/Leave/terminal disconnection cancel pending and active acquisitions immediately; a dropped pending publication also releases its capture lease. The pipeline goes to NULL on success, cancellation and errors. Only compressed access units are copied into LiveKit's pre-encoded API. Queues are bounded, raw frames can drop under pressure, and encoded reference frames are not deliberately discarded.

Room invitations and cookies stay in process memory. Remote cleartext origins, URL credentials, query parameters, redirects and malformed invitation authorities are rejected. Normal logs exclude SDK diagnostics. Optional `FREEDOMSTREAM_DIAGNOSTICS=1` emits a strict allowlist of connection-state/candidate information without signaling tokens or SDP authority. Keep diagnostic output private because candidate IP addresses remain network information.

Original application code is MIT licensed. GStreamer and its plugins retain their upstream licenses (including LGPL components), as do LiveKit/WebRTC and other dependencies. The installer is fetched separately for development; a redistributable package must include applicable notices, license texts and LGPL compliance materials before release. The application does not link libobs or a GPL software encoder. Adding Linux/macOS requires platform capture/encoder adapters and runtime qualification, not changes to private-room authorization or browser subscriptions.
