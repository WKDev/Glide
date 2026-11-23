# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

Rendered inside a Tauri 2 WebView2 window on Windows only (Windows 10/11).

## Users

Everyday Windows users, including people trying modifier-key window dragging for the first time (as on macOS/Linux desktops). They open the settings window rarely: once to understand how Glide works, then occasionally to change a shortcut or exclude an app. Plain explanations and safe defaults matter more than density.

## Product Purpose

Glide lets a user move or resize any window by holding modifier keys and using the mouse, without aiming for the title bar or window edges. Success is a user who understands the gestures within seconds of opening settings and can change them without breaking anything.

## Operating Context

- Runs in the system tray; the settings window opens from the tray icon.
- Distributed through GitHub Releases (NSIS/MSI) and being prepared for the Microsoft Store (MSIX).
- Settings save automatically (debounced); a failed save offers a retry.
- The tray can pause and resume window controls.

## Capabilities and Constraints

- Move: modifier keys + left-click drag, or modifier keys + mouse movement past the drag threshold (no click).
- Resize: modifier keys + right-click drag; from the nearest corner or always bottom-right.
- Opacity: modifier key + scroll wheel.
- Modifier keys: Alt, Ctrl, Shift, Win. Move and Resize accept a combination of one to three keys; a gesture fires only on the exact combination.
- Edge snapping (optional Windows 11 snap groups), drag threshold, raise on grab, non-foreground windows.
- Process filter: blacklist or whitelist of executable names.
- Start with Windows, update check, version/about.
- UI copy is English only.

## Brand Commitments

- Name "Glide" and the logo at `assets/glide.png`.
- The settings window should feel like a native Windows 11 settings surface first.

## Evidence on Hand

- `assets/glide.png` logo, `assets/glide-readme-title.png` banner.
- No demo screenshots, testimonials, or usage data exist; do not fabricate any.

## Product Principles

1. Explain each gesture in terms of what the hands do.
2. Defaults work without visiting settings.
3. Feel native to Windows 11, not like a web page.
4. Never let a setting silently break a gesture; surface conflicts.
