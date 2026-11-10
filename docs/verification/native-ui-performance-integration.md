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

Final package validation, fresh source review and PR checks are pending.

## PR23 adaptation

Pending PR19 integration. Retain only optimizations valid for the redesigned
layout and keep all experimental rendering/decoder paths disabled by default.
