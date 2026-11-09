use super::policy::{Identity, Inventory, Process};
use crate::win32::Handle;
use anyhow::{Context, Result, ensure};
use std::collections::{HashMap, HashSet};
use windows::{
    Win32::{
        Foundation::{FILETIME, HANDLE},
        Media::Audio::{
            AudioSessionStateExpired, DEVICE_STATE_ACTIVE, IAudioSessionControl2,
            IAudioSessionManager2, IMMDeviceEnumerator, MMDeviceEnumerator, eRender,
        },
        System::{
            Com::{
                CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
            },
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
                TH32CS_SNAPPROCESS,
            },
            Threading::{
                GetProcessTimes, OpenProcess, PROCESS_NAME_WIN32,
                PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
            },
        },
    },
    core::{Interface, PWSTR},
};

struct Com;
impl Drop for Com {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

pub fn identity(pid: u32) -> Result<Identity> {
    let handle = Handle(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }?);
    identity_from_handle(pid, handle.0)
}

pub fn lease(expected: Identity) -> Result<Handle> {
    let handle =
        Handle(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, expected.pid) }?);
    ensure!(
        identity_from_handle(expected.pid, handle.0)? == expected,
        "The audio process changed. Refresh audio applications."
    );
    // Holding this process object prevents Windows recycling its PID while the
    // corresponding loopback capture is alive.
    Ok(handle)
}

pub fn identity_from_handle(pid: u32, handle: HANDLE) -> Result<Identity> {
    let mut created = FILETIME::default();
    let mut exited = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    unsafe { GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) }?;
    Ok(Identity {
        pid,
        created: ((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64,
    })
}

pub fn inventory() -> Result<Inventory> {
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }
        .context("Audio process discovery could not initialize.")?;
    let _com = Com;
    let mut processes = HashMap::new();
    let snapshot = Handle(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }?);
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut available = unsafe { Process32FirstW(snapshot.0, &mut entry) }.is_ok();
    let mut path = vec![0_u16; 32768];
    while available {
        let pid = entry.th32ProcessID;
        let length = entry
            .szExeFile
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(entry.szExeFile.len());
        let name = String::from_utf16_lossy(&entry.szExeFile[..length]);
        let mut process = Process {
            identity: Identity { pid, created: 0 },
            parent: entry.th32ParentProcessID,
            name,
            image: None,
        };
        if let Ok(raw) = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
            let handle = Handle(raw);
            if let Ok(identity) = identity_from_handle(pid, handle.0) {
                process.identity = identity;
            }
            let mut size = path.len() as u32;
            if unsafe {
                QueryFullProcessImageNameW(
                    handle.0,
                    PROCESS_NAME_WIN32,
                    PWSTR(path.as_mut_ptr()),
                    &mut size,
                )
            }
            .is_ok()
            {
                process.image =
                    Some(String::from_utf16_lossy(&path[..size as usize]).to_lowercase());
            }
        }
        processes.insert(pid, process);
        available = unsafe { Process32NextW(snapshot.0, &mut entry) }.is_ok();
    }
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
            .context("Audio devices could not be enumerated.")?;
    let endpoints = unsafe { enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) }?;
    let mut sessions = HashSet::new();
    for index in 0..unsafe { endpoints.GetCount() }? {
        let device = unsafe { endpoints.Item(index) }?;
        let manager: IAudioSessionManager2 = unsafe { device.Activate(CLSCTX_ALL, None) }?;
        let applications = unsafe { manager.GetSessionEnumerator() }?;
        for index in 0..unsafe { applications.GetCount() }? {
            let session = unsafe { applications.GetSession(index) }?;
            if unsafe { session.GetState() }.is_ok_and(|state| state == AudioSessionStateExpired) {
                continue;
            }
            if let Ok(session) = session.cast::<IAudioSessionControl2>()
                && let Ok(pid) = unsafe { session.GetProcessId() }
                && pid != 0
            {
                sessions.insert(pid);
            }
        }
    }
    Ok(Inventory {
        processes,
        sessions,
    })
}
