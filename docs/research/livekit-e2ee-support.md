# LiveKit native E2EE: support and qualification limits

Research date: 2026-11-10. Research only; no media tests or application changes.
Local dependency context: `apps/publisher/Cargo.toml` pins `livekit =0.10.1`;
the workspace patches `livekit` and `webrtc-sys` from `.local/native-sdk`.
The local LiveKit package records upstream commit
`88705b15086ecbe2c26ee91ede6203f974a0fc62`. Upstream evidence below does not
certify those local patches or the shipped binary.

## Official support and encryption boundary

**Documented API:** LiveKit's Rust example uses
`KeyProvider::with_shared_key(KeyProviderOptions::default(), key_bytes)`,
`RoomOptions.encryption = Some(E2eeOptions { key_provider,
encryption_type: EncryptionType::Gcm })`, and
`room.e2ee_manager().set_enabled(true)`. This is official native Rust support,
not a browser-only feature. [Rust implementation guide](https://docs.livekit.io/transport/encryption/start/).

**Source evidence:** the pinned manager installs AES-GCM frame cryptors on
published/subscribed tracks, skipping publications marked `EncryptionType::None`.
Configuration is per client; this is not evidence of a server-enforced
encrypted-only room. Applications requiring that policy must reject plaintext
publications and avoid disabling encryption. [Pinned manager](https://github.com/livekit/rust-sdks/blob/88705b15086ecbe2c26ee91ede6203f974a0fc62/livekit/src/room/e2ee/manager.rs).

**Boundary:** enabled E2EE protects media content from the SFU and is available
self-hosted and in Cloud. Signaling and API requests are TLS protected in transit,
but readable by the server. Room/participant/track information and traffic patterns
are therefore not made private by media E2EE. Current docs also distinguish
`RoomOptions.encryption` (media plus outgoing data messages) from deprecated
`e2ee` (does not enable data-channel encryption). HTTPS/WSS and room admission
remain necessary. [Encryption overview](https://docs.livekit.io/transport/encryption/).

## Custom preencoded video

**Documented API:** Rust preencoded publishing exists from `livekit` 0.7.53:
`NativeVideoSource::new_encoded`, `capture_encoded_frame`, and
`VideoEncoderBackend::PreEncoded`. Submit complete access units with codec,
key/delta type, capture timestamp and dimensions. Source and publication codecs
must match and remain fixed for that source. The API lists H.264, H.265 and AV1;
it does not establish that every codec works with every E2EE receiver. Use one
spatial layer, disable simulcast, and propagate keyframe/rate-control requests to
the external encoder. H.264 examples use Annex B with SPS/PPS at keyframes.
[Official preencoded guide](https://docs.livekit.io/transport/media/pre-encoded/).

**Source evidence:** the native cryptor is attached between encoder and RTP
packetizer, with decryption between depacketizer and decoder. Thus a custom
encoder using the supported source can enter the normal cryptor path; do not
preencrypt access units and assume the packetizer can interpret them.
This is an architectural inference, not a successful interoperability test.
[Native cryptor wiring](https://github.com/livekit/rust-sdks/blob/88705b15086ecbe2c26ee91ede6203f974a0fc62/webrtc-sys/src/frame_cryptor.cpp).

| Codec | Native source evidence | Qualification still required |
| --- | --- | --- |
| H.264 | NAL-aware clear prefix and RBSP escaping/unescaping | Actual external encoder bitstream, profile, fragmentation and receiver decode with E2EE |
| H.265 / HEVC | Explicit NAL-aware handling, guarded by `RTC_ENABLE_H265` | Exact build flag, parameter sets, packetization and each receiver |
| AV1 | Explicit AV1 branch with zero clear frame-prefix bytes | Encrypted AV1 packetization/depacketization, negotiated RTP extensions and native decode |

The codec evidence is from LiveKit-authored
[frame cryptor source at `a545de4`](https://github.com/webrtc-sdk/webrtc/blob/a545de4/api/crypto/frame_crypto_transformer.cc).
It also shows that selected codec headers remain clear; E2EE does not mean every
transport byte is opaque. The Rust SDK's
[pinned build helper](https://github.com/livekit/rust-sdks/blob/88705b15086ecbe2c26ee91ede6203f974a0fc62/webrtc-sys/build/src/lib.rs)
selects `webrtc-a545de4` by default but permits `LK_CUSTOM_WEBRTC` overrides.
Source support is not binary provenance or end-to-end compatibility evidence.

## Authenticated key distribution

**Documented responsibility:** LiveKit does not distribute E2EE keys; the
application must generate, store and deliver them securely. Its shared-key example
allows an application backend to deliver a key alongside a room access token.
That design trusts that backend with plaintext keys and media access; it does not
hide media from a compromised key-distribution backend.
[Key distribution](https://docs.livekit.io/transport/encryption/#key-distribution).

**Available primitives:** Rust exposes shared and participant-identity keys,
key indices, ratcheting, PBKDF2/HKDF selection and key-ring options. These APIs do
not themselves authenticate a participant's key or implement an authenticated
group key agreement. [Pinned key provider](https://github.com/livekit/rust-sdks/blob/88705b15086ecbe2c26ee91ede6203f974a0fc62/livekit/src/room/e2ee/key_provider.rs).

**Design implications, not implemented claims:** authenticate room membership
before releasing any key; bind key delivery to the intended room, participant
and key epoch. If the application server must also be excluded, use an independently
authenticated client key-distribution protocol rather than trusting server-provided
identity/key mappings. Shared-key holders can decrypt and generate content under
that key, so AES-GCM authentication alone does not prove an individual sender's
identity. Revocation requires fresh keys withheld from removed members; admission
revocation alone cannot erase a key already learned. Ratcheting APIs alone do not
establish forward secrecy or post-compromise security. Keep keys out of signaling
metadata, logs, server-bound URLs and browser local storage.

## Evidence needed before claiming compatibility

No H.264, HEVC or AV1 E2EE combination was run for this note. Qualify the exact
portable runtime through the real SFU with native sender/receiver: correct-key
playback, wrong/missing-key rejection, no plaintext fallback, late join, reconnect,
key rotation/removal and codec fallback. Verify audio separately, plus encrypted
data/preview paths if those are claimed. Ordinary decode tests, compilation and
unencrypted transport checks cannot satisfy these gates. HTTP preview uploads
do not acquire E2EE merely because the media room uses it.
