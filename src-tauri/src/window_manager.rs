use std::collections::HashSet;
use std::mem;
use std::path::Path;

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetAncestor, GetClassNameW, GetDesktopWindow, GetWindowLongW, GetWindowPlacement,
    GetWindowRect, GetWindowThreadProcessId, IsWindow, IsWindowVisible, IsZoomed,
    SetWindowPos, ShowWindow, WindowFromPoint, GA_ROOT, GWL_STYLE, SET_WINDOW_POS_FLAGS,
    SWP_NOACTIVATE, SWP_NOZORDER, SW_RESTORE, WINDOWPLACEMENT, WS_CHILD,
};

/// Flags not exported by the `windows` crate v0.61 — raw Win32 values.
const SWP_NOSIZE: SET_WINDOW_POS_FLAGS = SET_WINDOW_POS_FLAGS(0x0001);
const SWP_NOOWNERZORDER: SET_WINDOW_POS_FLAGS = SET_WINDOW_POS_FLAGS(0x0200);

/// AltSnap MOVETHICKBORDERS: synchronous move, no size change.
/// Most Win10/11 windows have thick (invisible) DWM borders, so AltSnap
/// uses synchronous positioning for them rather than ASYNCWINDOWPOS.
const MOVE_FLAGS: SET_WINDOW_POS_FLAGS = SET_WINDOW_POS_FLAGS(
    SWP_NOZORDER.0 | SWP_NOOWNERZORDER.0 | SWP_NOACTIVATE.0 | SWP_NOSIZE.0,
);

/// AltSnap RESIZEFLAG: synchronous, size may change.
const RESIZE_FLAGS: SET_WINDOW_POS_FLAGS =
    SET_WINDOW_POS_FLAGS(SWP_NOZORDER.0 | SWP_NOOWNERZORDER.0 | SWP_NOACTIVATE.0);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn is_system_class_name(class_name: &str) -> bool {
    let lower = class_name.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "shell_traywnd" | "progman" | "workerw" | "shell_secondarytraywnd"
    )
}

fn get_window_class_name(hwnd: HWND) -> Option<String> {
    let mut buffer = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, &mut buffer) };
    if len <= 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buffer[..len as usize]))
}

fn is_system_window(hwnd: HWND) -> bool {
    get_window_class_name(hwnd)
        .map(|name| is_system_class_name(&name))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Window queries
// ---------------------------------------------------------------------------

pub fn window_from_point(x: i32, y: i32) -> Option<HWND> {
    let hwnd = unsafe { WindowFromPoint(POINT { x, y }) };
    if hwnd.is_invalid() {
        return None;
    }

    let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
    if root.is_invalid() {
        return None;
    }

    if root == unsafe { GetDesktopWindow() } || is_system_window(root) {
        return None;
    }

    Some(root)
}

pub fn get_process_name(hwnd: HWND) -> Option<String> {
    let mut process_id = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
    }
    if process_id == 0 {
        return None;
    }

    let process =
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id).ok()? };

    let mut buffer = vec![0u16; 1024];
    let mut size = buffer.len() as u32;
    let query_result = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        )
    };

    let _ = unsafe { CloseHandle(process) };
    if query_result.is_err() || size == 0 {
        return None;
    }

    let full_path = String::from_utf16_lossy(&buffer[..size as usize]);
    let file_name = Path::new(&full_path).file_name()?.to_str()?;
    Some(file_name.to_string())
}

/// Returns the window rect as reported by the OS (includes DWM invisible borders).
pub fn get_window_rect(hwnd: HWND) -> Option<RECT> {
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_ok() {
        Some(rect)
    } else {
        None
    }
}

/// Returns the DWM extended-frame rect (visible content area, no invisible borders).
/// Falls back to `GetWindowRect` if DWM attribute is unavailable.
pub fn get_dwm_frame_rect(hwnd: HWND) -> Option<RECT> {
    let mut rect = RECT::default();
    let hr = unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut RECT as *mut _,
            mem::size_of::<RECT>() as u32,
        )
    };
    if hr.is_ok() {
        Some(rect)
    } else {
        get_window_rect(hwnd)
    }
}

/// Compute the invisible-border offsets: (left, top, right, bottom).
/// For a typical Win10/11 window: left ~= 7, top ~= 0, right ~= 7, bottom ~= 7.
pub fn get_border_offsets(hwnd: HWND) -> (i32, i32, i32, i32) {
    let Some(outer) = get_window_rect(hwnd) else {
        return (0, 0, 0, 0);
    };
    let Some(inner) = get_dwm_frame_rect(hwnd) else {
        return (0, 0, 0, 0);
    };
    (
        inner.left - outer.left,
        inner.top - outer.top,
        outer.right - inner.right,
        inner.bottom - outer.bottom,
    )
}

pub fn is_maximized(hwnd: HWND) -> bool {
    unsafe { IsZoomed(hwnd).as_bool() }
}

/// Detect if a window is in a Windows Snap state (aero-snapped but not maximized).
/// Heuristic: the window placement's "restored" rect differs from its actual rect,
/// and the window is NOT maximized.
pub fn is_snapped(hwnd: HWND) -> bool {
    if is_maximized(hwnd) {
        return false;
    }

    let mut placement: WINDOWPLACEMENT = unsafe { mem::zeroed() };
    placement.length = mem::size_of::<WINDOWPLACEMENT>() as u32;
    if unsafe { GetWindowPlacement(hwnd, &mut placement) }.is_err() {
        return false;
    }

    let restored = placement.rcNormalPosition;
    let Some(actual) = get_window_rect(hwnd) else {
        return false;
    };

    // If the actual rect differs from the placement's "normal" rect,
    // the window is in a snapped/tiled state managed by DWM.
    restored.left != actual.left
        || restored.top != actual.top
        || restored.right != actual.right
        || restored.bottom != actual.bottom
}

pub fn restore_window(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_RESTORE);
    }
}

// ---------------------------------------------------------------------------
// Window positioning — matches AltSnap flag patterns
// ---------------------------------------------------------------------------

/// Move window to (x, y) without changing size.
/// Synchronous call — the window moves immediately, providing the most
/// responsive feel.  Matches AltSnap's `MOVETHICKBORDERS` pattern used
/// for the vast majority of modern Win10/11 windows.
pub fn move_window(hwnd: HWND, x: i32, y: i32) {
    unsafe {
        let _ = SetWindowPos(hwnd, None, x, y, 0, 0, MOVE_FLAGS);
    }
}

/// Resize (and reposition) window to the given rect.
/// Uses synchronous flags because the size is changing.
pub fn resize_window(hwnd: HWND, x: i32, y: i32, w: i32, h: i32) {
    if w <= 0 || h <= 0 {
        return;
    }
    unsafe {
        let _ = SetWindowPos(hwnd, None, x, y, w, h, RESIZE_FLAGS);
    }
}

/// Legacy combined call — kept for any call site that doesn't distinguish
/// move vs resize.  Prefers async move when size is unchanged.
pub fn set_window_rect(hwnd: HWND, x: i32, y: i32, w: i32, h: i32) {
    if w <= 0 || h <= 0 {
        return;
    }
    // When only position changes, use async move path.
    if let Some(cur) = get_window_rect(hwnd) {
        let cur_w = cur.right - cur.left;
        let cur_h = cur.bottom - cur.top;
        if cur_w == w && cur_h == h {
            move_window(hwnd, x, y);
            return;
        }
    }
    resize_window(hwnd, x, y, w, h);
}

// ---------------------------------------------------------------------------
// Enumeration
// ---------------------------------------------------------------------------

pub fn is_valid_target(hwnd: HWND) -> bool {
    if hwnd.is_invalid() {
        return false;
    }

    if !unsafe { IsWindow(Some(hwnd)).as_bool() } {
        return false;
    }

    if !unsafe { IsWindowVisible(hwnd).as_bool() } {
        return false;
    }

    if hwnd == unsafe { GetDesktopWindow() } || is_system_window(hwnd) {
        return false;
    }

    let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
    if root != hwnd {
        return false;
    }

    let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;
    if style & WS_CHILD.0 != 0 {
        return false;
    }

    true
}

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
    if !is_valid_target(hwnd) {
        return windows::core::BOOL(1);
    }

    if let Some(name) = get_process_name(hwnd) {
        let names = &mut *(lparam.0 as *mut HashSet<String>);
        names.insert(name);
    }

    windows::core::BOOL(1)
}

pub fn get_running_process_names() -> Vec<String> {
    let mut names = HashSet::<String>::new();
    let ptr = &mut names as *mut HashSet<String>;

    unsafe {
        let _ = EnumWindows(Some(enum_windows_proc), LPARAM(ptr as isize));
    }

    let mut result: Vec<String> = names.into_iter().collect();
    result.sort_unstable_by_key(|s| s.to_ascii_lowercase());
    result
}
