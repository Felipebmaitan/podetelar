# macOS Apple Silicon

The macOS port targets Apple Silicon (`arm64`) and macOS 14 or newer. It uses
ScreenCaptureKit for display/window and application/system audio capture,
GStreamer's hardware-only VideoToolbox H.264/HEVC encoders, a pinned WebRTC decoder
adapter requiring hardware VideoToolbox sessions, and output-only WebRTC/CoreAudio
playback. AV1 receive (8-bit Main profile) is advertised only when VideoToolbox
reports hardware support; AV1 encoding is unavailable. Older chips without an
AV1 hardware decoder advertise H.264/HEVC instead. There is no
software encoder fallback. Actual supported modes are checked when capture starts;
this does not establish 4K120 or every M-series chip's performance.

The Finder application launches a native GUI without a terminal. Its media UI
has a nested application bundle so macOS can identify its window and permissions
independently of the outer update/link host. Launch Services
routes `freedomstream://` invitations through the app host and the authenticated
local activation bridge. Joining, changing rooms, sharing and subscriptions retain
the existing explicit controls. Grant Screen & System Audio Recording permission
to PodeTelar when macOS requests it, then refresh sources. Capture never opens a
microphone. Desktop audio excludes PodeTelar and Discord; application/window audio
uses ScreenCaptureKit filters. Exclusion, actual output and physical A/V timing
require separate measurements, described in the verification record.
The [qualification record](verification/macos.md) distinguishes measured behavior
from remaining hardware, GUI permission and distribution gates.

## Development and checks

Use Node 24, the pinned Rust 1.95 toolchain, Apple's command-line developer tools,
`pkgconf` and .NET 8 for packaging. All commands run noninteractively.

```sh
npm ci
npm run native:setup
npm run check
npm run native:check
npm run native:test
cargo clippy --locked --manifest-path apps/installed-launcher/Cargo.toml -- -D warnings
cargo test --locked --manifest-path apps/installed-launcher/Cargo.toml
npm run native:qualify
INSTALLER_BUILD_NUMBER=1 npm run native:package:macos
node scripts/qualify-macos-updates.mjs
```

`native:setup` verifies and extracts the official pinned GStreamer 1.28.8 runtime
and development installers into `.local`, without installing a shared framework or
requiring administrator privileges. The macOS commands select that SDK and a
curated plugin directory. Release builds disable Rust's stripping path because
Rust 1.95 can produce dylibs rejected by macOS 27's loader; see
[upstream issue](https://github.com/rust-lang/rust/issues/157750).

A clean committed checkout is required for a release. `--qualification` permits
an explicitly marked local test artifact. Packages include the allowlisted
runtime's complete dependency closure, license texts, original source archives,
Cerbero recipes, Rust/WebRTC notices and source provenance. Packaging rejects
unqualified libraries, missing notices, wrong architectures and libraries whose
minimum OS exceeds macOS 14. The app has no Homebrew or shared GStreamer runtime
requirement. LGPL dynamic libraries remain replaceable; re-sign a locally modified
bundle ad hoc after replacement.

`qualify-macos-media.mjs` requires an isolated, reachable local API/SFU and screen
recording permission. It uses the newest locally packaged app by default. Set
`MACOS_TEST_APP` for an explicit bundle, `MACOS_TEST_ORIGIN` for the API, and
`MACOS_TEST_SOURCE_KIND=window` to exercise a window. Pass the local `.env` using
Node's `--env-file=.env`; invitations remain in memory and child stdin. It verifies
native H.264/HEVC hardware decode, publication, Stop watching traffic and rewatch.
Window checks use the actual source resolution instead of assuming a display
scale. `MACOS_TEST_SOURCE_KIND=pattern` explicitly tests hardware codecs without
screen capture, for example when the display is asleep; it does not qualify
ScreenCaptureKit behavior.
`node --env-file=.env scripts/qualify-macos-capture.mjs` additionally creates an
owned changing window and stereo source and requires real CoreAudio output.
`node scripts/qualify-macos-exclusions.mjs` checks an initially quiet desktop,
late application capture and excluded application launch/restart using distinct
real tones. Run a release build first; this qualifier uses the development
helper and release Rust binary.

For actual hardware AV1 receive qualification, create a local test fixture using
an external FFmpeg build with SVT-AV1, then build the explicit qualification
sender. Neither that encoder nor the qualification sender is bundled in releases:

```sh
ffmpeg -y -f lavfi -i testsrc2=size=1280x720:rate=30 -frames:v 600 \
  -c:v libsvtav1 -preset 12 -crf 38 -g 30 -svtav1-params pred-struct=1 \
  -pix_fmt yuv420p -f ivf .local/macos/av1-fixture.ivf
node scripts/native.mjs build-fixtures
node --env-file=.env scripts/qualify-macos-av1.mjs
```

This sends the pre-encoded AV1 stream through the real SFU to the latest packaged
receiver, requiring hardware decode, frames delivered to the native consumer, Stop watching
and rewatch.
It does not qualify AV1 encoding on the Mac. Packaging also compares the built
app's decoder set with a separate VideoToolbox query and rejects software AV1
advertisement or any AV1 encoder. Hosted runners exercise their actual reported
capability set, including absence of hardware AV1.
The updater qualifier uses real packages/helpers with a process fixture; it
separately checks Later, corrupted cache replacement and explicit restart.
Neither fixture substitutes for physical speaker/display timing or a fresh Mac.

## GitHub releases and updates

The `macOS Apple Silicon` workflow runs on `macos-15` ARM64 for relevant PRs and
workflow dispatch. Every published GitHub release builds the tagged source and
attaches the `.dmg`, full `.nupkg`, `releases.osx-arm64.json`, macOS manifest and
`SHA256SUMS-macos`. The tag must match `package.json` (`vX.Y.Z`). Run numbers create
increasing internal preview versions. Existing Windows assets and the `preview`
feed are preserved; macOS uses package ID `PodeTelarMac` and channel `osx-arm64`.

Updates check the public GitHub repository. Download and restart are explicit;
Later leaves the current version installed. The launcher verifies the full
package's size and SHA-256, waits for the media process to exit, then invokes
Velopack's macOS replacement helper. No invitation is passed to the updater.

Current CI produces an ad hoc signed preview. Developer ID signing and Apple
notarization require release credentials and are a distribution gate. Local
packaging supports `MACOS_SIGN_IDENTITY` and an existing
`MACOS_NOTARY_PROFILE`; this does not configure credentials in GitHub Actions.
An ad hoc signature is not Developer ID signing or notarization. GitHub-hosted
builds do not qualify GPU capture, permissions, audible output or a fresh-machine
Gatekeeper/update experience.
