# Implementation phases

Each phase is an acceptance gate. Phase 1 is the initial commit on `main`; every later phase gets its own branch and PR. Felipe authorized autonomous implementation and merging: each PR must pass its acceptance checks and a new GPT-6.1-Sol review at high reasoning before merge. Address review findings before proceeding. Code can be MIT licensed while third-party runtimes retain their own licenses; qualify distribution licenses before bundling native media dependencies. Report external hardware/deployment verification separately and do not waive it merely to finish the sequence.

**Current checkpoint:** Phases 1 and 2 are merged. Phase 3 implements the native Windows publisher, with NVIDIA encoding and browser decoding through the Canada VPS verified. AMD hardware is unavailable; AMD, real-game/fullscreen/alt-tab and 4K120 qualification remain open external acceptance items. An implementation PR does not certify those results. See [Phase 2 verification](verification/phase-2.md) and [Phase 3 verification](verification/phase-3.md). Each remaining phase still requires checks and a fresh high-reasoning review before merge.

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

## Later portability

After the Windows experience meets the above gates, add macOS and Linux in separate PRs. Preserve the room, invitation, subscription and preview contracts; replace platform capture/audio/encoder adapters. No cross-platform promise until each backend is tested on its actual OS.
