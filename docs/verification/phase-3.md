# Phase 3 verification

Phase 3 implements a native Windows video publisher. These results distinguish implementation checks from hardware/product qualification. The absence of AMD hardware was confirmed by the owner; it remains unverified.

## Observed locally

Windows host: NVIDIA RTX 5080, driver 621.14, 16 GB VRAM; monitor 1920×1080 at 300 Hz. GStreamer 1.28.8, Rust 1.95.0, LiveKit Rust SDK 0.10.1, LiveKit server 1.13.11. NVENC HEVC/H.264 factories detected; AMF factories were not available on this NVIDIA host. Capture/convert/encode negotiate D3D11-memory NV12 and use the allowlisted hardware factory.

| Check                                            | Result                                                                                                                          |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------- |
| GPU test-pattern HEVC/H.264 encoding             | 120 frames each, approximately 60 encoded fps; pipeline released                                                                |
| Physical monitor via DXGI                        | 120 frames each codec; approximately 60 encoded fps; buffers discarded locally                                                  |
| Physical monitor via WGC, including static scene | 120 frames each codec; approximately 60 encoded fps after adding clocked latest-frame retention; buffers discarded locally      |
| Native HEVC and H.264 → actual VPS → Chrome      | Both decoded at 1920×1080; selective Watch, Stop watching and rewatch verified with RTP byte counters; native pipeline released |
| Native cancellation/redaction tests              | Queued/active cancellation, dropped pending capture lease, strict invitation parsing and secret-safe diagnostics pass           |
| Unsupported browser HEVC                         | Lifecycle test verifies no video or associated audio subscription                                                               |

WAN verification used a temporary resource-limited LiveKit container on the supplied Canada VPS, public SRTP media, and trusted TLS signaling. Existing application containers and production Nginx routes were not replaced. This is a transport qualification, not a production deployment. No desktop pixels or window titles were uploaded: WAN tests publish a GPU-generated fixture only.

An existing window was also captured through WGC with 120 frames per codec at approximately 60 encoded fps; all buffers were discarded locally.

## Remaining acceptance evidence

- AMD AMF HEVC/H.264 on a real supported Windows GPU.
- Real game GPU/CPU utilization, frame-time impact and visual quality under motion.
- Exclusive fullscreen, alt-tab, minimized/restored/resized windows and mixed-DPI/multi-monitor transitions.
- Browser/driver compatibility beyond the tested Chrome build; no universal HEVC promise.
- 4K120, 3–100 Mbps control behavior and high-rate network capacity (Phase 6).
- Audio isolation/feedback behavior (Phase 4), an evening with 4–5 simultaneous streams and final shared-VPS resource measurements (Phase 8).

CI compiles, lints and tests the Windows application and builds a release executable. Hosted CI does not have the physical encoder/capture hardware needed to close these items. No AMD, 4K120 or game-performance certification is implied by merging implementation work.

The first hosted native setup run exposed PowerShell 7 module-path inheritance through npm into Windows PowerShell 5.1, hiding `Get-FileHash`. The helpers now prioritize their executing host's built-in modules. A Windows regression test runs setup from Node with a deliberately inherited PowerShell 7 module path and a tampered cached installer, verifies checksum rejection, and uses a path containing spaces. Installer download progress is suppressed and the download cache remains subject to the pinned SHA-256 check.
