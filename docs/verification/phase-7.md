# Phase 7 verification

Phase 7 adds invitation lifecycle management, durable session/media revocation, admission on signaling handshakes, bounded recovery, aggregate diagnostics and authenticated external TURN/TLS. Production deployment remains Phase 8.

## Observed checks

- API tests cover rotation with preserved sessions, disable/revoke-all, selective revocation, replayed/refreshed JWTs, expiry removal queues, restart retries and denial of administrative proxy paths. Existing preview/API tests pass.
- Real browser testing passed rotation without interrupting media, selective session termination while another publisher stayed connected, session/token/preview/WebSocket denial, disable/revoke-all and management Lock. No invitation screenshots/traces are generated.
- Both NVIDIA HEVC/H.264 streams passed through the gate. Testing identified native bearer-header authentication and an IPv6-localhost/IPv4-listener mismatch; both are handled without changing production HTTPS hosts.
- A real local app restart recovered native publication/watched playback through the Canada SFU. Revoking only that native session removed its stream, kept the viewer connected and stopped GPU capture before its synthetic fixture deadline.
- The TLS test passed real publisher/viewer video with non-proxied UDP disabled and Relay only selected. Both candidates reported TLS and a `turns:` URL despite their peer-reflexive labels. Invalid credentials yielded no relay candidate; certificate verification stayed enabled.
- The temporary SFU has a 1 CPU/512 MiB cap; external TURN has 0.5 CPU/128 MiB, PID/read-only/tmpfs limits and allocation quotas. No business application or its HTTPS listener was replaced. Qualification uses a test-scoped hostname mapping and an existing valid certificate, not a FreedomStream production domain.

The final serial regression passed nine desktop/mobile browser scenarios (three intentional duplicate transport skips) and all nine standard native scenarios (the restart/TLS scenarios run explicitly). Synthetic 4K120, live bitrate edits, audio isolation/stereo, minute previews and zero unwatched traffic remain covered. API tests also cover schema-1 migration, static/API quota separation and immediate publisher-preview denial behind a blocked SFU removal. `npm run check` passes 28 API/config tests, 21 browser lifecycle/network tests and two relay-configuration tests. Native format/clippy/20 Rust tests and three setup-integrity tests pass.

An explicit TLS run continuously decoded for 365 seconds beyond setup with 300-second credentials and a 120-second allocation limit configured, then rejected invalid credentials. This validates the exercised renewal interval, not an entire group evening. Docker smoke/restart and the final explicit native app-restart/revocation scenario also passed. Exact-head CI and a fresh GPT-6.1-Sol high review remain the PR merge gate.

## External qualification

Native Relay only compiles; native TLS relay with the eventual production address/certificate still needs verification. The browser's test-only hostname mapping does not replace that check. Temporary signaling is operator-restricted test infrastructure; production must keep SFU signaling private behind the app gateway.

AMD, physical 4K120/game/fullscreen/alt-tab capture, full shared-VPS load, fresh-machine distribution, production renewal/restore and a four/five-publisher evening remain open. Automated success does not certify those outcomes.
