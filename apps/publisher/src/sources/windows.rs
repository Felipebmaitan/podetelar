use super::{Source, SourceInfo, Target};
use windows::Win32::{
    Foundation::{HWND, LPARAM, RECT},
    Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute},
    Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO},
    UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindow, IsWindowVisible,
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
    if !unsafe { IsWindowVisible(handle) }.as_bool() {
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
