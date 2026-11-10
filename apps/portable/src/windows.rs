use anyhow::{Context, Result, ensure};
use std::{
    os::windows::ffi::OsStrExt,
    path::Path,
    process::{Command, ExitStatus},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VS_FIXEDFILEINFO, VerQueryValueW,
    },
    System::JobObjects::{
        CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JobObjectExtendedLimitInformation, SetInformationJobObject,
    },
    System::LibraryLoader::SetDllDirectoryW,
};

mod process;

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn lifetime_job() -> Result<Handle> {
    unsafe {
        let raw = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        ensure!(
            !raw.is_null(),
            "Could not create the publisher lifetime guard."
        );
        let job = Handle(raw);
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
        Ok(job)
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
        let job = lifetime_job()?;
        // Job membership is established by the kernel during creation, before
        // the publisher or any descendant can run, even if this launcher dies.
        let child = process::spawn(command, job.0, 0)?;
        let result = child.wait();
        // Closing the job also ends any unexpected surviving descendants.
        drop(job);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        process::{Child, Stdio},
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::WAIT_OBJECT_0,
        System::{
            JobObjects::IsProcessInJob,
            Threading::{
                CREATE_SUSPENDED, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
                PROCESS_SYNCHRONIZE, TerminateProcess, WaitForSingleObject,
            },
        },
    };

    // Child-only fixture: stop immediately after kernel creation, before the
    // publisher could execute any code. This is the former orphaning window.
    #[test]
    fn startup_guardian() {
        let Some(root) = std::env::var_os("FREEDOMSTREAM_TEST_GUARDIAN") else {
            return;
        };
        let root = std::path::PathBuf::from(root);
        let job = lifetime_job().unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args(["--exact", "windows::tests::argument_fixture"]);
        let child = process::spawn(&command, job.0, CREATE_SUSPENDED).unwrap();
        let mut member = 0;
        assert_ne!(
            unsafe { IsProcessInJob(child.raw_handle(), job.0, &mut member) },
            0
        );
        assert_ne!(
            member, 0,
            "Child must belong to its job at the creation boundary."
        );
        std::fs::write(root.join("ready.tmp"), child.id.to_string()).unwrap();
        std::fs::rename(root.join("ready.tmp"), root.join("ready")).unwrap();
        loop {
            std::thread::park();
        }
    }

    struct Guardian(Child);
    impl Drop for Guardian {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[test]
    fn startup_termination_cannot_leave_an_unassigned_child() {
        let root = tempfile::tempdir().unwrap();
        let mut guardian = Guardian(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "windows::tests::startup_guardian", "--nocapture"])
                .env("FREEDOMSTREAM_TEST_GUARDIAN", root.path())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let ready = root.path().join("ready");
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ready.is_file() {
            assert!(
                guardian.0.try_wait().unwrap().is_none(),
                "Creation fixture exited early."
            );
            assert!(Instant::now() < deadline, "Creation fixture timed out.");
            std::thread::sleep(Duration::from_millis(20));
        }
        let pid: u32 = std::fs::read_to_string(ready).unwrap().parse().unwrap();
        let raw = unsafe {
            OpenProcess(
                PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
                0,
                pid,
            )
        };
        assert!(!raw.is_null());
        let child = Handle(raw);
        guardian.0.kill().unwrap();
        guardian.0.wait().unwrap();
        let outcome = unsafe { WaitForSingleObject(child.0, 5000) };
        if outcome != WAIT_OBJECT_0 {
            unsafe {
                TerminateProcess(child.0, 1);
                WaitForSingleObject(child.0, 1000);
            }
        }
        assert_eq!(
            outcome, WAIT_OBJECT_0,
            "Publisher survived termination during startup."
        );
    }

    #[test]
    fn argument_fixture() {
        let Some(expected) = std::env::var_os("FREEDOMSTREAM_TEST_ARGUMENT") else {
            return;
        };
        assert_eq!(std::env::args_os().next_back().unwrap(), expected);
    }

    #[test]
    fn windows_argument_quoting_and_environment_reach_the_real_child() {
        let expected = "spaces λ \"quoted\" trailing\\";
        let job = lifetime_job().unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "windows::tests::argument_fixture",
                "--nocapture",
                "--skip",
                expected,
            ])
            .env("FREEDOMSTREAM_TEST_ARGUMENT", expected);
        assert!(
            process::spawn(&command, job.0, 0)
                .unwrap()
                .wait()
                .unwrap()
                .success()
        );
    }
}
