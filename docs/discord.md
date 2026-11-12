# Discord room creation

Discord HTTP interactions can create rooms directly inside FreedomStream: there
is no gateway connection, message-content intent or Discord SDK dependency.
`/stream` and `/telar` are global, server-installed commands. They each create a new room and reply in the invoking channel.
Anyone who can read or forward the invitation can join; use a private channel.
Embeds and automatic mentions are suppressed. Discord necessarily receives and
stores the invitation in that message. It is not end-to-end secret from Discord.

The existing lifecycle applies: an unused room waits for its first join; after
participants connect it closes when everyone leaves, with the existing short
reconnect grace. Creating a new room does not close a previous occupied room.
Administrator room creation remains available for recovery and installations
without Discord. Clients remember server origin and display name, not invitations,
and never autojoin on startup. A saved server is a destination, not admission.
Opening or pasting a fresh link selects the room and its server explicitly.
Installer-managed links still activate the running app. A link for the saved server
can load its one-use handoff directly into Join confirmation; an unfamiliar server
needs trust first. Loading never joins automatically. Download/Later/restart update
controls continue to work while waiting for a room.

## Setup

1. Create a dedicated application in the Discord Developer Portal. Record its
   Application ID and Public Key. Enable server installation with the
   `applications.commands` scope. Global commands become available in every server
   where the app is installed; DMs and user-only installations are excluded.
2. Set `DISCORD_APPLICATION_ID`, `DISCORD_PUBLIC_KEY` (64 hexadecimal characters),
   and `DISCORD_GUILD_IDS=*` in the server environment. Use an explicit
   comma-separated ID list only when intentionally restricting a private deployment.
   Production Compose passes these optional values through. All three are required
   together; leave all unset to disable the endpoint. Restart the app.
3. Set the application's Interactions Endpoint URL to
   `https://YOUR_PUBLIC_ORIGIN/api/discord/interactions`. Discord must reach a
   public HTTPS endpoint with a trusted certificate; its PING is signature checked.
   Confirm the actual configured origin/port is supported and reachable from Discord
   before rollout. Retain the production proxy's disabled request logging.
4. Store `DISCORD_BOT_TOKEN` only in a local untracked environment file used for
   command registration (never the runtime container, logs, source, or CLI args).
   Run `node --env-file=PATH_TO_PRIVATE_ENV scripts/register-discord.mjs` with Node 24. The file must also set the application and guild IDs. The script upserts
   only these two guild commands and does not bulk-delete other commands. Errors
   report HTTP status only. For HTTP 429, wait before rerunning.
5. Restrict command access using Discord server integration permissions if needed.
   Test both commands in a private channel, opening each response with a second
   participant. Confirm they enter distinct rooms and a stranger without a valid
   invitation is refused. Confirm room closure after all members leave.

The live server needs only the application ID, public verification key and guild
policy, never the bot token or FreedomStream administrator key. Request
signatures cover the timestamp plus exact raw bytes; accepted timestamps are within
five minutes. Successful interaction IDs are recorded transactionally with room
creation and retained ten minutes. A retry returns an ephemeral already-handled
notice without recreating or redisclosing the invitation, including after restart.
Creation is limited to five rooms per minute for a guild or invoking user. Invalid
configuration fails startup. Other POST routes still enforce browser Origin.

## Server authorization and verification

With `DISCORD_GUILD_IDS=*`, a signed interaction must identify the same guild as
its guild installation owner (`authorizing_integration_owners["0"]`). A user-only
installation, mismatched owner or DM cannot create rooms. The same existing
application/signature/timestamp/replay and per-guild/user quotas still apply.
Adding the bot to another server needs no server configuration change in this
mode. An explicit ID list retains the earlier restricted behavior.

Global commands can take time to refresh in Discord clients. Legacy guild-scoped
commands may shadow the global commands in servers used for earlier testing; they
use the same handler. Remove only PodeTelar's own legacy commands when retiring
the old guild-specific registrations; preserve unrelated app commands.

Live trials of both aliases, public HTTPS validation and native invitation
admission are recorded in [verification](verification/discord.md). Production
needs a permanent endpoint; stopping a temporary development tunnel stops its bot.

Primary references:
[Discord interactions](https://docs.discord.com/developers/interactions/receiving-and-responding)
and [application commands](https://docs.discord.com/developers/interactions/application-commands).
