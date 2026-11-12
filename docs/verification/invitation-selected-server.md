# Invitation-selected native server

The native app no longer reads or writes a saved server, nickname or language
choice. Legacy profile/preferences files are removed without parsing. Runtime
extraction, protocol registration and installer/update state remain separate.

Local Windows acceptance on 2026-11-12, Node 24.11.1:

- `npm run check`: formatting, types, 62 API tests, two web tests, seven tooling
  tests and production builds passed.
- `npm run native:check`: formatting and workspace Clippy passed. Pinned external
  SDK warnings remain; application warnings are denied.
- `npm run native:test`: 75 publisher tests and ten portable tests passed. Five
  live-fixture/hardware publisher tests remain excluded from the default run.
- `node scripts/qualify-invitation-servers.mjs`: explicitly ran the live-fixture
  native test against two actual HTTP listeners with independent SQLite stores.
  Each one-use handoff selected its own backend and room; replay and foreign
  session reuse were denied. The fixture substitutes the media directory; it
  establishes native handoff/admission behavior, not an SFU/media qualification.
- UI regressions cover first nickname submission, a partially typed nickname
  followed by another server's link, idle-session nickname reuse, active-room
  confirmation, latest-link replacement, dismissal and worker cleanup on close.

No capture or subscription starts from a room link. Existing codec, hardware,
fresh-machine and network qualification limits are unchanged. Packaged binaries
must be qualified separately before replacing release assets.
