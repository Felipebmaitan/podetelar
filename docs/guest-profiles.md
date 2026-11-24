# Guest profiles

The native app remembers the last successfully used guest nickname in a local
`PodeTelar/guest-name.json` preferences file. It contains the nickname only, not
invitations, credentials, room history or raw hardware information. Guests can
open User settings from Room options or the gear in the room roster. Open the
roster from the room name menu. Advanced settings are available from User
settings and the arrow menu on **Share screen** in the room dock.

Changing a nickname updates the existing participant session and the SFU display
name without replacing its identity or restarting sharing/watching. Failed SFU
updates are persisted for fair retries with a five-second backoff; expired queue
entries are removed. Other connected native apps observe the change.

## Pictures and ownership

Guests can upload, replace or remove PNG, JPEG, WebP and animated GIF pictures while
admitted to a room. The upload limit is **3,000,000 bytes**, enforced by the app
and server. Input is limited to 4096 pixels per side, 200 frames and 16 million
total decoded pixels. The server accepts two concurrent decodes, normalizes to
128 by 128 pixels, strips metadata and stores static PNG or animated GIF output.
Native decoding and HTTP operate on a separate profile worker; native display
uses 64-pixel animation frames and a single texture per avatar revision.

SQLite stores one current picture per server-scoped PC identifier, shared across
that PC's guest sessions and rooms. Pictures are independent of temporary stream
previews and do not capture screens or subscribe to media. Database backups
include the profile metadata and image bytes. Storage is capped at 128 MiB of
image bytes and 10,000 profile records per backend.
Regenerate the production HTTPS proxy configuration when deploying this feature:
the avatar upload path needs its separate 3,000,000-byte allowance. The default
256 KiB preview limit is retained, including on installations with split origins.

**PC claims intentionally authorize cosmetic changes.** A current room session
fixes the claimed PC identifier at admission; an upload cannot supply a different
identifier. The server cannot prove that a modified client derives its claim from
the actual hardware. Someone who obtains another PC's identifier and admits a
session claiming it can change that profile's picture. This accepted limitation
does not grant room admission, media authority or administrator permissions.
There is no separate profile edit credential or credential-reset workflow.

Participant responses and image URLs expose participant IDs and random image
revisions, never PC identifiers. Image retrieval requires an admitted session in
the same room, and rechecks the target session and image revision. Profile metadata
refreshes every three seconds; deleting/replacing an image retires the old revision.
Authenticated profile routes use per-session request budgets, keeping several PCs
behind one network address from exhausting unrelated room/session request limits.

## Administration

The administrator dashboard's Guest profiles section lists every stored profile,
including profiles with no image or a blocked upload policy. It shows the PC
identifier, latest nickname, image size, animation status and last image/policy
change time. Open the image for inspection, confirm deletion, or block/allow future
uploads independently. Policies and deletions survive server restarts. A deletion
also invalidates a decode already in flight; the app never reuploads cached bytes.

These routes require the dashboard administrator session; room membership and an
administrator bearer token alone cannot access the gallery. Guests may still
remove their own picture while uploads are blocked.

## Dependencies and checks

Server image processing uses pinned Sharp 0.35.5 (Apache-2.0) with its platform
libvips runtime and bundled dependency licenses. It is a server-only dependency;
it is not bundled in the Windows publisher. Platform packages retain license
notices in `node_modules`; distributors must retain applicable notices and source
availability obligations. Primary references: [Sharp installation](https://sharp.pixelplumbing.com/install/)
and [libvips distribution sources](https://github.com/lovell/sharp-libvips/tree/v1.3.4).
The native GIF/image decoders and file picker retain their Rust dependency
licenses, collected by the existing packaging workflow.

Run `npm run check`, `npm run native:check`, `npm run native:test`, Docker smoke,
and `npm run test:admin -- e2e/admin-profiles.spec.ts`. Actual native profile and
media checks remain required for release; API fixtures alone do not qualify the
installed app or animated rendering on an untested platform.
