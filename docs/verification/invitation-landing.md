# Invitation landing correction

On 2026-11-11, changed invitation URLs to show only the native join action and
connected room members. The browser does not join a room, acquire a media token,
or expand instructions after launch. Existing native server trust and nickname
confirmation and optional per-user protocol registration remain required.

Validation: Node 24.11.1 `npm run check`; invitation-preview authorization,
no-session side effects, active-member filtering, SFU failure, and invitation
rotation during lookup; existing one-use/expiry/revocation/resource handoff tests.
Desktop 1280px and mobile 390px Playwright checks use the built HTTP server and
a real local LiveKit service (empty room). They verify mouse/Space protocol
navigation, stable page contents, ticket redemption/replay denial, automatic
refresh, absent browser admission/media requests and no browser-storage secrets.
They do not establish actual Windows activation or populated-room behavior
through a real native client. Those checks remain unverified for this correction.

A fresh GPT-6.1-Sol high-reasoning independent review approved the patch after
fixing Space-key activation. No native/runtime distribution changes are included.
