# Phase 8 qualification — 9 November 2026

The implementation is Windows first. Hardware evidence uses Windows 11, an RTX
5080 and Chrome on a 1080p monitor. Synthetic sources and controlled audio from
earlier phases do not qualify real games, AMD, physical 4K capture or a group call.

## Automated and deployment checks

| Check                        | Evidence                                                                                                                                                                                                                                                                                                          |
| ---------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Node 24.22.2 `npm run check` | 34 server/config/backup/download/proxy tests, 21 web lifecycle tests, 4 deployment/relay policy tests; formatting, types and builds pass.                                                                                                                                                                         |
| Rust                         | Formatting, clippy, 20 native tests and 3 SDK/setup integrity checks pass.                                                                                                                                                                                                                                        |
| Local browser suite          | 10 pass, 6 deliberate skips (duplicate mobile media checks and opt-in installed-release download). Includes real RTP, Watch/Stop, previews, access rotation/revocation and route changes during pending admission.                                                                                                |
| Windows installation         | Isolated per-user portable extraction, clean DLL/plugin search path, publisher probe, retained notices, unchanged existing GStreamer registration, ownership rejection and full uninstall pass. Final release archive is checked again before merge.                                                              |
| Production download          | The authenticated room UI downloads the ZIP and verifies its SHA-256/size; anonymous access is denied. No public GitHub repository is required.                                                                                                                                                                   |
| Native direct route          | Hardware HEVC and H.264 reach Chrome through the VPS at 1080p60; Watch/Stop changes traffic, preview appears without a full subscription, and native capture stops. Sample RTT 134–139 ms, zero reported loss/retransmits.                                                                                        |
| Native TLS route             | Both hardware codecs reach the browser with native and browser selected routes checked as TURN/TLS, certificate verification enabled, 60 encoded fps and approximately 136 ms RTT.                                                                                                                                |
| Long native TLS run          | 380 seconds of HEVC at 60 encoded fps, decoded-frame progress checked every five seconds; seven minute-spaced captures/uploads. Extends beyond the 300-second credential lifetime and 120-second allocation lifetime. Final aggregate: TURN/TLS, RTT 139 ms, zero reported loss/retransmits.                      |
| Restrictive browser route    | Non-proxied UDP disabled, both peers select TLS, media decodes, and invalid TURN credentials produce no relay candidate.                                                                                                                                                                                          |
| Recovery                     | Online WAL-aware backup, integrity verification, revocation, stopped-app restore, restored invitation/session admission and test-session cleanup pass against the actual deployment.                                                                                                                              |
| Certificates                 | Separate staging/trusted IP+DNS issuance and certificate replacement pass. Injected validator failure leaves application pending; retry reloads nginx/relay even with unchanged certificate bytes. A subsequent unchanged renewal leaves the relay running. Six-hour renewal and daily backup timers are enabled. |

The pinned WebRTC native TURN verifier needs a DNS SAN; IP-only TLS was rejected
after the handshake. The room remains at the HTTPS IP origin. TURN temporarily
uses `144-217-161-239.sslip.io`, with a dedicated certificate containing both names.
This external DNS dependency is documented in [deployment](deployment.md) and
should be replaced with the operator's own direct DNS name when available.
Existing business sites and certificates are independent.

Actual deployment exposed Coturn's bandwidth reservation behavior: setting
`bps-capacity` equal to one allocation's `max-bps` denied subsequent ICE allocations
with 486. Aggregate reservation is disabled, per-allocation policing remains, and
allocation quotas are 8 per participant / 64 total. This does **not** provide a
host-wide bandwidth shaper.

## Controlled concurrent load

Five native hardware HEVC sources ran at **720p30 / 3 Mbps**, with five independent
browser contexts each explicitly watching four sources: **20 selected
subscriptions**. All twenty advanced decoded frames during the observation
interval. Source encoder samples were 30 fps, delivered sender samples about
29.9 fps, RTT 136–142 ms, zero reported loss and retransmits. A separate **1080p60
native TLS source and its viewer** were running concurrently for the long test;
resource figures below include that additional stream/subscription.

Ten samples over roughly 90 seconds on the shared Canada VPS:

| Service/measurement  | Observed peak                           |
| -------------------- | --------------------------------------- |
| App CPU / RAM        | 0.029 CPU core / 58 MiB                 |
| SFU CPU / RAM        | 0.508 CPU core / 93 MiB                 |
| TURN CPU / RAM       | 0.052 CPU core / 8 MiB                  |
| SFU outbound         | 69.9 Mbps                               |
| Shared host outbound | 69.8 Mbps (includes other host traffic) |

These CPU values convert Docker's percentage of **one core**; they are not a
percentage of the whole four-core host. Container caps stay at 0.5/1/0.5 CPU and
512/512/128 MiB for app/SFU/TURN. No server media decoding or thumbnail transcoding.
No OOM was observed. The test establishes concurrency at this workload, not
700 Mbps capacity or maximum-quality performance. Higher bitrates, packet rates,
TLS load and shared-host contention need a separate capacity test; the current
CPU limits may become the bottleneck before the network port.

## Remaining external acceptance

- An AMD Windows PC and an independent fresh Windows installation, including the
  missing-Visual-C++ prerequisite/UAC path.
- Real games, physical cameras, fullscreen/alt-tab, real Discord/group-call audio
  exclusion under game load, and glass-to-glass latency measurements.
- Physical 4K120 capture; the earlier synthetic GPU/decoder check remains separate.
- A full evening with four/five friends at their intended quality/subscriptions,
  including higher-rate CPU/network qualification on the shared VPS.
- An operator-chosen encrypted off-host backup destination and retention policy.
- macOS/Linux capture/audio/encoder backends, as the later portability plan states.

The private production-room invitation is saved only in the operator's ignored
local files, not this report or the repository. Automated success does not waive
the external checks above. The Windows publisher is currently unsigned; see
[distribution](distribution.md) for verification and installation details.
