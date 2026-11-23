# Discord room creation

For opt-in Discord login and voice following, see [automatic rooms](discord-automatic-rooms.md)
and [Discord accounts](discord-accounts.md).

Discord HTTP interactions create rooms inside PodeTelar. An optional runtime
bot Gateway connection also discovers voice membership across installed servers.
`/stream` and `/telar` are global, server-installed commands. When voice discovery is available, they reuse the invoking user's voice-channel
room and return its permanent invitation in a reply visible only to the caller.
Outside voice, each creates an ordinary room and replies in the invoking channel.
Anyone who can read or forward the invitation can join; use a private channel.
Embeds and automatic mentions are suppressed. Discord necessarily receives and
stores the invitation in that message. It is not end-to-end secret from Discord.

The existing lifecycle applies: an unused room waits for its first join; after
participants connect it closes when everyone leaves, with the existing short
reconnect grace. Creating a new room does not close a previous occupied room.
Administrator room creation remains available for recovery and installations
without Discord. Each invitation selects its server; the app keeps the nickname
only while running and requires confirmation before switching an active room.
See [native invitation behavior](distribution.md#invitations-and-launch-behavior).

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
   command registration and optional Gateway runtime (never logs, source or CLI args).
   Run `node --env-file=PATH_TO_PRIVATE_ENV scripts/register-discord.mjs` with Node 24. The file must also set the application and guild IDs. The script upserts
   only these two global commands and does not bulk-delete other commands. Errors
   report HTTP status only. For HTTP 429, wait before rerunning.
5. Restrict command access using Discord server integration permissions if needed.
   Test both commands in a private channel, opening each response with a second
   participant. Confirm they enter distinct rooms and a stranger without a valid
   invitation is refused. Confirm room closure after all members leave.

The live server needs only the application ID, public verification key and guild
policy. Runtime voice discovery additionally needs the private bot token. Request
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

Open **Admin → Discord → Overview** and select **Add bot to a Discord server**.
PodeTelar generates the Discord install link from the configured application ID.
The link requests server installation, slash commands, and View Channels and
Send Messages permissions for room announcements. With an explicit guild list,
add the new server's ID to `DISCORD_GUILD_IDS` and restart the app.

Global commands can take time to refresh in Discord clients. Legacy guild-scoped
commands may shadow the global commands in servers used for earlier testing; they
use the same handler. Remove only PodeTelar's own legacy commands when retiring
the old guild-specific registrations; preserve unrelated app commands.

Use a permanent HTTPS endpoint for production; stopping a development tunnel
makes its commands unavailable. Test actual Discord registration and native
admission as well as signed HTTP fixtures before rollout.

Primary references:
[Discord interactions](https://docs.discord.com/developers/interactions/receiving-and-responding)
and [application commands](https://docs.discord.com/developers/interactions/application-commands).

## Voice discovery and administration

With a runtime `DISCORD_BOT_TOKEN`, pinned discord.js connects using only Guilds
and GuildVoiceStates intents. No message-content or privileged member/presence
intent is needed. Install the actual bot user with access to intended channels.
All ordinary voice channels are discovered by default, without opt-in commands.
Guild snapshots initialize an in-memory index; ordered voice events update it.
Lookup does not scan every server when a desktop client connects. DMs, group-DM
calls and Stage channels are outside the initial integration.

The administrator dashboard Discord section shows connection/readiness, installed
allowed guilds, channels and member counts. Reconnect and resynchronize rebuilds
the index. Credentials and raw SDK errors never appear in dashboard responses.
An unavailable guild or disconnected transport means unknown presence, not a
confirmed departure. New Gateway sessions discard stale state. No voice history
is persisted. Targeted member REST requests support nickname/avatar resolution
without storing user OAuth refresh tokens.

Live qualification requires a real bot and Discord users: already-in-voice startup,
movement, departure, guild/transport outage and recovery, bot installation/removal
and dashboard reconnect. Deterministic tests do not qualify those external events.

## Channel rooms and permanent invitations

Each Discord guild/channel pair maps transactionally to at most one active room.
The room follows the existing 60-second empty grace. Closing it clears the mapping,
but its encrypted/hash-protected permanent channel invitation remains valid until
rotated or disabled. Opening a valid invitation creates the next room on demand.
A channel URL is `/c/GUILD/CHANNEL#invite=SECRET`; secrets are submitted in request
bodies, never HTTP URLs or browser storage. Native pasted links and browser app
handoffs both support these invitations. Merely viewing the landing page does not
create a room. Invitation joins remain independent of Discord voice membership,
including while the bot reconnects. The server can resolve previously known
channels from persisted metadata during outages.

The Discord administrator section manages permanent invitation copy/rotation and
disable/enable, automatic admission pause/resume, and announcement repair. These
controls are independent; disabling an invitation preserves established sessions.
The existing room directory can close the current room or manage its sessions.
An existing room admission lock cannot be bypassed through the channel resolver.

The bot posts or updates one message per channel when a room is first requested.
It updates that message when the room closes or its guest access changes. Links
are explicitly shareable guest capabilities and necessarily disclosed to Discord.
Message permission failures are visible in administration and retried with bounded
backoff; they do not prevent joining. Slash command replay protection and quotas
are retained, and unknown voice state does not create a competing fallback room.

Live Discord message delivery/permissions remain to be checked with a configured
bot. Automated tests cover permanent-invitation lifecycle and slash room reuse;
they do not qualify actual Discord installations or message delivery.
