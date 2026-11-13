//! Usage recognition only: never a credential, SFU identity or raw hardware ID.
use anyhow::{Result, ensure};
use sha2::{Digest, Sha256};

fn derive(raw: &[u8], server: &str) -> Result<String> {
    ensure!(
        crate::admission::valid_server(server),
        "Invalid device scope."
    );
    ensure!(
        !raw.is_empty() && raw.iter().any(|byte| *byte != 0),
        "PC identity unavailable."
    );
    let mut hash = Sha256::new();
    // Version/domain separators stay fixed across app updates and rebranding.
    hash.update(b"podetelar.device.v1\0");
    hash.update(server.as_bytes());
    hash.update(b"\0");
    hash.update(raw);
    Ok(format!("pc1_{:x}", hash.finalize()))
}

#[cfg(windows)]
pub fn for_server(server: &str) -> Result<String> {
    use windows::{
        Security::Cryptography::CryptographicBuffer,
        System::Profile::{SystemIdentification, SystemIdentificationSource},
        Win32::System::Com::CoIncrementMTAUsage,
        Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
    };
    // windows-rs caches agile activation factories for the process lifetime.
    // Keep the MTA alive for that same lifetime: tearing down its last apartment
    // between calls invalidated the cached cryptography factory on real Windows.
    // One process-owned usage reference is reclaimed by OS process teardown.
    static MTA: std::sync::OnceLock<std::result::Result<(), ()>> = std::sync::OnceLock::new();
    MTA.get_or_init(|| unsafe { CoIncrementMTAUsage() }.map(|_| ()).map_err(|_| ()))
        .map_err(|_| anyhow::anyhow!("PC identity unavailable."))?;
    // Called on a blocking worker, not the UI/COM apartment. Balance every
    // successful initialization, including S_FALSE, on this same thread.
    unsafe { RoInitialize(RO_INIT_MULTITHREADED) }
        .map_err(|_| anyhow::anyhow!("PC identity unavailable."))?;
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe { RoUninitialize() };
        }
    }
    let _apartment = Apartment;
    let read = || -> windows::core::Result<_> {
        let info = SystemIdentification::GetSystemIdForPublisher()?;
        let source = info.Source()?;
        let mut bytes = windows::core::Array::new();
        CryptographicBuffer::CopyToByteArray(&info.Id()?, &mut bytes)?;
        Ok((source, bytes))
    };
    let (source, raw) = read().map_err(|_| anyhow::anyhow!("PC identity unavailable."))?;
    ensure!(
        matches!(
            source,
            SystemIdentificationSource::Tpm
                | SystemIdentificationSource::Uefi
                | SystemIdentificationSource::Registry
        ),
        "PC identity unavailable."
    );
    // No files/registry entries owned by PodeTelar are needed. Uninstalling or
    // moving the app does not reset this value. Windows' registry-backed fallback
    // can change after a clean OS install. Never substitute a per-launch ID.
    derive(&raw, server)
}

#[cfg(not(windows))]
pub fn for_server(_server: &str) -> Result<String> {
    anyhow::bail!("PC identity requires Windows.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_scoped_identity_is_independent_of_sessions_and_names() {
        let raw = b"synthetic-test-machine-only";
        let first = derive(raw, "https://rooms.example").unwrap();
        assert_eq!(first, derive(raw, "https://rooms.example").unwrap());
        assert_ne!(first, derive(raw, "https://other.example").unwrap());
        assert_ne!(
            first,
            derive(b"another-test-machine", "https://rooms.example").unwrap()
        );
        assert_eq!(first.len(), 68);
        assert!(first.starts_with("pc1_"));
        assert!(
            first[4..]
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
        assert!(derive(&[], "https://rooms.example").is_err());
        assert!(derive(&[0; 32], "https://rooms.example").is_err());
        assert!(derive(raw, "https://rooms.example/path").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn actual_windows_identity_is_stable_across_independent_threads() {
        let read = || {
            std::thread::spawn(|| for_server("https://device-test.invalid").unwrap())
                .join()
                .unwrap()
        };
        assert!(
            read() == read(),
            "Independent threads must recognize the same PC"
        );
    }
}
