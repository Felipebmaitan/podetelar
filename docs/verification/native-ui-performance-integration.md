# Native UI and performance integration

## Scope and qualification

PR19 is being integrated with native UI PR22 at immutable head
`21a9efae61434612c56e1824149877e9b15839e1`. PR23 will remain a separate,
subsequent change. Neither performance PR may merge until PR22 is confirmed
merged and its commit is present in `origin/main`.

That prerequisite is now satisfied: PR22 merged as
`f5c738f2a4f78eab8ed8a5f229b93ee29fbdcfc0`, and that actual `main` was merged
into PR19 before acceptance.

The earlier [performance campaign](local-performance-experiments.md) measured
the previous UI. Its measurements remain historical evidence, **not measured
gains on the redesigned UI**. New silent measurements and acceptance results
will be recorded here separately. No audible tests are authorized for this pass.
AMD, physical A/V, game impact, deployment and physical 4K output qualification
remain external gates.

## PR19 adaptation

- Use PR22's UI as the layout authority. Keep the new sharing-panel source
  confirmation, source picker, menus, focus/fullscreen controls and shutdown
  coordination. Do not restore the removed self-preview grid tile.
- Keep the image converter, exclusive bounded image reuse, deferred decoder
  ownership, bounded readback, desired-subscription comparisons and other
  non-UI performance changes.
- Port Arc image handoff and presentation leases into the new image consumer.
  Guard minimized/hidden viewport calls before consuming pictures or renewing
  presentation demand. Watch intent and audio remain independent.
- Port unchanged-JPEG revision filtering into the new source confirmation.
  A replacement source owner with the same sequence still refreshes the image.
  Preview capture scheduling remains on its existing 60-second gate.
- Preserve source-picker clearing when hidden and the new close/cleanup path.
  Optional frame-driven repaint remains experimental; default repaint is retained.
- Prune retired textures before the fullscreen early return, so another stream
  stopping does not retain its GPU texture until fullscreen exits.
- Add the new UI/source-picker modules to the feature-only benchmark example.
  Permit an immutable UI baseline in the existing source-export instrumenter,
  with counters and the same benchmark driver rather than performance patches.

## Validation in progress

Node 24 workspace checks, all-target/all-feature Clippy, 59 publisher tests and
nine portable tests passed. The two normally ignored Windows hardware tests were
also attempted silently. Window filtering passed; initial still capture passed
the pattern and monitor but failed on its arbitrary largest user window. The
test now creates and destroys its own non-activating Windows fixture rather than
depending on a potentially protected or disappearing user window. Both hardware
tests pass with that isolation. No product capture logic was changed for it.

Runtime recovery uses a separate local SDK directory; no missing runtime
directory in the original checkout is modified. The installer matches the
existing SHA-256 pin. The recovered SDK is a build prerequisite, not a newly
qualified redistributable runtime. Execution uses a clean copy of the 44 preserved
runtime files, each verified against the existing distribution manifest.

## Redesigned UI: PR22 versus PR19

[Sanitized results and input hashes](native-ui-performance-results.json) record
four real local SFU/native UI runs in A/B/B/A order: two copies of PR22 with only
measurement counters/the common driver, and two adapted PR19 runs. The CPU,
memory and frame-rate summary is reproducible with
`node scripts/summarize-ui-performance.mjs`. The machine is Ryzen 7 9800X3D,
16 logical CPUs, RTX 5080, driver 621.14. All publishers explicitly used audio
off. Every active phase retained four watches and approximately 240 decoded
frames/s; stopping watches brought both decoded frames and image handoffs to zero.

| View          | CPU before, one core = 100% | CPU after | CPU reduction |
| ------------- | --------------------------: | --------: | ------------: |
| Grid          |                     213.62% |    87.23% |        59.17% |
| Focus         |                     213.96% |    35.36% |        83.47% |
| Fullscreen    |                     252.75% |    34.59% |        86.31% |
| Minimized     |                     197.29% |    12.76% |        93.53% |
| Restored grid |                     254.72% |    96.29% |        62.20% |

These are two-run medians over short interiors of eight-second phases, not
sustained/game/latency qualification. Both matched pairs separately reduced CPU
by at least 10% in every active view. Focus/fullscreen convert approximately one
stream, minimization converts none, and restoration resumes four-stream
conversion. Grid egui handoffs remain approximately 220/s. Handoffs and decoded
frames do not establish physical displayed frame rates or actual GL upload counts.

Memory is a tradeoff, not a universal reduction. Grid private commit falls 3.49%,
but resident memory rises 7.42% (411.20 to 441.69 MiB). Focus, fullscreen and
minimization reduce both commit and resident memory in this sample. The short
stopped phase has **higher** memory: median commit 402.55 to 529.52 MiB and
resident 249.34 to 308.75 MiB, with substantial baseline variation. Stopped CPU
is also higher (0.69% to 2.09% of one core). These short post-stop samples are
recorded costs, not evidence of steady idle behavior or an unbounded leak.

An additional silent run per variant used 20-second phases. In its final five
seconds, at least 15 seconds after stopping, baseline/adapted commit was
276.21/270.39 MiB and resident memory was 194.14/184.84 MiB. Stopped-phase CPU
was 1.39/1.28% of one core. The earlier memory spike therefore did not persist
in this longer control; one extra pair does not establish a general idle gain.

## Portable acceptance and limitations

The default, non-experimental portable package was built from `4e2c6f611b1b`.
Its SHA-256 is
`0eaa420c55150f4e39847dc514c32725dd8bf6f3cc9284daa070056dec1dc00f`;
the inner publisher is
`e23c48bf8c6091798a9c4bdcc9ffd075164ab77ba8ee61e12a25459784cd61ef`.
Both PE subsystem values are Windows GUI (2); feature-only experiment markers
are absent. Runtime file/source pins, DLL imports and dependency/font notices
passed packaging checks. A forced bundled-runtime probe exited successfully.

An initial receive attempt from a deeply nested isolated cache exited before
producing native diagnostics. Its exact exit code was not retained by the old
harness and its cause is not established. Subsequent H.264 and HEVC controls
using a short, isolated Local AppData path each passed 30-second actual packaged
hardware receive checks, including stop/rewatch, with 1920×1080 pictures and
1,125/1,129 frames in their final watch generation. This is qualified acceptance;
the failed cache-path attempt is not hidden. The harness now records bounded
exit/failure classifications for future diagnostic failures. Launcher sampler
values are not interpreted as the native child's CPU/memory measurements.

These silent CLI controls do not establish packaged egui presentation or
physical A/V acceptance. No full audible suite was rerun. Previous campaign
limitations and external gates remain in force. Fresh review and CI approval of
the integrated range are required before merging [PR19](https://github.com/Felipebmaitan/freedomstream/pull/19).

## PR23 adaptation

PR19 merged as `feefc2f08a9daf4d6207265630bba59f69a9974e` after fresh independent
approval and successful Windows/Linux/container CI (optional native CI skipped).
PR23 includes that actual merge commit. The redesigned tile now allocates its
video rectangle before deciding whether to renew presentation demand and consume
its mailbox. Focus/fullscreen, the source picker and coordinated shutdown retain
PR19/PR22 behavior. The regression test covers clipped-to-visible resumption
without changing watch intent or volume.

The disabled planar presenter was adapted to the new video rectangle and
constructor fields, including fallback and GL resource cleanup. The feature-only
scripted-scroll hook was also ported into the new scroll area. These paths remain
experimental/off by default; earlier planar pixel/performance qualification is
historical, not new-UI renderer acceptance. Normal and all-feature Rust tests and
Clippy pass for the initial adaptation. Historical JSON cache behavior remains
separate from rendering and is being checked through the authenticated API.

Initial five-stream narrow-window A/B/B/A controls completed. All five tiles
remain at least partly visible in the redesigned layout: both variants convert
approximately 300 frames/s. The old UI's four/five-stream clipping improvement
does **not** reproduce in that situation. CPU and memory varied between runs;
there is no measured clipping win claimed for that layout. The unused four-stream
cases in the original batch were intentionally stopped at a completed-case
boundary (exit 20); no partial case is counted as evidence.

The benchmark-only stream allowance was extended to seven to exercise fully
clipped rows without changing the product's layout or watching policy.
[New-UI clipping/cache results](native-ui-clipping-results.json) and input hashes
are reproduced with `node scripts/summarize-ui-clipping.mjs`.

Seven-stream B/A/A/B comparisons, two runs per variant, consistently have six
intersecting video rectangles and one fully clipped rectangle. Decode delivery
remains approximately 419 frames/s for all seven watches. Conversion falls from
419.02 to 359.46 frames/s, avoiding the fully clipped stream's expansion.

| Seven-stream grid metric | Clipping off |  Clipping on |  Change |
| ------------------------ | -----------: | -----------: | ------: |
| CPU, one core = 100%     |      226.55% |      185.63% | −18.06% |
| GPU 3D engine activity   |       53.27% |       43.94% | −17.52% |
| Dedicated GPU memory     |   488.95 MiB |   476.49 MiB |  −2.55% |
| Private commit           | 1,137.72 MiB | 1,140.69 MiB |  +0.26% |
| Resident memory          |   603.52 MiB |   615.10 MiB |  +1.92% |

Both matched CPU pairs improve by approximately 18%. There is no demonstrated
RAM reduction. These are short background-window measurements, not foreground,
game, latency, physical display or packaged egui qualification. Occlusion was
unreported. All eight grid observations per seven-stream trial are retained.
GPU engine counters are separate, and lower egui handoffs for the hidden stream
are not measured physical GL upload or display counts.

The five-stream control remains a counterexample to extrapolating the old UI's
gains. One control has only three grid observations and unusually low CPU/GPU
activity; its CPU sampling window does not have matching application observation
coverage. All raw results and aggregate arithmetic remain included. Neither a
five-stream gain nor a causal regression is established by those mixed samples.

The shared server cache was also rechecked through the actual authenticated
HTTP routes: five alternating pairs, each with ten closed-history reports and
40 health requests, using copies of the preserved 522,000-row synthetic fixture.
Full response equivalence, fresh collector metadata, denial before login and
denial after logout pass in every trial. Median report latency falls from
836.85 to 3.89 ms; health p95 under that workload falls from 821.96 to 1.99 ms.
Server CPU for the fixed burst is median 8,124 versus 16 ms; cached samples range
from 0 to 62 ms, so timer granularity limits precise percentage interpretation.
Zero recorded milliseconds does not mean zero work. This is a warm historical
report win, not a normal live Today/Month dashboard, server RAM or WAN/VPS claim.
No historical evidence files or fixture databases were overwritten.

Final local checks pass: Node 24 full workspace checks, all-target/all-feature
Clippy, 59 default publisher tests, 60 all-feature publisher tests, nine portable
tests in each run, and both explicit silent Windows hardware tests. The package
from `a453145ab69a` has SHA-256
`a3c3b54401ab4a25fae1b6040187d4ec3eef904a104b1e7f628ec12230923d99`;
its inner publisher is
`b310cc56e58ff80d8361be0aa95c4992ddae0b18a8b4738f684903e558ec9d98`.
Both are GUI executables, experiment markers are absent, and the same pinned
runtime/source/import/license checks pass. Silent packaged H.264/HEVC hardware
receive passes (1,125/1,127 frames in the final watch generation) using a short
isolated cache; the forced bundled probe passes. No full audible or packaged
egui/physical A/V acceptance is claimed. Product sources are unchanged after
`4eadabc`; subsequent edits concern evidence/tooling only.

Fresh source review and PR23 CI remain required before merge. Main is refreshed
before merging. PR20 remains a draft with an **unwaived** native interaction gate;
it must not merge until those interactions are verified. A future combination
must retain its `browser_links` module import in `room-benchmark.rs` and
`with_browser_link(None)` construction, adapting the disabled planar qualifier's
constructor too. PR20's branch/checkout and E2EE PR21 remain untouched here.
