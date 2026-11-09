# Room access

Each browser and native publisher has its own 24-hour session. An invitation admits new sessions; it is not a person's identity. Friends retain the same publishing/watching privileges. No accounts or viewer roles are needed to manage this private group.

Open **Manage room access**, or `/manage/<room-id>`, and supply the server access key. The key and new invitation stay only in the mounted form; Lock clears them. This page works after ending your own session, so the new invitation remains available to save. Every management request requires the server key independently of membership.

- **Rotate invitation** invalidates the old link for new joins and keeps current sessions by default.
- **Disable invitation** prevents new joins. Rotate to enable a fresh invitation.
- **Also end every current session** applies either change to current API/media sessions too.
- **End session** revokes one browser/publisher and leaves other participants connected. A holder of a still-valid invitation can join again; rotate or disable a leaked invitation when ending its sessions.
- Leaving, rejoining in the same browser and expiry also revoke the old session's media authority.

Only invitation/session hashes enter SQLite. Schema version 3 preserves version-1 rooms/sessions while adding invitation state and a durable removal queue. Migration DDL and its version update commit together, with rollback on failure. Version-2 qualification databases gain persisted retry scheduling in the same transaction. Each room admits at most 32 current sessions.

## Signaling admission

Short JWT expiry alone cannot revoke self-hosted media: SDKs receive refreshed JWTs, and deleting an HTTP session does not terminate a peer connection. The app's signaling gateway verifies signature/issuer/expiry, room grant and the current session identity on every initial/resumed handshake, including refreshed tokens. It accepts browser query tokens and native bearer headers, rejecting duplicate/ambiguous authority. Browser Origin checks remain exact; bearer-header native clients need not fabricate a browser Origin.

Only `/media/rtc`, `/media/rtc/v1` and their `/validate` endpoints are exposed. SFU administrative routes are never proxied. This module forwards opaque signaling, with no media decoding/transcoding: media travels directly to the SFU or through TURN. Limits are 128 connections, four per session, 1 MiB messages/queues and an eight-second upstream handshake. Backpressure/errors close the link; SDKs own reconnect attempts.

**Keep the SFU HTTP/WebSocket listener private.** A public route directly to it bypasses the app's admission checks. Checked-in Compose configurations bind that listener to loopback. A production reverse proxy must send `/media/*`, including upgrades, to the app and must not publish SFU administration/signaling. Existing room cookies remain HttpOnly/SameSite/HTTPS-protected.

`PUBLIC_URL` derives the public gateway address. Remove the old direct `LIVEKIT_URL` setting. `LIVEKIT_INTERNAL_URL` supplies the private HTTP(S) SFU origin for signaling/administration. Vite proxies `/media` upgrades. HTTP localhost development advertises explicit IPv4 loopback because the native WebSocket transport otherwise attempts `::1` against the IPv4 Docker listener. Production HTTPS hosts are preserved.

## Ending established access

Revocation commits session deletion and queued SFU removal in one SQLite transaction. API/preview/new signaling authority is denied immediately. Cached previews and pending uploads are purged; the gateway closes revoked links and rechecks authority while forwarding. Administrative participant removal has a three-second SDK request timeout.

If SFU enforcement fails, management returns **202** with pending disconnections, rather than reporting a completed live disconnect. A non-overlapping two-second sweep retries persisted work after restart with at most eight concurrent RPCs. Persisted next-attempt times move attempted work behind older due jobs, so repeated failures cannot monopolize a bounded batch. Successful removals retain a 30-second retry tombstone across a handshake already in flight; failed work remains until successful. Expiry uses the same path. Established encrypted transport cannot be recalled until an unreachable SFU enforces removal.

The browser checks its session on connection changes and every 20 seconds while quiet, with one bounded request, and releases the room view/captures after loss of authority. Native terminal disconnect cancels its capture group. SDK recovery can resume admitted sessions after app restart; it cannot recreate revoked authority. Genuine new publications still require Watch.

Primary context: [LiveKit authentication/self-hosted revocation](https://docs.livekit.io/home/get-started/authentication/), [pinned SFU participant removal](https://github.com/livekit/livekit/blob/v1.13.11/pkg/service/roomservice.go), [Fastify WebSocket lifecycle](https://github.com/fastify/fastify-websocket).
