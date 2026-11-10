//! The portable executable owns extraction and process setup, not media behavior.
//! Payloads are verified on extraction. Cached shared libraries remain replaceable.
use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{Cursor, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

pub const MAGIC: &[u8; 16] = b"FREEDOMSTREAM01!";
#[cfg(windows)]
pub mod browser_links;
#[cfg(windows)]
mod windows;
const FOOTER_SIZE: u64 = 56;
const MAX_ARCHIVE: u64 = 512 * 1024 * 1024;
const MAX_EXTRACTED: u64 = 1024 * 1024 * 1024;

pub struct Bundle {
    bytes: Vec<u8>,
    pub id: String,
}

impl Bundle {
    pub fn read(executable: &Path) -> Result<Self> {
        let mut file = File::open(executable)?;
        let length = file.metadata()?.len();
        ensure!(length > FOOTER_SIZE, "Portable payload is missing.");
        file.seek(SeekFrom::End(-(FOOTER_SIZE as i64)))?;
        let mut footer = [0u8; FOOTER_SIZE as usize];
        file.read_exact(&mut footer)?;
        ensure!(&footer[40..] == MAGIC, "Portable payload is missing.");
        let size = u64::from_le_bytes(footer[..8].try_into()?);
        ensure!(
            size > 0 && size <= MAX_ARCHIVE && size <= length - FOOTER_SIZE,
            "Portable payload has an invalid size."
        );
        file.seek(SeekFrom::Start(length - FOOTER_SIZE - size))?;
        let mut bytes = vec![0; size as usize];
        file.read_exact(&mut bytes)?;
        let digest = Sha256::digest(&bytes);
        ensure!(digest[..] == footer[8..40], "Portable payload is damaged.");
        Ok(Self {
            bytes,
            id: format!("{digest:x}"),
        })
    }

    /// Serializes first launch, extracts atomically, and never overwrites a cache.
    pub fn prepare(&self, base: &Path) -> Result<PathBuf> {
        ensure!(
            base.is_absolute(),
            "The runtime cache must be an absolute path."
        );
        check_ancestors(base)?;
        fs::create_dir_all(base)?;
        check_ancestors(base)?;
        let lock_path = base.join("extraction.lock");
        check_ancestors(&lock_path)?;
        let lock = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)?;
        lock.lock()
            .context("Could not lock the portable runtime cache.")?;
        let directory = base.join(&self.id);
        check_ancestors(&directory)?;
        if directory.exists() {
            ensure!(
                fs::read_to_string(directory.join(".complete"))? == self.id,
                "The runtime cache is incomplete. Move it aside and try again."
            );
            check_tree(&directory)?;
            self.check_required(&directory)?;
            return Ok(directory);
        }
        self.extract_new(&directory)?;
        Ok(directory)
    }

    /// Also exposes sources/notices and permits independent LGPL library changes.
    pub fn extract_new(&self, destination: &Path) -> Result<()> {
        ensure!(
            destination.is_absolute(),
            "Choose an absolute extraction path."
        );
        check_ancestors(destination)?;
        ensure!(
            !destination.exists(),
            "The extraction destination already exists."
        );
        let parent = destination
            .parent()
            .context("Choose a destination folder.")?;
        ensure!(
            parent.is_dir(),
            "The extraction parent folder does not exist."
        );
        let staging = tempfile::Builder::new()
            .prefix(".freedomstream-")
            .tempdir_in(parent)?;
        self.extract(staging.path())?;
        self.check_required(staging.path())?;
        fs::write(staging.path().join(".complete"), &self.id)?;
        fs::rename(staging.path(), destination)?;
        Ok(())
    }

    fn check_required(&self, root: &Path) -> Result<()> {
        for path in [
            "bin/freedomstream-publisher.exe",
            "libexec/gstreamer-1.0/gst-plugin-scanner.exe",
            "notices/README.md",
            "notices/sources/README.md",
            "crt/msvcp140.dll",
            "crt/vcruntime140.dll",
            "crt/vcruntime140_1.dll",
        ] {
            ensure!(
                root.join(path).is_file(),
                "Portable runtime files are missing."
            );
        }
        Ok(())
    }

    fn extract(&self, root: &Path) -> Result<()> {
        let mut zip = zip::ZipArchive::new(Cursor::new(&self.bytes))?;
        ensure!(zip.len() <= 20_000, "Portable payload has too many files.");
        let mut seen = HashSet::new();
        let mut total = 0u64;
        for index in 0..zip.len() {
            let mut entry = zip.by_index(index)?;
            let entry_name = entry.name()?.into_owned();
            let name = entry_name.trim_end_matches('/');
            ensure!(safe_name(name), "Portable payload contains an unsafe path.");
            ensure!(
                seen.insert(name.to_ascii_lowercase()),
                "Duplicate payload path."
            );
            ensure!(
                entry
                    .unix_mode()
                    .is_none_or(|mode| mode & 0o170000 != 0o120000),
                "Portable payload contains a symbolic link."
            );
            let path = root.join(name);
            if entry.is_dir() {
                fs::create_dir_all(path)?;
                continue;
            }
            total = total
                .checked_add(entry.size())
                .context("Payload size overflow.")?;
            ensure!(
                total <= MAX_EXTRACTED,
                "Portable payload expands beyond its limit."
            );
            fs::create_dir_all(path.parent().context("Invalid payload path.")?)?;
            let mut output = OpenOptions::new().write(true).create_new(true).open(path)?;
            let declared = entry.size();
            let copied = std::io::copy(&mut entry.by_ref().take(declared + 1), &mut output)?;
            ensure!(
                copied == declared,
                "Portable payload has an invalid file size."
            );
            output.flush()?;
        }
        Ok(())
    }
}

fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains(['\\', ':', '\0', '<', '>', '"', '*', '?'])
        && name.split('/').count() <= 32
        && name.split('/').all(|part| {
            !part.is_empty()
                && part.len() <= 255
                && part != "."
                && part != ".."
                && !part.ends_with(['.', ' '])
                && !part.chars().any(|c| c.is_control())
                && !matches!(
                    part.split('.')
                        .next()
                        .unwrap_or("")
                        .to_ascii_uppercase()
                        .as_str(),
                    "CON"
                        | "PRN"
                        | "AUX"
                        | "NUL"
                        | "COM1"
                        | "COM2"
                        | "COM3"
                        | "COM4"
                        | "COM5"
                        | "COM6"
                        | "COM7"
                        | "COM8"
                        | "COM9"
                        | "LPT1"
                        | "LPT2"
                        | "LPT3"
                        | "LPT4"
                        | "LPT5"
                        | "LPT6"
                        | "LPT7"
                        | "LPT8"
                        | "LPT9"
                )
        })
}

pub fn check_ancestors(path: &Path) -> Result<()> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) => {
                #[cfg(windows)]
                let redirected = {
                    use std::os::windows::fs::MetadataExt;
                    metadata.file_attributes() & 0x400 != 0
                };
                #[cfg(not(windows))]
                let redirected = metadata.file_type().is_symlink();
                ensure!(
                    !redirected,
                    "The runtime path contains a link or reparse point."
                );
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn check_tree(path: &Path) -> Result<()> {
    check_ancestors(path)?;
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            check_tree(&entry?.path())?;
        }
    }
    Ok(())
}

/// Clear developer/foreign media search paths while retaining normal OS context.
pub fn command(directory: &Path) -> Result<std::process::Command> {
    check_tree(directory)?;
    let windows = std::env::var_os("WINDIR").context("Windows directory is unavailable.")?;
    let mut command = std::process::Command::new(directory.join("bin/freedomstream-publisher.exe"));
    command.current_dir(directory);
    for (key, _) in std::env::vars_os() {
        let upper = key.to_string_lossy().to_ascii_uppercase();
        if upper.starts_with("GST_")
            || upper.starts_with("GSTREAMER_")
            || matches!(
                upper.as_str(),
                "GIO_EXTRA_MODULES" | "GIO_MODULE_DIR" | "GI_TYPELIB_PATH"
            )
        {
            command.env_remove(key);
        }
    }
    command.env(
        "PATH",
        std::env::join_paths([
            directory.join("bin"),
            directory.join("crt"),
            PathBuf::from(&windows).join("System32"),
            PathBuf::from(windows),
        ])?,
    );
    command.env("GST_PLUGIN_PATH_1_0", "");
    command.env(
        "GST_PLUGIN_SYSTEM_PATH_1_0",
        directory.join("lib/gstreamer-1.0"),
    );
    command.env(
        "GST_PLUGIN_SCANNER_1_0",
        directory.join("libexec/gstreamer-1.0/gst-plugin-scanner.exe"),
    );
    command.env("GST_REGISTRY_1_0", directory.join("registry.bin"));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW, no console alongside the UI.
    }
    Ok(command)
}

/// Keep the publisher/viewer lifetime tied to the caller, including forced exits.
pub fn run_publisher(
    command: &mut std::process::Command,
    bundled_probe: bool,
) -> Result<std::process::ExitStatus> {
    #[cfg(windows)]
    {
        windows::run(command, bundled_probe)
    }
    #[cfg(not(windows))]
    {
        let _ = bundled_probe;
        Ok(command.status()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zip::write::SimpleFileOptions;

    fn executable(files: &[(&str, &[u8])]) -> (tempfile::TempDir, PathBuf) {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in files {
            zip.start_file(*name, SimpleFileOptions::default()).unwrap();
            zip.write_all(bytes).unwrap();
        }
        let payload = zip.finish().unwrap().into_inner();
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("portable.exe");
        let mut bytes = b"MZ-test-launcher".to_vec();
        bytes.extend(&payload);
        bytes.extend((payload.len() as u64).to_le_bytes());
        bytes.extend(Sha256::digest(&payload));
        bytes.extend(MAGIC);
        fs::write(&path, bytes).unwrap();
        (temp, path)
    }

    #[test]
    fn refuses_corrupt_and_truncated_payloads() {
        let (_temp, path) = executable(&[("a", b"hello")]);
        let mut bytes = fs::read(&path).unwrap();
        bytes[30] ^= 1;
        fs::write(&path, &bytes).unwrap();
        assert!(Bundle::read(&path).is_err());
        fs::write(&path, b"MZ").unwrap();
        assert!(Bundle::read(&path).is_err());
    }

    #[test]
    fn refuses_escape_alias_stream_and_device_paths() {
        for path in [
            "../escape",
            "/absolute",
            "a/../b",
            "C:/a",
            "a\\b",
            "a:x",
            "a/NUL.txt",
            "a./b",
            "a//b",
        ] {
            assert!(!safe_name(path), "{path}");
        }
        let (_temp, path) = executable(&[("A.txt", b"one"), ("a.txt", b"two")]);
        let bundle = Bundle::read(&path).unwrap();
        let output = tempfile::tempdir().unwrap();
        assert!(bundle.extract(output.path()).is_err());
    }

    #[test]
    fn cache_is_atomic_and_preserves_modified_shared_libraries() {
        let (_temp, path) = executable(&[
            ("bin/freedomstream-publisher.exe", b"publisher"),
            ("libexec/gstreamer-1.0/gst-plugin-scanner.exe", b"scanner"),
            ("notices/README.md", b"notices"),
            ("notices/sources/README.md", b"sources"),
            ("bin/library.dll", b"original"),
            ("crt/msvcp140.dll", b"crt"),
            ("crt/vcruntime140.dll", b"crt"),
            ("crt/vcruntime140_1.dll", b"crt"),
        ]);
        let bundle = Bundle::read(&path).unwrap();
        let base = tempfile::tempdir().unwrap();
        let installed = bundle.prepare(base.path()).unwrap();
        fs::write(installed.join("bin/library.dll"), b"user replacement").unwrap();
        assert_eq!(bundle.prepare(base.path()).unwrap(), installed);
        assert_eq!(
            fs::read(installed.join("bin/library.dll")).unwrap(),
            b"user replacement"
        );
        fs::remove_file(installed.join(".complete")).unwrap();
        assert!(bundle.prepare(base.path()).is_err());
    }
}
