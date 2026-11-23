//! Native-only encryption capability for an existing Discord voice session.
//! The envelope contains no room admission credential.
use super::{valid_server, valid_token};
use crate::{
    following::Target,
    protection::{RoomSecret, decode, random},
};
use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

pub(crate) const PREFIX: &str = "PTVOICE1:";

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Scope {
    pub(crate) origin: String,
    pub(crate) guild_id: String,
    pub(crate) channel_id: String,
    pub(crate) room_id: String,
}

impl Scope {
    fn valid(&self) -> bool {
        fn snowflake(id: &str) -> bool {
            (17..=20).contains(&id.len())
                && !id.starts_with('0')
                && id.bytes().all(|byte| byte.is_ascii_digit())
                && id.parse::<u64>().is_ok()
        }
        valid_server(&self.origin)
            && snowflake(&self.guild_id)
            && snowflake(&self.channel_id)
            && valid_token(&self.room_id, 22)
    }

    pub(crate) fn matches_target(&self, target: &Target) -> bool {
        target.server == self.origin
            && target.location.guild_id == self.guild_id
            && target.location.channel_id == self.channel_id
    }

    fn secret(&self, root: &[u8; 32], generation: &[u8; 32]) -> RoomSecret {
        RoomSecret::from_parts(
            root,
            &[
                b"PodeTelar private Discord voice room v1",
                self.origin.as_bytes(),
                self.guild_id.as_bytes(),
                self.channel_id.as_bytes(),
                self.room_id.as_bytes(),
                generation,
            ],
        )
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u8,
    scope: Scope,
    root: String,
    generation: String,
}

impl Drop for Envelope {
    fn drop(&mut self) {
        self.root.zeroize();
    }
}

pub(crate) fn create(scope: &Scope) -> Result<String> {
    ensure!(scope.valid(), "Invalid Discord voice room.");
    let root = Zeroizing::new(random::<32>()?);
    let envelope = Envelope {
        version: 1,
        scope: scope.clone(),
        root: B64.encode(root.as_ref()),
        generation: B64.encode(random::<32>()?),
    };
    let bytes = Zeroizing::new(serde_json::to_vec(&envelope)?);
    let code = format!("{PREFIX}{}", B64.encode(bytes.as_slice()));
    ensure!(code.len() <= 2048, "Private voice code is too long.");
    Ok(code)
}

pub(crate) fn parse(code: &str) -> Result<(Scope, RoomSecret)> {
    const INVALID: &str = "Invalid private Discord voice code.";
    ensure!(code.len() <= 2048, "{INVALID}");
    let encoded = code
        .strip_prefix(PREFIX)
        .ok_or_else(|| anyhow::anyhow!(INVALID))?;
    let bytes = Zeroizing::new(B64.decode(encoded).map_err(|_| anyhow::anyhow!(INVALID))?);
    let envelope: Envelope =
        serde_json::from_slice(&bytes).map_err(|_| anyhow::anyhow!(INVALID))?;
    ensure!(envelope.version == 1 && envelope.scope.valid(), "{INVALID}");
    let root = Zeroizing::new(decode::<32>(&envelope.root)?);
    let generation = decode::<32>(&envelope.generation)?;
    let secret = envelope.scope.secret(&root, &generation);
    Ok((envelope.scope.clone(), secret))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope() -> Scope {
        Scope {
            origin: "https://room.invalid".into(),
            guild_id: "1".repeat(17),
            channel_id: "2".repeat(17),
            room_id: "r".repeat(22),
        }
    }

    #[test]
    fn receivers_require_the_exact_voice_context_even_when_root_is_identical() {
        use crate::protection::Session;
        let original = scope();
        let root = [57; 32];
        let generation = [81; 32];
        let a = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
        let b = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";
        let sender = Session::new(original.secret(&root, &generation), a.into()).unwrap();
        let raw = sender.descriptor().unwrap();
        let verify = |key| {
            Session::new(key, b.into())
                .unwrap()
                .authorize(a, Some(&raw))
        };
        assert!(verify(original.secret(&root, &generation)));
        for (key, value) in [
            ("origin", "https://other.invalid"),
            ("guildId", "33333333333333333"),
            ("channelId", "44444444444444444"),
            ("roomId", "qqqqqqqqqqqqqqqqqqqqqq"),
        ] {
            let mut changed = serde_json::to_value(&original).unwrap();
            changed[key] = serde_json::json!(value);
            let changed: Scope = serde_json::from_value(changed).unwrap();
            assert!(!verify(changed.secret(&root, &generation)));
        }
        assert!(!verify(original.secret(&root, &[82; 32])));
        assert!(!verify(RoomSecret::from_parts(
            &root,
            &[
                b"FreedomStream private room v1",
                original.origin.as_bytes(),
                original.room_id.as_bytes(),
                &generation
            ]
        )));
    }

    #[test]
    fn voice_code_is_versioned_has_no_admission_authority_and_requires_canonical_scope() {
        let scope = scope();
        let code = create(&scope).unwrap();
        assert!(parse(&code).unwrap().0 == scope);
        assert!(code != create(&scope).unwrap());
        assert!(parse(&code.replace("PTVOICE1:", "PTVOICE2:")).is_err());
        let mut contents: serde_json::Value =
            serde_json::from_slice(&B64.decode(&code[PREFIX.len()..]).unwrap()).unwrap();
        assert!(contents.get("invitation").is_none());
        contents["scope"]["roomId"] = serde_json::json!("invalid");
        let encode = |value: &serde_json::Value| {
            format!("{PREFIX}{}", B64.encode(serde_json::to_vec(value).unwrap()))
        };
        assert!(parse(&encode(&contents)).is_err());
        contents["scope"]["roomId"] = serde_json::json!(scope.room_id);
        contents["invitation"] = serde_json::json!("unexpected");
        assert!(parse(&encode(&contents)).is_err());
        for origin in [
            "https://room.invalid/",
            "http://room.invalid",
            "https://other@room.invalid",
        ] {
            let mut invalid = scope.clone();
            invalid.origin = origin.to_owned();
            assert!(create(&invalid).is_err());
        }
    }
}
