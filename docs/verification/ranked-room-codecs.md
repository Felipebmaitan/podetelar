# Ranked room codec adaptation

Implemented on 2026-11-13, based on the native settings correction branch
(`07ae082`, PR #41). This change remains a separate branch and PR.

## Behavior

The shared publication chooses AV1, then HEVC, then H.264 from the intersection
of every connected member's advertised decoders and the sender's available
hardware encoders at the requested mode. The Advanced codec selection is the
preferred ceiling; explicitly selecting HEVC or H.264 stays respected. A legacy
member without decoder capabilities conservatively constrains the room to H.264.
Preview-only members announce capabilities without subscribing to media.

The sender reevaluates membership/capability changes and can upgrade toward the
preferred codec after a restrictive member leaves. A watched advanced codec
without frames for six seconds is rejected for that publisher's current watch;
the next compatible codec is selected. Failures persist across replacement SIDs,
temporary missing tracks and watching-budget pauses. Resuming a budget-paused
watch resets its watchdog so the intentional pause does not reject a decoder.
Stop watching clears that watch's failure state and allows a subsequent retry.

Codec replacement stops the old publication before starting its replacement.
Watch intent survives, resolution/FPS stay unchanged, and the shared preview gate
retains its original capture clock. The cached still can be uploaded against a
replacement SID without capturing another image. A missing common codec stops
publication with an explicit error rather than silently rewriting the mode.

## Automated verification

- Node **24.11.1**, `npm run check`: formatting, workspace/tool types, API/config
  tests and production builds passed after the implementation and review fixes.
- `npm run native:check`: Rust formatting and Clippy passed. The pinned local
  LiveKit SDK retains its existing dependency warnings.
- `npm run native:test`: publisher **79 passed, 5 ignored**; portable **10 passed**
  and its isolated process fixture **1 passed**. New policy coverage includes
  ordered intersections, absent encoders, selected codec ceilings, disjoint
  decoder sets, failed-codec progression, capability serialization and watchdog
  reset on budget resumption.
- The real isolated Windows API/SFU adaptation scenario passed with fresh frame
  growth in each AV1 → HEVC → H.264 → HEVC → AV1 phase. A HEVC-capable member and
  an H.264-only member joined and left without subscribing. Both received zero
  media bytes; the observing member kept watch intent. The final mode remained
  1920×1080 at 60 FPS and the shared gate counted exactly one preview capture
  across all replacements. AV1 GPU and CPU decode, HEVC/H.264 decode and explicit
  Stop/rewatch also passed in the same run.
- The complete selected media regression passed **11/11** in **4.8 minutes**:
  `npm run native:room-smoke -- native-e2e/codec-adaptation.spec.ts
native-e2e/av1.spec.ts native-e2e/native-room.spec.ts`, with
  `FREEDOMSTREAM_SMOKE_APP_PORT=22110` and
  `FREEDOMSTREAM_SMOKE_SFU_PORT=22880`. This also covered stereo game audio with
  Discord rejection, full reconnect, preview-only reconnect without recapture
  and the watcher's aggregate allowance constraining the shared encoder.
- Independent **GPT-6.1-Sol, high** source review approved after fixing persistent
  failure advertisements during track gaps, budget-resume watchdog timing and
  strengthening the media test's fresh-frame/capture assertions. Approval is
  contingent on the real SFU acceptance checks passing; all eleven subsequently
  passed on the reviewed implementation.

## Qualification limits

The local native SFU checks use synthetic GPU content and restricted decoder
advertisements on this NVIDIA machine. Restricted capabilities exercise real
codec negotiation and transport, not a second physical GPU or a broken driver.
AMD hardware, real multi-friend/game load, physical A/V timing, production rollout
and 4K120 remain separate acceptance gates. No new runtime dependency is bundled.
