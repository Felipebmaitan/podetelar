# Development

Use Node 24 LTS and npm workspaces. Start with the [README](../README.md) for local
setup and [testing](testing.md) for validation commands. Native toolchains are
covered in the [Windows](native-publisher.md) and [macOS](macos.md) guides.

## Code layout

- `packages/contracts`: validated API inputs and shared response types.
- `apps/server/src/rooms.ts`: SQLite room/session storage, invitation protection and expiry.
- `apps/server/src/app.ts`: HTTP authorization, origin checks and media grants.
- `apps/server/src/access.ts` and `signaling.ts`: durable revocation and bounded signaling admission.
- `apps/server/src/previews.ts`: private still-image storage, cadence and publication checks.
- `apps/web/src`: administrator dashboard, invitation pages, access management and downloads.
  The legacy `media/` controller is not connected to the participant entry point.
- `apps/publisher`: Rust native UI, capture, encoding, audio, subscriptions and playback.
- `apps/installed-launcher`: installer updates and application activation.
- `deploy/`: pinned runtimes, license inputs and production configuration.
- `scripts/`: setup, packaging and qualification tools. Generated artifacts belong under ignored `.local/`.

LiveKit forwards compressed media. The app server handles room authority, previews
and diagnostics without decoding video. Native capture and playback use platform
adapters; see [quality](quality.md), [audio](audio.md) and [macOS](macos.md).

## Configuration

`npm run setup` creates a private `.env` without replacing existing secrets.

| Setting                | Purpose                                                               |
| ---------------------- | --------------------------------------------------------------------- |
| `ADMIN_TOKEN`          | Administrator credential, at least 32 characters.                     |
| `LIVEKIT_API_KEY`      | Server-side SFU key, at least 8 characters.                           |
| `LIVEKIT_API_SECRET`   | Server-side SFU secret, at least 32 characters.                       |
| `PUBLIC_URL`           | Exact website origin, without a path, query, fragment or credentials. |
| `DIRECT_URL`           | Optional native service origin; defaults to `PUBLIC_URL`.             |
| `LIVEKIT_INTERNAL_URL` | Private SFU HTTP origin, default `http://127.0.0.1:7880`.             |

Remote origins require HTTPS and secure WebSocket signaling. Loopback HTTP is
allowed for development. Mutating browser/API requests require the configured
origin. Native admission uses the same endpoints and a memory-only cookie jar;
it never receives administrator or SFU credentials.

Local npm development uses `http://localhost:5173`. Compose uses
`http://localhost:3000` and reaches the SFU at `http://livekit:7880`. Public signaling
passes through the app's `/media` gateway; keep the SFU HTTP listener private.
See [deployment](deployment.md) for separate website/direct origins and proxy setup.

## Room contracts

Invitations admit new sessions; sessions are not accounts. Sessions last 24 hours.
New rooms await their first join, then close after everyone leaves and the
reconnect grace expires. Operators can rotate/disable invitations and revoke
sessions. API, previews and signaling check current authority; failed SFU removals
remain in a durable retry queue. See [room access](access.md).

SQLite supports a single application instance. Independent databases do not share
sessions. Back up the database before migrations and retain a matching backup
when rolling back to code that cannot read the new schema.

Watch is an actual subscription. Stop watching releases video and audio; hiding
a tile does not. Uploaded previews capture once at stream start and at most once
per minute afterward. Codec replacements and reconnects preserve that clock.
Native media-token requests include a bounded `hardwareDecoders` list (`av1`,
`hevc`, `h264`). The server puts that declaration in the initial participant's
`podetelar.decode.hardware.v1` attribute; `none` represents an explicitly empty
set, while an absent attribute remains unknown. Declarations describe native
hardware availability, not binary attestation or a throughput certificate. Every
room member participates in hardware compatibility, including preview-only
members. Playback health never removes a codec from that declaration.

## Native dependency maintenance

Native setup verifies pinned GStreamer and SDK archives and materializes patched
sources under `.local/native-sdk`. `deploy/native-sdk.json` records archive and
source-tree hashes. Generated-source mismatches stop setup.

The stereo patch adds the SDK's stereo publication feature for two-channel native
sources. On SDK upgrades, check whether upstream provides it, remove the patch
when possible, and rerun actual channel-separation and recovery checks. Decoder
adapters have their own pinned source inputs. Preserve upstream licenses and
include notices and corresponding sources when packaging.

## Pull request checks

Run `npm run check`. Native changes also require native formatting/Clippy, tests,
packaging and the affected media acceptance scenarios on the actual platform.
Record the tested commit and material limits in the PR. Independent review and
passing compilation cannot qualify unavailable hardware or physical A/V timing.
