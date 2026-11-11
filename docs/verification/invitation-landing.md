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
The first actual Windows activation exposed the native failure: ShellExecute
canonicalizes `freedomstream://join?...` into `freedomstream://join/?...`. The
installed parser rejected the root slash and showed Invalid browser invitation.
The correction accepts the empty/root path only, retains every other validation,
and tests both canonical forms and rejection of other paths. A new portable
release and updated cached protocol launcher are required.

A fresh GPT-6.1-Sol high-reasoning independent review approved the patch after
fixing Space-key activation. A second fresh GPT-6.1-Sol high-reasoning review approved the native URL-parser
correction. No runtime dependencies or media capabilities change.
