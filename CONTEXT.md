# PodeTelar

A private place for friends to share screens and cameras and choose what to watch together.

## Language

**Room**:
A private gathering place that stays open while friends are connected, including when nobody is sharing. It awaits its first join, then closes after everyone leaves, allowing a short reconnect grace period.
_Avoid_: Stream, call

**Invitation**:
A shareable secret that admits its holder to one room.
_Avoid_: Room name, room code, password

**Participant**:
A person who has joined a room for a session, whether sharing, watching, or neither.
_Avoid_: Account, host

**Session**:
One browser or native publisher's time-limited authority to join a room. A person can have several sessions; ending one does not ban them from a still-valid invitation.
_Avoid_: Account, device identity

**Publisher**:
A participant contributing a live screen or camera stream.
_Avoid_: Room owner, host

**Subscription**:
A participant's explicit choice to receive one published stream's full video and audio.
_Avoid_: Room membership, visible tile

**Preview**:
A compressed still image used to identify an unwatched live stream.
_Avoid_: Subscription, low-quality live stream

**Server**:
The backend where a room lives. Each invitation identifies its server.
