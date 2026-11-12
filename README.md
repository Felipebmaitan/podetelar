<p align="center">
  <img src="assets/PodeTelar.png" alt="PodeTelar icon" width="128" height="128" />
</p>

# PodeTelar

Private screen and camera sharing for friends. Native Windows capture, server-forwarded media, a Discord-like room interface and selective watching are the intended product. Licensed under [MIT](LICENSE).

**Current branch: native Windows rooms.** Sharing and watching move into the
Windows app; the website keeps invitations, access management and downloads.
Local NVIDIA synthetic checks pass native HEVC/H.264 decode, compatibility fallback,
explicit A/V subscriptions, reconnects, higher-resolution previews and source-derived
audio exclusion. The self-contained portable EXE launches without a terminal and
opens without a saved server or nickname; each room invitation selects its backend. This is draft PR #10, stacked on portable
PR #9, and requires the matching server update. AMD, physical 4K120, fresh-PC/offline
startup, native TURN, A/V latency and real-game/group qualification remain open.
See [verification](docs/verification/phase-11.md), [distribution](docs/distribution.md)
and [quality](docs/quality.md).

See the [phase plan](docs/implementation-plan.md), [domain glossary](CONTEXT.md) and [research](docs/research/design-notes.md). Phase 1 is the initial commit; subsequent phases are separate PRs, each requiring acceptance checks and a GPT-6.1-Sol review at high reasoning before merge.

## Windows downloads

Use [PodeTelar Setup](https://github.com/Felipebmaitan/podetelar/releases/download/v0.1.0/PodeTelar-v0.1.0-Setup.exe)
for a per-user installation with shortcuts and room links. The unsigned installer
preview and the separate portable EXE are on the [release page](https://github.com/Felipebmaitan/podetelar/releases/tag/v0.1.0).
Verify the published checksums. Automatic update delivery is not enabled yet.

## Local development

Use Node.js 24 LTS (Node 25 is also accepted), npm and Docker Desktop or Docker Engine for the media server.

```sh
npm ci
npm run setup
docker compose up -d livekit
npm run dev
```

Open **http://localhost:5173**. The server owner can find `ADMIN_TOKEN` in the generated `.env` and use it in the create-room form. Keep that file private. Friends only need the room invitation; they do not need the server access key.

Configuration generation is idempotent and does not print or replace existing secrets. The API defaults to loopback; Vite forwards `/api` and `/media` WebSocket upgrades to it. Local room data is stored in `data/freedomstream.db`. Use `localhost` consistently rather than mixing it with `127.0.0.1`: invitations, cookies and request origins are checked against the configured origin.

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

Add `-- --restart-app` to verify session continuity through an app-container restart.
For the desktop administrator dashboard and access-management UI against the running stack:

```sh
npx playwright install chromium
npm run test:admin
```

The smoke and administrator checks create uniquely named rooms in the running database
and report no invitation/session/media authority. API tests use isolated databases
and exercise access, persistence, CSRF, revocation, grants and authenticated downloads.
Browser tests cover desktop operator workflows; mobile and participant browser-room
scenarios are retired. `npm run test:e2e` remains an alias for `test:admin`.
`npm run native:room-smoke` exercises actual native
HEVC/H.264 decode, fallback, subscriptions, preview/reconnect and audio against an
isolated Windows SFU. These checks do not qualify real games, AMD, physical 4K120,
A/V latency or large-group performance. Docker smoke is not a streaming test.
See [test scope and commands](docs/testing.md) for the current coverage inventory
and the distinction between automated checks and hardware qualification.

## Access model

- Only the server access key can create a room. There is no public room directory.
- A room invitation contains 32 random bytes of authority after `#invite=`. The browser removes it from its address bar and keeps it in memory for the admission/copy flow; no token is stored in browser local storage.
- SQLite holds hashes of invitations and session tokens, not their raw values. API responses are not cached and automatic request logging is disabled.
- Admission creates an HttpOnly, SameSite=Strict cookie scoped to that room. HTTPS origins also use Secure cookies. Sessions last 24 hours; the pinned invitation remains valid across restarts and departures.
- LiveKit tokens are room-scoped and expire after two minutes. The native route grants an admitted participant publishing/subscription authority; browser media grants deny both. Invitation rotation, disablement and individual/all-session revocation enforce admission through the signaling gateway. Accounts and viewer-only roles are not implemented.
- Leaving deletes the current session, not the room or invitation. A member can rejoin with the invitation.

The original code in this repository is MIT licensed. Dependencies and any future bundled native runtime retain their own licenses. See [configuration and interfaces](docs/development.md) for the current module contracts and deliberate limits.

See [Windows distribution](docs/distribution.md), [production deployment and recovery](docs/deployment.md), and [Phase 8 qualification](docs/phase8-qualification.md) for release/install steps, private room downloads, HTTPS without a domain, backup/restore and remaining hardware/group checks.

The native room work on `feat/native-rooms` moves sharing and watching into the
Windows app. Browsers retain invitations, access management and downloads. See
[the native acceptance workflow](docs/native-publisher.md), [quality controls](docs/quality.md)
and [automatic source audio](docs/audio.md). This branch is not a qualified
production release until its acceptance gates and independent review pass.

Optional [Discord commands](docs/discord.md) create a new private room with `/stream` or `/telar`.
