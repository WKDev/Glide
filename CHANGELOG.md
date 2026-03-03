# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
