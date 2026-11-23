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
pub fn appearance_path() -> Option<PathBuf> {
    Some(guest_path()?.with_file_name("appearance.json"))
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
