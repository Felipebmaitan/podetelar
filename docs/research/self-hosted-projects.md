# Self-hosted screen sharing: existing OSS projects

Research date: **2026-11-08**. Scope: private sharing of a friend's local screen/game or camera, native capture where practical, NVIDIA/AMD encoding, system/application audio, low latency, a Docker-hosted VPS media route, browser viewers, and preferably reusable rooms with simultaneous streams.

This is a source audit, not a deployment or performance test. **Confirmed** below means documented or implemented in inspected first-party sources; it does not mean tested on the group's machines. Felipe subsequently confirmed Windows first, Linux/macOS later if practical, roughly four or five concurrent streams at peak, and a Canada-hosted VPS. Exact GPU models and viewer counts remain unknown. No software was installed or deployed.

## Decision

**Existing projects are close enough to trial before building anything.**

1. **First relay/capture trial: OBS → Broadcast Box → browser viewers.** This has a documented native publisher, server media path, and shareable playback links. Use different stream keys for different friends. Current `main` also has a multi-view player. OBS supplies NVIDIA/AMD encoding and application audio; it avoids depending on the sharing browser's screen-capture implementation. [Broadcast Box](https://github.com/Glimesh/broadcast-box), [OBS hardware encoding](https://obsproject.com/kb/hardware-encoding), [OBS application audio](https://obsproject.com/kb/application-audio-capture-guide).
2. **Closest integrated friend-group app: Piik.** Native Windows screen/window capture, browser invitations, room voice/chat, persistent hosted room authority, Docker, and **`SFU_ONLY=true`** are documented in v2.6.1. However, a room has **one sharing Host**, its video paths are H.264/VP8, and the project's own native AMD acceptance evidence remains incomplete. Trial it if one broadcaster per room is acceptable. [Piik release](https://github.com/TNTcraftHIM/Piik/releases/tag/v2.6.1), [deployment](https://github.com/TNTcraftHIM/Piik/blob/v2.6.1/docs/operations/self-hosting.md), [room contract](https://github.com/TNTcraftHIM/Piik/blob/v2.6.1/docs/standards/rooms-access.md), [native evidence](https://github.com/TNTcraftHIM/Piik/blob/v2.6.1/docs/research/native-client-media.md).
3. **Best ready-made meeting-room route: MiroTalk SFU + OBS WHIP.** Native publishers appear as participant tiles alongside browser cameras/screens. Increase the default one-WHIP-session-per-room limit for multiple native broadcasters; a browser participant must already have activated the room. [WHIP ingest](https://docs.mirotalk.com/mirotalk-sfu/whip/), [configuration defaults](https://github.com/miroslavpejic85/mirotalksfu/blob/b0df1bede17ccdd69bff949fc3a97f0be4992f96/.env.template).
4. **Additional native publisher worth testing: VDO.Ninja Game Capture.** Its Windows Qt app explicitly supports NVIDIA/AMD encoders, window/camera sources, application/system audio, browser links, and WHIP output. It could replace OBS for a simpler publisher; combining its WHIP output with Broadcast Box is a standards-based trial proposal, **not an interoperability result established here**. [Game Capture](https://github.com/steveseguin/game-capture), [Windows guide](https://github.com/steveseguin/game-capture/blob/v0.2.65/docs/windows-guide.md).

**No audited option establishes every requested feature together, especially universal browser HEVC playback, qualified NVIDIA+AMD native capture, persistent shared rooms, and multiple simultaneous independent broadcasters.** Server codec support, native encoder support, and actual browser decoding must be assessed separately.

## Requirement comparison

Legend: **Yes** = explicit first-party support; **Partial** = a condition, extra component, or narrower workflow; **No** = contrary to the inspected implementation/product model; **?** = not established. “Persistent” means durable room/channel identity or authority, not keeping live media running through a restart. Hardware columns refer to the publisher, not a GPU on the VPS.

| Project / evaluated workflow | Native Windows capture | NVIDIA + AMD encoding | System/game audio | Browser viewer | VPS Docker media route | Persistent rooms | Multiple broadcasters together | HEVC to browser |
|---|---|---|---|---|---|---|---|---|
| [Broadcast Box + OBS](#broadcast-box) | Yes, OBS | Yes, OBS; device dependent | Yes, OBS | Yes | Yes | Partial: stable stream keys/profiles, no meeting-room model | Yes across stream keys; multi-view in current main | Partial: server registers H.265; end-to-end playback unverified |
| [Piik](#piik) | Yes, native helper | Partial: hardware H.264; AMD physical acceptance open | Yes on supported Windows capture paths | Yes | Yes; SFU-only option | Yes for hosted room authority | No: one screen/camera Host per room | No in inspected H.264/VP8 paths |
| [MiroTalk SFU + OBS](#mirotalk-sfu) | Yes through OBS; browser client otherwise | Yes through OBS | Yes through OBS | Yes | Yes, SFU | Partial: reusable URL; durable idle room not established | Yes after raising WHIP per-room limit | No in documented router/WHIP codec set |
| [Game Capture + relay](#vdoninja-game-capture) | Yes | Yes, explicit NVENC/AMF paths; device dependent | Yes | Yes for VDO.Ninja output; relay supplies WHIP viewers | Partial: separate relay required | Partial: depends on receiving application | Partial: depends on receiving application | Partial: advanced native codecs; WHIP output is H.264 |
| [Screego](#screego) | No, browser capture | ? Browser-controlled | Yes where browser/OS supports it | Yes | Yes, integrated TURN, not SFU | Partial: recreate same room via URL | Yes, multi-user screenshare | ? Not established |
| [Deskreen CE](#deskreen-ce) | Partial: Electron desktop app | ? No vendor controls established | No in inspected capture code | Yes | ? No documented VPS relay product | ? Not established | ? Not a multi-publisher room product | ? Not established |
| [RustDesk OSS](#rustdesk) | Yes | Yes, native hardware codec library | Partial: desktop audio service; game suitability untested | Partial: self-hosted official web client is a Pro offering | Yes, hbbs/hbbr; force-relay available | No friend-group room model established | ? Separate remote sessions, not a shared stream grid | ? Native H.265 support does not establish browser parity |
| [Sunshine / Moonlight](#sunshine-and-moonlight) | Yes | Yes | Yes, loopback | No general browser viewer in core workflow | No turnkey VPS fan-out relay established | No friend-group room model | ? Multiple-client behavior is not independent broadcaster rooms | No browser path; native HEVC yes |
| [LiveKit Meet](#livekit-meet) | No native capture app included | ? Browser-controlled; OBS needs separate ingress | Partial: browser/OS-dependent capture | Yes | Partial: Meet frontend + separate self-hosted LiveKit | Partial: reusable URL; durable room registry not established | Yes, conferencing | ? Not established for this app |
| [VDO.Ninja core](#vdoninja-core) | No; Game Capture is separate | ? Browser-controlled in core | Partial: browser/OS dependent | Yes | Partial: frontend Docker is not a complete media stack | Partial: reusable room/link identifiers | Yes, room/group workflows | ? Browser and selected path dependent |
| [Galène + OBS](#galene) | Yes through OBS | Yes through OBS | Yes through OBS | Yes | Partial: single binary; upstream discourages Docker | Yes, configured group definitions | Yes, conference groups | No H.265 in documented group codec list |
| [Sharkord](#sharkord) | Partial: desktop app, capture/encoder parity not qualified here | ? No verified vendor encoder controls | Partial: screen audio advertised; exact OS/app path unqualified | Yes | Yes, mediasoup SFU | Yes, configured channels/database | Partial: group media supported; exact concurrent-share limits unverified | ? Not established |

The project notes below provide evidence and explain the qualifications in this matrix.

Camera contribution is confirmed for the OBS-based workflows, Piik's selectable Camera source, Game Capture, MiroTalk, LiveKit Meet, VDO.Ninja, Galène and Sharkord. A standalone camera publishing workflow was not established for Screego, Deskreen CE, RustDesk or Sunshine; displaying a camera app on a captured desktop is a different workaround. Relevant primary sources are linked in each project audit.

## Project audits

### Broadcast Box

Canonical repository: **[Glimesh/broadcast-box](https://github.com/Glimesh/broadcast-box)**, currently unarchived. The inspected default branch is `main`, commit [`259aaa19c3cce13f3f93e638181029105d3ea7ed`](https://github.com/Glimesh/broadcast-box/commit/259aaa19c3cce13f3f93e638181029105d3ea7ed).

- **Confirmed:** WHIP publishing, WHEP playback, built-in browser player, Docker/Compose instructions, and a stream-key playback URL. Current main documents browser screen/webcam publishing and adding multiple streams to one player. Native capture comes from OBS or another publisher. [Current README](https://github.com/Glimesh/broadcast-box/blob/259aaa19c3cce13f3f93e638181029105d3ea7ed/README.md).
- **Codec evidence:** both v2.0.2 and inspected main register H.265, H.264, AV1 and VP9 in the server's codec table. That establishes signaling capability, **not a working HEVC chain on every browser**, or transcoding into a compatible fallback. [Tagged codec source](https://github.com/Glimesh/broadcast-box/blob/v2.0.2/internal/webrtc/codecs/codecs.go).
- **Access model:** default publishing/playback use the same chosen key. Reserved profiles restrict publishing; the webhook mechanism can separate publish and watch authorization. A stream marked private for discovery is not by itself a complete invitation/account system. [Authorization documentation](https://github.com/Glimesh/broadcast-box/blob/v2.0.2/README.md#webhook---authentication-and-logging), [authorization helpers](https://github.com/Glimesh/broadcast-box/blob/259aaa19c3cce13f3f93e638181029105d3ea7ed/internal/server/authorization/helpers.go).
- **Version qualification:** latest published GitHub release is **v2.0.2, 2026-02-21**. Its README lacks the current browser publisher/multi-view descriptions. Treat those UI features as **verified on main, not guaranteed in v2.0.2**; pin the actual trial revision/image. Upstream's example latency is not a WAN guarantee. [Release](https://github.com/Glimesh/broadcast-box/releases/tag/v2.0.2), [tagged README](https://github.com/Glimesh/broadcast-box/blob/v2.0.2/README.md).

**Fit:** strongest small streaming appliance to trial with OBS. It still needs room/social UX if Discord-style persistent shared rooms become mandatory.

### Piik

Canonical repository: **[TNTcraftHIM/Piik](https://github.com/TNTcraftHIM/Piik)**, linked from [piik.tv](https://piik.tv/). Audit of release **[v2.6.1](https://github.com/TNTcraftHIM/Piik/releases/tag/v2.6.1)**, published 2026-11-08, whose source revision is `07e37c722869955a183aee523f45fcbc7119964d`.

- **Confirmed native capture:** Windows helper uses Windows Graphics Capture/D3D11 with Media Foundation H.264 or software VP8. Screen audio uses output loopback; window audio can include only the selected process tree. Camera is another selectable source; do not assume simultaneous separate camera-plus-screen publications. [Windows helper](https://github.com/TNTcraftHIM/Piik/blob/v2.6.1/native/capture/windows/README.md), [sharing/audio guide](https://github.com/TNTcraftHIM/Piik/blob/v2.6.1/docs/guide/sharing.md).
- **Confirmed VPS option:** one server packages UI, signaling, room management and optional embedded SFU. Docker Compose is documented. Set `SFU_UDP_PORT` to enable forwarding and **`SFU_ONLY=true`** to route every room through the server without P2P convergence. The default setup, hosted demo and App Public invite are P2P-oriented; merely self-hosting the website does not select server-only media. Both media modes require usable UDP. [Deployment](https://github.com/TNTcraftHIM/Piik/blob/v2.6.1/docs/operations/self-hosting.md#optional-media-fallback), [configuration](https://github.com/TNTcraftHIM/Piik/blob/v2.6.1/docs/standards/configuration.md#optional-services).
- **Confirmed rooms:** one Host plus up to 20 authenticated Viewers; invitation credentials and optional passwords; hosted SQLite persists room authority across restarts. Default retention is indefinite, but explicitly configured empty-room reclamation can invalidate invitations. Participants, capture, media routes and voice state are not persisted; App-local rooms use memory. [Room/storage contract](https://github.com/TNTcraftHIM/Piik/blob/v2.6.1/docs/standards/rooms-access.md).
- **Important limits:** codec selection is H.264/VP8, with no HEVC option in inspected paths. The project's retained tests demonstrate native NVIDIA H.264 but explicitly leave native AMD/Intel physical acceptance open; recorded AMD MFT activation failures are not proof of successful AMD encoding. Do not label Piik “qualified on both GPU vendors.” [Codec source](https://github.com/TNTcraftHIM/Piik/blob/v2.6.1/src/client/webrtc/video-codec.ts), [native media evidence](https://github.com/TNTcraftHIM/Piik/blob/v2.6.1/docs/research/native-client-media.md#current-boundary).

**Fit:** closest ready-made private viewing experience, especially for one friend hosting at a time. Several rooms do not equal several simultaneous screen publishers in the same room.

### MiroTalk SFU

Canonical repository: **[miroslavpejic85/mirotalksfu](https://github.com/miroslavpejic85/mirotalksfu)**. Inspected main commit [`b0df1bede17ccdd69bff949fc3a97f0be4992f96`](https://github.com/miroslavpejic85/mirotalksfu/commit/b0df1bede17ccdd69bff949fc3a97f0be4992f96).

**Confirmed:** browser conferencing with cameras/screens, room access controls, mediasoup SFU, and Docker. OBS WHIP ingest appears as an ordinary room participant; it is **ingress**, distinct from MiroTalk's RTMP/WHIP publishing-to-other-services features. [Repository](https://github.com/miroslavpejic85/mirotalksfu), [WHIP ingest guide](https://docs.mirotalk.com/mirotalk-sfu/whip/).

WHIP is disabled by default. Inspected defaults are five sessions server-wide, **one per room**, and 6 Mbps incoming per session. Raise the per-room count for multiple OBS publishers. A Bearer token is required; per-room tokens are supported. [Environment template](https://github.com/miroslavpejic85/mirotalksfu/blob/b0df1bede17ccdd69bff949fc3a97f0be4992f96/.env.template).

The destination must already be active with a participant; WHIP does not create it. When the last ordinary participant leaves, WHIP sessions end and the room closes. Documented codecs are Opus with H.264/VP8/VP9, with no HEVC listed. WHIP authorization bypasses the ordinary room password/lobby path, so use room-scoped publisher credentials. A reusable URL does not establish durable idle-room membership. [WHIP requirements, lifecycle and security notes](https://docs.mirotalk.com/mirotalk-sfu/whip/).

**Fit:** a serious near-turnkey choice when native contribution and multi-person conversation must share one interface. More conferencing functionality than Broadcast Box; some WHIP setup is required.

### VDO.Ninja Game Capture

Additional close candidate: **[steveseguin/game-capture](https://github.com/steveseguin/game-capture)**, release **[v0.2.65, 2026-10-18](https://github.com/steveseguin/game-capture/releases/tag/v0.2.65)**. This is a native Qt app, separate from VDO.Ninja's browser frontend. Windows packages are available; macOS is explicitly experimental. [README](https://github.com/steveseguin/game-capture/blob/v0.2.65/README.md).

**Confirmed:** window/game, webcam and Spout2 sources; application/system audio and optional microphone mixing; VDO.Ninja viewer links; explicit NVIDIA, AMD AMF and Intel encoder paths. WHIP accepts an endpoint and optional Bearer token. Its documented external-output mode uses **H.264 + Opus for WHIP**; selecting HEVC for another mode does not make its WHIP route HEVC. VDO.Ninja room controls/viewer links apply to VDO.Ninja output, not automatically to arbitrary WHIP servers. [Windows guide](https://github.com/steveseguin/game-capture/blob/v0.2.65/docs/windows-guide.md).

The release records defaults of 720p60/4 Mbps and hardware-first H.264, plus receiver-dependent HEVC/AV1 compatibility and unqualified clean-machine/HDR/overnight scenarios. This is valuable disclosed evidence, not proof of reliability on this group. [Release notes](https://github.com/steveseguin/game-capture/releases/tag/v0.2.65), [encoder policies](https://github.com/steveseguin/game-capture/blob/v0.2.65/docs/streaming-defaults.md).

**Fit:** particularly useful as a reusable native publisher. Trial against Broadcast Box or MiroTalk through WHIP; both pairings remain untested here.

### Screego

Canonical repository: **[screego/server](https://github.com/screego/server)**. Browser screen sharing, multiple users, Docker/single binary, and integrated STUN/TURN are confirmed. Its media topology is peer connections with TURN relay when needed, rather than an SFU forwarding one upload to many. [README](https://github.com/screego/server), [NAT documentation](https://github.com/screego/server/blob/master/docs/nat-traversal.md).

**Correction to stale comparisons:** v1.12.0 added system/tab audio on supported systems. Current source requests audio in `getDisplayMedia`. It remains browser/OS dependent, with no native game capture, selectable NVIDIA/AMD encoder backend, or documented HEVC workflow established. `forceTurn=true` selects relay-only ICE in the client; TURN authentication is required by default on self-hosted instances. This can avoid direct P2P while retaining a separate publisher connection per viewer. [Releases](https://github.com/screego/server/releases), [client source](https://github.com/screego/server/blob/master/ui/src/useRoom.ts).

The FAQ documents `create=true` for recreating a named room automatically, not durable room storage. [FAQ](https://github.com/screego/server/blob/master/docs/faq.md).

**Fit:** quick browser-only comparison, but it retains the capture dependency the user wants to escape.

### Deskreen CE

Canonical repository: **[pavlobu/deskreen](https://github.com/pavlobu/deskreen)**. The current Community Edition is an Electron desktop-to-browser second-screen product for Windows/macOS/Linux, with a separate Pro offering mentioned in its README. The inspected screen/window capture function explicitly uses `audio: false`. [Current README](https://github.com/pavlobu/deskreen), [capture source](https://github.com/pavlobu/deskreen/blob/master/src/renderer/src/features/PeerConnection/getDesktopSourceStreamBySourceID.ts).

No first-party turnkey VPS fan-out relay, persistent friend rooms, native HEVC workflow, or vendor-selectable encoder implementation was established. Its desktop packaging alone should not be interpreted as a native capture/codec stack comparable to OBS or Sunshine.

**Fit:** weak for internet friend-group gaming/audio. Keep for local second-screen use.

### RustDesk

Canonical repositories: **[rustdesk/rustdesk](https://github.com/rustdesk/rustdesk)** and **[rustdesk/rustdesk-server](https://github.com/rustdesk/rustdesk-server)**. Native remote desktop, self-hosted rendezvous/relay and Docker are documented. Server `ALWAYS_USE_RELAY=Y` disables direct connections. [Self-hosting](https://rustdesk.com/docs/en/self-host/), [Docker](https://rustdesk.com/docs/en/self-host/rustdesk-server-oss/docker/), [server configuration](https://github.com/rustdesk/rustdesk-server#configuration).

Native H.264/H.265 hardware codecs are documented, with NVIDIA NVENC and AMD AMF Windows paths in its hardware codec library; desktop audio is a server-side service. These do not prove an equivalent browser viewer or low-latency group watching experience. [Feature documentation](https://rustdesk.com/docs/en/index.html), [hwcodec](https://github.com/rustdesk-org/hwcodec), [client structure](https://github.com/rustdesk/rustdesk#file-structure).

The official self-hosted web client is offered with qualifying **Server Pro** plans. The OSS relay's WebSocket ports are not proof that a complete free web viewer ships with OSS. No persistent multi-broadcaster room UI was verified. [Official pricing/FAQ](https://www.rustdesk.com/pricing/).

**Fit:** use if the actual need becomes remote assistance/control with installed clients; poor match to a free browser viewing party.

### Sunshine and Moonlight

Canonical repositories: **[LizardByte/Sunshine](https://github.com/LizardByte/Sunshine)** and **[moonlight-stream/moonlight-qt](https://github.com/moonlight-stream/moonlight-qt)**. Sunshine confirms low-latency native hosting with AMD/Intel/NVIDIA hardware encoding. Moonlight confirms H.264, HEVC, AV1 and audio playback. Sunshine's web UI configures/pairs clients; it is not the stream viewer. [Sunshine README](https://github.com/LizardByte/Sunshine), [Moonlight README](https://github.com/moonlight-stream/moonlight-qt).

HEVC capability and loopback audio are explicit in Sunshine configuration. The usual viewer is an installed Moonlight client. No turnkey Docker VPS fan-out relay, browser invitation room, or simultaneous independent broadcasters in one shared room was established. Do not confuse “self-hosted game-stream host” with a server that relays friends' local PCs. [Configuration](https://docs.lizardbyte.dev/projects/sunshine/latest/md_docs_2configuration.html), [Moonlight client platforms](https://moonlight-stream.org/).

**Fit:** strongest native HEVC alternative when installed viewers and a different room/network model are acceptable.

### LiveKit Meet

Canonical repository: **[livekit-examples/meet](https://github.com/livekit-examples/meet)**. An Apache-licensed Next.js conferencing application using LiveKit Components, not a bundled native capture client or a one-container full media deployment. The environment allows a chosen `LIVEKIT_URL` and API credentials, so a self-hosted LiveKit backend is possible. [README](https://github.com/livekit-examples/meet), [environment template](https://github.com/livekit-examples/meet/blob/main/.env.example).

Camera/screen conference UI and multi-participant media are present. The sample token endpoint accepts a requested room/name and grants publishing/subscribing without a group-membership check in that handler. A private-group deployment needs an access boundary beyond merely knowing the example application's URL. [Conference source](https://github.com/livekit-examples/meet/blob/main/app/rooms/%5BroomName%5D/PageClientImpl.tsx), [token route](https://github.com/livekit-examples/meet/blob/main/app/api/connection-details/route.ts).

Native capture, OBS ingress, durable room records and any HEVC workflow require separate backend/client decisions; they are not established by Meet itself.

**Fit:** reusable UI and application foundation. The main agent's separate LiveKit architecture audit should decide the backend/ingress details.

### VDO.Ninja core

Canonical repository: **[steveseguin/vdo.ninja](https://github.com/steveseguin/vdo.ninja)**. Default branch `develop` is a preview; published v30.2 is a separate release reference. Browser camera/screen contributions, rooms, viewer links and WHIP/WHEP helpers are documented. The separate Game Capture project above addresses native Windows capture. [Repository](https://github.com/steveseguin/vdo.ninja).

**Deployment qualification:** an HTTPS static site or `docker-vdon` frontend still uses public connection services by default. Fully owned infrastructure requires explicitly choosing/deploying signaling and STUN/TURN; an optional SFU is another media-path choice. Hosting the frontend alone does not change the default P2P media route into a VPS SFU. [Self-hosting sections](https://github.com/steveseguin/vdo.ninja#backend-services-and-self-hosting), [TURN guide](https://github.com/steveseguin/vdo.ninja/blob/develop/turnserver.md).

Reusable room/link identifiers are supported, but a durable friend-room database comparable to Piik's hosted room authority was not verified. The inspected core license is **AGPL**, not MIT; original examples have a separate MIT exception. [Current licensing](https://github.com/steveseguin/vdo.ninja/blob/ba97fb07fc508797a4ef2628d268649e9de3b894/LICENCE.md), [v30.2 licensing](https://github.com/steveseguin/vdo.ninja/blob/v30.2/LICENCE.md).

**Fit:** extensive contribution/viewer toolkit, with more assembly for a completely self-hosted media route.

### Galene

Additional candidate: **[jech/galene](https://github.com/jech/galene)**. A Go conferencing/SFU server with browser UI and groups configured in files. Persistent group definitions, users/permissions and tokens provide a useful reusable-room model. [README](https://github.com/jech/galene), [group administration](https://github.com/jech/galene/blob/master/galene.md).

OBS WHIP contribution has been documented by the maintainer since Galène 0.8 / OBS 30; group codecs must include H.264/Opus. Its documented video list has VP8/VP9/AV1/H.264, no H.265. Native capture/audio/hardware encoding would come from OBS. [Maintainer's OBS setup](https://lists.galene.org/galene/87h6kvar08.wl-jch%40irif.fr/T/), [codec configuration](https://github.com/jech/galene/blob/master/galene.md).

Upstream explicitly **does not recommend Docker**, preferring an internet-connected single binary. This is a deployment preference mismatch, not inability to run in a container. [Official FAQ](https://galene.org/faq.html#is-it-possible-to-run-galene-within-a-docker-container).

**Fit:** good lightweight meeting alternative if the Docker preference can bend.

### Sharkord

Additional candidate: **[Sharkord/sharkord](https://github.com/Sharkord/sharkord)**. Explicitly alpha. It combines persistent text/voice channels, cameras/screens, invitations/roles, a built-in mediasoup SFU and Docker distribution; a desktop app is separate. [Repository](https://github.com/Sharkord/sharkord), [quick start](https://sharkord.com/docs/introduction/quick-start).

The desktop app loads the server's web client and adds native integration and a screen picker. That is insufficient evidence of a separate native GPU encoding stack. Exact NVIDIA/AMD controls, HEVC, native audio behavior and simultaneous screen-publication limits remain unverified. Its media port is documented for both TCP and UDP; durable channels do not preserve live sessions. [Desktop architecture](https://sharkord.com/docs/desktop-app), [media configuration](https://sharkord.com/docs/configuration).

A searched `/docs/voice` page returned 404 when directly opened; its indexed codec/audio statements were not treated as confirmation. Recent release notes include voice/media fixes, which show activity but also reinforce the alpha qualification. [v0.0.30](https://github.com/Sharkord/sharkord/releases/tag/v0.0.30).

**Fit:** worth a secondary trial if a self-hosted Discord-like room interface matters more than proven native capture/vendor encoding.

## Licenses and maintenance signals

All listed repositories were reported **unarchived** by the public GitHub API during this audit. “Pushed” is the repository metadata timestamp, not proof of a tested release, active support, security review, or healthy contribution diversity. License identifiers apply to the inspected first-party repository; dependencies and separate proprietary services have their own terms.

| Repository | Inspected license | Latest stable release observed | Repository pushed date, UTC |
|---|---|---|---|
| [Broadcast Box metadata](https://api.github.com/repos/Glimesh/broadcast-box) | MIT | [v2.0.2 — 2026-02-21](https://github.com/Glimesh/broadcast-box/releases/tag/v2.0.2) | 2026-11-07 |
| [Piik metadata](https://api.github.com/repos/TNTcraftHIM/Piik) | MIT, Piik-owned code; dependency notices separate | [v2.6.1 — 2026-11-08](https://github.com/TNTcraftHIM/Piik/releases/tag/v2.6.1) | 2026-11-08 |
| [MiroTalk SFU metadata](https://api.github.com/repos/miroslavpejic85/mirotalksfu) | AGPL-3.0; separate commercial license offered | No published latest-release API result; inspected main revision above | 2026-11-08 |
| [Game Capture metadata](https://api.github.com/repos/steveseguin/game-capture) | AGPL-3.0 | [v0.2.65 — 2026-10-18](https://github.com/steveseguin/game-capture/releases/tag/v0.2.65) | 2026-11-05 |
| [Screego metadata](https://api.github.com/repos/screego/server) | GPL-3.0 | [v1.12.6 — 2026-09-27](https://github.com/screego/server/releases/tag/v1.12.6) | 2026-10-25 |
| [Deskreen metadata](https://api.github.com/repos/pavlobu/deskreen) | AGPL-3.0 for current CE | [v3.2.16 — 2026-07-08](https://github.com/pavlobu/deskreen/releases/tag/v3.2.16) | 2026-07-08 |
| [RustDesk client metadata](https://api.github.com/repos/rustdesk/rustdesk) | AGPL-3.0 | [1.5.1 — 2026-11-01](https://github.com/rustdesk/rustdesk/releases/tag/1.5.1) | 2026-11-08 |
| [RustDesk OSS server metadata](https://api.github.com/repos/rustdesk/rustdesk-server) | AGPL-3.0 | [1.1.16 — 2026-07-20](https://github.com/rustdesk/rustdesk-server/releases/tag/1.1.16) | 2026-11-08 |
| [Sunshine metadata](https://api.github.com/repos/LizardByte/Sunshine) | GPL-3.0 | [v2026.914.233613 — 2026-09-15](https://github.com/LizardByte/Sunshine/releases/tag/v2026.914.233613) | 2026-11-08 |
| [Moonlight Qt metadata](https://api.github.com/repos/moonlight-stream/moonlight-qt) | GPL-3.0 | [v6.2.0 — 2026-10-04](https://github.com/moonlight-stream/moonlight-qt/releases/tag/v6.2.0) | 2026-11-06 |
| [LiveKit Meet metadata](https://api.github.com/repos/livekit-examples/meet) | Apache-2.0 | No published latest-release API result; example application | 2026-11-06 |
| [VDO.Ninja metadata](https://api.github.com/repos/steveseguin/vdo.ninja) | AGPL-3.0-only in inspected develop license; exceptions per file | [v30.2 — 2026-05-12](https://github.com/steveseguin/vdo.ninja/releases/tag/v30.2); develop reports session 31.4 | 2026-11-07 |
| [Galène metadata](https://api.github.com/repos/jech/galene) | MIT | [1.2.1 — 2026-09-08 in CHANGES](https://github.com/jech/galene/blob/master/CHANGES); 1.2.2 marked unreleased | 2026-11-06 |
| [Sharkord metadata](https://api.github.com/repos/Sharkord/sharkord) | MIT | [v0.0.30 — 2026-11-06](https://github.com/Sharkord/sharkord/releases/tag/v0.0.30), alpha | 2026-11-06 |

Piik was created on **2026-08-18** and Game Capture on **2026-02-28**, per their API metadata. Their release activity is encouraging but does not establish years of operational maturity. A missing `releases/latest` result is not evidence of abandonment or absence of tags.

## Recommended trial sequence and reusable parts

These are proposed evaluation steps, not actions performed by this research task.

1. **Establish capture and relay reliability with OBS + Broadcast Box using H.264/Opus.** Two different friends publish two keys; another friend watches both. Check game/window capture, camera source, intended audio, NVIDIA and AMD separately, and a real cross-network path. Choose a known revision; current main is needed for the multi-view feature established here.
2. **Compare Piik's product experience on its self-hosted server with native App capture and `SFU_ONLY=true`.** Preserve its data volume; verify the same invitation/Host authority after a restart while expecting media to reconnect. Test the AMD machine early because that requirement is not qualified by upstream's native evidence. Do not use its P2P-only public demo to judge the forced-server deployment.
3. **If a shared meeting room is essential, compare MiroTalk SFU with two OBS WHIP publishers.** Enable WHIP, raise the per-room/session/bitrate limits appropriately, use per-room tokens, and join in a browser first. This directly tests whether a complete existing room UI is sufficient.
4. **Try Game Capture as the alternative native sender.** Explicit AMD AMF and NVIDIA paths plus camera/application audio make it particularly relevant. Its arbitrary-WHIP-server combinations need an interoperability trial; do not assume its VDO.Ninja-generated viewer link points to that external relay.
5. **Test HEVC only after a compatible baseline works.** Record the actual browser/OS/GPU, negotiated codec and decoder behavior for every intended viewer. Broadcast Box's H.265 registration and Sunshine's native HEVC support are useful evidence, but neither establishes universal browser HEVC viewing.

Reusable components, if a custom product is still justified:

- **OBS or Game Capture:** existing native Windows capture, audio routing and vendor encoders. Use as publishers before writing a capture client.
- **Broadcast Box:** compact WHIP/WHEP relay and browser playback; multi-view and authorization hooks are relevant extension points.
- **Piik:** a complete alternative product and MIT reference for room invitations, persistence, App capture and embedded forwarding. Its one-Host room model and H.264/VP8 scope are architectural constraints to consider before reuse.
- **MiroTalk SFU / Galène:** ready-made conference rooms with native OBS contribution; compare their operating and licensing models.
- **LiveKit Meet:** reusable meeting interface/components when the separate LiveKit/ingress architecture audit supports that choice.
- **Sunshine / Moonlight:** a native HEVC benchmark/alternative workflow, with a different viewer and room model.

The claim supported by this audit is **“several near-turnkey trials exist,” not “everything must be built” or “one option already satisfies the full specification.”**

## Evidence limits and inconsistencies

- Primary sources only were used for conclusions: official repositories, source, release records, docs and a maintainer mailing-list post. Search results helped discover candidates; third-party directories and forum recommendations were not used as proof.
- Branch and release documentation can differ. Broadcast Box multi-view/browser publishing and VDO.Ninja develop features must not be silently attributed to older release images. Piik's inspected deployment/Windows documents matched v2.6.1; Game Capture's inspected Windows guide matched v0.2.65.
- Piik's guide mentions Safari/iOS 27. That browser/OS claim was **not independently verified as available on 2026-11-08** and is not used to establish mobile screen sharing here. This report focuses on the confirmed Windows-first publishing requirement.
- Some indexed pages are stale: Screego “no audio” descriptions conflict with upstream release/source evidence; Sharkord's indexed voice guide returned 404. Directly inspected current primary sources take priority.
- No runtime measurements, VPS sizing guarantees, security certification, browser HEVC compatibility guarantee, or conclusion about the causes of TeamSpeak/crystal.lat failures is established by this research. The main agent's separate architecture/codec/privacy audit remains complementary.
