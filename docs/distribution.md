# Windows distribution

The v0.1.0 GitHub release provides **PodeTelar-v0.1.0-Setup.exe**, an unsigned
per-user installer preview with shortcuts, an uninstall entry and managed room
links. Public releases offer only the installer, its manifest and checksums;
do not upload a standalone EXE or a portable release manifest. Keep the portable
payload as an internal installer build input and maintainer qualification tool.
Existing standalone users can migrate by running Setup. Automatic
update delivery is not enabled in this first installer preview. See
[installer and update rollout](windows-updates.md) and its
[actual lifecycle qualification](verification/global-bot-installer.md).

Download **PodeTelar Setup** from the GitHub release and run it for a per-user
installation. Then open a room invitation, enter your name and join. The installer
contains the publisher, its curated media
runtime, app-local Visual C++ DLLs, notices and corresponding library sources.
Windows 11 and a current NVIDIA/AMD driver are recommended. Process-loopback audio
requires Windows build 20348 or newer; publishing requires a hardware encoder.
Sharing and watching use the native app. No browser is embedded or launched.

## Invitation-selected server and launch behavior

The app opens without a default server, nickname or restored room. Use `/stream`
or `/telar` in Discord and open the resulting invitation, or receive a friend's
link. Every link selects its own backend, including independently hosted bots.
Browser links load automatically; the first room asks for your nickname. While
the app stays open, later links reuse that nickname and join automatically when
idle. Switching from an active room asks before ending its sharing and watching.
Sharing and watching still require explicit actions.

Server, nickname, invitations, session cookies, media tokens and language choices
are kept only in memory. Startup removes obsolete `settings.json` and
`preferences.json` files from `%LOCALAPPDATA%\FreedomStream` without loading them.
Runtime extraction, installation and optional browser-link registration still use
local files. Older app releases can save settings again; use the rebuilt client.

Both launcher and native app use the Windows GUI subsystem and launch without a
terminal. Development helper subprocesses are hidden too. Windows Firewall can
still request network permission when no applicable rule exists. Development
executables and the local test SFU previously changed paths between tests; the
test SFU now uses a stable versioned path. Reopening the same cached portable build
should reuse an applicable rule, but updates extract to a new payload path and can
prompt again, as can different network profiles or administrator policy. The app
does not create firewall rules, request elevation or disable firewall protection.
See [Microsoft's firewall rule behavior](https://learn.microsoft.com/en-us/windows/security/operating-system-security/network-security/windows-firewall/rules).

## Internal portable payload and legacy standalone builds

First launch extracts into `%LOCALAPPDATA%\FreedomStreamPortable\<payload-sha256>`.
Concurrent first launches serialize extraction; a cache appears only after complete
extraction. The launcher verifies the embedded ZIP's SHA-256, bounds its size and
rejects unsafe/duplicate Windows paths and symbolic links. Runtime paths containing
reparse points are refused. It isolates media/DLL search paths from development
installations and inherited GStreamer settings. It prefers a complete installed
Microsoft CRT trio at least as new as the bundled one, otherwise selects the private
bundled CRT directory. This avoids shadowing newer Windows audio/COM components
with older DLLs. The package remains usable without a shared CRT installation.
An incomplete cache is retained
for inspection instead of overwritten. Move that version folder aside to retry.

The publisher joins its cleanup job during Windows process creation, before it
can run or create descendants. Closing or forcibly terminating the launcher
therefore ends its publisher and owned child processes, including during startup.

Completed caches are reused without overwriting user changes. In particular,
compatible LGPL DLL replacements remain usable. Each release gets a separate
cache. The app's **Licenses and library sources** button opens its notices folder.
`FreedomStream.exe --licenses` does the same; `--extract C:\new\absolute-folder`
extracts a separate copy to a previously nonexistent directory. Normal launch
needs neither command. The extracted publisher remains dynamically linked.

Keep the EXE wherever you prefer. To remove it, close the publisher and delete the
EXE and its own `FreedomStreamPortable` cache. It creates no Start-menu shortcut,
registry entry or system PATH change. The earlier script-based installation is
separate: retain it for rollback or remove it with its original `Uninstall.ps1`.
Do not remove shared Microsoft/GPU components to uninstall PodeTelar.

The executable is currently unsigned. Verify its SHA-256 against the authenticated
room's download details. This checks integrity against that trusted manifest,
not an independent publisher signature. Code signing remains future work.

## Windows security warnings and app icon

The launcher, native publisher and installer use the supplied PodeTelar artwork
from `assets/PodeTelar.png`, with 16–256 pixel Windows resources and PodeTelar
product/version metadata.
The native window also sets that icon explicitly for its title bar and taskbar.
Regenerate the resized icons with `python scripts/make-windows-icon.py` (Pillow required
only for regeneration). Portable qualification checks both executables for an
embedded icon and product metadata.

PodeTelar is the display name. Existing executable filenames, `freedomstream://`
links, update package IDs and per-user storage paths retain their identifiers for
compatibility with installed versions and saved settings.

An icon and version metadata do not establish publisher trust. The current release
is unsigned. “Windows protected your PC” or “Unknown publisher” usually indicates
SmartScreen reputation; a named threat in Defender Protection history is a separate
antivirus detection. Record the exact message, detection name and affected release
SHA-256 before diagnosing a friend's report. Do not require antivirus exclusions.

For direct downloads, use a publicly trusted Authenticode signing identity, SHA-256
and trusted timestamping, and keep a consistent publisher identity. This requires
identity validation and access to a certificate or managed signing account; a
self-signed certificate does not solve public distribution trust. Even valid OV/EV
signatures do not guarantee immediate SmartScreen reputation. Microsoft recommends
Artifact Signing for non-Store distribution; Microsoft Store distribution is another
route if its packaging and eligibility requirements are met. See
[Microsoft's reputation guidance](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation).

Signing integration must account for the portable format: the current reader expects
its payload footer at EOF, whereas Authenticode adds a PE certificate table. Simply
running SignTool on today's final bundle is not a supported release procedure.
Before shipping signed releases, make payload discovery certificate-table aware,
sign the internal publisher before hashing/archiving, sign the completed bundle,
then verify signatures and run portable extraction/startup checks on the signed
artifact. Generate the final download hash and size after signing. Qualify bundled
executable libraries under Smart App Control as well. No signing credentials or
verified signed-release result are included in this change.

If Defender identifies a named threat and investigation establishes a false positive,
submit the exact affected binary through
[Microsoft's software developer submission portal](https://www.microsoft.com/en-us/wdsi/filesubmission).
This is separate from building SmartScreen reputation. Firewall network-access
prompts are also separate and follow the behavior described above.

## Build and qualify

Use Windows, Node 24, Rust 1.95, a licensed Visual Studio C++ toolchain, and `npm ci`:

```powershell
npm run native:setup
npm run native:check
npm run native:test
npm run native:package
$release = Get-Content .local/distribution/publisher.json -Raw | ConvertFrom-Json
./scripts/qualify-portable.ps1 -Executable (Join-Path .local/distribution $release.filename)
```

Packaging requires a clean committed checkout. `--qualification` permits an
explicitly local package from an uncommitted checkout; never distribute it.
The builder uses the pinned SDK/runtime and allowlisted media DLL hashes, gathers
Cargo/WebRTC license texts, downloads checksum-pinned corresponding sources,
checks every shipped PE's imports and each selected plugin's license, then builds
a launcher with a static CRT. All source downloads happen on the **build machine**.
The release CRT comes unmodified from that machine's Visual Studio `VC/Redist`
release directory; its version/hashes are recorded in the notices. Toolchain/CRT
versions can differ between builders. Bit-for-bit reproducibility is not claimed.

The single EXE and `publisher.json` are the distribution artifacts. The intermediate
payload ZIP and expanded tree are build evidence. Copy the EXE/manifest together
into the server's read-only `PUBLISHER_DIRECTORY` and restart the app. The server
verifies the executable at startup and serves it only to current room members.
Deploy the new server and EXE together; this server expects an EXE manifest.
A previous open tab's ZIP endpoint redirects admitted members to the EXE.

## Dependency licenses

Original application code is MIT. Bundled libraries retain their own terms.
`deploy/portable-runtime.json` lists the exact media files and hashes, source URLs,
source hashes and upstream revisions. The curated runtime omits GPL x264/x265/libav
plugins and unrelated network/framework plugins. Each selected plugin's LGPL
metadata and PE import closure are checked during packaging. GPU driver and Windows
system DLLs remain supplied by their vendors, rather than copied from the host.

The EXE includes complete matching GStreamer/GLib/proxy-libintl sources, other
library/header sources, Cerbero recipes/patches/configurations, license texts and
copyright notices. See [runtime scope](../deploy/windows/portable-runtime.md) and
[corresponding-source details](../deploy/windows/portable-sources.md). These materials
accompany the binaries offline; source availability does not depend on the private
GitHub repository. Cached shared libraries remain replaceable. Follow the original
library license when redistributing modified copies.

Unmodified app-local release CRT files are distributed under the installed Visual
Studio toolchain's Distributable Code terms, separately from MIT/LGPL. Their
provenance and links to Microsoft's terms are recorded in `Microsoft-CRT.json`.
See [Microsoft redistribution](https://learn.microsoft.com/en-us/visualstudio/releases/2022/redistribution#visual-c-runtime-files)
and [local deployment](https://learn.microsoft.com/en-us/cpp/windows/redistributing-visual-cpp-files).
The [GStreamer licensing advisory](https://gstreamer.freedesktop.org/documentation/application-development/appendix/licensing.html)
explains its plugin license boundary. Hardware codec support is not a codec patent license.

Cargo notices include the pinned upstream license overrides in
`deploy/license-overrides.json` and the Apache-licensed LiveKit stereo patch.

## Qualification limits

The automated script checks empty-cache startup from a fresh directory with
spaces and an isolated PATH, concurrent extraction, the actual runtime probe,
extracted-file/source hashes, reuse, explicit extraction and damaged-payload
rejection, reparse-point refusal and a forced bundled-CRT startup probe. It retains
evidence under `.local/portable-qualification-*`.
This host is not a fresh Windows PC or a disconnected network environment.
Native hardware capture, audio and browser delivery need actual runtime checks;
AMD, physical 4K120, real-game/fullscreen behavior, macOS/Linux and a full evening
with four/five friends remain separate gates. See [Phase 8 qualification](phase8-qualification.md).

## Optional browser invitation links

Run the portable EXE once, then choose **Room options → Enable browser links**.
This registers `freedomstream://` for your Windows user only. No administrator
privileges or installer are needed. A complete launcher copy is retained under
`%LOCALAPPDATA%\FreedomStreamPortable\browser-links`; moving the downloaded EXE
afterward does not break the registration. Choose **Update browser links to this version** from a newer release
to update the registered launcher. **Disable browser links** removes our URL
registration; cached launchers stay on disk because a running app may use them.
Other applications' registrations are never silently replaced or removed.

On an invitation page, choose **Join room in PodeTelar**. The page prepares
a temporary ticket in advance, so this single click launches the app directly. Allow the browser's app-launch prompt if shown. In the app,
verify the server, choose **Load invitation**, enter your name, then **Join room**.
Loading alone starts no room session, capture or subscriptions. If PodeTelar
is already running, the link opens another window without changing that room.

Handoffs expire after one minute and work only once. If the app is missing, the
browser refuses the launch, or the ticket expires, run the EXE/enable links and
retry the join action. Manual copy/paste still works. Both browser invitations and ordinary EXE launches require explicit room entry;
ordinary launches restore only the server and name. Short-lived handoff tickets can appear in OS activation
arguments; permanent invitations and media/session credentials do not.

Windows canonicalizes activation URLs by adding a root slash after `join`. The
app accepts both forms while rejecting other paths. Older builds reject this
Windows form: download the corrected EXE and choose **Update browser links to
this version** once so the registered cached launcher also receives the fix.
