# Development interfaces

## Modules

- `packages/contracts`: validated admission inputs and shared response types; preview cadence is 60,000 ms.
- `apps/server/src/rooms.ts`: room admission/session persistence behind a small interface. Storage owns hashing, expiry, identity creation and serialization.
- `apps/server/src/app.ts`: HTTP authorization, origin checks, cookies, request limits and LiveKit grants. It does not expose the media server API secret.
- `apps/web/src/media/MediaRoom.ts`: owns LiveKit signaling, browser capture and explicit audio/video subscriptions. UI receives browser tracks and immutable snapshots, not SDK objects. The SDK is dynamically loaded only after room admission.
- `apps/web/src/media/LiveStreams.tsx`: renders streams with per-stream volume, mute and grid/focus controls; playback elements detach when a subscription stops.

SQLite is sufficient for the initial single-VPS, single-app deployment. Do not point several independent instances at separate databases and expect shared sessions. Sessions are not accounts: a fresh admission creates a fresh participant identity.

## Configuration

`npm run setup` creates a private `.env`. Required secrets: `ADMIN_TOKEN` (at least 32 characters), `LIVEKIT_API_KEY` (at least 8), and `LIVEKIT_API_SECRET` (at least 32). `PUBLIC_URL` is the exact browser origin, with no path/query/fragment or credentials. Remote origins must use HTTPS; HTTPS also requires a `wss://` media URL. Loopback HTTP is accepted for local development. Port must be an integer in 1–65535. Credentials are never included in error messages.

The API does not trust forwarded IP headers by default. All mutating requests require an `Origin` matching `PUBLIC_URL`, including scripted/admin requests. Future native client authentication will need its own explicit contract; do not weaken browser origin validation to work around that later.

The local Compose configuration intentionally overrides the browser origin/media URL to `http://localhost:3000` and `ws://localhost:7880`. Native npm development uses the `.env` origin `http://localhost:5173`. A production static build can be served with `npm run build`, setting `PUBLIC_URL=http://localhost:3000` in `.env`, then `npm start`.

## HTTP interface

All errors are `{ "error": "safe explanation" }`. There is no public metadata or list endpoint.

| Method/path                       | Authorization                         | Behavior                                                                                                          |
| --------------------------------- | ------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| `GET /api/health`                 | None                                  | App liveness; not media readiness.                                                                                |
| `POST /api/rooms`                 | `Authorization: Bearer <ADMIN_TOKEN>` | Body `{name, displayName}`. Returns room/session and invitation URL once, sets room cookie.                       |
| `POST /api/rooms/:id/join`        | Invitation in JSON body               | Body `{invitation, displayName}`. Issues a fresh room cookie; replaces this browser's prior session in that room. |
| `GET /api/rooms/:id/session`      | Room cookie                           | Returns current room and participant; returns 401 for absent, expired or different-room sessions.                 |
| `POST /api/rooms/:id/leave`       | Room cookie when present              | Deletes the current session and clears its cookie; idempotent.                                                    |
| `POST /api/rooms/:id/media-token` | Room cookie                           | Returns a two-minute LiveKit join token and public media URL, limited to this room and participant.               |

Room creation is limited to 5 requests per minute per IP, admission to 10 per minute, and general API access to 120 per minute. These in-process limits reset on restart and are not a distributed abuse defense. JSON bodies are limited to 4 KiB. Media tokens do not prove LiveKit is running; liveness and authenticated media readiness are checked separately by the smoke script.

## Deliberate current limits

Browser camera/screen publication and playback are supported, with no microphone publication. Screen audio depends on what the browser/source offers. The browser baseline uses VP8 without simulcast; it does not establish native HEVC, hardware acceleration, Discord audio exclusion, minute previews, 4K120, invite rotation, TURN/TLS or production deployment. A 24-hour session survives server restarts but is intentionally time-bounded. An invitation authorizes all holders equally; it does not authenticate a person's real identity. Existing media tokens can be used until expiration. The app's Leave action first disconnects media and stops local capture; API session deletion alone does not evict an arbitrary already-connected participant. Server-enforced revocation arrives in Phase 7.

One active app tab per browser room session is supported: tabs sharing a cookie receive the same media identity, so a second tab can replace the first connection. Separate browsers/devices join as separate participants. SDK diagnostic log-level preferences may appear in browser storage; invitations, session cookies and media tokens must not be stored there. SDK logging is suppressed to avoid exposing signaling credentials.

Future native integration must keep encoder feedback, source switching and actual received quality separate from user-selected resolution/FPS/bitrate. Do not generate previews by decoding high-bitrate streams on the VPS. The publisher should generate the initial preview, then update at most once per minute.
