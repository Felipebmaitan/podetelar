# Bitrate guidance for native rooms

Research date: 2026-11-09. Scope: SDR, low-latency native HEVC/H.264 screen/game publication and simultaneous subscriptions through LiveKit. This is a research proposal, not measured FreedomStream quality or hardware qualification. Published recommendations and our inferences are identified separately.

## What the primary sources actually recommend

### NVIDIA: useful lower-bandwidth reference, with FPS context

NVIDIA's January 2025 [NVENC OBS broadcasting guide](https://www.nvidia.com/en-us/geforce/guides/broadcasting-guide/) favors 60 FPS in its surrounding discussion, but its bitrate/resolution table has no per-row FPS column. It associates 6 Mbps with HEVC 1080p and 8 Mbps with H.264 1080p. These are useful approximately-60-FPS broadcast starting references, not separately validated 1080p30/60 guarantees. It recommends using approximately 75% of upload capacity and reducing resolution for high-motion games; its example is Fortnite at 720p60/6 Mbps. The table also associates HEVC 4K with 15 Mbps and H.264 4K with 20 Mbps, without promising equivalent quality across content or latency settings.

**Our inference:** 1080p does not inherently require 20–30 Mbps. Allow modest rates, and test motion quality before increasing them. NVIDIA's OBS settings include look-ahead/B-frames and quality-oriented presets; copying their bitrate does not establish equivalent quality in a different low-latency pipeline or on AMD hardware.

### YouTube: higher ingest targets and downstream transcoding

YouTube's current [encoder guidance](https://support.google.com/youtube/answer/2853702) distinguishes minimum from recommended ingestion rates. The following are its **recommended** values, in Mbps; H.265 shares a column with AV1:

| Input   | H.265/AV1 | H.264 |
| ------- | --------: | ----: |
| 720p30  |         6 |     8 |
| 720p60  |         6 |     8 |
| 1080p30 |        10 |    14 |
| 1080p60 |        12 |    17 |
| 1440p30 |        15 |    21 |
| 1440p60 |        24 |    34 |
| 2160p30 |        30 |    42 |
| 2160p60 |        35 |    50 |

These are broadcast **ingest** targets: YouTube automatically transcodes to multiple output formats for different viewers. Its page specifies up to 60 FPS, and says 4K uses normal latency rather than its low-latency option.

**Our inference:** these are useful quality-oriented upper reference points, not minimum downlink requirements for every YouTube viewer and not directly transferable to our single-layer, non-transcoding SFU path. Older circulating bitrate tables should not override the currently retrieved guidance.

### Twitch: connection-friendly broadcast reference, access caveat

Twitch's [official broadcasting guidelines](https://help.twitch.tv/s/article/broadcasting-guidelines?language=zh_CN) were available as search-indexed first-party text: 720p30 at 3 Mbps, 720p60 at 4.5 Mbps, 1080p30 at 4.5 Mbps, and 1080p60 at 6 Mbps in its NVENC/x264 broadcast tables, with CBR and a two-second keyframe interval. The English portal and direct locale fetch returned a loading/CSS-error shell; the legacy [encoding URL](https://stream.twitch.tv/encoding/) was inaccessible. Treat these indexed figures as a corroborating, potentially stale broadcast reference, not verified current service limits or HEVC guidance.

**Our inference:** stable 3–6 Mbps HD broadcasting is a credible constrained-network tradeoff. It does not promise clean fast-motion 1080p60 or justify imposing Twitch's broadcast constraints on FreedomStream.

## Why there is no universal optimal bitrate

**Engineering inference from the references above:** bitrate trades compression artifacts against network load, latency and hardware work. Resolution, FPS, motion, foliage/particles, text size, encoder generation/preset, GOP, rate control and latency restrictions all matter. A mostly static desktop can use fewer actual bits than a fast game at the same dimensions. More bits eventually give diminishing returns and can harm delivery when queues grow. A bits-per-pixel-per-frame formula is a rough interpolation tool, not a codec quality law; neither doubling pixels nor FPS guarantees that precisely twice the bitrate is optimal.

Choose a content-appropriate starting target, then measure visual quality and delivery on actual encoders/decoders. Do not claim that HEVC always saves a fixed percentage over H.264, or that NVENC results qualify AMF. A configured target/cap is distinct from encoded output, RTP delivery and decoded/rendered output.

## Proposed quality-oriented starting targets

**All values below are FreedomStream inferences, not a source's recommended table.** They balance the lower broadcast references against YouTube's higher ingest targets. Units are decimal Mbps of video payload, excluding audio, transport overhead and retransmissions. Assume 16:9 SDR; preserve source aspect ratio and never upscale. These are starting points for testing typical desktop/game content, not minimums or guarantees for high-motion games or fine text.

| Output            | HEVC start | H.264 start |
| ----------------- | ---------: | ----------: |
| 1280×720, 30 FPS  |          3 |           4 |
| 1280×720, 60 FPS  |          4 |           6 |
| 1920×1080, 30 FPS |          5 |           7 |
| 1920×1080, 60 FPS |          8 |          10 |
| 2560×1440, 30 FPS |         10 |          14 |
| 2560×1440, 60 FPS |         14 |          20 |
| 3840×2160, 30 FPS |         20 |          28 |
| 3840×2160, 60 FPS |         28 |          40 |

Proposed tuning: test around the starting target, initially in approximately 25% steps, respecting the network budget. Motion artifacts may require a higher target or an explicitly selected smaller resolution. Text/detail mode should prioritize readability; do not automatically halve bitrate just because the source is a screen. Static content may naturally undershoot its cap. For slow connections, explicitly choosing 720p30 is preferable to retaining a high-resolution/high-FPS request that cannot be delivered acceptably.

**Above 60 FPS is extrapolated and unqualified.** If an experimental estimate is needed, `B(f) = B(60) × f/60` is a transparent initial extrapolation for 90/120 FPS, not a sourced recommendation or verified quality model. It must remain within configured resource limits and cannot certify real-time encode/decode/render throughput. Do not silently clamp this estimate and label the result equivalent quality. No source above establishes FreedomStream 4K120.

## SFU feedback and independent watcher quality

**Direct LiveKit documentation:** [SFU architecture](https://docs.livekit.io/reference/internals/livekit-sfu/) describes forwarding compressed tracks without decoding/re-encoding and estimating downstream bandwidth separately for each subscriber. [Simulcast documentation](https://docs.livekit.io/transport/media/advanced/#video-simulcast) describes multiple publisher encodings and recipient-specific layer selection; Dynacast pauses unused encodings. [Subscription controls](https://docs.livekit.io/transport/media/subscribe/#simulcast-controls) condition maximum received quality controls on simulcast. The [protocol definition](https://github.com/livekit/protocol/blob/main/protobufs/livekit_models.proto) represents a single-layer track as HIGH; that label is not evidence of additional lower renditions.

LiveKit also documents [congestion-triggered video pausing](https://kb.livekit.io/articles/8062994967-managing-video-quality-during-network-congestion): when enabled, video can pause/resume while audio continues, with stream-state signaling. This is a documented capability, not confirmation of our pinned self-hosted configuration or native UI handling. Audio still needs a viable connection.

**Architectural inference:** publisher → SFU and SFU → watcher A/B/C are different network paths. Sender congestion feedback can constrain the upload into the SFU; successful upload does not prove that any individual watcher can download it, let alone several streams. Each watcher also shares its downlink among all subscriptions and other traffic. Do not display the publisher's encoder feedback as a viewer bandwidth test.

With one ordinary non-scalable HEVC/H.264 encoding, the SFU has no smaller rendition to select. A request for LOW cannot synthesize one. Pausing or indiscriminately dropping packets does not create a clean independently decodable lower-bitrate stream. A global publisher bitrate reduction affects every watcher. Supporting independent per-viewer quality while retaining high quality for others requires additional publisher encodings (simulcast), qualified scalable coding/layers, or transcoding. SVC can use one layered encoding, but that is distinct from the current ordinary single-layer HEVC/H.264 path. Do not assume it is supported merely because LiveKit documents SVC for other codecs.

For this phase, keep single encoding and explicit watch/stop semantics. Offer fewer subscriptions or a coordinated publisher setting change when a watcher cannot fit. Treat simulcast as a later scope decision with extra upload and encoder cost. HEVC → H.264 compatibility fallback changes the shared publication; it must recompute the starting bitrate and budget check without silently changing resolution/FPS.

## Proposed upload and aggregate watcher budgets

These are **application policy proposals**, not implemented controls or promises of network capacity. NVIDIA's 75% upload rule motivates headroom; extending it to downlink and choosing caps below are our own conservative heuristics.

- Publisher: use sustained usable upload `U`, not an ISP headline rate. Start with total media payload at most `0.75 × U`; subtract audio and other media before assigning video. Upload to an SFU is one copy per encoding, not one copy per watcher.
- Watcher: define a configurable aggregate media cap `C` (proposed default 25 Mbps; constrained options 5 or 10 Mbps). When a credible sustained downlink estimate `D` exists, use `M = min(C, 0.75 × D)`. Require `sum(video_i + audio_i) <= M` across actual subscriptions. Reserve additional capacity inside this allowance for preview/control activity as measurements warrant.
- The unused 25% is combined margin for overhead, bursts/recovery and competing traffic; it is not a measured overhead constant. If those exceed the margin, lower the budget further. For a payload-based congestion estimate, apply a safety margin to that estimate rather than pretending it measures wire capacity. Avoid double-counting the same aggregate estimate per track.
- If capacity is unknown, `C` is an administrative/user allowance only. Start with one subscription, show capacity as unknown, and use receiver evidence before admitting more. Do not infer spare bandwidth from a quiet/static stream's low current rate. Reserve against configured/credible sustained demand so movement does not suddenly overwhelm a watcher.
- Give focused subscriptions priority. If a new Watch would exceed the budget, explain the estimate and offer Stop another stream or a lower shared publisher setting. Shrinking/hiding a tile does not stop traffic; require an actual unsubscribe of video and audio. Preserve previews as still images, captured initially and at most once per minute.

Illustrative calculations using 0.192 Mbps audio per subscription, matching the current publisher cap (actual traffic and redundancy can differ):

- A sustained 5 Mbps upload gives 3.75 Mbps media allowance: 720p30 HEVC at 3 plus audio fits; 4 Mbps H.264 plus audio does not. Ask for an explicit lower mode/target; HEVC availability is not guaranteed.
- A 10 Mbps downlink gives 7.5 Mbps allowance: two 3 Mbps HEVC subscriptions plus audio total 6.384 Mbps. One 8 Mbps 1080p60 HEVC subscription already exceeds it. A 5 Mbps downlink gives only 3.75 Mbps allowance, supporting roughly one such 720p30 stream.
- Four 8 Mbps HEVC subscriptions plus audio total 32.768 Mbps. With the proposed 25 Mbps cap they are refused regardless of a fast connection; a raised cap and at least roughly 43.7 Mbps sustained downlink would satisfy this heuristic. Decoder/render capacity still needs testing.
- A nominal 3 Mbps connection leaves only 2.25 Mbps allowance. The current UI's 3 Mbps minimum cannot fit with audio. A future lower-resolution/FPS mode or sub-3-Mbps manual target could address this; current congestion reductions below the UI minimum do not establish acceptable quality there.

**Server inference:** budget outbound copies separately: `sum over subscriptions(video + audio)`, plus overhead, recovery, previews and other hosted workloads. Five publishers watched by four others create 20 subscriptions: at 10 Mbps each, video alone is 200 Mbps; at 40 Mbps, 800 Mbps. A 1 Gbps port needs shared headroom across rooms and other services. A per-publication ceiling is not a deployment-wide limiter.

## Receiver evidence and implementation caveats

**Direct specification:** W3C [RTCStats](https://www.w3.org/TR/webrtc-stats/#dom-rtcicecandidatepairstats-availableincomingbitrate) defines `availableIncomingBitrate` as an aggregate estimate for incoming RTP on a candidate pair, in bits/s, excluding transport headers. It exists only when receive-side estimation is calculated; TWCC does not guarantee this field. `availableOutgoingBitrate` describes the outgoing direction. [Received RTP stats](https://www.w3.org/TR/webrtc-stats/#dom-rtcreceivedrtpstreamstats) define per-SSRC packet loss (cumulative, potentially negative) and jitter in seconds. Inbound stats also expose decoded/dropped frames, freezes and jitter-buffer counters.

**Proposed implementation:** keep absent estimates unknown; verify fields exposed by our pinned native SDK. Use selected active transports, aggregate only distinct paths, and avoid counting the same candidate pair for each track. Measure recent bitrate from byte/time deltas with consistent payload/wire accounting. Handle counter resets, reordering and retransmissions before calculating interval loss. Correlate loss, jitter, RTT trends, decode/render drops, freezes and playback delay; no single threshold proves congestion or spare capacity. Use smoothing/hysteresis, prompt reductions and slower recovery; validate timings on real networks. Never confuse decoder overload with a download bottleneck.

Branch inspection for this note found `simulcast: false` in [publisher.rs](../apps/publisher/src/publisher.rs), sampled encoder targets in [encoding.rs](../apps/publisher/src/encoding.rs), and formula-based Auto bitrate capped at 35 Mbps in [quality.rs](../apps/publisher/src/quality.rs). The proposed table is deliberately different; its 4K60 H.264 start exceeds that Auto ceiling. Adopting it requires a separate reviewed code/policy change, including how to report budget-limited quality. This note changes no runtime behavior.

Before promoting targets to defaults, measure representative static text, scrolling, fast games and scene changes on NVIDIA and AMD, with simultaneous publishing and multiple native decoders. Exercise constrained upload independently from each watcher's constrained aggregate download, packet loss/jitter, codec fallback and subscription stop. Record requested, encoded, sent, received and rendered rates plus latency and hardware load. Compilation, mocks and source recommendations cannot replace those acceptance checks.

## Implementation decision after research

The research proposals above are retained as proposals. This branch chose lower
starting targets: 1080p60 motion at 6 Mbps HEVC / 8 Mbps H.264, and 4K60 at 18 / 25.
See the complete [implemented table and receiver policy](quality.md). Manual
presets now start at 1 Mbps. The aggregate watching allowance defaults to 12 Mbps,
with 25% headroom and 256 kbps audio reserve per stream. Receiver interval loss
and jitter-buffer feedback reduce that cap; optional incoming-bitrate estimates
are not qualified and are not used as downlink capacity measurements. This is a
conservative allowance, not a speed test. The lowest active watcher video cap
constrains the shared encoding. Separate per-viewer quality is still unavailable.
Hardware/real-network quality and synchronization remain acceptance work.
