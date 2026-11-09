# Phase 5 verification

Phase 5 implements publisher-generated private still previews. This record distinguishes the implementation from hardware/production certification and is updated with the acceptance results before its PR merges.

## Acceptance checks

- API checks: JPEG dimensions/size/structure, same-room admission, Origin, ownership and active unmuted SFU publication, minute write limit, latest-only revisions, late join, expiry, bounded capacity, SFU outage, and Stop during an in-flight announcement.
- Browser lifecycle checks: descriptors must be bounded and match the current publication, and cannot create video/audio subscriptions. Existing capture cleanup/reconnect tests remain required.
- Real-browser minute scenario: two actual still captures at least 60 seconds apart, one camera acquisition, a late viewer receiving the existing still, 320×180 decode, no unwatched `<video>`, and image denial/capture cleanup after Stop.
- Native NVIDIA scenarios: HEVC and H.264 still/live decode through the Canada SFU; the minute scenario measures exactly two generated/uploaded stills in 68 seconds, zero incoming RTP at two unwatched viewers, late-join reuse, and image denial after Stop while the publisher remains admitted.
- Full existing audio isolation/recovery and selective subscription scenarios, Node checks, native format/clippy/tests, Docker smoke/browser checks, CI and a fresh GPT-6.1-Sol high-reasoning review.

## Observed results

- Node formatting, types, 20 API/config checks, 17 browser lifecycle checks and production builds pass. Native formatting/clippy, 15 Rust tests and three setup-integrity checks pass. Docker smoke also passed, including an app restart and session persistence.
- NVIDIA GPU fixture encodes remain near 60 fps for HEVC and H.264 with one initial JPEG in each 120-frame check. The complete six-scenario native end-to-end suite passed through the Canada SFU, including audio isolation, stereo, Watch/Stop watching, reconnect and cleanup.
- The real native minute scenario generated/uploaded exactly two 320×180 stills over 68 seconds. Two unwatched Chrome viewers, including a late join, received zero RTP bytes. After Stop, the latest image returned 404 while the media identity intentionally lingered.
- A full native reconnect reattached the cached JPEG to its new SID: two uploads but only one generated image over 26 seconds. The prior revision returned 404, and Stop removed the regenerated A/V publications.
- The full desktop/mobile browser suite passed six scenarios with two intentional duplicate transport skips. Its minute scenario captured exactly twice at least 60 seconds apart using one camera acquisition, recovered from a failed first upload using the cached image, served a late viewer immediately, and denied the image/stopped capture after Stop.

PR #4 merged after all four CI jobs passed and a fresh GPT-6.1-Sol high review approved head `b292093e72b20279ac11c157bc26fbdc6143ca3c`. AMD remains unavailable; these checks do not certify AMD, real-game/fullscreen capture, 4K120, a production deployment or an evening with five friends.

## Test privacy

Preview qualification uses only a synthetic GPU pattern or the browser fake camera, not desktop/camera pixels. Tests do not record audio. Preview JPEGs go to the local private API; synthetic encoded A/V uses the temporary authenticated qualification SFU. A shared create-room helper replaces transport failures with a fixed message so Playwright's network-error call log cannot echo the administrator header. Native diagnostics remain bounded and do not print invitations, tokens or raw SDP.
