//! Per-user installed handler; permanent invitations are never stored here.
use anyhow::{Context, Result, ensure};
use std::path::Path;
use windows_sys::Win32::{Foundation::ERROR_SUCCESS, System::Registry::*};
#[cfg(not(any(feature = "link-qualification", feature = "update-qualification")))]
const SCHEME: &str = "Software\\Classes\\freedomstream";
#[cfg(any(feature = "link-qualification", feature = "update-qualification"))]
const SCHEME: &str = env!("FREEDOMSTREAM_LINK_TEST_SCHEME");
#[cfg(not(any(feature = "link-qualification", feature = "update-qualification")))]
const ACTIVE: &str = "Software\\FreedomStream\\InstalledActivation";
#[cfg(any(feature = "link-qualification", feature = "update-qualification"))]
const ACTIVE: &str = env!("FREEDOMSTREAM_LINK_TEST_ACTIVE");
const OWNER: &str = "FreedomStreamInstalled";
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

pub fn register(migrate: bool) -> Result<()> {
    register_at(SCHEME, &std::env::current_exe()?, migrate)
}
fn command(exe: &Path) -> Result<String> {
    let exe = exe.to_str().context("Invalid installed path.")?;
    ensure!(
        !exe.contains(['\"', '\0']) && Path::new(exe).is_absolute(),
        "Invalid installed path."
    );
    Ok(format!("\"{exe}\" --browser-link \"%1\""))
}
fn register_at(path: &str, exe: &Path, migrate: bool) -> Result<()> {
    let expected = command(exe)?;
    if let Some(key) = open(path)? {
        let ours = string(&key, OWNER).ok();
        let legacy = string(&key, "FreedomStreamPortable").ok().as_deref() == Some("1");
        ensure!(
            ours.as_deref() == exe.to_str() || (migrate && (ours.is_some() || legacy)),
            "Room links are registered to another application."
        );
    }
    let key = create(path)?;
    set(
        &key,
        OWNER,
        exe.to_str().context("Invalid installed path.")?,
    )?;
    let shell = create(&format!("{path}\\shell\\open\\command"))?;
    set(&shell, "", &expected)?;
    set(&key, "", "URL:FreedomStream room invitation")?;
    set(&key, "URL Protocol", "")?;
    unsafe {
        RegDeleteValueW(key.0, wide("FreedomStreamPortable").as_ptr());
    }
    Ok(())
}
pub fn unregister() -> Result<()> {
    unregister_at(SCHEME, &std::env::current_exe()?)
}
pub fn enabled() -> bool {
    let check = || -> Result<bool> {
        let exe = std::env::current_exe()?;
        let Some(key) = open(SCHEME)? else {
            return Ok(false);
        };
        let Some(shell) = open(&format!("{SCHEME}\\shell\\open\\command"))? else {
            return Ok(false);
        };
        Ok(string(&key, OWNER).ok().as_deref() == exe.to_str()
            && string(&shell, "")? == command(&exe)?)
    };
    check().unwrap_or(false)
}
fn unregister_at(path: &str, exe: &Path) -> Result<()> {
    if let Some(key) = open(path)? {
        if string(&key, OWNER).ok().as_deref() != exe.to_str() {
            return Ok(());
        }
        let Some(shell) = open(&format!("{path}\\shell\\open\\command"))? else {
            return Ok(());
        };
        if string(&shell, "")? != command(exe)? {
            return Ok(());
        }
        drop(shell);
        drop(key);
        ensure!(
            unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, wide(path).as_ptr()) } == ERROR_SUCCESS,
            "Could not remove installed room links."
        );
    }
    Ok(())
}
pub fn publish(address: &str, token: &str) -> Result<()> {
    let key = create(ACTIVE)?;
    set(
        &key,
        "Executable",
        std::env::current_exe()?
            .to_str()
            .context("Invalid installed path.")?,
    )?;
    set(&key, "Endpoint", address)?;
    set(&key, "Token", token)?;
    Ok(())
}
pub fn active() -> Result<(String, String)> {
    let key = open(ACTIVE)?.context("App is not ready.")?;
    ensure!(
        string(&key, "Executable")?
            == std::env::current_exe()?
                .to_str()
                .context("Invalid installed path.")?,
        "Different installation is running."
    );
    Ok((string(&key, "Endpoint")?, string(&key, "Token")?))
}
pub fn clear() {
    if let Ok(Some(key)) = open(ACTIVE)
        && let Ok(exe) = std::env::current_exe()
        && string(&key, "Executable").ok().as_deref() == exe.to_str()
    {
        drop(key);
        unsafe {
            RegDeleteTreeW(HKEY_CURRENT_USER, wide(ACTIVE).as_ptr());
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn installed_registration_migrates_portable_and_preserves_foreign_handlers() {
        let path = format!(
            "Software\\Classes\\freedomstream-installed-test-{}",
            std::process::id()
        );
        let exe = Path::new("C:\\Room app\\current\\freedomstream-installed.exe");
        let key = create(&path).unwrap();
        set(&key, "FreedomStreamPortable", "1").unwrap();
        drop(key);
        assert!(register_at(&path, exe, false).is_err());
        register_at(&path, exe, true).unwrap();
        register_at(&path, exe, false).unwrap();
        assert_eq!(
            string(
                &open(&format!("{path}\\shell\\open\\command"))
                    .unwrap()
                    .unwrap(),
                ""
            )
            .unwrap(),
            command(exe).unwrap()
        );
        unregister_at(
            &path,
            Path::new("C:\\Other\\current\\freedomstream-installed.exe"),
        )
        .unwrap();
        assert!(open(&path).unwrap().is_some());
        unregister_at(&path, exe).unwrap();
        assert!(open(&path).unwrap().is_none());
        let key = create(&path).unwrap();
        set(&key, "", "Another application").unwrap();
        drop(key);
        assert!(register_at(&path, exe, true).is_err());
        unregister_at(&path, exe).unwrap();
        assert!(open(&path).unwrap().is_some());
        unsafe {
            RegDeleteTreeW(HKEY_CURRENT_USER, wide(&path).as_ptr());
        }
    }
}
