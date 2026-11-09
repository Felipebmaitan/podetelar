# Phase 6 verification

Phase 6 adds native quality controls, live bitrate changes, explicit source/codec/mode restarts, encoder-cap negotiation checks, congestion policy and aggregate sender/receiver measurements.

## Observed and automated checks

- The pure quality tests cover portrait/ultrawide/even/source/4K bounds, Auto dependence on resolution/FPS/codec/profile, manual geometry preservation, invalid rates and requested-versus-congestion-capped bitrate. Browser lifecycle tests cover received counter deltas and stopping collection on unwatch.
- Real native transport checks exercise a synthetic 3840×2160 GPU texture at 120 fps and a 100 Mbps request through the Canada SFU to Chrome, a live 3→25 Mbps H.264 edit at 1080p120 without changing publication SID, and a source/codec/resolution/FPS restart followed by explicit Watch and 720p60 H.264 decode. The observed 4K five-second receive window was 120.1 decoded fps. Qualification uses frame-counter windows rather than requiring every instantaneous RTC label to equal 120.
- Actual native UI inspection confirmed that 1440p/4K were unavailable for the 1080p monitor, switching 60→120 fps changed Auto 13→25 Mbps, and choosing Manual 3 Mbps preserved 1080p120. No room invitation or real desktop frames were transmitted during that UI check.
- A blank app window exposed the invisible resize-margin mismatch during source-bound validation. DWM bounds correctly report its 762×652 captured area; both NVIDIA HEVC/H.264 then encoded 120 frames near 60 fps. These window frames were discarded locally. No qualification uploads desktop/window/camera pixels; WAN scenarios use only synthetic GPU patterns/audio tones.
- Local DXGI monitor qualification at 1080p120 encoded 360 frames for each NVIDIA codec near the selected output rate after adding raw-GPU frame limiting/clock pacing. These desktop frames were also discarded locally.
- Repeated H.264 live-bitrate tests identified small packet loss and incomplete frames on the long WAN hop, while per-frame decode cost was below 0.4 ms. A 300 ms receiver-buffer preference improved recovery; an A/B loss window decoded about 116 fps instead of roughly 42–57 fps without that preference. Three subsequent runs through the actual application/public SDK playback path decoded about 120 fps, with zero decoder-dropped frames in their sampled windows. Tests no longer inject receiver-buffer overrides. Low delay remains an explicit user choice.

The final regression passed all nine native scenarios through the Canada SFU and six desktop/mobile browser scenarios, with two intentional duplicate browser transport skips. This includes audio isolation/stereo, stopped A/V subscriptions, reconnect cleanup, minute previews/zero unwatched RTP, 4K120, live bitrate edits and the source/codec/mode restart. The native quality scenario also verified the real Low delay/Smooth control changes the receiver preference, without a test-injected override. The final synthetic 4K window decoded 119.9 fps with a 100 Mbps request; sampled encoder/RTP rates were about 40 Mbps with a 44 Mbps effective congestion target. The live H.264 edit retained its SID/1080p120 geometry and decoded about 120 fps at its settled 25 Mbps target.

Docker smoke with restart/session persistence, Node checks (20 API/config and 19 media lifecycle tests), native format/clippy/20 Rust tests and three setup-integrity checks pass. Exact-head CI and a fresh GPT-6.1-Sol high review remain the final PR gate.

## Qualification limits

The host is an RTX 5080 with a 1920×1080 monitor. Synthetic 4K120 encoding/forwarding/decoding does not certify physical 4K120 monitor capture, real games, fullscreen/alt-tab, every GPU generation, driver, display, or an entire group evening. There is no AMD machine available. AMD factory/caps/AMF paths remain implemented and source-reviewed but unqualified on hardware. No domain or production deployment is claimed here.

The temporary SFU has its existing 1 CPU/512 MiB qualification limit. These short single-publisher scenarios are not a five-publisher load test. Requested bitrate is distinct from observed encoder/RTP rate; bandwidth adaptation and actual scene compressibility can prevent a 100 Mbps setting from producing 100 Mbps on the wire.
