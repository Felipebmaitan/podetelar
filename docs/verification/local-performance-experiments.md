# Local performance experiment ledger

**Local experiment matrix completed. Final packaged acceptance and evidence recheck remain.**

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
are used for comparisons. The recorded host is Windows 11 Pro build 26300,
NVIDIA driver 32.0.16.2114, eight physical/sixteen logical CPU cores and the
High performance power scheme. A Parsec virtual adapter is also installed.
This configuration snapshot is not a fixed-clock/thermal qualification.

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
  workload coverage. Longer steady and lifecycle checks are reported below.
- The summary scripts select the upper middle observation for even sample
  counts. Two-pair 4K results also retain their individual values and ranges;
  they are not presented as a repeated three-pair estimate.
- Raw sanitized evidence and preserved binaries live under `.local/experiments/`.
  The committed [sanitized aggregate](local-performance-results.json) preserves individual results, artifact identities and evidence hashes.
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
| R02 | Whole-app disconnected/empty/publish/watch/combined states, one/four streams, codecs, CPU/RAM/GPU and contention  | Final short matrix, sustained publish/one/four-stream checks and seven-cycle before/after lifecycle runs completed. Coverage includes hardware H.264/HEVC/AV1, forced software AV1, audio and synthetic GPU contention. Invalid helpers and codec-changing attempts are excluded. Four-stream 4K remains capacity-limited; see the separate results.                                                                                                                                                                                                                                                |
| R03 | Presentation demand; skip invisible conversion while keeping subscription, audio and health                       | **Retain.** The UI renews a 250 ms lease for displayed pictures, while receive health advances before conversion. Focus/fullscreen/minimize preserve subscriptions and audio; Stop unsubscribes. Native audio/codec/reconnect acceptance and both seven-cycle lifecycle runs passed.                                                                                                                                                                                                                                                                                                                |
| R04 | Bounded conversion workers and frame-driven repaint                                                               | Two blocking workers preserved received rate but increased stopped-state CPU/private memory during settling. Frame-driven repaint had no consistent gain after other wins and worsened some 60 FPS tails. **Reject both defaults.** Existing 16 ms watching/250 ms idle policy remains.                                                                                                                                                                                                                                                                                                             |
| R05 | Reuse owned CPU images; partial texture upload; bounded I420 pooling                                              | **Retain two reusable RGBA images.** Exclusive mutation preserves egui-owned pixels. Three images added RAM; one cached image with dropping lost updates. A bounded temporary image saved RAM but lost updates in the final combined configuration. Partial GL uploads gave no consistent return. I420 pooling added RAM without convincing whole-app CPU savings; reject those variants.                                                                                                                                                                                                           |
| R06 | Unchanged JPEG/profile work and immutable snapshot collections                                                    | **Retain** revision/source checks before JPEG cloning and unchanged profile checks before parsing. Snapshot deep clones cost about 2 microseconds for a deliberately large 32-source fixture; **reject** a broad snapshot ownership rewrite on this evidence. Preview capture cadence remains unchanged.                                                                                                                                                                                                                                                                                            |
| R07 | Fullscreen texture maintenance and unused appsink last-sample references                                          | Fullscreen cleanup already preceded the early return in the baseline; no new gain. **Retain unused last-sample disabling** on native decoder appsinks after three matched four-stream pairs and codec acceptance. CPU improved; working set did not uniformly fall. Removing a sample reference is not equivalent to freeing an entire surface.                                                                                                                                                                                                                                                     |
| R08 | Apply subscription intent only when it changes                                                                    | **Retain.** Compare the SDK's desired state, including orphan audio. Four steady publications previously caused about eight setter calls/s; unchanged candidate setters measured zero. These are SDK calls, not counted network packets. Explicit Stop/rewatch, codec fallback and full reconnect passed actual native acceptance.                                                                                                                                                                                                                                                                  |
| R09 | Stream portable verification/extraction from a bounded file range                                                 | **Retain candidate.** Five alternating cold/warm/concurrent pairs reduced wrapper startup peak commit from about 78.8–79.0 MiB to 1.3–1.5 MiB. Startup time/CPU did not consistently improve. Final packaged acceptance remains pending.                                                                                                                                                                                                                                                                                                                                                            |
| R10 | Audio gain/control handles, owned PCM transfer, scalar telemetry and single-root bypass                           | **Retain** gain-on-change, cached bus and safe owned little-endian PCM storage. Full PCM byte equivalence passed. Publishing scalars every ten frames did not improve its operation timing; **reject** it. The single-root bypass had no consistent CPU gain and removes silence/restart behavior, so it is rejected.                                                                                                                                                                                                                                                                               |
| R11 | Shared output/mixer and device-clock timing                                                                       | Four independent sinks versus shared output reduced threads from roughly 25 to 20 and about 0.4 MiB in the synthetic case, but CPU varied or worsened. Synchronization also added cost. **Reject the backend change:** independent gain/mute, stereo, hotplug, exclusion and physical A/V behavior would require additional qualification. No microphone-enabled SDK backend is introduced.                                                                                                                                                                                                         |
| R12 | Software-AV1 thread budgets and explicit Tokio workers                                                            | Stock dav1d was tested at 1/2/4/8/16 requested cores with one/four 1080p and 4K contexts; all expected frames decoded. Lower counts can save substantial fixture CPU/threads, but varied-content/game deadlines and aggregate fairness remain unqualified: **reject a global decoder cap.** Three actual-app Tokio pairs reduced threads 121 to 110 but gave no meaningful private-memory/CPU win; retain the host default.                                                                                                                                                                         |
| R13 | NVIDIA presets, DXGI/WGC, adapters and pacing                                                                     | Three-pair pipeline runs plus fixed-source quality checks ran. P1 did not show a consistent CPU advantage over existing P4; P7 cost more. Existing DXGI monitor capture used less memory than WGC on this host; WGC remains necessary for windows. Removing clocksync did not consistently help. **Retain existing defaults.** Only one physical adapter was available.                                                                                                                                                                                                                             |
| R14 | Compressed frame/age limits and keyframe-safe recovery                                                            | A 750 ms output stall accumulated about 44 outstanding frames. Resetting at eight lowered tail age, but an example delivered only 150/300 frames and reset twice. **Reject:** dropping pictures is not a valid CPU improvement. Existing byte guards and timestamp bounds remain.                                                                                                                                                                                                                                                                                                                   |
| R15 | Process/audio inventory caching and events                                                                        | A benchmark-only cache kept fresh process creation/access/parent/session checks and saved several ms per 500 ms sweep. **Reject for now:** modest return and incomplete rename/PID/protected-descendant equivalence do not justify capture-policy risk. WMI subscriptions failed with `CimException`, status 1; the cause was not established as access denial. No exclusion window or polling interval was weakened.                                                                                                                                                                               |
| R16 | SDK factory lifetime, ADM timer, logging and weak registry                                                        | **Retain expired weak-track pruning** under the existing lock. Five 10,000-track pairs reduced churn about 588 to 156 ms. Reusing a whole capability factory retains execution contexts; reject. ADM gating removed idle callbacks without CPU benefit; reject. Logging microbench and real-app count ran (about 129 messages/s in grid); no actual-app logging-policy CPU win established, so preserve diagnostics.                                                                                                                                                                                |
| R17 | Upload blocks, prepared/narrow queries and SFU lookup coalescing                                                  | **Retain** one immutable bounded upload block and three prepared statements, while rechecking current authentication on every chunk. Five 100,000-session pairs reduced query wall time from about 1,253 to 425 ms. Actual HTTP transfers preserved one lease/seven busy clients but showed no consistent throughput/combined-process CPU gain. A boolean-only scalar join is faster but does not implement the full session-return contract; reject a second authority API for a small per-transfer saving. **Reject SFU coalescing:** a controlled unpublish race reused a stale positive answer. |
| R18 | Direct NV12-to-owned-I420 conversion without the intermediate GStreamer I420 allocation/copy                      | Five pairs per one/four contexts and 1080p/4K passed whole sampled-frame checks. Single 1080p improved 17.5%, but four 1080p contexts regressed 11.7%; 4K cases improved. **Reject the fused-only variant.** R19 uses the conversion primitive with a different lifetime/scheduling path.                                                                                                                                                                                                                                                                                                           |
| R19 | Defer CPU I420 planes behind the SDK’s supported interface                                                        | **Retain with R20.** Owned GstSamples retain GPU storage until supported I420 planes are read. One synchronized materialization produces immutable owned pixels; hidden frames avoid readback. Concurrency, initialized failure output, old-generation isolation, sampled whole-plane equivalence, native codec/audio/reconnect acceptance, sustained 1080p and repeated lifecycle checks passed. Visible playback still reads back, converts and uploads. One-stream/4K memory costs and four-4K capacity limits remain explicit.                                                                  |

## Measured tradeoffs and rejected improvements

The application matrix revealed one additional receive-path point:

| ID  | Point tested                                      | Evidence and decision                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| --- | ------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| R20 | Bound CPU-readback staging per decoder generation | **Retain bounded staging with generic fallback.** Both 48-run prototypes preserved expected frames and sampled whole-plane hashes. The first increased four-decoder CPU; the revision narrows the device lock. A matched app pair saved about 62 MiB private memory with four 1080p streams at modest CPU cost versus the prior candidate. One 4K pair saved about 203 MiB but delivered unequal work. Final native acceptance, sustained 1080p and lifecycle runs passed. Staging releases on shutdown and generations remain isolated; original-baseline 4K tradeoffs are reported separately. |

### Positive results that are deliberately not shipped

| Experiment                             | Local benefit                                                                                                                                     | Why it is not selected                                                                                                                                                                      |
| -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Native D3D11 presentation              | HEVC renderer 3D-engine median about 2.83% to 0.49% at 1080p; 9.73% to 0.49% at 4K                                                                | This isolates rendering. Import into the real deferred Glow UI, synchronization, adapter changes and device loss remain unqualified. R19 takes the smaller supported-interface improvement. |
| Fewer software AV1 decoder threads     | Four 4K contexts: requested 1 versus 16 cores used about 4,969 versus 15,156 CPU ms and 11 versus 75 process threads; all expected frames decoded | These fixed fixtures do not establish an aggregate policy for varied content and competing games. A global cap can miss deadlines.                                                          |
| Shared audio output                    | About five fewer threads and 0.4 MiB less private memory in the synthetic four-track case                                                         | CPU did not improve consistently; a new mixer/device clock adds gain, hotplug and timing risks.                                                                                             |
| Process image cache                    | Twenty sweeps about 266 to 168 ms, with fresh process/access/session enumeration                                                                  | Incomplete rename/PID/protected-descendant equivalence risks capture exclusions for a modest steady CPU saving.                                                                             |
| Narrow session-validity query          | 100,000 prepared full-session reads about 425 ms versus scalar join 170 ms                                                                        | The scalar does not return the required session snapshot. A second authority contract adds complexity for a small per-transfer saving.                                                      |
| SFU request coalescing                 | Sixteen simultaneous requests used one RPC instead of sixteen                                                                                     | A controlled unpublish race returned a stale positive. The freshness failure rules it out despite lower latency.                                                                            |
| Bounded I420 pool                      | Allocation/conversion CPU improved strongly in isolation                                                                                          | Whole-app CPU was nearly unchanged and private memory rose. Another retained pool is not worthwhile.                                                                                        |
| One cached RGBA image plus a temporary | Initially about 41 MiB less private memory than two retained images                                                                               | The final combined configuration lost texture updates in repeated runs; the same-binary two-image control restored them.                                                                    |

### Rendering, allocation and receive path

The native presenter removed the CPU round trip in an isolated renderer. In the
GPU-counter HEVC runs, median 3D-engine utilization was roughly 2.83% to 0.49%
at 1080p and 9.73% to 0.49% at 4K. Decode-engine utilization did not uniformly
fall. These are separate driver engines, not a total GPU percentage or game
performance result. The renderer bridge remains unshipped.

The separate HEVC GPU-counter experiment also measured private memory
238.67 to 185.20 MiB and dedicated GPU memory 67.02 to 50.05 MiB at 1080p.
At 4K those were 487.98 to 263.30 MiB and 178.68 to 114.69 MiB. These compare
an isolated CPU-round-trip presenter with a native D3D11 presenter, not two
complete application builds. They show why a future renderer bridge is worth
investigating, while its integration risks still prevent selecting it here.

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
changes, while old external owners keep their pixels. The rejected temporary-image
policy holds a weak reference to at most one extra live image, rather than
retaining another full allocation. Three matched pairs reduced median private
memory from 885.7 to 845.1 MiB and working set from 423.7 to 390.6 MiB versus
two cached images, at a CPU cost of 88.3% to 91.6% of one core. Median texture
updates initially changed 235.3 to 234.0/s, within the observed run variation.
However, two final-configuration runs reduced updates to 224.0 and 225.6/s.
A two-image control using the exact same executable restored 236.9/s. The final
default therefore keeps two reusable images; the temporary-image experiment
remains feature-only. Lower RAM alone did not justify this update-rate loss.

The I420 pool’s allocation/conversion operation test was much faster, but the
actual four-stream app median changed only about 73.9% to 72.1% of one core and
private memory rose from about 870.6 to 883.9 MiB. That does not justify adding
another retained pool. The rejected prototype is preserved for inspection.

Disabling unused `enable-last-sample` on the three native decoder appsinks
passed three alternating four-stream application pairs with the same Rust
harness and two-image policy. Median grid CPU fell from 102.7% to 72.8% of one
core; texture updates were 236.2/s and 236.6/s. Private memory was 879.7 and
865.4 MiB, but working set rose from 421.1 to 460.8 MiB. This is a repeatable
CPU result, not evidence that removing the sample reference uniformly frees
resident RAM. The selected final configuration combines this change with the
two-image cache. Its final short matrix and codec acceptance passed.

R19’s standalone comparisons used six retained validation frames per context,
hashed in full after CPU/wall timing. The baseline retains CPU images, whereas
hidden deferred frames retain GPU samples until validation. Those memory peaks
are not equivalent ordinary app RAM measurements. Whole-app memory is reported
separately. One early raw summary’s phrase “inflate ... equally” is incorrect;
this paragraph and the corrected runner specify the actual limitation.

In that isolated deferred-I420 matrix, median process CPU milliseconds for
the same requested frame count were:

| Geometry / contexts | Visible before → deferred | Hidden before → deferred |
| ------------------- | ------------------------: | -----------------------: |
| 1080p / one         |           1,062.5 → 937.5 |            984.4 → 484.4 |
| 1080p / four        |         5,484.4 → 3,718.8 |          4,671.9 → 734.4 |
| 4K / one            |         2,437.5 → 2,093.8 |          2,406.3 → 453.1 |
| 4K / four           |        14,843.8 → 9,828.1 |         15,031.2 → 781.3 |

Each context decoded 300 frames; three alternating pairs covered each case.
All expected counts and sampled whole-plane hashes matched. These numbers
precede bounded staging and isolate R19, rather than predicting final app CPU.

### Selected whole-app results

Three alternating 1080p60 HEVC pairs, four watched publishers, native source
`000129f9153a560a38030d77f06e8a1844dbed99`, labels `v4-final-four-*`.
Values are medians of each run's phase interior. Other-codec coverage is
reported separately below; this table does not substitute for long-run checks.

| State      | Baseline CPU, one-core % | Selected CPU, one-core % | Baseline private MiB | Selected private MiB | Baseline working set MiB | Selected working set MiB |
| ---------- | -----------------------: | -----------------------: | -------------------: | -------------------: | -----------------------: | -----------------------: |
| Grid       |                   129.48 |                    75.20 |               839.86 |               806.81 |                   376.39 |                   410.48 |
| Focus      |                   111.83 |                    37.86 |               842.76 |               757.11 |                   379.43 |                   360.43 |
| Fullscreen |                   104.19 |                    31.04 |               832.46 |               742.57 |                   382.48 |                   354.35 |
| Minimized  |                   103.67 |                    18.46 |               835.86 |               726.92 |                   384.23 |                   339.91 |
| Restored   |                   134.15 |                    78.82 |               838.45 |               810.31 |                   378.51 |                   412.55 |
| Stopped    |                     0.69 |                     0.93 |               265.86 |               266.76 |                   180.15 |                   185.31 |

Grid decode rates were 239.59 and 239.88 frames/s;
texture updates were 233.85 and 236.03/s.
Minimized decode continued at 239.42 and 240.08/s while uploads
were zero. All six runs retained HEVC and reported no sampled UI/stream errors.
Focus/fullscreen presented one picture while all four remained watched.

Grid private memory fell about 33.0 MiB, while resident working set rose about
34.1 MiB. Minimized private memory fell about 108.9 MiB and working set about
44.3 MiB. This is a substantial CPU and private-memory improvement with a
remaining active-residency tradeoff. It is not a claim that every RAM measure
decreased. Individual run variation is preserved in the aggregate.

The longer `v4-retry-steady-four-*` pair extended the initial grid by 360
seconds and excluded its first minute, leaving about five minutes with 273/274
OS samples. CPU was 128.41% to 76.46% of one core; private memory 853.47 to
828.10 MiB and working set 386.28 to 423.80 MiB. Decoded frames were 239.69 to
240.00/s and texture updates 235.77 to 236.58/s. Both retained HEVC with no
sampled errors. UI invocation p99 was about 17 ms in both. This supports the
CPU gain and confirms the active-residency cost over the longer interval.

The default publisher EXE changed from 38,850,048 to 38,871,040 bytes,
an increase of 20,992 bytes. No EXE-size saving is claimed.

### One-stream tradeoff

For one watched 1080p60 HEVC publisher, three `v4-final-one-*` pairs measured
grid CPU 40.94% to 27.69% of one core, and minimized CPU 37.94% to 3.71%.
Grid decode rates were 59.85 and 59.97/s; texture updates were 58.17 and 56.96/s.
Active private memory rose 360.93 to 376.45 MiB, and working set 213.37 to
244.06 MiB. Minimized private memory was essentially flat at 351.45/351.40 MiB,
while working set rose 215.72 to 230.59 MiB. The four-stream memory gains must
not be extrapolated to one stream.

The longer one-stream comparison extended the initial grid by 180 seconds and
excluded its first minute. Both retained HEVC and delivered about 60 frames/s.
Texture updates settled at 58.79/s before and 58.71/s after, so the short-run
update gap did not persist. CPU was 36.59% to 27.08% of one core; private memory
361.50 to 382.10 MiB and working set 215.09 to 246.91 MiB. The CPU gain and
single-stream memory tradeoff both remain. Neither run reported sampled errors.

### GPU counters in the real app

The additional four-stream 1080p60 codec cases used one matched pair each:

| Codec/backend  | Grid CPU before → after, one-core % | Private MiB before → after | Decoded frames/s before → after | Texture updates/s before → after |
| -------------- | ----------------------------------: | -------------------------: | ------------------------------: | -------------------------------: |
| H.264 hardware |                      129.44 → 58.80 |            779.77 → 847.89 |                 239.94 → 239.82 |                  236.07 → 235.81 |
| AV1 hardware   |                      127.25 → 76.64 |            826.39 → 824.61 |                 239.60 → 240.03 |                  235.85 → 234.92 |
| AV1 software   |                     133.45 → 101.19 |            349.34 → 410.60 |                 239.42 → 239.93 |                  234.99 → 235.91 |

All retained the requested codec and reported no sampled UI/stream errors.
These are coverage pairs, not repeated estimates. H.264 and software AV1
show CPU savings with higher active private memory; HEVC's memory result
must not be generalized to every codec.

The same three-pair four-stream 1080p comparisons recorded these separate GPU
engine medians. These are receiver-process driver counters, not total-card
percentages, game frame times or fixed-clock hardware work measurements.

| State     | 3D engine before → after, % | Decode engine before → after, % | Dedicated GPU MiB before → after |
| --------- | --------------------------: | ------------------------------: | -------------------------------: |
| Grid      |               29.20 → 23.03 |                   11.82 → 13.15 |                  291.14 → 288.77 |
| Focus     |                12.26 → 5.33 |                   16.17 → 18.14 |                  291.12 → 288.75 |
| Minimized |                 6.54 → 0.00 |                   18.52 → 20.83 |                  313.56 → 311.19 |

The copy-engine counter was zero in both variants. That does not prove that no
copies occurred: driver work can be accounted to another engine. Rendering and
readback-related 3D activity fell, while decode utilization rose and dedicated
memory was nearly unchanged. No uniform total-GPU or VRAM saving is claimed.

### 4K capacity and failed attempts

The earlier `v2-final-*` matrix exposed an active one-stream 4K private-memory
increase of roughly 300 MiB. R20 was added in response. Both 48-run staging
prototypes preserved expected counts and sampled whole-plane hashes. The first
increased four-decoder CPU. Narrowing the device lock to graphics API calls
improved its four-4K result, while a matched application pair reduced private
memory about 62 MiB with four 1080p streams and about 203 MiB with one 4K stream
against that earlier candidate. Its final generic fallback passed helper tests
and the actual native codec/audio/reconnect suite.

Two final `v4-final-4k-one-*` pairs still show a 4K memory tradeoff against the
original baseline. Baseline private memory ranged 665.5–681.5 MiB versus
741.1–838.8 MiB selected; working set was 328.5–340.4 versus 419.6–491.1 MiB.
CPU was 93.5–105.4% versus 45.7–50.1% of one core, but delivered frames were
59.94–60.18/s versus 55.27–57.71/s and updates 50.80–52.57/s versus
48.63–51.19/s. This is **not an equal-work CPU speedup**. The texture-byte
counter established 3840×2160 images; it does not establish physical 4K60
presentation. Both pairs retained HEVC and reported no sampled errors.

Isolated 4K decode/presentation and complete sampled-plane equivalence passed.
Actual four-stream 4K UI tests did not establish 240 displayed/updated frames/s.
Some HEVC attempts changed to H.264, so they are invalid fixed-codec comparisons.
A forced-H.264 four-stream baseline attempt failed readiness. These failures are
retained as limitations, not silently discarded as successes.

The final `v4-retry-final-4k-four-capacity` UI run retained HEVC without sampled
errors, but grid delivered only 181.98 decoded frames/s and 108.71 texture
updates/s. CPU was 499.74% of one core, private memory 2,678.57 MiB and working
set 1,456.21 MiB. Focus/fullscreen/minimize kept decoded rate near 240/s while
showing at most one picture. This is a demonstrated presentation-path capacity
limit, not a four-stream 4K qualification.

The frozen default publisher's separate 30-second four-4K diagnostic confirmed
all four `FreedomStream Direct3D11` hardware backends and HEVC/3840×2160 images.
Its final ten samples (seconds 20–29, after connection/admission startup) delivered
241.67 aggregate frames/s without egui presentation. Earlier original-receiver
attempts changed codec, so they do not provide an equal-work CPU comparison.
A separate earlier one-stream 4K H.264 diagnostic delivered 60/s through the same
hardware backend. Physical 4K120 remains unqualified.

One original 4K UI attempt wrote a completed phase report but failed child-exit
validation. A repeat exited normally. The first attempt is excluded; the runner
now preserves numeric exit/timeout and sanitized failure classifications if it
recurs. No root cause is claimed from that single failure.

### Repeated lifecycle and cleanup

Both `v4-retry-cycles-four-*` runs completed seven two-minute cycles, including
three Leave/rejoins, with stable HEVC and no sampled errors. All stopped phase
interiors had zero active watches and decoded-frame delivery. Texture counters
sometimes reset when media ends, so rates with detected decreases are null in
the aggregate. A cached picture update is distinct from continued traffic. Minimized
phases kept all four watches near 240 decoded frames/s with zero texture updates.

The last 4.5 seconds of each 20-second Stop phase start at least 15 seconds
after Stop, beyond the ordinary blocking-thread retirement interval. First/last
cycle observations were:

| Settled stopped measure | Baseline first → last cycle | Selected first → last cycle |
| ----------------------- | --------------------------: | --------------------------: |
| Private MiB             |             237.02 → 314.60 |             261.12 → 339.07 |
| Working set MiB         |             177.70 → 211.26 |             175.88 → 213.35 |
| Handles                 |               2,320 → 2,291 |               2,320 → 2,288 |
| Threads                 |                     44 → 35 |                     45 → 34 |

Private memory rose about 77.6/78.0 MiB in the baseline/selected session, with
no corresponding monotonic handle/thread growth. This bounded-session test
does **not** establish a settled memory plateau, identify all retained owners or
prove leak freedom. It also does not establish a new stopped-memory saving.
Grid intervals immediately after rejoining include connection settling; use the
separate sustained grid comparison for throughput and CPU claims.

### Portable, audio, control and server

The longer publish-only comparison used a 120-second extension and excluded
the first 60 seconds of the initial grid. Baseline/candidate CPU was 8.20%/9.16%
of one core, private memory 370.48/373.88 MiB, and encoded rate 60.21/59.81/s.
Both counted one new preview in the measured interior. No publishing-only
speedup is claimed; the short case's larger CPU difference did not persist at
that magnitude in the longer comparison.

The final three-pair software-AV1 comparison also tested four Tokio workers
against the host default of sixteen. Grid process threads fell from 121 to 110,
but private memory was 411.53 versus 412.14 MiB and CPU 106.34% versus 104.52% of
one core. Decoded rates stayed around 240/s. That does not establish a meaningful
RAM/CPU win for a global runtime-policy change; the default is retained.

Corrected audio cases use the duration-safe synthetic helper and explicitly
verify that its source process remains alive at app shutdown. Earlier audio
cases retain their measured traffic but lack that liveness check. Two original
contention cases are excluded: the helper's 75-second run-loop guard rejected
its accepted 90-second request, while the app itself completed its phases.
The corrected pair ran for 90 seconds in both variants, delivering 293,519 and
311,415 synthetic GPU-load buffers. Receiver grid CPU was 137.60% to 86.87% of
one core, decoded frames 239.49 to 239.96/s, and texture updates 233.13 to 235.94/s.
This is an unpaced competing workload with its own measured throughput, not a
fixed-work game frame-time comparison.

With four audible sources, the corrected receive-only pair measured grid CPU
122.55% to 85.49% of one core, decoded frames 239.74 to 239.87/s and summed
per-channel audio frames about 191,884 to 191,988/s. Simultaneous publishing
plus four audible watches measured 156.44% to 95.58%, encoded frames 58.92 to
59.44/s and decoded frames 239.53 to 240.11/s. No sampled errors or playback
queue guard drops were reported. These guards do not measure physical WASAPI
underruns, speaker timing or game frame-time tails.

The streamed ZIP reader hashes 64 KiB chunks and confines seeks/reads to the
verified payload. It preserves digest/footer/path/count/expanded-size/cache
checks, extraction locking, completion rules and child/job supervision. Windows
keeps one handle denying writers/deletion between verification and extraction.
Non-Windows uses a private anonymous verified snapshot because ordinary file
locks are advisory; that platform’s added test has not been run on this host.
Waiting memory was already small before this pass. The roughly 77.5 MiB new
saving is startup peak, not a second waiting-memory saving.

Small-operation measurements used five runs of the same batch. These are
median **total wall milliseconds per batch**, not whole-app CPU percentages:

| Operation / iterations                                | Before → retained change, ms |
| ----------------------------------------------------- | ---------------------------: |
| Unchanged profile parsing / 50,000                    |                18.60 → 0.121 |
| Unchanged 96 KiB JPEG copy / 10,000                   |                 8.69 → 0.002 |
| Unchanged audio gain / 100,000                        |                39.05 → 0.037 |
| Audio bus lookup / 100,000                            |                 9.73 → 0.019 |
| Owned PCM plus copy → owned transfer / 100,000        |                33.45 → 30.00 |
| Fresh → shared immutable 64 KiB upload block / 20,000 |                 6.93 → 0.180 |

These remove redundant operations; they do not establish a multi-percent
native-app CPU gain individually. The PCM comparison checked every byte.

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

The real-app message counter enabled Debug logging but retained only counts.
The final four-stream case counted about 128.8 native SDK messages/s in grid,
53.7/s minimized and 31.4/s stopped. The separate formatting/filter operation
test's 100,000 messages are a stress batch, not this app's observed rate. No
actual-app CPU benefit from changing the logging policy was established, so
the default diagnostics policy is retained.

Twenty temporary capability factories took a median 30.83 ms versus 1.18 ms
when one was reused, but the reused factory retains execution contexts. That
small occasional saving does not justify a persistent capability-factory cache.

The three-pair monitor-capture pipeline experiment measured sampled peak
private memory about 233.84 MiB for existing DXGI versus 272.25 MiB for WGC,
with 72 versus 78 threads. Delivered buffers over roughly three seconds were
175 versus 169–170. This is a short capture-stage observation, not a game
frame-time result or a reason to remove WGC window capture. NVIDIA P1/P4/P7 and
clocksync experiments did not justify changing the existing P4/pacing defaults.
The fixed-source encoder quality check used 30-frame ball/snow sequences and
the same bitrate target; output bytes varied and it is not a perceptual-quality
or gameplay qualification.

The server query microbenchmark uses fresh token hashes, room/session reads and
current expiry on every call. Immediate revocation, expiry and room separation
passed. The real HTTP comparison preserved bounded paced transfers and global
lease behavior. Its CPU includes the synthetic fetch client and transferred
amounts varied under Windows timer scheduling, so it is not an isolated server
CPU or production throughput result. No native-app RAM saving is attributed to
server changes.

## Not executable as hardware/deployment qualifications tonight

| ID  | Gate                                                                   | Treatment                                                                                                 |
| --- | ---------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| X01 | AMD AMF/AV1, RX 7600 and the reported Siege capture/stutter case       | No AMD GPU supplied. Keep separate; NVIDIA evidence does not qualify it.                                  |
| X02 | Physical 4K120 source/display and other GPU generations                | Synthetic 4K experiments are labelled individually. No physical 4K120 or cross-generation claim.          |
| X03 | Fresh physical PCs, other OS backends and friends/group field sessions | Isolated cache/PATH/dependency checks only; no fresh-PC or physically disconnected-network qualification. |
| X04 | Production WAN/VPS capacity, deployment and field latency              | Local API/SFU experiments only.                                                                           |
| X05 | Physical display-to-wired-speaker A/V offset                           | Pipeline timestamps/queues are distinct from physical output timing; no offset fix is claimed.            |

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

The frozen candidate is native source `000129f9153a560a38030d77f06e8a1844dbed99`.
The branch also includes `main` through `0d16ce8` (merge `18d1d29`). Completed:

- `npm run check`: formatting, types, builds and 67 tests (40 server, 23 web,
  four tool/config tests).
- Locked Rust tests: nine portable plus 39 publisher tests by default; nine
  plus 40 with the experiment feature. Both all-target Clippy configurations pass.
- Six SDK/setup tamper tests; original baseline SDK reproduction; native helper
  checks including generic mapping fallback and rejected destination dimensions.
- Eleven actual native API/SFU acceptance cases: H.264/HEVC/AV1 GPU, forced AV1
  CPU, preview cadence, explicit unsubscribe/rewatch, codec fallback, stereo
  audio/exclusions/restart, full reconnect and aggregate connection allowance.
- All 22 `v4-final-*` short actual-app comparisons completed.

The final recovery (six), explicit-worker (six) and endurance (eleven) cases
completed successfully. The original matrix status retains helper failures and
intentional suite-boundary stops; `final-v4-tail-status.json` records the successful
reruns. No failed helper is counted as a successful contention workload.

The required GPT-6.1-Sol high implementation review approved
`0d16ce85a4408209a49975da1d36ddff6c652d14..ff5a3e3d473a286d30259d97d80cdfcacddc2c5b`
with no blocking findings. Packaged qualification and the separate final-range
evidence recheck remain before the final PR update.
