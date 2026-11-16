# macOS qualification — 2026-11-16

Test host: Apple M5 Pro, ARM64, macOS 27.0.1 (26A434), Node 24.22.3,
Rust 1.95.0, official GStreamer 1.28.8 SDK. This record qualifies this host;
macOS 14 is the deployment target, not an additional tested operating system.

## Passed

- `npm run check`: formatting, TypeScript, API/tool tests and production builds.
- Native publisher formatting/Clippy and Rust tests: 80 passed, five existing
  integration tests ignored. Installed launcher formatting/Clippy and both tests
  passed. Decoder source was regenerated from the pinned crate, rather than
  trusting the development directory, and matched the committed tree digest.
- Native tool integrity tests, including macOS adapter tampering, and release
  feed checksum/channel/path/version tests passed.
- Full Docker build, `npm run smoke -- --restart-app`, and nine admin browser
  tests passed against actual HTTP services and persistent room storage.
- Hardware-only `vtenc_h264_hw` and `vtenc_h265_hw`: each encoded 180 actual
  1920×1080 frames at 60 FPS in the local qualification pipeline. Each captured
  one initial preview. This encoder test does not measure network delivery.
- Actual ScreenCaptureKit display capture → native LiveKit SFU → native
  VideoToolbox H.264/HEVC receive: both codecs delivered over 500 frames at
  1280×720/30, reported hardware decode and delivered decoded frames to the native consumer, stopped receiving
  media after Stop watching (zero measured bytes), and resumed on rewatch.
- Actual owned changing window and stereo CoreAudio source through the same SFU:
  H.264 delivered 546 frames; HEVC delivered 553. Both used the hardware encoder
  and hardware decoder, captured one preview, received distinct stereo channels,
  started the platform output renderer without microphone recording, and
  measured zero bytes after Stop watching. The first receive interval contained
  287,040 and 283,200 audio frames respectively.
- Desktop audio spectral check: external 440 Hz tone amplitude 0.00498;
  excluded PodeTelar 1320 Hz playback leakage 0.00000649. This checks self-output
  exclusion, not an actual Discord call.
- Late application audio test: after 4.5 seconds of initially quiet desktop,
  a newly launched allowed source was captured. A louder excluded source
  remained below 0.0001 amplitude throughout launch and restart. The actual
  Rust audio pipeline also stayed alive through the quiet interval. Desktop
  capture uses positive membership refreshed once per second so an excluded
  process cannot enter the mix while awaiting an inventory refresh.
- AV1 follow-up: actual VideoToolbox hardware receive through the same native
  SFU passed on M5 Pro. A 1280×720/30 inter-frame AV1 fixture delivered 540
  accepted frames and 180 frames delivered to the native consumer, with hardware decoding, successful
  rewatch and zero bytes after Stop watching. An earlier all-keyframe 640×360
  fixture also passed (540 accepted, 205 native frames). These are receive checks;
  the pre-encoded fixtures were generated externally using software SVT-AV1.
  The qualification sender is behind an explicit build feature and excluded
  from the shipped app. AV1 encoding remains unavailable on macOS.
  The packaged AV1 receiver also passed (540 accepted, 190 native frames, zero Stop
  watching bytes). The display was asleep during the follow-up, so current
  H.264/HEVC codec regression used an explicit test pattern; both codecs
  accepted 548 frames with hardware decode and zero Stop watching bytes. These
  counters establish native delivery, not physical display presentation. Earlier actual
  ScreenCaptureKit checks above remain the capture qualification evidence.
- Self-contained ARM64 application, DMG, full Velopack package and
  `releases.osx-arm64.json` built; nested code signatures passed strict deep
  verification. Runtime load paths are bundle relative and the dependency
  allowlist rejects unqualified libraries/minimum OS versions.
- Real two-version Velopack macOS update test: discovery, verified download,
  Later preserving the installed version, repairing a corrupted cached package,
  explicit restart, actual UpdateMac replacement and relaunch all passed. The
  process fixture uses an isolated state directory; it does not prove media
  shutdown behavior during an upgrade.
  Its temporary directory avoids macOS Documents privacy prompts on relaunch.
- Outer application launch opened the native GUI without a terminal. A
  `freedomstream://` handoff to the running application prompted for a name and
  explicit confirmation, joined the local room, and allowed leaving and closing.

The Docker Desktop SFU's media path was not reachable from the native test client.
Hardware media tests therefore used the real SFU built from the same pinned
LiveKit source on the Mac, with a separate local API/room database. They did not
substitute a mock for network transport. Generated inputs, raw logs, credentials,
room databases and binary test artifacts remain local and ignored.

## Remaining distribution and hardware gates

- CI currently signs ad hoc. Developer ID credentials/notarization are not
  configured. Gatekeeper installation and updates on a fresh Mac remain
  unqualified; an ad hoc signature is not Apple notarization.
- The GUI's separate capture permission could not be added through System
  Settings because macOS required protected Touch ID/password authentication.
  The tool cannot complete that dialog. GUI capture after that permission grant
  remains unverified; CLI hardware qualification used existing development
  capture permission. The permission failure is surfaced in the app.
- No physical speaker/display A/V offset measurement, actual Discord exclusion
  session, alternate output-device unplug/replug test, fresh Mac installation,
  macOS 14 run, Intel Mac support, other M-series chip, 4K120 or Windows GPU
  regression qualification is claimed.
- GitHub-hosted macOS checks build and validate artifacts and the real updater
  fixture. They cannot establish interactive capture, GPU qualification or
  physical output. The release workflow's actual hosted result is tracked on
  the PR/release run.

See [macOS development and distribution](../macos.md) for reproducible commands.
