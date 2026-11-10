# Local performance experiment ledger

**In progress — final configuration, long runs, acceptance and independent review remain.**

Felipe authorized this complete local pass on 2026-11-10, including local network
permission prompts and benchmark setup. Work is isolated on `perf/local-experiments`
in `freedomstream-perf`; unrelated changes in the original checkout are preserved.
The inventory below includes every optimization family from the
[performance research](../research/performance-opportunities.md) and
[footprint research](../research/resource-footprint.md), including negative results.
The research files describe earlier source snapshots; measurements here take
precedence over their hypotheses.

## Baseline and measurement meaning

The baseline is `93d6ee13d72898fa52b22305ff0f64e7ea05aea2`, the original head of
PR #19. It already releases the portable payload before waiting and converts
straight into final `ColorImage` storage. Those earlier improvements are in
[native-resource-footprint.md](native-resource-footprint.md) and are not counted
again here. The additional portable result below concerns **startup peak**.

The host is a Ryzen 7 9800X3D / RTX 5080 on Windows. The app uses Rust,
eframe/egui/Glow 0.37.0, the pinned LiveKit/WebRTC SDK and the existing bundled
GStreamer runtime. No UI framework replacement or additional native runtime is
included. Node 24, locked Rust release builds and the same synthetic publishers
are used for comparisons.

- CPU percentages use **one full CPU core = 100%**, rather than whole-machine
  normalization. CPU milliseconds are accumulated process CPU time.
- **Private bytes** measure process commit. **Working set** measures resident
  private/shared pages. Both concern memory; neither measures EXE size. GPU
  dedicated/shared memory and individual GPU engines are separate accounts.
- Decoded frames delivered to the Rust consumer, conversions, texture updates
  and UI invocations are counted separately. None establishes physical presented
  FPS, display-to-speaker latency or game frame times.
- Actual-app comparisons run isolated API/SFU services, real native publishers
  and the actual UI/controller. Publishers are separate processes; receiver CPU
  excludes their CPU. No microphone or personal screen is published. Monitor
  capture experiments discard their output instead of saving/transmitting it.
- Short comparisons alternate order and use phase interiors, excluding state
  transitions. Three-pair repeated HEVC cases complement single-pair codec and
  workload coverage. Longer steady and lifecycle checks remain pending below.
- Raw sanitized evidence and preserved binaries live under `.local/experiments/`.
  The compact committed aggregate and final artifact identities are pending.
  Invitations, cookies, credentials, raw SDK diagnostics and process command
  lines are excluded from reports.

## Complete inventory and current decisions

“Reject” means an experiment was attempted and the proposed product change is
not retained. A rejected experiment does not need further risky integration or
hardware acceptance to become a shipping change. Prototypes under the explicit
`performance-experiments` feature are excluded from the default app.

| ID  | Point tested                                                                                                      | Evidence and decision                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| --- | ----------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| R01 | GPU-resident receive/render interface; minimal native presenter, flexible surface lease and common rendering seam | Native D3D11 presentation passed three alternating pairs for each H.264/HEVC/AV1 at 1080p and 4K, 300 frames per run. C++/Rust lease and supported SDK-wrapper probes also ran. **Reject the full renderer/SDK bridge for this patch:** isolated gains do not qualify GL import, synchronization, adapter/device loss or deferred GUI painting. R19 retains a smaller receive change.                                                                                                                                                                                                               |
| R02 | Whole-app disconnected/empty/publish/watch/combined states, one/four streams, codecs, CPU/RAM/GPU and contention  | Repeated 1080p HEVC and H.264/AV1 hardware/software comparisons have run. **Measurement row, final matrix and long-run acceptance pending.** Failed or codec-changing 4K app attempts are recorded separately, not treated as controlled wins.                                                                                                                                                                                                                                                                                                                                                      |
| R03 | Presentation demand; skip invisible conversion while keeping subscription, audio and health                       | **Retain candidate.** The UI renews a 250 ms lease only for displayed pictures. Receive health advances before expensive conversion. Four streams continue receiving while focus/fullscreen/minimize reduce conversions; Stop explicitly unsubscribes. Final lifecycle/audio acceptance is pending.                                                                                                                                                                                                                                                                                                 |
| R04 | Bounded conversion workers and frame-driven repaint                                                               | Two blocking workers preserved received rate but increased stopped-state CPU/private memory during settling. Frame-driven repaint had no consistent gain after other wins and worsened some 60 FPS tails. **Reject both defaults.** Existing 16 ms watching/250 ms idle policy remains.                                                                                                                                                                                                                                                                                                             |
| R05 | Reuse owned CPU images; partial texture upload; bounded I420 pooling                                              | **Cache policy under final comparison.** Three retained RGBA images increased RAM; one image with frame dropping reduced updates and was rejected. Two images saved CPU. One cached image plus one bounded temporary image is being compared for lower RAM. Partial GL upload had no consistent benefit. I420 pooling looked fast in isolation but added RAM without a convincing whole-app gain, so it is rejected.                                                                                                                                                                                |
| R06 | Unchanged JPEG/profile work and immutable snapshot collections                                                    | **Retain** revision/source checks before JPEG cloning and unchanged profile checks before parsing. Snapshot deep clones cost about 2 microseconds for a deliberately large 32-source fixture; **reject** a broad snapshot ownership rewrite on this evidence. Preview capture cadence remains unchanged.                                                                                                                                                                                                                                                                                            |
| R07 | Fullscreen texture maintenance and unused appsink last-sample references                                          | Fullscreen cleanup already preceded the early return in this baseline; no new gain is claimed. CPU appsink tests removed a retained 3.1/12.4 MB sample reference without a measurable private-memory peak improvement. **GPU appsink comparison in progress; no default retention change accepted yet.**                                                                                                                                                                                                                                                                                            |
| R08 | Apply subscription intent only when it changes                                                                    | **Retain candidate.** Compare SDK desired state, including orphan audio. Four steady video publications previously caused about 8 setter calls/s; the candidate measured zero unchanged setters. These are SDK boundary calls, not counted network packets. Stop/rewatch/replacement/reconnect acceptance remains required.                                                                                                                                                                                                                                                                         |
| R09 | Stream portable verification/extraction from a bounded file range                                                 | **Retain candidate.** Five alternating cold/warm/concurrent pairs reduced wrapper startup peak commit from about 78.8–79.0 MiB to 1.3–1.5 MiB. Startup time/CPU did not consistently improve. Final packaged acceptance remains pending.                                                                                                                                                                                                                                                                                                                                                            |
| R10 | Audio gain/control handles, owned PCM transfer, scalar telemetry and single-root bypass                           | **Retain** gain-on-change, cached bus and safe owned little-endian PCM storage. Full PCM byte equivalence passed. Publishing scalars every ten frames did not improve its operation timing; **reject** it. The single-root bypass had no consistent CPU gain and removes silence/restart behavior, so it is rejected.                                                                                                                                                                                                                                                                               |
| R11 | Shared output/mixer and device-clock timing                                                                       | Four independent sinks versus shared output reduced threads from roughly 25 to 20 and about 0.4 MiB in the synthetic case, but CPU varied or worsened. Synchronization also added cost. **Reject the backend change:** independent gain/mute, stereo, hotplug, exclusion and physical A/V behavior would require additional qualification. No microphone-enabled SDK backend is introduced.                                                                                                                                                                                                         |
| R12 | Software-AV1 thread budgets and explicit Tokio workers                                                            | Stock dav1d was tested at 1/2/4/8/16 requested cores with one/four 1080p and 4K contexts. All expected frames decoded. Lower counts can save substantial CPU/threads in these fixtures. **Reject a new global policy for this patch:** varied-content/game deadlines and aggregate fairness are not established. Small Tokio counts also reduce threads; final actual-app comparison remains pending.                                                                                                                                                                                               |
| R13 | NVIDIA presets, DXGI/WGC, adapters and pacing                                                                     | Three-pair pipeline runs plus fixed-source quality checks ran. P1 did not show a consistent CPU advantage over existing P4; P7 cost more. Existing DXGI monitor capture used less memory than WGC on this host; WGC remains necessary for windows. Removing clocksync did not consistently help. **Retain existing defaults.** Only one physical adapter was available.                                                                                                                                                                                                                             |
| R14 | Compressed frame/age limits and keyframe-safe recovery                                                            | A 750 ms output stall accumulated about 44 outstanding frames. Resetting at eight lowered tail age, but an example delivered only 150/300 frames and reset twice. **Reject:** dropping pictures is not a valid CPU improvement. Existing byte guards and timestamp bounds remain.                                                                                                                                                                                                                                                                                                                   |
| R15 | Process/audio inventory caching and events                                                                        | A benchmark-only cache kept fresh process creation/access/parent/session checks and saved several ms per 500 ms sweep. **Reject for now:** modest return and incomplete rename/PID/protected-descendant equivalence do not justify capture-policy risk. WMI subscriptions failed with `CimException`, status 1; the cause was not established as access denial. No exclusion window or polling interval was weakened.                                                                                                                                                                               |
| R16 | SDK factory lifetime, ADM timer, logging and weak registry                                                        | **Retain expired weak-track pruning** under the existing lock. Five 10,000-track pairs reduced churn from about 588 to 156 ms. Reusing a whole capability factory retains unnecessary execution contexts; reject that cache. ADM gating removed idle callbacks but gave no CPU win; reject. Native logging filter microbench ran; real-app message-rate check remains pending.                                                                                                                                                                                                                      |
| R17 | Upload blocks, prepared/narrow queries and SFU lookup coalescing                                                  | **Retain** one immutable bounded upload block and three prepared statements, while rechecking current authentication on every chunk. Five 100,000-session pairs reduced query wall time from about 1,253 to 425 ms. Actual HTTP transfers preserved one lease/seven busy clients but showed no consistent throughput/combined-process CPU gain. A boolean-only scalar join is faster but does not implement the full session-return contract; reject a second authority API for a small per-transfer saving. **Reject SFU coalescing:** a controlled unpublish race reused a stale positive answer. |
| R18 | Direct NV12-to-owned-I420 conversion without the intermediate GStreamer I420 allocation/copy                      | Five pairs per one/four contexts and 1080p/4K passed whole sampled-frame checks. Single 1080p improved 17.5%, but four 1080p contexts regressed 11.7%; 4K cases improved. **Reject the fused-only variant.** R19 uses the conversion primitive with a different lifetime/scheduling path.                                                                                                                                                                                                                                                                                                           |
| R19 | Defer CPU I420 planes behind the SDK’s supported interface                                                        | **Retain candidate pending final acceptance.** An owned `GstSample` retains the GPU buffer, slice and device. The first plane read performs synchronized readback and one owned NV12-to-I420 conversion; subsequent reads share immutable pixels. Hidden frames can be released without readback. Concurrent reads, odd/padded planes, whole sampled frames after graph shutdown, initialized failure pixels and separate failure generations passed. Visible presentation still reads back, converts and uploads pixels.                                                                           |

## Measured tradeoffs and rejected improvements

### Rendering, allocation and receive path

The native presenter removed the CPU round trip in an isolated renderer. In the
GPU-counter HEVC runs, median 3D-engine utilization was roughly 2.83% to 0.49%
at 1080p and 9.73% to 0.49% at 4K. Decode-engine utilization did not uniformly
fall. These are separate driver engines, not a total GPU percentage or game
performance result. The renderer bridge remains unshipped.

The common Glow probe ran 24 actual eframe uploads: one/four textures,
1080p/4K and three alternating pairs. It verified synthetic texels and requested
frames. `glTexSubImage2D` did not consistently beat `glTexImage2D`; both transfer
the full picture in this experiment. Available interop extensions are not proof
that texture import was exercised.

The flexible C++/Rust lease proof showed retained buffers survive graph/frame
release and delay CPU mapping until requested. Decoder NV12 surfaces are not
simply shareable GL textures. Native enum/layout mismatches, synchronization and
null fallback hazards identified by the independent designs are avoided by R19’s
supported I420 interface. A GPUI app port was not benchmarked; the native render
stage was isolated instead. No framework speedup is claimed.

RGBA pools must not mutate images retained by egui texture deltas. Reuse uses
`Arc::get_mut`; live owners prevent it. Pools clear on hidden state and geometry
changes, while old external owners keep their pixels. The selected temporary-image
policy holds a weak reference to at most one extra live image, rather than
retaining another full allocation. Three matched pairs reduced median private
memory from 885.7 to 845.1 MiB and working set from 423.7 to 390.6 MiB versus
two cached images, at a CPU cost of 88.3% to 91.6% of one core. Median texture
updates changed 235.3 to 234.0/s, within the observed run variation. Final
baseline-versus-selected application and lifecycle checks remain required.

The I420 pool’s allocation/conversion operation test was much faster, but the
actual four-stream app median changed only about 73.9% to 72.1% of one core and
private memory rose from about 870.6 to 883.9 MiB. That does not justify adding
another retained pool. The rejected prototype is preserved for inspection.

R19’s standalone comparisons used six retained validation frames per context,
hashed in full after CPU/wall timing. The baseline retains CPU images, whereas
hidden deferred frames retain GPU samples until validation. Those memory peaks
are not equivalent ordinary app RAM measurements. Whole-app memory is reported
separately. One early raw summary’s phrase “inflate ... equally” is incorrect;
this paragraph and the corrected runner specify the actual limitation.

### Whole-app checkpoint, before final cache selection

Three alternating 1080p60 HEVC pairs, four watched publishers, source `4c77914`:

| State     | Baseline CPU, one-core % | Two-image candidate CPU, one-core % | Baseline private MiB | Candidate private MiB |
| --------- | -----------------------: | ----------------------------------: | -------------------: | --------------------: |
| Grid      |                   138.75 |                               78.95 |               837.77 |                860.90 |
| Minimized |                   106.69 |                               12.74 |               836.82 |                781.69 |

Both delivered about 240 decoded frames/s in aggregate. Grid texture updates
were approximately 235 and 236/s; minimized uploads were zero. This checkpoint
shows a substantial CPU benefit and a real active-memory tradeoff. It is not the
final selected configuration or a claim that all RAM measures decreased.

The one-image variant that simply skipped work lost updates, around 208–215/s
versus 237/s. It was rejected. One cached image with a bounded temporary image
reduced memory in later comparisons with some extra CPU cost; final selection
and long-run results remain pending.

### 4K capacity and failed attempts

Isolated 4K decode/presentation and complete sampled-plane equivalence passed.
Actual four-stream 4K UI tests did not establish 240 displayed/updated frames/s.
Some HEVC attempts changed to H.264, so they are invalid fixed-codec comparisons.
A forced-H.264 four-stream baseline attempt failed readiness. These failures are
retained as limitations, not silently discarded as successes.

A diagnostic receiver without egui presentation confirmed the native
`FreedomStream Direct3D11` hardware backend. In a 30-second four-4K HEVC run, the
candidate delivered about 240 frames/s in the final steady section; the original
receiver changed codec and delivered about 76/s. This is a capacity observation
with unequal delivered work, not an equal-work CPU percentage comparison.
A separate one-stream 4K H.264 diagnostic delivered 60/s through the same hardware
backend. Physical 4K120 remains unqualified.

One original 4K UI attempt wrote a completed phase report but failed child-exit
validation. A repeat exited normally. The first attempt is excluded; the runner
now preserves numeric exit/timeout and sanitized failure classifications if it
recurs. No root cause is claimed from that single failure.

### Portable, audio, control and server

The streamed ZIP reader hashes 64 KiB chunks and confines seeks/reads to the
verified payload. It preserves digest/footer/path/count/expanded-size/cache
checks, extraction locking, completion rules and child/job supervision. Windows
keeps one handle denying writers/deletion between verification and extraction.
Non-Windows uses a private anonymous verified snapshot because ordinary file
locks are advisory; that platform’s added test has not been run on this host.
Waiting memory was already small before this pass. The roughly 77.5 MiB new
saving is startup peak, not a second waiting-memory saving.

The audio scalar test preserved all sample peaks: 20,000 frames took about
13.23 ms with per-frame publication and 13.47 ms with publication every ten
frames. No benefit was found. Gain/control and owned-PCM changes are small safe
operation wins, not measured multi-percent whole-app CPU reductions. Queue
limits, per-track gain/mute, stereo and error polling are retained.

Pruning expired SDK weak entries does not remove live wrappers. The stress test
created/dropped 10,000 real SDK tracks on one factory per child; no frames were
submitted. Median final-500-track time fell from about 46.8 to 7.8 ms. This
removes metadata/search growth; it does not establish that the old registry
retained video frames or decoders.

The server query microbenchmark uses fresh token hashes, room/session reads and
current expiry on every call. Immediate revocation, expiry and room separation
passed. The real HTTP comparison preserved bounded paced transfers and global
lease behavior. Its CPU includes the synthetic fetch client and transferred
amounts varied under Windows timer scheduling, so it is not an isolated server
CPU or production throughput result. No native-app RAM saving is attributed to
server changes.

## Not executable as hardware/deployment qualifications tonight

| ID  | Gate                                                                   | Treatment                                                                                        |
| --- | ---------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| X01 | AMD AMF/AV1, RX 7600 and the reported Siege capture/stutter case       | No AMD hardware supplied. Keep separate; NVIDIA evidence does not qualify it.                    |
| X02 | Physical 4K120 source/display and other GPU generations                | Synthetic 4K experiments are labelled individually. No physical 4K120 or cross-generation claim. |
| X03 | Fresh physical PCs, other OS backends and friends/group field sessions | Local isolated cache/PATH/offline tests only; these cannot certify another PC.                   |
| X04 | Production WAN/VPS capacity, deployment and field latency              | Local API/SFU experiments only.                                                                  |
| X05 | Physical display-to-wired-speaker A/V offset                           | Pipeline timestamps/queues are distinct from physical output timing; no offset fix is claimed.   |

Runtime/DLL removal remains conditional on import closure and all codec,
capture/audio and vendor distribution checks. The runtime is already curated.
A loaded-module inventory ran, but virtual image size is not resident/private
RAM; an unloaded plugin is not an established steady-RAM saving. No unqualified
runtime pruning, heap-limit reduction, forced working-set trim, EXE compression,
quality reduction, hidden unsubscribe or microphone acquisition is included.

## Reproduction and final acceptance

The following checked-in runners reproduce individual families:

- `prepare-performance-baseline.mjs` exports the original source and verifies the
  restored original SDK tree; it adds shared measurement instrumentation only.
- `local-performance-room.mjs` and `run-app-performance-matrix.mjs` drive actual
  isolated UI/SFU workloads. `sample-native-process.ps1` collects only the owned
  receiver’s counters and restores its minimized window.
- `build-decoder-experiments.ps1`, `benchmark-decoder-resources.ps1`,
  `benchmark-fused-receive.ps1` and `benchmark-i420-pool.ps1` exercise actual pinned
  decoder/conversion interfaces. `-Size 4k -SoftwareOnly` selects the 4K dav1d matrix.
- `benchmark-gpu-counters.ps1`, `benchmark-synthetic-pipelines.ps1` and
  `benchmark-nvidia-quality.mjs` cover GPU, pipeline and short fixed-source quality
  cases. The 30-frame PSNR check uses the same bitrate target, not identical
  output bytes or a perceptual/game quality qualification.
- `benchmark-portable-startup.ps1`, `benchmark-sdk-churn.ps1`,
  `benchmark-room-queries.mjs`, `benchmark-session-validity.mjs` and
  `benchmark-server-http.mjs` cover the remaining measured paths.
- `summarize-local-performance.mjs` and `summarize-performance-pass.mjs` produce
  phase summaries and compact sanitized evidence with hashes.

**Still required before the final PR update:** finalize cache/last-sample choices;
fix/verify stale playback-generation isolation; finish lifecycle/audio/contention
and explicit-worker comparisons; run long steady and repeated lifecycle checks;
run Node/Rust checks, actual native codec/audio/reconnect/preview acceptance and
packaged qualification; commit the aggregate; obtain the required fresh
GPT-6.1-Sol high review of the exact final range. No completed PR or acceptance
claim is made by this draft.
