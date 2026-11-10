# Administrator dashboard verification

Implemented in the isolated `freedomstream-admin` worktree on
`feat/admin-dashboard`, based on `9447d0b` (`feat/native-room-feedback`). No root
AV1/icon work, fullscreen work, `network-test.ts` edits or proxy upload-limit edits
are included. No production activation was attempted; deployment and base
integration must be coordinated with the next friends-beta release.

The root page is now the single-administrator login. The password is the existing
private `ADMIN_TOKEN`; it is exchanged for an opaque eight-hour HttpOnly cookie
and never persisted in browser storage. Login is rate-limited; cross-origin
mutations are rejected. Logout, expiration and server restart revoke browser
access. The room admission API and private invitation/download routes remain
compatible with existing native clients and invitations.

The approved dark dashboard includes service totals, server traffic, individual
participant-session traffic/totals, real-time (15-minute), today, seven-day,
calendar-month and custom date filters, local timezone labels, live refresh,
missing-measurement states and paginated users. Multi-day charts include dates.
An isolated sample is drawn as a point; mobile charts adapt their coordinate
space to preserve readable labels. Speed is **Mb/s**; data is decimal **MB/GB**.

## Observed checks

- Node 24.11.1 `npm run check`: formatting, TypeScript, **39 API tests, 23 web
  tests, 4 deployment-policy tests**, and production builds pass.
- Patched pinned LiveKit `385aa40f9292fe1db8caad87301e4b285b7c198a` builds as a
  Windows executable using Go 1.26. The actual binary was used in the loopback
  transport check, not a mock SFU.
- Desktop and mobile Playwright suites: **10 passed**, 2 portable-download
  checks skipped because this isolated test has no installed portable artifact.
  Checks cover login, protected analytics, filters/custom dates, reload/logout,
  responsive width and existing room privacy/access flows.
- `scripts/verify-admin-usage.mjs`: actual headless Chromium H.264 canvas
  publication and an independent subscription passed through the admitted
  signaling gateway and patched SFU. Upload and download counters increased and
  were attributed to the correct distinct users. Report totals equalled the sum
  of user counters; user series contained measured traffic. Actual-data desktop
  and mobile dashboard screenshots were captured and inspected.
- Explicit unsubscribe: after a 15-second telemetry settling window, downstream
  media increased by less than 1000 bytes over ten seconds while publication
  uploads continued. The check fails if the subscription continues to carry media.
- API/unit checks additionally verify CSRF, cookie flags, expiration, restart
  invalidation, malformed queries, rate limiting, minute boundaries, new-session
  first counters, SFU generation changes, unknown gaps, independent same-name
  users, history persistence and SQLite online backup/restore.
- A fresh GPT-6.1-Sol review at high reasoning approved the isolated PR after
  the initial accounting/date-label/runbook findings were addressed.

One cold-start repeat of the local media harness failed its signaling connection.
A repeat after explicitly waiting for the SFU administrative RPC to become ready
passed, including actual-data screenshots and the unsubscribe check. Metrics
readiness alone is insufficient to establish readiness for a media join.

## Scope and deployment limits

Totals cover recorded SFU media/data bytes, including retransmissions, and exclude
HTTPS/speed tests, transport framing and extra TURN relay traffic. They are not
whole-VPS NIC counters or provider billing. Timing is stored at minute resolution;
SFU telemetry delivery and bucket averaging limit timing precision. Unobserved
traffic during collector downtime is not reconstructed. See
[collector setup and accounting](../../deploy/livekit-usage/README.md).

The custom Docker image build/runtime and Docker smoke remain unverified locally:
Docker Desktop's Linux engine is unavailable. CI runs the Linux image build,
Docker HTTP/restart smoke and browser suites. This local binary/media check does
not replace that deployment gate. Production activation, TLS proxy verification,
real friends traffic and TURN-path accounting remain separate acceptance work.
No native capture/encoder/hardware behavior changed or was newly qualified here.
