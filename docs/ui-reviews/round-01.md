# FreedomStream Native UI/UX Review — Round 1

- **Reviewer:** Independent Gemini 4 Argon (`gemini-4-argon-high` via Antigravity harness)
- **Date:** November 9, 2026
- **Branch:** `feat/native-rooms`
- **Application Source Commit:** `3432929` (`34329298cd69bc8a1f46cbca067e0da3bc804bdf` — `fix: stack stream cards vertically and capture the native layout`)
- **Repository HEAD Commit:** `12f4fc9` (`12f4fc9bfabc0e2919f6efdce6b04072f97459ed` — `docs: define fresh Gemini UI review and improvement criteria`)
- **Executable Target:** `.local/gstreamer/bin/FreedomStream-native.exe` in `C:/Users/Felipe/Documents/Coding/freedomstream`
- **Overall Round 1 Gate Status:** **BLOCKED / INCOMPLETE (Hands-on computer use unavailable in provider; supplemental visual & static UX review provided below)**

---

## 1. Environment & Computer-Use Blocker Report

### Live Environment State

- Verified active processes in the non-elevated Windows session:
  - `FreedomStream-native.exe` (5 running instances: 1 GUI client instance + 4 synthetic 720p30 publisher fixtures `Test stream 1` through `Test stream 4`)
  - `livekit-server.exe` (PID `21588`, local test SFU)
  - `PickerHost.exe` (PIDs `16724`, `27112`, Windows permission/picker host observed by the parent session)
- No secrets, invitations, cookies, session/media tokens, or private window inventories were printed, captured, or exposed.

### Exact Computer-Use Tool Limitation

1. **No Windows Computer-Use Tool in the Antigravity Provider:**
   - This session runs in T3 Code through the **Antigravity** provider (`gemini-4-argon-high`).
   - The bundled Windows Computer Use skill (`C:/Users/Felipe/.codex/plugins/cache/openai-bundled/computer-use/26.1103.32239/skills/computer-use/SKILL.md` and `docs/guidance.md`) requires the Codex-specific `node_repl` (or `cua_repl`) JavaScript kernel where `@oai/sky` is injected (`const { sky } = await import("@oai/sky")`).
   - Neither `node_repl`, `cua_repl`, nor any `@oai/sky` / Windows UI Automation MCP tool is exposed to the Antigravity provider.
   - The `t3-code` MCP server exposes only embedded web-browser `preview_*` tools and mobile simulator/emulator `device_*` tools (which return `Agent device access is turned off for this environment`).
2. **Prohibition on Custom Automation Workarounds:**
   - Both the Computer Use skill policy (`SKILL.md` / `guidance.md`) and the review instructions explicitly prohibit spawning `codex-computer-use.exe` directly, building custom helper protocol clients, or synthesizing custom PowerShell/Win32 UI automation to drive windows or bypass Windows security prompts (`PickerHost.exe`).

Per the rubric and prompt instructions, **this review does not claim a hands-on pass**. All interaction-specific categories are marked **Unverified** for hands-on computer use. To maximize actionable value for Round 1 implementation, a candid supplemental assessment of the actual 1280×720 native framebuffer captures (`docs/images/native-room.png` and `.local/screenshots/freedomstream-four-streams.png`) and the native `egui` implementation (`apps/publisher/src/ui.rs`, `main.rs`, `publisher.rs`, `playback.rs`, `quality.rs`, `admission.rs`, `settings.rs`) is provided below.

---

## 2. Actions Exercised vs. Inaccessible / Untested Areas

### Exercised / Inspected

- Verified computer-use tool availability and documented exact provider/tool boundaries (`SKILL.md`, `guidance.md`, `api.md`, `confirmations.md`, and `t3-code` MCP capabilities).
- Verified running process state (`FreedomStream-native`, `livekit-server`, `PickerHost`) without opening any visible console window or reading room authority files.
- Inspected the actual 1280×720 native renderer framebuffer captures with 1 synthetic publisher (`docs/images/native-room.png`) and 4 synthetic publishers (`.local/screenshots/freedomstream-four-streams.png`).
- Audited the complete native UI/UX implementation in `apps/publisher/src/ui.rs` and supporting state/control modules (`main.rs`, `publisher.rs`, `playback.rs`, `quality.rs`, `admission.rs`, `settings.rs`).

### Inaccessible / Untested (Hands-On Computer Use Blocked)

- **Windows Security / Firewall Prompt:** Allowing the `livekit-server.exe` firewall prompt in `PickerHost.exe`.
- **Watching Workflow Interactions:** Clicking `Watch` and `Stop watching` on `Test stream 1–4`, simultaneous multi-stream playback, toggling `Focus` and `Back to grid`, scrubbing the `Volume` slider, and expanding `Playback details`.
- **Layout & Responsiveness Interactions:** Live window resizing between the `960×540` minimum and larger viewports, left-panel splitter dragging (`260–380 px`), and vertical scrolling in the left sharing panel and right stream grid.
- **Sharing Workflow Interactions:** Opening the `Source` dropdown, selecting the controlled FreedomStream window, adjusting the `Resolution` dropdown, dragging the `FPS` slider, switching `Preset` options, expanding `Advanced` (`Codec / hardware encoder`, `Bitrate` Auto/Manual, `Watching limit`, `Route`, `Refresh sources`), and clicking `Start sharing` / `Apply settings` / `Stop sharing`.
- **Room Onboarding & Recovery Interactions:** Typing/pasting into `Room invitation` and `Your name`, clicking `Leave room`, rejoining, clicking `Copy invitation`, and clicking `Forget saved room`.
- **Keyboard Navigation:** Live `Tab` traversal order, `Enter` submission, slider arrow keys, and `Escape` behavior.

---

## 3. Rubric Evaluation & Score Table

> **Scoring Note:** Per `docs/ui-reviews/rubric.md`, an interaction-specific category that could not be exercised hands-on must be marked **Unverified** rather than awarded a passing score from screenshots or source inspection. The **Supplemental Static / Framebuffer Score (0–10)** column provides an honest, non-passing baseline rating of the current visual and structural design to guide Round 1 implementation.

| Category                                 | Official Hands-On Score                                            | Supplemental Static / Framebuffer Score | Key Evidence & Justification                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| :--------------------------------------- | :----------------------------------------------------------------- | :-------------------------------------- | :----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **1. Visual design**                     | **6.0 / 10** _(static framebuffer; interactive states unverified)_ | **6.0 / 10**                            | Clear high-level split (48 px header, 294 px left sharing panel, wide right stream canvas) and good mint accent (`#43CDB1`) on primary `Start sharing` and `Watch` CTAs. However, the UI relies on raw uncustomized `egui::Visuals::dark()` defaults (`#1b1b1b`), inconsistent control label placements (top labels vs. right-side `from_label` labels), inconsistent dropdown widths, a raw lowercase `"connected"` string in the header, and cramped default button padding (`~18–20 px` height on secondary buttons).                                                                                                       |
| **2. Layout and responsiveness**         | **Unverified** _(resizing & scrolling untested hands-on)_          | **6.5 / 10**                            | Commit `3432929` fixed vertical stacking inside stream cards and removed the missing play glyph. At `1280×720`, the 2-column grid (`columns = 2` when width `>= 680 px`) gives strong `~444×250 px` 16:9 tiles. However, with 4 streams (`.local/screenshots/freedomstream-four-streams.png`), two rows of tiles plus the `"Live streams"` header exceed the `672 px` central panel height, clipping the bottom of Row 2 even before any stream is watched. Watching a stream adds 3 stacked rows of controls below the video, pushing lower tiles far off-screen.                                                             |
| **3. Room onboarding**                   | **Unverified** _(join/leave/rejoin untested hands-on)_             | **5.5 / 10**                            | Auto-join via DPAPI profile works at startup, and the invitation/name fields hide once connected to save space. However, once connected, **the user's own display name is completely hidden**, **non-sharing participants in the room are invisible** (the room looks empty when nobody is sharing), `Leave room` is absent while `connection == "connecting"` (no way to cancel a stuck connection attempt), and `Forget saved room` is buried at the bottom of the collapsed `Advanced` section instead of near room/session controls.                                                                                       |
| **4. Sharing workflow**                  | **Unverified** _(source/FPS/preset/share untested hands-on)_       | **6.5 / 10**                            | Product constraints are preserved: left sharing menu, `Resolution` capped at 4K SDR (`Source · up to 4K`), visible `FPS` slider (`15–120 fps`), quality-oriented `Preset` (`Games / motion`, `Text / desktop`, `Data saver`), source-derived audio copy, and `Codec` + `Bitrate` in `Advanced`. Friction remains: inconsistent dropdown widths (`Resolution` and `Preset` do not span full width like `Source`), no visual indicator when edited settings differ from the active stream (`Apply settings` stays enabled even with no changes), and active sharing status (`Sharing W×H · FPS`) omits active/effective bitrate. |
| **5. Watching workflow**                 | **Unverified** _(Watch/Stop/Focus/Volume untested hands-on)_       | **6.5 / 10**                            | Unwatched tiles display 16:9 previews with a prominent centered `126×44 px` pill `Watch` button (`ui.rs:304`). However, tiles lack an explicit visual badge distinguishing a static minute-cadence **Preview** from an active **Live** subscription. When watching, `Stop watching`, `Focus`/`Back to grid`, `Volume`, and `Playback details` stack across three separate vertical rows beneath the video; `Volume` lacks a `%` unit suffix and a quick Mute toggle; and `Playback details` prints raw MIME types (`video/h265`) and cumulative frame counts instead of user-friendly codec/resolution labels.                 |
| **6. Feedback and recovery**             | **Unverified** _(error/reconnect flows untested hands-on)_         | **5.5 / 10**                            | Bounded, authority-redacted error messages exist across `admission.rs`, `publisher.rs`, `playback.rs`, and `settings.rs`. However, in `ui.rs:233–239`, both `settings_error` and `state.error` are rendered at the **very bottom** of the left scroll area _below_ `Advanced`—where they can be scrolled out of view. The busy indicator (`ui.rs:161`) is a bare `ui.spinner()` with no text explaining what operation is in progress, and clicking `Copy invitation` provides no `"Copied"` confirmation.                                                                                                                     |
| **7. Accessibility and discoverability** | **Unverified** _(keyboard focus/navigation untested hands-on)_     | **5.0 / 10**                            | Primary CTAs (`Start sharing`, `Watch`) have strong contrast and hit targets (`36 px` and `44 px`), but secondary action buttons (`Leave room`, `Copy invitation`, `Stop watching`, `Focus`, `Forget saved room`, `Refresh sources`) use `egui`'s tiny default height (`~18–20 px`). Pressing `Enter` in the `Room invitation` / `Your name` fields does not submit `Join room`; pressing `Escape` does not exit `Focus` mode; `ComboBox::from_id_salt` controls lack programmatic label associations; and the manual bitrate `DragValue` has no label.                                                                        |

---

## 4. Prioritized Actionable Findings

### Finding 1 — Connected room hides the user's own participant identity and shows zero presence for non-sharing participants

- **Severity:** High
- **Categories:** Room onboarding, Watching workflow, Feedback and recovery
- **Location:** `apps/publisher/src/ui.rs:62–104`, `353–380`, `392–397`; `apps/publisher/src/publisher.rs:42–57` (`Snapshot`)
- **Observation:**
  1. When `connected` or `reconnecting`, the `if !connected` block hides the `Your name` field, and neither the top bar (`room-header`) nor the left panel displays `self.name`. When the app auto-joins a saved room on launch, the user has no way to see which display name they joined under.
  2. `Snapshot.tiles` only includes remote participants who are actively publishing `TrackSource::Screenshare`. In a persistent room where friends have joined but nobody is sharing yet (or some friends are only watching), there is no participant count or participant list anywhere in the UI.
- **Recommendation:**
  - Display the local participant's active display name in the top header (e.g., `Joined as <name>`) and track remote participant count/names in `Snapshot` so the header and empty-room state can show how many participants are currently in the room (e.g., `"3 participants in room — no one is sharing yet"`).

### Finding 2 — No way to cancel an in-progress connection attempt, and `Forget saved room` is buried inside `Advanced`

- **Severity:** High
- **Categories:** Room onboarding, Feedback and recovery, Accessibility and discoverability
- **Location:** `apps/publisher/src/ui.rs:64–104`, `167–231`, `364–376`
- **Observation:**
  1. When `state.connection == "connecting"` (including automatic startup join with an unreachable server or stale saved room), the `Room invitation` and `Your name` inputs are non-interactive, `Join room` is disabled, and `Leave room` in the header is hidden because line 364 only checks `matches!(state.connection, "connected" | "reconnecting")`. The user is locked out of editing the invitation or cancelling the join until the network timeout expires.
  2. `Forget saved room` is placed at the bottom of the collapsed `Advanced` section (`ui.rs:210`)—below hardware encoders, bitrate, watching budgets, and routing—and is disabled whenever `state.busy` is true.
- **Recommendation:**
  - Allow cancelling an in-flight connection (`Command::Leave`) by showing a `Cancel` / `Leave room` action while `state.connection == "connecting"`.
  - Move `Forget saved room` out of `Advanced` into the room/onboarding area (or header session controls) and allow invoking it even while a startup auto-join is attempting to connect (which already sends `Command::Leave` and cancels capture).

### Finding 3 — Inconsistent form layout, narrow unaligned dropdowns, and unstyled `egui` defaults in the left sharing panel

- **Severity:** High
- **Categories:** Visual design, Sharing workflow, Accessibility and discoverability
- **Location:** `apps/publisher/src/main.rs:213`, `apps/publisher/src/ui.rs:106–205`, `424–514`
- **Observation:**
  - `Source` (`ui.rs:108`) and `Codec / hardware encoder` (`ui.rs:170`) place a `ui.label(...)` above a full-width `ComboBox` (`.width(ui.available_width())`).
  - `Resolution` (`ui.rs:429`) places a `ui.label("Resolution")` above a `ComboBox` **without** `.width(ui.available_width())`, so it renders at a narrow content-dependent width.
  - `Preset` (`ui.rs:457`), `Bitrate` (`ui.rs:491`), `Watching limit` (`ui.rs:180`), and `Route` (`ui.rs:198`) use `ComboBox::from_label(...)`, which in `egui` renders a narrow dropdown on the left and the label text to its right—creating a jarring mix of stacked full-width controls and narrow side-labeled controls in the `294 px` sidebar.
  - The FPS slider (`ui.rs:448`) also uses default slider width with a right-side `"FPS"` label instead of a consistent stacked label + full-width control layout.
- **Recommendation:**
  - Standardize all left-panel controls (`Source`, `Resolution`, `FPS`, `Preset`, `Codec / hardware encoder`, `Bitrate`, `Watching limit`, `Route`) to use consistent stacked section labels (`ui.strong(...)` or `ui.label(...)`) above full-width controls (`width(ui.available_width())` / `spacing.slider_width = ui.available_width()`).
  - Customize `egui::Style` / `Visuals` at startup with cohesive surface colors (e.g., distinct panel background `#11141C`, card surface `#181D27`, subtle border stroke, `6–8 px` corner rounding, and `vec2(10.0, 6.0)` button padding) so secondary buttons and cards look intentional rather than raw debug `egui`.

### Finding 4 — Error banners and busy spinners are poorly positioned and lack context

- **Severity:** Medium-High
- **Categories:** Feedback and recovery, Visual design
- **Location:** `apps/publisher/src/ui.rs:161–163`, `233–239`, `372–375`
- **Observation:**
  1. `self.settings_error` and `state.error` are rendered at the bottom of `sharing_menu` (`ui.rs:233–239`), _after_ the `Advanced` collapsible section. When `Advanced` is open or on smaller window heights (`540 px` minimum), errors triggered by `Join room`, `Start sharing`, or `Watch` appear below the fold.
  2. When `state.busy` is true (`ui.rs:161`), a bare `ui.spinner()` is rendered under `Start sharing` with no text indicating what operation is running (`Joining room…`, `Starting stream…`, `Applying settings…`, `Stopping…`).
  3. In the top header (`ui.rs:372–375`), `ui.small(state.connection)` renders raw lowercase `"connected"` / `"reconnecting"`, and clicking `Copy invitation` gives no visual confirmation.
- **Recommendation:**
  - Render a prominent, high-contrast status/error callout card near the **top** of the left panel (or top bar) so errors are immediately visible without scrolling.
  - Pair the spinner with descriptive status text (e.g., `"Joining room…"`, `"Starting capture…"`, `"Updating stream…"`).
  - Replace raw `ui.small(state.connection)` in the header with a styled connection badge (`Connected`, `Reconnecting…`, `Connecting…`, `Not in a room` with a status dot), and show temporary `"Copied!"` feedback on the `Copy invitation` button for ~2 seconds after click.

### Finding 5 — Stream tiles lack explicit Preview vs. Live badges, and 4-stream grid clips vertically at 1280×720

- **Severity:** Medium-High
- **Categories:** Watching workflow, Layout and responsiveness, Visual design
- **Location:** `apps/publisher/src/ui.rs:255–349`, `389–421`
- **Observation:**
  1. In `.local/screenshots/freedomstream-four-streams.png` (`1280×720`), four unwatched 16:9 stream cards in a 2×2 grid slightly exceed the `672 px` central panel height, clipping the bottom of Row 2 (`Test stream 3` and `Test stream 4`).
  2. Unwatched tiles with a loaded preview image show only the publisher's name and the centered `Watch` button—there is no pill/badge indicating `Preview · updates every 60s` vs. `LIVE` when watched.
  3. When a stream is watched (`tile.watching`), its controls (`Stop watching` + `Focus`, then `Volume` slider, then `Playback details` collapsing header) stack across 3 vertical rows below the video, adding ~80 px of height per watched card. Additionally, `Volume` lacks a `%` suffix and a quick `Mute` toggle, and `Playback details` prints raw MIME strings (`video/h265`) instead of `H.265 / HEVC`.
- **Recommendation:**
  - Add a clear status badge in each tile's header row (e.g., `Preview` badge when unwatched; accent `Live` badge when watched).
  - Consolidate watched-tile controls into a single compact toolbar row beneath the 16:9 surface (`Stop watching`, `Focus` / `Back to grid`, `Mute` toggle, and compact `Volume` slider with `%` suffix), and format `tile.codec` human-readably (`H.265 / HEVC` or `H.264`) in the header or details line.
  - Slightly tighten vertical padding/header spacing in `CentralPanel` and `stream_tile` (or cap tile width so 2×2 unwatched tiles fit cleanly inside the initial `1280×720` viewport without clipping Row 2).

### Finding 6 — Live sharing settings edits have no "unsaved changes" indicator and `Apply settings` stays enabled when nothing changed

- **Severity:** Medium
- **Categories:** Sharing workflow, Feedback and recovery
- **Location:** `apps/publisher/src/ui.rs:121–145`, `475–488`
- **Observation:**
  - While actively sharing (`state.sharing`), `Apply settings` is always enabled (`state.sharing && !state.busy`), even when the form controls exactly match the running stream configuration.
  - Conversely, when the user modifies `Resolution`, `FPS`, `Preset`, or `Bitrate` while sharing, there is no visual indication that the selected settings haven't been applied yet, nor whether applying them will adjust bitrate live or restart capture.
  - The active summary `Sharing W×H · FPS` (`ui.rs:141`) does not display the current target/effective bitrate unless the user opens `Advanced`.
- **Recommendation:**
  - Track or compare current UI settings against the active stream settings: disable `Apply settings` (or show `"Settings applied"`) when unchanged, and highlight `Apply settings` with a note when pending edits exist (e.g., `"Unapplied changes"`).
  - Include bitrate in the active sharing summary line (e.g., `Sharing 1920×1080 · 60 FPS · 6.0 Mbps`).

### Finding 7 — Keyboard shortcuts and control labels for accessibility

- **Severity:** Medium
- **Categories:** Accessibility and discoverability, Room onboarding, Watching workflow
- **Location:** `apps/publisher/src/ui.rs:64–95`, `320–330`, `502–511`
- **Observation:**
  - Pressing `Enter` while focused in the `Room invitation` or `Your name` single-line text fields does not submit the `Join room` action.
  - Pressing `Escape` while a stream tile is in `Focus` mode (`self.focused.is_some()`) does not return to the grid view.
  - In manual bitrate mode (`ui.rs:505`), the `egui::DragValue` widget has no accompanying label (`Fine bitrate (Mbps)`), and secondary buttons throughout the top bar and stream tiles have small default click targets (`~18–20 px` tall).
- **Recommendation:**
  - Trigger `Command::Join` when `Enter` is pressed inside `Room invitation` or `Your name` (when both fields are non-empty and `!state.busy`).
  - Handle `Escape` (`ctx.input(|i| i.key_pressed(egui::Key::Escape))`) to clear `self.focused = None` when a stream is focused.
  - Increase minimum button height (`spacing.interact_size.y` or explicit `min_size`) to at least `28–32 px` for all interactive buttons and dropdowns.
