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

Protected invitations use the versioned opaque `FSPRIVATE1:` format, rather than
an HTTPS link. Keep them in the native application and the trusted channel used
to invite friends. Never paste one into the room website. Website JavaScript can
read URL fragments; a fragment is not secret from the website operator.

The native application sends only the separate legacy admission credential to
the API. The private root stays local, including in the existing Windows DPAPI
saved-room envelope. Forget saved room removes that saved authority. Copying an
invitation deliberately puts the secret in the user's clipboard, just as sharing
any bearer invitation does.

The server-mediated browser handoff in PR #20 is for legacy invitations only. It
must never transport a protected invitation or its root. Legacy rooms remain
explicitly unprotected; clients must not infer protection from server metadata.

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
content. The native client clears server-provided injected-frame bypass material
before subscriptions.

The pinned SDK installs encryption directly in the receiver attachment path,
before enabling or exposing the track. Protection does not depend on a removable
notification callback, including if a publication is retired during attachment.
Attachments for one publication are also serialized. Playback uses one captured
track for readiness and pump creation. A stale map entry for another same-SID
receiver cannot turn an untransformed receiver into protected content.

The native C++ wrapper enables cryptors before attaching them and keeps attached
cryptors enabled even when SDK cleanup requests disablement. Encryption cannot be
toggled off on an existing protected transport; leaving closes that connection.
This prevents plaintext windows during reattachment and cleanup. Protected
streams do not install the optional standalone packet-timing transformer or
advertise its trailers. Legacy transports do not create frame cryptors and retain
their existing behavior.

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
- The operator still observes signaling, room/participant/track information and
  traffic patterns, and can deny service. Application secrets must not enter
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
Record DPAPI save/reopen/Forget and exact runtime provenance. Record unavailable
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
