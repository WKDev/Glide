//! Snap preview overlay — a transparent Win32 window that shows where
//! the window will snap to when the user releases the modifier key.
//!
//! Architecture:
//! - The overlay HWND is created on the hook thread (which has a message loop).
//! - `show` / `hide` can be called from the worker thread — Win32 cross-thread
//!   window manipulation is safe (it posts messages to the owning thread).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWINDOWATTRIBUTE};
use windows::Win32::Graphics::Gdi::CreateSolidBrush;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, SetLayeredWindowAttributes,
    SetWindowPos, ShowWindow, LWA_ALPHA, SET_WINDOW_POS_FLAGS, SWP_NOACTIVATE, SW_HIDE,
    WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSW,
};

/// Wrapper for HWND to allow storage in OnceLock (HWND is a raw pointer
/// and doesn't implement Send/Sync in windows-rs 0.61).
/// This is safe because Win32 window handles are process-wide identifiers
/// that can be used from any thread.
#[derive(Clone, Copy)]
struct SendHwnd(HWND);
unsafe impl Send for SendHwnd {}
unsafe impl Sync for SendHwnd {}

static OVERLAY_HWND: OnceLock<SendHwnd> = OnceLock::new();
static OVERLAY_VISIBLE: AtomicBool = AtomicBool::new(false);

/// Semi-transparent blue fill (Tailwind blue-400 in BGR).
const OVERLAY_COLOR: COLORREF = COLORREF(0x00FA_A5_60);
/// Overlay opacity: 64/255 ≈ 25%
const OVERLAY_ALPHA: u8 = 64;

// Window style flags as raw values.
const WS_POPUP: WINDOW_STYLE = WINDOW_STYLE(0x8000_0000);
const WS_EX_LAYERED: WINDOW_EX_STYLE = WINDOW_EX_STYLE(0x0008_0000);
const WS_EX_TRANSPARENT: WINDOW_EX_STYLE = WINDOW_EX_STYLE(0x0000_0020);
const WS_EX_TOPMOST: WINDOW_EX_STYLE = WINDOW_EX_STYLE(0x0000_0008);
const WS_EX_TOOLWINDOW: WINDOW_EX_STYLE = WINDOW_EX_STYLE(0x0000_0080);
const WS_EX_NOACTIVATE: WINDOW_EX_STYLE = WINDOW_EX_STYLE(0x0800_0000);
const SWP_SHOWWINDOW: SET_WINDOW_POS_FLAGS = SET_WINDOW_POS_FLAGS(0x0040);

/// DWM Window Corner Preference (Win11+).
const DWMWA_WINDOW_CORNER_PREFERENCE: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(33);
const DWMWCP_ROUND: i32 = 2;

fn wide_string(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe extern "system" fn overlay_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// Create the overlay window. Must be called from a thread with a message loop
/// (the hook thread).
pub fn create() {
    let class_name = wide_string("glide_snap_overlay");

    let instance = unsafe { GetModuleHandleW(None) }.unwrap_or_default();
    let brush = unsafe { CreateSolidBrush(OVERLAY_COLOR) };

    let wc = WNDCLASSW {
        lpfnWndProc: Some(overlay_wndproc),
        hInstance: instance.into(),
        lpszClassName: PCWSTR(class_name.as_ptr()),
        hbrBackground: brush,
        ..Default::default()
    };

    let atom = unsafe { RegisterClassW(&wc) };
    if atom == 0 {
        log::error!("overlay: RegisterClassW failed");
        return;
    }

    let ex_style = WINDOW_EX_STYLE(
        WS_EX_LAYERED.0
            | WS_EX_TRANSPARENT.0
            | WS_EX_TOPMOST.0
            | WS_EX_TOOLWINDOW.0
            | WS_EX_NOACTIVATE.0,
    );

    let hwnd = match unsafe {
        CreateWindowExW(
            ex_style,
            PCWSTR(class_name.as_ptr()),
            None,
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance.into()),
            None,
        )
    } {
        Ok(h) => h,
        Err(e) => {
            log::error!("overlay: CreateWindowExW failed: {}", e);
            return;
        }
    };

    if hwnd.is_invalid() {
        log::error!("overlay: CreateWindowExW returned invalid HWND");
        return;
    }

    // Set transparency alpha.
    let _ = unsafe { SetLayeredWindowAttributes(hwnd, COLORREF(0), OVERLAY_ALPHA, LWA_ALPHA) };

    // Try rounded corners on Win11+ (silently fails on Win10).
    let _ = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &DWMWCP_ROUND as *const i32 as *const _,
            std::mem::size_of::<i32>() as u32,
        )
    };

    let _ = OVERLAY_HWND.set(SendHwnd(hwnd));
    log::info!("overlay: created hwnd={:?}", hwnd);
}

/// Show the overlay at the given screen rect (snap zone destination).
///
/// Safe to call from any thread — SetWindowPos posts to the owning thread.
pub fn show(rect: RECT) {
    let Some(&SendHwnd(hwnd)) = OVERLAY_HWND.get() else {
        return;
    };

    let topmost = HWND(-1isize as *mut std::ffi::c_void);

    unsafe {
        let _ = SetWindowPos(
            hwnd,
            Some(topmost),
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SET_WINDOW_POS_FLAGS(SWP_NOACTIVATE.0 | SWP_SHOWWINDOW.0),
        );
    }

    OVERLAY_VISIBLE.store(true, Ordering::Relaxed);
}

/// Hide the overlay. No-op if already hidden.
pub fn hide() {
    if !OVERLAY_VISIBLE.load(Ordering::Relaxed) {
        return;
    }

    if let Some(&SendHwnd(hwnd)) = OVERLAY_HWND.get() {
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
    }

    OVERLAY_VISIBLE.store(false, Ordering::Relaxed);
}

/// Destroy the overlay window. Call during shutdown.
pub fn destroy() {
    if let Some(&SendHwnd(hwnd)) = OVERLAY_HWND.get() {
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
        log::info!("overlay: destroyed");
    }
}
