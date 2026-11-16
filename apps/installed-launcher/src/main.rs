#![cfg_attr(windows, windows_subsystem = "windows")]
mod activation;
#[path = "../../activation_protocol.rs"]
mod activation_protocol;
#[path = "../../update_protocol.rs"]
mod protocol;
#[cfg(windows)]
mod registration;
#[cfg(target_os = "macos")]
#[path = "registration_macos.rs"]
mod registration;
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
    let app = velopack::VelopackApp::build()
        .set_args(hooks)
        .set_auto_apply_on_startup(false);
    #[cfg(windows)]
    let app = app
        .on_after_install_fast_callback(|_| {
            let _ = registration::register(true);
        })
        .on_after_update_fast_callback(|_| {
            let _ = registration::register(false);
        })
        .on_before_uninstall_fast_callback(|_| {
            let _ = registration::unregister();
            registration::clear();
        });
    let mut app = app;
    app.run();
    let link = if std::env::args().nth(1).as_deref() == Some("--browser-link") {
        let args: Vec<_> = std::env::args().skip(1).collect();
        if args.len() != 2 || !activation_protocol::valid(&args[1]) {
            notice("Invalid room link. Open a fresh invitation from your browser.");
            return;
        }
        Some(args[1].clone())
    } else {
        None
    };
    let instance = match activation::Instance::start() {
        Ok(Some(instance)) => instance,
        Ok(None) => {
            if activation::forward(link).is_err() {
                notice("PodeTelar is busy. Try opening the room link again.");
            }
            return;
        }
        Err(_) => {
            notice("PodeTelar could not prepare room links. Try reopening the application.");
            return;
        }
    };
    if std::env::current_exe()
        .ok()
        .and_then(|exe| {
            exe.parent()
                .and_then(std::path::Path::parent)
                .map(|root| root.join("Update.exe").exists())
        })
        .unwrap_or(false)
    {
        let _ = registration::register(false);
    }
    let bridge = updates::Bridge::start().ok();
    let result = std::env::current_exe().and_then(|exe| {
        #[cfg(windows)]
        let filename = "FreedomStream.exe";
        #[cfg(target_os = "macos")]
        let filename = "FreedomStream";
        let executable = exe.with_file_name(filename);
        #[cfg(target_os = "macos")]
        let executable = {
            let nested = exe
                .parent()
                .and_then(std::path::Path::parent)
                .ok_or(std::io::ErrorKind::NotFound)?
                .join("Helpers/PodeTelar.app/Contents/MacOS/FreedomStream");
            if nested.is_file() { nested } else { executable }
        };
        let mut child = std::process::Command::new(executable);
        #[cfg(target_os = "macos")]
        {
            let contents = exe
                .parent()
                .and_then(std::path::Path::parent)
                .ok_or(std::io::ErrorKind::NotFound)?;
            child
                .env("GST_PLUGIN_SYSTEM_PATH_1_0", contents.join("PlugIns"))
                .env("GST_PLUGIN_PATH_1_0", "")
                .env(
                    "GST_PLUGIN_SCANNER_1_0",
                    contents.join("MacOS/gst-plugin-scanner"),
                )
                .env(
                    "GST_REGISTRY_1_0",
                    registration::root()?.join("gstreamer-registry.bin"),
                );
        }
        child
            .env(activation_protocol::INSTALLED, &exe)
            .env(
                "FREEDOMSTREAM_INSTALLED_LINKS",
                if registration::enabled() { "1" } else { "0" },
            )
            .env(activation_protocol::ENDPOINT, &instance.address)
            .env(activation_protocol::TOKEN, &instance.token);
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
        notice("The update could not start. Reopen PodeTelar and try again.");
    }
    // Never log arguments: browser launch arguments can contain invitations.
    drop(instance);
    std::process::exit(result.ok().and_then(|status| status.code()).unwrap_or(1));
}

#[cfg(windows)]
fn notice(message: &str) {
    let text: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            windows_sys::w!("PodeTelar"),
            0x10,
        );
    }
}

#[cfg(target_os = "macos")]
fn notice(_message: &str) {
    // A fixed message only; never interpolate invitations into AppleScript.
    let _ = std::process::Command::new("/usr/bin/osascript").args(["-e", "display alert \"PodeTelar could not start. Reopen the application and try again.\" as critical"]).output();
}
