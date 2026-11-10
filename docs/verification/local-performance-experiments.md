# Local performance experiment ledger

Started 2026-11-10. Felipe authorized the complete local experiment pass, local
network permission dialogs and the work needed to compare patches fairly. This
file tracks every outstanding point from the previous explanation. Work is on
`perf/local-experiments`, initially based on `93d6ee1` (PR #19). Existing work in
the original checkout is preserved. The earlier archive-release/direct-image
changes are the starting baseline, not new gains to count twice.

## Execution and retention rules

- A row is finished only with a reproducible run/evidence reference and a
  decision: **retain**, **reject/no measurable return**, **reject/risk**, or
  **external gate**. Compilation, inspection and proposed benchmarks are not
  performance results. Pending rows remain pending across sessions.
- Preserve baseline/candidate source identities, release binaries, runtime pins
  and raw sanitized measurements under `.local/experiments/`. Commit compact
  aggregates/reproduction details. Run matched workloads in alternating order;
  avoid concurrent controlled benchmarks/builds. Separate microbenchmarks,
  pipeline experiments and real application/SFU measurements.
- Record CPU time, private bytes, working set, threads/handles, applicable GPU
  engine/VRAM counters, delivered/converted/presented frames, queue age/high-water
  and tail timing. Report unsupported counters honestly. Use deterministic moving
  and static fixtures, one/four subscriptions, publication plus watching, grid,
  focus/fullscreen, minimize/restore and lifecycle/recovery cases where relevant.
- Retain improvements only after their intended metric improves repeatably and
  correctness/security checks pass. Record useful but risky experiments without
  enabling them by default. A framework rewrite is not a prerequisite.
- Preserve explicit Watch/Stop traffic, audio gain/mute, no microphone,
  Discord/own-playback exclusion, fail-closed process/session checks, explicit
  geometry/FPS/bitrate, one publication and start/60-second preview capture.
- Run helpers hidden/noninteractive. No invitation/session/media credentials,
  raw SDP, process command lines or captured personal media in logs or artifacts.
  Local generated room credentials/databases stay ignored. No production/VPS
  changes are required for the local comparisons.

## Complete experiment queue

All rows below are authorized for local attempts. Sub-items are included so a
small successful change cannot silently stand in for the whole finding.

| ID  | Point and required local attempts                                                                                                                                                                                                      | Status                                               | Evidence / decision                       |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- | ----------------------------------------- |
| R01 | Keep received video on the GPU: compare D3D11 presentation against CPU readback/upload; exercise an owned surface lease, existing UI interop feasibility, decoder/SDK integration and recovery risk                                    | In progress: three independent module designs/probes | Pending                                   |
| R02 | Whole-application measurements: idle/room, publishing, one/four actual subscriptions, combined load, UI states, process/GPU memory and CPU/engine contention; attempt repeatable game load or explicitly labelled synthetic contention | Pending                                              | Pending                                   |
| R03 | Convert only pictures with presentation demand, retaining subscriptions/audio and updating receive health before gating; restore/fallback behavior                                                                                     | Pending                                              | Pending                                   |
| R04 | Bounded conversion workers and frame-driven/coalesced repaint; fairness, cancellation, stale generations and control responsiveness                                                                                                    | Pending                                              | Pending                                   |
| R05 | Image/texture reuse: same-size partial uploads, bounded owned CPU image reuse and I420 pool experiments; resize and retention cost                                                                                                     | In progress: native OpenGL comparison probe          | Pending                                   |
| R06 | Unchanged preview/snapshot/profile work: check JPEG revision before copying, share immutable snapshot collections, avoid repeated profile parsing and test allocation impact                                                           | Pending                                              | Pending                                   |
| R07 | Stale fullscreen texture/cache cleanup and unused appsink last-sample retention; stop/leave/restore, preroll and pool high-water                                                                                                       | Pending                                              | Pending                                   |
| R08 | Coalesce duplicate subscription setters by publication/recovery generation; test pending subscribe, retries, Stop/rewatch, unpublish and reconnect                                                                                     | Pending                                              | Pending                                   |
| R09 | Stream portable archive verification/extraction through a bounded seekable range; cold/warm launch, peaks, concurrent launches, corruption/path/cache/job checks                                                                       | Pending                                              | Pending                                   |
| R10 | Audio small changes: gain only on change, lower-rate scalar telemetry, cached control handles, safe owned sample transfer/reuse and optional single-root mixer bypass                                                                  | Pending                                              | Pending                                   |
| R11 | Shared audio output/device timing prototype: independent gain/mute, queue/device-clock accounting, stereo, exclusion, hotplug/reconnect; physical timing remains separately gated                                                      | Pending                                              | Pending                                   |
| R12 | Aggregate software-AV1/conversion CPU budgets and explicit Tokio worker counts; one/four software decode contexts, contention, deadlines and queue/tail effects                                                                        | Pending                                              | Pending                                   |
| R13 | NVIDIA presets, DXGI/WGC capture, adapter choice and pacing/reconfiguration; fixed geometry/FPS/bitrate and image-quality checks                                                                                                       | Pending (local NVIDIA portion)                       | AMD-specific qualification excluded below |
| R14 | Decoder frame/age bounds with keyframe-safe recovery; controlled stalls/loss, outstanding count, true wall-clock queue age and reference/header correctness                                                                            | Pending                                              | Pending                                   |
| R15 | Process/audio inventory cost and event-assisted identity cache prototype; restart/PID reuse/denied access/protected descendants; no weaker exclusion window                                                                            | Pending                                              | Pending                                   |
| R16 | SDK startup/control/logging churn: capability-query factory lifetime/cache, synthetic ADM wakeups, verbose bridge formatting, weak-track registry metadata and lifecycle soak                                                          | Pending                                              | Pending                                   |
| R17 | Local server improvements: reusable upload chunks, prepared session statements/narrow equivalent queries, coalesced fresh SFU lookups; measure allocation/query/event-loop load and preserve revocation/cancellation semantics         | Pending                                              | Pending                                   |

Runtime/DLL pruning is a conditional packaging experiment only if loaded-memory
evidence identifies unused shipped code; compressed file size is not RAM use.
GPUI is an architectural alternative, not an assumed improvement; a native GPU
presenter will be tested without requiring a UI rewrite.

## Unavailable hardware and external gates — not executed as qualifications tonight

| ID  | Gate                                                                                                | Local work still permitted                                                                                             |
| --- | --------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| X01 | Actual AMD AMF/AV1 and RX 7600/Siege whole-monitor 1080p120/25 Mbps result: no AMD machine supplied | NVIDIA tests and source/contract checks; never label them AMD qualification                                            |
| X02 | Physical 4K120 source/display and other GPU generations                                             | Synthetic 4K/120 throughput with explicit source/display limitations                                                   |
| X03 | Fresh physical PC, other OS backends and friends/group field use                                    | Isolated cache/PATH/offline-launch attempts on this Windows installation                                               |
| X04 | Production WAN/VPS capacity and deployment/field latency                                            | Isolated local API/SFU/server load; no claim of production capacity                                                    |
| X05 | Physical display-to-wired-speaker A/V offset without an independent measurement setup               | Flash/click source timestamps, pipeline/device queue timing and synchronization regression probes; no substitute claim |

## Baseline and run record

- Initial app source: `93d6ee13d72898fa52b22305ff0f64e7ea05aea2`.
- Host supplied/previously identified: Ryzen 7 9800X3D / RTX 5080, Windows;
  current counter/driver/display inventory will be recorded with the runs.
- Baseline already includes the measured PR #19 changes. Its benchmarks and
  acceptance are in [the first footprint report](native-resource-footprint.md).
- Preserved baseline, instrumentation identity and each experiment's results:
  pending. No new performance outcome is claimed by this initial ledger.

## Final retained changes and rejected experiments

Pending execution. This section must list the final applied patch set, independent
review/acceptance, measurable returns, negative results and beneficial experiments
left disabled because their risks or missing acceptance outweigh the return.
