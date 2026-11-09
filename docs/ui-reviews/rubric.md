# Native UI and UX review rubric

Review the current FreedomStream Windows app through hands-on computer use. Keep
these categories unchanged between rounds. Each round uses a new Gemini 4 Argon
context; judge the current build independently rather than inheriting prior scores.

| Category                          | Assess                                                                                                              |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| Visual design                     | Hierarchy, typography, color, spacing, consistency and polish.                                                      |
| Layout and responsiveness         | One/multiple stream layouts, useful preview sizes, density, resizing and scrolling.                                 |
| Room onboarding                   | Joining, identity, understanding the room, saved-room behavior and leaving/rejoining.                               |
| Sharing workflow                  | Source selection, resolution, visible FPS, presets, source-derived audio, advanced settings and start/stop clarity. |
| Watching workflow                 | Discovering streams, Watch/Stop, multiple subscriptions, focus/grid, volume and clear preview/live distinctions.    |
| Feedback and recovery             | Connected/connecting/reconnecting, busy, empty/loading, invalid input, errors and actionable recovery.              |
| Accessibility and discoverability | Contrast, legibility, labels, keyboard focus/navigation, target sizes and accessible semantics.                     |

Score each category from 0–10 and justify it with concrete observations. Nine
means polished and readily usable on the exercised flows, with only minor optional
improvements left. Ten is exceptional. Identify remaining friction even at high
scores. Do not raise a score merely because a prior round requested improvement.
An interaction-specific category that could not be exercised must be marked
unverified, not awarded a passing score from screenshots or source inspection.

The requested completion condition is an independent fresh-context review giving
at least 9/10 in every category. Subjective design scores do not replace hardware,
network, security, media or field acceptance checks.

Each report should contain the exact build/commit reviewed, test environment,
actions performed and states observed; a score table; prioritized concrete
findings with reproduction steps and suggested improvements; and blockers/limits.
Use synthetic local streams. Never include invitations, session/media tokens,
credentials, private app/window inventories or screenshots exposing room authority.

Preserve product requirements: native Windows rendering/playback without a browser;
left sharing controls and larger right stream region; centered Watch; source
resolution capped at 4K SDR; visible FPS; codec/bitrate in Advanced; automatic
source audio with Discord/owned playback exclusion and no microphone; explicit
Watch/Stop subscriptions; start/minute preview cadence; and encrypted saved-room
autojoin without automatic sharing/watching.
