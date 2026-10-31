# AGENTS.md — AI Coding Agent Instructions

This repository contains **Tele-Task-Monitor**, a low-overhead Windows 11 System Tray telemetry server (`src/`) and pluggable multi-theme web dashboard (`web/`) designed for iPad and desktop browsers.

## Quick Orientation

- **Backend (`src/`):** Rust + Axum + Win32/NT/DXGI/PDH APIs. Runs as a windowless Windows System Tray app (`#![cfg_attr(not(test), windows_subsystem = "windows")]`).
- **Frontend (`web/`):** Vanilla ES modules, HTML, and CSS served directly from `web/` with `Cache-Control: no-cache` (no bundler or build step).
- **Themes (`web/themes/<id>/`):** Each theme is self-contained (`template.html`, `style.css`, `theme.js`) and registered in `web/themes/manifest.json`.

## Creating or Modifying Themes

Read **[docs/THEME_GUIDE_AI.md](docs/THEME_GUIDE_AI.md)** before creating or editing any theme. It specifies:
1. The exact `mount(root, host)` lifecycle contract and `host` API (`drawGraph`, `fmtBytes`, `openThemeMenu`, `openSettings`, `getState`, `getSettings`).
2. Strict viewport and iPadOS 27.2 standalone PWA invariants (`height: 100%`, `overflow: hidden`, `var(--top-safe-pad)`, `bgColor` registration, non-scrolling process grids).
3. Complete TypeScript/JSON schemas for `Info`, `Sample`, and `UserSettings`.

## Rebuilding the Standalone Executable

Whenever you modify Rust code in `src/`:
```powershell
Stop-Process -Name "tele-task-monitor" -Force -ErrorAction SilentlyContinue
cargo test
cargo build --release
Copy-Item target\release\tele-task-monitor.exe .\tele-task-monitor.exe -Force
Start-Process -FilePath ".\tele-task-monitor.exe"
```
*(Edits under `web/` do not require rebuilding `tele-task-monitor.exe`—simply refresh the browser.)*
