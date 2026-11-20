# Native private invitations and media encryption

This phase protects audio, video and previews from an application/SFU operator
who does not hold the native private invitation. It is a capability-based design:
every holder of the private invitation is authorized. It does not verify a human
identity, and it does not remove trust in the client executable or its updates.

## Invitation flow

An organizer joins an existing room in the native application and chooses
**Create private invitation**. The application creates fresh 256-bit random
private material locally. The organizer distributes the new native invitation to
friends, who paste and join as before. Old server-issued invitations do not gain
protection retroactively, and joiners never create independent roots silently.

Guest-capable private invitations use the versioned opaque `FSPRIVATE1:` format.
Voice-only private codes use `PTVOICE1:`. Keep both in the native application and the trusted channel used
to invite friends. Never paste one into the room website. Website JavaScript can
read URL fragments; a fragment is not secret from the website operator.

The native application sends the separate admission credential to the API.
**Remember private invitation on this device** stores one private invitation
in Windows Credential Manager or macOS Keychain. The ordinary server, account
and nickname preferences contain no room key. Forget removes the saved entry.
Copying a private invitation puts it on the clipboard. Browser handoff, launcher
arguments and Discord bot announcements carry only ordinary admission material.

## Discord voice rooms

Discord sign-in and voice presence authorize access to a PodeTelar session.
The server controls that session and supplies the displayed account identity.
Neither proves possession of a media key. The native app generates that key
material independently and keeps it out of Discord OAuth, Gateway and bot traffic.

A single Discord channel can host a succession of PodeTelar rooms. When guest
access is enabled, a voice-admitted organizer can create a private invitation
using the current room's revocable guest handoff. The permanent channel link
continues to open whichever room the channel currently uses. A private invitation
is valid only for the original public server origin and concrete room.

When guest access is disabled the server withholds both guest links. An organizer
already admitted by voice chooses **Create private voice code**. It contains an
independent secret and binds the server origin, Discord guild and channel,
concrete room and generation. It contains no guest credential. Each friend
uses their own Discord login and voice membership for admission, then pastes the
code into **Use a private invitation for this room**. An ordinary guest cannot
use that code to join. Share either code through a channel trusted to carry it.
Anyone who can read it can use its keys if they obtain the ciphertext.

A saved private invitation lets a matching voice admission use encryption again.
Automatic admission cannot supply, replace or reset the private root. Changing
channels stops media, and a different concrete room needs its own private
invitation. A guest joining by invitation has independent admission. Discord voice
departure and login revocation govern sessions obtained through voice following.
A legacy participant can still publish their own unprotected stream in the same
server room. The native protection indicator describes this client's media.

## Cryptographic boundary

Use the pinned native LiveKit encryption machinery for media frames; do not
encrypt opaque access units before handing them to the codec-aware RTP path.
Derive separate authentication, media and preview material using versioned
domain separation, canonical expected server origin, room ID and private-room
generation. Each application media connection has a fresh random sender-session
identifier. Authenticate its descriptor, including transport identity, before
installing participant-specific SDK keys or admitting remote subscriptions.

In protected mode, missing or invalid descriptors and plaintext publications are
rejected. Configure encryption before publishing. Codec fallback, reconnect,
saved-room restore and source changes must retain this requirement. An encrypted
room setting alone is insufficient: the SDK skips cryptors on publications
marked as plaintext.

The pinned SDK patch also attaches receiver cryptors independently of remote
encryption metadata. Playback checks actual cryptor readiness before retaining
any pump, so changing a publication label cannot turn plaintext into protected
content. The SDK discards the server-injected frame trailer at connection setup.

The pinned SDK installs encryption directly in the receiver attachment path,
before enabling or exposing the track. Protection does not depend on a removable
notification callback, including if a publication is retired during attachment.
Attachments for one publication are also serialized. Playback uses one captured
track for readiness and pump creation. A stale map entry for another same-SID
receiver cannot turn an untransformed receiver into protected content.

A protected native peer must acknowledge that its WebRTC Call was created with
the receive guard. At construction of every audio and video receive stream,
including unsignaled streams, that guard installs a transform that drops all
frames unless an enforcing receiver cryptor is already registered. This prevents
plain media from entering NetEQ, the video frame buffer or a decoder while
the SDK waits for track metadata. Once a verified cryptor replaces the initial
guard, media takes the normal encrypted path. Frame header, IV and tag bounds
are checked before the upstream decryptor parses the frame.

For a full media reconnect, the client requests a fresh media grant with its
existing room session and creates a fresh sender session and key provider.
This preserves Discord voice control and the backend's ability to terminate
that session. Reconnection restores watch intent and leaves sharing stopped.

The native C++ wrapper enables cryptors before attaching them and keeps attached
cryptors enabled even when SDK cleanup requests disablement. Encryption cannot be
toggled off on an existing protected transport; leaving closes that connection.
This prevents plaintext windows during reattachment and cleanup. Protected
streams do not install the optional standalone packet-timing transformer or
advertise its trailers. Legacy transports do not create frame cryptors and retain
their existing behavior.

The SDK's standalone packet-timing helpers are private to the SDK. Native audio
receivers start muted, and the playback controller grants speaker output only
to a watched, authorized publication. It closes output on replacement or stop.

The version 1 key schedule remains compatible with the initial PR, including
its original protocol labels after the application rename. The client performs
HKDF extraction once per parsed secret and keeps the descriptor authentication
key for the connection. Repeated valid descriptors use a bounded per-connection
cache. Each new sender session has its own media key. Reconciliation looks up
cryptor readiness for the selected track without copying the whole SDK map.

The native WebRTC build uses the pinned LiveKit AES-GCM implementation with a
shared sender nonce allocator. Audio, video and SDK data encryption draw from
one process-wide sequence. Threads reserve blocks, so ordinary frames need
neither a random-number call nor a shared counter update. A fresh random offset
and a reversible encoding produce 12-byte nonces without zero bytes. The
sequence survives track replacement, codec changes and Stop/Start. Exhaustion
or failure to initialize the random offset stops encryption. New processes and
full media reconnects establish independent sender keys.

H.264 and HEVC use emulation-prevention bytes for RTP packetization. The receiver
removes them before reading the nonce and authenticating the ciphertext. It
accepts older valid encrypted frames whose nonce itself needed escaping. The
pinned codec parser uses an 8-bit clear-prefix length. Encoder output with more
than 255 bytes of leading codec headers is outside the tested compatibility set.

Protected publishers use hardware HEVC or H.264. These codecs preserve the headers
needed for RTP packetization. The pinned cryptor encrypts all AV1 OBU headers,
which the AV1 RTP packetizer needs to frame the stream. Private clients exclude
AV1 from publication, decoder capabilities and subscriptions. A saved AV1 preference
selects HEVC or H.264 according to available hardware. Transport-only rooms retain
hardware AV1. [Pinned AV1 RTP packetizer](https://github.com/webrtc-sdk/webrtc/blob/a545de4/modules/rtp_rtcp/source/rtp_packetizer_av1.cc)

Protected publishers use codec capabilities and bandwidth limits from peers with
verified private sender descriptors. When another private participant arrives, its advertised hardware
capabilities join the existing codec selection policy. These signaling values
are quality hints and can be altered by the server. Legacy rooms retain the
all-participant policy.

Previews use separate authenticated encryption. The native sender encrypts its
bounded JPEG before uploading. The server stores only an opaque bounded `FSP1`
envelope under `application/vnd.freedomstream.preview-v1`; it cannot authenticate
or inspect the image. The receiver authenticates sender/room/stream context,
decrypts, validates dimensions and only then decodes. Server-reported dimensions
are layout hints, not trusted image dimensions. Latest-only storage, expiry,
publication proof, room access and the start/once-per-minute cadence still apply.

## Deliberate limits

- A private-invitation holder can derive keys, impersonate display names, record
  content or redistribute it. A server-issued token without the root is not
  sufficient to decrypt protected content.
- Server session removal and admission-invitation rotation are not cryptographic
  revocation of a private-root holder against a malicious server. Excluding a
  leaked root requires independent new private material distributed only to the
  remaining trusted members. Deriving another key from the old root cannot do it.
- This design does not claim forward secrecy, post-compromise security, verified
  devices, authenticated group key agreement, complete replay protection or proof
  that received content is currently live. Previously valid descriptors and
  ciphertext can be replayed by an operator to a fresh client.
- The operator can deny service and still observes signaling, room/participant/track
  information, packetization headers needed for the codec, and traffic patterns.
  Application secrets must stay out of
  metadata, HTTP URLs, diagnostics, argv, browser storage or committed fixtures.
- Existing browser/native invitations cannot be transparently upgraded without
  the organizer distributing new private invitation material.

## Review decision and qualification

The independent pre-implementation GPT-6.1-Sol high-reasoning review returned
**GO WITH CONDITIONS** for the boundary above, and **NO GO** for claims of complete
operator independence, malicious-server-resistant member revocation or verified
human identities. A separate review of the finished implementation is required.

Real-SFU checks must establish correct-root audio/video/preview playback,
wrong/missing-root rejection with valid admission, plaintext and descriptor
tampering rejection, reconnect/republish/fallback, concurrent sender sessions and
actual Stop/rewatch traffic. Unit tests must cover cross-context substitution,
versions, authentication and decode bounds. Qualify H.264, HEVC and AV1 separately.
Record OS credential save/reopen/Forget and exact runtime provenance. Record unavailable
checks honestly; compilation and mocked media are not substitutes.

CPU impact at approximately 35 Mbps requires an encryption-on/off comparison in
the actual application, including multiple subscriptions and actual delivered
bitrate. An encoder target or isolated AES benchmark is not a media performance
result. Physical AMD, gaming, device/network timing and fresh-machine gates remain
separate from local automated results.

## Primary references

- [LiveKit native encryption configuration](https://docs.livekit.io/transport/encryption/start/)
- [LiveKit encryption boundary and key distribution](https://docs.livekit.io/transport/encryption/)
- [Pinned SDK cryptor manager](https://github.com/livekit/rust-sdks/blob/88705b15086ecbe2c26ee91ede6203f974a0fc62/livekit/src/room/e2ee/manager.rs)
- [Pinned WebRTC frame encryption implementation](https://github.com/webrtc-sdk/webrtc/blob/a545de4/api/crypto/frame_crypto_transformer.cc)
