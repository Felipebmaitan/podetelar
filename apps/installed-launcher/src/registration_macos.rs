//! Per-user activation state. Tickets stay in authenticated memory-only IPC.
use anyhow::{Context, Result, ensure};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::PathBuf,
};
pub fn root() -> std::io::Result<PathBuf> {
    if let Some(path) = option_env!("PODETELAR_MACOS_STATE_TEST").filter(|path| !path.is_empty()) {
        let root = PathBuf::from(path);
        if !root.is_absolute() {
            return Err(std::io::ErrorKind::InvalidInput.into());
        }
        fs::create_dir_all(&root)?;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
        return Ok(root);
    }
    let home = std::env::var_os("HOME").ok_or(std::io::ErrorKind::NotFound)?;
    let root = PathBuf::from(home).join("Library/Application Support/PodeTelar");
    fs::create_dir_all(&root)?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
    Ok(root)
}
pub fn lock() -> std::io::Result<Option<File>> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(root()?.join("launcher.lock"))?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(error)) => Err(error),
    }
}
pub fn publish(address: &str, token: &str) -> Result<()> {
    let path = root()?.join("active.json");
    let stage = root()?.join("active.new");
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&stage)?;
    file.write_all(&serde_json::to_vec(&(address, token))?)?;
    fs::rename(stage, path)?;
    Ok(())
}
pub fn active() -> Result<(String, String)> {
    let mut bytes = Vec::new();
    File::open(root()?.join("active.json"))?
        .take(1025)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 1024, "Invalid activation state.");
    serde_json::from_slice(&bytes).context("Invalid activation state.")
}
pub fn clear() {
    if let Ok(root) = root() {
        let _ = fs::remove_file(root.join("active.json"));
    }
}
// Launch Services registration comes from the app's Info.plist, never a shell command.
pub fn register(_install: bool) -> Result<()> {
    Ok(())
}
pub fn enabled() -> bool {
    true
}
