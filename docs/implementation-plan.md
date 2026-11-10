# Implementation phases

Each phase is an acceptance gate. Phase 1 is the initial commit on `main`; every later phase gets its own branch and PR. Felipe authorized autonomous implementation and merging: each PR must pass its acceptance checks and a new GPT-6.1-Sol review at high reasoning before merge. Address review findings before proceeding. Code can be MIT licensed while third-party runtimes retain their own licenses; qualify distribution licenses before bundling native media dependencies. Report external hardware/deployment verification separately and do not waive it merely to finish the sequence.

**Current checkpoint:** Phases 1–8 and clipboard PR #8 are merged. Portable
packaging PR #9 remains a separate phase. Native room PR #10 is stacked on it as
a draft, with local native/media/DPAPI/portable checks and independent review
recorded in [Phase 11 verification](verification/phase-11.md). Remaining physical
hardware, deployment/network and real-game/group gates are not waived by those
results.

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

## Later portability

After the Windows experience meets the above gates, add macOS and Linux in separate PRs. Preserve the room, invitation, subscription and preview contracts; replace platform capture/audio/encoder adapters. No cross-platform promise until each backend is tested on its actual OS.

## Administrator room directory — independent PR

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

Accept with ticket authorization/replay/expiry/revocation/resource tests, real
per-user registry registration/removal and Windows launch handoff, malformed URL
rejection, native GUI loading without automatic admission, Node 24 checks,
native Clippy/tests, desktop/mobile browser workflows and independent review.
Keep untested browser prompts and deployment/distribution gates explicit.
