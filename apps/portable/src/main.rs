#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]
use anyhow::{Context, Result, ensure};
use freedomstream_portable::Bundle;
use std::path::PathBuf;

fn run() -> Result<i32> {
    ensure!(cfg!(windows), "The portable publisher requires Windows.");
    let bundle = Bundle::read(&std::env::current_exe()?)?;
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--extract") {
        ensure!(
            args.len() == 2,
            "Usage: FreedomStream.exe --extract <new absolute folder>"
        );
        bundle.extract_new(&PathBuf::from(&args[1]))?;
        return Ok(0);
    }
    #[cfg(windows)]
    if args
        .first()
        .is_some_and(|arg| arg == "--enable-browser-links" || arg == "--disable-browser-links")
    {
        ensure!(args.len() == 1, "Invalid browser-link settings request.");
        if args[0] == "--enable-browser-links" {
            freedomstream_portable::browser_links::enable(&std::env::current_exe()?, &bundle.id)?;
        } else {
            freedomstream_portable::browser_links::disable()?;
        }
        return Ok(0);
    }
    if args.first().is_some_and(|arg| arg == "--browser-link") {
        ensure!(
            args.len() == 2 && args[1].to_string_lossy().len() <= 4096,
            "Invalid browser invitation."
        );
    }
    let base =
        PathBuf::from(std::env::var_os("LOCALAPPDATA").context("Local AppData is unavailable.")?)
            .join("FreedomStreamPortable");
    let directory = bundle.prepare(&base)?;
    // The verified runtime directory owns what launch needs. Do not retain the
    // complete compressed payload throughout the supervised child's lifetime.
    drop(bundle);
    if args.first().is_some_and(|arg| arg == "--licenses") {
        ensure!(args.len() == 1, "Usage: FreedomStream.exe --licenses");
        #[cfg(windows)]
        std::process::Command::new(
            PathBuf::from(std::env::var_os("WINDIR").context("Windows directory is unavailable.")?)
                .join("explorer.exe"),
        )
        .arg(directory.join("notices"))
        .spawn()?;
        return Ok(0);
    }
    let bundled_probe = args.first().is_some_and(|arg| arg == "--bundled-probe");
    let mut command = freedomstream_portable::command(&directory)?;
    command.env("FREEDOMSTREAM_PORTABLE_EXE", std::env::current_exe()?);
    #[cfg(windows)]
    command.env(
        "FREEDOMSTREAM_BROWSER_LINKS",
        if (std::env::var_os("FREEDOMSTREAM_INSTALLED_EXE").is_some()
            && std::env::var("FREEDOMSTREAM_INSTALLED_LINKS").as_deref() == Ok("1"))
            || freedomstream_portable::browser_links::enabled()
        {
            "1"
        } else {
            "0"
        },
    );
    if bundled_probe {
        ensure!(args.len() == 1, "Usage: FreedomStream.exe --bundled-probe");
        command.arg("--probe");
    } else {
        command.args(args);
    }
    Ok(
        freedomstream_portable::run_publisher(&mut command, bundled_probe)?
            .code()
            .unwrap_or(1),
    )
}

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("FreedomStream: {error:#}");
            if std::env::args_os().len() == 1 {
                #[cfg(windows)]
                unsafe {
                    let text: Vec<u16> = format!("FreedomStream could not start.\n\n{error:#}\n\nDownload a fresh copy or check the runtime cache in Local AppData.")
                        .encode_utf16().chain(Some(0)).collect();
                    windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
                        std::ptr::null_mut(),
                        text.as_ptr(),
                        windows_sys::w!("FreedomStream"),
                        0x10,
                    );
                }
            }
            std::process::exit(1);
        }
    }
}
