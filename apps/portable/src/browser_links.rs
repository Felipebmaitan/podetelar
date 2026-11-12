//! Optional per-user protocol registration. Cached launchers survive moved downloads.
use anyhow::{Context, Result, ensure};
use std::{
    fs,
    path::{Path, PathBuf},
};
use windows_sys::Win32::{Foundation::ERROR_SUCCESS, System::Registry::*};

const KEY: &str = "Software\\Classes\\freedomstream";
const OWNER: &str = "FreedomStreamPortable";
struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.0);
        }
    }
}
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
fn open(path: &str) -> Result<Option<Key>> {
    let mut key = std::ptr::null_mut();
    let code = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            wide(path).as_ptr(),
            0,
            KEY_READ,
            &mut key,
        )
    };
    if code == 2 {
        return Ok(None);
    }
    ensure!(
        code == ERROR_SUCCESS,
        "Could not read browser-link settings."
    );
    Ok(Some(Key(key)))
}
fn string(key: &Key, name: &str) -> Result<String> {
    let mut kind = 0;
    let mut bytes = vec![0u16; 16384];
    let mut size = (bytes.len() * 2) as u32;
    let code = unsafe {
        RegGetValueW(
            key.0,
            std::ptr::null(),
            wide(name).as_ptr(),
            RRF_RT_REG_SZ,
            &mut kind,
            bytes.as_mut_ptr().cast(),
            &mut size,
        )
    };
    ensure!(
        code == ERROR_SUCCESS && (2..=32768).contains(&size) && size.is_multiple_of(2),
        "Browser links belong to another application or are damaged."
    );
    bytes.truncate(size as usize / 2 - 1);
    String::from_utf16(&bytes).context("Browser-link settings are damaged.")
}
fn create(path: &str) -> Result<Key> {
    let mut key = std::ptr::null_mut();
    let code = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            wide(path).as_ptr(),
            0,
            std::ptr::null(),
            0,
            KEY_WRITE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        )
    };
    ensure!(
        code == ERROR_SUCCESS,
        "Could not enable browser links for this user."
    );
    Ok(Key(key))
}
fn set(key: &Key, name: &str, value: &str) -> Result<()> {
    let value = wide(value);
    let code = unsafe {
        RegSetValueExW(
            key.0,
            wide(name).as_ptr(),
            0,
            REG_SZ,
            value.as_ptr().cast(),
            (value.len() * 2) as u32,
        )
    };
    ensure!(
        code == ERROR_SUCCESS,
        "Could not save browser-link settings."
    );
    Ok(())
}
fn owned(path: &str) -> Result<bool> {
    let Some(key) = open(path)? else {
        return Ok(false);
    };
    ensure!(
        string(&key, OWNER)? == "1",
        "This browser-link scheme belongs to another application."
    );
    Ok(true)
}
pub fn enabled() -> bool {
    owned(KEY).unwrap_or(false)
}
fn register(path: &str, executable: &Path) -> Result<()> {
    ensure!(
        executable.is_absolute(),
        "Browser-link launcher must use an absolute path."
    );
    let existed = owned(path)?; // Never overwrite another application's handler.
    let exe = executable
        .to_str()
        .context("Browser-link launcher path is invalid.")?;
    ensure!(
        !exe.contains(['\"', '\0']),
        "Browser-link launcher path is invalid."
    );
    // Write the command before the root protocol marker; a new partial registration
    // is not advertised as a URL handler if a later write fails.
    let result = (|| {
        let command = create(&format!("{path}\\shell\\open\\command"))?;
        set(&command, "", &format!("\"{exe}\" --browser-link \"%1\""))?;
        let key = create(path)?;
        set(&key, OWNER, "1")?;
        set(&key, "", "URL:PodeTelar invitation")?;
        set(&key, "URL Protocol", "")?;
        Ok(())
    })();
    if result.is_err() && !existed {
        unsafe {
            RegDeleteTreeW(HKEY_CURRENT_USER, wide(path).as_ptr());
        }
    }
    result
}
fn unregister(path: &str) -> Result<()> {
    if !owned(path)? {
        return Ok(());
    }
    let code = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, wide(path).as_ptr()) };
    ensure!(code == ERROR_SUCCESS, "Could not disable browser links.");
    Ok(())
}
pub fn disable() -> Result<()> {
    unregister(KEY)
}
pub fn enable(executable: &Path, payload_id: &str) -> Result<PathBuf> {
    ensure!(
        payload_id.len() == 64 && payload_id.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "Invalid portable payload."
    );
    let _ = owned(KEY)?;
    let base =
        PathBuf::from(std::env::var_os("LOCALAPPDATA").context("Local AppData is unavailable.")?)
            .join("FreedomStreamPortable/browser-links");
    ensure!(
        base.is_absolute(),
        "Local AppData must be an absolute path."
    );
    super::check_ancestors(&base)?;
    fs::create_dir_all(&base)?;
    use sha2::{Digest, Sha256};
    let bytes = fs::read(executable)?;
    let digest = format!("{:x}", Sha256::digest(&bytes));
    let target = base.join(format!("FreedomStream-{digest}.exe"));
    super::check_ancestors(&target)?;
    if target.exists() {
        ensure!(
            fs::read(&target)? == bytes,
            "Cached browser launcher is damaged."
        );
    } else {
        use std::io::Write;
        let mut temporary = tempfile::NamedTempFile::new_in(&base)?;
        temporary.write_all(&bytes)?;
        temporary.as_file().sync_all()?;
        temporary
            .persist_noclobber(&target)
            .context("Could not save browser-link launcher.")?;
    }
    register(KEY, &target)?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_per_user_registration_quotes_paths_and_removes_only_our_scheme() {
        let path = format!(
            "Software\\Classes\\freedomstream-test-{}",
            std::process::id()
        );
        assert!(!owned(&path).unwrap());
        register(&path, Path::new("C:\\A folder\\FreedomStream.exe")).unwrap();
        assert!(owned(&path).unwrap());
        let command = open(&format!("{path}\\shell\\open\\command"))
            .unwrap()
            .unwrap();
        assert_eq!(
            string(&command, "").unwrap(),
            "\"C:\\A folder\\FreedomStream.exe\" --browser-link \"%1\""
        );
        drop(command);
        unregister(&path).unwrap();
        assert!(!owned(&path).unwrap());
        let foreign = create(&path).unwrap();
        set(&foreign, "", "Another app").unwrap();
        drop(foreign);
        assert!(unregister(&path).is_err());
        assert!(register(&path, Path::new("C:\\App.exe")).is_err());
        unsafe {
            RegDeleteTreeW(HKEY_CURRENT_USER, wide(&path).as_ptr());
        }
    }
}
