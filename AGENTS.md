# AGENTS.md — AI Coding Agent Instructions

This repository contains **Tele-Task-Monitor**, a low-overhead Windows 11 System Tray telemetry server (`src/`) and pluggable multi-theme web dashboard (`web/`) designed for iPad and desktop browsers.

## Quick Orientation

- **Backend (`src/`):** Rust + Axum. A portable core (`server.rs`, `settings.rs`, `config.rs`, `model.rs`, `aggregate.rs`, `viewers.rs`, `lan.rs`, `sampler/`) plus one OS backend per platform under `src/platform/<os>/`. On Windows (Win32/NT/DXGI/PDH, complete) it runs as a windowless System Tray app (`#![cfg_attr(not(test), windows_subsystem = "windows")]`). On macOS (Mach/libproc/IOKit/AppKit, macOS 11+) it runs as a menu-bar app; package it with `scripts/bundle-macos.sh`.
- **Frontend (`web/`):** Vanilla ES modules, HTML, and CSS served directly from `web/` with `Cache-Control: no-cache` (no bundler or build step).
- **Themes (`web/themes/<id>/`):** Each theme is self-contained (`template.html`, `style.css`, `theme.js`) and registered in `web/themes/manifest.json`.

## Cross-Platform Rules (Backend)

1. **No OS APIs outside `src/platform/`.** Never `use windows::…` (or macOS FFI) in the portable core. If the core needs something new from the OS, add it to the contract in `src/platform/mod.rs`: a trait method, or a free function listed in its `const _` signature check. Then implement it in **every** backend.
2. **Put pure logic in the core.** Math and data shaping that doesn't call the OS (e.g. `sampler/cpu.rs`, `aggregate.rs`) belongs in the portable core so it is shared and unit-tested everywhere.
3. **Target-specific crates** go under `[target.'cfg(windows)'.dependencies]` or `[target.'cfg(target_os = "macos")'.dependencies]` in `Cargo.toml`.
4. **Type-check both backends** after any Rust change, from any OS:
   ```bash
   cargo check --all-targets --target x86_64-pc-windows-msvc
   cargo check --all-targets --target aarch64-apple-darwin
   ```
5. Keep the macOS backend free of subprocesses on the 1 Hz sampling path: shelling out is only acceptable for rare user actions (open URL, copy, alerts).

## Creating or Modifying Themes

Read **[docs/THEME_GUIDE_AI.md](docs/THEME_GUIDE_AI.md)** before creating or editing any theme. It specifies:
1. The exact `mount(root, host)` lifecycle contract and `host` API (`drawGraph`, `fmtBytes`, `openThemeMenu`, `openSettings`, `getState`, `getSettings`).
2. Strict viewport and iPadOS 27.2 standalone PWA invariants (`height: 100%`, `overflow: hidden`, `var(--top-safe-pad)`, `bgColor` registration, non-scrolling process grids).
3. Complete TypeScript/JSON schemas for `Info`, `Sample`, and `UserSettings`.

## Rebuilding the Standalone Executable

Whenever you modify Rust code in `src/` (Windows):
```powershell
Stop-Process -Name "tele-task-monitor" -Force -ErrorAction SilentlyContinue
cargo test
cargo build --release
Copy-Item target\release\tele-task-monitor.exe .\tele-task-monitor.exe -Force
Start-Process -FilePath ".\tele-task-monitor.exe"
```
On macOS: `cargo test`, then `scripts/bundle-macos.sh` and `open dist/Tele-Task-Monitor.app` (quit the running copy from its menu-bar item first).

*(Edits under `web/` do not require rebuilding `tele-task-monitor.exe`—simply refresh the browser.)*
