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

## Worker ownership and teardown investigation

The original `0xc0000005` and identical clean retry remain in the record. The
failing process was the default publisher unit-test harness after its 60-pass
summary. No original fault-thread stack, matching WER event or dump was available.
Five alternating plain pairs and two debugger pairs against main `90a6f5d`
previously exited cleanly. A handled first-chance baseline access violation was
not the original fatal event and does not establish a shared fatal cause.

The follow-up used a text-only Windows debugger with fault-thread stack walking,
thread descriptions and a test-summary observation point. Separate owned,
synthetic handled/fatal fault fixtures verified that it forwards first-chance
exceptions, captures an unhandled second chance and records the fatal exit. Those
fixtures are diagnostic calibration, **not** a FreedomStream crash reproduction.
No process memory dump or environment dump was collected.

One default and one serial run of each unchanged tree again exited cleanly. At
the first debug event after the summary, PR20 still had room/Tokio/RTC workers
alive (93 total threads in its default run; 72 serial). Main also retained room
workers in its serial run (72 total threads). This demonstrates a shared fixture
ownership gap; it does not identify the cause of the original fatal exit.

The resulting repair retains the room and picker worker handles. Normal close
cancels the picker and any pending browser request and waits for all three
workers. On Windows, a zero-timeout thread-handle check includes TLS/native
cleanup; it does not block the UI on a destructor. The existing explicit Force
close after ten seconds remains available. Browser loading now owns its request,
cancellation and result on one current-thread Tokio runtime; closing never applies
a late invitation result. Unit fixtures drain and join their native workers
before finishing rather than abandoning them to process shutdown.

Five new regressions cover pending request cancellation, single result collection,
picker cleanup before close, browser request cleanup before close, and a Windows
TLS destructor held open behind a barrier while the caller stays responsive.
The live ignored HTTP test now exercises the managed Load worker too.

Post-repair, one default and one serial run of the repaired candidate and rebuilt
main baseline used the same runtime, short Cargo cache, isolated profile and
GStreamer registry. All four exited zero without an access violation. Neither
candidate run retained named room, picker, invitation, Tokio or RTC workers at the
summary observation; the serial baseline retained one room and 16 Tokio workers.
Other OS/library threads remained (8/13 total on the candidate), so this is not a
claim of an empty process. Debugging changes timing and the summary observation
occurs at the next debug event, not atomically with the print.

Validation on the repaired tree passed Node 24.11.1 project checks (43 server,
23 web, four configuration tests), native formatting, workspace/all-targets/
all-features Clippy, debug build, 10 portable and 65 publisher tests (three ignored),
and all six desktop/mobile headless HTTP browser workflows. A rebuilt local debug
bundle passed real per-user enable/update, moved-download Windows dispatch,
publisher startup, disable and cleanup. The managed Rust Load passed real local
HTTP redemption and replay rejection. This remains a local qualification bundle,
not a new release. The benchmark import, `with_browser_link(None)` and explicit
profile-free planar constructor remain intact. No audible test or GPU/planar
qualification ran; PR21, root edits, perf checkout and shared SDK were untouched.

The sanitized [investigation record](browser-links-teardown.json) includes binary
hashes, original log hashes, bounded runs, calibration and limitations. The local
full record is `.local/teardown-investigation/report.txt`. The demonstrated
ownership defects are repaired; **the original fatal exit remains unexplained**.
There is no evidence-backed claim that the repair fixes that event. Keep its
merge blocker open rather than inferring clearance from successful repetitions.

## Native interaction recovery and centered confirmation

The earlier missing-pipe diagnosis is historical. After enabling Computer Use
and restarting the host/session, the supported API recovered. Actual native
interactions on the final local modal build passed:

- The centered server-trust screen appears before any admission. Load is explicit.
- Expired handoffs show rejection in the modal without joining.
- A fresh Load opens the nickname/Join step; entering a nickname does not join.
- Explicit Join connects to the real local SFU: room name and participant appear,
  with zero published streams and zero subscriptions. No audible test ran.
- After the successful join saved an encrypted profile, a second browser launch
  using that same profile opened a separate trust modal with zero participants,
  suppressing saved-room autojoin. The original joined window remained connected
  with its participant unchanged. This tests connection/window isolation with an
  empty room, not preservation of a running media stream.

The approved presentation uses an inactive, darkened background with a fixed
3�3 defocus kernel on onboarding paint only. It captures no framebuffer, retains
no background image, and never replays native paint callbacks. The modal contains
server trust/Load followed by server, nickname and explicit Join; the long
invitation field is omitted. Dismiss cancels pending loading and discards the
invitation. The first screen's dismiss button is centered under Load; its
nickname-screen position is unchanged. Actual native screenshots verified the
centered first screen and the final nickname layout. Unit layout/Enter regressions
cover 1280�720 and 960�540; dismissal cannot join. The profile-free planar fixture
initializes the new modal flag false and still loads no saved profile.

Node 24 checks, native workspace/all-targets/all-features Clippy, native build,
10 portable and 67 publisher tests passed (three environment tests ignored).
These local runs exited cleanly; they do not diagnose the original fatal exit.

The remaining actual browser external-app permission/dispatch check stopped with:
`browser URL policy enforcement is not yet supported for the current Windows browser`.
No raw input, alternate driver or security-policy bypass was attempted after that
stop. This is a browser tooling restriction, not a missing native helper. The
browser prompt gate remains open until exercised through a supported connection
or a precisely recorded manual interaction. The historical `0xc0000005` also
remains unexplained and unwaived. Keep PR20 draft; neither merge nor the user's
conditional post-merge shutdown may occur while either gate remains open.
