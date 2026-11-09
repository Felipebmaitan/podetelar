//! Stable Rust does not yet expose Command::spawn_with_attributes. Keep the
//! Windows creation boundary here; callers use inherited standard I/O, and the
//! program/arguments/environment/current directory from our Command builder.
use super::Handle;
use anyhow::{Context, Result, ensure};
use std::{
    cmp::Ordering,
    ffi::{OsStr, OsString},
    os::windows::{ffi::OsStrExt, process::ExitStatusExt},
    process::{Command, ExitStatus},
};
use windows_sys::Win32::{
    Foundation::{
        DUPLICATE_SAME_ACCESS, DuplicateHandle, GENERIC_READ, GENERIC_WRITE, HANDLE,
        INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
    },
    Globalization::{CSTR_EQUAL, CSTR_LESS_THAN, CompareStringOrdinal},
    Security::SECURITY_ATTRIBUTES,
    Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    },
    System::{
        Console::{GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE},
        Threading::{
            CREATE_NO_WINDOW, CREATE_UNICODE_ENVIRONMENT, CreateProcessW,
            DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT, GetCurrentProcess,
            GetExitCodeProcess, INFINITE, InitializeProcThreadAttributeList,
            LPPROC_THREAD_ATTRIBUTE_LIST, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
            PROC_THREAD_ATTRIBUTE_JOB_LIST, PROCESS_INFORMATION, STARTF_USESTDHANDLES,
            STARTUPINFOEXW, UpdateProcThreadAttribute, WaitForSingleObject,
        },
    },
};

fn wide(value: &OsStr) -> Result<Vec<u16>> {
    let mut units: Vec<_> = value.encode_wide().collect();
    ensure!(
        !units.contains(&0),
        "Publisher process input contains a NUL."
    );
    units.push(0);
    Ok(units)
}

fn append_argument(output: &mut Vec<u16>, value: &OsStr) -> Result<()> {
    // Windows CRT argv escaping, including embedded quotes and trailing slashes.
    let units = wide(value)?;
    output.push(b'"' as u16);
    let mut slashes = 0;
    for &unit in &units[..units.len() - 1] {
        match unit {
            92 => slashes += 1,
            34 => {
                output.extend(std::iter::repeat_n(92, slashes * 2 + 1));
                output.push(unit);
                slashes = 0;
            }
            _ => {
                output.extend(std::iter::repeat_n(92, slashes));
                output.push(unit);
                slashes = 0;
            }
        }
    }
    output.extend(std::iter::repeat_n(92, slashes * 2));
    output.push(b'"' as u16);
    Ok(())
}

fn compare_keys(a: &OsStr, b: &OsStr) -> Ordering {
    let a: Vec<_> = a.encode_wide().collect();
    let b: Vec<_> = b.encode_wide().collect();
    match unsafe { CompareStringOrdinal(a.as_ptr(), a.len() as i32, b.as_ptr(), b.len() as i32, 1) }
    {
        CSTR_LESS_THAN => Ordering::Less,
        CSTR_EQUAL => Ordering::Equal,
        _ => Ordering::Greater,
    }
}

fn environment(command: &Command) -> Result<Vec<u16>> {
    let mut variables: Vec<(OsString, OsString)> = std::env::vars_os().collect();
    for (key, value) in command.get_envs() {
        variables.retain(|(existing, _)| compare_keys(existing, key) != Ordering::Equal);
        if let Some(value) = value {
            variables.push((key.to_owned(), value.to_owned()));
        }
    }
    variables.sort_by(|(a, _), (b, _)| compare_keys(a, b));
    let mut block = Vec::new();
    for (key, value) in variables {
        let key = wide(&key)?;
        ensure!(key.len() > 1, "Publisher environment has an empty key.");
        block.extend(&key[..key.len() - 1]);
        block.push(b'=' as u16);
        block.extend(wide(&value)?);
    }
    if block.is_empty() {
        block.push(0);
    }
    block.push(0);
    Ok(block)
}

fn standard_handle(kind: u32) -> Result<Handle> {
    unsafe {
        let source = GetStdHandle(kind);
        if source.is_null() || source == INVALID_HANDLE_VALUE {
            // A double-clicked GUI launcher has no console/redirected handles.
            let attributes = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: std::ptr::null_mut(),
                bInheritHandle: 1,
            };
            let access = if kind == STD_INPUT_HANDLE {
                GENERIC_READ
            } else {
                GENERIC_WRITE
            };
            let handle = CreateFileW(
                windows_sys::w!("NUL"),
                access,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                &attributes,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                std::ptr::null_mut(),
            );
            ensure!(
                handle != INVALID_HANDLE_VALUE,
                "Could not create publisher standard I/O."
            );
            return Ok(Handle(handle));
        }
        let mut handle = std::ptr::null_mut();
        ensure!(
            DuplicateHandle(
                GetCurrentProcess(),
                source,
                GetCurrentProcess(),
                &mut handle,
                0,
                1,
                DUPLICATE_SAME_ACCESS
            ) != 0,
            "Could not inherit publisher standard I/O."
        );
        Ok(Handle(handle))
    }
}

struct Attributes {
    storage: Vec<u128>,
    initialized: bool,
}
impl Attributes {
    fn pointer(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.storage.as_mut_ptr().cast()
    }
    fn new() -> Result<Self> {
        unsafe {
            let mut size = 0;
            InitializeProcThreadAttributeList(std::ptr::null_mut(), 2, 0, &mut size);
            ensure!(
                size > 0 && size <= 65536,
                "Invalid process attribute storage size."
            );
            let mut list = Self {
                storage: vec![0; size.div_ceil(16)],
                initialized: false,
            };
            ensure!(
                InitializeProcThreadAttributeList(list.pointer(), 2, 0, &mut size) != 0,
                "Could not initialize publisher creation attributes."
            );
            list.initialized = true;
            Ok(list)
        }
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        if self.initialized {
            unsafe {
                DeleteProcThreadAttributeList(self.pointer());
            }
        }
    }
}

pub(super) struct Process {
    handle: Handle,
    #[cfg(test)]
    pub(super) id: u32,
}
impl Process {
    pub(super) fn wait(self) -> Result<ExitStatus> {
        unsafe {
            ensure!(
                WaitForSingleObject(self.handle.0, INFINITE) == WAIT_OBJECT_0,
                "Could not wait for the publisher."
            );
            let mut code = 0;
            ensure!(
                GetExitCodeProcess(self.handle.0, &mut code) != 0,
                "Could not read the publisher exit status."
            );
            Ok(ExitStatus::from_raw(code))
        }
    }
    #[cfg(test)]
    pub(super) fn raw_handle(&self) -> HANDLE {
        self.handle.0
    }
}

pub(super) fn spawn(command: &Command, job: HANDLE, extra_flags: u32) -> Result<Process> {
    let application = wide(command.get_program())?;
    let mut line = Vec::new();
    append_argument(&mut line, command.get_program())?;
    for argument in command.get_args() {
        line.push(b' ' as u16);
        append_argument(&mut line, argument)?;
    }
    line.push(0);
    ensure!(line.len() <= 32767, "Publisher command line is too long.");
    let environment = environment(command)?;
    let directory = command
        .get_current_dir()
        .map(|path| wide(path.as_os_str()))
        .transpose()?;
    let input = standard_handle(STD_INPUT_HANDLE)?;
    let output = standard_handle(STD_OUTPUT_HANDLE)?;
    let error = standard_handle(STD_ERROR_HANDLE)?;
    let handles = [input.0, output.0, error.0];
    let jobs = [job];
    let mut attributes = Attributes::new()?;
    unsafe {
        ensure!(
            UpdateProcThreadAttribute(
                attributes.pointer(),
                0,
                PROC_THREAD_ATTRIBUTE_JOB_LIST as usize,
                jobs.as_ptr().cast(),
                std::mem::size_of_val(&jobs),
                std::ptr::null_mut(),
                std::ptr::null()
            ) != 0,
            "Could not set publisher job membership."
        );
        ensure!(
            UpdateProcThreadAttribute(
                attributes.pointer(),
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                handles.as_ptr().cast(),
                std::mem::size_of_val(&handles),
                std::ptr::null_mut(),
                std::ptr::null()
            ) != 0,
            "Could not limit publisher handle inheritance."
        );
        let mut startup = STARTUPINFOEXW::default();
        startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = input.0;
        startup.StartupInfo.hStdOutput = output.0;
        startup.StartupInfo.hStdError = error.0;
        startup.lpAttributeList = attributes.pointer();
        let mut info = PROCESS_INFORMATION::default();
        if CreateProcessW(
            application.as_ptr(),
            line.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
            CREATE_NO_WINDOW
                | CREATE_UNICODE_ENVIRONMENT
                | EXTENDED_STARTUPINFO_PRESENT
                | extra_flags,
            environment.as_ptr().cast(),
            directory
                .as_ref()
                .map_or(std::ptr::null(), |path| path.as_ptr()),
            &startup.StartupInfo,
            &mut info,
        ) == 0
        {
            return Err(std::io::Error::last_os_error())
                .context("Could not start the guarded publisher.");
        }
        let _thread = Handle(info.hThread);
        Ok(Process {
            handle: Handle(info.hProcess),
            #[cfg(test)]
            id: info.dwProcessId,
        })
    }
}
