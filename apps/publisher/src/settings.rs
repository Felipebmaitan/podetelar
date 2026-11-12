//! Remove obsolete profiles without reading or restoring their contents.
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
