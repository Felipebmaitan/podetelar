//! Remove only registrations owned by previous releases. Old URLs are not accepted.
use anyhow::{Result, ensure};
use windows_sys::Win32::{Foundation::ERROR_SUCCESS, System::Registry::*};

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
        "Could not read previous room-link registration."
    );
    Ok(Some(Key(key)))
}
fn string(key: &Key, name: &str) -> Option<String> {
    let mut bytes = vec![0u16; 16384];
    let mut size = (bytes.len() * 2) as u32;
    let code = unsafe {
        RegGetValueW(
            key.0,
            std::ptr::null(),
            wide(name).as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            bytes.as_mut_ptr().cast(),
            &mut size,
        )
    };
    if code != ERROR_SUCCESS || !(2..=32768).contains(&size) || !size.is_multiple_of(2) {
        return None;
    }
    bytes.truncate(size as usize / 2 - 1);
    String::from_utf16(&bytes).ok()
}
fn launcher(command: &str) -> Option<&str> {
    command
        .strip_prefix('"')?
        .strip_suffix("\" --browser-link \"%1\"")
}
fn previous_launcher(path: &str) -> bool {
    let filename = path
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    filename == "freedomstream-installed.exe"
        || filename == "freedomstream.exe"
        || filename
            .strip_prefix("freedomstream-")
            .and_then(|v| v.strip_suffix(".exe"))
            .is_some_and(|hash| hash.len() == 64 && hash.bytes().all(|v| v.is_ascii_hexdigit()))
}
pub fn cleanup() -> Result<()> {
    cleanup_at("Software\\Classes\\freedomstream")
}
fn cleanup_at(path: &str) -> Result<()> {
    let Some(key) = open(path)? else {
        return Ok(());
    };
    let Some(shell) = open(&format!("{path}\\shell\\open\\command"))? else {
        return Ok(());
    };
    let Some(command) = string(&shell, "") else {
        return Ok(());
    };
    let Some(executable) = launcher(&command) else {
        return Ok(());
    };
    let installed = string(&key, "FreedomStreamInstalled").is_some_and(|owner| owner == executable);
    let portable = string(&key, "FreedomStreamPortable").as_deref() == Some("1");
    if !previous_launcher(executable) || !(installed || portable) {
        return Ok(());
    }
    drop(shell);
    drop(key);
    ensure!(
        unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, wide(path).as_ptr()) } == ERROR_SUCCESS,
        "Could not remove previous room-link registration."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn create(path: &str, values: &[(&str, &str)]) {
        let mut handle = std::ptr::null_mut();
        assert_eq!(
            unsafe {
                RegCreateKeyExW(
                    HKEY_CURRENT_USER,
                    wide(path).as_ptr(),
                    0,
                    std::ptr::null(),
                    0,
                    KEY_WRITE,
                    std::ptr::null(),
                    &mut handle,
                    std::ptr::null_mut(),
                )
            },
            ERROR_SUCCESS
        );
        let key = Key(handle);
        for (name, value) in values {
            let data = wide(value);
            assert_eq!(
                unsafe {
                    RegSetValueExW(
                        key.0,
                        wide(name).as_ptr(),
                        0,
                        REG_SZ,
                        data.as_ptr().cast(),
                        (data.len() * 2) as u32,
                    )
                },
                ERROR_SUCCESS
            );
        }
    }
    #[test]
    fn removes_owned_previous_handlers_and_preserves_foreign_or_modified_commands() {
        let path = format!(
            "Software\\Classes\\podetelar-legacy-test-{}",
            std::process::id()
        );
        let shell = format!("{path}\\shell\\open\\command");
        for installed in [false, true] {
            let exe = "C:\\Room app\\freedomstream-installed.exe";
            let marker = if installed {
                ("FreedomStreamInstalled", exe)
            } else {
                ("FreedomStreamPortable", "1")
            };
            create(&path, &[marker]);
            create(&shell, &[("", &format!("\"{exe}\" --browser-link \"%1\""))]);
            cleanup_at(&path).unwrap();
            assert!(open(&path).unwrap().is_none());
            cleanup_at(&path).unwrap();
        }
        create(&path, &[("", "Another application")]);
        create(&shell, &[("", "\"C:\\Other.exe\" --browser-link \"%1\"")]);
        cleanup_at(&path).unwrap();
        assert!(open(&path).unwrap().is_some());
        create(
            &path,
            &[(
                "FreedomStreamInstalled",
                "C:\\Room app\\freedomstream-installed.exe",
            )],
        );
        cleanup_at(&path).unwrap();
        assert!(open(&path).unwrap().is_some());
        unsafe {
            RegDeleteTreeW(HKEY_CURRENT_USER, wide(&path).as_ptr());
        }
    }
}
