# Implementation phases

## Ranked room codec adaptation — corrective PR

Replace direct AV1/HEVC-to-H.264 fallback with AV1 → HEVC → H.264, using
the sender's available hardware encoders at the requested mode and every connected
room member's advertised native decoders, even before Watch. The Advanced codec
selection is the preferred ceiling: a deliberate HEVC or H.264 selection remains
respected. Missing legacy capabilities conservatively allow H.264 only. Recompute
on membership/capability changes and restore the preferred codec when constraints
leave. A failed advanced decoder removes only that codec for that publisher's
current watch, retaining failures across replacement SIDs. Report and stop when
no common encoder/decoder exists; never silently change resolution or frame rate.
Keep one publication, explicit subscriptions, watch intent, and the original
start/minute preview capture clock across codec replacements.

Accept with Node 24 checks, native Clippy/tests, real SFU AV1 → HEVC → H.264
decode, joining/leaving preview-only members, recovery/upgrades and Stop/rewatch
traffic checks. Obtain a fresh GPT-6.1-Sol high review. Synthetic decoder masks
exercise negotiation on the local GPU; AMD and physical device failures remain
separate hardware gates.
See [verification](verification/ranked-room-codecs.md).

## Native settings, stream information and Discord room names — corrective PR

Repair the locally drawn sidebar gear and anchor it at the sidebar's right edge.
Keep the room label left-aligned next to the brand, translate only the legacy
`Friends` placeholder in PT-BR, and preserve user-provided room names. Advanced
confirmation must dispatch the existing validated Apply command for changed live
settings; idle confirmation must never start publication. Keep busy and invalid
selections disabled. Preserve existing in-place bitrate/profile updates and
stream restart behavior for source, encoder, resolution and frame-rate changes.

Group watched-stream measurements into reception, network/decoding and sender
sections. Distinguish muted labels from stronger values, collapse detailed
counters and measurement notes, and retain freshness, unavailable values and the
physical-presentation limitations. Preserve demand leases, subscriptions and the
start/minute preview gate.

Name Discord /stream and /telar rooms from the invoking member's guild nickname,
global display name or username, with an ID fallback for older payloads. Bound
room names without splitting Unicode pairs or retaining control characters.
Preserve signatures, authorization, replay protection and quotas.

Accept with Node 24 checks, native Clippy/tests, signed HTTP name/admission cases,
real native publication bitrate changes, renderer captures at 1280×720 and
960×540, and independent GPT-6.1-Sol high review. Renderer fixture screenshots
establish layout only; native desktop automation and a production Discord command
remain separate checks. See [verification](verification/native-ui-room-fixes.md).

## Native audio output latency — corrective PR

Follow-up authorized scope: add an opt-in synchronized flash/click source and
receiver event measurement across actual window capture, encoding, SFU decode
and Windows process-loopback output. Use those results to qualify smaller,
bounded audio queues; preserve uncertainty about physical display/speaker timing.
Aim at zero relative offset, with 15 ms audio-leading / 45 ms audio-lagging
reference limits and a tighter normal-operation audio-lag target around 20 ms.
Do not confuse audio-only output latency with A/V offset. In parallel, research
receiver synchronization using the existing WebRTC audio clock/native ADM;
report feasibility before implementing that larger architecture. The user has
approved output-only native WebRTC playback and now explicitly requires it as
the standard release path. Remove its environment switch and alternate application
playback path. Qualify per-track gain,
initial silence, Stop/rewatch/reconnect, shared-device teardown and the pinned
SDK's guards against recording and microphone initialization. Retain LiveKit and
existing codecs. Use the actual track factory's output device and preserve its
lifetime. Qualify actual process-loopback output, not only pre-gain PCM taps.
Close mixer admission before receive-channel activation; a track callback alone
is too late to guarantee initial silence. Bind gain to source generations so
removed or reused channels cannot inherit authorization. Verify a muted late
subscription and replacement while another stream is audible. Enable LiveKit
`room.sync_streams` in local/test/generated deployment configuration and verify
negotiated audio/video association per publisher; production rollout remains an
explicit deployment gate until the actual VPS is exercised.
Full physical synchronization, output-device changes and sustained drift remain
acceptance gates; compile success cannot establish any of those properties.
Follow the Windows default render endpoint, suspend output while no endpoint is
available and retry without room rejoin. Preserve mixer generations and gains
through recovery. Verify actual output stop/start, available alternate outputs,
repeated transitions and sustained timing before promoting the next release.

Investigate reported audio-after-video across AV1/H.264 capture, transport,
decode and native output. Bound the demonstrated GStreamer audio sink ring
buffer independently of WASAPI's low-latency device-period option. Add an
explicit process-only loopback qualifier using the production playback pipeline,
and native SFU stage-counter acceptance for both codecs. Never compensate with
an arbitrary video delay or treat callback/loopback timing as physical lip sync.
Record measured improvements and unresolved capture-clock/device/network timing
in `docs/verification/audio-sync-investigation.md`. Run Node 24 checks, native
Clippy/tests, actual loopback and native media checks, and obtain a fresh
GPT-6.1-Sol high review. Live-room, physical output, AMD and WAN impairment
qualification remain explicit; this correction alone cannot close full A/V sync.

## Persistent PC usage identity — independent PR

Derive an app-and-server-scoped identifier from Windows SystemIdentification,
without transmitting its raw value or collecting IP addresses. Aggregate measured
SFU usage by device across concurrent instances, rooms and later admissions; retain
session details and separate unidentified history/older clients. Keep invitations,
session authorization and SFU identities independent of device recognition.

Accept with actual Windows identity reads across independent processes and app
copies, native admission over HTTP, database migration/restart and grouped usage
tests, real SFU traffic and administrator UI checks, Node 24 and native checks,
and a fresh GPT-6.1-Sol high review. Document the registry fallback's clean-Windows
reinstall limit and client-spoofing limit. Do not claim fresh-Windows/hardware
replacement persistence from local-process checks. No account/login work.

## Fullscreen stream menu and installer-only releases — corrective PR

Open the existing viewing menu on right-click anywhere in the fullscreen video.
Preserve audio settings, diagnostics and explicit subscriptions; keep the cursor
usable while the menu is open after HUD inactivity. Exit fullscreen when choosing
Exit full screen, Focus stream/Back to grid or Stop watching. Verify egui
pointer/menu behavior, native checks and a fresh GPT-6.1-Sol high review.

Public GitHub releases now offer only Setup, its installer manifest and checksums.
Remove the standalone EXE and its release manifest from the existing v0.1.0
release, update its notes/checksums, and retain the installer bytes and tag.
Do not make a new release for this correction. The portable payload stays an
internal installer input and qualification tool. This supersedes earlier plans
to offer a standalone download alongside Setup.

## Installed Windows distribution and update rollout — separate PR

Add an unsigned preview installer using a pinned native Rust Velopack bootstrap
and the existing qualified portable payload. Keep installer/update ownership
outside the portable cleanup job. Build in GitHub Actions without publishing
release assets yet. Track the update badge, hover changelog, background checks,
click-to-download and explicit graceful restart in [the rollout plan](windows-updates.md).
Public GitHub release access, rebrand identity, signing, helper notices and real
two-version/fresh-Windows acceptance remain gates before shipping auto-updates.
Private repository visibility does not block implementation or local testing.

Follow-up `feat/update-badge-controls` adds a per-launch local update bridge,
background checks, hover notes, explicit download/progress/retry and Restart/Later
controls. The existing media/picker shutdown must complete before the outer
launcher applies an update. Verify real Velopack package/helper behavior with a
private local feed and distinguish fixture verification from actual installed
publisher, fresh-machine and signed-distribution gates. Keep this dependent PR
separate from the installer foundation; no public feed or automatic publishing.
The installer also owns per-user room-link registration. Install/update hooks
point `freedomstream://` at the stable installed launcher; uninstall removes only
that installation's handler. Migrate the old portable handler on explicit install,
then keep portable registration controls separate. Forward later links/focus to
the existing instance using bounded per-launch authenticated local IPC. Never
persist handoff tickets or invoke Update.exe with invitation arguments. Previously
used server origins may load straight into native Join confirmation; unfamiliar
origins still need trust. Room changes/sharing/watching stay explicit, dismissing
a link restores the current room invitation, and pending links must not overwrite
the last successful saved profile. With Discord gatherings, that profile stores
only the server origin and display name. Qualify hooks/registry/real-process forwarding
with an isolated test scheme; actual browser prompts/native focus and production
installation remain separate acceptance gates.

Each phase is an acceptance gate. Phase 1 is the initial commit on `main`; every later phase gets its own branch and PR. Felipe authorized autonomous implementation and merging: each PR must pass its acceptance checks and a new GPT-6.1-Sol review at high reasoning before merge. Address review findings before proceeding. Code can be MIT licensed while third-party runtimes retain their own licenses; qualify distribution licenses before bundling native media dependencies. Report external hardware/deployment verification separately and do not waive it merely to finish the sequence.

**Current checkpoint:** Phases 1–8 and clipboard PR #8 are merged. Portable
packaging PR #9 remains a separate phase. Native room PR #10 is stacked on it as
a draft, with local native/media/DPAPI/portable checks and independent review
recorded in [Phase 11 verification](verification/phase-11.md). Remaining physical
hardware, deployment/network and real-game/group gates are not waived by those
results.

## Current test scope

The participant experience is native Windows. Mobile and participant browser-room
tests are retired; browser automation covers desktop administrator and access-management
workflows. API admission, session, signaling, preview and download authorization remain
required by the native app. See [test scope and commands](testing.md). Earlier phase
browser acceptance records below describe historical verification.

## 1. Persistent private-room foundation — initial commit

Deliver a runnable TypeScript workspace, MIT license, configuration generation, persistent private rooms, hashed invitations, room-scoped browser sessions, authenticated LiveKit token issuance, a responsive room interface, local-only Docker Compose, and CI.

Accept when an administrator can create a room, another browser can join using its invitation, unauthenticated requests cannot read the room or obtain a media token, room/session state survives a process restart, and the container build and checks pass. This phase does not publish/play media or claim presence tracking. Do not expose this local stack on the VPS yet.

## 2. Live rooms and selective watching — PR

Connect the custom interface to LiveKit. Add real participant/publisher presence, browser camera/screen publication as a transport baseline, explicit Watch/Stop watching subscriptions, focus/grid layouts, independent volume/mute, leave/cleanup, late-join and reconnect behavior. Keep invitations and tokens behind the room admission interface. Temporary browser capture validates transport; native game capture remains Phase 3.

Accept with two real publishers and an independent viewer. Verify stopping a subscription stops its incoming video/audio bytes, room members see current state, joining/leaving does not leak media elements or tracks, and reload/reconnect works. Reject unauthenticated signaling access.

## 3. Native Windows capture and hardware publication — PR

Add a native Windows publisher behind a capture/encoder interface that can later gain macOS/Linux adapters. Qualify a mature engine (prefer an MIT-compatible integration with separately licensed runtime dependencies) before committing to bindings. Implement source enumeration/selection, monitor/window/game capture, runtime encoder capability probing, NVIDIA and AMD hardware HEVC, timestamps/keyframes/pacing, native participant authentication and publication into the same rooms. Keep browser H.264 fallback explicit.

Accept with an actual NVIDIA and AMD machine, game performance measurements, window/fullscreen/alt-tab handling, hardware-encoder evidence and browser decode verification. CI compile checks cannot substitute for those hardware results; report unavailable hardware qualification as incomplete.

## 4. Audio isolation — PR

Implement selected-application audio and desktop capture with Discord exclusion. Keep our own watched-stream playback out of the captured mix. Follow process restarts, route/mix audio deliberately when excluding multiple process trees, and keep microphone controls separate. Do not apply voice noise suppression to game/music audio.

Accept when friends hear the intended game audio without their Discord voices or watched-stream audio feeding back, including while a publisher watches another stream and Discord restarts.

## 5. Compressed previews — PR

Generate a preview at the publisher on stream start and then no more than once every **60 seconds**. Proposed size: 320×180 with source aspect ratio preserved. Distribute through authenticated room access, maintain only the latest preview, expire/remove stopped streams, and keep full media unsubscribed until Watch is selected. Bound image dimensions and upload size. No server decoding/transcoding just to make previews.

Accept that unwatched tiles update at the configured minute interval, never carry full stream traffic, respect room access, and remove stale content after stop/leave. New viewers may receive the current preview immediately without causing extra captures.

## 6. Quality and resource controls — PR

Expose bitrate presets spanning 3–100 Mbps, resolution choices limited by the selected source with a 4K ceiling, and frame-rate choices up to 120 fps. Auto mode recalculates bitrate recommendations from resolution/FPS/codec/content profile; manual bitrate edits preserve resolution and FPS. Show measured delivery separately from requested settings. Integrate congestion feedback and explicit adaptation policy; never promise a manual target can override network capacity.

Accept independent controls, encoder reconfiguration/source switches, hardware-aware mode availability, no silent setting rewrites, and a tested compatibility fallback. Qualify 4K120 end-to-end on capable hardware; do not mark it complete merely because the UI offers the option.

## 7. Durable access management and restrictive-network recovery — PR

Add invitation rotation/revocation and defined effects on API sessions and existing media access, publisher/viewer grants where useful, connection diagnostics, reconnection limits, and authenticated TURN/TLS fallback. Scope preview access consistently with media. Test TCP/TLS fallback with UDP deliberately unavailable, and resource limits on the shared VPS.

Accept denial at page, token, preview and media access paths; verify rotation/session behavior, no anonymous relay, and recovery during real interruptions. Phase 1 access controls remain required throughout; this phase adds lifecycle/revocation and network fallbacks.

## 8. Windows distribution and production deployment — PR

Package the native publisher with documented dependency notices, versioning and installation/uninstallation. Add the production Docker configuration, HTTPS/domain/TURN configuration, persistent data/backup/restore instructions, health checks and diagnostics. Configure the Canada VPS only with supplied access and concrete deployment settings. Budget live outbound traffic against the 1 Gbps port, despite unlimited monthly transfer; leave CPU/RAM for other workloads.

Accept fresh-machine installation, reproducible builds, a restore test, a private production room, and a full evening with four/five publishers using the expected subscriptions. Record latency, actual encoder/decoder performance, loss/retransmissions, CPU/RAM, and network load. Deployment and hardware qualification must be reported separately from automated test success.

## 9. Native clipboard integration — corrective PR

Enable the UI framework's operating-system clipboard backend. Verify paste from
another Windows application and copying an invitation back out, then update the
local test build. This corrects the minimal feature configuration discovered in
the first real user trial; an internal-only fallback clipboard is insufficient.

## 10. Fully bundled portable Windows application — PR

Replace the PowerShell installer ZIP with one portable EXE containing its media
runtime and notices. First launch may extract a private runtime cache, but must
not install shared runtimes, change system settings, or download dependencies.
The optional browser-invitation feature is an explicit exception: after the user
enables it, register only a per-user URL handler with a matching disable action.
Qualify the shipped DLL/plugin set, matching source availability and licenses.
Test offline startup, invocation from a fresh directory, actual capture/audio and
room download behavior, then review and deploy the replacement artifact.

## 11. Native room experience — PR

Replace the external Chrome viewer with native video decoding/rendering and native
audio playback in the Windows app. The website admits participants, manages access
and distributes the app; publishing and watching belong in the app. Preserve room
and explicit subscription contracts. Qualify the actual native SDK receive path
before claiming that bundling a decoder provides playback.

Use SDR only, source resolution bounded to 4K, and 60 FPS initially. Keep resolution
and an FPS slider visible. Favor image quality in the presets; put bitrate and
codec in Advanced. Automatically fall back from HEVC to H.264 for an incompatible
watcher, switching the publication for all its watchers rather than double encoding.
Preserve watch intent across that transition. Do not silently change resolution/FPS.

Derive audio from the video source: application process-tree audio for a window,
approved desktop application mix for a monitor. Always exclude Discord and our own
playback; never acquire the microphone. Keep existing fail-closed process policies.
Increase previews to 960×540 at improved JPEG quality with bounded upload/storage,
preserving the initial capture and at-most-once-per-minute cadence.

Use a 1280×720 initial native viewport, a left sharing menu with the FPS slider
visible, and larger right-side 16:9 stream tiles with centered Watch buttons.
Replace aggressive bitrate estimates with source-informed lower targets and a
bounded aggregate receiver allowance. Receiver feedback may cap the one shared
encoding; document its effects on other watchers and do not imply independent
per-viewer quality or a speed test.

Launch both portable wrapper and native app without a console. Keep test-server
executable paths stable and helper processes hidden. Do not modify firewall rules
to hide Windows prompts. Persist the last successful room/name with Windows
per-user encryption, automatically rejoin on launch without starting capture or
subscriptions, and provide an explicit Forget saved room action. Keep session and
media tokens in memory only. Verify encrypted persistence and actual reopen/join.

Refine the native app with repeated image-only Gemini 4 Argon UI/UX reviews,
judging visual polish and UX inferred from native-renderer screenshots.
Use a fresh review context after each implementation pass and the fixed rubric
in [UI review criteria](ui-reviews/rubric.md). Address actionable findings and
continue until an independent pass scores every category at least 9/10. Record
unobserved interactions honestly; visual ratings never waive media,
hardware, security or field gates.

Accept with native HEVC and H.264 decode, audible synchronized stereo playback,
explicit watch/stop traffic checks, fallback and reconnect recovery, preview access
and cadence, source/audio switching and exclusion, portable/offline startup, and
gaming while publishing and watching multiple friends. Run automated checks and a
fresh GPT-6.1-Sol high-reasoning review. AMD, fresh-machine, real-game/Discord and
4K120 qualification remain external until actually exercised. No embedded browser,
server transcoding, HDR, or new platform promise.

## 12. Friends feedback, measured bandwidth and localization — PR

Build on the native-room beta in a separate dependent branch/PR. Add a bounded,
authenticated VPS down/up measurement capped at 100 Mbps on connect and an Advanced
rerun action. Report it as capacity to this VPS, not an ISP maximum. Derive an
automatic aggregate watching allowance and upload recommendation with headroom;
adapt the shared publication to actual subscribers as they arrive/leave, while
preserving geometry and explicit manual settings. Bound transfer duration, bytes,
concurrency and server load, and cancel stale tests on leave/rejoin.

Expose room members from the room-name control, a local source-confirmation
preview without increasing uploaded preview capture cadence, simpler room actions,
visible bitrate/FPS and persistent PT-BR/English selection at the top right.
Investigate and measure the reported audio lag across sender, RTP/playout and
device queues. Do not substitute an arbitrary delay or compilation for actual
audio/video timing acceptance. Verify a synchronized audiovisual fixture and
record any remaining device/network limitations before claiming the fix.

## 13. Fullscreen viewing, bandwidth headroom and game performance — PR

Build on the feedback beta in a separate dependent branch. Add native borderless
fullscreen for a selected watched stream, explicit controls and Escape/F11 exit,
without changing other subscriptions. Show measured received video bitrate at the
bottom right of watched tiles and fullscreen views. Raise the negotiated test
ceiling to 200 Mbps while retaining the 100 Mbps encoding ceiling and network
headroom. Keep legacy 100 Mbps clients compatible, and bound transfer bytes,
concurrency and proxy settings accordingly.

Investigate the reported RX 7600 whole-monitor HEVC capture stutter at 1080p120
and 25 Mbps. Remove proven unnecessary rendering/reconfiguration work, preserve
explicit resolution/FPS/bitrate controls, and report AMD hardware qualification
separately. Local NVIDIA tests cannot establish an RX 7600/Siege fix.

## 14. Hardware AV1 publication and native decoding — PR

Add runtime-probed NVIDIA NVENC and AMD AMF AV1 publication, SDR Main-profile
8-bit framing and native Direct3D11 AV1 decoding. Retain the SDK dav1d CPU
decoder on receivers without an AV1 GPU decoder. Keep one publication and honor
H.264 compatibility requests, watch intent, explicit unsubscribe and preview cadence.
Use HEVC automatic bitrate targets until AV1 quality has been measured.

Accept with actual AV1 publication and native hardware/software decode through
the SFU, stop/rewatch traffic checks, compatibility fallback and reconnect.
Record NVIDIA and AMD hardware results independently; quantify CPU/game impact
for multiple software-decoded streams before promising low CPU usage. Existing
packaging, AMD, fresh-machine and 4K120 gates remain open.

## Administrator usage dashboard — independent PR

Replace the public home page with a single-administrator login and the approved
usage dashboard. Use the existing administrator credential to issue bounded,
HttpOnly, same-site sessions. Persist measured SFU traffic by participant session,
with server totals, per-user activity graphs, real-time/today/seven-day/calendar
month/custom periods, MB/GB transfer volumes and Mb/s speeds. Keep missing data
explicit and protect every analytics API. Retain the native admission API and
private invitation/download paths.

The standard SFU lacks participant byte counters. Build the separately licensed,
pinned LiveKit runtime with the small private collector in `deploy/livekit-usage`;
do not expose its metrics listener. Accept with real publication/subscription
traffic, restart/history/authentication tests, desktop/mobile browser checks and
independent GPT-6.1-Sol high-reasoning review. Coordinate the deployment base with
the parallel friends-beta/fullscreen release before activation; do not overwrite
its network-test/proxy changes or existing invitations.

## 15. Stream-first native UI — design branch

Implement the approved ImageGen concept on `feat/stream-first-ui`: charcoal
surfaces, a compact sharing/member sidebar, and large 16:9 stream surfaces.
Move viewing actions into a right-click menu, with a hover ellipsis alternative;
retain explicit Watch/Stop subscriptions, mute/volume, focus and fullscreen.
Show the received publication's codec immediately left of its measured video
bitrate inside watched video only on mouse hover; fullscreen may fade its HUD
after mouse inactivity. Never show delivery metrics on an unsubscribed preview.

Keep source resolution and FPS controls, source-derived audio, start/minute
preview cadence and saved-room behavior. In short windows, pin sharing actions
and keep source/FPS accessible; source confirmation can be expanded in the sidebar.
Truncate long room names while preserving invitation, leave and language controls.
Default the automatic watching allowance to 30 Mbps when no download measurement
is available. Remove the connection-route selector and use automatic routing.
Match the approved concept's visual treatment with embedded Inter regular and
semibold fonts, charcoal controls, custom stream menu rows with vector icons,
volume slider and mute switch, consistent corner radii and compact identity chips.
Keep the stream menu open while changing audio settings; preserve keyboard and
pointer controls and distribute the font's OFL notice with the application.

Implement the subsequently approved settings/source mockups: roomier Advanced
groups (Video, Reception, Connection), two-column encoder/bitrate controls, short
status text, help tooltips and collapsed diagnostics. Remove the redundant
microphone-off row. Add a local source-list confirmation popover on the right:
hover/focus requests one still after a short debounce, clicking only changes the
pending source, and Start/Apply remains explicit. This authorized local capture
is separate from uploaded previews, whose start/minute gate is unchanged. Bound
the local worker/request/cache, cancel stale/dismissed work and release its
source-only pipeline without starting audio, encoding or a publication.

Keep the last pointer-hovered preview across row gaps and the preview popover;
use focused rows when navigating by keyboard. Retry a transient local capture
failure once after a cancellable delay. Refresh the source catalogue quietly
while its list is open and exclude hidden, minimized, DWM-cloaked, tool and
nonactivating windows from discovery. Preserve HWND/PID validation for an active
publication so minimizing an already shared window does not revoke its source.

Intercept native window close before process teardown. Cancel pending join and
capture authority, prioritize shutdown independently of the bounded command queue,
await stream unpublication and session departure, then allow window destruction.
Keep a responsive closing screen; offer explicit forced exit after ten seconds if
driver or network cleanup stalls. Repeated close requests must not skip cleanup.

Accept with native-renderer captures at 1280×720 and 960×540, open stream menus,
sharing, onboarding/error and focus states; native Clippy/tests and Node 24 checks;
and independent technical/visual review. Synthetic UI captures establish layout
only. Existing uncommitted media/packaging prerequisites and outstanding physical
hardware/network acceptance remain separate from this design branch.

## 16. Admin room directory and lifecycle — PR

Implement the approved charcoal/lavender admin concept with a sidebar, a New room
dialog and expandable room rows. Expanding a room reveals connected members,
Copy invite and a confirmed Close room action. Preserve the Usage dashboard.
New managed rooms wait for their first join; admitted rooms close after 60 seconds
of observed absence from the SFU. Presence failures and server restarts reset the
grace period. Closed rooms are deleted from the directory and lose invitations,
sessions, previews and signaling/media access through durable removal retries.
This intentionally supersedes the original persistent-empty-room behavior.
On schema-3 migration, existing rooms enter the presence check even if their old
sessions have already been removed. Successfully observed empty legacy rooms
close after the same grace period; only newly created managed rooms receive the
awaiting-first-join exception. Take the existing deployment backup before
migration; rolling back to schema-3 code requires its matching database backup.

Store recoverable invitations encrypted with a key derived from ADMIN_TOKEN,
bound to their room ID; retain hashes for admission. Rotation of ADMIN_TOKEN or
legacy hash-only invitations requires an explicit replacement invitation, without
disconnecting current members. Never expose invitations in the room directory,
logs, request URLs or browser storage. Historical usage remains available.

Accept with Node 24 checks, authenticated API lifecycle and migration coverage,
real HTTP/browser creation, expansion, member presence, clipboard, reload, search,
close/revocation and mobile checks; Docker smoke and independent high-reasoning
GPT-6.1-Sol review. No native hardware capability changes or new qualification.

## Later portability

After the Windows experience meets the above gates, add macOS and Linux in separate PRs. Preserve the room, invitation, subscription and preview contracts; replace platform capture/audio/encoder adapters. No cross-platform promise until each backend is tested on its actual OS.

## Administrator room directory — independent PR

This earlier directory behavior is superseded by phase 16 above.

Add a Rooms tab to the authenticated administrator dashboard. List persistent
rooms in bounded pages and create a named room without admitting the administrator
as a participant. Return a fresh invitation only on creation, with explicit copy
and manual-copy fallback; keep it in memory. Use the existing expiring HttpOnly
administrator session for both APIs, retaining native admission compatibility.

Accept with unauthenticated/participant/expired/revoked session denial, same-origin
mutation checks, restart persistence and invitation admission, bounded pagination,
Node 24 checks, desktop/mobile browser workflows and a fresh independent review.
Simpler native onboarding (app links or short-lived pairing) remains a proposal.

## Portable browser invitations — independent PR

Add optional per-user `freedomstream://` registration from Room options. Save a
content-addressed launcher copy in Local AppData so moving/deleting the downloaded
EXE does not break the handler. Re-enabling in a newer release updates the handler;
disabling removes only our registration and leaves cached files for safe running
processes. This is the explicit opt-in exception to default no-settings-change
portability; it needs no administrator privileges or shared runtime installation.

Invitation pages prepare a one-use 60-second handoff ticket and offer Open in
FreedomStream. Permanent invitations stay out of activation arguments and HTTP
URLs. Ticket storage is bounded and memory-only; restart, expiry, consumption,
invitation rotation or disabling invalidate handoff. The native app displays the
server and asks before loading; it never automatically joins a saved room on a
browser launch. Joining, sharing and watching remain explicit. A browser launch
opens a separate app window so existing streams are undisturbed.

Present native browser invitations in a centered modal over an inactive, lightly
softened and darkened onboarding screen. Keep server trust/Load and nickname/Join
explicit inside the modal; hide the long invitation field in this flow. Center
the dismiss action under Load on the first screen, retaining its existing
alignment on the nickname screen. Dismissal cancels pending work and never joins.

Accept with ticket authorization/replay/expiry/revocation/resource tests, real
per-user registry registration/removal and Windows launch handoff, malformed URL
rejection, native GUI loading without automatic admission, Node 24 checks,
native Clippy/tests, desktop/mobile browser workflows and independent review.
Keep untested browser prompts and deployment/distribution gates explicit.

During the native teardown follow-up, retain room, source-picker and browser-load
worker ownership through normal window close. Cancel pending work, poll full
Windows thread termination without blocking the closing UI, and retain explicit
Force close for stalled cleanup. Drain native test fixtures before their tests
finish. Validate cancellation, held cleanup/TLS destructors and real HTTP Load
behavior. Record the original unexplained fatal test-process exit independently;
passing retries and fixing demonstrated ownership gaps do not prove its cause or
waive native interaction acceptance.

## Invitation landing correction - independent PR

Keep invitation pages limited to a single Join room in FreedomStream action and
the connected room members. Prepare short-lived tickets before the click so
protocol navigation remains synchronous with the browser user gesture; refresh
and replace tickets without expanding the page. Do not create a browser room
session or request media tokens. Fetch current SFU members only after validating
the invitation in a POST body, filter against active room sessions, and recheck
invitation validity after the SFU request. Keep registration opt-in and native
server trust/name/join confirmation. Verify authorization, stale sessions,
revocation, replay/expiry, desktop/mobile layout and protocol navigation; report
actual Windows activation separately from browser navigation assertions.

Windows activation qualification exposed shell URL canonicalization: accept both
empty and root-slash `join` paths while rejecting other paths. Rebuild the
portable EXE and update the cached registered launcher; verify native Load/Join
and live member appearance before marking this correction deployed.

## On-demand stream diagnostics — independent PR (#26)

Add watched-stream Advanced information in both viewing menus, EN/PT-BR. Combine
sender aggregates relayed by authenticated room APIs with local receive/decode/UI
submission measurements. Default off; use three-second aggregates, ten-second
viewer demand leases, one shared publisher worker, bounded memory-only latest
samples and live track generations. Preserve subscriptions, preview cadence,
quality controls and compatibility. Expose unavailable/stale facts honestly,
including physical presentation and end-to-end latency. Keep performance fixes
separate. See [measurement semantics and verification](verification/stream-diagnostics.md).

Accept with authorization, multi-viewer demand, expiry/revocation/replacement,
resource/retention bounds, real publisher/API/native-viewer checks, native UI
interaction and fresh independent review. Compilation and synthetic frames do
not qualify actual screen refresh, hardware load or network degradation.

## Discord-created gatherings — independent PR

Support optional /stream and /telar guild commands through signed Discord HTTP
interactions. Each command creates a fresh managed room without issuing a browser
session and replies in the invoking channel with a fragment-only invitation.
Allow only configured guilds, suppress mentions and embeds, bound payloads and
creation rate, reject stale signatures and persist interaction replay protection.
Retain administrator-created rooms and existing short reconnect/automatic closure.

Replace native saved-room restoration with encrypted server-origin/name settings.
Migrate legacy encrypted invitations to server/name only and do not autojoin on
launch. Installer-managed links and existing-app activation retain the known-server
auto-load path using that saved origin; unfamiliar origins still need trust, and
all room joins require native confirmation. Keep update badge/download/Later/restart
controls independent of room admission. A fresh
invitation may select a different server explicitly; saving a server never grants
room authority. This supersedes earlier saved-room requirements for this phase.

Accept with signed HTTP tests (bad signatures, raw-body tampering, stale requests,
unauthorized guilds/DMs, retries/restarts, independent room creation and admission),
Node 24 checks, Windows DPAPI migration and native startup/join/forget checks,
independent GPT-6.1-Sol high review, and an actual registered Discord command in a
private test channel. Discord application setup, production HTTPS validation and
native UI qualification remain open until exercised. Do not merge on mocks alone.

## Public website and direct service domains — independent PR

Support optional `DIRECT_URL` alongside `PUBLIC_URL`, defaulting to the existing
single origin. Keep Discord/admin/share invitations on the website while native
admission, authenticated signaling and probes use the direct origin. Discover
the operator's direct origin before native admission; app handoff and native
Invite retain the canonical public invitation. Retain host-only session cookies, access
revocation and private SFU HTTP. Move binary bytes through bounded, expiring,
single-use download tickets in fragments and POST bodies, rechecking the issuing
room session without sharing cookies across domains. Deny bulk probes and ticket
redemption on the public host. Keep preview cadence and subscriptions unchanged.

Generate proxied website 443, DNS-only direct HTTPS 8443 and DNS-only TURN 5349
configuration and certificates, preserving existing services and private keys.
Document DNS/proxy flags, cache/challenge/logging policy and rollout compatibility.
Accept with Node 24 checks, real HTTP/browser download and cookie-isolation checks,
native discovery and direct upload/download measurement, nginx validation, Docker
smoke and independent GPT-6.1-Sol high review. Actual DNS/Cloudflare/TLS renewal,
WAN media and forced TURN remain deployment gates until exercised on the VPS.

## Global Discord commands and first public installer — corrective PR

Register /stream and /telar globally with guild-installation and guild-only
contexts. Support explicit `DISCORD_GUILD_IDS=*` admission only when Discord's
signed guild-installation owner matches the invoking guild. Preserve restricted
ID-list deployments, DM denial, signature/replay checks and creation quotas.
Configure a permanent production endpoint; a temporary tunnel is not deployment.

Before the first public Windows installer, fix package ID `PodeTelar` so the
installation folder cannot collide with legacy DPAPI settings in `FreedomStream`.
Preserve the fixed launcher/protocol, qualify real install/startup/shortcuts/room
handler/uninstall and profile preservation, then publish a clearly named Setup
asset alongside the portable release. Keep unsigned-preview, fresh-machine,
media-update and public update-feed limitations explicit. Locally verified build
artifacts may be used while GitHub runner billing is blocked; never call blocked
hosted jobs passing. Obtain an independent GPT-6.1-Sol high source review before
merging and record native source/artifact provenance separately.

## Invitation-selected native backend — corrective PR before v0.1 rebuild

Remove default/saved native server, nickname and language persistence. Each
Discord/browser handoff selects and loads its own backend without a saved-origin
gate. A fresh process asks for a nickname; an idle running app may reuse its
in-memory nickname. Confirm before switching an active room, never start capture
or subscriptions from activation, and cancel superseded handoffs without losing
worker ownership. Ignore and remove obsolete profile/preferences files.

Accept with native startup/link/cancellation checks, two independently hosted
backend origins, actual portable and installed protocol flows, Node 24 checks
and a fresh GPT-6.1-Sol high review. Rebuild and qualify both v0.1.0 Windows assets
from the merged source and verify downloaded bytes and provenance. Preserve the
installer's disabled public update feed and existing media qualification limits.
