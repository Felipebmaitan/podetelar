# AV1 hardware support and multi-stream decoding cost

Research date: 2026-11-09. Scope: NVIDIA Ampere/Ada/Blackwell and AMD desktop RDNA 2/3/4, CPU fallback, and pixel-rate arithmetic. This is research, not a change to FreedomStream's codec policy or implementation phases. No hardware or CPU benchmark was run for this note.

Evidence labels used below:

- **Documented capability:** a vendor lists a codec/profile as supported; this does not establish real-time performance in FreedomStream.
- **Vendor measurement:** a published test with its own configuration and limits.
- **Calculation:** reproducible arithmetic, not a benchmark.
- **Inference/proposal:** an engineering implication requiring application and hardware verification.

## GPU support

The table concerns fixed-function video acceleration. Encode and decode are separate capabilities: an AV1 publisher does not require every viewer to have an AV1 encoder. Conversely, successful AV1 encoding says nothing about a viewer's decoder.

| GPU family / relevant products | Hardware AV1 decode | Hardware AV1 encode | Primary evidence |
| --- | --- | --- | --- |
| NVIDIA Ampere GA10x, including GeForce RTX 30 series | Yes | No | [NVIDIA support matrix](https://developer.nvidia.com/video-encode-decode-support-matrix), [NVDEC note](https://docs.nvidia.com/video-technologies/video-codec-sdk/13.0/nvdec-application-note/index.html), [NVENC note](https://docs.nvidia.com/video-technologies/video-codec-sdk/13.0/nvenc-application-note/index.html) |
| NVIDIA Ampere GA100 / A100 | No | No | Architecture name alone is insufficient; see [NVDEC GA100 exception](https://docs.nvidia.com/video-technologies/video-codec-sdk/13.0/nvdec-application-note/index.html) and [exact product matrix](https://developer.nvidia.com/video-encode-decode-support-matrix) |
| NVIDIA Ada, including GeForce RTX 40 series | Yes | Yes | [NVIDIA support matrix](https://developer.nvidia.com/video-encode-decode-support-matrix) |
| NVIDIA Blackwell, including GeForce RTX 50 series | Yes | Yes | [NVIDIA support matrix](https://developer.nvidia.com/video-encode-decode-support-matrix) |
| AMD RDNA 2 desktop RX 6000 with a full media block, e.g. RX 6800 | Yes | No | [RX 6800 specifications](https://www.amd.com/en/products/graphics/desktops/radeon/6000-series/amd-radeon-rx-6800.html), [AMD encoding compatibility](https://www.amd.com/en/products/software/adrenalin/video-streaming.html) |
| AMD RDNA 2 RX 6400 and RX 6500 XT exceptions | **No** | **No** | [RX 6400 specifications](https://www.amd.com/en/products/graphics/desktops/radeon/6000-series/amd-radeon-rx-6400.html), [RX 6500 XT specifications](https://www.amd.com/en/products/graphics/desktops/radeon/6000-series/amd-radeon-rx-6500-xt.html), [AMD encoding compatibility](https://www.amd.com/en/products/software/adrenalin/video-streaming.html) |
| AMD RDNA 3 desktop RX 7000, e.g. RX 7600 | Yes | Yes | [RX 7600 specifications](https://www.amd.com/en/products/graphics/desktops/radeon/7000-series/amd-radeon-rx-7600.html), [AMD encoding compatibility](https://www.amd.com/en/products/software/adrenalin/video-streaming.html) |
| AMD RDNA 4 desktop RX 9000, e.g. RX 9070 XT | Yes | Yes | [RX 9070 XT specifications](https://www.amd.com/en/products/graphics/desktops/radeon/9000-series/amd-radeon-rx-9070XT.html), [AMD encoding compatibility](https://www.amd.com/en/products/software/adrenalin/video-streaming.html) |

**Documented details:** NVIDIA lists AV1 Main-profile 4:2:0 8/10-bit encoding on Ada and Blackwell, with dimensions up to 8192×8192. That dimension limit is not a frame-rate promise. [NVENC AV1 capabilities](https://docs.nvidia.com/video-technologies/video-codec-sdk/13.0/nvenc-application-note/index.html#nvenc-av1-hardware-capabilities).

AMD's AMF table lists AV1 8/10-bit decode and no AV1 encode for VCN 3.0/3.1; VCN 4.0/5.0 list AV1 8/10/12-bit decode and 8/10-bit encode, all 4:2:0 and up to 8K. It maps RX 7600-class Navi33 to one VCN instance and larger Navi31/32 products to two, but explicitly limits AV1 encoding to one VCN instance except Strix Halo. Therefore, two VCN instances must not be interpreted as two AV1 encoders. The generic VCN row must also be qualified by the RX 6400/6500 XT product exceptions. [AMD AMF hardware table, pinned wiki revision](https://github.com/GPUOpen-LibrariesAndSDKs/AMF/wiki/GPU-and-APU-HW-Features-and-Support/1de2f2f99100d57c5e4795ebeabc19d8f1b4243c).

AMD explicitly marks H.264 and HEVC encoding unavailable on RX 6400/6500 XT too; these cards still list H.264/HEVC decoding. “No AV1 decode” does not mean “no video decoding.” [RX 6400](https://www.amd.com/en/products/graphics/desktops/radeon/6000-series/amd-radeon-rx-6400.html), [RX 6500 XT](https://www.amd.com/en/products/graphics/desktops/radeon/6000-series/amd-radeon-rx-6500-xt.html).

**Inference/proposal:** capability detection should use the exact adapter and runtime API/profile, followed by opening the requested configuration. Do not infer support from shader performance, the RDNA/Ampere name, an installed codec, or a laptop's discrete GPU alone. A separate integrated GPU might provide decoding, but cross-adapter transfers and actual adapter selection need verification. Application codec negotiation and the shipped receive path must support AV1 as well.

## Pixel-rate calculations

Assumptions: 1080p = 1920×1080; 1440p = 2560×1440; 4K = UHD 3840×2160, not DCI 4096×2160. Rates are exactly 60/120 FPS, not 59.94/119.88. Four streams means four simultaneous full-resolution subscriptions of the same format on **one receiver**.

```text
pixels_per_frame = width × height
pixels_per_second = width × height × FPS
four_streams = 4 × pixels_per_second
1080p60_equivalents = pixels_per_second / 124,416,000
```

| Format | Pixels/frame | One stream, pixels/s | One stream, MPixels/s | Four streams, pixels/s | Four streams, MPixels/s | Four streams, 1080p60 equivalents |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1080p60 | 2,073,600 | 124,416,000 | 124.416 | 497,664,000 | 497.664 | 4 |
| 1440p60 | 3,686,400 | 221,184,000 | 221.184 | 884,736,000 | 884.736 | 7.111… |
| 4K60 | 8,294,400 | 497,664,000 | 497.664 | 1,990,656,000 | 1,990.656 | 16 |
| 4K120 | 8,294,400 | 995,328,000 | 995.328 | 3,981,312,000 | 3,981.312 | 32 |

**Calculation:** one 1440p60 stream is 16/9 ≈ 1.778 times 1080p60; one 4K60 is 4 times; one 4K120 is 8 times. Four 1080p60 streams equal one 4K60 in pixel rate. Four 4K120 streams require 3.981312 Gpixels/s. One 8K60 stream at 7680×4320 is 1.990656 Gpixels/s: equal to four 4K60 streams or two 4K120 streams in pixels alone.

**Inference limits:** these ratios normalize geometry; they do not predict CPU cycles, decoder FPS, video bitrate, or latency. Multiple bitstreams add contexts, reference surfaces, scheduling and synchronization. Compression tools, bit depth, frame dependencies and content change decoding work. A small on-screen tile can still require full-resolution decoding. A nominal “8K60” capability does not prove four independent 4K60 subscriptions or 4K120 end-to-end operation. AMD's RDNA 3 launch statement specifically attaches simultaneous streams up to 8K60 to **HEVC** and separately mentions AV1 encode; do not silently transfer that wording to AV1 throughput. [AMD RDNA 3 announcement](https://ir.amd.com/news-events/press-releases/detail/1099/amd-unveils-worlds-most-advanced-gaming-graphics-cards-built-on-groundbreaking-amd-rdna-3-architecture-with-chiplet-design).

## What vendor performance numbers establish

**Vendor measurement:** NVIDIA SDK 13.0 publishes indicative AV1 decode rates of 790 FPS for Ampere, 1018 for Ada and 1119 for Blackwell, **per NVDEC**, at 1920×1080 YUV 4:2:0, Windows 11, maximum video clocks (1665/2160/2362 MHz respectively). Multiple contexts share finite hardware capacity; additional NVDECs can raise aggregate throughput, not one session's ceiling. [NVDEC performance and conditions](https://docs.nvidia.com/video-technologies/video-codec-sdk/13.0/nvdec-application-note/index.html#nvdec-performance).

**Calculation/inference, not measured 4K FPS:** dividing those rates by four gives 197.5/254.5/279.75 “4K-equivalent FPS” per engine. Four 4K60 streams would demand 240 such FPS and four 4K120 streams 480. This is a rough comparison only: it cannot certify Ada's four-stream 4K60 playback or reject/approve a particular GPU. Exact engine count, clocks, bitstream, memory traffic, rendering and game contention still matter.

Engine counts vary by SKU: NVIDIA currently lists one NVDEC on RTX 4090 versus two on RTX 5090, and two NVENCs versus three respectively. Encoder count is not decoder count. These are product-specific examples, not universal Ada/Blackwell counts. [NVIDIA SKU matrix](https://developer.nvidia.com/video-encode-decode-support-matrix).

**Vendor measurement:** NVIDIA's AV1 encode table changes substantially with preset: Ada P1 CBR low-latency is 1090 FPS versus P7 at 356 FPS, per encoder at 1080p 8-bit 4:2:0. These encode figures do not measure playback. [NVENC performance and test conditions](https://docs.nvidia.com/video-technologies/video-codec-sdk/13.0/nvenc-application-note/index.html#nvenc-performance).

## CPU software decoding caveats

**Documented implementation:** dav1d is a production AV1 software decoder with optimized C/assembly paths, including AVX2 and SSSE3. CPU decoding is a viable implementation option; absence of GPU AV1 does not make decoding impossible. It does not follow that every CPU can sustain any of the workloads above. [VideoLAN dav1d README](https://github.com/videolan/dav1d/blob/master/README.md).

The decoder version, build and available instruction set matter. VideoLAN's release notes document continuing architecture-specific work, including AVX-512, ARM and filter optimizations. Old decoder results cannot serve as a current universal CPU minimum. [VideoLAN release notes](https://github.com/videolan/dav1d/blob/master/NEWS).

dav1d exposes thread count, frame delay and film-grain controls. Its API defaults thread count to the host's logical-core count, documents a frame-delay setting of one for low latency, and enables grain application by default. [dav1d API settings](https://github.com/videolan/dav1d/blob/master/include/dav1d/dav1d.h).

**Engineering inferences, not CPU benchmarks:**

- CPU model/core count alone cannot establish sustained FPS. SIMD support, clocks, power/thermal limits, bit depth, encoded tools, grain and parallelism must be recorded. Benchmark the actual SDR 8-bit stream as well as any proposed 10-bit path.
- Low-latency decoding and an offline decoder allowed to buffer frames are different workloads. More threads or buffered frames may improve throughput while increasing contention or delay; test the application's settings.
- Four decoder contexts each allowed all logical cores can oversubscribe a gaming PC. A single-stream test cannot be multiplied into a four-stream guarantee. Record process CPU, game frame times and decoder queues together.
- Lower network bitrate does not reduce width × height × FPS. Content and coding choices can alter CPU cost, so bitrate is not a reliable decoder-load control by itself.
- CPU output also needs conversion/upload/rendering and memory for reference/output frames. “Decoder-only 60 FPS” is insufficient evidence of smooth synchronized playback.
- 1080p60 may be practical on a particular CPU, but this note establishes no CPU minimum. Four 4K60 or 4K120 streams are especially demanding candidates for physical qualification; the pixel table establishes scale, not infeasibility or success.

No CPU benchmark FPS or percent utilization is claimed here. No four-stream result, vendor comparison on identical workloads, or FreedomStream hardware qualification was measured.

## Proposed FreedomStream qualification

These are proposed checks, not completed acceptance results or changes to the implementation plan:

1. Probe encode and decode separately on exact adapters/drivers. Confirm the selected AV1 profile and hardware path with runtime evidence; successful playback can otherwise conceal software fallback.
2. Exercise one and four real subscriptions for each table row, including simultaneous local publishing and representative gaming. Record requested versus delivered resolution/FPS, decode time distribution, dropped frames, queue growth, A/V synchronization, CPU, video-engine utilization and game frame times over a sustained session.
3. Repeat with software decoding forced, recording CPU model, decoder build/version, thread/frame-delay settings and bitstream parameters. Treat a no-render decode test as a separate throughput benchmark.
4. Verify mixed-capability watchers and explicit fallback negotiation. Hardware AV1 support alone must not silently change requested resolution/FPS or establish codec interoperability.
5. Verify Stop watching ends the actual subscription and incoming media traffic. Hiding a tile is not resource control. Keep previews as still images on the existing start/at-most-once-per-minute schedule.

**Research conclusion:** AV1 is a candidate for publishers on Ada/Blackwell or RDNA 3/4, with hardware viewing also possible on GA10x Ampere and most desktop RDNA 2 products. RX 6400/6500 XT require another decoder path for AV1. Select codec and subscription budgets using verified receiver behavior; neither family support tables nor pixel-rate estimates establish four-stream or 4K120 readiness.
