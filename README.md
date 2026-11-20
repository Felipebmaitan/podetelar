<p align="center">
  <img src="assets/PodeTelar.png" alt="PodeTelar icon" width="128" height="128" />
</p>

# PodeTelar

Private screen and camera sharing for friends on **Windows and macOS**.
Native apps capture and play media, with hardware video encoding and independent
volume and mute controls for each watched stream. LiveKit forwards media without
server transcoding. Unwatched streams show still previews instead of receiving video.

Supports Windows x64 and Apple Silicon Macs running macOS 14 or newer.
Licensed under [MIT](LICENSE).

## Downloads

Get the installer for your platform from [GitHub Releases](https://github.com/Felipebmaitan/podetelar/releases/latest).

- **Windows x64:** use Setup for a per-user installation with shortcuts and room links.
  Built-in update controls are available starting with v0.1.2; earlier versions need
  one manual Setup upgrade. See [Windows distribution](docs/distribution.md).
- **macOS Apple Silicon, macOS 14+:** use the DMG. Intel Macs are not supported.
  See [macOS installation and development](docs/macos.md).

These are preview builds. Windows installers are unsigned; macOS builds are ad hoc
signed, without Apple notarization. Check the published checksums. Hardware support
and remaining qualification limits are described in the platform guides.

Open a room invitation from a friend or the optional Discord `/stream` and `/telar`
commands. The invitation selects the server. Sharing and watching require explicit
actions; Stop watching unsubscribes from both video and audio. Rooms close after
all participants leave, allowing a short reconnect grace period.

## Local development

Use Node.js 24 LTS, npm and Docker Desktop or Docker Engine for the media server.

```sh
npm ci
npm run setup
docker compose up -d livekit
npm run dev
```

Open **http://localhost:5173** and sign in to the administrator dashboard using
`ADMIN_TOKEN` from the generated `.env`. Create a room and open its invitation in
the native app. Keep `.env` private; friends need only the invitation.

Setup preserves existing secrets. The API binds to loopback and Vite proxies
`/api` and `/media`. Room data lives in `data/podetelar.db`. Use `localhost`
consistently: configured origins, cookies and invitations must agree.

To run the complete local stack:

```sh
npm run setup
docker compose up --build -d
```

Open **http://localhost:3000**. The `room-data` volume survives container restarts
and `docker compose down`. This configuration is local-only; use the
[deployment guide](docs/deployment.md) for public HTTPS, media ports and TURN.

## Contributing

```sh
npm run check
docker compose config --quiet
```

With the Docker stack running, check actual HTTP and administrator behavior:

```sh
npm run smoke -- --restart-app
npx playwright install chromium
npm run test:admin
```

See [development interfaces](docs/development.md) for the code layout and
configuration, [testing](docs/testing.md) for coverage and native acceptance, and
the [documentation index](docs/README.md) for platform, deployment and feature guides.
Application code is MIT licensed; bundled dependencies retain their own licenses.

## Privacy and access

Rooms have no public directory. Invitations grant admission, while time-limited
room sessions authorize API and media access. Clients keep permanent invitations in
URL fragments and memory; invitation/session secrets and media credentials must never
enter logs or browser local storage. Operators can rotate invitations, revoke
sessions and close rooms. See [room access](docs/access.md) and the
[domain glossary](CONTEXT.md).
