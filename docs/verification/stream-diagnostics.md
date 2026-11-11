# On-demand stream diagnostics (issue 26)

Advanced information is available in the shared watched-stream context/ellipsis
menu, in English and Portuguese. It does not issue Watch, change audio, or capture
previews. Closing preserves the subscription. A video-pump replacement resets the
panel and requires opening it again.

## Collection and interpretation

One publisher worker checks demand every three seconds. Without demand it does
not snapshot diagnostic aggregates or upload telemetry. Capture/encoder input pad
counters and capture-queue overrun counting are gated by that demand. Existing
outbound delivery and receiver budget monitoring continue their original media
control duties. An open viewer panel requests one aggregate every three seconds;
its lease lasts ten seconds. The publisher shares one task across viewers. Closing
is noticed within three seconds; a crash, unsubscribe/revocation or lost close
request is bounded by the lease and next publisher demand check. Reconnect,
unsubscribe, absent video pumps and publication replacement retire viewer workers.

Samples use the SFU track SID as their generation, a monotonic sender sequence and
an elapsed window. The server supplies sample age from its own receipt clock. No
cross-machine clock subtraction occurs. Sender capture/output/accepted FPS and
encoded payload bitrate are interval deltas. Encoder counters carry their own
elapsed window because their existing snapshot can lag by half a second. Sender sent bitrate/FPS and transport
RTT/loss reuse the most recent existing one-second outbound delivery sample;
they are not exact simultaneous three-second measurements. Configured dimensions
are encoder input geometry; decoder output may include padding. Neither proves
visible crop. The viewport is measured in physical pixels from the actual image
rectangle and UI pixel scale. RTT is a transport component, never physical
end-to-end latency.

Capture output counts actual source buffers before the bounded leaky entry queue;
encoder input counts buffers reaching the hardware encoder, after cadence/scaling.
Window capture may repeat a retained image downstream. Queue depths are gauges
from the latest half-second encoder snapshot, not interval maxima. Capture queue
overruns count genuine queue saturation signals, not inferred encoder drops.
SDK handoff skips/rejections include deliberate keyframe waiting. Encoder target
changes count bitrate property edits, not all internal hardware reconfiguration.
Requested target, actual encoder property, upload/watcher caps and congestion
feedback are separate. Configured caps need not explain every transient encoder
ramp or network limitation.

Receiver decoded output counts frames delivered by NativeVideoStream. Texture
submissions count uploads into the UI, not physical display refresh/presentation.
Mailbox overwrites measure replacement before the UI takes the converted picture.
SDK received/decoded FPS, mean decode time and jitter-buffer delay use local
counter deltas. SDK loss/drop/retransmission/NACK/PLI/freeze events are shown only
when positive: its deserializer defaults absent fields to zero, so omitted events
are explicitly unavailable or unreported, rather than proof of zero loss. Decoder
implementation names are allowlisted. Unsupported timing, visible crop, encoder
internal drops/overload and physical presentation/latency stay unavailable.

## Server boundary and retention

The room cookie authenticates each request. Owner is derived from that session
for publisher `self` routes; uploads require matching live track ownership.
Viewer demand requires the participant-owned selective-watch attribute in the
SFU and a current room session. The SFU administration API does not expose actual
RTP subscription state; this is the existing authenticated watch-intent contract,
not independent RTP subscription proof. Clients cannot create watch intent using
the diagnostics endpoint. Publisher demand polls revalidate all leases against
the bounded SFU participant snapshot; reads/writes recheck live publication before
mutation. Per-publication exclusion fences concurrent generation mutations.

Limits: 4 KiB strict numeric/enum-only upload, 2-second minimum upload interval,
240 requests/minute per hashed session, 256 publications, 32 leases/publication,
16 in-flight publication requests, at most 100 participants in each SFU snapshot,
one latest sample/publication, ten-second lease, one-second expiry sweep. No
media contents, invitation, token, credentials, raw SDK structures or arbitrary
publisher strings are accepted. The native HTTP client also bounds replies to
8 KiB and requests to two seconds. Failures affect diagnostics only.

Samples never enter SQLite, filesystem retention, usage analytics or backup
inputs. Last demand expiry removes the entry; publication-invalid requests clear
it immediately. Source review: Fastify disables request logging; diagnostic
failures return fixed messages without logging exceptions. Generated production
Nginx disables access logging and sends errors to `/dev/null`, with response
buffering disabled (`scripts/prepare-production.mjs`). No diagnostic body logger
is added. The production app's bounded container error log is not a telemetry
sink. This review does not claim inspection/deployment of the running VPS config.

## Verification

- Node 24 checks: formatting/types/API tests/builds pass.
- Native Clippy passes; native tests pass (68 publisher, three existing ignored;
  portable results in the local native test log).
- API tests: default-off, two leases/reference counts, last close/expiry, missing
  and stale samples, wrong owner/anonymous/cross-room/unwatched denial, bounds,
  invalid fields, live replacement and an in-flight publication race.
- A new isolated native AV1 1080p120 fixture exercises publisher -> API -> two
  native viewers, including Stop/rewatch and a deliberately reduced watcher cap.
  It passed in 46.6 seconds: both viewers received fresh sender aggregates,
  collection continued after one left, and the reduced viewer allowance was
  visible separately from the 25 Mbps requested target. This fixture does not touch saved
  rooms or the live production session.

Physical screen presentation, AMD/game load, deliberate packet-loss/network
impairment, controlled CPU overload and real UI interaction are separate gates.
Adding this tool does not establish a stutter or low-bitrate performance fix.

Observed NVIDIA fixture evidence: capture and encoder input near 120 FPS,
send near 120 FPS, effective target about 4.24 Mbps while the deliberately
restricted viewer was present, and one initial preview upload. The first
short-duration run observed no fresh samples for its second viewer; extending
that viewer through a complete post-unsubscribe/rewatch window passed. The
reports are local ephemeral acceptance artifacts, not production telemetry.
