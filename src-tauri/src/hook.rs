use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
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
    WH_MOUSE_LL, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_QUIT, WM_RBUTTONDOWN,
    WM_RBUTTONUP, WM_SYSKEYDOWN,
};

use crate::config::{AppConfig, FilterMode, GestureTrigger, ModifierKey, ResizeMode};
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
/// do not cancel an active drag.
const GLIDE_SYNTHETIC_EXTRA_INFO: usize = 0x474C_4944; // b'G','L','I','D'
/// Mouse message constants not in the windows crate import set.
const WM_MOUSEWHEEL: u32 = 0x020A;

/// Unassigned virtual key tapped while a hold-and-move modifier is still down,
/// so releasing Alt/Win afterwards does not open a menu bar or the Start menu.
const VK_MENU_MASK: VIRTUAL_KEY = VIRTUAL_KEY(0xE8);

/// Opacity change per scroll tick (out of 255).
const OPACITY_STEP: i32 = 15;
/// Minimum opacity — still slightly visible.
const OPACITY_MIN: u8 = 20;

static HOOK_ENABLED: AtomicBool = AtomicBool::new(true);
static HOOK_READY: AtomicBool = AtomicBool::new(false);
static SHARED_CONFIG: OnceLock<Arc<Mutex<AppConfig>>> = OnceLock::new();
static WORKER_TX: OnceLock<Sender<WorkerEvent>> = OnceLock::new();
static QUEUED_INPUT: AtomicUsize = AtomicUsize::new(0);
static HOOK_THREAD_ID: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Copy)]
struct CapturedButton {
    mode: DragMode,
    modifiers: u32,
    dragging: bool,
}

/// Hold-and-move (`GestureTrigger::Pan`) gesture tracked by the hook thread.
#[derive(Clone, Copy)]
struct PanCapture {
    modifiers: u32,
    /// False when this modifier combination did not start a move (wrong
    /// combination or no eligible window); it stays idle until the combination changes.
    active: bool,
}

thread_local! {
    static CAPTURED_BUTTON: Cell<Option<CapturedButton>> = const { Cell::new(None) };
    static PAN_CAPTURE: Cell<Option<PanCapture>> = const { Cell::new(None) };
}

/// Bitset tracking currently-pressed non-modifier keys, updated by `keyboard_hook_proc`.
/// 256 bits = 8 × AtomicU32, one bit per VK code (0x00–0xFF).
/// Read by `mouse_hook_proc` / the worker to suppress grabs during keyboard shortcuts.
static NON_MOD_PRESSED: [AtomicU32; 8] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];

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
    /// Hold-and-move grab: tap `VK_MENU_MASK` on commit because no click
    /// separates the modifier press from its release.
    mask_menu: bool,
}

enum WorkerEvent {
    /// Point, mode, window, and whether this is a hold-and-move grab.
    Start(POINT, DragMode, usize, bool),
    MouseMove(POINT),
    Finish,
    Cancel,
    MouseWheel(i16, usize),
    Shutdown,
}

fn send_event(event: WorkerEvent) -> bool {
    WORKER_TX.get().is_some_and(|tx| tx.send(event).is_ok())
}

// Bound motion and wheel backlog while preserving ordered gesture transitions.
fn queue_input(event: WorkerEvent) -> bool {
    if QUEUED_INPUT.load(Ordering::Relaxed) >= WORKER_QUEUE_SIZE {
        return false;
    }
    QUEUED_INPUT.fetch_add(1, Ordering::Relaxed);
    if send_event(event) {
        return true;
    }
    QUEUED_INPUT.fetch_sub(1, Ordering::Relaxed);
    false
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

fn poll_modifiers() -> u32 {
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
    mods
}

/// Returns `true` when any non-modifier keyboard key is physically held down.
/// Backed by `NON_MOD_PRESSED` bitset that `keyboard_hook_proc` maintains.
/// A recorded key that the system no longer reports as down lost its key-up
/// (focus moved to an elevated window, a driver sent none); it is cleared here
/// so one missed release cannot block every gesture until restart.
fn any_non_modifier_key_down() -> bool {
    let mut any_down = false;
    for (word, bits) in NON_MOD_PRESSED.iter().enumerate() {
        let mut pending = bits.load(Ordering::Acquire);
        while pending != 0 {
            let bit = pending.trailing_zeros();
            pending &= pending - 1;
            let vk = (word as u32) * 32 + bit;
            if is_virtual_key_down(vk as i32) {
                any_down = true;
            } else {
                bits.fetch_and(!(1u32 << bit), Ordering::Release);
            }
        }
    }
    any_down
}

/// IME and language-toggle keys. Korean Han/Eng and Hanja keys often report a
/// key-down with no matching key-up, so they never count as a held key.
fn is_ime_toggle_key(vk: u32) -> bool {
    matches!(
        vk,
        0x15 // VK_HANGUL / VK_KANA
            | 0x16 // VK_IME_ON
            | 0x17 // VK_JUNJA
            | 0x18 // VK_FINAL
            | 0x19 // VK_HANJA / VK_KANJI
            | 0x1A // VK_IME_OFF
            | 0x1C // VK_CONVERT
            | 0x1D // VK_NONCONVERT
            | 0x1E // VK_ACCEPT
            | 0x1F // VK_MODECHANGE
            | 0xE5 // VK_PROCESSKEY
    )
}

/// Set or clear a bit in `NON_MOD_PRESSED` for a non-modifier VK code.
/// Called from `keyboard_hook_proc` for every key-down / key-up event
/// that is NOT a modifier key.
#[inline]
fn mark_non_mod_key(vk: u32, down: bool) {
    if vk == 0 || vk > 255 || is_ime_toggle_key(vk) {
        return;
    }
    let word = (vk / 32) as usize;
    let bit = 1u32 << (vk % 32);
    if down {
        NON_MOD_PRESSED[word].fetch_or(bit, Ordering::Release);
    } else {
        NON_MOD_PRESSED[word].fetch_and(!bit, Ordering::Release);
    }
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

fn combo_mask(keys: &[ModifierKey]) -> u32 {
    keys.iter()
        .fold(0, |mask, key| mask | modifier_to_mask(*key))
}

/// Whether `mode` is enabled, uses `trigger`, and its exact key set is `mods`.
fn gesture_matches(mode: DragMode, trigger: GestureTrigger, mods: u32, config: &AppConfig) -> bool {
    let (enabled, gesture_trigger, keys) = match mode {
        DragMode::Move => (
            config.move_enabled,
            config.move_trigger,
            &config.move_modifiers,
        ),
        DragMode::Resize => (
            config.resize_enabled,
            config.resize_trigger,
            &config.resize_modifiers,
        ),
    };
    enabled && gesture_trigger == trigger && mods != 0 && mods == combo_mask(keys)
}

fn matches_drag(mode: DragMode, mods: u32, config: &AppConfig) -> bool {
    gesture_matches(mode, GestureTrigger::Click, mods, config)
}

/// The hold-and-move gesture for `mods`; Move wins when both share a key set.
fn pan_mode(mods: u32, config: &AppConfig) -> Option<DragMode> {
    [DragMode::Move, DragMode::Resize]
        .into_iter()
        .find(|mode| gesture_matches(*mode, GestureTrigger::Pan, mods, config))
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

fn eligible_window(point: POINT, config: &AppConfig) -> Option<HWND> {
    let hwnd = window_manager::window_from_point(point.x, point.y)?;
    if !window_manager::is_valid_target(hwnd)
        || (!config.allow_nonforeground && window_manager::get_foreground_window() != Some(hwnd))
    {
        return None;
    }
    if config.filter_mode == FilterMode::Blacklist && config.filter_list.is_empty() {
        return Some(hwnd);
    }
    let name = window_manager::get_process_name(hwnd)?;
    process_allowed(config, &name).then_some(hwnd)
}

fn try_create_grab_state(
    cursor_pos: POINT,
    mode: DragMode,
    hwnd: HWND,
    mask_menu: bool,
) -> Option<GrabState> {
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
        mask_menu,
    })
}

/// Apply all side-effectful operations that must happen exactly once, at the
/// moment the dead-zone threshold is crossed.  Separated from
/// `try_create_grab_state` so that snapped/maximised windows are only
/// restored — and raise_on_grab only fires — when the user has
/// demonstrated clear drag intent (≥ drag_threshold pixels of movement).
fn commit_grab(grab: &mut GrabState, config: &AppConfig, point: POINT) {
    // Restore snapped or maximized windows before the first real move.
    let restored = window_manager::is_maximized(grab.hwnd) || window_manager::is_snapped(grab.hwnd);
    if restored {
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
    if restored {
        grab.last_cursor = point;
        if matches!(grab.mode, DragMode::Resize) {
            grab.resize_dir = determine_resize_direction(point, grab.origin_rect);
        }
    }
    if grab.mask_menu {
        send_menu_mask();
    }
    grab.committed = true;
}

fn finish_grab(state: &mut Option<GrabState>, snap: bool) {
    let Some(grab) = state.take() else { return };
    overlay::hide();
    if !snap || !HOOK_ENABLED.load(Ordering::Acquire) || !window_manager::is_valid_target(grab.hwnd)
    {
        return;
    }
    let Some(config) = current_config() else {
        return;
    };
    let Some((zone, rect)) = grab.snap_target.filter(|_| config.snap_enabled) else {
        return;
    };
    if zone == snap::SnapZone::Maximize {
        window_manager::maximize_window(grab.hwnd);
    } else if !config.snap_native || !apply_snap_native(grab.hwnd, zone) {
        window_manager::resize_window(
            grab.hwnd,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
        );
    }
}

fn worker_handle_mouse_move(point: POINT, state: &mut Option<GrabState>) {
    let Some(config) = current_config() else {
        return;
    };
    let Some(grab) = state.as_mut() else { return };
    if !config.enabled || !window_manager::is_valid_target(grab.hwnd) {
        finish_grab(state, false);
        return;
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
            return;
        }
        commit_grab(grab, &config, point);
    }

    let dx = point.x - grab.last_cursor.x;
    let dy = point.y - grab.last_cursor.y;
    if dx == 0 && dy == 0 {
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
            } else if grab.snap_target.take().is_some() {
                overlay::hide();
            }
        }
        DragMode::Resize => {
            let mut r = grab.origin_rect;
            if config.resize_mode == ResizeMode::Absolute {
                // Absolute mode: cursor right = grow right, cursor down = grow down.
                r.right += grab.cumulative_dx;
                r.bottom += grab.cumulative_dy;
                clamp_rect_for_min_size(&mut r, ResizeDirection::BottomRight);
            } else {
                // Quadrant mode: direction depends on cursor position.
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
            }
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
}

fn worker_handle_scroll(hwnd: HWND, delta: i16) {
    if !HOOK_ENABLED.load(Ordering::Acquire) || !window_manager::is_valid_target(hwnd) {
        return;
    }
    let current = i32::from(window_manager::get_window_opacity(hwnd));
    let step = if delta > 0 {
        OPACITY_STEP
    } else {
        -OPACITY_STEP
    };
    let alpha = (current + step).clamp(i32::from(OPACITY_MIN), 255) as u8;
    window_manager::set_window_opacity(hwnd, alpha);
}

fn worker_loop(rx: Receiver<WorkerEvent>) {
    let mut state = None;
    let mut pending = None;
    while let Some(event) = pending.take().or_else(|| rx.recv().ok()) {
        match event {
            WorkerEvent::Start(point, mode, window, mask_menu) => {
                finish_grab(&mut state, false);
                if HOOK_ENABLED.load(Ordering::Acquire) {
                    state = try_create_grab_state(point, mode, HWND(window as *mut _), mask_menu);
                }
            }
            WorkerEvent::MouseMove(mut point) => {
                QUEUED_INPUT.fetch_sub(1, Ordering::Relaxed);
                while let Ok(event) = rx.try_recv() {
                    if let WorkerEvent::MouseMove(latest) = event {
                        QUEUED_INPUT.fetch_sub(1, Ordering::Relaxed);
                        point = latest;
                    } else {
                        pending = Some(event);
                        break;
                    }
                }
                worker_handle_mouse_move(point, &mut state);
            }
            WorkerEvent::Finish => finish_grab(&mut state, true),
            WorkerEvent::Cancel => finish_grab(&mut state, false),
            WorkerEvent::MouseWheel(delta, window) => {
                QUEUED_INPUT.fetch_sub(1, Ordering::Relaxed);
                worker_handle_scroll(HWND(window as *mut _), delta);
            }
            WorkerEvent::Shutdown => break,
        }
    }
}

fn finish_captured_button(button: &Cell<Option<CapturedButton>>, snap: bool) {
    if let Some(mut capture) = button.get().filter(|capture| capture.dragging) {
        capture.dragging = false;
        // Still consume the matching button-up after a key release or cancellation.
        button.set(Some(capture));
        send_event(if snap {
            WorkerEvent::Finish
        } else {
            WorkerEvent::Cancel
        });
    }
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
            let msg = w_param.0 as u32;
            let pressed = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
            let modifier = key_to_mask(kb.vkCode);
            if modifier.is_none() {
                mark_non_mod_key(kb.vkCode, pressed);
            }
            if let Some(pan) = PAN_CAPTURE.get() {
                match modifier {
                    Some(mask) if !pressed && pan.modifiers & mask != 0 => {
                        if !other_modifier_key_down(kb.vkCode) {
                            end_pan(true);
                        }
                    }
                    Some(mask) if pressed && pan.modifiers & mask == 0 => end_pan(false),
                    None if pressed => end_pan(false),
                    _ => {}
                }
            }
            CAPTURED_BUTTON.with(|button| {
                let Some(capture) = button.get().filter(|capture| capture.dragging) else {
                    return;
                };
                match modifier {
                    Some(mask) if !pressed && capture.modifiers & mask != 0 => {
                        // The key release has not reached GetAsyncKeyState yet.
                        if !other_modifier_key_down(kb.vkCode) {
                            finish_captured_button(button, true);
                        }
                    }
                    Some(mask) if pressed && capture.modifiers & mask == 0 => {
                        finish_captured_button(button, false);
                    }
                    None if pressed => finish_captured_button(button, false),
                    _ => {}
                }
            });
        }
    }

    unsafe { CallNextHookEx(None, n_code, w_param, l_param) }
}

fn other_modifier_key_down(vk: u32) -> bool {
    for (left, right) in [
        (VK_LMENU, VK_RMENU),
        (VK_LCONTROL, VK_RCONTROL),
        (VK_LSHIFT, VK_RSHIFT),
        (VK_LWIN, VK_RWIN),
    ] {
        if vk == u32::from(left.0) {
            return is_virtual_key_down(i32::from(right.0));
        }
        if vk == u32::from(right.0) {
            return is_virtual_key_down(i32::from(left.0));
        }
    }
    false
}

/// Create a keyboard INPUT carrying our synthetic marker.
fn synthetic_key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
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
}

/// Tap an unassigned key so the held modifier no longer counts as a lone
/// press; its release then opens no menu bar (Alt) or Start menu (Win).
fn send_menu_mask() {
    let inputs = [
        synthetic_key(VK_MENU_MASK, false),
        synthetic_key(VK_MENU_MASK, true),
    ];
    unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
}

/// End the hold-and-move gesture, if any.
fn end_pan(snap: bool) {
    if let Some(pan) = PAN_CAPTURE.take() {
        if pan.active {
            send_event(if snap {
                WorkerEvent::Finish
            } else {
                WorkerEvent::Cancel
            });
        }
    }
}

/// Hold-and-move: follow the cursor while the move modifier is held, with no
/// click. The worker's drag threshold decides when the window starts moving.
fn handle_pan_move(point: POINT) {
    let mods = poll_modifiers();
    if let Some(pan) = PAN_CAPTURE.get() {
        if !HOOK_ENABLED.load(Ordering::Acquire) || any_non_modifier_key_down() {
            end_pan(false);
            return;
        }
        if mods == pan.modifiers {
            if pan.active {
                queue_input(WorkerEvent::MouseMove(point));
            }
            return;
        }
        end_pan(true);
    }
    if !HOOK_ENABLED.load(Ordering::Acquire) || mods == 0 || any_non_modifier_key_down() {
        return;
    }
    let Some(config) = SHARED_CONFIG.get().and_then(|shared| shared.try_lock()) else {
        return;
    };
    let target = pan_mode(mods, &config).map(|mode| (mode, config.clone()));
    drop(config);
    let active = target
        .and_then(|(mode, config)| eligible_window(point, &config).map(|hwnd| (mode, hwnd)))
        .is_some_and(|(mode, hwnd)| {
            send_event(WorkerEvent::Start(point, mode, hwnd.0 as usize, true))
        });
    PAN_CAPTURE.set(Some(PanCapture {
        modifiers: mods,
        active,
    }));
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
fn apply_snap_native(hwnd: HWND, zone: snap::SnapZone) -> bool {
    // On mouse release a modifier may still be down. Direct placement keeps
    // that modifier from changing the meaning of the synthesized shortcut.
    if poll_modifiers() != 0 || any_non_modifier_key_down() {
        return false;
    }
    if window_manager::get_foreground_window() != Some(hwnd) {
        window_manager::set_foreground(hwnd);
        if window_manager::get_foreground_window() != Some(hwnd) {
            return false;
        }
    }
    let h_vk: VIRTUAL_KEY = match zone {
        snap::SnapZone::Left | snap::SnapZone::TopLeft | snap::SnapZone::BottomLeft => VK_LEFT,
        snap::SnapZone::Right | snap::SnapZone::TopRight | snap::SnapZone::BottomRight => VK_RIGHT,
        snap::SnapZone::Maximize => return false, // handled separately via SW_MAXIMIZE
    };

    let v_vk: Option<VIRTUAL_KEY> = match zone {
        snap::SnapZone::TopLeft | snap::SnapZone::TopRight => Some(VK_UP),
        snap::SnapZone::BottomLeft | snap::SnapZone::BottomRight => Some(VK_DOWN),
        _ => None,
    };

    let ki = synthetic_key;

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

    (unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) }) as usize == inputs.len()
}

/// Handle button capture and pass matching window work to the worker.
unsafe extern "system" fn mouse_hook_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0
        && CAPTURED_BUTTON.with(|button| {
            let msg = w_param.0 as u32;
            let down = match msg {
                WM_LBUTTONDOWN => Some(DragMode::Move),
                WM_RBUTTONDOWN => Some(DragMode::Resize),
                _ => None,
            };
            let up = match msg {
                WM_LBUTTONUP => Some(DragMode::Move),
                WM_RBUTTONUP => Some(DragMode::Resize),
                _ => None,
            };
            if let Some(capture) = button.get() {
                if up == Some(capture.mode) {
                    finish_captured_button(button, true);
                    button.set(None);
                    return true;
                }
                if msg == WM_MOUSEMOVE && capture.dragging {
                    if !HOOK_ENABLED.load(Ordering::Acquire) || any_non_modifier_key_down() {
                        finish_captured_button(button, false);
                    } else if poll_modifiers() != capture.modifiers {
                        finish_captured_button(button, true);
                    } else {
                        let mouse = unsafe { &*(l_param.0 as *const MSLLHOOKSTRUCT) };
                        queue_input(WorkerEvent::MouseMove(mouse.pt));
                    }
                }
                return down == Some(capture.mode);
            }
            if msg == WM_MOUSEMOVE {
                let mouse = unsafe { &*(l_param.0 as *const MSLLHOOKSTRUCT) };
                handle_pan_move(mouse.pt);
                return false;
            }
            if !HOOK_ENABLED.load(Ordering::Acquire) || (down.is_none() && msg != WM_MOUSEWHEEL) {
                return false;
            }
            let mods = poll_modifiers();
            if mods == 0 || any_non_modifier_key_down() {
                return false;
            }
            let Some(config) = SHARED_CONFIG.get().and_then(|shared| shared.try_lock()) else {
                return false;
            };
            let matches = if let Some(mode) = down {
                matches_drag(mode, mods, &config)
            } else {
                config.scroll_opacity && mods == modifier_to_mask(config.scroll_opacity_modifier)
            };
            if !matches {
                return false;
            }
            let config = config.clone();
            let mouse = unsafe { &*(l_param.0 as *const MSLLHOOKSTRUCT) };
            let Some(hwnd) = eligible_window(mouse.pt, &config) else {
                return false;
            };
            let window = hwnd.0 as usize;
            if let Some(mode) = down {
                end_pan(false);
                if send_event(WorkerEvent::Start(mouse.pt, mode, window, false)) {
                    button.set(Some(CapturedButton {
                        mode,
                        modifiers: mods,
                        dragging: true,
                    }));
                    return true;
                }
                false
            } else {
                let delta = (mouse.mouseData >> 16) as i16;
                delta != 0 && queue_input(WorkerEvent::MouseWheel(delta, window))
            }
        })
    {
        return LRESULT(1);
    }
    unsafe { CallNextHookEx(None, n_code, w_param, l_param) }
}

fn hook_thread_main(ready: Sender<bool>) {
    // Create the snap overlay window on this thread (needs the message loop).
    overlay::create();

    let (worker_tx, worker_rx) = mpsc::channel::<WorkerEvent>();
    let _ = WORKER_TX.set(worker_tx);

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
            let _ = ready.send(false);
            return;
        }
    };

    thread::spawn(move || worker_loop(worker_rx));
    HOOK_READY.store(true, Ordering::Release);
    let _ = ready.send(true);
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
    HOOK_READY.store(false, Ordering::Release);
    let _ = unsafe { UnhookWindowsHookEx(keyboard_hook) };
    let _ = unsafe { UnhookWindowsHookEx(mouse_hook) };
    send_event(WorkerEvent::Shutdown);
    overlay::destroy();
}

pub fn start_hook_thread(config: Arc<Mutex<AppConfig>>) -> bool {
    let _ = SHARED_CONFIG.set(config);
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        HOOK_THREAD_ID.store(unsafe { GetCurrentThreadId() }, Ordering::Release);
        hook_thread_main(tx);
        HOOK_THREAD_ID.store(0, Ordering::Release);
    });
    rx.recv().unwrap_or(false)
}

pub fn is_ready() -> bool {
    HOOK_READY.load(Ordering::Acquire)
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
    let previous = HOOK_ENABLED.swap(enabled, Ordering::AcqRel);
    if previous && !enabled {
        send_event(WorkerEvent::Cancel);
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

    #[test]
    fn each_button_requires_its_configured_modifiers() {
        let config = AppConfig::default();
        assert!(matches_drag(DragMode::Move, MOD_ALT, &config));
        assert!(!matches_drag(DragMode::Resize, MOD_ALT, &config));
        assert!(matches_drag(DragMode::Resize, MOD_ALT | MOD_SHIFT, &config));
        assert!(!matches_drag(DragMode::Move, MOD_ALT | MOD_SHIFT, &config));
        assert!(!matches_drag(DragMode::Move, 0, &config));
    }

    #[test]
    fn buttons_can_share_modifiers_and_be_disabled_independently() {
        let mut config = AppConfig {
            resize_modifiers: vec![ModifierKey::Alt],
            ..AppConfig::default()
        };
        assert!(matches_drag(DragMode::Move, MOD_ALT, &config));
        assert!(matches_drag(DragMode::Resize, MOD_ALT, &config));
        config.move_enabled = false;
        assert!(!matches_drag(DragMode::Move, MOD_ALT, &config));
        assert!(matches_drag(DragMode::Resize, MOD_ALT, &config));
        config.resize_enabled = false;
        assert!(!matches_drag(DragMode::Resize, MOD_ALT, &config));
    }

    #[test]
    fn hold_and_move_leaves_click_to_the_app() {
        let config = AppConfig {
            move_trigger: GestureTrigger::Pan,
            ..AppConfig::default()
        };
        assert!(!matches_drag(DragMode::Move, MOD_ALT, &config));
        assert!(matches_drag(DragMode::Resize, MOD_ALT | MOD_SHIFT, &config));
        assert_eq!(pan_mode(MOD_ALT, &config), Some(DragMode::Move));
        assert_eq!(pan_mode(MOD_ALT | MOD_SHIFT, &config), None);
    }

    #[test]
    fn language_toggle_keys_never_count_as_held() {
        assert!(is_ime_toggle_key(0x15));
        assert!(is_ime_toggle_key(0x19));
        assert!(!is_ime_toggle_key(0x41)); // A
        assert!(!is_ime_toggle_key(0x09)); // Tab
    }

    #[test]
    fn combinations_match_exactly() {
        let config = AppConfig {
            move_modifiers: vec![ModifierKey::Ctrl, ModifierKey::Alt, ModifierKey::Win],
            resize_modifiers: vec![ModifierKey::Win, ModifierKey::Shift],
            resize_trigger: GestureTrigger::Pan,
            ..AppConfig::default()
        };
        assert!(matches_drag(
            DragMode::Move,
            MOD_CTRL | MOD_ALT | MOD_WIN,
            &config
        ));
        assert!(!matches_drag(DragMode::Move, MOD_CTRL | MOD_ALT, &config));
        assert!(!matches_drag(
            DragMode::Resize,
            MOD_WIN | MOD_SHIFT,
            &config
        ));
        assert_eq!(
            pan_mode(MOD_WIN | MOD_SHIFT, &config),
            Some(DragMode::Resize)
        );
        assert_eq!(pan_mode(MOD_WIN, &config), None);
    }

    #[test]
    fn shared_pan_combination_prefers_move() {
        let config = AppConfig {
            move_trigger: GestureTrigger::Pan,
            resize_modifiers: vec![ModifierKey::Alt],
            resize_trigger: GestureTrigger::Pan,
            ..AppConfig::default()
        };
        assert_eq!(pan_mode(MOD_ALT, &config), Some(DragMode::Move));
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
