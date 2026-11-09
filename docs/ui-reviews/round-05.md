# FreedomStream UI & Image-Inferred UX Review — Round 5

## Metadata & Review Context

- **Reviewer:** Independent Gemini 4 Argon UI-only Reviewer (Fresh Context, Round 5)
- **Repository / Path:** `C:/Users/Felipe/Documents/Coding/freedomstream`
- **Branch / Commit:** `feat/native-rooms` @ `f491a11`
- **Target Platform:** Native Windows desktop application (native renderer, no browser chrome)
- **Review Scope:** UI-only visual design and image-inferred UX evaluation across 15 actual multimodal PNG attachments delivered in two initial-evidence batches (8 + 7). No source code, prior review rounds, logs, or live runtime execution were inspected.

---

## 1. Preflight Pixel-Access Verification

Actual multimodal pixel inspection is confirmed across all 15 attached PNG screenshots. Answers to the mandatory preflight visual verification checks:

1. **`short-watching.png` — Leftmost live stream details:**
   - **Stream name:** `Sam · Game` (paired with a green `Live` badge in the top header of the upper-left stream card).
   - **Mute/unmute action button text:** `Unmute` (located between `Focus` and `Vol` in the bottom control bar of the `Sam · Game` card).
   - **Volume percentage:** `0%` (displayed in the pill readout to the right of the volume slider thumb at the minimum left position).
2. **`prejoin-settings.png` — Connection-status badge behind modal:**
   - **Badge text:** `Not in a room` (dimmed gray pill in the upper-left status bar beneath the `FreedomStream` brand title, to the left of `"Private by invitation. Sharing and watching are always your choice."`).
3. **`room-options.png` — Menu action between saved-access explanation and saved-room removal:**
   - **Menu action text:** `Advanced settings` (positioned directly below `"Leaving keeps any saved room for next launch."` and above a horizontal divider and `"Forget saved room"`).

---

## 2. Screenshots Inspected & Visible States

All 15 screenshots were inspected directly as rendered images:

| Filename               | Approx. Viewport | Visible Application State & Evidence Type                                                                                                                                                                                                                                                                                                                                                                                        |
| :--------------------- | :--------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `streams.png`          | 1280×720         | Connected (`Joined as Alex`, `FreedomStream demo`, `6 in room`); left `Share your screen` panel idle (`Start sharing`); right `Friends' streams` 2×2 grid (`4 streams · 0 watching`) showing four preview cards (`Sam · Game`, `Jamie · Desktop`, `Taylor · Workspace`, `Casey · Screen`) with `Preview` badges, centered `Watch` CTAs, and `"Still preview · updates up to once a minute"` footers.                             |
| `watching.png`         | 1280×720         | Connected (`6 in room`, `4 streams · 2 watching`); top row (`Sam · Game`, `Jamie · Desktop`) actively watched with green `Live` badges, animated-style SMPTE noise patch in lower-right corner, and bottom control rows (`Stop`, `Focus`, `Mute`, `Vol` slider at `100%`); bottom row (`Taylor · Workspace`, `Casey · Screen`) remains in `Preview` state with centered `Watch` buttons.                                         |
| `one.png`              | 1280×720         | Connected (`7 in room`, `1 stream · 0 watching`); single large `Sam · Game` preview card filling the main `Friends' streams` canvas with centered `Watch` button over the 16:9 color-bar canvas.                                                                                                                                                                                                                                 |
| `focus.png`            | 1280×720         | Connected (`7 in room`); right pane switched to `Focused stream` header with a `Back to grid` button at top-right; large single live stream card (`Sam · Game`, `Live` badge) with bottom controls (`Stop`, `Mute`, wide `Vol` slider at `100%`).                                                                                                                                                                                |
| `narrow.png`           | 960×540          | Compact/narrow viewport (`8 in room`, `4 streams · 0 watching`); single-column stream layout with `"Scroll to see all streams"` helper text, visible vertical scrollbars on both the left sharing sidebar and the right stream pane, and `Start sharing` pinned at the bottom of the left sidebar.                                                                                                                               |
| `advanced.png`         | 1280×720         | Connected (`5 in room`) with the centered native `Advanced settings` modal open over the dimmed workspace; displays `Close` button, `"Publishing settings apply with Start sharing or Apply changes."`, `Codec / hardware encoder` (`NVIDIA NVENC HEVC`), `Video bitrate` (`Automatic`), `Watching allowance` (`Automatic · 12 Mbps`), `Retry watching connection`, `Connection route` (`Automatic`), and primary `Done` button. |
| `onboarding.png`       | 1280×720         | Disconnected empty pre-join state (`Not in a room`, `Your private room`); left `Join your room` panel with empty `Room invitation` (`Show` button, `"Paste your private invitation link"`), empty `Your name` (`"How friends will see you"`), disabled `Join room` button, encrypted saved-room explanation, and center empty-state guidance (`"A room for your friends' screens"`).                                             |
| `joining.png`          | 1280×720         | Connecting fixture (`Joining room` amber badge, `Cancel join` top-right button); full-width progress banner (`"Joining your private room..."` with spinner arc); masked `Room invitation` dots, `Your name` (`Alex`), disabled `Join room` button.                                                                                                                                                                               |
| `error.png`            | 1280×720         | Failed-join fixture (`Not in a room`); full-width red error banner (`"We couldn't reach this room. Check your connection and try joining again."`); `Room invitation` (masked) and `Your name` (`Alex`) preserved with active teal `Join room` button ready for immediate retry.                                                                                                                                                 |
| `empty.png`            | 1280×720         | Connected empty-streams fixture (`Connected`, `Joined as Alex`, `5 in room`, `0 streams · 0 watching`); right pane centered message: `"You're in. Make yourself at home. / Friends are here. Their streams appear when they start sharing. / Previews first. Live video and audio only when you choose Watch."`                                                                                                                  |
| `sharing.png`          | 1280×720         | Active screen-sharing fixture (`Connected`, green `Sharing` badge, `Joined as Alex`, `5 in room`); bottom of left panel displays green status line `"Sharing 1920×1080 · 60 FPS"`, red/burgundy `Stop sharing` button, and disabled `Apply changes` button.                                                                                                                                                                      |
| `dirty.png`            | 1280×720         | Active screen-sharing with unapplied edits (`Connected`, `Sharing`); left panel controls show `60 FPS` while active status line reads `"Sharing 1920×1080 · 30 FPS"`; `Apply changes` is highlighted in active teal beside `Stop sharing`, with amber warning `"Changes pending · restarts stream"` below.                                                                                                                       |
| `room-options.png`     | 1280×720         | Connected (`5 in room`) with top-right `Room options` dropdown open, showing header `"Room and saved access"`, note `"Leaving keeps any saved room for next launch."`, action `"Advanced settings"`, divider, and action `"Forget saved room"`.                                                                                                                                                                                  |
| `short-watching.png`   | 1280×540         | Short-height connected watching view (`5 in room`, `4 streams · 2 watching`); top row (`Sam · Game` live & muted at `0%` showing `Unmute`; `Jamie · Desktop` live at `100%` showing `Mute`) fully visible with controls; bottom row headers visible with vertical scrollbar on the right pane; left sidebar also displays a scrollbar above the pinned `Start sharing` footer.                                                   |
| `prejoin-settings.png` | 1280×720         | Disconnected join-form fixture (`Not in a room`, filled invitation and `Alex`) with the native `Advanced settings` modal open on top, proving pre-join access to `Codec / hardware encoder`, `Video bitrate`, `Watching allowance`, and `Connection route`.                                                                                                                                                                      |

---

## 3. Category Scores & Confidence Summary

| Category                              | Score (0–10) | Confidence  | Summary Justification                                                                                                                                                                                                                                                                                                |
| :------------------------------------ | :----------: | :---------: | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Visual design**                     | **9.0 / 10** |    High     | Cohesive dark slate palette, crisp typographic hierarchy, semantic status colors (teal primary, green live/connected, amber pending/joining, red error/stop), and clean card framing. Minor popover overlap and audio callout clipping at 540px height remain.                                                       |
| **Layout and responsiveness**         | **8.5 / 10** |    High     | Excellent 2×2 grid (`streams.png`), single-stream expansion (`one.png`, `focus.png`), and pinned primary footer CTA (`narrow.png`, `short-watching.png`). Held below 9 due to the `Audio follows your source` box clipping its second line at 540px height and `Room options` dropdown obscuring the stream counter. |
| **Room onboarding**                   | **9.5 / 10** |    High     | Crystal-clear pre-join flow (`onboarding.png`), password-style masking with `Show` toggle (`joining.png`), explicit encrypted local persistence messaging, pre-join `Room options` access, and reassuring empty room state (`empty.png`).                                                                            |
| **Sharing workflow**                  | **9.0 / 10** |    High     | Directly satisfies all product constraints: source/window selector with `Refresh`, `Source · up to 4K`, prominent discrete `Frame rate` slider (`60 FPS`), `Quality preset`, explicit automatic audio rules, and unambiguous `Sharing` / `Changes pending · restarts stream` states (`sharing.png`, `dirty.png`).    |
| **Watching workflow**                 | **9.5 / 10** |    High     | Effortless opt-in model: centered `Watch` button on 16:9 canvas, clear `Preview` vs. `Live` badges, independent multi-stream controls (`Stop`, `Focus`, `Mute`/`Unmute`, `Vol` 0–100%), and dedicated `Focused stream` view with `Back to grid`.                                                                     |
| **Feedback and recovery**             | **9.0 / 10** |    High     | Strong state visibility across `Joining room` (`Cancel join`), non-destructive error banner (`error.png`), dirty-setting warning (`dirty.png`), and `Retry watching connection` in `Advanced settings`. Minor ambiguity around having `Retry watching connection` enabled while `Not in a room`.                     |
| **Accessibility and discoverability** | **9.0 / 10** | Medium-High | High text contrast, explicit text labels on every control (no mystery icons), dual state cues (`Unmute` + `0%`, `Live` + control bar), and discoverable pre-join settings via `Room options`. Minor vertical clipping of the audio note at 540px height.                                                             |

_(Note: Confidence is High for visual layout and static UX clarity, and Medium-High where static screenshots cannot verify keyboard navigation, focus rings, or live scroll behavior.)_

---

## 4. Detailed Per-Category Evaluation

### 4.1. Visual Design — **9.0 / 10** (Confidence: High)

- **Facts (Visible Evidence):**
  - The UI uses a disciplined dark theme: deep navy-charcoal backgrounds, slightly lighter elevated card surfaces with subtle 1px rounded borders (`streams.png`), and a vibrant mint-teal accent for primary actions (`Start sharing`, `Watch`, `Join room`, `Apply changes`, `Done`).
  - Semantic status badges are distinct by both color and label: green-tinted `Connected`, `Sharing`, and `Live` (`watching.png`, `sharing.png`); amber-tinted `Joining room` (`joining.png`); muted gray `Not in a room` and `Preview` (`onboarding.png`, `streams.png`); and a muted crimson `Stop sharing` button (`sharing.png`).
  - Centered `Watch` buttons in `streams.png`, `one.png`, and `narrow.png` sit at the geometric center of the full 16:9 stream canvas (including the lower dark test-pattern bands).
  - In `room-options.png` (top-right region), the open `Room options` dropdown menu overlaps the underlying `5 in room` badge and partially occludes the `"4 streams · 0 watching"` counter (`"eams · 0 watching"` remains visible sticking out to the right of the menu border).
- **Preferences / Polish Opportunities:**
  - Giving the `Room options` popover a slightly stronger drop shadow or aligning its right edge flush with the right edge of `Leave room` would avoid awkward partial text collisions with `"4 streams · 0 watching"`.

### 4.2. Layout and Responsiveness — **8.5 / 10** (Confidence: High)

- **Facts (Visible Evidence):**
  - **1280×720 Multi-Stream & Single-Stream Layouts:** `streams.png` and `watching.png` allocate a fixed-width left sharing sidebar (~24% width) and a spacious right stream region (~76% width) housing a balanced 2×2 grid. When only one stream is present (`one.png`) or a stream is focused (`focus.png`), the single card scales up cleanly to utilize the right pane while preserving its 16:9 aspect ratio.
  - **1280×540 Compact Height (`short-watching.png`):** The top row of live watched streams (`Sam · Game` and `Jamie · Desktop`) fits completely—including their header badges, 16:9 video canvases, and bottom control bars (`Stop`, `Focus`, `Unmute`/`Mute`, `Vol` slider)—while the second row (`Taylor · Workspace`, `Casey · Screen`) peeks cleanly at the bottom with a visible vertical scrollbar on the right edge.
  - **960×540 Narrow Viewport (`narrow.png`):** The right stream grid adapts to a 1-column vertical stack with a helpful `"Scroll to see all streams"` subtitle under `Friends' streams` and a visible scrollbar thumb on the right edge.
  - **Left Sidebar Overflow Issue at 540px Height (`narrow.png` and `short-watching.png`, bottom-left region):** While `Start sharing` remains pinned at the bottom of the left panel and a vertical scrollbar appears along the sidebar divider, the dark callout card (`"Audio follows your source"`) is vertically clipped at the bottom edge—cutting off `"Discord and FreedomStream excluded."` mid-glyph and hiding `"Microphone off."` entirely before the horizontal divider above `Start sharing`.
- **Unknowns (Static Image Limits):**
  - Static screenshots confirm the presence of scrollbars in `narrow.png` and `short-watching.png`, indicating overflow is reachable, but cannot verify scroll smoothness or mouse-wheel behavior.

### 4.3. Room Onboarding — **9.5 / 10** (Confidence: High)

- **Facts (Visible Evidence):**
  - **Pre-Join Clarity (`onboarding.png`):** The header clearly states `Not in a room` alongside `"Private by invitation. Sharing and watching are always your choice."` The left panel (`Join your room`) requires only two inputs: `Room invitation` (with a dedicated `Show` button) and `Your name`, followed by a disabled `Join room` button when empty.
  - **Saved-Room & Privacy Transparency (`onboarding.png`, `room-options.png`):** Below the join button, two concise notes explain: `"Your last joined room and name are encrypted on this PC and remembered for next launch."` and `"Saved access can be removed in Room options above."` Opening `Room options` (`room-options.png`) reinforces this with `"Leaving keeps any saved room for next launch."` and an explicit `"Forget saved room"` action.
  - **Invitation Masking (`joining.png`, `error.png`, `prejoin-settings.png`):** Pasted invitation tokens are masked with bullets by default so room credentials are not exposed on screen, paired with the `Show` button for verification.
  - **Connected Empty State (`empty.png`):** Once connected (`Joined as Alex`, `5 in room`), when no one is streaming yet, the center canvas reassures the user: `"You're in. Make yourself at home. / Friends are here. Their streams appear when they start sharing. / Previews first. Live video and audio only when you choose Watch."`

### 4.4. Sharing Workflow — **9.0 / 10** (Confidence: High)

- **Facts (Visible Evidence):**
  - **Core Controls (`streams.png`, left sidebar):** All primary sharing controls are immediately accessible without opening a modal: `Screen or window` dropdown (`Monitor 1 · 1920×1080`) with an adjacent `Refresh` button, `Resolution` (`Source · up to 4K`), prominent `Frame rate` slider with discrete `60 FPS` pill readout and helper note (`"15–120 FPS · higher frame rates use more upload"`), `Quality preset` (`Games / motion`), live summary line (`1920×1080 · 60 FPS · SDR`), and the automatic source-audio card (`"Audio follows your source / Discord and FreedomStream excluded. / Microphone off."`).
  - **Active & Dirty Sharing States (`sharing.png`, `dirty.png`):** While publishing (`sharing.png`), a green `Sharing` badge appears in the top bar, a green status line `"Sharing 1920×1080 · 60 FPS"` appears above `Stop sharing`, and `Apply changes` is disabled. When a setting is modified mid-stream (`dirty.png`, where the UI is set to `60 FPS` while actively `"Sharing 1920×1080 · 30 FPS"`), `Apply changes` turns bright teal and an amber warning `"Changes pending · restarts stream"` clearly explains the consequence of applying.
  - **Advanced Publishing Controls (`advanced.png`):** Codec (`NVIDIA NVENC HEVC` with compatibility fallback note `"HEVC switches to H.264 when a watcher needs compatibility."`) and `Video bitrate` (`Automatic`, `"up to 35 Mbps. Manual: 1–100 Mbps"`) are cleanly housed in `Advanced settings` with the top note `"Publishing settings apply with Start sharing or Apply changes."`

### 4.5. Watching Workflow — **9.5 / 10** (Confidence: High)

- **Facts (Visible Evidence):**
  - **Preview vs. Live Distinction (`streams.png`, `watching.png`):** Unwatched streams display a muted `Preview` badge, a centered `Watch` button on the 16:9 canvas, and the footer `"Still preview · updates up to once a minute"`. Active streams (`Sam · Game`, `Jamie · Desktop` in `watching.png`) switch to a green `Live` badge, remove the overlay button, and replace the footer text with a dedicated control bar (`Stop`, `Focus`, `Mute`, `Vol` slider, `100%`).
  - **Independent Multi-Stream Audio & Mute (`watching.png`, `short-watching.png`):** Each watched stream has its own independent volume slider and mute toggle. In `short-watching.png`, `Sam · Game` is muted (`Unmute` button, slider at left, `0%` readout) while `Jamie · Desktop` plays at `100%` (`Mute` button, slider at right, `100%` readout).
  - **Focus & Grid Switching (`focus.png`):** Clicking `Focus` expands the stream into the `Focused stream` view with a clear `Back to grid` button at the top-right and a wider `Vol` slider in the bottom control bar, while keeping `Stop` and `Mute` in the exact same relative order.
- **Preferences / Minor Friction:**
  - In `focus.png`, the top-right header replaces the `"4 streams · 2 watching"` summary with `Back to grid`, so a user in Focus mode cannot see at a glance if another stream is still being watched in the background.

### 4.6. Feedback and Recovery — **9.0 / 10** (Confidence: High)

- **Facts (Visible Evidence):**
  - **Joining & Cancellation (`joining.png`):** Displays an amber `Joining room` status pill, a full-width `"Joining your private room..."` banner with a loading spinner arc, disabled form inputs/submit button to prevent double submission, and a top-right `Cancel join` button to abort a hung connection attempt.
  - **Failed Join & Recovery (`error.png`):** Displays a clear, non-jargon red alert banner (`"We couldn't reach this room. Check your connection and try joining again."`) while preserving the user's `Room invitation` and `Your name` (`Alex`) and re-enabling the primary `Join room` button for one-click retry.
  - **Advanced Troubleshooting (`advanced.png`, `prejoin-settings.png`):** Provides a `Retry watching connection` button under `Watching allowance` and a `Connection route` selector (`Automatic`, `"Route applies the next time you join."`).
- **Minor Observation:**
  - In `prejoin-settings.png`, when `Advanced settings` is opened while `Not in a room`, the `Retry watching connection` button still appears enabled/interactive even though no room or watching session is active.

### 4.7. Accessibility and Discoverability — **9.0 / 10** (Confidence: Medium-High)

- **Facts (Visible Evidence):**
  - **Explicit Text Labels:** Every action in the application uses clear text labels rather than icon-only buttons (`Copy invitation`, `Room options`, `Leave room`, `Advanced`, `Refresh`, `Watch`, `Stop`, `Focus`, `Mute`/`Unmute`, `Back to grid`, `Show`, `Close`, `Done`).
  - **Pre-Join Settings Discoverability (`onboarding.png`, `room-options.png`, `prejoin-settings.png`):** Even before joining a room (`onboarding.png`), `Room options` is available in the top-right header. As shown in `room-options.png`, its menu contains `Advanced settings`, making pre-join codec, bitrate, watching allowance, and connection route configuration (`prejoin-settings.png`) discoverable without cluttering the simple `Join your room` sidebar.
  - **Redundant Visual Cues:** State changes do not rely on color alone—e.g., muted audio in `short-watching.png` changes both the button label (`Mute` → `Unmute`) and the numeric badge (`100%` → `0%`); dirty sharing settings in `dirty.png` compare `"60 FPS"` in the control against `"Sharing 1920×1080 · 30 FPS"` in green and add `"Changes pending · restarts stream"` in amber.
  - **Contrast & Target Sizes:** Primary and secondary text contrast strongly against the dark surfaces, and buttons/dropdowns have comfortable heights (~32–38px estimated).
- **Unknowns (Static Image Limits):**
  - Keyboard tab order, focus-ring visibility on keyboard navigation, and screen-reader accessibility tree announcements cannot be evaluated from static screenshots.

---

## 5. Prioritized Actionable Findings

### P1 (Medium) — Left Sidebar Audio Info Box Clips Text at 540px Viewport Height

- **Evidence:** `narrow.png` (960×540, bottom-left region above `Start sharing`) and `short-watching.png` (1280×540, bottom-left region above `Start sharing`).
- **Observation (Fact):** At a 540px window height, the scrollable upper region of the left `Share your screen` sidebar ends immediately above the pinned `Start sharing` footer. The dark callout box (`"Audio follows your source"`) is partially cut off at its bottom edge: the second line (`"Discord and FreedomStream excluded."`) is sliced horizontally through the bottom of the letters, and the third line (`"Microphone off."`) is hidden.
- **Why It Matters:** Although the sidebar has a scrollbar, the clipping occurs mid-line inside a bordered card without a visual fade or sufficient bottom padding above the sticky footer divider, making the text look unintentionally truncated rather than cleanly scrollable.
- **Suggested Improvement:** Slightly compact vertical spacing between left-sidebar form groups on short viewports (e.g., tightening label-to-input margins by ~2–4px) so the entire `Audio follows your source` card fits at 540px height without scrolling, or add bottom scroll padding / a subtle bottom scroll shadow above the pinned `Start sharing` footer so text does not clip mid-line.

### P2 (Low) — `Room options` Dropdown Overlaps Right-Pane Stream Counter and Participant Pill

- **Evidence:** `room-options.png` (top-right region under `Room options` button).
- **Observation (Fact):** When the `Room options` menu is open, its floating container extends downward over the lower header row and top of the `Friends' streams` pane, partially covering `"4 streams · 0 watching"` so that `"eams · 0 watching"` pokes out from behind the right border of the menu.
- **Suggested Improvement:** Right-align the `Room options` menu with the right edge of the `Leave room` button (or widen the menu slightly and give it a subtle backdrop/elevation shadow) so it cleanly covers or avoids colliding mid-word with `"4 streams · 0 watching"`.

### P3 (Low) — `Retry watching connection` Appears Active When Disconnected (`Not in a room`)

- **Evidence:** `prejoin-settings.png` (center `Advanced settings` modal, below `Watching allowance`).
- **Observation (Fact):** When `Advanced settings` is opened prior to joining a room (`Not in a room` badge visible behind the backdrop), the `Retry watching connection` button is rendered with the same active border and text contrast as when connected in `advanced.png`.
- **Suggested Improvement:** Visually disable `Retry watching connection` (similar to the disabled `Join room` button in `onboarding.png` or `Apply changes` in `sharing.png`) when the user is not currently connected to a room, or append a brief hint that it applies while connected.

### P4 (Low / Optional Polish) — Keep Active Watch Count Visible in `Focused stream` View

- **Evidence:** `focus.png` (top-right of the right pane, where `Back to grid` replaces `"4 streams · 2 watching"`).
- **Observation (Preference):** In `focus.png`, the stream summary counter (`"X streams · Y watching"`) is hidden or replaced by the `Back to grid` button. Displaying the summary text to the left of `Back to grid` (e.g., `"4 streams · 1 watching"`) would let users know whether other streams are still active in the background while focused on one stream.

---

## 6. UI-Only Review Limits & Non-Claims

In accordance with the review rubric and scope constraints:

1. **Static Visual Evidence Only:** All assessments are derived strictly from the 15 static PNG screenshots provided. `streams.png`, `watching.png`, `one.png`, `focus.png`, `narrow.png`, `advanced.png`, `onboarding.png`, `room-options.png`, and `short-watching.png` reflect rendered application states, while `joining.png`, `error.png`, `empty.png`, `sharing.png`, `dirty.png`, and `prejoin-settings.png` are evaluated as visual state fixtures.
2. **No Runtime / Media / Network Certification:** High UI and image-inferred UX scores do not verify live WebRTC/media transport, actual 4K/60–120 FPS capture pacing, NVENC hardware encoding fallback, audio loopback exclusion of Discord/FreedomStream, DPAPI encryption on disk, or network reconnection behavior.
3. **Unobserved Interactions & Accessibility Limits:** Static images cannot verify keyboard tab order, focus-ring behavior during keyboard navigation, screen-reader announcements, dropdown menu item lists when clicked (`Screen or window`, `Resolution`, `Quality preset`, `Codec`, `Video bitrate`, `Watching allowance`, `Connection route`), or interactive scrolling physics in `narrow.png` and `short-watching.png`.
