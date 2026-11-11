# Independent follow-up performance experiments

**Historical UI measurements:** this campaign predates native UI PR22. Its
clipping numbers are not verified gains on the redesigned layout. See the
[native UI integration ledger](native-ui-performance-integration.md) for the
separate post-redesign checks and counterexamples. The measurements below remain
unchanged historical evidence.

Status: **local experiment campaign completed**. Packaged acceptance and its
remaining verification limits are documented at the end of this report.
These experiments start from the optimized application at
`e93b71d437810192bd1648e65088f1a6b9196754`, not the original pre-optimization
application. Work is isolated on `perf/deep-individual-experiments` in
`freedomstream-perf`. The original checkout is preserved.

Each family is compared separately. An experimental switch is not a shipping
default, and a source-level cost or calculated byte reduction is not a measured
performance win. Functional success and performance qualification are separate;
unequal work does not establish an equal-work CPU gain.
Raw sanitized measurements, executable identities and diagnostic receipts are
kept under `.local/experiments/deep-pass` and named actual-room run directories.

| Experiment         | Baseline                             | Candidate                                             | Required countermetrics                                       | Status                                                                                                        |
| ------------------ | ------------------------------------ | ----------------------------------------------------- | ------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| E1 clipped tiles   | Renew all tile leases                | Renew intersecting video rectangles only              | Same subscriptions/audio/delivery; visible updates; restore   | Retain by default; scroll/audio/restoration passed; all-visible control has no gain                           |
| E2 planar Glow     | I420 to pooled RGBA to egui          | Owned I420 planes and video shader                    | Rendered pixel equivalence, unique frames, age, GL work, RAM  | Keep disabled prototype; 27 pixels and 12 lifecycle checks pass in both ownership modes; CPU tradeoffs remain |
| E3 usage reports   | Current synchronous full report      | Batched queries; bounded closed-history JSON cache    | Every response field, history edges/pages, concurrent latency | Batching rejected; production cache and authenticated HTTP checks passed                                      |
| E4 stopped memory  | Current lifecycle                    | Attribution and conditional default-heap optimization | Completed teardown, live resources, next-watch latency        | Reject default heap optimization; seven-cycle pairs and diagnostic-history replay completed                   |
| E5 decoder devices | Independent pipeline contexts        | Compatible active shared context                      | Actual device count, equal pixels/frames, locks, recovery     | Keep disabled prototype; 20 app cases pass; 4K delivery/memory tradeoff prevents broad adoption               |
| E6 pool/readback   | Current pool and one staging texture | Polling with and without explicit submission          | No stale pixels/drops, frame age, memory, CPU and waits       | Reject polling latency tradeoff; no observed pool-copy pressure justifies resizing                            |

Trials alternate order and never run simultaneous performance workloads. CPU is
reported with one core equal to 100%; private bytes are commit and working set is
resident memory. Driver GPU engines are separate. Egui image handoffs are not
named actual GL uploads. Diagnostic instrumentation is separated from ordinary
release measurements. External AMD, physical 4K120, device-loss, WAN/VPS and
physical A/V qualification remain open.

## Experiment results

The first frozen clipping repeat uses `sdk-v4-room.exe`, SHA-256
`08e59fd265bcf3bc42a7975deb4670bffa5b60dcc37d1c10fa55c20082d4ad64`.
Every comparison within that campaign changes one candidate switch in the same
executable. Experimental SDK hooks remain in an isolated copy; the normal pinned
decoder headers and SDK are unchanged.

### E3: historical usage reports

The fixture uses the real SQLite schema, 30 synthetic users, 400 days and 522,000
participant-minute rows. Five alternating pairs of six query cases compare every
response field. Batched/prepared SQL did not improve substantial history: median
30-day wall time was 107.828 → 119.634 ms and 366-day time was 738.716 → 778.957 ms.
The microsecond empty-range improvement does not justify adopting this candidate.

Two bounded cache prototypes were tested through real administrator login,
usage, health and logout HTTP routes in separate client/server processes. Cache
hits reuse closed history only, while current writes occur outside that range.
Fresh sample timestamps and collector availability are checked on every report.
Both prototypes passed 26 data/invalidation/ownership checks. The JSON variant's
five-pair burst medians were 9,046 → 62 ms server CPU for ten reports and forty
health requests, report p50 929.176 → 4.192 ms and health p95 922.622 → 2.560 ms.
These are warmed, unchanged-history hits; they do not speed every report or its
first SQL calculation.
The normal Today/7 days/Month dashboard ranges include the live minute and bypass
this cache. The measured benefit applies to repeated closed custom-history
requests; it must not be advertised as a blanket reduction in ordinary server
CPU or the default dashboard's query work.

The short burst's end RSS increased about 26.9 MiB, with different elapsed times
and garbage-collection opportunities. A separate fixed 60-second pair delivered
30 reports and 30 health requests per variant, paced every two seconds, without
forced GC or working-set trimming. CPU was 22,999 → 391 ms, report p50
778.934 → 4.652 ms, and health p95 799.874 → 5.555 ms. End RSS was
77.23 → 75.72 MiB; sampled RSS peaks were 78.39 → 95.65 MiB. Thus the original
burst RSS gap is not established as persistent retained memory, and the transient
peak remains a real cost. One steady pair is a diagnostic, not replicated VPS
qualification. The cache accounts about 1 MiB of retained serialized content in
this fixture, within a four-entry/2 MiB bound; that bound is not a promise about
the JavaScript engine's whole heap.

### E5: sharing compatible active decoder devices

Three alternating pairs of four visible decoder contexts process the same 300
encoded HEVC fixture frames per context. At 1080p, median CPU was
3,515.62 → 2,296.88 ms, sampled peak private memory 418.81 → 338.55 MiB and
decoder-input-to-materialized-frame age p95 4.845 → 4.641 ms. At 4K, CPU was
10,140.6 → 6,156.25 ms, private memory 1,064.78 → 904.09 MiB and age p95
11.694 → 11.647 ms. Observed device creation fell from four to one. These are
decoder probe measurements including Configure/Release CPU, not whole-app gains.

All 300 whole-plane frame hashes agree across baseline, device-sharing and
readback variants for H.264, HEVC and AV1 at one context, and HEVC at four
contexts. Observed native resources drain after teardown. Same-adapter-LUID
matching and the pinned upstream BGRA creation flag are preserved. Adapter
changes, device removal, other drivers and AMD remain rollout risks; positive
probe results alone do not enable sharing in the shipped decoder.

### E6: readback scheduling and decoder pool pressure

The first nonblocking Map/polling candidate reduced CPU but regressed four-stream
age p95 from 17.562 → 32.456 ms at 1080p and 24.707 → 395.573 ms at 4K.
**Reject this candidate.** Complete output and matching pixels do not compensate
for that latency regression.

A revised candidate explicitly submits GPU work before polling. Three paired
four-context medians were 3,968.75 → 1,640.62 ms CPU at 1080p, with age p95
4.966 → 8.279 ms; at 4K CPU was 10,500 → 5,531.25 ms and age p95
24.416 → 35.849 ms. This removes the initial severe stall but retains a CPU versus
latency tradeoff. It is not an unqualified speedup or a shipping default.

Separate trace-enabled visible/hidden decoder probes counted 3,186 fixed-category
messages each and 1,200 received samples, with no output-copy or pool-full
classifications. All samples were single textures. A direct-render log count of
zero does not prove zero direct rendering: the pinned code has other return
paths. The probe retains six frames per context for post-timing hash checks,
which changes ownership pressure. No pool size is increased on this evidence.

The polling, device and ownership hooks are experimental source copies. The
documented [HeapOptimizeResources API](https://learn.microsoft.com/en-us/windows/win32/api/heapapi/nf-heapapi-heapsetinformation)
acts on the process's own default heap in the separate E4 experiment; it is not a
working-set trim and does not free live allocations. E4's remaining comparisons,
the completed seven-cycle comparison is documented below. Final package and
independent review is tracked with the PR.

## Validation checkpoint and measurement repair

The first completed application repeat contains 18 successful cases: three
alternating pairs each of narrow and wide windows, plus scroll, received audio
and five-incoming-stream pairs. All functional assertions passed. Scrolling
exposed every tile and resumed image handoffs; fully hidden intervals stop
converting after the presentation lease expires. Received audio continues at
about 192,000 PCM frames/s across four subscribed streams, with no output-frame
drops in the asserted phase interiors. This is not a physical A/V synchronization
test.

The narrow-window three-pair grid medians were CPU 85.832 → 39.099% of one core,
private commit 737.594 → 640.480 MiB, working set 414.113 → 331.688 MiB,
driver 3D engine 25.506 → 5.489%, and dedicated GPU memory 287.047 → 256.855 MiB.
Consumer delivery remains about 240 frames/s; the one visible tile preserves its
handoff rate in every pair. These savings are conditional on fully clipped
pictures. The wide, all-visible controls show mixed CPU results and substantial
variation (84.160 → 97.857% median), prompting longer steady-window controls.
The five-stream single pair lowers CPU 127.599 → 46.511% but increases resident
memory 471.027 → 504.102 MiB. It is a CPU win, not a RAM win.

An additional frozen benchmark build, `sdk-v6-room.exe`, SHA-256
`5f3022e94f35c9bad57678aa4e8be2f77d811ec52fa731ff722ec94a2d1ce26e`,
observes the regular RGBA path at the actual Glow upload call. A private copy of
egui_glow 0.37.0 is extracted from its Cargo.lock-hash-verified crate archive.
Only explicitly tagged watched-video textures are counted; fonts and other UI
textures are excluded. The observer's tag registry is bounded and removes freed
IDs. The temporary resolution is verified to differ solely in Glow's
registry-to-local-source origin; Cargo.lock is restored byte for byte. No
distributed dependency or runtime is changed. The decoder SDK is separately
verified before and after building.
The v6 observer retains a tag if a watched texture ID is later reused for a
stopped thumbnail. The active watched-grid comparisons remain valid; stopped
thumbnail uploads must not be described as live-video work. The next observer
explicitly untags that transition and will assert zero live-video tags after
Stop. Stopped memory still intentionally includes four thumbnails (about 8 MiB
of logical RGBA texture content in the seven-cycle baseline).

The first GL-observed clipping pair measured video API upload payload
1.860 → 0.458 GB/s and RGBA upload calls 224.282 → 55.167/s while all four
streams still delivered about 240 frames/s. These are API-call input bytes,
not physical PCIe bandwidth. Planar uploads count three plane calls per picture;
their logical payload is counted separately. Earlier image-handoff counters are
retained under their proper name. Counter overhead is not subtracted from
instrumented application CPU measurements.

The production `Usage.reportJson` integration passed all 41 server tests. Its
five-pair authenticated HTTP comparison and separate steady pair reproduce the
cache benefit. The steady pair used 23,781 → 281 ms server CPU in 60 seconds,
report p50 812.313 → 6.872 ms and health p95 791.083 → 7.001 ms. End RSS was
80.04 → 75.26 MiB and sampled peaks 93.67 → 94.93 MiB. The burst still shows
higher end RSS; no general RAM-saving claim is made for this cache.

At this checkpoint, Node 24 `npm run check`, Rust formatting, Clippy with all
targets/features, 9 portable tests, 40 publisher tests, the default release build
and all 27 pixel cases on the new benchmark executable passed. One earlier
typecheck found unchecked test-array accesses; explicit presence assertions fixed
the test, then the complete workspace check passed. An initial observer-preparation
attempt expected vendor checksum metadata absent from the installed registry;
the corrected helper verifies and extracts the pinned crate archive instead.
Neither helper failure is counted as a performance trial. The first queue was
intentionally stopped at its idle validation boundary after the 18 cases; the
remaining matrix uses the new frozen build.

The planar renderer and decoder-device/readback changes remain prototypes.
The implementation checkpoint review approved the default clipping/cache paths
and identified two experimental-only issues: stale planar state after an RGBA
fallback and nontransactional partial GL initialization. Both have been hardened
and pass actual-GL transition, failure and retry qualification on frozen sdk-v8
in both retention modes. Default application changes currently consist
of rectangle-based presentation demand and the bounded historical-report cache;
packaged acceptance is documented below; independent review is tracked with the PR.

The longer all-visible control has now completed three alternating pairs, each
with a 70-second grid phase. Excluding the first ten seconds, CPU medians were
97.262 → 98.304% of one core, consumer delivery 239.895 → 239.857 frames/s and
actual RGBA uploads 222.341 → 224.118/s. Per-pair CPU changes ranged from a 0.8%
reduction to a 5.0% increase. No all-visible performance gain is claimed; the
larger short-run CPU difference did not persist. Resident memory medians were
416.414 → 428.945 MiB, so this control establishes no RAM saving either.

Resource summaries use the upper-middle empirical median (sorted index
`floor(n/2)`); three-pair comparisons therefore use the middle trial. Raw
per-phase and per-pair values remain available. Equal-work classification uses
consumer rates within 3%, visible handoffs within 5%, and simultaneous publication
within 3%. A separate noninferiority flag permits greater delivered work. Neither
flag establishes equal physical presentation or equal latency.

## Completed visible-presentation experiments

Percentage headlines below use the **median of paired percentage changes**.
The before/after values are separate empirical medians and need not produce
that percentage when divided. For example, the one-1080p planar CPU medians
are 28.918 → 23.247% of a core, but the three paired CPU savings are 23.9%,
3.1% and 5.2%; the typical paired saving is **5.2%**, not 19.6%.

### E2: planar rendering is a resource tradeoff

All comparisons in this section use the frozen GL-observed `sdk-v6` executable.
The custom renderer remains disabled by default. The original 27 actual-GL
pixel comparisons cover odd/padded planes, three patterns and three output
sizes, with at most two component levels of error and mean error ≤ 0.35.
They qualify those synthetic pictures, not every graphics driver or color mode.

| Workload                       | Paired CPU saving | Paired resident RAM saving | Paired 3D-engine saving | Paired API upload-payload saving | Interpretation                                                                                |
| ------------------------------ | ----------------: | -------------------------: | ----------------------: | -------------------------------: | --------------------------------------------------------------------------------------------- |
| Four HEVC 1080p60, three pairs |             22.7% |                      20.7% |                   51.6% |                            61.5% | Delivered about 240 consumer frames/s; every pair noninferior, one pair has >5% more handoffs |
| One HEVC 1080p60, three pairs  |              5.2% |                      12.3% |                   47.5% |                            62.4% | CPU improvement varies substantially by pair                                                  |
| Four HEVC 1080p30, three pairs |         **−9.8%** |                      13.4% |                   51.5% |                            62.5% | More CPU despite lower memory/GPU costs at comparable useful work                             |
| One HEVC 4K30, three pairs     |         **−1.9%** |                      25.0% |     Lower in every pair |                      About 62.5% | Not a CPU win; 1080p physical display                                                         |

For four 1080p60 streams the independent medians are CPU 96.932 → 74.973% of
one core, private commit 784.457 → 675.953 MiB, working set 457.949 →
366.449 MiB, 3D engine 24.595 → 11.759%, and API payload 1.831 → 0.699 GB/s.
Dedicated GPU memory falls 292.113 → 271.910 MiB. Decode-engine occupancy
does **not** fall uniformly (11.766 → 12.870% medians); these GPU engines must
not be added or described as a total GPU saving. The paired CPU reductions are
29.7%, 5.8% and 22.7%, with only three trials and unlocked GPU clocks.

The two one-4K60 pairs show CPU 55.731 → 56.675% of one core, resident RAM
487.363 → 429.133 MiB, 3D engine 21.026 → 8.549%, and payload 1.635 →
0.637 GB/s. One pair delivers more than 3% additional decoded frames. No
equal-work CPU improvement is established.

The single four-4K60 capacity pair delivers 178.856 → 231.656 consumer frames/s
and 98.329 → 194.824 picture updates/s while using CPU 492.593 → 214.014% of
one core. That is substantially **unequal work**. It demonstrates local
presentation-path capacity, not an equal-work CPU percentage, physical 4K
presentation, 4K120 support or game-FPS improvement.

Single four-1080p60 pairs also complete for H.264, hardware AV1 and forced
software AV1. Their CPU values are 160.975 → 82.325, 94.611 → 72.514 and
116.495 → 99.264% of one core, respectively. Resident RAM and API upload payload
fall in each pair; software AV1 has no hardware decode-engine work. These are
codec coverage and single-pair observations, not replicated codec-specific gains.

An independent ownership refinement compares retaining versus releasing I420
planes after their synchronous client-memory uploads, with planar rendering
enabled on both sides. Ordinary client-memory GL transfers consume the supplied memory
before returning; pixel unpack buffers have different ownership semantics.
The prototype explicitly unbinds/restores the unpack buffer around client data
([Khronos pixel buffer contract](https://wikis.khronos.org/opengl/Pixel_Buffer_Object),
[synchronization contract](https://wikis.khronos.org/opengl/Synchronization)).
The hardened `sdk-v8` build, SHA-256
`6d127fda7e0e013cf0a802c079f5a3544820a93e5147148f4d9ea214dd7f603c`,
passes all 27 pixel comparisons and 12 injected-failure/lifecycle checks in both
retention modes. The transition check uses a real native NV12 buffer rejected by
the I420-only path, normal RGBA conversion, and the UI mailbox consumer, followed
by I420 again. This is a synthetic consumer/API qualification, not a negotiated
unsupported-buffer media-session test or physical device-loss test.

Eight actual-app cases compare only CPU-plane retention. Across three
four-1080p60 pairs, observed median live I420 bytes fall **23.730 → 11.865 MiB**
in every pair. Other owners still retain planes; they do not all disappear after
upload. Paired median private/resident savings are only **1.4% / 1.9%**. CPU
changes are mixed (+14.1% saving, 5.7% cost, 0.3% saving), so no CPU gain is
claimed. Consumer delivery stays about 240 frames/s and API payload is unchanged.
The single one-4K pair also halves observed live plane bytes, but process private
memory rises 596.313 → 599.070 MiB, resident RAM rises 378.316 → 388.242 MiB and
CPU rises 47.318 → 49.202% of a core. Logical owner reduction is not a universal
OS-memory saving. Keep the simpler ownership lifetime inside the disabled
prototype; it does not become a shipping-default performance claim.

### E4: no worthwhile idle-heap improvement

Both 14-minute runs finish seven Watch/Stop cycles, including Leave/rejoin on
cycles 3, 5 and 7. Every stopped phase has zero observed decoder devices,
GstSamples, deferred frames, I420 plane bytes and staging textures. Video task
completion catches up with starts, and RGBA pools release. The four stopped
thumbnails remain intentionally present. These observations qualify the counted
owners; they do not account for every driver or allocator object.

All seven candidate idle-heap evaluations are eligible and the Windows API
returns success. The first five calls immediately decommit about 1.00–1.66 MiB;
the last two decommit **nothing**. Calls take 1.27–2.20 ms. Default-heap committed
memory ends at 156.555 MiB without the call and 156.758 MiB with it. Last-cycle
stopped-tail private memory is 297.355 → 319.609 MiB, while resident RAM is
217.938 → 212.379 MiB. There is no useful persistent private-memory reduction.
**Do not adopt automatic heap optimization.**

Six non-prewarmed restart observations per run complete. The comparison preserves
actual subscriptions and expected received frames after restart, but individual
handoff/CPU results vary and two grid cycles miss the strict handoff noninferiority
threshold. No active-playback or restart-speed gain is claimed. Seven cycles
inside one before/after pair are not seven independent process trials.

An attribution limitation is explicit: the benchmark itself retains every
per-second JSON snapshot in `BenchmarkUi.samples`. That diagnostic history is
absent from the shipped UI and contributes live allocations during a long run.
The default-heap allocated growth must not be attributed entirely to product
media owners or called a leak. A separate process replays ownership of the same
799 JSON snapshots while holding its parsed input constant. Cloning the history
increases allocated default-heap storage by about **8.49 MiB**; dropping those
clones releases about **8.48 MiB** of live allocations. Committed heap storage
does not return to its initial level. This demonstrates diagnostic-history
ownership; it is not the exact overhead of the original GUI collector, an app
optimization, or a substitute for allocation-stack attribution. No precise
fraction of the app run's growth is assigned to it.

### E5: actual-app device sharing

Three alternating four-HEVC-1080p60 pairs deliver approximately 240 consumer
frames/s in both variants. Private commit medians fall 771.410 → 515.426 MiB,
resident RAM 444.051 → 332.371 MiB, dedicated GPU memory 292.066 → 203.266 MiB,
and threads 157 → 87. Median paired reductions are **34.1% private commit,
26.0% resident RAM, 30.4% dedicated GPU memory and 43.8% threads**. Actual
contexts use one observed device instead of four. CPU medians are 92.579 →
86.819% of a core, but median paired CPU saving is only **3.6%**. Graphics
work and upload payload remain essentially unchanged; this is chiefly a
resource-sharing benefit.

The one-4K control increases private/resident memory (681.563 → 726.840 MiB and
445.383 → 487.844 MiB). A single decoder does not establish a memory benefit.
The mixed-codec pair reduces resident RAM 425.980 → 352.910 MiB. The combined
publish/watch/audio pair reduces resident RAM 488.328 → 423.156 MiB and CPU
110.785 → 97.509% of a core, but publication rises 54.215 → 59.149 frames/s;
that pair is noninferior and **not strict equal work**. Received PCM remains
about 191,930 frames/s. No physical A/V claim follows.

Eight further cases on sdk-v8 finish successfully: four-4K capacity, synthetic
GPU conversion contention, three repeated recovery cycles per process, and
stopping one subscription while three remain active. All counted native
resources drain after the final Stop. Selective Stop preserves one shared device
for the remaining streams and restores four subscriptions when rewatching.
The revised source rejects removed devices and prunes expired weak entries.

The four-4K capacity pair lowers private/resident memory 1,947.000 → 1,253.305 MiB
and 1,152.758 → 816.551 MiB, but consumer delivery falls **185.149 → 173.767/s
(6.1%)** while handoffs increase **99.561 → 109.657/s (10.1%)**. This fails the
delivery noninferiority threshold; CPU 506.149 → 467.051% of one core is not an
equal-work win. The contention pair preserves about 240 consumer frames/s and
lowers private/resident memory 751.770 → 547.973 and 423.266 → 342.488 MiB.
Its load is repeated 4K RGBA/NV12 GPU conversion, not a game benchmark. Recovery
and selective Stop pass, but their cycles do not constitute independent repeats.
**Keep sharing experimental:** the strong 1080p footprint return does not establish
a uniform benefit at 4K capacity. Physical device removal, adapter switching,
other drivers and AMD also remain unqualified.
The reuse guard checks the documented HRESULT from
[GetDeviceRemovedReason](https://learn.microsoft.com/en-us/windows/win32/api/d3d11/nf-d3d11-id3d11device-getdeviceremovedreason);
it does not itself qualify device-removal recovery.

### E6: whole-app readback and pool observations

The revised flush/poll candidate completes twelve actual-app cases: three
four-1080p60 pairs, one one-4K60 pair, one mixed-codec pair and one simultaneous
publish/watch/audio pair. The four-stream grid has median paired CPU reduction
**30.7%**, with consumer delivery approximately 240 frames/s and comparable
handoffs in every pair. CPU medians are 102.749 → 69.058% of one core. This is a
real CPU reduction, but the separate fixed-work native comparison establishes
increased materialization age. The implementation is **not adopted**: a CPU win
with worse frame-age tails is not the intended low-latency outcome. The combined
case also delivers >3% more publication frames and is not strict equal work.

Four additional actual-app cases compare tracing disabled/enabled at four
1080p60 and four-4K60 subscriptions. Trace-enabled runs classify 28,178 and
28,150 fixed-category messages, respectively, with zero pool-full or output-copy
classifications and no mapping failures. This agrees with the earlier native
probe for the exercised single-texture path. It does not prove all driver paths
avoid copies or identify every allocated DPB surface. Trace-on CPU/RAM differences
are not optimization gains. **No decoder-pool resizing is implemented.**

## Evidence, adoption and reproduction

All **108 controlled actual-app cases** across 15 families complete with stable
codecs and pass their functional assertions. Work eligibility is separate:
unequal/capacity cases remain labeled and are not counted as equal-work CPU wins.
Another 28 successful pilot/calibration runs are preserved but excluded from
headlines. Native probes, authenticated HTTP runs, pixel/lifecycle checks and the
history-ownership replay are separate evidence classes.

The [sanitized aggregate](deep-performance-results.json) preserves paired values,
workload classifications, fixture/binary/source identities and hashes of the
local evidence. The [interactive comparison](deep-performance-comparison.html)
uses those values for 32 selectable scenarios. Positive bars mean reduced cost;
negative bars expose increases. Percentages are not additive across experiments
or with the earlier performance pass.

The first 26 clipping receipts predate recording the new switches in
`experimentSettings`. Their frozen runner records the before/after selection,
and conversion/visibility assertions establish the exercised clipping behavior.
Their raw receipts remain unchanged; the aggregate explicitly marks missing
switch metadata rather than inventing it. Later switch receipts are checked
directly. Publisher/helper identities and all other recorded enabled settings
must agree inside each pair. All measured publisher/helper binaries are retained.
Server source and compiled modules are archived after the runs and checked
against the final compilation; these are not falsely dated as pre-run hashes.

**Retained defaults:** clipped video rectangles stop renewing presentation demand,
and repeated closed historical usage reports reuse bounded JSON. The shipped
native decoder headers, their pins, Cargo.lock and distributed runtime remain
unchanged. **Disabled experiments:** planar rendering and compatible device
sharing, with their measured benefits and counterexamples above. **Rejected:**
automatic idle heap optimization, SQL batching and both polling-readback policies.
No pool resizing is justified by the observed classifications.

Node 24 `npm run check`, all-target/all-feature Clippy, Rust formatting and all
49 Rust tests pass for the implementation. Every one of the 257 fixed chart
configurations passes the chart validator. Desktop/dark and 320px/light rendering
show no console errors or clipped content. The control configs are validated;
automated browser interaction coverage is not claimed.

Use the preserved local runtime, fixtures and frozen publishers described in the
aggregate. The runners protect existing measurements and require a fresh output
workspace for a new campaign. `build-deep-sdk.ps1 -Label <new-label> -ObserveGl`
creates a separate candidate SDK and GL observer, preserves source/binary
identities and restores Cargo.lock. Run each family sequentially with
`run-deep-performance.mjs`; never benchmark alongside builds or another workload.
To recompute the current evidence and chart:

```powershell
node scripts/summarize-deep-performance.mjs <family>
node scripts/check-deep-performance.mjs <family>
node scripts/summarize-deep-pass.mjs
node scripts/render-deep-performance.mjs
```

## Final package and audio qualification caveat

The default-feature portable package was built from clean implementation commit
`e24856c6308375083f5dbb2dd30f0f725e5e839f`. File:
`freedomstream-windows-x64-0.1.0-e24856c63083.exe`, 81,891,966 bytes, SHA-256
`edddc75ec4b552f9d9c28b56dd29a958ac56e5e5be7f8f9d72a1c6107f37b5ec`.
The bundled publisher is a Windows GUI executable, SHA-256
`4093ff6b4c5eb90dac940ca511de1f7be749ecb49d314f5695cc44057077dd7f`.
Experimental renderer/device/heap/observer markers are absent. The original
native SDK and dependency manifests are preserved. Later fixture/report changes
do not alter the packaged application sources.

The initial actual portable/API/SFU suite passes **10 of 11 cases** at its normal
cache. The desktop-audio test fails a frequency-rejection threshold. Four matched
controls on the earlier and current packages also fail with ordinary background
desktop audio present. Four further controls, two per package, pass every original
audio criterion after pre-existing background application images are excluded
before starting the synthetic tones. No raw PCM, background image paths, personal
audio, or user volume changes enter the evidence; only counts and fixed-frequency
magnitudes are retained. This is consistent with background contamination of the
spectral test, rather than a newly introduced audio-policy regression.

The official fixture now uses the same isolation while keeping its rejection
thresholds and restart checks. Node checks pass after that test-only change.
**The complete audible suite was not rerun after the fixture correction because
Felipe requested no further audible tests.** The initial failure and all eight
controls remain recorded; no clean 11/11 single-run result is claimed. Audio
product code is unchanged. This caveat and the independent final review are
explicit PR acceptance information, not hidden by the performance wins.
