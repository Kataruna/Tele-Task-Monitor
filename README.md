# Tele-Task-Monitor

A lean, windowless **Windows 11 System Tray telemetry server** and **pluggable web dashboard** that streams live CPU, RAM, GPU, VRAM, per-core load, and top processes to an iPad (or any browser on your LAN) at 1 Hz over Server-Sent Events (SSE).

- **0% CPU when idle:** The sampler thread parks on a condition variable when no browser/iPad is connected.
- **Stops sampling when nobody can see it:** a dashboard in a background tab, or on a locked iPad, closes its stream and reconnects when shown again, so the PC stops sampling too.
- **Gives memory back when idle:** the sampler frees its scratch buffers when the last viewer leaves, and trims the process working set after 30 s without one.
- **< 0.5% CPU & < 15 MB RAM when active:** Runs at `BELOW_NORMAL_PRIORITY_CLASS` + Windows 11 EcoQoS so games always pre-empt it.
- **Native System Tray App:** Runs silently in the Windows notification tray with one-click LAN URL copy, dashboard launch, Settings, and Quit.
- **Pluggable Full-Layout Themes:** Each theme in `web/themes/<id>/` defines its own complete HTML structure, CSS styling, and JS rendering logic.

---

## Project Structure

```text
Tele-Task-Monitor/
├── Cargo.toml                  # Rust dependencies & release profile (LTO, stripped)
├── README.md                   # Project overview & quick start
├── AGENTS.md                   # Entry point & strict invariants for AI coding agents
├── tele-task-monitor.exe       # Standalone release binary (windowless tray app)
├── settings.json               # Auto-generated persistent user settings (port, theme, limits)
│
├── docs/
│   ├── THEME_GUIDE.md          # Manual implementation guide & starter template for creating new themes
│   └── THEME_GUIDE_AI.md       # AI Agent specification for zero-shot theme generation
│
├── src/                        # Rust Backend (Axum). Portable core + one OS backend per platform
│   ├── main.rs                 # Entry point, hot-reloading TCP listener, tray thread launch
│   │
│   │   # ---- Portable core (no OS-specific code; shared by every platform) ----
│   ├── server.rs               # Axum routes: `/api/stream` (SSE), `/api/settings` (GET/POST), static `web/`
│   ├── settings.rs             # Persistent `UserSettings` (`settings.json`) load/save/sanitize
│   ├── config.rs               # CLI flag parser (`--port`, `--bind`, `--web-dir`)
│   ├── model.rs                # Wire JSON structs (`Info`, `Sample`, `AppEntry`)
│   ├── aggregate.rs            # Case-insensitive process grouping & Top-N sorting
│   ├── viewers.rs              # Active SSE viewer counter + Condvar sleep/wake gate
│   ├── lan.rs                  # LAN URLs shown in the tray menu (`if-addrs`)
│   ├── sampler/
│   │   ├── mod.rs              # 1 Hz sampling loop (sleeps when `viewers == 0`), drives the platform readers
│   │   ├── cpu.rs              # Busy % math, `RawProc`, per-process CPU % tracker
│   │   └── gpu.rs              # `GpuReading` shared by all GPU readers
│   │
│   │   # ---- Everything OS-specific ----
│   └── platform/
│       ├── mod.rs              # Picks the backend for the target OS + the contract (traits) each backend implements
│       ├── windows/            # Windows 10/11 backend (complete)
│       │   ├── dialog.rs       # `MessageBoxW` error/help popup
│       │   ├── priority.rs     # `BELOW_NORMAL` process priority + EcoQoS power throttling, working-set trim
│       │   ├── tray.rs         # Win32 System Tray icon, procedural ARGB icon, context menu
│       │   └── sampler/
│       │       ├── mod.rs          # `Gpu` = DXGI adapter + PDH counters
│       │       ├── system.rs       # `GetSystemTimes`, `NtQuerySystemInformation` per-core CPU, RAM, CPUID
│       │       ├── procs.rs        # `NtQuerySystemInformation(SystemProcessInformation)` CPU time & private working set
│       │       ├── gpu_adapter.rs  # DXGI hardware adapter selection + `D3DKMTQueryAdapterInfo` temp/power
│       │       └── gpu_counters.rs # PDH `GPU Engine` & `GPU Process Memory` filtered by primary adapter LUID
│       └── macos/              # macOS 11+ backend (Apple Silicon & Intel)
│           ├── ffi.rs          # Mach host/task ports, timebase, QoS declarations
│           ├── dialog.rs       # Alert via `osascript`
│           ├── priority.rs     # Utility QoS + nice 10
│           ├── tray.rs         # `NSStatusItem` menu-bar item (objc2-app-kit)
│           └── sampler/
│               ├── mod.rs
│               ├── system.rs   # `host_processor_info` per-core CPU, `host_statistics64` RAM, `sysctl`
│               ├── procs.rs    # `proc_listpids` + `proc_pid_rusage` CPU time & physical footprint
│               └── gpu.rs      # IOKit `IOAccelerator` "PerformanceStatistics" usage & memory
├── scripts/
│   └── bundle-macos.sh         # Builds dist/Tele-Task-Monitor.app (menu-bar only, ad-hoc signed)
│
└── web/                        # Frontend Host & Pluggable Themes (no build step required)
    ├── index.html              # Host shell, PWA meta tags, theme dropdown, and Settings modal
    ├── style.css               # Full-bleed viewport lock, iPadOS safe-area plates, Settings UI styles
    ├── app.js                  # SSE client, 60s rolling history store, theme loader, `hostApi`
    ├── manifest.webmanifest    # Standalone PWA manifest for iPadOS / iOS Home Screen installation
    └── themes/
        ├── manifest.json       # Registry of installed themes (`id`, `name`, `bgColor`)
        ├── nier/               # "Automata" (default) — Inspired by NieR:Automata YoRHa Plug-in Chips UI
        │   ├── template.html
        │   ├── style.css
        │   └── theme.js
        ├── doraemon/           # "Village" — Inspired by Doraemon Story of Seasons wooden farm status board
        │   ├── template.html
        │   ├── style.css
        │   └── theme.js
        └── default/            # "Classic Dark" — 4-tile dark telemetry dashboard
            ├── template.html
            ├── style.css
            └── theme.js
```

## Platform Support & Cross-Platform Development

| Platform | Status |
|---|---|
| Windows 10 / 11 | Complete |
| macOS 11+ (Apple Silicon & Intel) | Implemented (menu-bar app). Per-app GPU %, GPU temperature/power, and other users' processes aren't available through public macOS APIs, so those parts of the dashboard stay empty. |

All OS-specific code lives under `src/platform/<os>/`. Everything else in `src/` and the whole `web/` folder is shared. `src/platform/mod.rs` defines the contract each backend must implement: the `CpuLoad`, `ProcessSource` and `GpuSource` traits, plus a few free functions such as `tray::run`. If a backend is missing a piece, it fails to compile. The `/api/stream` JSON is the same on every OS, so themes need no changes.

To add a platform-specific dependency, put it under `[target.'cfg(windows)'.dependencies]` or `[target.'cfg(target_os = "macos")'.dependencies]` in `Cargo.toml`, so the other OS never builds it.

You can type-check both backends from any machine (no linker needed):

```bash
rustup target add x86_64-pc-windows-msvc aarch64-apple-darwin
cargo check --all-targets --target x86_64-pc-windows-msvc
cargo check --all-targets --target aarch64-apple-darwin
```

To build and test for real, run `cargo test` and `cargo build --release` on the target OS itself.

### Running on macOS

```bash
xcode-select --install
```
```bash
scripts/bundle-macos.sh
```
```bash
open dist/Tele-Task-Monitor.app
```

A gauge icon appears in the menu bar. Its menu matches the Windows tray menu. Settings are stored in `~/Library/Application Support/Tele-Task-Monitor/settings.json`. The first time an iPad connects, macOS asks whether to accept incoming connections: click **Allow**. To run without bundling, use `cargo run --release` from the repo root.

---

## Quick Start

### 1. Run the Application
Double-click **`tele-task-monitor.exe`** (or build from source with `cargo build --release`).
The application starts silently in your **Windows System Tray** (near the clock).

- **Left-click or Right-click the Tray Icon:**
  - **Open Dashboard (`http://localhost:8800/`)** *(or double-click the tray icon)*
  - **Settings…** *(opens the System Settings modal)*
  - **Copy iPad URL: `http://192.168.x.x:8800/`** *(copies your LAN address to the clipboard)*
  - **Quit Tele-Task-Monitor** *(cleanly removes the tray icon and stops the server)*

### 2. Open on iPad (and Install as a Full-Screen Web App)
1. Open `http://<your-pc-lan-ip>:8800/` in **Safari** on your iPad.
2. Tap **Share → Add to Home Screen** to install it as a standalone full-screen web app.

### 3. Windows Firewall (First Run on a Private LAN)
If your iPad cannot reach the PC on first run, allow incoming connections on your Private network in an elevated PowerShell:

```powershell
New-NetFirewallRule -DisplayName "Tele-Task-Monitor" -Direction Inbound -Action Allow `
  -Program "$PWD\tele-task-monitor.exe" -Profile Private
```

---

## System Settings

Open **Settings** from any theme via **`Theme ▾` → `⚙ Settings…`**, the **`⚙ SYSTEM`** tab in **Automata**, or **`Settings…`** in the Windows System Tray menu:

| Setting | Default | Description |
|---|---|---|
| **Default Theme** | `Automata` (`nier`) | Startup theme for all connected devices (`Automata`, `Village`, `Classic Dark`) |
| **Overview Items Limit** | `8` (`1–25`) | Processes shown per category box on the Overview screen |
| **Detail Page Items Limit** | `24` (`1–60`) | Processes shown on full detail pages (`CPU`, `MEMORY`, `GPU / VRAM`, `INTEL`) |
| **Server Port** | `8800` (`1024–65535`) | Hot-switches the HTTP server & System Tray icon to the new port without restarting |

All settings are saved automatically to `settings.json`.

---

## Built-in Themes

| Theme Name | Folder ID | Style |
|---|---|---|
| **Automata** *(Default)* | `web/themes/nier/` | Inspired by *NieR:Automata* — YoRHa UI with `OVERVIEW`, `CPU`, `MEMORY`, `GPU / VRAM`, and `INTEL` tabs, live mode-aware central capacity stack, 4× 60s oscilloscope waves, and 12 core squares. |
| **Village** | `web/themes/doraemon/` | Inspired by *Doraemon Story of Seasons* — Wooden status board with `Status`, `CPU Crop`, `Memory`, and `GPU / VRAM` folder tabs, stamina bars, cookware core stars, and bell checkerboard parchment. |
| **Classic Dark** | `web/themes/default/` | High-contrast dark telemetry dashboard with 4 sparkline cards, per-core bar strip, and 3 process columns. |

---

## Creating New Themes

- **Manual Implementation:** Read [docs/THEME_GUIDE.md](docs/THEME_GUIDE.md) for a step-by-step tutorial, API reference, and starter template.
- **For AI Agents:** Point your AI coding assistant to [docs/THEME_GUIDE_AI.md](docs/THEME_GUIDE_AI.md) (or [AGENTS.md](AGENTS.md)) for the complete zero-shot specification.

---

## AI Usage Disclosure

This project was pair-programmed and generated with AI assistance:
- **Harness:** Google Antigravity IDE (Windows 11)
- **Installed Plugins:** `superpowers`, `chrome-devtools-plugin`
- **Role:** Architecture design, Rust Win32/NT/DXGI/PDH backend implementation, System Tray integration, unit test suite, and frontend HTML/CSS/JS theme implementation (`Automata`, `Village`, and `Classic Dark`), guided and verified interactively by the repository owner on Windows 11 and iPadOS 27.2.
