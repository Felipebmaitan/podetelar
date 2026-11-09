# FreedomStream: architecture research and discussion

Research date: 2026-11-08. Brainstorming only: no product implementation or deployment. Recommendations below are engineering judgments; referenced capabilities come from first-party documentation or source. No candidate has been tested with the friends' actual machines or ISPs.

## Requirements supplied by Felipe

- Native Windows broadcaster first, with a practical path to Linux and macOS later.
- NVIDIA and AMD hardware acceleration; NVIDIA machines are roughly newer than the GTX 10 / RTX 20 generations, exact models unknown.
- Good quality and motion at constrained bitrate, with HEVC/H.265 a primary requirement.
- Capture monitors, windows/games, and game/system audio; support cameras too.
- Reliable connections through the VPS rather than requiring each pair of friends to connect directly.
- Browser viewing through shareable links; persistent private rooms are preferable to separate temporary stream pages.
- Several people can publish and watch concurrently; peak is roughly four or five streams in one room.
- VPS is in Canada, with 4 vCPUs, 8 GB RAM shared with other workloads, unlimited monthly transfer, and a 1 Gbps connection in each direction. Sender uplinks, viewer counts, and available CPU/memory headroom remain unknown.
- Selecting which streams to watch is required. Compressed thumbnails are desirable; a Discord-like streaming-room interface is preferred over Broadcast Box's existing UI.
- Users should choose bitrate presets spanning 3–100 Mbps, resolution up to the selected source's native resolution with a 4K ceiling, and frame rates up to 120 fps where supported.
- Resolution/frame-rate choices should produce an automatic bitrate recommendation, with an independent manual bitrate override that does not change those choices.
- Captured audio must omit Discord so viewers do not hear themselves. Watched stream playback must also be kept out of the captured mix.
- Reuse a finished project where it fits; develop only what existing software does not solve satisfactorily.

The reported TeamSpeak failures are consistent with connectivity problems, but the cause was not diagnosed. Do not treat NAT as a proven cause, or assume any relay guarantees success on every network.

## Working recommendation

**Update after product clarification:** the intended product now clearly needs a custom Discord-like room interface. Broadcast Box remains a possible transport component or diagnostic tool; its existing interface is not the proposed final product. Prefer evaluating LiveKit for room membership and selective subscriptions, with a native Windows capture/encoder backend and a custom desktop/web interface. MediaMTX or Broadcast Box remain alternatives if their media path is a better fit. Keep this decision conditional on HEVC, encoder feedback, high-frame-rate and browser playback validation; none has been benchmarked here.

Evaluate near-turnkey alternatives described in [the project comparison](self-hosted-projects.md) before committing to a build. The first recommended trial is **OBS Studio → HTTPS WHIP → Broadcast Box → browser viewers**, with separate stream keys and the multi-view UI on the audited main revision. Its server codec table registers H.265. Those features establish a promising experiment, not successful HEVC playback on the group's actual browsers. Multi-view is verified on main and should not be assumed present in the older v2.0.2 release image. Configure access control before sharing a public deployment: the default key serves both publishing and viewing. [Audited README](https://github.com/Glimesh/broadcast-box/blob/259aaa19c3cce13f3f93e638181029105d3ea7ed/README.md), [tagged codecs](https://github.com/Glimesh/broadcast-box/blob/v2.0.2/internal/webrtc/codecs/codecs.go).

**MediaMTX is an alternative media-path test**, especially when its path-level authentication and control API are useful. OBS supplies an established native capture/encoding stack; MediaMTX documents WHIP publishing and browser playback. A trial with either relay should establish whether HEVC, native audio, and the existing VPS route actually feel good for this group. Neither has been deployed here. [OBS WHIP guide](https://obsproject.com/kb/whip-streaming-guide), [MediaMTX OBS publishing](https://mediamtx.org/docs/publish/obs-studio), [MediaMTX WebRTC publishing](https://github.com/bluenviron/mediamtx/blob/main/docs/3-publish/05-webrtc-clients.md).

If a custom product is still useful, favor a small desktop experience and room/access-control layer over writing new capture, codec, or WebRTC infrastructure. For a full multi-participant room product, **LiveKit is a strong candidate**, while **MediaMTX is a simpler candidate for independently published streams collected into a room UI**. This is a shortlist, not a final backend decision: validate the exact HEVC publisher-to-browser combination first.

## Separate capture from distribution

The browser is not inherently the cause of low bitrate or poor playback. The useful separation is:

1. A native publisher controls screen/game capture, audio isolation, GPU encoding, and local performance.
2. A forwarding media server distributes compressed streams without normal decode/re-encode.
3. Browsers decode and display streams, subject to codec/device capabilities.
4. A small control service manages persistent rooms, identities/sessions, invitations, permissions, and active stream metadata.

Each broadcaster should upload one rendition to the server per published track/layer. A forwarding server sends that track to subscribers. A TURN relay can help a mesh application connect, but does not by itself replace the mesh with upload-once fan-out. An SFU or suitable media router provides the latter. TURN remains useful as a fallback between a participant and the media server on restrictive networks. LiveKit documents UDP, TCP, and TURN/TLS options; MediaMTX also documents ICE connectivity and Docker/public-IP considerations. [LiveKit ports](https://docs.livekit.io/transport/self-hosting/ports-firewall/), [MediaMTX connectivity](https://mediamtx.org/docs/features/webrtc-specific-features).

Suggested deployment shape: a Docker Compose stack with the chosen media server, room service/web client, HTTPS entry point, and TURN if not supplied by the media server. Prefer UDP for media and an authenticated TCP/TLS fallback, including port 443 where supported and needed. HTTPS signaling working does not establish that the media path works. Map media ports and advertise reachable server addresses correctly. Avoid making a web-only proxy the sole exposed path for a UDP-based media service.

No GPU should be required on the VPS for ordinary forwarding. CPU, memory, packet rate, NIC capacity, and traffic allowance still require measurement; do not promise a particular VPS size before testing. Server transcoding would materially change the cost and architecture.

## Native capture and portable implementation choices

OBS already has native game/window capture and per-application audio capture on supported Windows versions, plus NVIDIA NVENC and AMD AMF integration. That is a more credible starting point than writing all capture paths afresh. OBS's developer documentation describes libobs and the modules that provide capture, encoders, and outputs. [Game capture](https://obsproject.com/kb/game-capture-source), [application audio](https://obsproject.com/kb/application-audio-capture-guide), [hardware encoding](https://obsproject.com/kb/hardware-encoding), [libobs design](https://docs.obsproject.com/backend-design), [modules](https://docs.obsproject.com/plugins).

Potential routes, from lowest initial media risk to most custom work:

- Use ordinary OBS with a prepared profile for the first real-world test.
- Give OBS a small companion/launcher that manages the selected room and publishing credentials.
- Build a focused native interface using libobs and the relevant plugins.
- Use native OS capture plus FFmpeg/GStreamer or vendor encoder APIs where a smaller independent capture engine is justified.

Rust can be useful for UI/control logic, but mature capture and video libraries will still involve native dependencies and FFI. C++/Qt is also viable. Neither language guarantees a small installer, zero-copy frames, or good encoding. Choose after the media path is validated. If distributing a product using libobs, evaluate the actual project's GPL license and dependency licenses before choosing an integration strategy; this note is not a licensing determination. [OBS license](https://github.com/obsproject/obs-studio/blob/master/COPYING).

Design platform boundaries around screen capture, audio capture, hardware encoders, and rendering. Windows can use Windows Graphics Capture and native game-capture facilities, plus WASAPI loopback. Microsoft's process-loopback sample demonstrates including or excluding a process tree; OS version support must be checked against the actual API implementation. A later macOS backend can use ScreenCaptureKit; a Linux/Wayland backend can use the desktop portal/PipeWire ecosystem. These are replacement capture backends, not reasons to replace the server protocol or room model. [Microsoft application loopback](https://learn.microsoft.com/en-us/samples/microsoft/windows-classic-samples/applicationloopbackaudio-sample/), [Apple ScreenCaptureKit](https://developer.apple.com/documentation/screencapturekit), [XDG ScreenCast portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html).

Default to **selected application's audio**, keeping room voice and watched streams out. Otherwise someone sharing while watching can send other participants' audio back to them. Keep microphone and game audio as separate concepts; game/music audio should not receive voice-oriented noise suppression or automatic gain processing. Provide an explicit desktop-audio mode with Discord exclusion for users who want it.

Windows process-loopback capture can exclude one process tree, as Microsoft's sample demonstrates. Discord exclusion needs to follow the actual audio-rendering processes through restarts. Excluding Discord alone does not prevent recapturing other watched streams. Excluding both unrelated process trees is not equivalent to passing a list to the sample's single-tree API: a broader exclusion mixer requires deliberate per-application routing/capture. Watching inside the native app gives us a clear playback process to omit; watching in an external browser complicates excluding playback without excluding other browser content the user intended to share. Selected-game audio is the simplest reliable default.

Camera capture can be a separate track. Independent camera and screen tracks preserve separate layouts/volumes and let people stop a camera without restarting their screen. Compositing a camera into the game picture is also possible, but removes that control from viewers.

## Codec policy and current support

HEVC is viable for this controlled Windows group, but cannot be promised for every browser. Chrome introduced HEVC WebRTC support in version 136, with platform/hardware conditions. A browser decoding an HEVC file does not prove its WebRTC path supports HEVC. Check actual receive capabilities and run playback tests on each friend's device. [Chrome 136 announcement](https://developer.chrome.com/blog/chrome-136-beta), [Chromium media support](https://www.chromium.org/audio-video/), [MediaMTX browser caveats](https://mediamtx.org/docs/features/webrtc-specific-features).

Suggested product policy:

- Prefer HEVC when the publisher has a working hardware encoder and every selected subscriber can decode it.
- Have a deliberate H.264 compatibility strategy. For an initial version, restarting that publisher's stream as H.264 for everyone is simpler than dual encoding. A second H.264 encode can preserve HEVC for compatible viewers later, at extra sender GPU/upload cost.
- Consider AV1 when the actual machines support hardware encoding and the viewers can decode it. Do not assume the supplied NVIDIA age range supports AV1 encoding.
- Avoid routine server-side transcoding. An SFU cannot make an incompatible codec playable merely by forwarding it.

Use a conservative SDR/8-bit/4:2:0 profile initially, then validate HDR and other profiles separately. Favor a low-latency encoder configuration, sensible keyframes, bounded queues, and congestion feedback. Codec choice alone does not prevent lag or preserve high-motion detail at arbitrarily low bitrate. Proposed test points, not quality guarantees: compare 1080p60 at 3, 6, and 10 Mbps on the actual games and GPU vendors, with a 720p option for constrained connections. Measure latency and game performance as well as visual quality.

Probe encoder and decoder capabilities on startup rather than assuming a GPU's brand or approximate age guarantees support. There are relatively recent cards without a hardware video encoder, such as AMD's RX 6500 XT. Check exact codec/profile support and available concurrent sessions when adding extra renditions. [AMD product specifications](https://www.amd.com/en/products/graphics/desktops/radeon/6000-series/amd-radeon-rx-6500-xt.html), [NVIDIA support matrix](https://developer.nvidia.com/video-encode-decode-support-matrix).

OBS added HEVC WebRTC output in 30.2; current versions also offer WHIP simulcast, which MediaMTX documents for OBS 32.1 and later. That makes established tools useful for experiments involving multiple resolutions. Confirm what the chosen codec, encoder, and server combination really forwards rather than assuming all features compose automatically. [OBS 30.2 notes](https://obsproject.com/blog/obs-studio-30-2-release-notes), [MediaMTX OBS multitrack documentation](https://mediamtx.org/docs/publish/obs-studio).

## LiveKit: avoid an outdated codec conclusion

LiveKit's general codec overview currently lists H.264, VP8, VP9, and AV1 without HEVC. However, released JS SDK v2.23.0 explicitly includes `h265`, and the server changelog records enabling H.265 by default. It would be incorrect to rule out LiveKit based only on that overview. [Overview](https://docs.livekit.io/transport/media/advanced/), [released JS codec types](https://github.com/livekit/client-sdk-js/blob/v2.23.0/src/room/track/options.ts), [server changelog](https://github.com/livekit/livekit/blob/v1.13.11/CHANGELOG.md).

LiveKit documents publishing pre-encoded video through Rust, C++, and Go APIs, including H.265 access units. That offers a possible bridge from our own NVENC/AMF pipeline without another encode. The documented Rust/C++ path has one spatial layer; the SDK cannot synthesize additional renditions from a single encoded frame. Native AMD HEVC on Windows still needs deliberate integration and testing: the automatic hardware-encoder table documents AMD VAAPI/H.264, not a turnkey Windows AMF/HEVC path. [Pre-encoded publishing](https://docs.livekit.io/transport/media/pre-encoded/), [hardware encoder table](https://docs.livekit.io/robotics/media/video/encoders/).

Any external-encoder integration must also handle pacing, timestamps, late-join keyframes, and bitrate adaptation. Merely submitting encoded frames is not proof of a robust streaming implementation. Validate these behaviors in the selected API/version.

Public GitHub API checks on the research date returned LiveKit server v1.13.11 (2026-10-25), JS SDK v2.23.0 (2026-10-16), and MediaMTX v1.21.3 (2026-10-25). These identify the versions considered, not a promise that they are bug-free. [LiveKit release](https://github.com/livekit/livekit/releases/tag/v1.13.11), [JS release](https://github.com/livekit/client-sdk-js/releases/tag/v2.23.0), [MediaMTX release](https://github.com/bluenviron/mediamtx/releases/tag/v1.21.3).

## Room privacy

The requested pinned link is a reasonable **bearer invitation** for a small trusted group. Proposed format: `https://stream.example/r/friends#invite=<random-secret>`. Use a cryptographically random token rather than a memorable password. A fragment is not automatically included in the initial HTTP request; the client can exchange it over HTTPS for a scoped session and remove it from the address bar. This reduces routine URL logging; it does not make a copied link safe to publish. [URI fragments](https://developer.mozilla.org/en-US/docs/Web/URI/Reference/Fragment).

Design requirements:

- Protect room discovery, presence, publication, playback, and relevant media signaling endpoints. Hiding the room page alone is insufficient.
- Exchange invitations for bounded credentials, with server-enforced room and publish/subscribe permissions. Do not hand clients a server API secret.
- Keep publisher and viewer privileges distinct where desired; a viewing URL should not necessarily allow replacing someone else's stream.
- Allow invite rotation/revocation. Define whether rotation also disconnects existing sessions; token expiry alone does not necessarily disconnect an already-established WebRTC session.
- Anyone who receives a valid shared invitation can enter. For excluding one person without rotating everyone's invite, add individual device/membership approval or separate invitations.
- Authenticate TURN and avoid permanent public relay credentials. Do not log invitation tokens. Default to no recordings.
- Explain the trust model: transport encryption protects network transit, while an ordinary SFU/media server can access media. Hiding media from the VPS operator requires a separately verified end-to-end encryption design across the native sender and browser; it should not be implied by HTTPS or WebRTC alone.

MediaMTX documents internal, HTTP, and JWT authentication with distinct read/publish permissions, providing building blocks for room-scoped authorization. [Authentication](https://mediamtx.org/docs/features/authentication). LiveKit likewise supplies room-scoped access-token grants and an E2EE facility, but the exact HEVC/native-publisher/browser combination must be validated before claiming E2EE support. [LiveKit tokens](https://docs.livekit.io/frontends/authentication/), [LiveKit encryption](https://docs.livekit.io/transport/encryption/).

## Capacity and experience with four or five streams

**Update:** monthly transfer is unlimited, so quota calculations below are historical illustrations rather than a product concern. The 1 Gbps instantaneous limit still matters. At a 100 Mbps video target, five simultaneous full-quality subscriptions require 500 Mbps outbound before overhead; twenty subscriptions require 2 Gbps and exceed that connection. For example, five people each publishing and watching all four other publishers create twenty subscriptions. Ingress and egress are separate on the stated full-duplex connection; do not add both and compare the total to a single 1 Gbps direction. A 4-vCPU/8-GB shared VPS is a plausible initial forwarding host, not a measured capacity guarantee. Avoid server-side video transcoding and thumbnail generation from decoded main streams.

The relevant load is active **stream subscriptions**, not just people in the room. Forwarded video egress is approximately `sum(stream bitrate × viewers of that stream)`. Before overhead, one Mbps sustained for one hour is 0.45 decimal GB.

Illustrative model: five 6 Mbps streams, each watched by five people, yields 30 Mbps aggregate publisher ingress and 150 Mbps server egress: 67.5 GB per hour, or 4.05 TB for 60 hours of this exact peak load. This is not an estimate of the group's actual monthly usage. Exclude self-view where appropriate. Audio, cameras, retransmissions, encryption/transport overhead, and extra renditions increase traffic. A viewer watching all five such feeds downloads 30 Mbps video and decodes five video streams.

The earlier [bandwidth model](bandwidth-model.html) plots the original low-bitrate illustration; the updated [live-view capacity model](live-view-capacity.html) addresses the 1 Gbps port at the requested 100 Mbps upper setting. The product must support click-to-watch, per-stream audio/mute and actual media unsubscription, rather than merely hiding full-quality videos. Generate compressed preview images at the publisher (proposed size: 320×180, once on stream start then at most once every 60 seconds, as Felipe requested), then distribute them only to authorized room members. Clicking Watch subscribes to the main video/audio; Stop watching unsubscribes from both while leaving the preview available. This avoids a second continuous video encoder or server transcoding just to produce thumbnails. A forwarding server cannot independently reduce one viewer's main-stream bitrate without a suitable alternative layer/rendition or transcoding. Multiple encoder renditions cost sender resources. [LiveKit selective subscriptions](https://docs.livekit.io/guides/room/receive).

Canada hosting adds a geographical detour when both endpoints are in Brazil. Its practical impact depends on the actual ISP routes. Measure end-to-end latency, packet loss, and jitter through this VPS before deciding whether a Brazil/nearby location is worth changing. Do not infer real latency from the country name alone.

## Resolution, frame rate and bitrate controls

Proposed presets: 3, 5, 8, 12, 20, 35, 50, 75 and 100 Mbps. These are selectable target rates, not guarantees of picture quality or delivered throughput. Resolution options respect the selected source's native physical dimensions/aspect ratio, capped at 4K; do not offer fake upscaling as extra detail. Offer up to 120 fps where the capture source, encoder and receiver can sustain it. Actual game/source frame production may be lower than the selected capture ceiling.

Define two clear bitrate states:

- **Auto:** changing resolution or FPS recalculates and applies a recommended target. Recommendations should also account for codec and a motion/detail profile, then be tuned with actual game tests rather than marketed as universal quality guarantees.
- **Manual:** changing only the bitrate preserves resolution and FPS. Later resolution/FPS changes update the displayed recommendation but preserve the manual target until the user selects Auto again.

The encoder/network controller still needs congestion handling in either state. A manual target/ceiling cannot force a sender uplink or subscriber connection to sustain that rate. Keep user-selected resolution/FPS distinct from measured delivery; expose degraded conditions and make any automatic resolution/FPS adaptation an explicit policy rather than silently rewriting the user's settings.

Treat 4K/120 as a mode to qualify end-to-end: source capture cadence, GPU encode throughput at the chosen HEVC preset, negotiated codec/profile/level, decoder throughput, browser rendering and display refresh. HEVC support by itself does not establish 4K120 operation, and older GPUs should not be promised the maximum mode. NVIDIA documents substantial performance variation by GPU and encoder configuration; its quoted benchmark resolution must not be mistaken for a 4K120 guarantee. [NVENC performance documentation](https://docs.nvidia.com/video-technologies/video-codec-sdk/13.1/nvenc-application-note/index.html).

## Decision sequence before implementing a product

1. Try the most promising existing room product if its relay and capture paths fit; see the project comparison.
2. Validate one NVIDIA and one AMD Windows publisher using native game capture, application audio, HEVC, and browser playback through the VPS. Include a camera test and H.264 fallback.
3. Test restrictive-network fallback deliberately, not merely a successful connection on one home network. Test reconnects, late joins, source switches, game fullscreen/alt-tab, and a publisher streaming while watching another feed.
4. Exercise two then four/five simultaneous streams and the expected viewer subscriptions. Record capture/encode/decode FPS, dropped frames, bitrate, loss, jitter, latency, game FPS impact, and VPS transfer. Test over a full evening, not only minutes.
5. Test access denial at both page and media endpoints; test invite rotation and the intended session behavior.
6. Choose the room/media backend and native integration only after those results. A proposed target is consistent sub-second interaction; exact latency and visual-quality acceptance should be judged by the friends using their games and networks.

The potentially valuable new work is the focused experience: open a private persistent room, select a game and its audio, start sharing, and watch friends at the same time. Existing projects already implement substantial parts of the media path; a search cannot establish that no satisfactory finished alternative exists.
