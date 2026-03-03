use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, OnceLock};
use std::thread;

use parking_lot::Mutex;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL, VK_DOWN, VK_LCONTROL, VK_LEFT, VK_LMENU, VK_LSHIFT,
    VK_LWIN, VK_MENU, VK_RCONTROL, VK_RIGHT, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SHIFT, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, WH_KEYBOARD_LL,
    WH_MOUSE_LL, WM_KEYDOWN, WM_MOUSEMOVE, WM_QUIT, WM_SYSKEYDOWN,
};

use crate::config::{AppConfig, FilterMode, ModifierKey};
use crate::overlay;
use crate::snap;
use crate::window_manager;

const MOD_ALT: u32 = 1;
const MOD_CTRL: u32 = 2;
const MOD_SHIFT: u32 = 4;
const MOD_WIN: u32 = 8;
const MIN_WINDOW_SIZE: i32 = 100;
const WORKER_QUEUE_SIZE: usize = 1024;

/// Marker stored in `dwExtraInfo` of every INPUT we synthesise via SendInput.
/// The keyboard hook uses this to skip our own injected Win-key events so they
/// do not pollute `MODIFIER_STATE` and accidentally start a new grab.
const GLIDE_SYNTHETIC_EXTRA_INFO: usize = 0x474C_4944; // b'G','L','I','D'
/// Mouse message constants not in the windows crate import set.
const WM_MOUSEWHEEL: u32 = 0x020A;
const WM_MBUTTONDOWN: u32 = 0x0207;

/// Opacity change per scroll tick (out of 255).
const OPACITY_STEP: i32 = 15;
/// Minimum opacity — still slightly visible.
const OPACITY_MIN: u8 = 20;

static MODIFIER_STATE: AtomicU32 = AtomicU32::new(0);
static HOOK_ENABLED: AtomicBool = AtomicBool::new(true);
static ACTIVE_GRAB: AtomicBool = AtomicBool::new(false);
static SHARED_CONFIG: OnceLock<Arc<Mutex<AppConfig>>> = OnceLock::new();
static WORKER_TX: OnceLock<SyncSender<WorkerEvent>> = OnceLock::new();
/// Thread ID of the hook thread — used by `shutdown()` to post WM_QUIT for graceful teardown.
static HOOK_THREAD_ID: AtomicU32 = AtomicU32::new(0);

/// Pre-computed modifier masks for the hook thread to decide swallowing
/// synchronously, without waiting for the worker.
static MOVE_MASK: AtomicU32 = AtomicU32::new(MOD_ALT);
static RESIZE_MASK: AtomicU32 = AtomicU32::new(MOD_ALT | MOD_SHIFT);

/// Feature flags — readable from the hook thread without touching the config mutex.
static SCROLL_OPACITY_ACTIVE: AtomicBool = AtomicBool::new(true);
static MIDDLECLICK_TOPMOST_ACTIVE: AtomicBool = AtomicBool::new(true);
static SCROLL_OPACITY_MASK: AtomicU32 = AtomicU32::new(MOD_ALT);

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
    /// Snap target rect — set when cursor is in a snap zone during Move.
    /// On grab end, the window snaps to this rect.
    snap_target: Option<(snap::SnapZone, RECT)>,
    /// Cursor position at grab creation; used for the dead-zone threshold check.
    start_cursor: POINT,
    /// True once the cursor has moved ≥ `config.drag_threshold` px from `start_cursor`.
    /// Until committed, the window is not moved or resized.
    committed: bool,
}

/// Worker event carrying the modifier snapshot from the hook thread.
#[derive(Clone, Copy)]
enum WorkerEvent {
    MouseMove {
        point: POINT,
        mods: u32,
    },
    MouseWheel {
        point: POINT,
        delta: i16,
        mods: u32,
    },
    MiddleClick {
        point: POINT,
        mods: u32,
    },
    /// Sent by the hook thread to signal the worker to exit cleanly.
    Shutdown,
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

/// Sync pre-computed config values so the hook thread can make decisions
/// without touching the config mutex.
fn update_hook_state(config: &AppConfig) {
    let move_m = modifier_to_mask(config.move_modifier);
    let resize_m =
        modifier_to_mask(config.resize_modifier_1) | modifier_to_mask(config.resize_modifier_2);
    MOVE_MASK.store(move_m, Ordering::Release);
    RESIZE_MASK.store(resize_m, Ordering::Release);
    SCROLL_OPACITY_ACTIVE.store(config.scroll_opacity, Ordering::Release);
    SCROLL_OPACITY_MASK.store(
        modifier_to_mask(config.scroll_opacity_modifier),
        Ordering::Release,
    );
    MIDDLECLICK_TOPMOST_ACTIVE.store(config.middleclick_topmost, Ordering::Release);
    log::debug!(
        "hook state updated: move={:#x} resize={:#x} scroll_opacity={} middleclick_topmost={}",
        move_m,
        resize_m,
        config.scroll_opacity,
        config.middleclick_topmost,
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

    // Foreground-only mode: skip if the target window is not foreground.
    if !config.allow_nonforeground {
        let fg = window_manager::get_foreground_window();
        if fg != Some(hwnd) {
            return None;
        }
    }

    let process_name = window_manager::get_process_name(hwnd)?;
    if !process_allowed(config, &process_name) {
        log::debug!("process filtered: {}", process_name);
        return None;
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
        snap_target: None,
        start_cursor: cursor_pos,
        committed: false,
    })
}

/// Apply all side-effectful operations that must happen exactly once, at the
/// moment the dead-zone threshold is crossed.  Separated from
/// `try_create_grab_state` so that snapped/maximised windows are only
/// restored — and raise_on_grab only fires — when the user has
/// demonstrated clear drag intent (≥ drag_threshold pixels of movement).
fn commit_grab(grab: &mut GrabState, config: &AppConfig, point: POINT) {
    // Restore snapped or maximized windows before the first real move.
    if window_manager::is_maximized(grab.hwnd) || window_manager::is_snapped(grab.hwnd) {
        window_manager::restore_window(grab.hwnd);
        // Brief sleep to let DWM finish the restore animation.
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    // Raise the window to the top of Z-order if configured.
    // Uses SetWindowPos(HWND_TOP) instead of SetForegroundWindow to avoid
    // unintentional WS_EX_TOPMOST side-effects during drag activation.
    if config.raise_on_grab {
        window_manager::raise_to_top(grab.hwnd);
    }

    // Re-capture origin_rect after the restore — the window rect will have
    // changed from its snapped/maximised geometry to its restored geometry.
    // Without this, cumulative deltas would be anchored to the wrong rect.
    if let Some(rect) = window_manager::get_window_rect(grab.hwnd) {
        grab.origin_rect = rect;
    }

    // Recompute resize direction against the post-restore rect.
    if matches!(grab.mode, DragMode::Resize) {
        grab.resize_dir = determine_resize_direction(point, grab.origin_rect);
    }

    grab.committed = true;
    grab.last_cursor = point;
}

fn set_active_grab(active: bool) {
    ACTIVE_GRAB.store(active, Ordering::Relaxed);
}

/// Check if the modifier key(s) for "move" action are currently held.
fn is_move_modifier_held(mods: u32) -> bool {
    let move_mask = MOVE_MASK.load(Ordering::Acquire);
    move_mask != 0 && (mods & move_mask) == move_mask
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
        if state.is_some() {
            overlay::hide();
        }
        *state = None;
        set_active_grab(false);
        return;
    }

    let Some(config) = current_config() else {
        if state.is_some() {
            overlay::hide();
        }
        *state = None;
        set_active_grab(false);
        return;
    };

    if !config.enabled {
        if state.is_some() {
            overlay::hide();
        }
        *state = None;
        set_active_grab(false);
        return;
    }

    let Some(desired_mode) = determine_mode(mods, &config) else {
        // Grab ending — check for snap before clearing state.
        if let Some(old_grab) = state.take() {
            log::debug!("grab released: mods={:#x}", mods);
            // Hide the overlay immediately — don't wait for the snap to complete.
            overlay::hide();
            if let Some((zone, rect)) = old_grab.snap_target {
                if zone == snap::SnapZone::Maximize {
                    // Maximise via SW_MAXIMIZE so the window enters the DWM-tracked
                    // maximised state (taskbar peek, restore-on-drag, etc.).
                    window_manager::maximize_window(old_grab.hwnd);
                    log::debug!("snapped: Maximize → SW_MAXIMIZE");
                } else if config.snap_native {
                    // Trigger native Win+Arrow snap so the window is registered in the
                    // Win11 snap group — this enables the centre resize divider.
                    apply_snap_native(old_grab.hwnd, zone);
                    log::debug!("snapped to zone: {:?} (native)", zone);
                } else {
                    // Fallback: position the window directly via SetWindowPos.
                    window_manager::resize_window(
                        old_grab.hwnd,
                        rect.left,
                        rect.top,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                    );
                    log::debug!("snapped to zone: {:?} (SetWindowPos)", zone);
                }
            }
        }
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
        grab.snap_target = None;
        // If the grab has not committed yet, reset the dead-zone origin to the
        // mode-switch position so the threshold is re-evaluated from here.
        if !grab.committed {
            grab.start_cursor = point;
        }
        overlay::hide();
        if matches!(desired_mode, DragMode::Resize) {
            grab.resize_dir = determine_resize_direction(point, grab.origin_rect);
        }
    }

    // Dead-zone: require the cursor to move ≥ drag_threshold pixels (Euclidean)
    // from the grab-start position before actually moving or resizing the window.
    // This prevents accidental operations from minute cursor tremor while a
    // modifier key is being pressed or released.
    if !grab.committed {
        let ddx = point.x - grab.start_cursor.x;
        let ddy = point.y - grab.start_cursor.y;
        let thr = config.drag_threshold;
        if ddx * ddx + ddy * ddy < thr * thr {
            // Still inside dead-zone — do not move the window.
            set_active_grab(false);
            return;
        }
        // Threshold crossed — commit the grab.  Re-anchor last_cursor to the
        // current point so subsequent cumulative deltas start cleanly from here,
        // avoiding any position jump on the first committed frame.
        // Threshold crossed — commit the grab (restore/raise if needed, re-anchor).
        commit_grab(grab, &config, point);
        set_active_grab(true);
        return;
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

            // Edge snap detection during move.
            if config.snap_enabled {
                if let Some((zone, zone_rect)) =
                    snap::detect_snap_zone(point, config.snap_threshold)
                {
                    overlay::show(zone_rect);
                    grab.snap_target = Some((zone, zone_rect));
                } else {
                    if grab.snap_target.is_some() {
                        overlay::hide();
                    }
                    grab.snap_target = None;
                }
            }
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

/// Handle scroll wheel — modifier + scroll changes window opacity.
fn worker_handle_scroll(point: POINT, delta: i16, mods: u32) {
    let Some(config) = current_config() else {
        return;
    };
    if !config.enabled || !config.scroll_opacity {
        return;
    }

    // Only act when the scroll opacity modifier is held.
    let opacity_mask = SCROLL_OPACITY_MASK.load(Ordering::Acquire);
    if opacity_mask == 0 || (mods & opacity_mask) != opacity_mask {
        return;
    }

    let Some(hwnd) = window_manager::window_from_point(point.x, point.y) else {
        return;
    };
    if !window_manager::is_valid_target(hwnd) {
        return;
    }

    let current = window_manager::get_window_opacity(hwnd) as i32;
    let step = if delta > 0 {
        OPACITY_STEP
    } else {
        -OPACITY_STEP
    };
    let new_alpha = (current + step).clamp(OPACITY_MIN as i32, 255) as u8;

    window_manager::set_window_opacity(hwnd, new_alpha);
    log::debug!("opacity: {} → {} (delta={})", current, new_alpha, delta);
}

/// Handle middle-click — modifier + middle-click toggles always-on-top.
fn worker_handle_middleclick(point: POINT, mods: u32) {
    let Some(config) = current_config() else {
        return;
    };
    if !config.enabled || !config.middleclick_topmost {
        return;
    }

    if !is_move_modifier_held(mods) {
        return;
    }

    let Some(hwnd) = window_manager::window_from_point(point.x, point.y) else {
        return;
    };
    if !window_manager::is_valid_target(hwnd) {
        return;
    }

    let new_state = window_manager::toggle_topmost(hwnd);
    log::debug!("topmost toggled: {}", new_state);
}

fn worker_loop(rx: Receiver<WorkerEvent>) {
    let mut state: Option<GrabState> = None;
    // A non-MouseMove event encountered while draining mouse-move events.
    // Stored here so it is processed on the next iteration instead of dropped.
    let mut pending: Option<WorkerEvent> = None;
    loop {
        let event = if let Some(e) = pending.take() {
            e
        } else {
            match rx.recv() {
                Ok(e) => e,
                Err(_) => break,
            }
        };
        match event {
            WorkerEvent::Shutdown => break,
            WorkerEvent::MouseMove { .. } => {
                // Drain to latest mouse-move to skip stale coordinates.
                let (latest, pushed_back) = drain_to_latest_mouse_move(event, &rx);
                pending = pushed_back;
                if let WorkerEvent::MouseMove { point, mods } = latest {
                    worker_handle_mouse_move(point, mods, &mut state);
                }
            }
            WorkerEvent::MouseWheel { point, delta, mods } => {
                worker_handle_scroll(point, delta, mods);
            }
            WorkerEvent::MiddleClick { point, mods } => {
                worker_handle_middleclick(point, mods);
            }
        }
    }
    log::info!("worker loop exited");
}

/// Consume all immediately-available MouseMove events from the channel and return
/// the last one.  This ensures the worker always acts on the freshest
/// cursor position rather than processing a backlog of stale coordinates.
///
/// Returns `(latest_move, Option<non_move_event>)`. If a non-MouseMove event is
/// encountered while draining, it is returned as the second element so the caller
/// can process it on the next iteration — previously it would have been silently dropped.
#[inline]
fn drain_to_latest_mouse_move(
    first: WorkerEvent,
    rx: &Receiver<WorkerEvent>,
) -> (WorkerEvent, Option<WorkerEvent>) {
    let mut latest = first;
    while let Ok(ev) = rx.try_recv() {
        match ev {
            WorkerEvent::MouseMove { .. } => {
                latest = ev;
            }
            // Non-move event (Shutdown, MouseWheel, MiddleClick): stop draining
            // and hand it back to the caller so it is not silently dropped.
            other => return (latest, Some(other)),
        }
    }
    (latest, None)
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
        // Skip events injected by this process (our own SendInput calls for native snap).
        // Using a unique dwExtraInfo marker is more precise than LLKHF_INJECTED, which
        // would also suppress legitimate synthetic input from third-party tools.
        if kb.dwExtraInfo != GLIDE_SYNTHETIC_EXTRA_INFO {
            if let Some(mask) = key_to_mask(kb.vkCode) {
                let msg = w_param.0 as u32;
                if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                    MODIFIER_STATE.fetch_or(mask, Ordering::Release);
                } else {
                    MODIFIER_STATE.fetch_and(!mask, Ordering::Release);
                }
            }
        }
    }

    unsafe { CallNextHookEx(None, n_code, w_param, l_param) }
}

/// Apply a snap zone by simulating the native Win+Arrow keyboard shortcut.
///
/// Calling `SendInput(Win+Left/Right)` instead of `SetWindowPos` registers the
/// window in the Win11 **snap group**, which enables:
/// - The centre resize divider between two snapped windows
/// - Snap Assist (filling the adjacent zone after snap)
/// - `WINDOWPLACEMENT.rcNormalPosition` preservation (restore-on-drag works)
///
/// The Win key is held for the full sequence so Windows treats `Left`+`Up` as a
/// single quarter-snap gesture and skips the Snap Assist prompt between them.
fn apply_snap_native(hwnd: HWND, zone: snap::SnapZone) {
    // Bring the target window to foreground first.
    // SendInput targets the current foreground window — there is no per-HWND API.
    // SetForegroundWindow can be denied by the OS foreground-lock when called from a
    // background thread; log a warning so the snap silently degrades rather than failing hard.
    if !window_manager::set_foreground(hwnd) {
        log::warn!(
            "apply_snap_native: SetForegroundWindow denied — Win+Arrow may target wrong window"
        );
    }
    let h_vk: VIRTUAL_KEY = match zone {
        snap::SnapZone::Left | snap::SnapZone::TopLeft | snap::SnapZone::BottomLeft => VK_LEFT,
        snap::SnapZone::Right | snap::SnapZone::TopRight | snap::SnapZone::BottomRight => VK_RIGHT,
        snap::SnapZone::Maximize => return, // handled separately via SW_MAXIMIZE
    };

    let v_vk: Option<VIRTUAL_KEY> = match zone {
        snap::SnapZone::TopLeft | snap::SnapZone::TopRight => Some(VK_UP),
        snap::SnapZone::BottomLeft | snap::SnapZone::BottomRight => Some(VK_DOWN),
        _ => None,
    };

    // Helper: create a keyboard INPUT with our synthetic marker.
    let ki = |vk: VIRTUAL_KEY, up: bool| -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: if up {
                        KEYEVENTF_KEYUP
                    } else {
                        KEYBD_EVENT_FLAGS(0)
                    },
                    time: 0,
                    // Mark as ours so keyboard_hook_proc skips it.
                    dwExtraInfo: GLIDE_SYNTHETIC_EXTRA_INFO,
                },
            },
        }
    };

    // Build the sequence.  Win is held throughout so Windows processes the
    // horizontal + vertical arrows as one atomic gesture (quarter snap without
    // triggering the Snap Assist prompt in between).
    let mut inputs: Vec<INPUT> = Vec::with_capacity(6);
    inputs.push(ki(VK_LWIN, false)); // Win ↓
    inputs.push(ki(h_vk, false)); // H ↓
    inputs.push(ki(h_vk, true)); // H ↑
    if let Some(vk) = v_vk {
        inputs.push(ki(vk, false)); // V ↓
        inputs.push(ki(vk, true)); // V ↑
    }
    inputs.push(ki(VK_LWIN, true)); // Win ↑

    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Mouse hook — reads modifier snapshot and sends it with the event.
///
/// AltSnap pattern: **always pass WM_MOUSEMOVE through** (CallNextHookEx).
/// Swallowing causes OS mouse-tracking and DWM to lose context,
/// which can trigger snap-back or jitter.
///
/// WM_MOUSEWHEEL and WM_MBUTTONDOWN are **swallowed** when modifier is held
/// and the corresponding feature is enabled — this prevents the underlying
/// app from also receiving the event.
unsafe extern "system" fn mouse_hook_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code < 0 {
        return unsafe { CallNextHookEx(None, n_code, w_param, l_param) };
    }
    if !HOOK_ENABLED.load(Ordering::Relaxed) {
        set_active_grab(false);
        return unsafe { CallNextHookEx(None, n_code, w_param, l_param) };
    }

    let msg = w_param.0 as u32;

    match msg {
        WM_MOUSEMOVE => {
            let mouse = unsafe { &*(l_param.0 as *const MSLLHOOKSTRUCT) };
            let mods = MODIFIER_STATE.load(Ordering::Acquire);
            if let Some(tx) = WORKER_TX.get() {
                let _ = tx.try_send(WorkerEvent::MouseMove {
                    point: mouse.pt,
                    mods,
                });
            }
            // Always pass through — never swallow WM_MOUSEMOVE.
            unsafe { CallNextHookEx(None, n_code, w_param, l_param) }
        }

        WM_MOUSEWHEEL => {
            let mods = MODIFIER_STATE.load(Ordering::Acquire);
            let opacity_mask = SCROLL_OPACITY_MASK.load(Ordering::Acquire);
            let feature_active = SCROLL_OPACITY_ACTIVE.load(Ordering::Relaxed);

            if feature_active && opacity_mask != 0 && (mods & opacity_mask) == opacity_mask {
                // Modifier held + feature on → swallow and send to worker.
                let mouse = unsafe { &*(l_param.0 as *const MSLLHOOKSTRUCT) };
                let delta = (mouse.mouseData >> 16) as i16;
                if let Some(tx) = WORKER_TX.get() {
                    let _ = tx.try_send(WorkerEvent::MouseWheel {
                        point: mouse.pt,
                        delta,
                        mods,
                    });
                }
                LRESULT(1) // Swallow
            } else {
                unsafe { CallNextHookEx(None, n_code, w_param, l_param) }
            }
        }

        WM_MBUTTONDOWN => {
            let mods = MODIFIER_STATE.load(Ordering::Acquire);
            let move_mask = MOVE_MASK.load(Ordering::Acquire);
            let feature_active = MIDDLECLICK_TOPMOST_ACTIVE.load(Ordering::Relaxed);

            if feature_active && move_mask != 0 && (mods & move_mask) == move_mask {
                let mouse = unsafe { &*(l_param.0 as *const MSLLHOOKSTRUCT) };
                if let Some(tx) = WORKER_TX.get() {
                    let _ = tx.try_send(WorkerEvent::MiddleClick {
                        point: mouse.pt,
                        mods,
                    });
                }
                LRESULT(1) // Swallow
            } else {
                unsafe { CallNextHookEx(None, n_code, w_param, l_param) }
            }
        }

        _ => unsafe { CallNextHookEx(None, n_code, w_param, l_param) },
    }
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

    // Sync hook state from config.
    update_hook_state(&config.lock());

    // Sync modifier state from physical keyboard before hooks are active
    let initial = refresh_modifier_state_from_keyboard();
    log::debug!("initial modifier state: {:#x}", initial);

    // Create the snap overlay window on this thread (needs the message loop).
    overlay::create();

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
            overlay::destroy();
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
    // Signal the worker thread to exit gracefully before unhooking.
    // This prevents the worker from blocking indefinitely on rx.recv().
    if let Some(tx) = WORKER_TX.get() {
        let _ = tx.send(WorkerEvent::Shutdown);
    }
    let _ = unsafe { UnhookWindowsHookEx(keyboard_hook) };
    let _ = unsafe { UnhookWindowsHookEx(mouse_hook) };
    overlay::destroy();
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

    // Pre-compute state so the hook thread has them even before hook_thread_main runs.
    update_hook_state(&config.lock());

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let thread_id = unsafe { GetCurrentThreadId() };
        let _ = tx.send(thread_id);
        hook_thread_main(config);
    });

    let thread_id = match rx.recv() {
        Ok(tid) if tid != 0 => tid,
        Ok(_) | Err(_) => {
            log::error!("hook thread failed to start — hooks will not function");
            0
        }
    };
    HOOK_THREAD_ID.store(thread_id, Ordering::Release);
    if thread_id != 0 {
        log::info!("hook thread spawned: tid={}", thread_id);
    }
    thread_id
}

/// Signal the hook thread to shut down gracefully by posting `WM_QUIT` to its message loop.
/// Call this before `app.exit()` so the hook thread has a chance to:
///   - send `WorkerEvent::Shutdown` to the worker thread
///   - call `UnhookWindowsHookEx` on both hooks
///   - call `overlay::destroy()`
pub fn shutdown() {
    let tid = HOOK_THREAD_ID.load(Ordering::Acquire);
    if tid != 0 {
        log::info!("shutdown: posting WM_QUIT to hook thread tid={}", tid);
        unsafe {
            let _ = PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0));
        }
    }
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

    // Update hook state so the hook thread picks up new keybindings and feature flags.
    update_hook_state(&cfg);

    // SHARED_CONFIG is a OnceLock — it can only be set once (the first call to
    // start_hook_thread or update_config). After that, the OnceLock holds the
    // canonical Arc. If the caller passes a *different* Arc, we copy the inner
    // config value into the shared Arc rather than trying to replace the lock.
    // This keeps the hook thread's SHARED_CONFIG reference stable while still
    // picking up the new settings.
    if let Some(shared) = SHARED_CONFIG.get() {
        if !Arc::ptr_eq(shared, &config) {
            // Different Arc — copy the value into the existing shared one.
            *shared.lock() = cfg;
        }
        // Same Arc — hook thread already sees the updated value (shared ref).
    } else {
        let _ = SHARED_CONFIG.set(config);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Foundation::{POINT, RECT};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_MENU, VK_RCONTROL, VK_RMENU,
    };

    // ===== Tests for modifier_to_mask =====

    #[test]
    fn test_modifier_to_mask_alt() {
        assert_eq!(modifier_to_mask(ModifierKey::Alt), MOD_ALT);
        assert_eq!(modifier_to_mask(ModifierKey::Alt), 1);
    }

    #[test]
    fn test_modifier_to_mask_ctrl() {
        assert_eq!(modifier_to_mask(ModifierKey::Ctrl), MOD_CTRL);
        assert_eq!(modifier_to_mask(ModifierKey::Ctrl), 2);
    }

    #[test]
    fn test_modifier_to_mask_shift() {
        assert_eq!(modifier_to_mask(ModifierKey::Shift), MOD_SHIFT);
        assert_eq!(modifier_to_mask(ModifierKey::Shift), 4);
    }

    #[test]
    fn test_modifier_to_mask_win() {
        assert_eq!(modifier_to_mask(ModifierKey::Win), MOD_WIN);
        assert_eq!(modifier_to_mask(ModifierKey::Win), 8);
    }

    // ===== Tests for key_to_mask =====

    #[test]
    fn test_key_to_mask_lmenu() {
        assert_eq!(key_to_mask(VK_LMENU.0 as u32), Some(MOD_ALT));
    }

    #[test]
    fn test_key_to_mask_rmenu() {
        assert_eq!(key_to_mask(VK_RMENU.0 as u32), Some(MOD_ALT));
    }

    #[test]
    fn test_key_to_mask_menu() {
        assert_eq!(key_to_mask(VK_MENU.0 as u32), Some(MOD_ALT));
    }

    #[test]
    fn test_key_to_mask_lcontrol() {
        assert_eq!(key_to_mask(VK_LCONTROL.0 as u32), Some(MOD_CTRL));
    }

    #[test]
    fn test_key_to_mask_rcontrol() {
        assert_eq!(key_to_mask(VK_RCONTROL.0 as u32), Some(MOD_CTRL));
    }

    #[test]
    fn test_key_to_mask_lshift() {
        assert_eq!(key_to_mask(VK_LSHIFT.0 as u32), Some(MOD_SHIFT));
    }

    #[test]
    fn test_key_to_mask_lwin() {
        assert_eq!(key_to_mask(VK_LWIN.0 as u32), Some(MOD_WIN));
    }

    #[test]
    fn test_key_to_mask_unknown() {
        assert_eq!(key_to_mask(0), None);
        assert_eq!(key_to_mask(999), None);
    }

    // ===== Tests for process_allowed =====

    #[test]
    fn test_process_allowed_blacklist_empty() {
        let config = AppConfig {
            filter_mode: FilterMode::Blacklist,
            filter_list: vec![],
            ..AppConfig::default()
        };
        assert!(process_allowed(&config, "chrome.exe"));
        assert!(process_allowed(&config, "firefox.exe"));
    }

    #[test]
    fn test_process_allowed_blacklist_with_chrome() {
        let config = AppConfig {
            filter_mode: FilterMode::Blacklist,
            filter_list: vec!["chrome.exe".to_string()],
            ..AppConfig::default()
        };
        assert!(!process_allowed(&config, "chrome.exe"));
        assert!(process_allowed(&config, "firefox.exe"));
    }

    #[test]
    fn test_process_allowed_whitelist_empty() {
        let config = AppConfig {
            filter_mode: FilterMode::Whitelist,
            filter_list: vec![],
            ..AppConfig::default()
        };
        assert!(!process_allowed(&config, "chrome.exe"));
        assert!(!process_allowed(&config, "firefox.exe"));
    }

    #[test]
    fn test_process_allowed_whitelist_with_chrome() {
        let config = AppConfig {
            filter_mode: FilterMode::Whitelist,
            filter_list: vec!["chrome.exe".to_string()],
            ..AppConfig::default()
        };
        assert!(process_allowed(&config, "chrome.exe"));
        assert!(!process_allowed(&config, "firefox.exe"));
    }

    #[test]
    fn test_process_allowed_case_insensitive() {
        let config = AppConfig {
            filter_mode: FilterMode::Blacklist,
            filter_list: vec!["chrome.exe".to_string()],
            ..AppConfig::default()
        };
        assert!(!process_allowed(&config, "Chrome.exe"));
        assert!(!process_allowed(&config, "CHROME.EXE"));
    }

    #[test]
    fn test_process_allowed_whitespace_trimming() {
        let config = AppConfig {
            filter_mode: FilterMode::Blacklist,
            filter_list: vec!["  chrome.exe  ".to_string()],
            ..AppConfig::default()
        };
        assert!(!process_allowed(&config, "chrome.exe"));
    }

    #[test]
    fn test_process_allowed_empty_process_name_blacklist() {
        let config = AppConfig {
            filter_mode: FilterMode::Blacklist,
            filter_list: vec!["chrome.exe".to_string()],
            ..AppConfig::default()
        };
        assert!(process_allowed(&config, ""));
    }

    #[test]
    fn test_process_allowed_empty_process_name_whitelist() {
        let config = AppConfig {
            filter_mode: FilterMode::Whitelist,
            filter_list: vec!["chrome.exe".to_string()],
            ..AppConfig::default()
        };
        assert!(!process_allowed(&config, ""));
    }

    // ===== Tests for determine_mode =====

    #[test]
    fn test_determine_mode_no_modifiers() {
        let config = AppConfig::default();
        assert_eq!(determine_mode(0, &config), None);
    }

    #[test]
    fn test_determine_mode_move_default() {
        let config = AppConfig::default();
        assert_eq!(determine_mode(MOD_ALT, &config), Some(DragMode::Move));
    }

    #[test]
    fn test_determine_mode_resize_default() {
        let config = AppConfig::default();
        assert_eq!(
            determine_mode(MOD_ALT | MOD_SHIFT, &config),
            Some(DragMode::Resize)
        );
    }

    #[test]
    fn test_determine_mode_all_modifiers_resize_priority() {
        let config = AppConfig::default();
        // All modifiers held — resize has priority
        assert_eq!(determine_mode(0xF, &config), Some(DragMode::Resize));
    }

    #[test]
    fn test_determine_mode_only_resize_modifier_2() {
        let config = AppConfig::default();
        // Only shift held (resize_modifier_2) → None (need both resize modifiers)
        assert_eq!(determine_mode(MOD_SHIFT, &config), None);
    }

    #[test]
    fn test_determine_mode_custom_config() {
        let config = AppConfig {
            move_modifier: ModifierKey::Ctrl,
            ..AppConfig::default()
        };
        assert_eq!(determine_mode(MOD_CTRL, &config), Some(DragMode::Move));
    }

    // ===== Tests for determine_resize_direction =====

    #[test]
    fn test_determine_resize_direction_top_left() {
        let rect = RECT {
            left: 0,
            top: 0,
            right: 200,
            bottom: 200,
        };
        let cursor = POINT { x: 50, y: 50 };
        let dir = determine_resize_direction(cursor, rect);
        assert!(matches!(dir, ResizeDirection::TopLeft));
    }

    #[test]
    fn test_determine_resize_direction_top_right() {
        let rect = RECT {
            left: 0,
            top: 0,
            right: 200,
            bottom: 200,
        };
        let cursor = POINT { x: 150, y: 50 };
        let dir = determine_resize_direction(cursor, rect);
        assert!(matches!(dir, ResizeDirection::TopRight));
    }

    #[test]
    fn test_determine_resize_direction_bottom_left() {
        let rect = RECT {
            left: 0,
            top: 0,
            right: 200,
            bottom: 200,
        };
        let cursor = POINT { x: 50, y: 150 };
        let dir = determine_resize_direction(cursor, rect);
        assert!(matches!(dir, ResizeDirection::BottomLeft));
    }

    #[test]
    fn test_determine_resize_direction_bottom_right() {
        let rect = RECT {
            left: 0,
            top: 0,
            right: 200,
            bottom: 200,
        };
        let cursor = POINT { x: 150, y: 150 };
        let dir = determine_resize_direction(cursor, rect);
        assert!(matches!(dir, ResizeDirection::BottomRight));
    }

    #[test]
    fn test_determine_resize_direction_exact_center() {
        let rect = RECT {
            left: 0,
            top: 0,
            right: 200,
            bottom: 200,
        };
        let cursor = POINT { x: 100, y: 100 };
        let dir = determine_resize_direction(cursor, rect);
        // Center (>= comparison) → BottomRight
        assert!(matches!(dir, ResizeDirection::BottomRight));
    }

    // ===== Tests for clamp_rect_for_min_size =====

    #[test]
    fn test_clamp_rect_width_too_small_topleft() {
        let mut rect = RECT {
            left: 100,
            top: 0,
            right: 150, // width = 50 (too small)
            bottom: 200,
        };
        clamp_rect_for_min_size(&mut rect, ResizeDirection::TopLeft);
        assert_eq!(rect.left, 50); // right - 100
        assert_eq!(rect.right, 150);
    }

    #[test]
    fn test_clamp_rect_width_too_small_bottomright() {
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 50, // width = 50 (too small)
            bottom: 200,
        };
        clamp_rect_for_min_size(&mut rect, ResizeDirection::BottomRight);
        assert_eq!(rect.left, 0);
        assert_eq!(rect.right, 100); // left + 100
    }

    #[test]
    fn test_clamp_rect_height_too_small_topleft() {
        let mut rect = RECT {
            left: 0,
            top: 100,
            right: 200,
            bottom: 150, // height = 50 (too small)
        };
        clamp_rect_for_min_size(&mut rect, ResizeDirection::TopLeft);
        assert_eq!(rect.top, 50); // bottom - 100
        assert_eq!(rect.bottom, 150);
    }

    #[test]
    fn test_clamp_rect_both_too_small_topleft() {
        let mut rect = RECT {
            left: 100,
            top: 100,
            right: 150,  // width = 50
            bottom: 150, // height = 50
        };
        clamp_rect_for_min_size(&mut rect, ResizeDirection::TopLeft);
        assert_eq!(rect.left, 50); // right - 100
        assert_eq!(rect.top, 50); // bottom - 100
        assert_eq!(rect.right, 150);
        assert_eq!(rect.bottom, 150);
    }

    #[test]
    fn test_clamp_rect_already_large_enough() {
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 200,
            bottom: 200,
        };
        let original = rect;
        clamp_rect_for_min_size(&mut rect, ResizeDirection::TopLeft);
        assert_eq!(rect.left, original.left);
        assert_eq!(rect.top, original.top);
        assert_eq!(rect.right, original.right);
        assert_eq!(rect.bottom, original.bottom);
    }

    #[test]
    fn test_clamp_rect_exact_min_size() {
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 100,
            bottom: 100,
        };
        let original = rect;
        clamp_rect_for_min_size(&mut rect, ResizeDirection::BottomRight);
        // Should not change if exactly MIN_WINDOW_SIZE
        assert_eq!(rect.left, original.left);
        assert_eq!(rect.top, original.top);
        assert_eq!(rect.right, original.right);
        assert_eq!(rect.bottom, original.bottom);
    }
}
