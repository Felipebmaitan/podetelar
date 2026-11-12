# Installed Windows builds and update rollout

The installer foundation is available for **unsigned local/CI previews**. It does
not check for updates or publish a feed yet. The portable EXE remains available.
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
its cleanup job remains intact. The outer bootstrap is outside that job, providing
a place for a future updater handoff. It does not itself implement that handoff.
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

## Next implementation: update badge and delivery

1. Add an outer-launcher update controller with bounded authenticated local IPC
   to the publisher UI. Never invoke the updater from inside the portable job.
   Bind messages to this launch, avoid invitation/token-bearing restart arguments,
   and test concurrent launches and publisher crashes.
2. Check in the background on startup and every 30 minutes with jitter/backoff;
   add a manual check. Stable uses stable releases only; nightly is opt-in.
   No automatic download or install on quit. Network failure leaves the app usable.
3. Show an update icon in both onboarding and room headers. Hover/focus reveals
   version and a short bounded plain-text changelog; clicking downloads. Show
   progress, retry and cancel. After verification offer **Restart to update** and
   **Later**. Warn that restart ends sharing/watching; gracefully leave and stop
   capture/playback before handoff. Reopen using DPAPI settings without restarting
   capture or subscriptions. Never interpret release-note HTML or execute links.
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
  DPAPI room continuity, no resumed capture/subscriptions, and no visible console.
- Interrupt downloads; corrupt packages; test missing/auth-denied feeds, multiple
  instances, channel isolation, full fallback and failed application recovery.
- Verify signed artifacts/tamper rejection and inspect logs for session secrets.
- Run actual Windows installation/update/uninstall on a fresh machine; report
  firewall/SmartScreen observations separately from build checks.

These are pending acceptance gates. The foundation must not be advertised as a
working automatic updater. Merge only after appropriate checks and a fresh
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
