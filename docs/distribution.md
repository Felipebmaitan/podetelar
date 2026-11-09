# Windows distribution

The publisher is an unsigned x64 Windows application. Windows 11 is recommended;
process-loopback audio needs build 20348 or later. An NVIDIA/AMD hardware encoder
and a current GPU driver are required for publishing. Chrome is the qualified
browser receiver on this machine; HEVC support must be checked on each viewer.

## Build and install

Use Rust 1.95, Node 24 and `npm ci`, then:

```powershell
npm run native:setup
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/native.ps1 build
npm run native:package
```

Packaging requires a clean committed checkout, uses Cargo.lock and pinned SDK/runtime
checksums, and records the source commit. `--qualification` creates a separate local
test package from an uncommitted checkout; never distribute that package. These are
repeatable pinned-input builds; bit-for-bit PE/ZIP reproducibility is not claimed.
The release ZIP and `publisher.json` are written to `.local/distribution`. Preserve
both, compare the SHA-256, and mount their directory read-only at the server's
`PUBLISHER_DIRECTORY`. Authenticated room members see the download; the private
GitHub repository does not need to become public. The server verifies the archive
at startup. Replace releases with the app stopped/restarted, never mutate its
archive while it is serving downloads.

Extract the ZIP and right-click **Install.ps1 → Run with PowerShell**. The installer
verifies package hashes, downloads the pinned official GStreamer runtime directly
from its upstream HTTPS URL, verifies its checksum, and extracts it in upstream
`/portable=1 /CURRENTUSER /TYPE=runtime` mode. This does not register GStreamer,
change system PATH, or replace another installation. The initial download is about
542 MB. Existing GStreamer development installations are independent.

The package is not Authenticode signed. Windows may show an unsigned-app/download
prompt. Verify the ZIP checksum against the authenticated room download details
before running it. A checksum provides integrity against that trusted release
manifest; it is not an independent publisher signature. Code signing remains a
distribution improvement, not a reason to disable Windows protection globally.

Installation defaults to `%LOCALAPPDATA%\FreedomStream`, with versioned runtime
directories and a Start-menu shortcut. Launch the publisher and paste a room
invitation in its UI. No invitation is put into shortcut arguments or saved in the
installation. A clean runtime probe must pass before `current.json` changes.
Upgrades retain the previous version. For rollback, close the publisher and set
`current.json` to the previous installed release ID (a JSON string).

If the x64 Microsoft Visual C++ runtime is missing, the installer downloads a
pinned Microsoft installer, verifies SHA-256 and its Microsoft Authenticode
signature, and launches its normal Windows UAC installation. Shared Microsoft/GPU
drivers are not removed by our uninstaller. If an outdated CRT is already present
and the probe fails, update the Visual C++ x64 runtime from Microsoft before retrying.

Close the publisher, then run `%LOCALAPPDATA%\FreedomStream\Uninstall.ps1`.
It checks its ownership marker, resolved path, reparse points and running installed
processes before removing that installation. It removes its own shortcut only if
the shortcut still points there. `-KeepDownloads` keeps only the download cache.
An interrupted install is kept for inspection; do not point the installer at an
unrelated nonempty directory.

## Dependency notices

Original FreedomStream code is MIT licensed. The ZIP includes Cargo dependency
license texts and the prebuilt WebRTC aggregate notice. `deploy/license-overrides.json`
pins upstream texts and hashes for crates whose published archives omit workspace
licenses. Egui 0.37.0's unavailable crate revision uses its published 0.37.0 tag's
license text. `from_variants` declares MIT/Apache-2.0 but has no root license file;
we select Apache-2.0 and include its standard text with source provenance. The
LiveKit stereo feature patch is disclosed alongside its Apache license.

GStreamer is downloaded directly rather than redistributed in our ZIP. Its full
`runtime/share/licenses` and version inventory remain installed; its plugins and
dependencies have their own licenses. Do not describe the full runtime as MIT,
or redistribute a trimmed/repacked runtime without qualifying the licenses and
corresponding-source obligations for the actual included plugins. Hardware HEVC
support is not a statement about codec patent rights.

## Qualification limits

CI and a local isolated installation check packaging, notices, portable extraction,
probe and removal. This is not a fresh Windows PC. AMD, physical 4K120, real games,
fullscreen/alt-tab, capture/audio under game load, macOS/Linux, and an evening with
four/five friends remain separate hardware and usability checks. Their results
must not be inferred from compile checks or synthetic video.
