# Test scope

Participants join, publish and watch in the Windows app. Browser automation covers
desktop operator administration. The old browser media controller is no longer
connected to the web entry point, so its mocked capture, playback and network tests
are retired along with mobile emulation and participant browser-room scenarios.

## Automated checks

Use Node 24 LTS and `npm ci`.

| Command                          | Coverage                                                                                                                                                                                   |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `npm run check`                  | Formatting, workspace/tool types, API and configuration tests, administrator usage formatting tests, and production builds.                                                                |
| `npm run test:admin`             | Desktop administrator login/logout, usage filters, room creation/directory, and per-room access-management Lock. Requires Chromium and a running app. `test:e2e` is a compatibility alias. |
| `npm run smoke -- --restart-app` | Actual HTTP room API admission, authorization, persisted session recovery, leave, built web assets and authenticated SFU access against Docker.                                            |
| `npm run native:check`           | Native formatting and Clippy. Requires the qualified Windows media runtime and SDK.                                                                                                        |
| `npm run native:test`            | Native and portable Rust tests: capture cancellation, cleanup, audio policy, preview cadence, quality/network controls, encrypted settings and packaging/process contracts.                |
| `npm run native:room-smoke`      | Actual native decode, codec fallback, watch/stop/rewatch traffic, preview/reconnect, receiver allowance and audio against an isolated Windows API/SFU.                                     |

Install Chromium with `npx playwright install chromium` before running administrator
tests. Native acceptance uses Playwright's API client and test runner to launch
native executables; it requires no browser installation. `npm run native:e2e` runs
that same suite against an already configured server and native executable.

CI runs Node checks on Windows and Linux and the Docker smoke plus desktop
administrator UI on Linux. Hosted native build/packaging qualification remains an
explicit `Checks` workflow dispatch with `run_native` enabled. A green default CI
run does not mean native hardware/media qualification ran.

## Coverage retained outside browser participant flows

- Server tests protect invitation secrecy, room-scoped sessions, persistence,
  expiry, CSRF, revocation, signaling and preview authorization. Native clients
  still use these endpoints.
- `apps/server/test/admin-rooms.test.ts` checks both anonymous and admitted room
  participants cannot use administrator APIs. Administrator UI tests need no
  participant browser join to repeat this contract.
- `apps/server/test/distribution.test.ts` checks authenticated portable downloads,
  installed bytes/manifest integrity, expiry after leave and legacy redirects.
  Portable qualification checks the actual packaged executable separately.
- `apps/web/test/usage.test.ts` protects the active administrator charts' units and
  date ranges. The unused browser media controller's mock tests are removed.
- Native media and audio scenarios remain separate even when they use the same
  fixture: local process-audio isolation and end-to-end stereo delivery catch
  different failures. H.264, HEVC and AV1 decoder paths also remain distinct.

The opt-in `scripts/verify-admin-usage.mjs` probe uses browser-generated synthetic
media to measure the server's usage accounting. It does not exercise participant
browser-room UI or qualify native capture, and is retained for that server purpose.
It also checks PC aggregation and expandable session details using actual SFU
traffic. For independent-process Windows identity/native HTTP qualification, see
[PC usage identity](device-usage-identity.md#repeatable-qualification).

## Qualification limits

Synthetic fixtures and unit tests do not qualify real games, Discord sessions,
AMD hardware, fresh-machine/offline startup, production TURN/network recovery,
physical 4K120, synchronized A/V latency or multi-friend load. Preserve the separate
verification records and open acceptance gates.

The preview clock unit test covers the initial capture and minimum minute interval.
Current native media scenarios also check initial previews and reconnect recovery,
but do not observe two minute-separated uploads. They cannot establish full
end-to-end periodic preview cadence.
