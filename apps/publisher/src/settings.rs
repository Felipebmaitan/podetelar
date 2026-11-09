//! Only the last successful room/name are persisted. Session/media authority stays
//! in memory. DPAPI binds the saved invitation to the current Windows user.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{io::Read, path::PathBuf};

const MAX_FILE: u64 = 16 * 1024;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub invitation: String,
    pub name: String,
}

impl Profile {
    pub fn new(invitation: &str, name: &str) -> Result<Self> {
        let profile = Self {
            invitation: invitation.trim().to_owned(),
            name: name.trim().to_owned(),
        };
        profile.validate()?;
        Ok(profile)
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            crate::admission::valid_invitation(&self.invitation),
            "Invalid saved room."
        );
        ensure!(
            !self.name.is_empty()
                && self.name == self.name.trim()
                && self.name.encode_utf16().count() <= 60
                && !self.name.chars().any(char::is_control),
            "Invalid saved name."
        );
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u8,
    protected: Vec<u8>,
}

pub struct Store {
    path: PathBuf,
}

impl Store {
    pub fn open() -> Result<Self> {
        let root = std::env::var_os("LOCALAPPDATA")
            .filter(|s| !s.is_empty())
            .context("Windows profile storage is unavailable.")?;
        let root = PathBuf::from(root);
        ensure!(
            root.is_absolute(),
            "Windows profile storage is unavailable."
        );
        Ok(Self {
            path: root.join("FreedomStream").join("settings.json"),
        })
    }

    pub fn load(&self) -> Result<Option<Profile>> {
        let file = match std::fs::File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => anyhow::bail!("Could not read the saved room."),
        };
        let mut bytes = Vec::new();
        file.take(MAX_FILE + 1)
            .read_to_end(&mut bytes)
            .context("Could not read the saved room.")?;
        ensure!(
            bytes.len() as u64 <= MAX_FILE,
            "Saved room data is too large."
        );
        // Do not propagate serde errors: malformed input can contain authority.
        let envelope: Envelope = serde_json::from_slice(&bytes)
            .map_err(|_| anyhow::anyhow!("Could not read the saved room."))?;
        ensure!(envelope.version == 1, "Unsupported saved room version.");
        let plain = protect(&envelope.protected, false)?;
        let profile: Profile = serde_json::from_slice(&plain)
            .map_err(|_| anyhow::anyhow!("Could not read the saved room."))?;
        profile.validate()?;
        Ok(Some(profile))
    }

    pub fn save(&self, profile: &Profile) -> Result<()> {
        profile.validate()?;
        let envelope = Envelope {
            version: 1,
            protected: protect(&serde_json::to_vec(profile)?, true)?,
        };
        let parent = self
            .path
            .parent()
            .context("Saved room path is unavailable.")?;
        std::fs::create_dir_all(parent).context("Could not create saved room storage.")?;
        // A temporary file in this directory and the final file contain only
        // ciphertext. Persist atomically replaces the previous successful room.
        let mut file =
            tempfile::NamedTempFile::new_in(parent).context("Could not write the saved room.")?;
        serde_json::to_writer(file.as_file_mut(), &envelope)
            .context("Could not write the saved room.")?;
        file.as_file()
            .sync_all()
            .context("Could not write the saved room.")?;
        file.persist(&self.path)
            .map_err(|_| anyhow::anyhow!("Could not replace the saved room."))?;
        Ok(())
    }

    pub fn forget(&self) -> Result<()> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => anyhow::bail!("Could not forget the saved room."),
        }
    }
}

#[cfg(windows)]
fn protect(input: &[u8], encrypt: bool) -> Result<Vec<u8>> {
    use windows::Win32::{
        Foundation::{HLOCAL, LocalFree},
        Security::Cryptography::{
            CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
        },
    };
    ensure!(
        !input.is_empty() && input.len() as u64 <= MAX_FILE,
        "Invalid saved room data."
    );
    let source = CRYPT_INTEGER_BLOB {
        cbData: input.len() as u32,
        pbData: input.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        let result = if encrypt {
            CryptProtectData(
                &source,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &source,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        result.map_err(|_| {
            anyhow::anyhow!("Windows could not unlock or save this room for your user.")
        })?;
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(output.pbData.cast())));
        Ok(bytes)
    }
}

#[cfg(not(windows))]
fn protect(_: &[u8], _: bool) -> Result<Vec<u8>> {
    anyhow::bail!("Secure saved rooms require Windows.")
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn windows_profile_round_trip_replacement_tampering_and_forgetting() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = Store {
            path: directory.path().join("profile.json"),
        };
        assert!(store.load()?.is_none());
        let mut profile = Profile::new(
            &format!(
                "https://example.test/r/{}#invite={}",
                "a".repeat(22),
                "b".repeat(43)
            ),
            "Saved user",
        )?;
        store.save(&profile)?;
        assert!(store.load()?.as_ref() == Some(&profile));
        let text = std::fs::read_to_string(&store.path)?;
        assert!(!text.contains(&profile.invitation) && !text.contains(&profile.name));
        profile.name = "Replacement user".into();
        store.save(&profile)?;
        assert!(store.load()?.as_ref() == Some(&profile));
        let mut envelope: Envelope = serde_json::from_str(&std::fs::read_to_string(&store.path)?)?;
        let last = envelope.protected.len() - 1;
        envelope.protected[last] ^= 0xff;
        std::fs::write(&store.path, serde_json::to_vec(&envelope)?)?;
        assert!(store.load().is_err());
        std::fs::write(&store.path, vec![b' '; MAX_FILE as usize + 1])?;
        assert!(store.load().is_err());
        store.forget()?;
        store.forget()?;
        assert!(store.load()?.is_none());
        Ok(())
    }
}
