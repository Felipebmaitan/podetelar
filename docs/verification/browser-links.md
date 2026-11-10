# Portable browser invitation verification

## Behavior

- Room options offers optional per-user Enable/Disable browser links and an
  Update browser links to this version action. Default startup changes no URL
  associations. Other applications' registrations are refused.
- The complete launcher is cached by EXE content hash in Local AppData. The
  registration keeps working if the original download moves. Disabling removes
  only our registration; ordinary cached launchers remain for running processes.
- Invitation pages prepare one-use, 60-second handoff tickets. Permanent
  invitations never enter activation arguments or HTTP request URLs. The server
  stores at most 128 tickets in memory, periodically sweeps expired entries,
  rechecks the current invitation at redemption, and clears tickets at shutdown.
- Browser launches display the server and require Load invitation, followed by
  explicit Join. Saved-room autojoin is suppressed for browser launches. Capture
  and subscriptions remain explicit. Existing app windows keep their room.

## Original verification on 6191237

- Node 24.22.2 project check: formatting, TypeScript, 42 API/server tests,
  23 web unit tests, 4 configuration tests and production builds passed.
- Six headless Chromium workflows passed across desktop/mobile, covering admin
  regression and invitation preparation, no secret-bearing HTTP URLs/storage,
  browser redemption denial, native-style redemption/replay and expired-link UX.
- Native formatting/Clippy passed. Native tests passed: 8 portable and 35 publisher
  tests, including real isolated per-user registry operations and strict URL parsing.
- Explicit live native HTTP test passed: the Rust client redeemed a ticket from
  the real built local API and rejected replay. The test is ignored by default
  because it requires a local live fixture; it was explicitly enabled here.
- A local qualification bundle used the newly built debug launcher/publisher and
  the runtime/notices from an existing qualified portable payload. It is not a
  distributable release. `scripts/qualify-browser-links.py` passed real per-user
  enable, update to changed EXE bytes, dispatch after moving the original download,
  native process startup, no permanent invitation in process arguments, disable,
  and cleanup of its own temporary processes/files.
- Independent GPT-6.1-Sol high-reasoning review identified old-release registration
  and partial-registration retry issues. Added the explicit update action and
  rollback of a new registration on failure. The Windows integration check
  verified the update path. Actual qualification exposed a long cache-path issue;
  removing the redundant payload directory fixed it.

## Remaining acceptance

- The Computer Use native helper pipe was unavailable after the documented retry
  and session reset. No rendered native GUI or Load/Join click is claimed verified.
  The Rust HTTP exchange and Windows process startup results do not replace that
  manual interaction check.
- Browser tests inspect the launch link; browser-specific external-app permission
  prompts were not exercised. Chrome/Edge prompt acceptance remains to qualify.
- No production deployment, signed/release artifact or fresh-machine installation
  result is claimed. Existing physical hardware/media gates remain separate.

## Reproduce the Windows handoff check

Use a local HTTP server, a valid local test invitation, and a built portable EXE.
Supply JSON containing `invitationUrl` to the script through stdin; never put the
invitation in shell arguments. Run Python 3 with
`scripts/qualify-browser-links.py --executable <absolute portable EXE path>`.
The check refuses to overwrite an existing `freedomstream` handler. It uses an
isolated Local AppData directory, tests registration/update/dispatch, and removes
its registration and test files when done. It does not exercise GUI buttons.

For the live Rust redemption test, create a local JSON fixture with `link` and
`invitation`, set `FREEDOMSTREAM_APP_LINK_FIXTURE` to its path, and explicitly run
`browser_links::tests::redeem_real_server_invitation` with `--ignored`. Remove the
local fixture after use. Fixtures containing authority are never committed.

## Integration with redesigned UI and performance work

The branch now merges `origin/main` at `feefc2f` (PRs #22 and #19).
The browser-link actions use the new styled room-options popup. Both translation
catalogues are retained. The native test renderer's constructor now initializes
handoff state, and the performance-experiment entry point calls the updated
constructor with no browser activation. Startup still selects `Route::Auto` and
retains the source picker, graceful shutdown and existing picture/render paths.
These source changes do not establish runtime acceptance by themselves.

On the renewed attempt, `@oai/sky` import succeeded but `list_apps()` failed with:
`Computer Use native pipe is unavailable: failed to connect native pipe: The system cannot find the file specified. (os error 2)`.
The same failure persisted after the documented retry and JavaScript-session reset.
The supported unified CUA inventory independently returned no native apps and the
same missing-pipe error; it also reported `The browser access host is unavailable`.
T3 preview can open a blank browser tab, but this does not restore native app input.

The actual native Load/Join/server-trust interaction, saved-room autojoin suppression
and existing-window independence remain unverified, as does browser native
external-app permission handling. These are hard merge gates, not waived by a
headless browser, process launch, HTTP tests, synthetic UI rendering or compilation.

Fresh GPT-6.1-Sol high static review of runtime integration commit `d76dabe`
found no actionable findings and gave technical approval conditional on the
remaining acceptance checks. Integration-specific Node/native/browser checks
remain in progress. CPU-intensive builds and GPU/native launch checks are deferred during the
performance thread's explicitly coordinated PR #23 benchmark window. The read-only
GStreamer development SDK and performance checkout are not modified.
