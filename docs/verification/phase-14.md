# AV1 codec phase: local verification

Branch: `feat/av1-native-codec`, stacked on `feat/fullscreen-stream-performance`.
This phase adds NVIDIA NVENC and AMD AMF hardware-only AV1 publication, native
Direct3D11 AV1 decoding, and the SDK dav1d CPU receive path when no AV1 hardware
decoder is registered. No software encoder or additional runtime DLL is introduced.

## Behavior

The encoder sends SDR 8-bit Main-profile AV1 with OBU temporal-unit framing.
Advanced codec selection exposes only registered hardware factories. Automatic
AV1 bitrate targets initially reuse HEVC targets; this is a conservative application
policy, not a measured compression-efficiency claim. Requested resolution/FPS and
manual bitrate retain their existing behavior.

An incompatible/stalled watcher can request H.264 replacement of the one shared
publication. AV1 watcher requests participate in the existing aggregate bandwidth
allowance. Watch intent survives replacement, Stop watching unsubscribes video and
audio, and still previews retain the start/once-per-minute policy.

AV1 RTP omits temporal delimiters. The receive adapter restores the two-byte TD OBU
before each complete received single-layer frame. Without it, the pinned parser
cleared PTS on later frames, causing the RTP timestamp association to discard decoded
pictures. The new SFU tests reproduced that failure and passed after the delimiter
fix. This adaptation is specific to WebRTC complete-frame input, not arbitrary AV1
file or multi-layer temporal-unit input.

## Verification

Local hardware: NVIDIA GeForce RTX 5080, GStreamer 1.28.8. Development builds and a
1080p60 synthetic GPU source were used. Frame advancement proves functioning media
transport/decoding, not sustained 60 FPS, game performance or latency qualification.

- Node 24.11.1 `npm run check`: formatting, types, 36 server tests, 21 web tests,
  4 relay/production policy tests and production builds passed.
- `npm run native:check`: formatting and application Clippy checks passed; pinned
  upstream SDK warnings remain upstream.
- `npm run native:test`: 34 publisher and 7 portable tests passed, including the
  isolated Windows persistence child test.
- Fresh SDK extraction and patching reproduced both decoder checksum pins.
- Local AV1 SFU acceptance: actual NVENC AV1 publication, GPU implementation
  `FreedomStream Direct3D11` / hardware=true, forced CPU implementation `dav1d` /
  hardware=false, AV1 frame advancement, one initial preview, explicit Stop/rewatch
  byte checks, post-reconnect frame advancement and shared H.264 fallback passed.
- Final SFU run: all five tests passed in 2.2 minutes, including the three AV1
  cases above and existing HEVC/H.264 receive regression tests.
- Fresh GPT-6.1-Sol high-reasoning review approved the final AV1 diff after fixing
  reconnect assertions and the receive temporal-delimiter handling.

Ignored local logs: `av1-check-node24.log`, `av1-native-check.log`,
`av1-native-test.log` and `av1-final-room-smoke.log` under `.local/`.

## Open gates

AMD AMF AV1 execution, physical receivers lacking AV1 hardware, multiple concurrent
software streams while gaming, synchronized audio/video latency, fresh-machine and
portable/offline AV1 playback, and 4K120 remain unqualified. Forced dav1d playback on
the local NVIDIA machine exercises the actual CPU decoder, not an older physical
GPU. No small-CPU-cost or multi-stream real-time guarantee is made. No deployment,
portable release or merge is part of this change.

Vendor support and calculations are in [AV1 hardware research](../research/av1-hardware.md).
