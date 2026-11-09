# Friends beta delivery — 9 November 2026

This records the earlier delivery. See the subsequent
[friends feedback beta](friends-feedback-beta.md) for the current executable and
VPS build.

This delivery packages the final reviewed native UI and updates the existing VPS
room to its compatible native-app server. It is a friends beta, not AMD,
fresh-machine or real-game/group qualification. The existing room invitation is
kept in ignored operator files and supplied privately, never in this report.

## Packaged audio check and correction

The initial portable package of `095e680` passed isolated runtime/extraction and
integrity checks and six of eight native media checks. Two audio checks failed:
Discord/owned-playback tones reappeared in the automatic desktop mix.

Controlled diagnosis confirmed that direct Game-only process capture was clean,
and Discord was already absent from allowed application roots. NVIDIA Broadcast
was re-rendering a processed copy of the desktop mix under its own process.
Excluding that one proxy image path reduced the steady 880Hz/1320Hz tones from
approximately 0.02 to roughly 0.000001–0.000003 while preserving the 440Hz Game
tone at approximately 0.02. No microphone capture was opened.

The correction excludes `NVIDIA Broadcast.exe` and its descendants using the
existing protected-tree policy. Original application audio is captured directly;
the processed duplicate must not reintroduce Discord or owned playback. A
regression verifies desktop roots, a selected proxy window, and the allowed-app
inventory. Native formatting/Clippy and **7 portable + 29 native tests** passed.
UI source is unchanged from the final all-seven-9/10 Gemini review.

The fix received fresh GPT-6.1-Sol high code-scope approval without material
findings. Its rebuilt portable package passed both previously failing audio
checks and the complete eight-test native media suite.

## Delivered portable executable

- Build: `a4eb88e0c3df591486fdf446785ef1de2a61b8c2`.
- Filename: `freedomstream-windows-x64-0.1.0-a4eb88e0c3df.exe`.
- Size: **81,715,665 bytes**.
- SHA-256: `598bbb87963f22025c15383cd300e7c5a9eb718f4a2cb27bef7f6e3376d91367`.
- A verified identical copy is provided locally as `FreedomStream.exe` for sharing.
- Portable qualification passed isolated/concurrent extraction, installed and
  bundled runtime probes, source/payload integrity, cache preservation, explicit
  extraction, damaged-package rejection and GUI-subsystem checks.
- **Eight packaged native media tests passed** using the extracted shipped
  runtime: H.264/HEVC decode, preview delivery, Stop/rewatch traffic, shared codec
  fallback, stereo/Discord/owned-playback rejection, recovery and receiver caps.

The UI is unchanged from the final independently rated seven-category 9/10 pass.
Original application code remains MIT; matching third-party notices/sources stay
inside the self-contained EXE. No installation or shared media runtime is needed.

## VPS delivery and checks

The earlier browser-era server was upgraded to the compatible native-app server.
The final server/config release is `b04379f53069c95eb29ce1b1a6c0c889a680c103`,
serving the EXE above. The existing Friends invitation remains valid. Verified
WAL-aware backups and rollback copies were made before activation; private keys,
room state, SFU and relay services were preserved. Unrelated site/container
configurations and running services were not changed.

The site's old 80KiB nginx input limit was corrected to **256KiB**, matching the
native preview contract. A regression ties the generated proxy limit to
`PREVIEW_MAX_BYTES`. The Node 24 full project check passed. `nginx -t` and app
health checks passed. Through public HTTPS, a 100,000-byte preview request reached
API admission (401 without a session), while 262,145 bytes was rejected with 413.

Verified through the production origin with the actual existing invitation:

- Join and native publish/subscribe grants work; browser media grants deny both.
- Anonymous EXE download is denied. The admitted complete EXE download's size and
  SHA-256 match the delivered binary.
- Packaged native H.264 decoded **324 frames** in the first bounded production
  check; an extended independent native HEVC check decoded **1,216 frames**.
- A forced-relay H.264 **publisher** with an automatic-route native watcher decoded
  **1,098 frames**. This is not a blocked-UDP or watcher-forced-relay qualification.
- Test publications stopped, receiver clients closed, temporary test invitations
  were disabled, and owned verification sessions were cleaned up.

The short first mixed-codec production probe yielded insufficient HEVC frames;
the longer standalone HEVC check above passed. A redundant full-download repeat
later timed out/was stopped while bytes were still transferring; the earlier
complete checksum verification passed. The final proxy-only checks were bounded
and passed. These observations are retained rather than calling every probe a
success or claiming download-throughput/startup-latency guarantees.

Ignored operator evidence includes `friends-fixed-package-build.log`,
`friends-fixed-portable-check.log`, `friends-fixed-packaged-media.log`,
`friends-production-check.log`, `friends-vps-hevc-diagnostic.log`,
`friends-vps-relay-check.log` and `friends-preview-proxy-check.log`.

## Scope

This is the user-authorized friends beta deployment, not a merge or broad release
qualification. AMD, a genuinely fresh/offline Windows PC, real-game/Discord/group
usage, synchronized A/V latency, deliberately blocked UDP, physical 4K120 and an
evening with friends remain field checks. No compile result or synthetic frame
count substitutes for them. The existing invitation is not published in the PR.
