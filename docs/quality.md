# Video quality and resource controls

The Windows app keeps source resolution (up to 4K) and the FPS slider visible.
The default is Source at 60 FPS, with high-quality motion/detail presets and
Automatic bitrate. Codec/hardware encoder and manual bitrate live in Advanced.
The website handles invitations, access management and app downloads; it does not
publish or play streams.

Windows camera sharing has its own settings and defaults to 720p, 30 FPS and
2 Mbps. Its choices are 720p, 1080p or Source, 24/30/60 FPS, and 0.5–8 Mbps.
The camera needs to expose a mode that can supply the selected dimensions and
frame rate. Output preserves aspect ratio, downscales where needed, and can drop
frames from faster camera modes. Camera settings apply on the next start and use
the room's hardware codec negotiation and bitrate feedback.

## Choosing a mode

Resolution choices are 720p, 1080p, 1440p, 4K and Source. Each preserves source aspect ratio, rounds down to even dimensions, and never scales above its captured source. The 4K ceiling is 3840×2160, or 2160×3840 in portrait orientation. Ultrawide sources fit inside that envelope. Source means the bounded dimensions evaluated at capture start. A source resize does not silently change the active output; shrinking below its selected dimensions stops sharing with an explanation. The negotiated capture dimensions are checked before publication and during capture. Window enumeration uses visible DWM bounds instead of invisible resize margins, matching WGC capture.

The visible FPS slider steps through 15, 24, 30, 60, 90 and 120 fps. Resolution options above the source are disabled. The selected hardware factory's NV12/D3D11 sink caps must accept the dimensions/FPS before capture starts.

Format negotiation is not a throughput certificate: a mode may be negotiable but
exceed a particular GPU's real-time capacity. There is no silent CPU encoder.
Rooms select AV1 → HEVC → H.264 within the sender's preference and connected
members' hardware decoder capabilities, including members who are not watching.
Software availability does not authorize reception. Native admission sends the
explicit hardware set to the server for the initial participant attributes.
Missing/legacy capability reports remain unknown: they block a new stream with an
update/retry explanation and never trigger a live codec downgrade. An explicitly
empty set is incompatible. The current codec stays selected while everyone allows
it, including after a restrictive member leaves. Missing/dropped frames, pauses,
congestion and interruptions are playback-health conditions, not capability
changes. A codec change is allowed when explicit hardware capabilities exclude
the current codec, or when the streamer applies settings. It
replaces the single publication while retaining Watch intent and preview cadence.

| Bitrate mode | Behavior                                                                                                                |
| ------------ | ----------------------------------------------------------------------------------------------------------------------- |
| Auto         | Interpolates dimension/FPS anchors for AV1/HEVC/H.264 and the selected profile; rounds up to 0.5 Mbps within 1–35 Mbps. |
| Manual       | 6–100 Mbps presets or Custom (1–100 Mbps). Editing it preserves resolution and FPS.                                     |

Games/motion uses these starting video targets in Mbps; AV1 currently shares the
HEVC targets. They are application
heuristics informed by [primary-source guidance](bitrate-guidance.md), not an
established optimum for every scene, GPU or connection.

| Resolution | HEVC 30 / 60 FPS | H.264 30 / 60 FPS |
| ---------- | ---------------- | ----------------- |
| 720p       | 2.5 / 4          | 3 / 5             |
| 1080p      | 4 / 6            | 5 / 8             |
| 1440p      | 6 / 10           | 8 / 14            |
| 4K         | 12 / 18          | 16 / 25           |

Text/desktop uses 90% and Data saver 65% of these targets. Intermediate sizes
interpolate by pixel area. Above 60 FPS scales linearly and remains unqualified.
The 35 Mbps Auto ceiling bounds a requested target; it does not guarantee equal
quality at 4K120. Manual 100 Mbps remains available. Twenty subscriptions at
100 Mbps would exceed the VPS's 1 Gbps port; these per-publication controls do not
provide a deployment-wide shaper.

## Watching on slower connections

Advanced offers a watching allowance shared across actual subscriptions. Automatic
uses measured capacity when available and otherwise a 30 Mbps allowance, reserving
25% for overhead/competing traffic and 256 kbps per stream for audio. Capacity probes
inform this allowance; congestion feedback is not itself a speed test. Receiver loss
above 3% or mean jitter-buffer delay above 250 ms reduces the video budget by 25%.
Six healthy two-second samples allow a 5% recovery within the original allowance.
Absent feedback does not increase it; optional incoming-capacity stats are not
treated as a qualified downlink measurement. Subscriptions pause when the available budget cannot fit them, including all
watches on an extremely slow connection.

Each watcher advertises its per-stream video cap. The publisher uses the lowest
active watcher cap alongside its own sender feedback and requested target.
This single-layer publication is shared, so a slow watcher can lower quality for
everyone watching it. Independent per-viewer qualities are not supported.
Resolution/FPS are never silently reduced. Very slow
connections may need an explicitly smaller source mode, fewer watches, or Data
saver. Local tests do not establish performance on constrained networks.

## Applying settings

Bitrate/profile-only edits use Apply settings and update the existing encoder cap. The same RTC publication, video resolution/FPS, audio capture and preview clock remain active. Increasing the requested cap does not promise an immediate increase in network delivery. Source, codec, resolution, FPS or audio changes validate first and then briefly stop/restart capture/publication. Native watchers retain their watching choice through the new publication. Invalid edits leave the running stream intact. Stop remains available during acquisition/publication/reconfiguration and cancels queued work as well as active capture.

The RTC sender has a stable 100 Mbps ceiling so a live change can raise the requested cap without replacing the SDK sender or widening a native SDK patch. The GPU encoder separately obeys the user's smaller requested cap and sampled congestion feedback. User-requested reductions and the first feedback apply immediately; subsequent feedback writes are coalesced to at most once a second. Live requested increases ramp in steps of 25% of the current target, with a minimum 0.5 Mbps step, until reaching the requested/feedback ceiling. A fresh keyframe accompanies a user bitrate edit so codec-header changes do not depend on old queued references. This reduces bursts into a subscriber's existing bandwidth estimate and avoids needless hardware reconfiguration on small frequent BWE changes, especially on the [pinned AMF implementation](https://github.com/GStreamer/gstreamer/blob/1.28.8/subprojects/gst-plugins-bad/sys/amfcodec/gstamfh265enc.cpp). Congestion may temporarily lower effective bitrate below the 1 Mbps requested minimum; it does not rewrite the requested value, resolution or FPS. This is a sampled encoder target, not a per-packet bandwidth limiter.

Raw GPU buffers pass through a drop-only frame-rate limiter and pipeline-clock synchronization before encoding, preserving their D3D11 memory. A high-refresh monitor cannot quietly drive the encoder above its chosen cadence. Small startup queues can affect very short output-rate averages; stable received-frame windows remain the acceptance measurement.

## Understanding the measurements

The native UI distinguishes requested dimensions/FPS/bitrate, encoded output FPS/Mbps, effective encoder target, and actual outgoing video RTP Mbps/retransmissions to the SFU. The collector samples local aggregate RTC counters once a second with a bounded timeout; raw SDP, ICE credentials, candidates and room authority never enter UI snapshots. These sender facts do not prove what a friend decodes.

Native playback uses the pinned WebRTC receiver and platform decoder adapter.
Windows supports hardware H.264/HEVC/AV1 and filters out SDK software formats;
macOS uses hardware VideoToolbox decode and advertises AV1 only when supported. Frames pass
through bounded queues into native UI textures. Audio uses output-only WebRTC
playback with independent per-stream gain and mute.

Stop watching unsubscribes both A/V publications and drops playback pumps;
hiding or focusing a tile does not. Capability advertisement is followed by
actual receive checks. Missing output devices and stalled video surface errors.
HDR fidelity, physical 4K120, physical A/V timing and simultaneous game/capture/
multiple-viewer performance require separate hardware measurements.
