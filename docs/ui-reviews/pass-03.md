# Post-review connection and compact-window corrections

Round 4 met the seven-category 9/10 image-review target for `d2573ad`. A separate
fresh GPT-6.1-Sol high technical review then found two P2 regressions that the
provided static states did not reveal. Both were independently confirmed before
changes:

1. Advanced could only be opened from the connected sharing sidebar. This made
   Connection route → Relay only unreachable after a failed initial Automatic
   join. **Fix:** Room options now exposes Advanced settings in every room state,
   including before joining. The existing connected sidebar shortcut remains.
2. At 1280×540, height-based grid fitting could shrink cards to 202px while their
   unwrapped playback controls needed approximately 270px. **Fix:** retain a
   minimum 302px outer card width and allow vertical scrolling when the window is
   too short to display two rows. The default four-card 1280×720 layout still fits.

No transport, capture, audio, preview, admission or persistence contract changes.
The new `images/pass-03/short-watching.png` exercises actual local native watched
streams at 1280×540, including an Unmute control. Other captures cover the same
13 states as Round 4, including the revised Room options menu. An additional
`prejoin-settings.png` shows the existing Advanced renderer over a disconnected
join-form fixture, with Connection route visible. As before, selected
lifecycle screens use controlled UI fixtures and color bars are synthetic content.

The technical review reported no additional material correctness, privacy or
cancellation-cleanup defect in its scope. Its approval was pending these two
fixes; do not describe it as an unconditional phase/release approval. Native
formatting/Clippy passed after the fixes. The parent verifies the new framebuffer
geometry and removes temporary capture hooks before the normal clean build.

A new independent Gemini image-only review must judge this final correction
pass. Previous Round 4 scores remain valid for its recorded commit, not an
automatic score for changed screenshots. External hardware/deployment and field
acceptance gates remain open; PR #10 remains draft.
