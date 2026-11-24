# Test scope

Participants join, publish and watch in the native apps. Browser automation covers
desktop operator administration. Participant browser-room and mobile media
scenarios are outside the current product test scope. See [macOS checks](macos.md#development-and-checks) for its
platform-specific capture, packaging and updater qualification commands.

## Automated checks

Use Node 24 LTS and `npm ci`.

| Command                          | Coverage                                                                                                                                                                                   |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `npm run check`                  | Formatting, workspace/tool types, API and configuration tests, administrator usage formatting tests, and production builds.                                                                |
| `npm run test:admin`             | Desktop administrator login/logout, usage filters, room creation/directory, and per-room access-management Lock. Requires Chromium and a running app. `test:e2e` is a compatibility alias. |
| `npm run smoke -- --restart-app` | Actual HTTP room API admission, authorization, persisted session recovery, leave, built web assets and authenticated SFU access against Docker.                                            |
| `npm run native:check`           | Native formatting and Clippy. Requires the platform media runtime and SDK.                                                                                                                 |
| `npm run native:test`            | Native and portable Rust tests: capture cancellation, cleanup, audio policy, preview cadence, quality/network controls, activation and packaging/process contracts.                        |
| `npm run native:room-smoke`      | Actual native decode, codec fallback, watch/stop/rewatch traffic, preview/reconnect, receiver allowance and audio against an isolated Windows API/SFU.                                     |

Install Chromium with `npx playwright install chromium` before running administrator
tests. Native acceptance uses Playwright's API client and test runner to launch
native executables; it requires no browser installation. `npm run native:e2e` runs
that same suite against an already configured server and native executable.

CI runs Node checks on Windows and Linux and the Docker smoke plus desktop
administrator UI on Linux. Hosted native build/packaging qualification remains an
explicit `Checks` workflow dispatch with `run_native` enabled. A green default CI
run does not mean native hardware/media qualification ran.

For an isolated concurrent native smoke run, set `PODETELAR_SMOKE_APP_PORT`
and `PODETELAR_SMOKE_SFU_PORT` to unused ports. The SFU also uses the next
two ports. Defaults remain 3010 and 17880–17882. Do not stop another test run's
services to free these ports.

On Windows, set `PODETELAR_TEST_CAMERA` to an explicit device display name and run
`npm run native:room-smoke -- camera.spec.ts`. Select a camera whose output is safe
to transmit to the isolated local SFU. The suite checks video-only publication,
automatic webcam playback, concurrent sharing with a generated window, and
independent stop and restart. Set `PODETELAR_TEST_CAMERA_CODEC` to `av1`, `h265` or
`h264` to exercise a chosen camera codec in the concurrent test.

Run `npm run native:e2e -- stream-policy.spec.ts` with
`PODETELAR_TEST_STREAM_POLICY=1` and `PODETELAR_NATIVE_EXE` pointing at the
built publisher. Use an isolated local API and policy-enabled SFU because the
test changes the server's stream limits. It exercises real encoding and
decoding across live bitrate, resolution and FPS changes. With a selected 1080p
camera, run `npm run native:e2e -- camera.spec.ts` against the same isolated
policy-enabled SFU to exercise those changes during concurrent sharing.
The SFU image also runs the packet-budget and observed-resolution unit tests
during its build.
With the same local configuration, `npm run test:admin -- stream-policy.spec.ts`
checks dashboard saves and enforcement against a publisher that ignores the policy.

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
  date ranges.
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
physical 4K120, synchronized A/V latency or multi-friend load. Record actual hardware
and deployment evidence, with unresolved limits, in the PR.

The preview clock unit test covers the initial capture and minimum minute interval.
Current native media scenarios also check initial previews and reconnect recovery,
but do not observe two minute-separated uploads. They cannot establish full
end-to-end periodic preview cadence.
