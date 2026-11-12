# Installed Windows builds and update rollout

The installed app supports an update badge, hover changelog, explicit download,
progress/retry and Restart to update/Later. It is available for **unsigned local/CI
qualification**; no public feed is configured or published. Standalone portable
downloads have no automatic updater and remain available separately.
See [upstream research](research/windows-updates.md) for T3 Code's implementation,
Velopack licensing, runtime constraints and source citations.

## Build a preview

On Windows with Node 24, Rust 1.95, Visual Studio C++ and .NET SDK 8 or newer:

```powershell
npm ci
npm run native:setup
npm run native:package
$env:INSTALLER_BUILD_NUMBER = '1'
node scripts/package-installer.mjs
```

Use a clean committed checkout. Increase the preview number for each build.
The packager verifies the portable artifact's commit, size and SHA-256 and uses
the same pinned Velopack SDK/CLI. Outputs are under `.local/installers/`.
The manual **Windows installer preview** workflow builds the native payload,
runs checks and portable qualification, then uploads installer/feed artifacts.
Artifacts are for maintainers; they expire and are not a user update source.
No PAT is embedded and the workflow has no release-write permission.

Velopack installs per user, provides shortcuts and an uninstall entry. A small
GUI bootstrap handles Velopack startup before launching the portable application.
The existing portable application still extracts its licensed runtime and sources;
its cleanup job remains intact. The outer bootstrap is outside that job and owns
checking, downloading and handoff to Update.exe after the portable child exits.
The separate bootstrap Cargo workspace avoids coupling installer startup to media
SDK initialization. Neither .NET nor a shared CRT should be needed by users;
offline/fresh-machine verification remains required before advertising that result.

`deploy/windows/installer.json` separates display title from the permanent package
ID and executable identity. Preserve `FreedomStream` as the package ID after the
planned rebrand, or explicitly implement migration before the first public install.
Changing the display title may also change shortcuts. Saved rooms continue to use
the existing per-user DPAPI settings. Uninstall must preserve those settings unless
the user explicitly requests erasure; document cache cleanup separately.

## Installed room links

Installing registers `freedomstream://` for the current Windows user. The command
targets the fixed `current/freedomstream-installed.exe` with a quoted
`--browser-link "%1"` argument; it does not invoke Update.exe with the ticket.
Velopack install/update hooks maintain registration and uninstall removes only
the matching installation's command. Explicit installation can migrate the old
portable handler. Unrelated handlers are preserved. Standalone portable downloads
retain their opt-in link registration; installed users no longer enable or refresh
a content-addressed launcher copy from Room options.

The installed launcher serializes instances. Later links and ordinary launches
send an activation/focus request to the existing instance, using a random token
and loopback endpoint discovered in the current user's registry. Only local
broker credentials and executable paths enter that registry; handoff tickets stay
in bounded memory queues. Both transport and native URL validation are retained.
The publisher polls on a worker and requests restore/focus of its existing window.

The website keeps its explicit Join action and one-use short-lived handoff. A
previously used server origin loads directly into native Join confirmation. An
unfamiliar origin still asks for trust before contacting it. Joining another room
warns that sharing/watching will end, and does not happen automatically. Dismissing
the invitation restores the active room invitation; a pending invitation cannot
replace the last successful saved room. No capture or subscription starts from a
link. Browser external-app prompts remain controlled by the browser/Windows.

`node scripts/qualify-installed-links.mjs` builds a test-only launcher/publisher
substitute with isolated registry keys. It verifies real install/update/uninstall
hook registration, quoting paths with spaces, focus/link forwarding and one
publisher process. Local tests also exercise migration, foreign-owner preservation,
server-origin trust and dismissal. This establishes registry/process behavior,
not a browser prompt or native foreground-window observation, fresh-machine
installation or a production media-session switch.

## Badge, download and restart controls

Set `FREEDOMSTREAM_UPDATE_FEED` **when building the installer bootstrap**, to a
trusted HTTPS feed directory, public GitHub repository URL, or absolute local
feed directory for qualification. It is compiled into the launcher; room data,
IPC requests and runtime environment cannot select an arbitrary update source.
The workflow reads repository variable `WINDOWS_UPDATE_FEED`; leaving it empty
keeps updates disabled. No public visibility is needed to use a local directory.

The publisher receives a per-launch loopback endpoint and random 256-bit token
through its environment. Length-prefixed JSON is limited to 16 KiB, connections
have short timeouts and the command queue is bounded. No room invitation/session
authority crosses that connection. Normal activation arguments are excluded from
Velopack startup logging and restart arguments are empty. Installed launchers
serialize instances with a Windows file handle outside the replaced `current`
directory. The publisher GUI only polls in a worker, never on its render thread.

Onboarding and room headers show update status. Hover reveals bounded plain-text
notes and the version; Download starts a worker transfer and reports percentage.
Ready opens a restart confirmation explaining that sharing/watching will end.
Later keeps the downloaded package without applying it on ordinary exit/startup.
Only an acknowledged Restart triggers the existing picker/admission/media shutdown
path; Update.exe is launched outside the cleanup job after the portable child exits.
The saved-room behavior is unchanged, and no media restarts automatically.

Checks run on startup and at roughly 30-minute intervals with jitter while current
or retryable. Downgrades, cross-application packages, unsafe filenames, missing
SHA-256 and packages over 1 GiB are refused. Stable versions refuse prereleases.
Downloads explicitly discard SDK delta strategies and use full packages. The
launcher checks SHA-256/size itself before Ready and again before applying; invalid
cached packages are removed and downloaded again on an explicit download action.
This does not establish independent publisher authenticity. In-flight cancellation,
opt-in nightly switching and delta optimization remain follow-up work.

## Remaining delivery work

1. Qualify the actual publisher/portable media shutdown during an installed update;
   test concurrent launches, stale connections and publisher crashes. Add a native
   visual/interaction acceptance pass for the badge and restart dialog.
2. Add transfer cancellation and qualify timeout/backoff behavior for each provider.
3. Qualify saved-room continuity, offline startup and uninstall on fresh Windows.
4. Publish public GitHub Release assets after the rename/repository transition.
   No need to make source public to implement/test: a local feed or separate
   public binary repository also works. Never ship a private GitHub token.
   Publish packages before discoverable feed metadata; use full packages first,
   then add measured delta updates with full fallback. Keep previous releases.
5. Add stable tags and scheduled nightly CI only after update acceptance.
   Nightlies require new commits and a minimum publication interval (initially
   six hours). Use increasing SemVer prereleases, separate stable/nightly feeds,
   explicit promotion and no accidental downgrade or preview offers.
6. Integrate Authenticode signing and independently verify installer, helper and
   application signatures. The existing portable EOF footer must be made signing
   aware or replaced by a directly installed runtime before signing its payload.
   Feed hashes alone are not independent publisher authentication. Qualify all
   added helper/dependency licenses before public distribution.

## Acceptance before shipping

- Install two actual versions; check notes, download, Later and restart; verify
  the updater survives publisher/portable exit and cleans up all media processes.
- Verify offline launch, shortcut and uninstall, existing portable coexistence,
  DPAPI server/name continuity, no automatic room rejoin or resumed capture/subscriptions, and no visible console.
- Interrupt downloads; corrupt packages; test missing/auth-denied feeds, multiple
  instances, channel isolation, full fallback and failed application recovery.
- Verify signed artifacts/tamper rejection and inspect logs for session secrets.
- Run actual Windows installation/update/uninstall on a fresh machine; report
  firewall/SmartScreen observations separately from build checks.

These are pending acceptance gates. Local implementation and fixture results must
not be advertised as production/fresh-machine updater qualification. Merge only after appropriate checks and a fresh
GPT-6.1-Sol high-reasoning review; external verification stays explicitly open.

Follow-up tracking: [automatic update delivery, badge and release rollout #29](https://github.com/Felipebmaitan/freedomstream/issues/29).

## Local foundation verification

On 2026-11-11, Node 24.11.1 `npm run check`, the installer integrity-input test,
bootstrap Rust formatting/Clippy and Windows x64 release compilation passed.
The notice collector successfully collected the pinned bootstrap dependency texts.
Velopack 1.2.161 produced a real Setup EXE, full update package and preview feed
in a separate local smoke directory using an existing portable release plus the
new bootstrap. This checks package assembly only: the old portable payload is
not a build of this branch and those files must not be distributed. No actual
installation, two-version update, signing or fresh-machine acceptance is claimed.

## Update-controls qualification

`node scripts/qualify-update-controls.mjs` builds two actual Velopack packages
against a private local file feed. It uses a small publisher substitute to drive
the real launcher/transport, reject a bad token and a premature restart, download
the package, choose Later and verify version one stays unchanged, then explicitly
restart through the real Update.exe and observe version two relaunch. Evidence is
kept under `.local/update-controls-*`. The substitute is behind a Cargo feature
and is never staged by the production installer packager. The workflow runs this
test on Windows. It also offers an unsafe delta alongside a real base package and
verifies that delta is ignored, then corrupts the downloaded cache after Later and
requires the next download to repair it. This verifies helper lifetime and update application with a
fixture process; it does not establish actual media cleanup, fresh installation,
signed provenance or saved-profile continuity. Those remain separate gates above.

Local 2026-11-11 results: Node 24 `npm run check` passed; launcher Clippy and
protocol tests passed; the real two-package fixture qualified download, Later and
Update.exe apply/restart. The native suite passed 70 tests (three live-environment
tests skipped), including pointer interactions for Download/Restart/Later and
existing shutdown checks. These automated egui interactions are not a rendered
native-window visual acceptance pass or a production media-update run.
