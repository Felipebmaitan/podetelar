# Native UI and performance integration

## Scope and qualification

PR19 is being integrated with native UI PR22 at immutable head
`21a9efae61434612c56e1824149877e9b15839e1`. PR23 will remain a separate,
subsequent change. Neither performance PR may merge until PR22 is confirmed
merged and its commit is present in `origin/main`.

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

Silent application measurements, final source review and PR checks are pending.
Runtime recovery uses a separate local SDK directory; no missing runtime
directory in the original checkout is modified. The installer matches the
existing SHA-256 pin. The recovered SDK is a build prerequisite, not a newly
qualified redistributable runtime. Execution uses a clean copy of the 44 preserved
runtime files, each verified against the existing distribution manifest.

## PR23 adaptation

Pending PR19 integration. Retain only optimizations valid for the redesigned
layout and keep all experimental rendering/decoder paths disabled by default.
