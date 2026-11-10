# Stream-first visual polish

Branch: `feat/stream-first-ui`, based on `9447d0b` plus the existing local work.
This pass responds to the gap between the generated reference and the first native
implementation, particularly typography and the stream context menu.

## Changes

- Bundled actual Inter regular and semibold faces rather than egui's default font.
  Titles, participant identity, primary actions and numeric readouts use distinct
  font weights. The OFL notice is embedded in Room options and copied to portable
  notices. See [font source and license](../../assets/fonts/README.md).
- Unified charcoal surfaces, borders, corner radii, spacing and mint accents.
  Removed heavy audio cards, made Advanced a quiet sidebar action, added outlined
  invitation and selected grid/focus buttons, and centered primary button labels.
- Replaced default menu buttons with aligned full-width rows, locally drawn icons,
  separate volume percentage, a mint native slider, mute switch, shortcut column,
  danger treatment, rounded border and shadow. Changing volume or mute leaves the
  menu open. Explicit actions close it; outside click and Escape also close it.
- Rounded stream video corners, compact name chips, real semibold participant
  names, stable individual avatar colors and aligned live badges. The two-column
  grid now fits the supported 960px viewport with the wider sidebar padding.
- Kept explicit subscription commands, start/minute preview cadence, no-microphone
  policy, automatic route, 30 Mbps unmeasured allowance and mouse-only codec/rate.

## Evidence

[Native captures](images/stream-first-polish/) cover standard, wide and compact
viewports, grid, hover, stream menu, sharing, onboarding/error, advanced settings
and reconnecting. The room is labeled DEMO: participants, game images, media state,
codec labels and rate values are synthetic. Capture fixtures establish native
rendering, not real media or recovery acceptance. Temporary instrumentation was
removed from shipping source before final checks/build.

Native formatting/Clippy and all 39 publisher tests plus portable tests passed.
The new switch test caught and verified a correction for Space activation after
pointer focus. Existing tests check hovered metrics and long room header names.
Node 24 `npm run check` passed, including formatting, types, API tests and builds.

An independent fresh GPT-6.1-Sol high technical reviewer approved this polish scope
with no blocking findings. It checked stable popup ownership, outside-click menu
behavior, keyboard/volume controls, subscription preservation and font notices.
The requested Gemini visual scoring remains unavailable for the image transport
reason recorded in [the initial pass](stream-first.md); no substitute score or
9/10 visual acceptance is claimed. Deployment/hardware qualification is unchanged.
