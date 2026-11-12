# Discord-created rooms verification

Implementation branch: `feat/discord-room-creation`, integrated with main `380912a`.

## Completed locally

- Node **24.22.1** `npm run check`: formatting, types, **65 tests** (59 server,
  two web and four production/relay configuration), and production builds passed.
- New signed-interaction tests use an ephemeral Ed25519 key and actual SQLite.
  Cover signature rejection, exact-body tampering, stale timestamps, PING,
  application/command/guild/DM denial, channel reply flags and disabled mentions,
  distinct rooms for both aliases, room admission, transactional replay protection
  across restart, rate recovery and isolation from browser Origin checks.
- The same test opens a real loopback HTTP listener and posts a signed command.
- Docker Compose built the app/SFU and reached healthy state in a separate local
  project. `npm run smoke -- --restart-app` passed actual HTTP admission, built
  assets, persisted sessions, logout and authenticated SFU access.
- `npm run test:admin`: all **eight browser tests passed**, including real SFU
  membership/reconnect and room closure after actual elapsed grace time, room
  administration, invitation/handoff pages and viewport checks.
- `cargo fmt --all -- --check` passed.
- Independent GPT-6.1-Sol high source review caught stale optional experiment
  call sites and a Clippy return issue. Fixed those; a fresh independent review
  approved the implementation, requesting a distribution documentation correction
  that is now included. Source approval does not waive runtime acceptance.

## Verification limits

The integration results below supersede the initial missing-runtime/setup status.
Native GUI startup and actual OS-protected persistence are verified. The rebuilt
native executable also joins/leaves through the real HTTP admission path, including
an invitation created by a live Discord command. The direct graphical media
connection encountered an untargetable Windows Firewall dialog and timed out;
that GUI/media path is not claimed as qualified by API or source checks.

Preserve the existing hardware, codec, audio, signed-distribution, fresh-machine
and real-group qualification gates. This PR adds no media capability or hardware
performance claim. The Discord trial uses a temporary HTTPS tunnel and isolated
local room database; production still needs the documented environment settings
and a permanent interactions endpoint. Bot credentials remain local and uncommitted.

Setup and privacy semantics are documented in [Discord](../discord.md).

## Integration with installer and update controls

Merged `origin/main` at `380912a` (PRs #30 and #31). Known-server startup
and existing-app activation now compare the saved canonical server origin,
without requiring or restoring an invitation. Retained the native update client,
activation inbox, prior-room dismissal handling and explicit Join confirmation.
Corrected the restart dialog in EN/PT-BR to describe fresh invitation entry.
Nested Rust targets/logs are excluded from Docker build contexts.

Revalidation on this integration:

- Node 24.22.1 `npm run check` passed again (65 tests and builds).
- Native default and `performance-experiments` Clippy passed with warnings denied
  for application code. Third-party SDK warnings remain upstream.
- Actual Windows native suite: 73 publisher tests passed, three environment/media
  tests ignored; portable suite: 10 passed. DPAPI v1-to-v2 migration and encrypted
  round-trip/tamper/forget tests passed.
- Installed-launcher default Clippy and all three protocol/registration tests passed.
  An initial attempt to check all launcher fixture features without their required
  isolated registry compile-time variables correctly failed; default checks passed.
- Docker build/restart smoke and all eight actual browser tests passed again,
  including SFU membership, reconnect and elapsed-time closure. The first parallel
  Docker context transfer was canceled; rebuilding the app succeeded.
- Fresh independent GPT-6.1-Sol high source review approved after the restart-text
  correction. Optional-feature Clippy also completed after the review.
- Actual native GUI opened an isolated encrypted legacy profile, displayed its
  saved name/server with an empty invitation and no autojoin, and rewrote it to
  version 2 with exactly `server,name` fields. A test join reached a Windows
  Firewall dialog that the automation API does not enumerate/target; media
  connection timed out. This is a native connection limitation, not a passed join.
- Discord application identity/public key verified through the authenticated API.
  After owner-authorized installation, both guild commands registered successfully.
- Discord accepted the temporary HTTPS interactions endpoint after a real signed
  PING and an invalid-signature probe. The actual endpoint returned 200 and 401,
  respectively.
- The owner invoked `/telar` in the private guild and confirmed the reply. The
  signature-verified endpoint recorded the command and its channel-visible response
  with SUPPRESS_EMBEDS (4). No invitation or Discord continuation token was logged.
- Anonymous access and a wrong invitation were denied for that live-created room.
  The rebuilt native executable then used the correct Discord-issued invitation
  over public HTTPS to admit a session, measure actual bounded down/up capacity,
  and leave successfully. This exercises native room admission without claiming
  a passed graphical media connection.
- Native admission/measurement/leave also passed independently against Docker.
  The installer integrity-input test passed. All GitHub Linux, Windows and
  container checks passed on integrated head `b1b92dc`; native hardware CI remains
  deliberately skipped, with the local native results listed above.
