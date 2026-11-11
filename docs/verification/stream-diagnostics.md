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
  invalid fields, live replacement, an in-flight publication race, session departure
  and shared production snapshot validation.
- A new isolated native AV1 1080p120 fixture exercises publisher -> API -> two
  native viewers, including Stop/rewatch and a deliberately reduced watcher cap.
  It passed in 46.6 seconds: both viewers received fresh sender aggregates,
  collection continued after one left, and the reduced viewer allowance was
  visible separately from the 25 Mbps requested target. This fixture does not touch saved
  rooms or the live production session.

Physical screen presentation, AMD/game load, deliberate packet-loss/network
impairment, controlled CPU overload are separate gates.
Adding this tool does not establish a stutter or low-bitrate performance fix.

Observed NVIDIA fixture evidence: capture and encoder input near 120 FPS,
send near 120 FPS, effective target about 4.24 Mbps while the deliberately
restricted viewer was present, and one initial preview upload. The first
short-duration run observed no fresh samples for its second viewer; extending
that viewer through a complete post-unsubscribe/rewatch window passed. The
reports are local ephemeral acceptance artifacts, not production telemetry.

Independent review: fresh GPT-6.1-Sol high review approved the code after fixes
for generation mutation races, worker retirement, SFU lookup amplification and
receiver freshness. Follow-ups made non-advancing encoder snapshots unavailable
and corrected the availability footer. The subsequent final media rerun exposed
an intermittent second-viewer sampling failure; the native diagnostic client now
retries publication-busy responses with bounded jitter inside a two-second budget,
so periodic samplers do not repeatedly collide. Acceptance results below must be
read with that follow-up's rerun result.

Native setup in this worktree rejected a downloaded installer whose checksum did
not match the pin. Checks used the existing development runtime read-only from
the separate performance checkout, verified SDK copies in this worktree and an
isolated short Cargo target path. No existing SDK, live executable, saved room or
production service was replaced.

Final follow-up verification: Node 24 check, native Clippy and native workspace
tests passed. After bounded diagnostic-busy retries, the two-viewer real-media
fixture passed in 45.9 seconds and then again in 45.7 seconds. The existing AV1
software-decoder fixture also passed (27.7 seconds), including previews,
unsubscribe/rewatch and reconnect. This does not constitute CPU-overload testing.

Actual native UI acceptance used a separate local test room/profile and the
isolated SFU. Verified Watch, the PT-BR right-click Advanced information action,
local and sender live samples, scrolling, closing without stopping watching,
language change to English, the hover ellipsis action, reopening the English
panel and Stop watching returning to Preview/zero watching. Observed about
120 decoded FPS versus 62 UI texture submissions/s with presentation overwrites;
these are measured stages, not a display-refresh or performance-fix claim.
The test GUI closed normally and its fixture services terminated. The temporary
invitation was deleted; no production session or profile was touched.
Controlled interruption acceptance is recorded below. Literal packet-loss and CPU-saturation sweeps remain unverified.

## Pre-merge recheck

Merged current main, including its fullscreen cursor correction, into this
feature branch. Fresh independent GPT-6.1-Sol high
reviews approved the application code and the test-only follow-up for merge.

- Node 24.11.1 checks and native Clippy/workspace tests pass.
- Eight diagnostic API tests pass, including 31 simultaneous diagnostic viewers
  at the existing 32-session room limit, lease expiry at capacity, and the global
  16-publication concurrency limit. The participant snapshot path is exercised.
- The real two-viewer AV1 1080p120 acceptance passed again (46.3 seconds).
- Controlled degradation acceptance passed twice (45.7 and45.9 seconds): the isolated test SFU
  is paused for three seconds, resumed, and the fixture publisher is separately
  paused for three seconds. Receiver diagnostic FPS falls during the SFU-outage
  window and subsequently recovers above 90 FPS. Sender samples retain requested
  120 FPS while reporting both reduced (<100) and healthy (>110) capture FPS.
  Capture assertions cover a broad post-start window; they do not establish
  precise pause-to-sample timing or physical end-to-end latency.

The pause helper only targets PIDs captured from this harness's own child
processes, verifies the exact executable path, and resumes in a finally block.
The harness terminates its children afterward. No system-wide CPU load, firewall
rules, production processes, saved room or live session are changed. Native
fixture aggregates remain memory-only; no new telemetry file is retained.

This completes controlled stage-interruption verification for the diagnostic
feature. It does not qualify a statistical packet-loss sweep, CPU-saturation
benchmark, AMD/game performance, physical display refresh or a stutter fix.
