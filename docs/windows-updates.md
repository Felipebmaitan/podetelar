# Windows installers and updates

Starting with v0.1.2, installed builds check the public GitHub releases feed on
startup and periodically. Download and restart are explicit. Hover over the update
badge for notes; Later keeps the current version running. Restart ends sharing and
watching before applying the package. Versions 0.1.0 and 0.1.1 need one manual Setup
upgrade because their launchers have no compiled feed.

## Build and publish

Use Windows, Node 24, Rust 1.95, Visual Studio C++ and .NET SDK 8 or newer:

```powershell
npm ci
npm run native:setup
npm run native:package
$env:INSTALLER_BUILD_NUMBER = '1'
node scripts/package-installer.mjs
```

Use a clean committed checkout and increase the preview build number for each
build. The packager verifies the portable artifact's commit, size and SHA-256,
uses the pinned Velopack SDK/CLI, and writes `.local/installers/`. Update notes
come from `docs/releases/v<version>.md`; keep those files for packaging.

Publish the complete `public/` directory together: Setup, full `.nupkg`,
`releases.preview.json`, installer manifest and checksums. Prepare a draft release,
verify uploaded bytes, then publish with the package already present. Keep old
assets. Do not publish portable ZIPs or standalone executables. Public updater
releases must not be marked prerelease: `preview` is the internal package channel.

The manual **Windows installer preview** workflow builds, checks and qualifies
artifacts. Expiring Actions artifacts are not an update source. No private GitHub
token is embedded in the app.

## Feed and package validation

`deploy/windows/installer.json` defaults `updateFeed` to
`https://github.com/Felipebmaitan/podetelar`. A build-time
`PODETELAR_UPDATE_FEED` override may select a trusted credential-free HTTPS
feed or public GitHub repository; the workflow uses optional `WINDOWS_UPDATE_FEED`.
Packaging rejects invalid/missing feeds and local paths. Room data and runtime
IPC cannot change the compiled source.

The launcher refuses downgrades, cross-application packages, unsafe filenames,
missing hashes and packages over 1 GiB. It downloads full packages, verifies size
and SHA-256 before Ready and again before applying, and repairs corrupted cached
packages after an explicit download. Feed hashes do not establish an independent
publisher signature. The current Windows preview is unsigned.

## Process and invitation ownership

Velopack installs package ID `PodeTelar` under `%LOCALAPPDATA%\PodeTelar`.
The GUI bootstrap owns update checks and application of updates, outside the
portable publisher's cleanup job. Only an acknowledged Restart invokes Update.exe
after the media child exits. Later does not apply updates on ordinary exit/startup.

A per-launch authenticated loopback bridge carries bounded update status/commands;
room invitations and session authority never cross it. Rendering does not wait
on network checks or downloads. No capture or subscription resumes automatically.

Install/update hooks register `podetelar://` for the current user, targeting
the stable installed launcher. Uninstall removes only that installation's handler.
The launcher forwards subsequent activation/focus requests to the running app
through authenticated local IPC. Tickets remain in bounded memory queues and are
excluded from updater logging and restart arguments. See
[invitation behavior](distribution.md#invitations-and-launch-behavior).

## Acceptance

Run the workspace checks, native tests and packaging qualification, plus:

```powershell
cargo fmt --manifest-path apps/installed-launcher/Cargo.toml -- --check
cargo clippy --locked --manifest-path apps/installed-launcher/Cargo.toml -- -D warnings
cargo test --locked --manifest-path apps/installed-launcher/Cargo.toml
node scripts/qualify-installed-links.mjs
node scripts/qualify-update-controls.mjs
```

The link qualifier uses isolated registry keys and a publisher substitute to check
hooks and real-process forwarding. The update qualifier creates two real Velopack
packages and exercises discovery, download, Later, corrupt-cache repair and
Update.exe replacement/relaunch using a fixture process.

Before shipping, verify the actual installed publisher's media shutdown during
an update, concurrent launches, shortcut/room-link behavior, offline startup and
uninstall on fresh Windows. Verify public anonymous feed discovery/download and
uploaded package integrity for every release. Fixture success does not qualify
native UI interaction, production media cleanup, signing or fresh-machine behavior.
