# Discord-created rooms verification

Implementation branch: `feat/discord-room-creation`, based on main `6dfb8f2`.

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

## Remaining release gates

- `npm run native:check` stops before Clippy because this isolated checkout has no
  development media runtime. Full native checks (including performance experiments)
  and Windows DPAPI migration tests have **not** run here.
- Actual native opening, migration of a prior profile, server/name-only restore,
  fresh invitation join and Forget saved server need GUI verification.
- No Discord application credentials were supplied, installation/registration
  performed or channel message sent. Actual Discord PING validation, HTTPS
  reachability and both commands in a private guild remain unverified.
- Preserve the existing hardware, codec, audio and distribution qualification gates.
  This PR supplies no additional hardware/media performance evidence.

Keep the PR draft and do not merge/release until its outstanding acceptance gates
are exercised. Setup and privacy semantics are documented in [Discord](../discord.md).
