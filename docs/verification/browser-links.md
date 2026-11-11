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
remaining acceptance checks. The subsequent all-features check caught a missing
`browser_links` module import in the separate `room-benchmark` example; that
feature-gated import was added and all-features Clippy passed afterward. This is
in addition to adapting the experiment constructor, and does not change benchmark
measurement or product rendering behavior.

After the performance thread explicitly released its benchmark window:

- Node **24.11.1** `npm run check` passed formatting, types, 42 API/server tests,
  23 web unit tests, 4 configuration tests and production builds.
- The built HTTP app passed all six desktop/mobile headless Chromium workflows
  in `e2e/admin.spec.ts` and `e2e/app-links.spec.ts`.
- Native formatting/default Clippy passed. Native tests passed **10 portable**
  and **59 publisher** tests, with three environment-dependent tests ignored by
  default. Workspace/all-targets/**all-features** Clippy also passed, covering the
  performance-experiment example entry point. Existing third-party LiveKit
  warnings remain outside the workspace lint cap; no workspace warning was waived.
- The live Rust HTTP handoff test was explicitly enabled and passed redemption
  and replay rejection against the real local API.
- A new local qualification bundle with the integrated debug launcher/publisher
  passed real per-user enable/update, moved-download Windows URL dispatch,
  native-process startup, no permanent invitation in activation arguments,
  disable and cleanup. It reused the earlier qualified runtime payload and is
  not a new distributable release.
- The decoder SDK was regenerated from this checkout's current pin under its own
  `.local/native-sdk`. The GStreamer development SDK supplied by the performance
  thread was read only and was not modified. A private short build cache plus
  `LK_CUSTOM_WEBRTC` pointing to its unchanged downloaded WebRTC artifact avoided
  MSVC's long include-path limit; no source patch was used to bypass the failure.
- Validation was silent: no audible, media publication or playback test was run.
  Root's unrelated edits and the performance checkout were not changed.

`origin/main` was refreshed after these checks and still pointed to `feefc2f`
(PRs #22 and #19). PR #23 was not yet merged. Before any future PR #20 merge,
refresh main again and integrate/recheck #23 if it has landed.

The required native Load/Join/server-trust, saved-autojoin/existing-window and
browser-native external-app permission interactions **remain unverified** because
the supported native helper is unavailable. The successful process/HTTP/headless
checks above do not satisfy those interaction gates. Keep PR #20 draft and do not
merge until they actually pass.

## Follow-up integration with PR #23

Merged `origin/main` at `90a6f5dde0ab67a84a3dff24701efa4231809b56` into
PR #20 (runtime merge commit `1574350`). The merge was clean. The benchmark example
still imports `browser_links` and constructs its ordinary benchmark UI through
`with_browser_link(None)`. The disabled planar qualifier retains its explicit
`PublisherUi` literal with `store: None` and `attempted_profile: None`; only
`pending_link: None`, `link_result: None` and `browser_links_enabled: false` were
added. No saved-profile load or automatic join was introduced into that qualifier.
PR #23's clipping, history cache, source picker, shutdown and redesigned UX remain
intact. Neither the performance checkout nor its read-only SDK was modified.

On this integrated tree:

- Node 24.11.1 `npm run check` passed: 43 API/server, 23 web and four configuration
  tests, formatting, types and production builds.
- Native formatting and workspace/all-targets/all-features Clippy passed, including
  the planar qualifier and standalone benchmark example.
- The default native test run reported all 10 portable and 60 publisher tests
  passing (three ignored), but the publisher test process then exited with
  `0xc0000005` (`STATUS_ACCESS_VIOLATION`) during teardown. An identical default
  rerun exited cleanly with all tests passing. The first failure is not considered
  diagnosed or remediated; retain it as a native teardown reliability follow-up
  before merge, rather than treating assertion counts as process success.
- All six desktop/mobile headless HTTP browser workflows passed again. No audible
  test or real planar/GPU qualifier was run.

The previously documented native-helper and actual GUI/browser prompt acceptance
blockers are unchanged and unwaived. PR #20 remains draft and must not merge until
those interactions actually pass. The earlier conditional technical review is not
an approval to waive this follow-up integration's remaining acceptance.
