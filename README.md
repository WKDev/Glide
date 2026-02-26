# wkgrip

A modifier key + mouse window move/resize utility for Windows. Hold a modifier key and use your mouse to move or resize any window without clicking the title bar.

## Features

- **Window Moving**: Hold Alt + left-click and drag to move any window
- **Window Resizing**: Hold Alt+Shift + right-click and drag to resize any window
- **Configurable Modifiers**: Customize which modifier keys trigger move and resize actions (Alt, Ctrl, Shift, Win)
- **Process Filtering**: Whitelist or blacklist specific applications from using the utility
- **Auto-start**: Optionally launch wkgrip automatically on Windows startup
- **Window Snapping**: Snap windows to grid positions with configurable threshold
- **Scroll Opacity**: Adjust window opacity while scrolling with modifier key held
- **Middle-click Topmost**: Bring window to top with middle-click while holding modifier
- **System Tray Integration**: Lightweight tray icon for quick access to settings
- **Settings Persistence**: All configuration automatically saved with debouncing

## Installation

Download the latest installer from the [GitHub Releases](https://github.com/yourusername/wkgrip/releases) page:

- **NSIS Installer** (.exe) — Recommended for most users
- **MSI Installer** (.msi) — Alternative Windows installer format

Run the installer and follow the on-screen prompts. The application will be added to your system tray.

## Usage

### Basic Controls

- **Move Window**: Hold **Alt** + left-click and drag anywhere on a window
- **Resize Window**: Hold **Alt+Shift** + right-click and drag to resize
- **Adjust Opacity**: Hold **Alt** + scroll wheel to adjust window transparency
- **Bring to Top**: Middle-click while holding **Alt** to bring window to foreground

### Configuration

Access settings by clicking the wkgrip icon in your system tray. The following options are available:

| Option                | Description                                  | Default   |
| --------------------- | -------------------------------------------- | --------- |
| `move_modifier`       | Modifier key for moving windows              | Alt       |
| `resize_modifier_1`   | First modifier for resizing                  | Alt       |
| `resize_modifier_2`   | Second modifier for resizing                 | Shift     |
| `filter_mode`         | Process filtering mode (whitelist/blacklist) | Blacklist |
| `autostart`           | Launch on Windows startup                    | Off       |
| `snap_enabled`        | Enable window snapping to grid               | On        |
| `scroll_opacity`      | Allow opacity adjustment via scroll          | On        |
| `middleclick_topmost` | Bring window to top on middle-click          | On        |

Settings are automatically saved with a 220ms debounce.

## Development

### Prerequisites

- **Rust toolchain** (1.70+) — [Install from rustup.rs](https://rustup.rs/)
- **Node.js 18+** — [Download from nodejs.org](https://nodejs.org/)
- **pnpm** — Install with `npm install -g pnpm`
- **Windows** — This project is Windows-only

### Getting Started

1. Clone the repository:

   ```bash
   git clone https://github.com/yourusername/wkgrip.git
   cd wkgrip
   ```

2. Install dependencies:

   ```bash
   pnpm install
   ```

3. Start the development server:
   ```bash
   pnpm tauri dev
   ```

This launches the Tauri development window with hot-reload enabled.

### Type Checking

Run type checking with:

```bash
pnpm check
```

Watch mode:

```bash
pnpm check:watch
```

## Building

To create a production build:

```bash
pnpm tauri build
```

This generates:

- **NSIS Installer** — `src-tauri/target/release/bundle/nsis/wkgrip_0.1.0_x64-setup.exe`
- **MSI Installer** — `src-tauri/target/release/bundle/msi/wkgrip_0.1.0_x64.msi`

Both installers include the complete application and can be distributed to end users.

## Known Limitations

- **Windows Only** — This utility is designed exclusively for Windows and does not support macOS or Linux
- **Content Security Policy** — CSP is set to null for this application; this is a known security consideration and should be reviewed if the application expands in scope
- **Foreground Window Requirement** — By default, the utility only works on the foreground window (configurable via `allow_nonforeground` setting)
- **No Undo** — Window moves and resizes cannot be undone; they are immediate

## License

MIT License — See LICENSE file for details.
