//! Invitation-holder confidentiality. This is not human identity verification,
//! forward secrecy, replay protection, or revocation of a known invitation root.
use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use livekit::{
    Room, RoomOptions,
    e2ee::{
        E2eeOptions, EncryptionType,
        key_provider::{KeyDerivationAlgorithm, KeyProvider, KeyProviderOptions},
    },
};
use ring::{
    aead, hkdf, hmac,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Mutex};
use zeroize::{Zeroize, Zeroizing};

pub const PREFIX: &str = "FSPRIVATE1:";
pub const ATTRIBUTE: &str = "freedomstream.e2ee.v1";
pub const PREVIEW_TYPE: &str = "application/vnd.freedomstream.preview-v1";
pub const MAX_PREVIEW: usize = 266_240;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InvitationEnvelope {
    version: u8,
    invitation: String,
    root: String,
    generation: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    const A: &str = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
    const B: &str = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";

    fn legacy() -> String {
        format!(
            "https://example.test/r/{}#invite={}",
            "r".repeat(22),
            "s".repeat(43)
        )
    }
    fn secret(invite: &str) -> RoomSecret {
        unwrap_invitation(invite).unwrap().1.unwrap()
    }
    fn root(invite: &str) -> String {
        serde_json::from_slice::<InvitationEnvelope>(&B64.decode(&invite[PREFIX.len()..]).unwrap())
            .unwrap()
            .root
    }
    fn descriptor(session: &Session) -> String {
        serde_json::to_string(&session.local).unwrap()
    }

    #[test]
    fn opaque_invitation_roundtrip_and_versions_are_strict() {
        let invite = protect_invitation(&legacy()).unwrap();
        assert!(invite.starts_with(PREFIX));
        assert!(!invite.contains("https://"));
        assert!(crate::admission::valid_invitation(&invite));
        assert_eq!(unwrap_invitation(&invite).unwrap().0, legacy());
        assert!(unwrap_invitation(&invite.replace("FSPRIVATE1", "FSPRIVATE2")).is_err());
        assert!(protect_invitation(&invite).is_err());
        assert!(unwrap_invitation("FSPRIVATE1:malformed").is_err());
        let replaced = replace_invitation(&invite).unwrap();
        assert!(root(&invite) != root(&replaced));
        assert_eq!(unwrap_invitation(&replaced).unwrap().0, legacy());
        assert!(replace_invitation(&legacy()).is_err());
    }

    #[test]
    fn v1_keys_match_independent_hkdf_sha256_vectors() {
        // Public synthetic vector, calculated independently with Python's hmac.digest.
        let envelope = InvitationEnvelope {
            version: 1,
            invitation: legacy(),
            root: B64.encode([0x11; 32]),
            generation: B64.encode([0x22; 32]),
        };
        let invite = format!(
            "{PREFIX}{}",
            B64.encode(serde_json::to_vec(&envelope).unwrap())
        );
        let receiver = Session::new(secret(&invite), B.into()).unwrap();
        let descriptor = Descriptor {
            version: 1,
            identity: A.into(),
            session: B64.encode([0x33; 32]),
            tag: "auLN2U58rWnk7K4jJ3wwPxkbOyD2YH9hJBXTat15N4g".into(),
        };
        assert!(receiver.authorize(A, Some(&serde_json::to_string(&descriptor).unwrap())));
        assert_eq!(
            B64.encode(receiver.secret.media_key(&descriptor).unwrap().as_ref()),
            "VchIoursuUF_9-9qvHj_2wjUIwH0yO_HblJ0_7J3gdI"
        );
        assert_eq!(
            B64.encode(
                receiver
                    .preview_key(&descriptor, "TR_fixture")
                    .unwrap()
                    .0
                    .as_ref()
            ),
            "8_q1aYwg9Sk3MrOohMd5i463XtTvZ4tmZEADMjDCrKs"
        );
    }

    #[test]
    fn descriptors_authenticate_context_identity_version_and_session() {
        let invite = protect_invitation(&legacy()).unwrap();
        let publisher = Session::new(secret(&invite), A.into()).unwrap();
        let receiver = Session::new(secret(&invite), B.into()).unwrap();
        let raw = descriptor(&publisher);
        assert!(!publisher.authorize(A, Some(&raw)));
        assert!(receiver.authorize(A, Some(&raw)));
        assert!(!receiver.authorize(B, Some(&raw)));
        assert!(!receiver.authorize(A, None));
        let mut forged = publisher.local.clone();
        forged.session = B64.encode([0_u8; 32]);
        assert!(!receiver.authorize(A, Some(&serde_json::to_string(&forged).unwrap())));
        forged = publisher.local.clone();
        forged.version = 2;
        assert!(!receiver.authorize(A, Some(&serde_json::to_string(&forged).unwrap())));
        assert!(!receiver.authorize(A, Some(&"x".repeat(1025))));
        let another = protect_invitation(&legacy()).unwrap();
        assert!(
            !Session::new(secret(&another), B.into())
                .unwrap()
                .authorize(A, Some(&raw))
        );
        let bytes = B64.decode(&invite[PREFIX.len()..]).unwrap();
        let mut envelope: InvitationEnvelope = serde_json::from_slice(&bytes).unwrap();
        for replacement in [
            legacy().replace("example.test", "other.test"),
            legacy().replace(&"r".repeat(22), &"q".repeat(22)),
        ] {
            envelope.invitation = replacement;
            let changed = format!(
                "{PREFIX}{}",
                B64.encode(serde_json::to_vec(&envelope).unwrap())
            );
            assert!(
                !Session::new(secret(&changed), B.into())
                    .unwrap()
                    .authorize(A, Some(&raw))
            );
        }
    }

    #[test]
    fn reconnects_have_independent_media_keys_and_preview_domains() {
        let invite = protect_invitation(&legacy()).unwrap();
        let first = Session::new(secret(&invite), A.into()).unwrap();
        let next = Session::new(secret(&invite), A.into()).unwrap();
        assert!(first.local.session != next.local.session);
        let first_key = first.secret.media_key(&first.local).unwrap();
        assert!(first_key.as_ref() != next.secret.media_key(&next.local).unwrap().as_ref());
        assert!(
            first_key.as_ref()
                != first
                    .preview_key(&first.local, "TR_fixture")
                    .unwrap()
                    .0
                    .as_ref()
        );
    }

    #[test]
    fn preview_aead_rejects_tamper_wrong_sender_track_and_plaintext() {
        let invite = protect_invitation(&legacy()).unwrap();
        let publisher = Session::new(secret(&invite), A.into()).unwrap();
        let receiver = Session::new(secret(&invite), B.into()).unwrap();
        let raw = descriptor(&publisher);
        assert!(receiver.authorize(A, Some(&raw)));
        let plain = vec![0xff, 0xd8, 0xff, 0xd9];
        let encrypted = publisher.seal_preview("TR_fixture", plain.clone()).unwrap();
        assert_eq!(
            receiver
                .open_preview(A, "TR_fixture", encrypted.clone())
                .unwrap(),
            plain
        );
        assert!(
            receiver
                .open_preview(A, "TR_other", encrypted.clone())
                .is_err()
        );
        assert!(
            receiver
                .open_preview(B, "TR_fixture", encrypted.clone())
                .is_err()
        );
        assert!(
            receiver
                .open_preview(A, "TR_fixture", plain.clone())
                .is_err()
        );
        let mut tampered = encrypted.clone();
        *tampered.last_mut().unwrap() ^= 1;
        assert!(receiver.open_preview(A, "TR_fixture", tampered).is_err());
        assert!(receiver.authorize(A, Some(&raw)));
        assert!(!receiver.authorize(A, None));
        assert!(
            receiver
                .open_preview(A, "TR_fixture", encrypted.clone())
                .is_err()
        );
        assert!(receiver.authorize(A, Some(&raw)));
        assert!(!receiver.authorize(A, Some(&format!("{raw}junk"))));
        assert!(
            receiver
                .open_preview(A, "TR_fixture", encrypted.clone())
                .is_err()
        );
        let again = publisher.seal_preview("TR_fixture", plain).unwrap();
        assert_ne!(&encrypted[4..16], &again[4..16]);
        assert!(
            publisher
                .seal_preview("TR_fixture", vec![0; 262145])
                .is_err()
        );
        assert!(
            receiver
                .open_preview(A, "TR_fixture", vec![0; MAX_PREVIEW + 1])
                .is_err()
        );
    }

    #[test]
    fn descriptors_do_not_disclose_root_and_known_root_is_not_revocable_by_server() {
        let invite = protect_invitation(&legacy()).unwrap();
        let sender = Session::new(secret(&invite), A.into()).unwrap();
        let root_text = root(&invite);
        assert!(!descriptor(&sender).contains(&root_text));
        // The capability model deliberately does not bind to server admission state:
        // a holder of the root remains capable until independent root replacement.
        let other_holder = Session::new(secret(&invite), B.into()).unwrap();
        assert!(other_holder.authorize(A, Some(&descriptor(&sender))));
    }
}

pub struct RoomSecret {
    prk: hkdf::Prk,
    context: Vec<u8>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Descriptor {
    version: u8,
    identity: String,
    session: String,
    tag: String,
}

/// Per-connection participant keys. No secret material implements Debug.
pub struct Session {
    secret: RoomSecret,
    local: Descriptor,
    keys: KeyProvider,
    descriptor_auth: hmac::Key,
    // Keep the same lifetime bound as the SDK's installed participant keys.
    // An empty slot denies previews after a descriptor is withdrawn or rejected.
    verified: Mutex<HashMap<String, Option<VerifiedSender>>>,
}

struct VerifiedSender {
    wire: String,
    descriptor: Descriptor,
}

fn random<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0; N];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| anyhow::anyhow!("Secure random generation failed."))?;
    Ok(bytes)
}

fn decode<const N: usize>(value: &str) -> Result<[u8; N]> {
    let bytes = B64
        .decode(value)
        .map_err(|_| anyhow::anyhow!("Invalid protected invitation or descriptor."))?;
    ensure!(B64.encode(&bytes) == value, "Invalid protected encoding.");
    bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid protected invitation or descriptor."))
}

fn framed(parts: &[&[u8]]) -> Vec<u8> {
    let mut result = Vec::new();
    for part in parts {
        result.extend_from_slice(&(part.len() as u32).to_be_bytes());
        result.extend_from_slice(part);
    }
    result
}

pub fn protect_invitation(legacy: &str) -> Result<String> {
    ensure!(
        !legacy.starts_with(PREFIX),
        "This invitation is already protected."
    );
    // Validate before creating an opaque wrapper. Admission is never derived from root.
    crate::admission::invitation_context(legacy)?;
    let envelope = InvitationEnvelope {
        version: 1,
        invitation: legacy.trim().to_owned(),
        root: B64.encode(random::<32>()?),
        generation: B64.encode(random::<32>()?),
    };
    let encoded = serde_json::to_vec(&envelope)
        .map_err(|_| anyhow::anyhow!("Could not create private invitation."))?;
    let result = format!("{PREFIX}{}", B64.encode(encoded));
    ensure!(result.len() <= 2048, "Protected invitation is too long.");
    Ok(result)
}

/// A replacement root is independent; derived rotation cannot exclude an old holder.
pub fn replace_invitation(input: &str) -> Result<String> {
    let (legacy, protection) = unwrap_invitation(input.trim())?;
    ensure!(
        protection.is_some(),
        "Enter a private invitation to replace."
    );
    protect_invitation(&legacy)
}

/// Extract the legacy admission only inside the trusted native process.
pub fn unwrap_invitation(input: &str) -> Result<(String, Option<RoomSecret>)> {
    if !input.starts_with(PREFIX) {
        ensure!(
            !input.starts_with("FSPRIVATE"),
            "Unsupported protected invitation version."
        );
        return Ok((input.to_owned(), None));
    }
    ensure!(input.len() <= 2048, "Protected invitation is too long.");
    let bytes = Zeroizing::new(
        B64.decode(&input[PREFIX.len()..])
            .map_err(|_| anyhow::anyhow!("Invalid protected invitation."))?,
    );
    let mut envelope: InvitationEnvelope = serde_json::from_slice(&bytes)
        .map_err(|_| anyhow::anyhow!("Invalid protected invitation."))?;
    ensure!(
        envelope.version == 1 && !envelope.invitation.starts_with("FSPRIVATE"),
        "Unsupported protected invitation."
    );
    let root = Zeroizing::new(decode::<32>(&envelope.root)?);
    let generation = decode::<32>(&envelope.generation)?;
    let (origin, room) = crate::admission::invitation_context(&envelope.invitation)?;
    let context = framed(&[
        b"FreedomStream private room v1",
        origin.as_bytes(),
        room.as_bytes(),
        &generation,
    ]);
    let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, &context).extract(root.as_ref());
    envelope.root.zeroize();
    Ok((envelope.invitation, Some(RoomSecret { prk, context })))
}

impl RoomSecret {
    fn derive(&self, label: &[u8], extra: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
        struct Len;
        impl hkdf::KeyType for Len {
            fn len(&self) -> usize {
                32
            }
        }
        let info = framed(&[b"FreedomStream protected v1", label, extra]);
        let infos = [&info[..]];
        let okm = self
            .prk
            .expand(&infos, Len)
            .map_err(|_| anyhow::anyhow!("Protected key derivation failed."))?;
        let mut key = Zeroizing::new([0; 32]);
        okm.fill(key.as_mut())
            .map_err(|_| anyhow::anyhow!("Protected key derivation failed."))?;
        Ok(key)
    }

    fn descriptor_input(&self, d: &Descriptor) -> Result<Vec<u8>> {
        ensure!(
            d.version == 1
                && d.identity.len() == 36
                && d.identity
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() || b == b'-'),
            "Invalid protected sender."
        );
        let session = decode::<32>(&d.session)?;
        Ok(framed(&[
            &self.context,
            b"sender descriptor v1",
            d.identity.as_bytes(),
            &session,
        ]))
    }

    fn media_key(&self, d: &Descriptor) -> Result<Zeroizing<[u8; 32]>> {
        self.derive(b"media sender", &self.descriptor_input(d)?)
    }
}

impl Session {
    pub fn new(secret: RoomSecret, identity: String) -> Result<Self> {
        let mut local = Descriptor {
            version: 1,
            identity,
            session: B64.encode(random::<32>()?),
            tag: String::new(),
        };
        let key = secret.derive(b"descriptor authentication", b"")?;
        let auth = hmac::Key::new(hmac::HMAC_SHA256, key.as_ref());
        local.tag = B64.encode(hmac::sign(&auth, &secret.descriptor_input(&local)?).as_ref());
        let keys = KeyProvider::new(KeyProviderOptions {
            key_derivation_algorithm: KeyDerivationAlgorithm::HKDF,
            failure_tolerance: -1,
            ratchet_window_size: 0,
            ..Default::default()
        });
        ensure!(
            keys.set_key(
                &local.identity.clone().into(),
                0,
                secret.media_key(&local)?.to_vec()
            ),
            "Could not install protected sender key."
        );
        Ok(Self {
            secret,
            local,
            keys,
            descriptor_auth: auth,
            verified: Mutex::default(),
        })
    }

    pub fn configure(&self, options: &mut RoomOptions) {
        options.encryption = Some(E2eeOptions {
            encryption_type: EncryptionType::Gcm,
            key_provider: self.keys.clone(),
        });
    }

    pub fn verify_local_publication(
        room: &Room,
        publication: &livekit::prelude::LocalTrackPublication,
    ) -> Result<()> {
        if room.e2ee_manager().encryption_type() != EncryptionType::None {
            ensure!(
                room.e2ee_manager().enabled()
                    && publication.encryption_type() == EncryptionType::Gcm,
                "Protected media publication was downgraded."
            );
            ensure!(
                room.e2ee_manager()
                    .frame_cryptor_enabled(room.local_participant().identity(), publication.sid()),
                "Protected media encryption was not attached."
            );
        }
        Ok(())
    }

    pub async fn announce(&self, room: &Room) -> Result<()> {
        // A malicious SFU must not nominate an unencrypted server-injected-frame
        // sentinel that bypasses the receiver's cryptor.
        self.keys.set_sif_trailer(Vec::new());
        ensure!(
            room.e2ee_manager().enabled()
                && room.e2ee_manager().encryption_type() == EncryptionType::Gcm
                && room.local_participant().identity().as_str() == self.local.identity,
            "Protected room identity mismatch."
        );
        let descriptor = serde_json::to_string(&self.local)
            .map_err(|_| anyhow::anyhow!("Could not announce protected sender."))?;
        tokio::time::timeout(
            std::time::Duration::from_secs(3),
            room.local_participant()
                .set_attributes(HashMap::from([(ATTRIBUTE.into(), descriptor)])),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Protected sender announcement timed out."))?
        .map_err(|_| anyhow::anyhow!("Could not announce protected sender."))?;
        Ok(())
    }

    pub fn authorize(&self, identity: &str, raw: Option<&String>) -> bool {
        // Remote metadata must never replace this connection's own sender slot.
        if identity == self.local.identity {
            return false;
        }
        let Ok(mut verified) = self.verified.lock() else {
            return false;
        };
        let current = verified.get(identity).and_then(Option::as_ref);
        let raw = raw.filter(|raw| raw.len() <= 1024);
        if let (Some(raw), Some(current)) = (raw, current)
            && current.wire == *raw
        {
            return true;
        }
        let result = raw.and_then(|raw| {
            let descriptor: Descriptor = serde_json::from_str(raw).ok()?;
            if descriptor.identity != identity {
                return None;
            }
            hmac::verify(
                &self.descriptor_auth,
                &self.secret.descriptor_input(&descriptor).ok()?,
                &decode::<32>(&descriptor.tag).ok()?,
            )
            .ok()?;
            if current.is_none_or(|sender| sender.descriptor != descriptor) {
                if verified.len() >= 256 && !verified.contains_key(identity) {
                    return None;
                }
                let key = self.secret.media_key(&descriptor).ok()?;
                if !self
                    .keys
                    .set_key(&identity.to_owned().into(), 0, key.to_vec())
                {
                    return None;
                }
            }
            Some(VerifiedSender {
                wire: raw.clone(),
                descriptor,
            })
        });
        if let Some(sender) = result {
            verified.insert(identity.to_owned(), Some(sender));
            true
        } else {
            if let Some(previous) = verified.get_mut(identity) {
                *previous = None;
            }
            false
        }
    }

    fn preview_key(
        &self,
        descriptor: &Descriptor,
        sid: &str,
    ) -> Result<(Zeroizing<[u8; 32]>, Vec<u8>)> {
        let aad = framed(&[
            &self.secret.descriptor_input(descriptor)?,
            b"screen preview v1",
            sid.as_bytes(),
        ]);
        Ok((self.secret.derive(b"preview AEAD", &aad)?, aad))
    }

    pub fn seal_preview(&self, sid: &str, bytes: Vec<u8>) -> Result<Vec<u8>> {
        ensure!(bytes.len() <= 262_144, "Preview exceeds its size limit.");
        let (key, aad) = self.preview_key(&self.local, sid)?;
        let key = aead::LessSafeKey::new(
            aead::UnboundKey::new(&aead::AES_256_GCM, key.as_ref())
                .map_err(|_| anyhow::anyhow!("Preview encryption unavailable."))?,
        );
        let nonce = random::<12>()?;
        let mut result = Vec::with_capacity(16 + bytes.len() + aead::AES_256_GCM.tag_len());
        result.extend_from_slice(b"FSP1");
        result.extend_from_slice(&nonce);
        result.extend_from_slice(&bytes);
        let tag = key
            .seal_in_place_separate_tag(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(aad),
                &mut result[16..],
            )
            .map_err(|_| anyhow::anyhow!("Preview encryption failed."))?;
        result.extend_from_slice(tag.as_ref());
        Ok(result)
    }

    pub fn open_preview(&self, owner: &str, sid: &str, mut bytes: Vec<u8>) -> Result<Vec<u8>> {
        ensure!(
            (32..=MAX_PREVIEW).contains(&bytes.len()) && bytes.starts_with(b"FSP1"),
            "Invalid protected preview."
        );
        let descriptor = self
            .verified
            .lock()
            .map_err(|_| anyhow::anyhow!("Protected key state unavailable."))?
            .get(owner)
            .and_then(Option::as_ref)
            .map(|sender| sender.descriptor.clone())
            .ok_or_else(|| anyhow::anyhow!("Unverified preview sender."))?;
        let (key, aad) = self.preview_key(&descriptor, sid)?;
        let key = aead::LessSafeKey::new(
            aead::UnboundKey::new(&aead::AES_256_GCM, key.as_ref())
                .map_err(|_| anyhow::anyhow!("Preview decryption unavailable."))?,
        );
        let nonce: [u8; 12] = bytes[4..16].try_into().expect("checked nonce length");
        let plain = key
            .open_within(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(aad),
                &mut bytes,
                16..,
            )
            .map_err(|_| anyhow::anyhow!("Protected preview authentication failed."))?;
        ensure!(plain.len() <= 262_144, "Preview exceeds its size limit.");
        let len = plain.len();
        bytes.truncate(len);
        Ok(bytes)
    }
}
