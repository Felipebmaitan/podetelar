# Discord feature stack

Review and merge in dependency order. These PRs are intentionally unmerged:

1. [#51](https://github.com/Felipebmaitan/podetelar/pull/51): remembered guest nicknames, live editing, animated avatars and dashboard moderation.
2. [#52](https://github.com/Felipebmaitan/podetelar/pull/52): bot Gateway snapshots, voice index and connection administration.
3. [#53](https://github.com/Felipebmaitan/podetelar/pull/53): canonical channel rooms, permanent guest invitations, announcements and channel controls.
4. [#54](https://github.com/Felipebmaitan/podetelar/pull/54): browser OAuth handoff, local accounts, revocable login sessions and account administration.
5. [#55](https://github.com/Felipebmaitan/podetelar/pull/55): native OS credential storage, sign-in settings and guild profile identity.
6. [#56](https://github.com/Felipebmaitan/podetelar/pull/56): automatic voice following, session provenance/enforcement and global/user controls.

Each PR targets the preceding feature branch. Preserve ancestry with merge commits,
or restack and retarget the remaining branches after a squash/rebase merge. Do not
merge a child into its feature base while intending to review the parent separately.

## Deployment configuration

Read [bot setup](discord.md), [sign-in](discord-accounts.md) and
[voice behavior](discord-automatic-rooms.md). Configure the bot token and OAuth
client secret only on the backend, and register the exact public callback URL.
Enable Guilds and GuildVoiceStates intents; no privileged members/message-content
intent or user OAuth refresh token is used. Ordinary voice channels are enabled by
default; use the dashboard for exceptions, including AFK/private channels if needed.
The desktop defaults to the existing hosted backend and also supports guest links
to other deployments. Backend discovery is not a federation mechanism.

Back up the SQLite database and avatar directory before deployment. Migrations
add tables and preserve existing room/session access. Discord sign-in and Gateway
runtime remain optional when their configuration is absent. New credentials are
separate from room cookies and administrator sessions. The cosmetic PC-ID spoofing
limitation accepted for guest profiles remains documented in
[guest profiles](guest-profiles.md).

## Verification and remaining acceptance

The cumulative stack passes Node 24 checks and native clippy. Native tests exercise
voice state transitions, cancellation before a room switch, identity retry races,
and actual Windows Credential Manager storage. Local HTTP/SFU browser checks cover
administrator profile moderation and follow policies. A synthetic Gateway move
against an actual Windows SFU removes the controlled media peer and rejects its
next media token while an independent invitation peer remains connected.

The packaged Windows executable passes portable extraction/runtime/license-source
qualification. A packaged native participant changes its real SFU name and
retains an animated avatar across sessions. These checks qualify local artifacts;
they do not qualify a production deployment, fresh PC, macOS or unavailable hardware.

Before enabling Discord in production, check real consent/callback completion and
cancellation, bot permissions/member nickname/avatar lookup, two signed-in users
converging on one room, A-to-B moves while sharing/watching, confirmed voice leave,
bot restart/recovery, and an independent guest remaining connected. Confirm that
capture and subscriptions end and never resume automatically in the destination.
Also check the installed image picker and macOS Keychain on their actual platforms.
No real Discord credentials were available for those external checks in this run.
