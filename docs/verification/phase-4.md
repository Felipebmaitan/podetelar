# Phase 4 verification

Windows application audio is implemented with selected-application and desktop application mix modes, mandatory Discord/owned-playback exclusions, additional executable exclusions and shared audio/video cancellation. Tests distinguish controlled fixtures from field qualification.

## Observed results

- Policy tests cover ancestor/child deduplication, mandatory Discord/owned-viewer exclusions, executable-based restarts, explicit exclusions, PID reuse and unavailable process birth metadata.
- Actual Windows WASAPI process loopbacks capture the generated game tone, omit Discord and owned Chrome playback tones, recover after game and Discord fixture restarts, and honor a later explicit game exclusion. Only aggregate spectral measurements are retained. The test confirms that a fresh viewer profile is removed.
- The game fixture's audio reaches Chrome through the temporary Canada VPS SFU. Browser RTP counters confirm that Stop watching stops incoming audio and video, rewatch resumes reception, and Stop releases native capture.
- The existing HEVC and H.264 native video transport checks continue to pass. Native formatting/clippy and Rust tests pass locally; the final PR still requires exact-head CI and a new GPT-6.1-Sol high review.
- A real full native reconnect republishes both tracks with new IDs. Stop removes both while the participant intentionally stays connected for six more seconds, proving removal is not masked by a room disconnect. A native test verifies that cancelling a lease disables actual retained RTC audio/video tracks without opening capture devices.
- The review found that the SDK can empty its local publication map before sender removal fails. Cleanup now awaits both removal outcomes and closes the media identity on any error/timeout, even with an empty local map. A regression test reproduces that ordering and verifies both tracks are attempted; a successful cleanup test preserves the connected identity.

The application captures/mixes at 48 kHz with two channels and publishes Opus at a 192 kbps target, without voice noise suppression, AGC or echo cancellation. A distinct-left/right synthetic fixture checks channel separation rather than inferring stereo from a codec label alone.

That stronger test found intermittent mono negotiation in unmodified SDK 0.10.1. A minimal publication-feature patch now advertises native stereo; repeated VPS tests verify both `sprop-stereo` negotiation and sustained channel separation. SDK setup checks the package, changed file and complete source tree, including rejection of tampered archives/generated source. See the [dependency decision](../adr/0003-native-stereo-publication-feature.md).

## Field qualification still open

No AMD PC is available; hardware video encoding qualification is unchanged from Phase 3. These audio checks use generated sounds rendered by separate Windows processes, one named Discord.exe to exercise the exact discovery/exclusion path. They do not claim a session with friends in the actual Discord app, real-game performance, protected/DRM audio compatibility, headset route changes or long-running group usage. Regular browser playback needs explicit browser exclusion; automatic attribution is guaranteed only for the owned viewing window. Windows versions below build 20348 cannot use application audio and do not receive a less private fallback.
