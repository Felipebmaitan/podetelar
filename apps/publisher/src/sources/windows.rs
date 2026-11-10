use super::{Source, SourceInfo, Target};
use windows::Win32::{
    Foundation::{HWND, LPARAM, RECT},
    Graphics::Dwm::{DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute},
    Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO},
    UI::WindowsAndMessaging::{
        EnumWindows, GWL_EXSTYLE, GetWindowLongPtrW, GetWindowTextW, GetWindowThreadProcessId,
        IsIconic, IsWindow, IsWindowVisible, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    },
};
use windows::core::BOOL;

pub fn enumerate() -> Vec<Source> {
    let mut sources = Vec::new();
    // Callbacks only borrow this vector for the synchronous enumeration call.
    unsafe {
        let context = LPARAM(&mut sources as *mut Vec<Source> as isize);
        let _ = EnumDisplayMonitors(None, None, Some(monitor), context);
        let _ = EnumWindows(Some(window), context);
    }
    sources
}

unsafe extern "system" fn monitor(handle: HMONITOR, _: HDC, _: *mut RECT, context: LPARAM) -> BOOL {
    let sources = unsafe { &mut *(context.0 as *mut Vec<Source>) };
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetMonitorInfoW(handle, &mut info) }.as_bool() {
        let width = (info.rcMonitor.right - info.rcMonitor.left).max(0) as u32;
        let height = (info.rcMonitor.bottom - info.rcMonitor.top).max(0) as u32;
        let raw = handle.0 as usize as u64;
        sources.push(Source {
            info: SourceInfo {
                id: format!("monitor:{raw}"),
                label: format!("Monitor {} · {width}×{height}", sources.len() + 1),
                width,
                height,
            },
            target: Target::Monitor(raw),
        });
    }
    true.into()
}

unsafe extern "system" fn window(handle: HWND, context: LPARAM) -> BOOL {
    if !available_window(handle) {
        return true.into();
    }
    let mut title = [0_u16; 512];
    let length = unsafe { GetWindowTextW(handle, &mut title) };
    if length <= 0 {
        return true.into();
    }
    let mut rect = RECT::default();
    // WGC captures the painted DWM frame, not GetWindowRect's invisible resize
    // margins. Inflated dimensions would make Source accidentally upscale.
    if unsafe {
        DwmGetWindowAttribute(
            handle,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut _ as *mut _,
            std::mem::size_of::<RECT>() as u32,
        )
    }
    .is_err()
    {
        return true.into();
    }
    let width = (rect.right - rect.left).max(0) as u32;
    let height = (rect.bottom - rect.top).max(0) as u32;
    if width < 144 || height < 128 {
        return true.into();
    }
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(handle, Some(&mut pid)) };
    if pid == std::process::id() {
        return true.into();
    }
    let raw = handle.0 as usize as u64;
    let sources = unsafe { &mut *(context.0 as *mut Vec<Source>) };
    sources.push(Source {
        info: SourceInfo {
            id: format!("window:{raw}:{pid}"),
            label: String::from_utf16_lossy(&title[..length as usize]),
            width,
            height,
        },
        target: Target::Window { handle: raw, pid },
    });
    true.into()
}

// Discovery and preview availability are stricter than validation of an active
// publication. Minimizing/alt-tabbing must not itself revoke an existing source.
fn available_window(handle: HWND) -> bool {
    unsafe {
        if !IsWindow(Some(handle)).as_bool()
            || !IsWindowVisible(handle).as_bool()
            || IsIconic(handle).as_bool()
        {
            return false;
        }
        let style = GetWindowLongPtrW(handle, GWL_EXSTYLE);
        if style & (WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0) as isize != 0 {
            return false;
        }
        let mut cloaked = 0_u32;
        DwmGetWindowAttribute(
            handle,
            DWMWA_CLOAKED,
            &mut cloaked as *mut _ as *mut _,
            std::mem::size_of_val(&cloaked) as u32,
        )
        .is_ok()
            && cloaked == 0
    }
}

pub fn validate(source: &Source) -> bool {
    match source.target {
        Target::Pattern => true,
        Target::Monitor(raw) | Target::MonitorWgc(raw) => {
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            unsafe { GetMonitorInfoW(HMONITOR(raw as usize as *mut _), &mut info) }.as_bool()
        }
        Target::Window { handle, pid } => {
            let handle = HWND(handle as usize as *mut _);
            let mut current_pid = 0;
            unsafe { GetWindowThreadProcessId(handle, Some(&mut current_pid)) };
            unsafe { IsWindow(Some(handle)) }.as_bool() && current_pid == pid
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::{
        Win32::{
            Graphics::Dwm::{DWMWA_CLOAK, DwmSetWindowAttribute},
            UI::WindowsAndMessaging::{
                CreateWindowExW, DestroyWindow, SW_HIDE, SW_MINIMIZE, SW_SHOWNOACTIVATE,
                ShowWindow, WINDOW_EX_STYLE, WS_EX_APPWINDOW, WS_OVERLAPPEDWINDOW,
            },
        },
        core::w,
    };

    struct Window(HWND);
    impl Drop for Window {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyWindow(self.0);
            }
        }
    }
    fn fixture(style: WINDOW_EX_STYLE) -> Window {
        Window(
            unsafe {
                CreateWindowExW(
                    style,
                    w!("STATIC"),
                    w!("FreedomStream discovery test"),
                    WS_OVERLAPPEDWINDOW,
                    -30000,
                    -30000,
                    300,
                    200,
                    None,
                    None,
                    None,
                    None,
                )
            }
            .expect("Create offscreen test window"),
        )
    }

    #[test]
    #[ignore = "Requires an interactive Windows desktop; uses only offscreen fixture windows"]
    fn real_windows_hide_cloaked_minimized_and_tool_sources_without_revoking_live_handles() {
        let normal = fixture(WS_EX_APPWINDOW);
        assert!(!available_window(normal.0));
        unsafe {
            let _ = ShowWindow(normal.0, SW_SHOWNOACTIVATE);
        }
        assert!(available_window(normal.0), "Ordinary visible app window");
        let active = Source {
            info: SourceInfo {
                id: "test".into(),
                label: "test".into(),
                width: 300,
                height: 200,
            },
            target: Target::Window {
                handle: normal.0.0 as usize as u64,
                pid: std::process::id(),
            },
        };
        unsafe {
            let _ = ShowWindow(normal.0, SW_MINIMIZE);
        }
        assert!(!available_window(normal.0));
        assert!(validate(&active));
        unsafe {
            let _ = ShowWindow(normal.0, SW_SHOWNOACTIVATE);
        }
        let cloak = 1_i32;
        unsafe {
            DwmSetWindowAttribute(
                normal.0,
                DWMWA_CLOAK,
                &cloak as *const _ as *const _,
                std::mem::size_of_val(&cloak) as u32,
            )
        }
        .expect("Cloak fixture");
        assert!(!available_window(normal.0));
        assert!(validate(&active));
        unsafe {
            let _ = ShowWindow(normal.0, SW_HIDE);
        }
        assert!(!available_window(normal.0));
        let tool = fixture(WS_EX_TOOLWINDOW);
        unsafe {
            let _ = ShowWindow(tool.0, SW_SHOWNOACTIVATE);
        }
        assert!(!available_window(tool.0));
        let inactive = fixture(WS_EX_NOACTIVATE);
        unsafe {
            let _ = ShowWindow(inactive.0, SW_SHOWNOACTIVATE);
        }
        assert!(!available_window(inactive.0));
        drop(normal);
        assert!(!validate(&active));
    }
}
