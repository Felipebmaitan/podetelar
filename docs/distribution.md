# Windows distribution

Download **PodeTelar Setup** from [GitHub Releases](https://github.com/Felipebmaitan/podetelar/releases/latest).
It installs per user with shortcuts, an uninstall entry and `podetelar://`
room links. The installer includes the publisher, curated media runtime,
app-local Visual C++ libraries, notices and corresponding library sources.

Room links use `podetelar://`. Installing or launching the updated Windows app
removes the previous protocol registration when its ownership marker and launcher
command match this app. Other applications' handlers are preserved. Older clients
must be upgraded before using invitations from the renamed server.

Windows 11 and current NVIDIA/AMD drivers are recommended. Process-loopback audio
requires build 20348 or newer; publishing requires a supported hardware encoder.
The Windows preview is unsigned. Verify published checksums and the installer
manifest; hashes check integrity, not independent publisher authenticity.

## Invitations and launch behavior

Open an invitation from a friend or Discord `/stream` or `/telar`. Each link
selects its backend. The first room asks for a nickname; later links can reuse
that nickname while the process remains open and join when idle. Switching from
an active room requires confirmation. Links never start sharing or watching.

The last successful guest nickname is saved locally; pictures are associated with
the PC on each backend. See [guest profiles](guest-profiles.md). The Discord login
backend is remembered as a nonsecret preference; its revocable login uses OS
credential storage. Invitations, room session credentials and language choices
remain in memory. Startup
removes obsolete profile/preferences files left by previous versions. Installation
data and runtime caches are separate.
The installed launcher forwards later links to the existing app window.

Both launcher and publisher use the Windows GUI subsystem. Helpers run hidden.
Windows Firewall may request network permission, especially after updates change
the payload path. The app does not request elevation or modify firewall rules.
SmartScreen reputation warnings differ from named antivirus detections; record
the exact message and release hash when reporting a problem.

Built-in download/restart updates start with v0.1.2. Versions 0.1.0 and 0.1.1 need
one manual Setup upgrade. See [Windows updates](windows-updates.md).

## Build and qualify

Use Windows, Node 24, Rust 1.95, Visual Studio C++ build tools and `npm ci`:

```powershell
npm run native:setup
npm run native:check
npm run native:test
npm run native:package
$release = Get-Content .local/distribution/publisher.json -Raw | ConvertFrom-Json
./scripts/qualify-portable.ps1 -Executable (Join-Path .local/distribution $release.filename)
```

Release packaging requires a clean committed checkout. `--qualification` permits
a local artifact from an uncommitted checkout; never distribute that artifact.
Build the installer using the [installer instructions](windows-updates.md).

The portable EXE and `publisher.json` are internal installer inputs and
qualification tools. Public Windows releases include Setup, the full update
package, feed, installer manifest and checksums; do not publish the standalone EXE
or intermediate payload ZIP. Operators serving authenticated native downloads
can stage the EXE/manifest together in the server's read-only `PUBLISHER_DIRECTORY`.
The server checks size/hash at startup and permits only current room members.

## Runtime extraction

The portable launcher extracts to
`%LOCALAPPDATA%\PodeTelarPortable\<payload-sha256>`. It verifies the embedded
archive, bounds extraction, rejects unsafe paths/reparse points and serializes
concurrent first launches. Each release has a separate cache. Incomplete caches
are retained for inspection; move the affected folder aside to retry.

Media/DLL paths are isolated from development installations. A complete, compatible
installed Microsoft CRT is preferred; otherwise the bundled private CRT is used.
The publisher joins a cleanup job before it can create descendants, so terminating
the launcher also ends its publisher processes.

Completed caches are reused, preserving compatible LGPL library replacements.
**Licenses** shows the Inter license and gives access to the packaged notices and
library sources. The internal
`PodeTelar.exe --licenses` command does the same; `--extract C:\new\absolute-folder`
extracts to a previously nonexistent directory.

Installed room-link registration is managed by install/update/uninstall hooks.
Standalone operators can use `--enable-browser-links` or
`--disable-browser-links` on the portable EXE. Opting in uses a cached launcher
under `PodeTelarPortable\browser-links`. Permanent invitations are never passed
in activation arguments; one-use handoff tickets expire after a minute.

## Dependency licenses and signing

Application code is MIT; bundled libraries keep their own terms.
`deploy/portable-runtime.json` pins media files, hashes, source archives and
upstream revisions. Packaging checks plugin licenses and the PE import closure,
omitting GPL x264/x265/libav plugins. Windows and GPU driver DLLs remain vendor supplied.

Packages include matching library sources, Cerbero recipes/patches, license texts
and Rust/WebRTC notices. See [runtime scope](../deploy/windows/portable-runtime.md)
and [corresponding sources](../deploy/windows/portable-sources.md). Dynamic libraries
remain replaceable. Visual C++ release files use the toolchain's Distributable Code
terms; provenance is recorded in `Microsoft-CRT.json`. Toolchain differences mean
bit-for-bit reproducibility is not claimed.

The current portable reader expects a payload footer at EOF. Authenticode adds a
certificate table, so signing the final bundle requires a signing-aware reader
and renewed extraction/startup qualification. Sign inner executables before
archiving and generate final hashes after signing. See
[Microsoft's reputation guidance](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation).

## Qualification limits

Portable checks cover cache extraction/reuse, source hashes, runtime probes,
concurrent launches and damaged-payload rejection on the development Windows PC.
They do not establish fresh-PC/offline startup. Windows media checks use NVIDIA
hardware; AMD, physical 4K120, real-game/group load and physical speaker/display
synchronization require separate measurements. Installed media shutdown during
updates also needs actual application acceptance; updater fixtures alone cannot
establish it. See [testing](testing.md).
