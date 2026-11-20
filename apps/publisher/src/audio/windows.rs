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
    inventory_using(|_, _, handle, path| query_image(handle, path))
}

fn query_image(handle: HANDLE, path: &mut [u16]) -> Option<String> {
    let mut size = path.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(path.as_mut_ptr()),
            &mut size,
        )
    }
    .ok()?;
    Some(String::from_utf16_lossy(&path[..size as usize]).to_lowercase())
}

fn inventory_using(
    mut image: impl FnMut(Identity, &str, HANDLE, &mut [u16]) -> Option<String>,
) -> Result<Inventory> {
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
            process.image = image(process.identity, &process.name, handle.0, &mut path);
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

#[cfg(feature = "performance-experiments")]
pub fn inventory_benchmark() -> Result<serde_json::Value> {
    let mut cache = HashMap::<(Identity, String), String>::new();
    let mut rows = Vec::new();
    for pair in 1..=5 {
        for cached in if pair % 2 == 1 {
            [false, true]
        } else {
            [true, false]
        } {
            let started = std::time::Instant::now();
            let mut query_count = 0;
            let mut processes = 0;
            for _ in 0..20 {
                let current = inventory_using(|identity, name, handle, path| {
                    let key = (identity, name.to_owned());
                    if cached
                        && identity.created != 0
                        && let Some(image) = cache.get(&key)
                    {
                        return Some(image.clone());
                    }
                    query_count += 1;
                    let result = query_image(handle, path);
                    if identity.created != 0
                        && let Some(image) = &result
                    {
                        cache.insert(key, image.clone());
                    }
                    result
                })?;
                processes = current.processes.len();
                cache.retain(|(identity, name), _| {
                    current.processes.get(&identity.pid).is_some_and(|process| {
                        process.identity == *identity && process.name == *name
                    })
                });
            }
            rows.push(serde_json::json!({"pair":pair,"cached":cached,"iterations":20,"wallMs":started.elapsed().as_secs_f64()*1000.0,"freshImageQueries":query_count,"processCount":processes,"boundedCacheEntries":cache.len()}));
        }
    }
    Ok(
        serde_json::json!({"runs":rows,"limits":"Benchmark-only image-name cache. Every snapshot, parent/name, current creation identity, access result and audio endpoint/session remains fresh. Unknown identities never reuse cache; entries pruned every sweep. No process identifiers or names exported. Rename/relocation and protected-process qualification still required; default capture uses uncached queries."}),
    )
}
