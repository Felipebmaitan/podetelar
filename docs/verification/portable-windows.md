# Portable Windows qualification

Recorded 2026-11-09 on the existing Windows/NVIDIA RTX 5080 development PC.
These results concern the portable distribution change; they do not qualify AMD,
a fresh PC, real games, physical 4K120 or a complete group evening.

## Passed evidence

- Node 24 formatting/types/API/web/policy tests and builds; native formatting,
  clippy and 23 Rust tests (20 publisher, three archive/cache tests).
- Bundled payload generation with 43 allowlisted media files, 15 LGPL plugin
  metadata checks, PE dependency closure, 11 checksum-pinned source/build archives,
  Cargo/WebRTC notices and unmodified release CRT provenance/license texts.
- Actual first startup from an empty private cache, a directory with spaces and
  a PATH without the development runtime. Concurrent probes converged on one
  complete cache. The forced bundled-CRT probe also started GStreamer 1.28.8.
- All extracted release/source hashes matched. Reuse preserved local changes;
  explicit extraction succeeded, existing destinations and reparse-point AppData
  were refused, and a damaged payload failed before extraction.
- The running publisher loaded the installed 14.51.36247.0 CRT trio after version
  comparison with the bundled 14.44.35211.0 files. Forcibly terminating its launcher
  ended the owned publisher through the Windows job guard.
- Docker build/smoke with persisted private rooms, and the standard browser suite:
  **10 passed, six skipped**. The opt-in room download separately passed and
  verified all EXE bytes against the authenticated release manifest.
- Portable native NVIDIA HEVC and H.264 reached Chrome through the production SFU,
  at approximately 60 encoded fps and 136–139 ms media RTT. Tests verified actual
  Watch/Stop traffic behavior, previews and native cleanup. This is synthetic
  1080p60 qualification, not game or high-rate qualification. Test-room invitations
  were disabled and their sessions revoked after the runs.

## Outstanding audio/environment check

The portable stereo browser test received silent audio: the controlled tone
process failed to open the default Windows render endpoint (`0x80070490`), and
capture reported no eligible audio applications. Follow-up comparisons using the
**full development runtime** also failed at that endpoint. Windows audio services
were running, but the device query returned zero present AudioEndpoint devices.
An earlier full-runtime control played successfully; the repeated controls did not.
Do not infer that the bundled CRT or plugin set alone caused this result.

The launcher now keeps its CRT in a separate directory and prefers a complete
newer installed trio, preventing an older app-local copy from shadowing newer
in-process Windows components. Module inspection confirmed that selection. This
does **not** establish that stereo capture/audio exclusion has passed in this
portable release. Re-run the real stereo/exclusion checks when a working Windows
output device is available; no audio settings/services were changed for the test.

## Test limits and follow-up

Running the optional download test inside the whole browser suite consumed an
additional room/join quota and caused two later UI attempts to receive 429. The
standard suite passed after a local app restart, with the download check run
separately. Production admission limits were not weakened.

First-launch probes used a clean cache and isolated DLL paths; the host network
was not disconnected and this was not a fresh Windows installation. The launcher
contains no downloader or installer path, and matching library sources/notices
accompany the EXE offline. A truly disconnected fresh-PC trial remains external.
The portable production rollout and its final authenticated download check occur
only after the PR review/checks and required runtime gates. The earlier local
clipboard-enabled installation remains available; Felipe confirmed external paste,
while invitation copying back out is still an unperformed manual check.
