# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2026-11-23

### Added

- Move and Resize accept a combination of one to three modifier keys (Alt, Ctrl, Shift, Win) and fire only on that exact combination. Settings from 0.2 carry over.
- Each gesture can start with a click-and-drag or just by moving the mouse past the drag threshold, with no click.
- Redesigned settings window in the Windows 11 style: keycap pickers that light up as you hold keys, conflict warnings, and Gestures, Behavior, Apps, and About pages.
- About shows the author and source code links and offers optional donations through GitHub Sponsors and Buy Me a Coffee. A startup prompt appears at launch until you choose a donation link; "Not now" hides it for 30 days.
- GitHub builds accept supporter codes, tied to the supporter's email, which show a thank-you card and badge. Microsoft Store builds do not accept supporter codes, so donations there give nothing in return.

### Changed

- Window move and resize require their configured modifiers and mouse button.
- GitHub updates install from Settings on request. Store builds use Microsoft Store updates and the Windows startup task.
- The tray can pause and resume window controls and reports a hook startup failure.

### Fixed

- Serialize settings saves and retain the latest edit if an earlier write is still in progress.
- Finalize snapping when the button or modifier is released. Use direct placement when native snapping cannot safely target the grabbed window.
- Apply the process and foreground filters to opacity scrolling and pass through rejected wheel input.

### Security

- Bumped transitive dependencies to patch RustSec advisories: `rustls-webpki` 0.103.9 → 0.103.13 (RUSTSEC-2026-0049/0098/0099/0104) and `tar` 0.4.44 → 0.4.46 (RUSTSEC-2026-0067/0068)

## [0.2.3] - 2026-06-25

### Fixed

- Move/Resize enable toggles became unclickable once turned off — the toggle sat inside its own `pointer-events: none` disabled container, so the feature could not be re-enabled. The toggle is now a sibling of the dimmed controls and stays interactive.

## [0.2.2] - 2026-03-05

### Added

- Per-action enable/disable switches for Move and Resize in Modifier Keys section
- Resize mode setting: "Resize from nearest corner" (quadrant) vs absolute direction
- Scroll opacity controls moved to Modifier Keys section for better organization
- Visual spacers (dividers) between setting panels for clearer grouping
- Blacklist empty-state info message: "No processes are excluded. Glide will work on all windows."

### Fixed

- `raise_on_grab` (raise window on grab) now works reliably using `SetForegroundWindow` + `SetWindowPos` fallback
- Unclosed delimiter in `hook.rs` resize logic

### Removed

- Middle-click always-on-top feature (`middleclick_topmost` config option and all related code)

## [0.1.1] - 2026-03-01

### Fixed

- Settings window opening no longer forcibly re-enables the hook; saved enabled state is respected
- Removed duplicate config save that redundantly wrote to the frontend store on every settings change
- Autostart toggle failure is now surfaced in the UI instead of being silently ignored
- Hook enabled state sync failure on startup now logs to console instead of being silently swallowed
- Silent config deserialization failure now logs `warn` instead of silently falling back to defaults
- Silent config store open failure now logs `warn` instead of being swallowed
- Hook thread startup failure now logs `error` instead of silently running without hooks
- `apply_snap_native`: SetForegroundWindow denial now logged as `warn`
- Frontend: `get_config`, `isEnabled`, `get_running_processes` failures now log to `console.error`

### Changed

- `pnpm audit` threshold lowered from `high` to `moderate` to catch a wider class of npm vulnerabilities
- `set_foreground` in `window_manager.rs` now returns `bool` indicating OS acceptance
- ESLint `no-empty` rule tightened — empty catch blocks no longer allowed
- Sentry `traces_sample_rate` explicitly set to `0.0` (errors only, no performance overhead)
- `update_config` documents the `Arc` / OnceLock ownership pattern with inline comments

### Added

- Vitest unit test suite for `src/lib/config.ts` (24 tests covering defaults, modifier options, and type safety)
- Frontend test job added to CI pipeline
- Sentry DSN wiring in CI build and release workflows via `SENTRY_DSN` repository secret

### Removed

- `store.ts` frontend module (redundant; backend already persists config via `set_config` command)
- Dead Rust API surface: `get_dwm_frame_rect`, `get_border_offsets`, `set_window_rect` (window_manager.rs)
- `stop_hook_thread` (hook.rs) — hook lifecycle is managed by process exit

## [0.1.0] - 2026-02-26

### Added

- Modifier key + mouse window move (hold modifier + left-drag)
- Modifier key + mouse window resize (hold two modifiers + right-drag)
- Process filter (whitelist/blacklist mode)
- Autostart with Windows
- Edge snapping
- Scroll to change window opacity
- Middle-click always-on-top toggle
- System tray icon with settings window
- Settings auto-save
