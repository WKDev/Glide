use std::collections::HashSet;
use std::mem;
use std::path::Path;

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, COLORREF, HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetAncestor, GetClassNameW, GetDesktopWindow, GetForegroundWindow,
    GetLayeredWindowAttributes, GetWindowLongW, GetWindowPlacement, GetWindowRect,
    GetWindowThreadProcessId, IsWindow, IsWindowVisible, IsZoomed, SetForegroundWindow,
    SetLayeredWindowAttributes, SetWindowLongW, SetWindowPos, ShowWindow, WindowFromPoint, GA_ROOT,
    GWL_EXSTYLE, GWL_STYLE, LAYERED_WINDOW_ATTRIBUTES_FLAGS, LWA_ALPHA, SET_WINDOW_POS_FLAGS,
    SWP_NOACTIVATE, SWP_NOZORDER, SW_MAXIMIZE, SW_RESTORE, WINDOWPLACEMENT, WS_CHILD,
    WS_EX_LAYERED,
};

/// Flags not exported by the `windows` crate v0.61 — raw Win32 values.
const SWP_NOSIZE: SET_WINDOW_POS_FLAGS = SET_WINDOW_POS_FLAGS(0x0001);
const SWP_NOMOVE: SET_WINDOW_POS_FLAGS = SET_WINDOW_POS_FLAGS(0x0002);
const SWP_NOOWNERZORDER: SET_WINDOW_POS_FLAGS = SET_WINDOW_POS_FLAGS(0x0200);

/// AltSnap MOVETHICKBORDERS: synchronous move, no size change.
/// Most Win10/11 windows have thick (invisible) DWM borders, so AltSnap
/// uses synchronous positioning for them rather than ASYNCWINDOWPOS.
const MOVE_FLAGS: SET_WINDOW_POS_FLAGS =
    SET_WINDOW_POS_FLAGS(SWP_NOZORDER.0 | SWP_NOOWNERZORDER.0 | SWP_NOACTIVATE.0 | SWP_NOSIZE.0);

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

pub fn get_foreground_window() -> Option<HWND> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_invalid() {
        None
    } else {
        Some(hwnd)
    }
}

/// Attempts to bring `hwnd` to the foreground. Returns `true` if the OS accepted
/// the request, `false` if it was silently denied (Windows foreground-lock policy).
pub fn set_foreground(hwnd: HWND) -> bool {
    unsafe { SetForegroundWindow(hwnd).as_bool() }
}

// ---------------------------------------------------------------------------
// Monitor info
// ---------------------------------------------------------------------------

/// Get the working area (excludes taskbar) of the monitor containing the given point.
pub fn get_monitor_work_area(point: POINT) -> Option<RECT> {
    unsafe {
        let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        if monitor.is_invalid() {
            return None;
        }
        let mut info: MONITORINFO = mem::zeroed();
        info.cbSize = mem::size_of::<MONITORINFO>() as u32;
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return None;
        }
        Some(info.rcWork)
    }
}

// ---------------------------------------------------------------------------
// Opacity (SetLayeredWindowAttributes)
// ---------------------------------------------------------------------------

/// Get current window opacity (0–255). Returns 255 if window is not layered.
pub fn get_window_opacity(hwnd: HWND) -> u8 {
    let ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;
    if (ex_style & WS_EX_LAYERED.0) == 0 {
        return 255;
    }
    let mut alpha: u8 = 255;
    let mut flags = LAYERED_WINDOW_ATTRIBUTES_FLAGS(0);
    let result =
        unsafe { GetLayeredWindowAttributes(hwnd, None, Some(&mut alpha), Some(&mut flags)) };
    if result.is_ok() && (flags.0 & LWA_ALPHA.0) != 0 {
        alpha
    } else {
        255
    }
}

/// Set window opacity. 255 = fully opaque (removes WS_EX_LAYERED), <255 = translucent.
pub fn set_window_opacity(hwnd: HWND, alpha: u8) {
    let ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;

    if alpha == 255 {
        // Remove WS_EX_LAYERED to restore normal rendering
        if (ex_style & WS_EX_LAYERED.0) != 0 {
            unsafe {
                SetWindowLongW(hwnd, GWL_EXSTYLE, (ex_style & !WS_EX_LAYERED.0) as i32);
            }
        }
        return;
    }

    // Add WS_EX_LAYERED if not present
    if (ex_style & WS_EX_LAYERED.0) == 0 {
        unsafe {
            SetWindowLongW(hwnd, GWL_EXSTYLE, (ex_style | WS_EX_LAYERED.0) as i32);
        }
    }

    unsafe {
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
    }
}

// ---------------------------------------------------------------------------
// Z-order
// ---------------------------------------------------------------------------

/// Raise window to the foreground Z-order.
/// Combines SetForegroundWindow (most reliable, activates the window) with
/// SetWindowPos(HWND_TOP) as a belt-and-suspenders fallback.
pub fn raise_to_top(hwnd: HWND) {
    // SetForegroundWindow is the standard Windows API for bringing a window to
    // the top.  It activates the window — which is the expected behaviour for
    // "raise on grab" since the user is interacting with it.
    unsafe {
        let _ = SetForegroundWindow(hwnd);
    }

    // Fallback: also try SetWindowPos(HWND_TOP) without SWP_NOACTIVATE so the
    // Z-order change takes effect even if SetForegroundWindow was denied by the
    // OS foreground-lock policy.
    let hwnd_top = HWND(std::ptr::null_mut::<std::ffi::c_void>()); // HWND_TOP
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(hwnd_top),
            0,
            0,
            0,
            0,
            SET_WINDOW_POS_FLAGS(SWP_NOMOVE.0 | SWP_NOSIZE.0),
        );
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

/// Maximize the window via the OS-native `SW_MAXIMIZE` command.
/// This puts the window into the DWM-tracked maximised state (Snap Assist,
/// taskbar peek, and restore-on-drag all work correctly).
pub fn maximize_window(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_MAXIMIZE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_system_class_name_shell_traywnd() {
        assert!(is_system_class_name("shell_traywnd"));
    }

    #[test]
    fn test_is_system_class_name_progman() {
        assert!(is_system_class_name("progman"));
    }

    #[test]
    fn test_is_system_class_name_workerw() {
        assert!(is_system_class_name("workerw"));
    }

    #[test]
    fn test_is_system_class_name_shell_secondarytraywnd() {
        assert!(is_system_class_name("shell_secondarytraywnd"));
    }

    #[test]
    fn test_is_system_class_name_case_insensitive() {
        assert!(is_system_class_name("Shell_TrayWnd"));
        assert!(is_system_class_name("PROGMAN"));
        assert!(is_system_class_name("WorkerW"));
    }

    #[test]
    fn test_is_system_class_name_non_system() {
        assert!(!is_system_class_name("notepad"));
        assert!(!is_system_class_name("chrome_widgetwin_1"));
        assert!(!is_system_class_name(""));
        assert!(!is_system_class_name("shell_tray"));
        assert!(!is_system_class_name("shell_traywnd_extra"));
    }
}
