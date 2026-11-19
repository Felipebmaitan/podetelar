# Discord sign-in

Configure the existing Discord application settings and a server-only
`DISCORD_CLIENT_SECRET`. Register exactly
`PUBLIC_URL/api/auth/discord/callback` in the Developer Portal. Production and local
Compose pass the optional secret to the app; it never belongs in a native build,
repository, browser storage or log. The runtime bot token is separate.

The desktop initiates a five-minute sign-in request with a SHA-256 challenge for
its locally held random proof. The system browser opens a server authorization
entry, which sets an HttpOnly/SameSite=Lax browser nonce and redirects to Discord.
Only the `identify` scope is requested. The callback validates the state, browser
nonce and single-use flow before exchanging the code on the backend. It retrieves
the profile, discards Discord access/refresh tokens, and redirects to a clean
completion page. The native app must supply the original proof in a POST body to
redeem a new PodeTelar login exactly once. Browser completion never receives that
login credential. Cancellation/failure/expiry requires a fresh app request.

SQLite stores the Discord ID, display name and avatar hash plus opaque PodeTelar
login hashes, identifiers and expiry metadata. Logins are bound to the backend's
public origin, last 30 days, and are limited to 16 active logins per account. They
are separate from 24-hour room sessions and never grant administrator access.
Nickname and avatar synchronization inside a Discord-associated room uses the
runtime bot's targeted member query rather than user OAuth refresh tokens.

The Discord dashboard lists stored accounts and active login counts. Administrators
can inspect/revoke individual logins, revoke all logins, disable/enable accounts,
hide/show their Discord avatars, or confirm deletion of the stored profile.
Disabling preserves the identity restriction; deleting allows a later fresh sign-in
to create a profile again. Room/session management is separate.

Tests exercise callback browser binding, native proof, replay, origin scoping,
expiry, hashed credential persistence, administrator isolation and revocation. They
use synthetic Discord responses. Actual consent/callback behavior must still be
checked with the deployed Discord application and a real user; no live OAuth
qualification is implied by those tests.
