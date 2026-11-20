//! Remove obsolete profiles without reading or restoring their contents.
use std::path::PathBuf;

fn guest_path() -> Option<PathBuf> {
    #[cfg(windows)]
    let root = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)?;
    #[cfg(target_os = "macos")]
    let root = PathBuf::from(std::env::var_os("HOME")?).join("Library/Application Support");
    #[cfg(not(any(windows, target_os = "macos")))]
    let root = PathBuf::from(std::env::var_os("XDG_CONFIG_HOME")?);
    root.is_absolute()
        .then(|| root.join("PodeTelar").join("guest-name.json"))
}
pub fn login_server() -> Option<String> {
    let path = guest_path()?.with_file_name("login-server.json");
    if std::fs::metadata(&path).ok()?.len() > 4096 {
        return None;
    }
    let server: String = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    crate::admission::valid_server(&server).then_some(server)
}
pub fn save_login_server(server: &str) -> std::io::Result<()> {
    if !crate::admission::valid_server(server) {
        return Err(std::io::Error::other("Invalid server"));
    }
    if let Some(path) = guest_path() {
        std::fs::create_dir_all(path.parent().expect("profile directory"))?;
        std::fs::write(
            path.with_file_name("login-server.json"),
            serde_json::to_vec(server)?,
        )?;
    }
    Ok(())
}

pub fn valid_name(name: &str) -> bool {
    let name = name.trim();
    static INVALID: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    !name.is_empty()
        && name.encode_utf16().count() <= 60
        && !INVALID
            .get_or_init(|| regex::Regex::new(r"[\p{Cc}\p{Cf}]").expect("nickname character rules"))
            .is_match(name)
}

pub fn guest_name() -> String {
    guest_path()
        .and_then(|path| {
            if std::fs::metadata(&path).ok()?.len() > 1024 {
                return None;
            }
            let name: String = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
            valid_name(&name).then(|| name.trim().to_owned())
        })
        .unwrap_or_default()
}

pub fn save_guest_name(name: &str) -> std::io::Result<()> {
    if !valid_name(name) {
        return Err(std::io::Error::other("Invalid guest name"));
    }
    if let Some(path) = guest_path() {
        std::fs::create_dir_all(path.parent().expect("profile directory"))?;
        std::fs::write(path, serde_json::to_vec(name.trim())?)?;
    }
    Ok(())
}

// Only an explicit Remember action writes this credential. Desktop account and
// ordinary guest preferences are stored separately and never contain a room key.
pub struct PrivateStore(keyring::Entry);

impl PrivateStore {
    pub fn open() -> anyhow::Result<Self> {
        Self::entry("current")
    }

    fn entry(name: &str) -> anyhow::Result<Self> {
        keyring::Entry::new("PodeTelar native private invitation v1", name)
            .map(Self)
            .map_err(|_| anyhow::anyhow!("Private invitation storage is unavailable."))
    }

    pub fn load(&self) -> anyhow::Result<Option<String>> {
        match self.0.get_secret() {
            Ok(raw) => {
                let bytes = zeroize::Zeroizing::new(raw);
                let value = std::str::from_utf8(&bytes)
                    .map_err(|_| anyhow::anyhow!("Invalid saved private invitation."))?;
                anyhow::ensure!(valid_private(value), "Invalid saved private invitation.");
                Ok(Some(value.to_owned()))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => anyhow::bail!("Could not read the saved private invitation."),
        }
    }

    pub fn save(&self, value: &str) -> anyhow::Result<()> {
        anyhow::ensure!(valid_private(value), "Invalid private invitation.");
        self.0
            .set_secret(value.as_bytes())
            .map_err(|_| anyhow::anyhow!("Could not remember the private invitation."))
    }

    pub fn forget(&self) -> anyhow::Result<()> {
        match self.0.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => anyhow::bail!("Could not forget the private invitation."),
        }
    }
}

pub fn valid_private(value: &str) -> bool {
    if value.starts_with(crate::admission::voice_code::PREFIX) {
        crate::admission::voice_code::parse(value).is_ok()
    } else {
        value.starts_with(crate::protection::PREFIX) && crate::admission::valid_invitation(value)
    }
}

pub fn remove_legacy() {
    if let Some(root) = std::env::var_os("LOCALAPPDATA") {
        let root = std::path::PathBuf::from(root);
        if root.is_absolute() {
            remove_at(&root.join("FreedomStream"));
        }
    }
}

fn remove_at(root: &std::path::Path) {
    // Leave runtime caches, protocol registration and unrelated files alone.
    for name in ["settings.json", "preferences.json"] {
        let _ = std::fs::remove_file(root.join(name));
    }
}

#[cfg(test)]
mod tests {

    #[cfg(windows)]
    #[test]
    fn os_vault_only_remembers_valid_private_invitations_and_forgets() -> anyhow::Result<()> {
        let store = super::PrivateStore::entry(&format!("fixture-{}", rand::random::<u64>()))?;
        let room = format!(
            "https://room.invalid/r/{}#invite={}",
            "r".repeat(22),
            "s".repeat(43)
        );
        let private = crate::protection::protect_invitation(&room)?;
        let checked = (|| -> anyhow::Result<()> {
            assert!(store.load()?.is_none());
            assert!(store.save(&room).is_err());
            store.save(&private)?;
            assert!(store.load()?.as_deref() == Some(&private));
            let changed = crate::protection::replace_invitation(&private)?;
            store.save(&changed)?;
            assert!(store.load()?.as_deref() == Some(&changed));
            let voice =
                crate::admission::voice_code::create(&crate::admission::voice_code::Scope {
                    origin: "https://room.invalid".into(),
                    guild_id: "1".repeat(17),
                    channel_id: "2".repeat(17),
                    room_id: "r".repeat(22),
                })?;
            store.save(&voice)?;
            assert!(store.load()?.as_deref() == Some(&voice));
            store.0.set_secret(b"invalid")?;
            assert!(store.load().is_err());
            Ok(())
        })();
        store.forget()?;
        store.forget()?;
        assert!(store.load()?.is_none());
        checked
    }

    #[test]
    fn removes_only_legacy_preferences_without_parsing_them() {
        let directory = tempfile::tempdir().unwrap();
        for name in ["settings.json", "preferences.json", "runtime.dat"] {
            std::fs::write(directory.path().join(name), b"malformed legacy bytes").unwrap();
        }
        super::remove_at(directory.path());
        super::remove_at(directory.path());
        assert!(!directory.path().join("settings.json").exists());
        assert!(!directory.path().join("preferences.json").exists());
        assert!(directory.path().join("runtime.dat").exists());
    }
}
