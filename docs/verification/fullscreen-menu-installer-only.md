# Fullscreen menu and installer-only release correction

Verified on the Windows development PC, 2026-11-12:

- Node 24.11.1 `npm run check` passed (formatting, types, tests and builds).
- `npm run native:check` passed (Rust formatting and Clippy).
- `npm run native:test` passed, including the new fullscreen pointer-event test.
  The egui test opens the context menu after HUD inactivity, checks that its
  cursor remains visible after another idle timeout, clicks Mute and verifies
  that the menu stays open, and exercises Exit full screen, Focus stream,
  Stop watching and Advanced information.
- Fresh independent GPT-6.1-Sol review with high reasoning approved the patch
  without findings. Review and automated UI events do not establish physical
  media/hardware or installed Windows interaction qualification.

The existing [v0.1.0 release](https://github.com/Felipebmaitan/podetelar/releases/tag/v0.1.0)
was updated without creating a release or rebuilding binaries. Removed
`PodeTelar-v0.1.0-windows-x64.exe` and its `release-manifest.json`. Release notes
offer only Setup, and `SHA256SUMS` now lists Setup and `installer-manifest.json`.
Read-back verified exactly those three remaining assets and unchanged IDs,
sizes and SHA-256 digests for Setup and its manifest:

| Retained asset             | SHA-256                                                            |
| -------------------------- | ------------------------------------------------------------------ |
| PodeTelar-v0.1.0-Setup.exe | `60db0404551001b531f0cf30fb3a2c4b128e45b0d627241160bdb72f1b297d07` |
| installer-manifest.json    | `7a8aeed6e0c954d8e09027e632c5433bf13f3f5fa1090ba8e6d7e8522c2587df` |

The retained installer predates this fullscreen fix. This correction changes
source for the next planned build; it does not claim that v0.1.0 contains the fix.
The portable payload remains an internal installer build input.
