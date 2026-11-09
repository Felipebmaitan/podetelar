# Native UI and UX review rubric

Review native-renderer screenshots of the current FreedomStream Windows app. Keep
these categories unchanged between rounds. Each round uses a new Gemini 4 Argon
context; judge the current images independently rather than inheriting prior scores.

Felipe changed the review scope after Round 1: reviews are **UI-only**, judging
how good the app looks and the UX implied by the images. No direct desktop tool
is required. Clearly distinguish image-derived usability judgments from verified
interactive behavior. A high visual UX score does not certify a working flow.

| Category                          | Assess                                                                                                                                                   |
| --------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Visual design                     | Hierarchy, typography, color, spacing, consistency and polish.                                                                                           |
| Layout and responsiveness         | One/multiple stream layouts, useful preview sizes, density, visible overflow and screenshots at different viewport sizes.                                |
| Room onboarding                   | Joining, identity, understanding the room, visible saved-room messaging and leave/rejoin affordances.                                                    |
| Sharing workflow                  | Source selection, resolution, visible FPS, presets, source-derived audio, advanced settings and start/stop clarity.                                      |
| Watching workflow                 | Discovering streams, Watch/Stop, multiple subscriptions, focus/grid, volume and clear preview/live distinctions.                                         |
| Feedback and recovery             | Connected/connecting/reconnecting, busy, empty/loading, invalid input, errors and actionable recovery.                                                   |
| Accessibility and discoverability | Contrast, legibility, labels, visible focus/labels, target sizes and discoverability. Keyboard and screen-reader operation cannot be proved by an image. |

Score each category from 0–10 and justify it with concrete observations. Nine
means polished visible design and readily understandable image-derived UX across
the provided states, with only minor optional improvements left. Ten is exceptional. Identify remaining friction even at high
scores. Do not raise a score merely because a prior round requested improvement.
Rate image-derived UX for all seven categories, while explicitly withholding
claims about unobserved interactions, keyboard/screen-reader operation, network
recovery or media performance. Missing visual state evidence may lower confidence
or justify requesting another screenshot; it must not be invented.

The requested completion condition is an independent fresh-context review giving
at least 9/10 in every category. Subjective design scores do not replace hardware,
network, security, media or field acceptance checks.

Each report should contain the exact build/commit reviewed, test environment,
screenshots inspected and visible states; a score table; prioritized concrete
findings with reproduction steps and suggested improvements; and blockers/limits.
Use clearly labelled synthetic stream previews or demo UI state data. Never include invitations, session/media tokens,
credentials, private app/window inventories or screenshots exposing room authority.

Preserve product requirements: native Windows rendering/playback without a browser;
left sharing controls and larger right stream region; centered Watch; source
resolution capped at 4K SDR; visible FPS; codec/bitrate in Advanced; automatic
source audio with Discord/owned playback exclusion and no microphone; explicit
Watch/Stop subscriptions; start/minute preview cadence; and encrypted saved-room
autojoin without automatic sharing/watching.
