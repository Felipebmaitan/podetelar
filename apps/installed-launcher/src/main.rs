#![cfg_attr(windows, windows_subsystem = "windows")]
#[path = "../../update_protocol.rs"]
mod protocol;
mod updates;

fn main() {
    // Hooks must run before runtime extraction, media initialization or room restore.
    // This process is outside the portable launcher's kill-on-close child job.
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Velopack logs its startup arguments. Give it only its own hooks, never
    // browser invitations. Also require an explicit update action in the future.
    let hooks = if args.len() == 2
        && matches!(
            args[0].as_str(),
            "--veloapp-install"
                | "--veloapp-updated"
                | "--veloapp-obsolete"
                | "--veloapp-uninstall"
        )
        && !args[1].is_empty()
        && args[1].len() <= 80
        && args[1]
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b".-+".contains(&c))
    {
        args
    } else {
        Vec::new()
    };
    velopack::VelopackApp::build()
        .set_args(hooks)
        .set_auto_apply_on_startup(false)
        .run();
    let _instance = match instance_lock() {
        Ok(lock) => lock,
        Err(_) => {
            notice("FreedomStream is already running, or its installation is unavailable.");
            return;
        }
    };
    let bridge = updates::Bridge::start().ok();
    let result = std::env::current_exe().and_then(|exe| {
        let mut child = std::process::Command::new(exe.with_file_name("FreedomStream.exe"));
        if let Some(bridge) = &bridge {
            child
                .env(protocol::ENDPOINT_ENV, &bridge.address)
                .env(protocol::TOKEN_ENV, &bridge.token);
        } else {
            child
                .env_remove(protocol::ENDPOINT_ENV)
                .env_remove(protocol::TOKEN_ENV);
        }
        child.args(std::env::args_os().skip(1));
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            child.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        child.status()
    });
    if let Some(bridge) = bridge
        && !bridge.child_exited()
    {
        notice("The update could not start. Reopen FreedomStream and try again.");
    }
    // Never log arguments: browser launch arguments can contain invitations.
    std::process::exit(result.ok().and_then(|status| status.code()).unwrap_or(1));
}

fn instance_lock() -> std::io::Result<std::fs::File> {
    let exe = std::env::current_exe()?;
    let root = exe
        .parent()
        .and_then(std::path::Path::parent)
        .ok_or(std::io::ErrorKind::NotFound)?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    options.open(root.join("installed-launcher.lock"))
}

fn notice(message: &str) {
    #[cfg(windows)]
    unsafe {
        let text: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
        windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            windows_sys::w!("FreedomStream"),
            0x10,
        );
    }
    #[cfg(not(windows))]
    let _ = message;
}
