# FreedomStream

Private screen and camera sharing for friends. Native Windows capture, server-forwarded media, a Discord-like room interface and selective watching are the intended product. Licensed under [MIT](LICENSE).

**Current phase: 3 — native Windows hardware publication.** Create a private room, invite friends, and explicitly choose which streams to watch. The Windows publisher captures a monitor or window and sends hardware HEVC or explicit H.264 to the same browser rooms. Unwatched streams do not send their full video/audio to you. Audio isolation, minute previews and quality controls follow in their own phases. See the [native publisher guide](docs/native-publisher.md) and [qualification results](docs/verification/phase-3.md); AMD and game testing remain open.

See the [phase plan](docs/implementation-plan.md), [domain glossary](CONTEXT.md) and [research](docs/research/design-notes.md). Phase 1 is the initial commit; subsequent phases are separate PRs, each requiring acceptance checks and a GPT-6.1-Sol review at high reasoning before merge.

## Local development

Use Node.js 24 LTS (Node 25 is also accepted), npm and Docker Desktop or Docker Engine for the media server.

```sh
npm ci
npm run setup
docker compose up -d livekit
npm run dev
```

Open **http://localhost:5173**. The server owner can find `ADMIN_TOKEN` in the generated `.env` and use it in the create-room form. Keep that file private. Friends only need the room invitation; they do not need the server access key.

Configuration generation is idempotent and does not print or replace existing secrets. The API defaults to loopback; Vite forwards `/api` to it. Local room data is stored in `data/freedomstream.db`. Use `localhost` consistently rather than mixing it with `127.0.0.1`: invitations, cookies and request origins are checked against the configured origin.

## Entire local stack in Docker

```sh
npm run setup
docker compose up --build -d
```

Open **http://localhost:3000**. This runs the production app build and LiveKit with the same generated media credentials. The `room-data` volume preserves rooms/sessions across restarts. `docker compose down` retains that volume. Do not remove the volume unless you intend to erase that data.

This Compose file is **local-only**: published ports bind to loopback, media candidates include loopback and the private container interface, and HTTPS/TURN are not configured. Keeping the container candidate lets native Linux hosts reach the SFU without Docker Desktop's forwarding proxy. ICE Lite avoids external STUN discovery for this local stack. Production VPS deployment is Phase 8. Do not expose this stack by simply changing its port bindings.

## Checks

```sh
npm run check
docker compose config --quiet
```

With the Docker stack running, exercise actual room creation/admission, persistence and LiveKit credentials:

```sh
npm run smoke
```

Add `-- --restart-app` to the smoke command to also verify session continuity through an app-container restart. For desktop/mobile browser admission checks against the running stack:

```sh
npx playwright install chromium
npm run test:e2e
```

The smoke and browser checks create uniquely named rooms in the running database. The smoke check prints only pass/fail status. It does not output invitations, session cookies or media tokens. Automated API tests use isolated temporary databases and verify unauthorized access, room scoping, session expiry, persistence, CSRF, rate limiting and signed media grants. The browser media scenario uses synthetic capture with real LiveKit signaling, codecs and RTP: two publishers, an independent viewer, audio controls, measured stopped-video traffic, late joins, reconnect/reload and cleanup. It is not a GPU or real desktop-capture qualification. The Docker smoke check is not a streaming test.

## Access model

- Only the server access key can create a room. There is no public room directory.
- A room invitation contains 32 random bytes of authority after `#invite=`. The browser removes it from its address bar and keeps it in memory for the admission/copy flow; no token is stored in browser local storage.
- SQLite holds hashes of invitations and session tokens, not their raw values. API responses are not cached and automatic request logging is disabled.
- Admission creates an HttpOnly, SameSite=Strict cookie scoped to that room. HTTPS origins also use Secure cookies. Sessions last 24 hours; the pinned invitation remains valid across restarts and departures.
- LiveKit tokens are room-scoped and expire after two minutes. A valid invitation holder can publish and subscribe; individual accounts, viewer-only grants, invitation revocation and media-disconnect enforcement arrive in Phase 7.
- Leaving deletes the current session, not the room or invitation. A member can rejoin with the invitation.

The original code in this repository is MIT licensed. Dependencies and any future bundled native runtime retain their own licenses. See [configuration and interfaces](docs/development.md) for the current module contracts and deliberate limits.
