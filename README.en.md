# SysMon — Windows System Monitor

[中文](README.md) | [**English**](README.en.md)

A lightweight Windows desktop system monitor with a floating always-on-top panel showing CPU, memory, and network speed, plus a taskbar-embedded network speed bar.

## Features

- **Floating Monitor Panel** — Borderless always-on-top window displaying CPU/memory/network in a compact layout, no taskbar space used
- **Taskbar Network Speed Bar** — Embedded into the Windows taskbar via Win32 API, showing real-time upload/download speeds
- **System Tray** — Runs in the background with a right-click menu to show the main window, open settings, or exit
- **Theme Switching** — Light/dark dual themes, managed from the settings window
- **Persistent Settings** — Taskbar visibility, window always-on-top, and other preferences are saved automatically

## Tech Stack

| Component | Technology |
|-----------|------------|
| Desktop Framework | Tauri v2 |
| Backend | Rust + sysinfo + Windows API (Win32) |
| Frontend | Vanilla HTML / CSS / JS (no framework) |
| Async Runtime | Tokio |

## Project Structure

```
sysmon/
├── src/                    # Frontend
│   ├── main.html/js        # Main monitor window
│   ├── taskbar.html/js     # Taskbar network speed window
│   ├── settings.html/js    # Settings window
│   └── styles/             # Stylesheets (including theme variables)
└── src-tauri/
    ├── tauri.conf.json     # App configuration
    ├── Cargo.toml          # Rust dependencies
    ├── capabilities/       # Window permission configs
    └── src/
        ├── lib.rs          # App entry, event system, timers
        ├── sys_info.rs     # CPU/memory/network collection
        ├── tray.rs         # System tray
        ├── taskbar_window.rs # Taskbar window embedding
        ├── commands.rs     # Data structures
        └── settings.rs     # Settings persistence
```

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 18+
- [pnpm](https://pnpm.io/)
- Windows 10/11

### Install Dependencies

```bash
pnpm install
```

### Development

```bash
cargo tauri dev
```

### Build for Release

```bash
cargo tauri build
```

## Usage

### Main Window

After launch, the main window floats at the center of the screen, displaying:
```
Upload: 0.28 KB/s   Download: 0.61 KB/s
CPU: 34%            Memory: 70%
```

- The window is always on top and can be dragged to reposition
- Closing the window hides it to the system tray (does not exit)

### System Tray

Right-click the tray icon:
- **Show Main Window** — Show and focus the main window
- **Settings** — Open the settings window
- **Exit** — Quit the application

Left-click the tray icon to show the main window.

### Settings

| Setting | Description | Default |
|---------|-------------|---------|
| Theme | Light / Dark | Light |
| Taskbar Display | Show / hide the taskbar network speed bar | Hidden |
| Always on Top | Enable / disable main window always-on-top | Enabled |

Settings are saved to `%APPDATA%\com.sysmon.app\settings.json`.

## Architecture Overview

```
Windows API (sysinfo)
        │
   Rust Backend (1.5s polling)
        │
   ┌────┴────┐
   ▼         ▼
 Main      Taskbar
 Window    Window
(CPU/Mem   (Network
 /Net)      Speed)
```

- The backend collects system info every 1.5 seconds and pushes it to the frontend via Tauri events
- The taskbar window is embedded into the Windows taskbar using the `SetParent` Win32 API
- Theme changes are broadcast to all windows via `emit('theme-changed')`
- Settings are persisted via `invoke()` calls to Rust commands

## License

MIT
