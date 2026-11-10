# Independent follow-up performance experiments

Status: **in progress**. These experiments start from the optimized application at
`e93b71d437810192bd1648e65088f1a6b9196754`, not the original pre-optimization
application. Work is isolated on `perf/deep-individual-experiments` in
`freedomstream-perf`. The original checkout is preserved.

Each family is compared separately. An experimental switch is not a shipping
default, and a source-level cost or calculated byte reduction is not a measured
performance win. Successful assertions must include equivalent useful output.
Raw sanitized measurements, executable identities and diagnostic receipts are
kept under `.local/experiments/deep-pass` and named actual-room run directories.

| Experiment         | Baseline                             | Candidate                                             | Required countermetrics                                       | Status                                                                    |
| ------------------ | ------------------------------------ | ----------------------------------------------------- | ------------------------------------------------------------- | ------------------------------------------------------------------------- |
| E1 clipped tiles   | Renew all tile leases                | Renew intersecting video rectangles only              | Same subscriptions/audio/delivery; visible updates; restore   | Repeating actual app, including scroll/audio/five incoming streams        |
| E2 planar Glow     | I420 to pooled RGBA to egui          | Owned I420 planes and video shader                    | Rendered pixel equivalence, unique frames, age, GL work, RAM  | 27 actual-GL pixel cases passed; repeating resource comparisons           |
| E3 usage reports   | Current synchronous full report      | Batched queries; bounded closed-history JSON cache    | Every response field, history edges/pages, concurrent latency | Batching rejected; cache measured; production integration awaiting checks |
| E4 stopped memory  | Current lifecycle                    | Attribution and conditional default-heap optimization | Completed teardown, live resources, next-watch latency        | Observed native owners drain; seven-cycle comparisons queued              |
| E5 decoder devices | Independent pipeline contexts        | Compatible active shared context                      | Actual device count, equal pixels/frames, locks, recovery     | Native comparisons positive; actual-app qualification queued              |
| E6 pool/readback   | Current pool and one staging texture | Polling with and without explicit submission          | No stale pixels/drops, frame age, memory, CPU and waits       | Both native variants measured; latency costs documented below             |

Trials alternate order and never run simultaneous performance workloads. CPU is
reported with one core equal to 100%; private bytes are commit and working set is
resident memory. Driver GPU engines are separate. Egui image handoffs are not
named actual GL uploads. Diagnostic instrumentation is separated from ordinary
release measurements. External AMD, physical 4K120, device-loss, WAN/VPS and
physical A/V qualification remain open.

## Completed intermediate results

These are intermediate receipts, not final acceptance or adopted-default claims.
The frozen application repeat uses `sdk-v4-room.exe`, SHA-256
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
final adopted-path checks and final evidence review are still outstanding.

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
In particular, the current planar prototype is qualified for the exercised I420
path and does not establish robust recovery for a later unsupported-buffer
transition or partial GL initialization failure. Those paths need hardening and
qualification before promotion. Default application changes currently consist
of rectangle-based presentation demand and the bounded historical-report cache;
final adopted-path acceptance and independent review remain outstanding.

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
