# FreedomStream UI-Only Review — Round 3

- **Reviewer**: Independent Gemini 4 Argon (Fresh Context, UI-Only Scope)
- **Build / Commit**: Application code `0ed3377`, Documentation HEAD `ebc5beb`, branch `feat/native-rooms`
- **Workspace**: `C:/Users/Felipe/Documents/Coding/freedomstream`
- **Review Scope**: Multimodal visual inspection of 12 attached PNG screenshots. Evaluates visual design and image-inferred UX only. Interactive execution, media/network performance, and keyboard/screen-reader accessibility are outside scope and unverified.

---

## 1. Visual Preflight Verification

Actual multimodal image access was established across all 12 delivered attachments prior to scoring:

- **(a) Exact joined display name visible in `streams.png`**: `Alex` (shown in the top-left header status bar as `Joined as Alex` next to the `Connected` badge).
- **(b) FPS shown by the sharing slider AND the active-sharing summary in `dirty.png`**:
  - **Sharing slider**: `60 FPS` (and the pending configuration summary under Quality preset reads `1920×1080 · 60 FPS · SDR`).
  - **Active-sharing summary**: `30 FPS` (shown at the bottom-left above the action buttons in green text as `Sharing 1920×1080 · 30 FPS`, accompanied by the amber warning `Changes pending · restarts stream` below the buttons).
- **(c) General accent color of the main action buttons**: Bright **mint / seafoam green (teal-cyan/emerald)** with dark text for active primary actions (`Start sharing`, `Watch`, `Join room` in `error.png`, `Apply changes` in `dirty.png`), a muted/desaturated dark teal when disabled (`Join room` in `onboarding.png` and `joining.png`), and a muted dark rose/burgundy-red with light text for destructive `Stop sharing`.

---

## 2. Exact Filenames & Visible States Inspected

All 12 attachments were inspected directly via multimodal vision:

1. **`streams.png`** (`thread-delegated-task-command-3amcp-3a8074d293-8a06-4c29-84a9-6f268db21a9a-3adel-54e429ed-25b0-459d-8e78-c1b0f86902b4.png`): 1280×720 connected view (`FreedomStream demo`, `Joined as Alex`, `5 in room`), left sharing sidebar idle (`Start sharing`), right stream grid with 4 synthetic color-bar stream cards (`Jamie · Desktop`, `Taylor · Workspace`, `Sam · Game`, `Casey · Screen`) all in `Preview` mode with centered `Watch` buttons (`4 streams · 0 watching`).
2. **`watching.png`** (`thread-delegated-task-command-3amcp-3a8074d293-8a06-4c29-84a9-6f268db21a9a-3adel-317e82dc-063b-4a19-89cb-53697184621a.png`): 2×2 grid (`4 streams · 2 watching`). Top two cards (`Jamie · Desktop`, `Taylor · Workspace`) show green `Live` badges and bottom control bars (`Stop`, `Focus`, `Mute`, `Vol` slider + `100%` readout). Bottom two cards remain in `Preview` state with centered `Watch` buttons.
3. **`one.png`** (`thread-delegated-task-command-3amcp-3a8074d293-8a06-4c29-84a9-6f268db21a9a-3adel-87275075-54cb-4f71-a4bb-ba0931905d53.png`): Single available stream (`1 streams · 0 watching`, `6 in room`), `Jamie · Desktop` expanded to fill the right stream region in `Preview` mode with a centered `Watch` button.
4. **`focus.png`** (`thread-delegated-task-command-3amcp-3a8074d293-8a06-4c29-84a9-6f268db21a9a-3adel-d3fc0c8b-7259-45b7-910e-a4b5f1195289.png`): Focused live stream view (`Focused stream` heading, top-right `Back to grid` button). Single large card for `Jamie · Desktop` (`Live` badge) with bottom control bar (`Stop`, `Grid`, `Mute`, `Vol` slider + `100%`).
5. **`narrow.png`** (`thread-delegated-task-command-3amcp-3a8074d293-8a06-4c29-84a9-6f268db21a9a-3adel-9a5f25d6-0858-4242-92f7-841d379702fc.png`): 960×540 viewport (`5 in room`, `4 streams · 0 watching`). Shows single-column stream layout in the right pane where `Jamie · Desktop` fills the visible viewport height and the remaining 3 streams overflow below the fold without a visible scrollbar.
6. **`advanced.png`** (`thread-delegated-task-command-3amcp-3a8074d293-8a06-4c29-84a9-6f268db21a9a-3adel-6823bb0f-22e7-4094-bfda-1410acf865af.png`): Modal dialog (`Advanced settings`) overlaying the main view. Contains `Codec / hardware encoder` (`NVIDIA NVENC HEVC`), `Video bitrate` (`Automatic`), `Watching allowance` (`Automatic · 12 Mbps`), `Retry watching connection` button, `Connection route` (`Automatic`), and bottom buttons (`Done`, `Refresh sources`) plus a top-right `×` close button.
7. **`onboarding.png`** (`thread-delegated-task-command-3amcp-3a8074d293-8a06-4c29-84a9-6f268db21a9a-3adel-192721e8-a425-4303-bccc-5466f6c99164.png`): Disconnected initial state (`Not in a room`, `Your private room`). Left sidebar shows `Join your room` form with empty `Room invitation` and `Your name` fields, muted/disabled `Join room` button, and encryption/saved-access notes. Right area shows centered empty-state explanation (`A room for your friends' screens`).
8. **`joining.png`** (`thread-delegated-task-command-3amcp-3a8074d293-8a06-4c29-84a9-6f268db21a9a-3adel-c4ad7ec2-e499-4cf7-adb7-95d38dfb76a5.png`): Busy joining state (`Joining room` amber badge, `Cancel join` button in top-right header, full-width banner `Joining your private room...` with left arc spinner, masked `Room invitation` dots, `Your name` = `Alex`, disabled `Join room` button).
9. **`error.png`** (`thread-delegated-task-command-3amcp-3a8074d293-8a06-4c29-84a9-6f268db21a9a-3adel-ee37803e-48a6-4b5a-a28c-8edbaddeb2a0.png`): Failed join state (`Not in a room` badge, red-tinted full-width banner `We couldn't reach this room. Check your connection and try joining again.`, preserved masked `Room invitation` and `Your name` = `Alex`, bright mint enabled `Join room` button).
10. **`empty.png`** (`thread-delegated-task-command-3amcp-3a8074d293-8a06-4c29-84a9-6f268db21a9a-3adel-6cfbaa4a-683d-4081-8089-21eba8b5be90.png`): **Filename vs. state mismatch**: Although the manifest labels `empty.png` as "joined room/no visible streams", the actual delivered image shows a `Joining room` busy state (`Joining room...` banner, `Joining room` badge, `Join your room` left sidebar, and pre-join right-pane copy `Join with a private invitation to share and choose what to watch`). It does **not** show a connected empty room (`Connected` badge + `Share your screen` sidebar + 0 active streams).
11. **`sharing.png`** (`thread-delegated-task-command-3amcp-3a8074d293-8a06-4c29-84a9-6f268db21a9a-3adel-20237f71-37c2-481c-bc05-3a0af6eae3e5.png`): Active screen sharing state with no pending changes. Bottom-left shows `Sharing 1920×1080 · 60 FPS` in mint green, a red-tinted `Stop sharing` button, and a muted/disabled `Apply changes` button.
12. **`dirty.png`** (`thread-delegated-task-command-3amcp-3a8074d293-8a06-4c29-84a9-6f268db21a9a-3adel-cfc17d0d-17ab-4a64-8910-648e897e9040.png`): Active screen sharing (`Sharing 1920×1080 · 30 FPS`) with unapplied slider changes (`60 FPS` selected above). Bottom-left shows red-tinted `Stop sharing`, bright mint enabled `Apply changes`, and amber status text `Changes pending · restarts stream`.

---

## 3. Seven-Category Score Table & Confidence

| Category                              | Score (0–10) | Confidence | Summary Justification                                                                                                                                                                                                                                                                                                                                                                                                                     |
| :------------------------------------ | :----------: | :--------: | :---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Visual design**                     | **8.0 / 10** |    High    | Clean dark slate palette, crisp hierarchy, and semantic status colors (`Connected`, `Joining room`, `Live`, `Stop sharing`). Held back by a missing backdrop dim behind the `Advanced settings` modal (`advanced.png`), vertical misalignment in the live stream control bar (`watching.png`), and awkward positioning of `Watch` on single/narrow cards (`one.png`, `narrow.png`).                                                       |
| **Layout and responsiveness**         | **6.5 / 10** |    High    | 1280×720 2×2 grid (`streams.png`) and single-stream (`one.png`) layouts are well-proportioned. However, at 960×540 (`narrow.png`), the header says `4 streams · 0 watching` while only 1 stream card is visible and the remaining 3 are clipped below the fold with **no scrollbar or scroll affordance**, and the bottom control bar in `watching.png` overlaps the bottom of the 16:9 video frame.                                      |
| **Room onboarding**                   | **8.0 / 10** |   Medium   | Clear join form, masked invitation token, visible encryption/saved-access notes, and top-right `Copy invitation` / `Room options` / `Leave room` controls. Lowered because `empty.png` actually shows a `Joining room...` screen rather than a connected empty room, leaving the post-join 0-streams state unverified, and `Room options` is never shown open.                                                                            |
| **Sharing workflow**                  | **8.5 / 10** |    High    | Preserves all constraints cleanly: left sidebar, source cap (`Source · up to 4K`), prominent `60 FPS` slider + pill, `1920×1080 · 60 FPS · SDR` summary, explicit source-audio/no-mic callout, codec/bitrate tucked in `Advanced`, and very clear active/dirty states (`sharing.png`, `dirty.png`). Minor friction from lack of slider range ticks and lack of a room-level self-sharing badge outside the bottom-left footer.            |
| **Watching workflow**                 | **7.5 / 10** |    High    | Clear `Preview` vs. `Live` badges, explicit `Watch`/`Stop` subscriptions, multi-stream support, and `Focus`/`Grid` switching. Held back by the `Watch` button sitting low off-center on large cards (`one.png`, `narrow.png`), redundant `Back to grid` + `Grid` buttons (`focus.png`), unpluralized `1 streams` (`one.png`), and absence of preview cadence info in the connected view.                                                  |
| **Feedback and recovery**             | **8.0 / 10** |   Medium   | Strong `Joining room` busy state with `Cancel join` (`joining.png`), clear error banner with preserved inputs (`error.png`), and explicit `Changes pending · restarts stream` warning (`dirty.png`). Penalized because `empty.png` is a duplicate joining state rather than a connected empty room, the `Joining room...` spinner arc touches the left window edge, and `Retry watching connection` is buried inside `Advanced settings`. |
| **Accessibility and discoverability** | **7.5 / 10** |    High    | Good general label clarity and control sizing. Held back by low contrast on disabled `Join room` dark text (`onboarding.png`, `joining.png`), low contrast on `Preview` badges, lack of a `Show/Hide` toggle on the masked invitation field, and hidden scrollability in `narrow.png`.                                                                                                                                                    |

---

## 4. Category-by-Category Evaluation (Facts, Opinions & Unknowns)

### 4.1 Visual Design (8.0 / 10)

- **Visible Facts**:
  - Consistent dark navy/slate background with subtle 1px borders separating the top header, left sharing panel, and right stream area (`streams.png`).
  - Semantic color coding is consistent: mint green for primary CTAs (`Start sharing`, `Watch`, `Apply changes`, `Join room`), translucent green for active status (`Connected`, `Live`), amber for transitional states (`Joining room`, `Changes pending · restarts stream`), and muted red/rose for destructive/error states (`Stop sharing`, error banner in `error.png`).
  - In `advanced.png` (center, Overlaying `Friends' streams`), the `Advanced settings` dialog floats directly over the underlying UI with **no backdrop scrim/dimming**, causing the dialog borders and text to visually compete with the bright rainbow test bars and `Share your screen` heading (`Friends' st` is partially cut off right against the modal's left border).
  - In `watching.png` (center cards, bottom overlay bar) and `focus.png` (bottom of video card), the `Vol` text label is vertically bottom-aligned (`baseline`/low) relative to the centered `Stop`, `Focus`/`Grid`, `Mute` buttons and the `100%` pill.
  - In `advanced.png` (bottom of modal), `Done` (primary close action) and `Refresh sources` (utility action) share the exact same secondary button styling and sit side-by-side at the bottom-left.
- **Design Opinions**:
  - The overall native dark theme looks cohesive, restrained, and purpose-built without browser chrome.
  - Adding a semi-transparent dark backdrop (`rgba(0, 0, 0, 0.55)`) behind `Advanced settings` and differentiating `Done` from `Refresh sources` would substantially improve visual hierarchy.
- **Unknown Behaviors**:
  - Hover, active/pressed, and focus-ring visual states cannot be verified from static screenshots.

### 4.2 Layout and Responsiveness (6.5 / 10)

- **Visible Facts**:
  - At 1280×720 (`streams.png`, `watching.png`), four 16:9 stream cards fit comfortably in a 2×2 grid with balanced padding.
  - With a single stream (`one.png`, `focus.png`), the card expands cleanly to utilize the right pane.
  - At 960×540 (`narrow.png`, right region), the header displays `4 streams · 0 watching`, but only one large card (`Jamie · Desktop`) is visible, clipped right at its bottom border. The other three streams (`Taylor`, `Sam`, `Casey`) are completely hidden below the viewport fold, and there is **no visible scrollbar** or overflow indicator on the right edge.
  - In `watching.png` and `focus.png`, the dark translucent control bar (`Stop`, `Focus`/`Grid`, `Mute`, `Vol`, `100%`) is overlaid directly atop the bottom ~12% of the 16:9 video frame, obscuring the bottom section of the stream content.
- **Design Opinions**:
  - Hiding 3 of 4 streams at 960×540 without a scrollbar or adaptive grid sizing makes multi-stream discovery fragile on smaller windows. Either a visible scrollbar must appear when overflowing, or the grid should scale cards down to keep multiple streams in view.
  - Placing the stream control bar below the video canvas (mirroring the card header above the video) or auto-hiding it would prevent permanent occlusion of 16:9 content (such as a shared desktop taskbar).
- **Unknown Behaviors**:
  - Whether mouse-wheel scrolling works in `narrow.png` or whether the bottom control bar in `watching.png` auto-hides when the cursor leaves the card.

### 4.3 Room Onboarding (8.0 / 10)

- **Visible Facts**:
  - `onboarding.png` (left sidebar) presents a concise `Join your room` workflow with `Room invitation` (`Paste your private invitation link`), `Your name` (`How friends will see you`), and a disabled `Join room` button.
  - Below the button, two explicit notes state: _"Your last joined room and name are encrypted on this PC and remembered for next launch."_ and _"Saved access can be removed in Room options above."_
  - `joining.png` and `error.png` show `Room invitation` masked with bullet characters (`••••`) and `Your name` populated with `Alex`.
  - Once connected (`streams.png`, top header), the app displays `FreedomStream demo`, a `Connected` badge, `Joined as Alex`, a `5 in room` participant pill, and action buttons for `Copy invitation`, `Room options`, and `Leave room`.
  - **Missing State / Mislabelled Attachment (`empty.png`)**: The manifest describes `empty.png` as _"joined room/no visible streams"_. However, `empty.png` actually shows the disconnected/joining state (`Joining room` badge, `Joining room...` banner, `Cancel join` button, `Join your room` left form, and pre-join right-pane prompt `Join with a private invitation to share and choose what to watch`). It is visually almost identical to `joining.png` except the banner text reads `Joining room...` instead of `Joining your private room...`.
- **Design Opinions**:
  - The pre-join onboarding copy is reassuring, privacy-forward, and clear.
  - Because `empty.png` did not capture a connected empty room (`Connected` + `Share your screen` sidebar + 0 streams in the room), the actual post-join empty-room onboarding UX (e.g., whether it prompts the user to click `Copy invitation` or `Start sharing`) cannot be verified and appears untested in this image set.
- **Unknown Behaviors**:
  - What `Room options` contains when clicked (e.g., how saved room access is cleared or whether display name can be changed mid-session).

### 4.4 Sharing Workflow (8.5 / 10)

- **Visible Facts**:
  - Left sidebar (`streams.png`, `sharing.png`, `dirty.png`) contains:
    - `Screen or window` dropdown (`Monitor 1 · 1920×1080`)
    - `Resolution` dropdown (`Source · up to 4K`)
    - `Frame rate` horizontal slider with a prominent `60 FPS` pill readout
    - `Quality preset` dropdown (`Games / motion`)
    - Summary line: `1920×1080 · 60 FPS · SDR`
    - Info box: `Audio follows your source` / `Discord and FreedomStream excluded. Microphone off.`
  - `advanced.png` houses `Codec / hardware encoder` (`NVIDIA NVENC HEVC` with note `H.265 / HEVC — HEVC switches to H.264 when a watcher needs compatibility`) and `Video bitrate` (`Automatic` with note `Automatic follows resolution, FPS and content, up to 35 Mbps. Manual: 1–100 Mbps. Connection limits still apply.`).
  - `sharing.png` (bottom-left) shows active status `Sharing 1920×1080 · 60 FPS` in green, a red `Stop sharing` button, and a disabled `Apply changes` button.
  - `dirty.png` (bottom-left) shows active status `Sharing 1920×1080 · 30 FPS` while the slider above is set to `60 FPS`, enabling the bright mint `Apply changes` button and showing `Changes pending · restarts stream` in amber below.
- **Design Opinions**:
  - The separation between active sharing state (`Sharing 1920×1080 · 30 FPS`) and pending edits (`60 FPS` slider + `Apply changes` + `Changes pending · restarts stream`) in `dirty.png` is clear and well-thought-out.
  - Two minor visual gaps remain: (1) the `Frame rate` slider track shows no min/max labels or discrete step ticks (e.g., 15 / 30 / 60 FPS), and (2) `Refresh sources` is tucked inside the `Advanced settings` modal footer (`advanced.png`) rather than near the `Screen or window` dropdown where users look when a newly opened game/window isn't listed.
- **Unknown Behaviors**:
  - Dropdown options for `Screen or window`, `Resolution`, `Quality preset`, and manual `Video bitrate` input.

### 4.5 Watching Workflow (7.5 / 10)

- **Visible Facts**:
  - Available streams in `streams.png` display participant name and source (`Jamie · Desktop`, `Taylor · Workspace`, `Sam · Game`, `Casey · Screen`), a top-right `Preview` badge, and a `Watch` button over the preview frame.
  - Subscribed streams in `watching.png` switch their badge to a green `Live` pill, update the top-right counter to `4 streams · 2 watching`, and display a bottom control bar with `Stop`, `Focus`, `Mute`, `Vol` slider, and `100%` box.
  - **Button placement defect (`one.png`, `narrow.png`)**: While the `Watch` button is visually centered on the smaller 2×2 cards (`streams.png`), on the larger single-stream card (`one.png`, `narrow.png`) the `Watch` button sits roughly two-thirds of the way down the card rather than in the vertical center of the preview area.
  - **Redundant navigation (`focus.png`)**: Focused view provides both a `Back to grid` button in the top-right section header and a `Grid` button inside the bottom-left overlay bar.
  - **Grammar defect (`one.png`)**: Top-right stream counter reads `1 streams · 0 watching` instead of `1 stream · 0 watching`.
- **Design Opinions**:
  - Explicit subscription (`Watch` vs. `Stop`) and simultaneous multi-stream watching (`watching.png`) are immediately understandable.
  - Fixing the vertical centering of the `Watch` button on large cards (`one.png`, `narrow.png`) and pluralizing `1 stream` properly are quick, high-visibility polish fixes.
  - While `onboarding.png` mentions `"Previews first. Live video and audio only when you choose Watch"`, the connected views (`streams.png`, `one.png`) do not indicate the preview refresh cadence (start/minute preview cadence) on or near the `Preview` badge.
- **Unknown Behaviors**:
  - Whether clicking the video canvas itself toggles focus or full-screen mode.

### 4.6 Feedback and Recovery (8.0 / 10)

- **Visible Facts**:
  - `joining.png` shows a clear busy state: amber `Joining room` badge, top-right `Cancel join` button, and a full-width status bar (`Joining your private room...`) with a curved spinner graphic on the far left.
  - `error.png` shows a full-width red error banner (`We couldn't reach this room. Check your connection and try joining again.`), returns the status badge to `Not in a room`, keeps the user's invitation and name (`Alex`) intact, and re-enables `Join room` for one-click retry.
  - **Visual defect in `joining.png` and `empty.png` (top-left of status banner, ~y=105px)**: The white spinner arc `( ` is flush against the very left pixel edge of the window with zero left margin, looking clipped/misaligned compared to the `FreedomStream` header and `Join your room` sidebar padding (`~16px`).
  - **Missing connected-empty state (`empty.png`)**: As noted in Section 2 & 4.3, `empty.png` shows a `Joining room...` screen rather than a connected room with 0 streams.
  - In `advanced.png`, a `Retry watching connection` button is present under `Watching allowance`, but no per-card retry/reconnecting state is shown on the stream cards themselves.
- **Design Opinions**:
  - Preserving form inputs on connection failure (`error.png`) and offering `Cancel join` (`joining.png`) are strong recovery patterns.
  - However, we need a real screenshot of a **connected room with 0 streams** to verify how an empty room guides a connected user.
- **Unknown Behaviors**:
  - What feedback appears when `Copy invitation` is clicked (e.g., whether button text changes to `Copied!` or a toast appears).

### 4.7 Accessibility and Discoverability (7.5 / 10)

- **Visible Facts**:
  - **Low contrast on disabled primary button (`onboarding.png`, `joining.png`, left sidebar)**: The disabled `Join room` button uses dark navy/slate text on a medium-dark muted teal background, resulting in poor luminance contrast compared to the disabled `Apply changes` button in `sharing.png` (which uses lighter grey text on a dark surface).
  - **Low contrast on `Preview` badges (`streams.png`, `watching.png`, `one.png`, `narrow.png`)**: The muted grey text inside the dark grey `Preview` badge has noticeably lower contrast than the `Live` badge or card titles.
  - **No `Show/Hide` toggle on `Room invitation` (`joining.png`, `error.png`)**: The invitation input masks pasted characters as bullets (`••••`), with no eye/reveal icon to visually verify or edit a pasted link after an error (`error.png`).
  - **Discoverability of `Refresh sources` and `Retry watching connection` (`advanced.png`)**: Both troubleshooting/utility actions are hidden inside the `Advanced settings` modal rather than near the `Screen or window` selector or stream grid header.
- **Design Opinions**:
  - Harmonizing disabled button styling (`Join room` matching the subdued outline/text style of disabled `Apply changes`) and brightening the `Preview` badge text will immediately improve legibility.
- **Unknown Behaviors**:
  - Keyboard tab order, focus rings, `Escape` key dismissal for `Advanced settings`, and screen-reader accessibility tree labels cannot be evaluated from static screenshots.

---

## 5. Prioritized Actionable Issues

1. **[High] Missing connected empty-room screenshot (`empty.png` shows `Joining room...` instead)**
   - **Location**: `empty.png` (entire window: top-left badge `Joining room`, banner `Joining room...`, left sidebar `Join your room`).
   - **Observation (Fact)**: `empty.png` depicts an in-progress join state nearly identical to `joining.png` rather than a joined room with no visible streams (`Connected` badge + `Share your screen` left panel + empty `Friends' streams` right panel).
   - **Suggested Improvement**: Provide a true connected empty-room screenshot (`Connected` status, `Share your screen` sidebar visible, `0 streams · 0 watching` in the right pane) and ensure the right-pane empty state shows post-join guidance (e.g., _"No friends are sharing right now. Copy your room invitation to invite friends, or start sharing your screen on the left."_) rather than pre-join copy (_"Join with a private invitation..."_).

2. **[High] Hidden overflow with no scrollbar at 960×540 (`narrow.png`)**
   - **Location**: `narrow.png` (right stream area, `Friends' streams`).
   - **Observation (Fact)**: Header indicates `4 streams · 0 watching`, but only `Jamie · Desktop` is visible and fills the entire vertical space; the other 3 streams are clipped off-screen with no scrollbar or visual scroll indicator.
   - **Suggested Improvement**: Render a visible scrollbar on the stream grid container when content overflows vertically, or switch to a compact 2×2 grid at 960×540 so all 4 streams remain visible without scrolling.

3. **[Medium] `Watch` button sits vertically low (off-center) on large single-column stream cards (`one.png`, `narrow.png`)**
   - **Location**: `one.png` and `narrow.png` (center-bottom of `Jamie · Desktop` video preview area).
   - **Observation (Fact)**: On 2×2 cards (`streams.png`), `Watch` is centered vertically in the color-bar preview. On large single cards (`one.png`, `narrow.png`), `Watch` sits roughly 65–70% down the preview canvas instead of at the vertical midpoint (50%).
   - **Suggested Improvement**: Anchor the `Watch` button container to the true vertical and horizontal center (`50% / 50%`) of the video preview canvas across all card sizes.

4. **[Medium] `Advanced settings` modal lacks a backdrop scrim/dimmer (`advanced.png`)**
   - **Location**: `advanced.png` (center modal over `Share your screen` and `Friends' streams`).
   - **Observation (Fact)**: The modal sits over a fully bright background; `Friends' st` is clipped directly against the modal's left border and the bright rainbow test bars visually compete with the modal edges.
   - **Suggested Improvement**: Render a semi-transparent dark backdrop overlay (`rgba(0, 0, 0, 0.55)`) across the window behind `Advanced settings`, and visually distinguish the primary `Done` button from `Refresh sources` in the modal footer.

5. **[Medium] Live stream control bar overlaps bottom of 16:9 video content and misaligns `Vol` label (`watching.png`, `focus.png`)**
   - **Location**: `watching.png` (bottom of `Jamie · Desktop` and `Taylor · Workspace` cards) and `focus.png` (bottom of `Jamie · Desktop` card).
   - **Observation (Fact)**: The dark control bar covers the bottom strip of the 16:9 test pattern, and the `Vol` text label sits noticeably lower than the vertical centerline of the `Stop`, `Focus`/`Grid`, `Mute` buttons and `100%` readout.
   - **Suggested Improvement**: Vertically center-align the `Vol` label with its neighboring controls, and place the control bar in a dedicated footer strip below the 16:9 video canvas (matching the header strip above it) so shared desktop taskbars are never occluded.

6. **[Medium] Disabled `Join room` button has low text contrast (`onboarding.png`, `joining.png`)**
   - **Location**: `onboarding.png` and `joining.png` (left sidebar, below `Your name`).
   - **Observation (Fact)**: Dark text (`Join room`) on a dim teal fill has low visual contrast and uses a completely different disabled treatment than the disabled `Apply changes` button in `sharing.png`.
   - **Suggested Improvement**: Use the same high-legibility muted button style as disabled `Apply changes` in `sharing.png` (subtle dark surface with light-muted text) when `Join room` is disabled.

7. **[Low] Spinner arc in joining banner is flush against the left window edge (`joining.png`, `empty.png`)**
   - **Location**: `joining.png` and `empty.png` (far-left edge of the `Joining your private room...` / `Joining room...` banner).
   - **Observation (Fact)**: The white spinner arc touches `x = 0` with no left padding, misaligned with the `16px` left inset of `FreedomStream`, `Joining room`, and `Join your room`.
   - **Suggested Improvement**: Add `16px` left padding inside the status banner so the spinner aligns with the left column margin.

8. **[Low] Grammar error `1 streams` and redundant `Grid` buttons (`one.png`, `focus.png`)**
   - **Location**: `one.png` (top-right stream counter `1 streams · 0 watching`) and `focus.png` (top-right `Back to grid` vs. bottom-left `Grid` button).
   - **Observation (Fact)**: `1 streams` uses plural form for a count of 1; `focus.png` shows two differently named buttons (`Back to grid` and `Grid`) for returning to the grid view.
   - **Suggested Improvement**: Use singular `1 stream · 0 watching` when count is 1, and use a consistent label (or remove the duplicate button) in `focus.png`.

9. **[Low] Discoverability of `Refresh sources` (`advanced.png` vs. `streams.png`)**
   - **Location**: `advanced.png` (bottom footer) and `streams.png` (left sidebar, `Screen or window`).
   - **Observation (Opinion)**: Putting `Refresh sources` inside `Advanced settings` hides a routine capture workflow action when a user launches a game or window after opening the app.
   - **Suggested Improvement**: Add a small refresh icon button or inline action next to the `Screen or window` label in the main left sidebar.
