# Stream-first native UI

This is the historical first-pass report. See [the PR summary](pr-summary.md)
for the final scope, screenshots and branch isolation from current `main`.

The current typography/menu refinement and refreshed native captures are recorded
in [the visual polish pass](stream-first-polish.md).

Branch: `feat/stream-first-ui`. Implements the approved generated concept with
codec immediately before received bitrate in the hover HUD. This is a UI change;
it does not qualify hardware, codec performance, audio exclusion or deployment.

## Behavior

- Compact charcoal sharing/member sidebar, stream surfaces without permanent
  headers/footers, overlaid participant identity and live/preview distinction.
- Watch remains an explicit subscription. Right-click or the hovered ellipsis
  opens volume, mute, focus, fullscreen and Stop watching. Focus and fullscreen
  retain other subscriptions. Stop sends the existing unsubscribe command.
- Received publication MIME types map to HEVC, H.264 and AV1. Unknown codecs use
  a dash. The rate remains the existing measured receive rate, not a target.
  Both labels disappear on pointer exit; previews have neither. Fullscreen also
  requires pointer presence and fades with its existing controls timeout.
- Sharing controls retain independent source, resolution, FPS and quality.
  In short windows sharing actions are pinned. Source confirmation is a
  collapsible existing minute preview after settings; expanding it never captures
  or uploads another frame. No idle capture is introduced.
- Room names truncate within a bounded header button, with the full label on
  hover. Invitations remain in the existing clipboard flow.
- Automatic watching allowance falls back to 30 Mbps when no download measurement
  is available, retaining the existing audio/transport headroom. Valid measurements
  and manual limits still take precedence. The connection-route selector was
  removed; both saved-room and manual joins use automatic routing.

## Verification

Windows native Clippy/formatting passed. Native workspace tests passed: 37 publisher
tests plus portable tests. Added renderer-level regression tests for pointer
entry/exit, unsubscribed metrics suppression, codec formatting, fullscreen without
a pointer, and header actions at 960px with a 60-character room name.

An independent GPT-6.1-Sol high review approved the UI scope after fixes for long
room names and mouse-only fullscreen metrics. No remaining actionable findings
were reported. Node 24 `npm run check` passed, including formatting, types, API
tests and builds. A normal native debug build also passed after capture hooks
were removed.

Gemini 4 Argon high was requested for the required independent visual review.
It withheld all seven ratings because its Antigravity ACP file readers returned
PNG bytes as text; the HTML image tool also offloaded images without delivering
multimodal input. It reported no visual inspection. Visual sign-off remains
pending; no 9/10 completion or merge approval is claimed. The delegation interface
does not accept image attachments in its task arguments. No alternate provider
was substituted for the requested visual reviewer.

## Capture evidence and limits

[Native-renderer images](images/stream-first/) include grid, hover, menu, narrow,
focus, onboarding, error, advanced, empty, sharing, short-sharing and reconnecting.
The room is labeled DEMO. Names, rates, codec labels, connection state and frames
are synthetic fixtures. Frames come from the supplied screenshot/generated concept.
These images prove rendering only, not actual live media or interaction acceptance.
Temporary instrumentation was removed from production source before final checks.
The capture set predates the follow-up that removed the connection-route selector
and changed the unmeasured watching allowance to 30 Mbps.
That follow-up passed native formatting/Clippy, all 38 publisher tests and the
portable tests, plus Node 24 `npm run check`. Its regression test checks the
30 Mbps fallback, existing headroom, and measured/manual limit precedence.

The checkout contained ongoing media, fullscreen, AV1 and packaging edits before
this task. Those files remain preserved; this UI consumes their existing receive
rate/fullscreen behavior. No unrelated edits have been staged, committed or merged.
The design branch remains local while those prerequisites are uncommitted.
Another task switched the original shared checkout to `feat/admin-rooms` during
this work. The design branch is therefore preserved in a separate worktree at
`.local/stream-first-ui-worktree`, with the native prerequisites copied and local
SDK/runtime dependencies linked. UI source hashes match the checked build.
The redesign-only patch was reversed out of the shared UI/language files after
the copy, preserving their pre-task contents for the concurrent admin task.
