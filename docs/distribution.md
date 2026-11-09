# Windows distribution

Download the **portable Windows app** from an admitted room and open the EXE.
Paste the private room invitation, enter your name and join. No installation,
PowerShell, elevation, shared runtime installation or first-launch dependency
transfer is required. The executable contains the publisher, its curated media
runtime, app-local Visual C++ DLLs, notices and corresponding library sources.
Windows 11 and a current NVIDIA/AMD driver are recommended. Process-loopback audio
requires Windows build 20348 or newer; publishing requires a hardware encoder.
Chrome is the qualified receiver here; check HEVC support on each viewer.

## Portable behavior

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
Do not remove shared Microsoft/GPU components to uninstall FreedomStream.

The executable is currently unsigned. Verify its SHA-256 against the authenticated
room's download details. This checks integrity against that trusted manifest,
not an independent publisher signature. Code signing remains future work.

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

Original FreedomStream code is MIT. Bundled libraries retain their own terms.
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
