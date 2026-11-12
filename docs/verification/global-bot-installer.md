# Global bot availability and first public installer

The deployed server had no Discord settings and the release contained only a
portable executable. Older hosted installer jobs were rejected before their steps
by GitHub account billing/spending limits; they did not produce release assets.
The permanent HTTPS endpoint was configured and verified by Discord, then the
user clarified that commands must work for every server installing the app.

## Source and authorization

- Build source: `fda6ce63ae35ffa2be4bc35023fbd4e840b59891`.
- Register only `/stream` and `/telar` as global command POST upserts, with guild
  installation and interaction contexts. Discord API verification reports both
  with `integration_types: [0]` and `contexts: [0]`; the bot allows public installs.
- Explicit `DISCORD_GUILD_IDS=*` accepts a guild only when the signed guild owner
  field matches its ID. DMs, user-only owners and mismatches fail closed. Explicit
  ID lists retain private-deployment behavior. Preserve signature freshness,
  application identity, replay protection and per-user/guild quotas.
- Fresh GPT-6.1-Sol high source review approved without blockers.
- Node 24.22.1 check passed: formatting/types, 71 tests and builds. This includes
  global registration scope and wildcard installation-owner rejection tests.
- Actual Docker/restart smoke and all nine browser tests passed on isolated ports
  3031/17980/17981/17982, preserving the already-running local stack. The first
  attempt used occupied default ports; the isolated retry passed.

## Installer lifecycle

The first public package ID is `PodeTelar`, so its default root is
`%LOCALAPPDATA%\PodeTelar`. Keep encrypted profiles under
`%LOCALAPPDATA%\FreedomStream`; the former local-only preview ID would collide
with that directory during install/uninstall. Stable launcher/protocol names
remain compatible. No previous public installer base requires migration.

- Rebuilt native portable and installer from the committed source above. Portable
  SHA-256: `c74eca4356c4557930393c4a6faaa607162783f6e62061ee43c82ebae11325db`.
- Installer internal version: `0.1.0-preview.202611124`, built with pinned
  Velopack 1.2.161 and curated runtime/source notices. Public artifact is named
  `PodeTelar-v0.1.0-Setup.exe`; installer metadata records its own source/hash.
- Portable qualification passed isolated/concurrent first extraction, native
  runtime probes, bundled CRT fallback, sources/payload checks, cache retention,
  explicit extraction and corruption rejection.
- Actual silent Setup installed to the real default PodeTelar directory. Explicit
  post-install launch opened the real native window, using an isolated profile
  and cache for the test. Silent Setup itself does not auto-launch the UI; the
  initial harness expected this incorrectly, then the explicit-launch test passed.
- Both shortcuts and the stable installed `freedomstream://` handler were verified.
  The real uninstaller removed the installed app, preserving the existing real
  encrypted profile byte-for-byte. Reinstallation succeeded and remains available
  on the development machine. The user's existing portable process was preserved.

This is an unsigned installer preview. It is not a fresh-PC/offline qualification,
a signed distribution, a production media-update trial or a working public update
feed. Existing AMD, physical 4K120, group/game and A/V timing gates remain open.
There is no claim that GitHub's earlier blocked jobs passed; new PR checks are
running independently of the successful local acceptance results.

## Production rollout

Permanent bot endpoint: `https://stream.maitan.me/api/discord/interactions`.
Keep the bot token only in local registration tooling; production runtime uses
only the application ID, public verification key and installation policy. The
server's existing credentials/data are preserved. Global wildcard activation,
legacy guild-command cleanup and final public asset upload are recorded in the PR
and release metadata after deployment; no temporary tunnel is a production service.
