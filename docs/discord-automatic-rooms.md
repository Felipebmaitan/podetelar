# Discord automatic rooms

Sign in from the desktop and leave it open. With the bot connected, joining an
ordinary voice channel automatically admits the app to that channel's room.
All voice channels are enabled by default; administrators can pause individual
channels, users, or the whole backend from **Discord** in the dashboard. Stage
channels are excluded. AFK and private ordinary voice channels follow the same
default; pause them in the dashboard if desired. No bot commands configure this.

Each `(guild, channel)` maps to one active room, created only when needed. SQLite
serializes creation. Empty rooms keep the existing 60-second lifecycle; closing a
room clears its pointer while preserving the channel invitation and policy.

Voice admission requires a current PodeTelar Discord login and the bot's current
voice membership. It issues a separate room session tagged with the login, user,
guild and channel. Invitations never acquire this tag, even for signed-in users.
Leaving voice or moving channels revokes only tagged sessions immediately when
the Gateway event is observed. Existing signaling and durable SFU removal enforce
that revocation; unreachable SFUs retain the documented pending-removal limitation.
Login revocation and policy changes also end tagged sessions.

The desktop checks its authenticated voice location every two seconds while it
sees an eligible voice channel, and every ten seconds otherwise. Failed checks
back off and honor the server's retry delay. Moving A to B cancels capture, tears
down the old room and all subscriptions, then joins B
without confirmation. Sharing and watching stay stopped. An outage marks voice
state unknown: existing sessions remain connected, new automatic admissions pause,
and fresh snapshots reconcile membership after recovery. This also means an
unobserved departure cannot be enforced until Discord recovers.

**Leave room** pauses automatic joining until the observed voice channel changes
or **Resume automatic joining** is selected in user settings. Joining through a
URL explicitly selects independent access and pauses following for that room.
The existing invitation confirmation still applies when switching active rooms
using a URL. Multiple simultaneous Discord voice locations require selecting the
channel in settings. The bot's in-memory index avoids scanning guilds per login.

The permanent channel guest invitation opens the current room independently of
voice, and works during a bot outage once the channel is known. It is deliberately
a bearer invitation: everyone who receives or forwards it can join. Administrators
can rotate or disable it. Disabling automatic joining does not disable invitations.
The channel announcement updates one message with the permanent invitation;
`/stream` and `/telar` resolve the same channel room without requiring app login.

The dashboard lists voice-controlled sessions and blocked users, provides global
pause/resume and per-user pause/allow, and keeps channel policies and invitations
alongside the bot/account controls. Pausing ends current controlled sessions;
independent invitation sessions use the existing room access controls.

API tests use synthetic Gateway events and local media doubles to verify
membership, channel moves, outages, independent URL sessions and administrator
policy. Native state tests cover suppression, selection and no media start action.
The `native:room-smoke -- discord-voice.spec.ts` acceptance uses synthetic voice
events with actual HTTP, browser WebRTC peers and a real local Windows SFU. It
checks removal of the controlled peer and continued independent guest access.
Real Discord OAuth/Gateway events across two users and actual hardware capture
must still be checked on a configured deployment before rollout. No user OAuth
refresh token, privileged members intent or message-content intent is needed.
