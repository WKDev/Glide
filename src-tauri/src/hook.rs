use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, OnceLock};
use std::thread;

use parking_lot::Mutex;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_RCONTROL,
    VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, WH_KEYBOARD_LL,
    WH_MOUSE_LL, WM_KEYDOWN, WM_MOUSEMOVE, WM_QUIT, WM_SYSKEYDOWN,
};

use crate::config::{AppConfig, FilterMode, ModifierKey};
use crate::window_manager;

const MOD_ALT: u32 = 1;
const MOD_CTRL: u32 = 2;
const MOD_SHIFT: u32 = 4;
const MOD_WIN: u32 = 8;
const MIN_WINDOW_SIZE: i32 = 100;
const WORKER_QUEUE_SIZE: usize = 1024;

static MODIFIER_STATE: AtomicU32 = AtomicU32::new(0);
static HOOK_ENABLED: AtomicBool = AtomicBool::new(true);
static ACTIVE_GRAB: AtomicBool = AtomicBool::new(false);
static SHARED_CONFIG: OnceLock<Arc<Mutex<AppConfig>>> = OnceLock::new();
static WORKER_TX: OnceLock<SyncSender<WorkerEvent>> = OnceLock::new();

/// Pre-computed modifier masks for the hook thread to decide swallowing
/// synchronously, without waiting for the worker.
static MOVE_MASK: AtomicU32 = AtomicU32::new(MOD_ALT);
static RESIZE_MASK: AtomicU32 = AtomicU32::new(MOD_ALT | MOD_SHIFT);

#[derive(Debug, Clone, Copy, PartialEq)]
enum DragMode {
    Move,
    Resize,
}

#[derive(Clone, Copy)]
enum ResizeDirection {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone, Copy)]
struct GrabState {
    mode: DragMode,
    hwnd: HWND,
    last_cursor: POINT,
    /// Window rect captured at grab start (or mode switch).
    /// Position is computed as origin + cumulative delta, making us
    /// immune to external actors (Snap, DWM) resetting the position.
    origin_rect: RECT,
    cumulative_dx: i32,
    cumulative_dy: i32,
    resize_dir: ResizeDirection,
}

/// Worker event carrying the modifier snapshot from the hook thread.
#[derive(Clone, Copy)]
enum WorkerEvent {
    MouseMove { point: POINT, mods: u32 },
}

fn current_config() -> Option<AppConfig> {
    SHARED_CONFIG.get().map(|shared| shared.lock().clone())
}

fn modifier_to_mask(modifier: ModifierKey) -> u32 {
    match modifier {
        ModifierKey::Alt => MOD_ALT,
        ModifierKey::Ctrl => MOD_CTRL,
        ModifierKey::Shift => MOD_SHIFT,
        ModifierKey::Win => MOD_WIN,
    }
}

fn key_to_mask(vk_code: u32) -> Option<u32> {
    if vk_code == VK_LMENU.0 as u32 || vk_code == VK_RMENU.0 as u32 || vk_code == VK_MENU.0 as u32 {
        return Some(MOD_ALT);
    }
    if vk_code == VK_LCONTROL.0 as u32
        || vk_code == VK_RCONTROL.0 as u32
        || vk_code == VK_CONTROL.0 as u32
    {
        return Some(MOD_CTRL);
    }
    if vk_code == VK_LSHIFT.0 as u32
        || vk_code == VK_RSHIFT.0 as u32
        || vk_code == VK_SHIFT.0 as u32
    {
        return Some(MOD_SHIFT);
    }
    if vk_code == VK_LWIN.0 as u32 || vk_code == VK_RWIN.0 as u32 {
        return Some(MOD_WIN);
    }
    None
}

fn is_virtual_key_down(vk: i32) -> bool {
    (unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000) != 0
}

/// Poll the physical keyboard state for all modifier keys.
/// Used only for initial sync on startup.
fn refresh_modifier_state_from_keyboard() -> u32 {
    let mut mods = 0u32;

    if is_virtual_key_down(VK_LMENU.0 as i32)
        || is_virtual_key_down(VK_RMENU.0 as i32)
        || is_virtual_key_down(VK_MENU.0 as i32)
    {
        mods |= MOD_ALT;
    }

    if is_virtual_key_down(VK_LCONTROL.0 as i32)
        || is_virtual_key_down(VK_RCONTROL.0 as i32)
        || is_virtual_key_down(VK_CONTROL.0 as i32)
    {
        mods |= MOD_CTRL;
    }

    if is_virtual_key_down(VK_LSHIFT.0 as i32)
        || is_virtual_key_down(VK_RSHIFT.0 as i32)
        || is_virtual_key_down(VK_SHIFT.0 as i32)
    {
        mods |= MOD_SHIFT;
    }

    if is_virtual_key_down(VK_LWIN.0 as i32) || is_virtual_key_down(VK_RWIN.0 as i32) {
        mods |= MOD_WIN;
    }

    MODIFIER_STATE.store(mods, Ordering::Release);
    mods
}

/// Sync the pre-computed modifier masks from the current config so the
/// hook thread can make swallow decisions without touching the config mutex.
fn update_modifier_masks(config: &AppConfig) {
    let move_m = modifier_to_mask(config.move_modifier);
    let resize_m =
        modifier_to_mask(config.resize_modifier_1) | modifier_to_mask(config.resize_modifier_2);
    MOVE_MASK.store(move_m, Ordering::Release);
    RESIZE_MASK.store(resize_m, Ordering::Release);
    log::debug!(
        "modifier masks updated: move={:#x} resize={:#x}",
        move_m,
        resize_m
    );
}

fn process_allowed(config: &AppConfig, process_name: &str) -> bool {
    let process_name = process_name.to_ascii_lowercase();
    let listed = config
        .filter_list
        .iter()
        .map(|entry| entry.trim().to_ascii_lowercase())
        .any(|entry| entry == process_name);

    match config.filter_mode {
        FilterMode::Whitelist => listed,
        FilterMode::Blacklist => !listed,
    }
}

/// Inclusive (subset) match: all bits of the required mask must be present,
/// but extra modifier bits are tolerated.  This prevents transient modifier
/// flicker (e.g. a brief Ctrl press while Alt is held) from tearing down
/// an active grab.
fn determine_mode(mods: u32, config: &AppConfig) -> Option<DragMode> {
    // Check resize first — it requires strictly more modifiers than move.
    let resize_mask =
        modifier_to_mask(config.resize_modifier_1) | modifier_to_mask(config.resize_modifier_2);
    if resize_mask != 0 && (mods & resize_mask) == resize_mask {
        return Some(DragMode::Resize);
    }

    let move_mask = modifier_to_mask(config.move_modifier);
    if move_mask != 0 && (mods & move_mask) == move_mask {
        return Some(DragMode::Move);
    }

    None
}

fn determine_resize_direction(cursor: POINT, rect: RECT) -> ResizeDirection {
    let center_x = rect.left + (rect.right - rect.left) / 2;
    let center_y = rect.top + (rect.bottom - rect.top) / 2;

    match (cursor.x >= center_x, cursor.y >= center_y) {
        (false, false) => ResizeDirection::TopLeft,
        (true, false) => ResizeDirection::TopRight,
        (false, true) => ResizeDirection::BottomLeft,
        (true, true) => ResizeDirection::BottomRight,
    }
}

fn clamp_rect_for_min_size(rect: &mut RECT, dir: ResizeDirection) {
    if rect.right - rect.left < MIN_WINDOW_SIZE {
        match dir {
            ResizeDirection::TopLeft | ResizeDirection::BottomLeft => {
                rect.left = rect.right - MIN_WINDOW_SIZE;
            }
            ResizeDirection::TopRight | ResizeDirection::BottomRight => {
                rect.right = rect.left + MIN_WINDOW_SIZE;
            }
        }
    }

    if rect.bottom - rect.top < MIN_WINDOW_SIZE {
        match dir {
            ResizeDirection::TopLeft | ResizeDirection::TopRight => {
                rect.top = rect.bottom - MIN_WINDOW_SIZE;
            }
            ResizeDirection::BottomLeft | ResizeDirection::BottomRight => {
                rect.bottom = rect.top + MIN_WINDOW_SIZE;
            }
        }
    }
}

fn try_create_grab_state(
    cursor_pos: POINT,
    mode: DragMode,
    config: &AppConfig,
) -> Option<GrabState> {
    let hwnd = window_manager::window_from_point(cursor_pos.x, cursor_pos.y)?;

    if !window_manager::is_valid_target(hwnd) {
        return None;
    }

    let process_name = window_manager::get_process_name(hwnd)?;
    if !process_allowed(config, &process_name) {
        log::debug!("process filtered: {}", process_name);
        return None;
    }

    // Restore snapped or maximized windows before grabbing.
    if window_manager::is_maximized(hwnd) || window_manager::is_snapped(hwnd) {
        window_manager::restore_window(hwnd);
        // Brief sleep to let DWM finish the restore animation.
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    // Always capture the origin rect — used as the authoritative baseline
    // for position computation during the entire grab lifetime.
    let origin_rect = window_manager::get_window_rect(hwnd)?;

    let resize_dir = if matches!(mode, DragMode::Resize) {
        determine_resize_direction(cursor_pos, origin_rect)
    } else {
        ResizeDirection::BottomRight
    };

    Some(GrabState {
        mode,
        hwnd,
        last_cursor: cursor_pos,
        origin_rect,
        cumulative_dx: 0,
        cumulative_dy: 0,
        resize_dir,
    })
}

fn set_active_grab(active: bool) {
    ACTIVE_GRAB.store(active, Ordering::Relaxed);
}

/// Process a mouse-move event on the worker thread.
/// `mods` is the modifier snapshot captured on the hook thread — no re-polling.
///
/// Position is computed as `origin_rect + cumulative_delta` rather than
/// reading `GetWindowRect` every tick.  This makes us authoritative over
/// the window position and immune to external actors (Snap, app WndProc)
/// resetting it between our ticks.
fn worker_handle_mouse_move(point: POINT, mods: u32, state: &mut Option<GrabState>) {
    if !HOOK_ENABLED.load(Ordering::Relaxed) {
        *state = None;
        set_active_grab(false);
        return;
    }

    let Some(config) = current_config() else {
        *state = None;
        set_active_grab(false);
        return;
    };

    if !config.enabled {
        *state = None;
        set_active_grab(false);
        return;
    }

    let Some(desired_mode) = determine_mode(mods, &config) else {
        if state.is_some() {
            log::debug!("grab released: mods={:#x}", mods);
        }
        *state = None;
        set_active_grab(false);
        return;
    };

    if state.is_none() {
        *state = try_create_grab_state(point, desired_mode, &config);
        if state.is_some() {
            log::debug!("grab started: mode={:?} mods={:#x}", desired_mode, mods);
        }
    }

    let Some(grab) = state.as_mut() else {
        set_active_grab(false);
        return;
    };

    if grab.mode != desired_mode {
        log::debug!("mode switch: {:?} → {:?}", grab.mode, desired_mode);
        // On mode switch, resync origin from the actual window position
        // and reset cumulative deltas so the new mode starts cleanly.
        if let Some(rect) = window_manager::get_window_rect(grab.hwnd) {
            grab.origin_rect = rect;
            grab.cumulative_dx = 0;
            grab.cumulative_dy = 0;
        }
        grab.mode = desired_mode;
        grab.last_cursor = point;
        if matches!(desired_mode, DragMode::Resize) {
            grab.resize_dir = determine_resize_direction(point, grab.origin_rect);
        }
    }

    let dx = point.x - grab.last_cursor.x;
    let dy = point.y - grab.last_cursor.y;
    if dx == 0 && dy == 0 {
        set_active_grab(true);
        return;
    }

    // Accumulate cursor delta since grab start.
    grab.cumulative_dx += dx;
    grab.cumulative_dy += dy;

    match grab.mode {
        DragMode::Move => {
            window_manager::move_window(
                grab.hwnd,
                grab.origin_rect.left + grab.cumulative_dx,
                grab.origin_rect.top + grab.cumulative_dy,
            );
        }
        DragMode::Resize => {
            let mut r = grab.origin_rect;
            match grab.resize_dir {
                ResizeDirection::TopLeft => {
                    r.left += grab.cumulative_dx;
                    r.top += grab.cumulative_dy;
                }
                ResizeDirection::TopRight => {
                    r.right += grab.cumulative_dx;
                    r.top += grab.cumulative_dy;
                }
                ResizeDirection::BottomLeft => {
                    r.left += grab.cumulative_dx;
                    r.bottom += grab.cumulative_dy;
                }
                ResizeDirection::BottomRight => {
                    r.right += grab.cumulative_dx;
                    r.bottom += grab.cumulative_dy;
                }
            }
            clamp_rect_for_min_size(&mut r, grab.resize_dir);
            window_manager::resize_window(
                grab.hwnd,
                r.left,
                r.top,
                r.right - r.left,
                r.bottom - r.top,
            );
        }
    }

    grab.last_cursor = point;
    set_active_grab(true);
}

fn worker_loop(rx: Receiver<WorkerEvent>) {
    let mut state: Option<GrabState> = None;
    while let Ok(event) = rx.recv() {
        // Drain the channel: if multiple mouse-move events queued up while
        // we were busy with SetWindowPos, skip to the latest one so the
        // window tracks the *current* cursor position, not a stale one.
        let latest = drain_to_latest(event, &rx);

        match latest {
            WorkerEvent::MouseMove { point, mods } => {
                worker_handle_mouse_move(point, mods, &mut state);
            }
        }
    }
    log::info!("worker loop exited");
}

/// Consume all immediately-available events from the channel and return
/// the last one.  This ensures the worker always acts on the freshest
/// cursor position rather than processing a backlog of stale coordinates.
#[inline]
fn drain_to_latest(first: WorkerEvent, rx: &Receiver<WorkerEvent>) -> WorkerEvent {
    let mut latest = first;
    while let Ok(ev) = rx.try_recv() {
        latest = ev;
    }
    latest
}

/// Keyboard hook — event-driven modifier tracking.
/// Uses `w_param` (WM_KEYDOWN / WM_SYSKEYDOWN / WM_KEYUP / WM_SYSKEYUP) directly
/// instead of polling `GetAsyncKeyState` on every event.
unsafe extern "system" fn keyboard_hook_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        let kb = &*(l_param.0 as *const KBDLLHOOKSTRUCT);
        if let Some(mask) = key_to_mask(kb.vkCode) {
            let msg = w_param.0 as u32;
            if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                MODIFIER_STATE.fetch_or(mask, Ordering::Release);
            } else {
                MODIFIER_STATE.fetch_and(!mask, Ordering::Release);
            }
            log::debug!(
                "key vk={:#x} {} → state={:#x}",
                kb.vkCode,
                if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                    "DOWN"
                } else {
                    "UP"
                },
                MODIFIER_STATE.load(Ordering::Relaxed)
            );
        }
    }

    unsafe { CallNextHookEx(None, n_code, w_param, l_param) }
}

/// Mouse hook — reads modifier snapshot and sends it with the event.
///
/// AltSnap pattern: **always pass WM_MOUSEMOVE through** (CallNextHookEx).
/// Swallowing causes OS mouse-tracking and DWM to lose context,
/// which can trigger snap-back or jitter.
unsafe extern "system" fn mouse_hook_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code < 0 {
        return unsafe { CallNextHookEx(None, n_code, w_param, l_param) };
    }
    if w_param.0 as u32 != WM_MOUSEMOVE {
        return unsafe { CallNextHookEx(None, n_code, w_param, l_param) };
    }
    if !HOOK_ENABLED.load(Ordering::Relaxed) {
        set_active_grab(false);
        return unsafe { CallNextHookEx(None, n_code, w_param, l_param) };
    }
    let mouse = &*(l_param.0 as *const MSLLHOOKSTRUCT);
    let mods = MODIFIER_STATE.load(Ordering::Acquire);
    if let Some(tx) = WORKER_TX.get() {
        let _ = tx.try_send(WorkerEvent::MouseMove {
            point: mouse.pt,
            mods,
        });
    }

    // Always pass through — never swallow WM_MOUSEMOVE.
    // AltSnap does the same: the hook dispatches work to a worker thread
    // but lets the mouse event propagate normally through the OS.
    unsafe { CallNextHookEx(None, n_code, w_param, l_param) }
}

fn hook_thread_main(config: Arc<Mutex<AppConfig>>) {
    if let Some(shared) = SHARED_CONFIG.get() {
        if !Arc::ptr_eq(shared, &config) {
            let cfg = config.lock().clone();
            *shared.lock() = cfg;
        }
    } else {
        let _ = SHARED_CONFIG.set(config.clone());
    }

    // Sync modifier masks from config so the hook thread can swallow correctly.
    update_modifier_masks(&config.lock());

    // Sync modifier state from physical keyboard before hooks are active
    let initial = refresh_modifier_state_from_keyboard();
    log::debug!("initial modifier state: {:#x}", initial);

    let (worker_tx, worker_rx) = mpsc::sync_channel::<WorkerEvent>(WORKER_QUEUE_SIZE);
    let _ = WORKER_TX.set(worker_tx);
    thread::spawn(move || worker_loop(worker_rx));

    let keyboard_hook =
        unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), None, 0) };
    let mouse_hook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), None, 0) };

    let (keyboard_hook, mouse_hook) = match (keyboard_hook, mouse_hook) {
        (Ok(kh), Ok(mh)) => {
            log::info!("hooks installed: keyboard + mouse");
            (kh, mh)
        }
        (kh, mh) => {
            log::error!(
                "hook installation failed — keyboard={} mouse={}",
                if kh.is_ok() { "ok" } else { "FAILED" },
                if mh.is_ok() { "ok" } else { "FAILED" }
            );
            if let Ok(hook) = kh {
                let _ = unsafe { UnhookWindowsHookEx(hook) };
            }
            if let Ok(hook) = mh {
                let _ = unsafe { UnhookWindowsHookEx(hook) };
            }
            return;
        }
    };

    let mut msg = MSG::default();
    loop {
        let status = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if status.0 <= 0 {
            break;
        }

        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    log::info!("hook thread shutting down");
    let _ = unsafe { UnhookWindowsHookEx(keyboard_hook) };
    let _ = unsafe { UnhookWindowsHookEx(mouse_hook) };
}

pub fn start_hook_thread(config: Arc<Mutex<AppConfig>>) -> u32 {
    if let Some(shared) = SHARED_CONFIG.get() {
        if !Arc::ptr_eq(shared, &config) {
            let cfg = config.lock().clone();
            *shared.lock() = cfg;
        }
    } else {
        let _ = SHARED_CONFIG.set(config.clone());
    }

    // Pre-compute masks so the hook thread has them even before hook_thread_main runs.
    update_modifier_masks(&config.lock());

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let thread_id = unsafe { GetCurrentThreadId() };
        let _ = tx.send(thread_id);
        hook_thread_main(config);
    });

    let thread_id = rx.recv().unwrap_or(0);
    log::info!("hook thread spawned: tid={}", thread_id);
    thread_id
}

#[allow(dead_code)]
pub fn stop_hook_thread(thread_id: u32) {
    if thread_id == 0 {
        return;
    }

    let _ = unsafe { PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
}

pub fn set_enabled(enabled: bool) {
    log::info!("hook enabled={}", enabled);
    HOOK_ENABLED.store(enabled, Ordering::Release);
    if !enabled {
        set_active_grab(false);
    }
}

pub fn update_config(config: Arc<Mutex<AppConfig>>) {
    let cfg = config.lock().clone();

    // Update modifier masks so the hook thread picks up new keybindings.
    update_modifier_masks(&cfg);

    if let Some(shared) = SHARED_CONFIG.get() {
        if !Arc::ptr_eq(shared, &config) {
            *shared.lock() = cfg;
        }
    } else {
        let _ = SHARED_CONFIG.set(config);
    }
}
