# Phase 2 verification

## Implemented behavior

- Browser camera and screen publication into the authenticated LiveKit room, with screen audio where the capture source supplies it. Microphone capture stays off.
- Actual participant and publication metadata, including streams published before a viewer joins.
- Explicit Watch/Stop watching subscriptions for video plus its associated screen audio. Unwatched streams have metadata/placeholders only.
- Local preview without playing captured audio back locally; per-watched-stream volume/mute; focus/grid layout.
- Capture release on stop/leave, detached playback elements, recoverable connection errors, SDK reconnect state, and fresh admission/signaling after reload.
- Room-scoped short-lived tokens, unauthenticated signaling rejection, CSP media/signaling origins limited to configured endpoints, and no invitation/session credentials in browser storage.
- Media engine is loaded after admission, keeping SDK initialization/capture code off the landing page. React does not receive per-frame updates.

## Checks performed

- `npm run check`: formatting, workspace/tool type checks, 12 API/configuration tests, six media lifecycle regression tests and production build passed.
- `docker compose up --build -d --wait`: app and actual LiveKit server started successfully with production assets.
- `npm run smoke -- --restart-app`: actual HTTP room admission, session persistence across container restart, logout and LiveKit server credentials passed.
- `npm run test:e2e`: five tests passed; the duplicate mobile transport scenario is explicitly skipped, while room admission/authorization flows run at desktop and mobile viewport sizes.

The initial Linux CI run exposed Docker's different loopback forwarding behavior: forcing every SFU candidate to 127.0.0.1 worked through Docker Desktop but failed on the native Linux runner. The local configuration now advertises private container candidates as well as loopback, with ICE Lite and no external discovery. Published ports remain bound to loopback. LiveKit logging is restricted to warnings to avoid its verbose participant/SDP information logs.

The media scenario connects two publishing browsers and an independent viewer through the real local SFU. It verifies video decoding, screen-audio reception, per-stream output volume/mute, focus/grid behavior, metadata on late join, publication removal, publisher capture tracks ending, and room reload/leave behavior. It measures inbound RTP bytes: video is absent before Watch, video flows after Watch, and both video/audio stop increasing after subscriptions are removed and in-flight packets drain. The real media server rejects requests without a token.

The lifecycle regressions use the public controller interface with a controlled SDK transport. They verify immediate capture release while publication or failed-publication cleanup is pending, permission results arriving after leave, Watch preservation through the pinned SDK's teardown-before-Reconnecting event order, and pruning selections only when a publisher is genuinely absent. These tests cover event ordering and ownership; they do not simulate a WAN interruption or replace the real SFU browser scenario.

## Verification limits

Capture sources in automation are synthetic: Chromium's fake camera and a synthetic getDisplayMedia source containing audio/video. Signaling, encoding, RTP delivery and browser decoding are real. This does not certify physical camera drivers, actual desktop/window capture, anti-cheat compatibility, native NVENC/AMF, H.265, WAN quality or 4K120. Long network interruptions and TURN fallback belong to Phase 7. Native hardware qualification is separate; the available local GPU is NVIDIA, and no AMD test machine is currently available.

One active media connection per browser room session is currently supported. Browser storage may contain SDK log-level preferences, but must not contain invitations, session authority or media tokens. The SDK media chunk is loaded only after admission and remains a relatively large third-party dependency; the landing bundle does not include it.
