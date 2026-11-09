use anyhow::{Context, Result, ensure};
use std::{
    os::windows::{ffi::OsStrExt, io::AsRawHandle},
    path::Path,
    process::{Command, ExitStatus},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VS_FIXEDFILEINFO, VerQueryValueW,
    },
    System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    },
    System::LibraryLoader::SetDllDirectoryW,
};

struct Job(HANDLE);
impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn version(path: &Path) -> Option<u64> {
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let size = GetFileVersionInfoSizeW(path.as_ptr(), std::ptr::null_mut());
        if size == 0 || size > 1024 * 1024 {
            return None;
        }
        let mut bytes = vec![0u8; size as usize];
        if GetFileVersionInfoW(path.as_ptr(), 0, size, bytes.as_mut_ptr().cast()) == 0 {
            return None;
        }
        let mut info = std::ptr::null_mut();
        let mut length = 0;
        if VerQueryValueW(
            bytes.as_ptr().cast(),
            windows_sys::w!("\\"),
            &mut info,
            &mut length,
        ) == 0
            || info.is_null()
            || length < std::mem::size_of::<VS_FIXEDFILEINFO>() as u32
        {
            return None;
        }
        let info = std::ptr::read_unaligned(info as *const VS_FIXEDFILEINFO);
        if info.dwSignature != 0xFEEF04BD {
            return None;
        }
        Some(((info.dwFileVersionMS as u64) << 32) | info.dwFileVersionLS as u64)
    }
}

pub fn run(command: &mut Command, bundled_probe: bool) -> Result<ExitStatus> {
    let root = Path::new(command.get_program())
        .parent()
        .and_then(Path::parent)
        .context("Missing runtime directory.")?;
    let crt = root.join("crt");
    let system = std::path::PathBuf::from(
        std::env::var_os("WINDIR").context("Windows directory is unavailable.")?,
    )
    .join("System32");
    // An older app-local CRT can break newer Windows audio/COM components loaded
    // in-process. Prefer the complete newer central trio; retain offline fallback.
    let central = !bundled_probe
        && ["msvcp140.dll", "vcruntime140.dll", "vcruntime140_1.dll"]
            .iter()
            .all(
                |name| match (version(&system.join(name)), version(&crt.join(name))) {
                    (Some(installed), Some(bundled)) => installed >= bundled,
                    _ => false,
                },
            );
    let selected = if central { &system } else { &crt };
    let selected: Vec<u16> = selected.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        // Windows propagates this search directory to children. It precedes
        // System32 and avoids changing machine settings or copying CRTs beside
        // the executable. PATH is separately restricted to this runtime/Windows.
        ensure!(
            SetDllDirectoryW(selected.as_ptr()) != 0,
            "Could not configure the runtime DLL directory."
        );
        let raw_job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        ensure!(
            !raw_job.is_null(),
            "Could not create the publisher lifetime guard."
        );
        let job = Job(raw_job);
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        ensure!(
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const _,
                std::mem::size_of_val(&limits) as u32
            ) != 0,
            "Could not configure the publisher lifetime guard."
        );
        let mut child = command
            .spawn()
            .context("Could not start the bundled publisher.")?;
        if AssignProcessToJobObject(job.0, child.as_raw_handle() as HANDLE) == 0 {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("Could not guard the publisher process lifetime.");
        }
        let result = child.wait().context("Could not wait for the publisher.");
        // Closing the job also ends any unexpected surviving descendants.
        drop(job);
        result
    }
}
