#![cfg_attr(windows, windows_subsystem = "windows")]

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
    let result = std::env::current_exe().and_then(|exe| {
        let mut child = std::process::Command::new(exe.with_file_name("FreedomStream.exe"));
        child.args(std::env::args_os().skip(1));
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            child.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        child.status()
    });
    // Never log arguments: browser launch arguments can contain invitations.
    std::process::exit(result.ok().and_then(|status| status.code()).unwrap_or(1));
}
