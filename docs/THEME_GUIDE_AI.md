# AI Agent Theme Specification (`Tele-Task-Monitor`)

> **Target Audience:** Autonomous AI Coding Agents (Claude Code, Antigravity, Cursor, Codex, etc.).
> **Goal:** Create or modify full-layout themes in `web/themes/<theme-id>/` in a single pass with zero regressions, zero viewport scrolling, and full iPadOS 27.2 PWA compatibility.

---

## 1. Architecture & File Contract

- **No Frontend Build Step:** Files in `web/` are served directly by Axum (`src/server.rs`) with `Cache-Control: no-cache`. Do NOT introduce bundlers, npm packages, or external CDN dependencies. Use vanilla ES modules, inline SVG, and CSS.
- **Required Files per Theme (`web/themes/<id>/`):**
  1. `web/themes/<id>/template.html` — HTML fragment injected into `<main id="theme-root" class="theme-root"></main>`. Do NOT include `<html>`, `<head>`, or `<body>` tags.
  2. `web/themes/<id>/style.css` — Theme stylesheet loaded into `<link id="theme-css">`. Scope all selectors under your theme's root class (e.g. `.mytheme-screen ...`) so rules never leak into the host dropdown (`#theme-dropdown`) or Settings modal (`#settings-overlay`).
  3. `web/themes/<id>/theme.js` — ES module exporting `export function mount(root, host) { ... }`.
- **Theme Registration (`web/themes/manifest.json`):**
  Append `{ "id": "<id>", "name": "<Display Name>", "bgColor": "#rrggbb" }` to the `themes` array in `web/themes/manifest.json`.
  Both the **`Theme ▾`** dropdown menu and the **System Settings → Default Theme** selector populate dynamically from `web/themes/manifest.json`—no Rust rebuild or HTML edits are required when adding a new theme.

---

## 2. Strict Viewport & iPadOS 27.2 Invariants (CRITICAL)

Every theme MUST obey these 6 layout invariants:

1. **Zero Page Scrolling (`height: 100%`, `overflow: hidden`):**
   - `body` and `#theme-root` are `position: fixed; inset: 0; width: 100%; height: 100%; overflow: hidden;`.
   - Your theme root element in `template.html` MUST have `width: 100%; height: 100%; overflow: hidden;` (never `100dvh`, which leaves a 20px gap above `safe-area-inset-bottom` in iPadOS standalone mode).
   - Every flex/grid container holding process lists or canvases MUST have `min-height: 0; min-width: 0; overflow: hidden;`.
2. **iPadOS 27.2 Top Progressive Blur Clearance (`var(--top-safe-pad)`):**
   - iPadOS 27.2 applies a forced system progressive blur across the top `~46px` of standalone Home Screen web apps.
   - `web/style.css` defines `--top-safe-pad` (`48px` in standalone mode, `0px` in desktop browsers).
   - Your theme root container MUST use `var(--top-safe-pad)` for top padding:
     ```css
     padding: max(10px, var(--top-safe-pad, 10px))
              max(16px, env(safe-area-inset-right, 0px))
              max(8px, env(safe-area-inset-bottom, 0px))
              max(16px, env(safe-area-inset-left, 0px));
     ```
   - And your theme's background (`background-color` / `background-image`) MUST be applied to that root container so the background extends edge-to-edge behind the top padding area.
3. **Dynamic Process Row Limits (`overviewLimit` & `pageLimit`) + Pin / Hide / Focus / Scrollable Lists:**
   - Read initial limits via `host.getSettings()` (`{ overviewLimit: 8, pageLimit: 24, pinnedProcesses: [], hiddenProcesses: [] }`) and implement `onSettings(settings)` in the object returned by `mount(root, host)`.
   - Pass each list through `host.prepareList(entries, metric)` (`"cpu" | "mem" | "gpu"`), which filters out hidden processes, marks pinned processes with `e._pinned = true`, and sorts pinned processes to the top.
   - Render a clear visual separation line (`.pinned-separator`) after the last pinned process when a list contains both pinned and normal processes.
   - Track focused process by **process name** (`host.getFocusedProcess()` / `host.setFocusedProcess(name)`), NOT by static row index, so selection follows the process when its rank changes.
   - Make `<ol>` process lists vertically scrollable (`overflow-y: auto; overscroll-behavior: contain; -webkit-overflow-scrolling: touch; touch-action: pan-y;`) with `flex-shrink: 0` rows so users can scroll through lists when items exceed the box height while the outer page (`html, body`) stays locked.
4. **Navigation & Back-to-Overview Controls:**
   - If your theme has multiple tabs (`Overview` + detail tabs for `CPU`, `Memory`, `GPU`), clicking summary zones on `Overview` should drill down to that tab, and every detail tab MUST show a visible `Back to Overview` button (plus `Escape` / `Backspace` key handler).
5. **Theme Dropdown & Settings Triggers:**
   - Every theme MUST include a button wired to `host.openThemeMenu(btnEl, ev)` (`Theme ▾`) and a button/tab wired to `host.openSettings(ev)` (`⚙ Settings`).
6. **1 Hz DOM Reuse (Performance Budget):**
   - `onSample(sample)` is called every 1000 ms. Pre-allocate DOM nodes for process rows and core bars during `mount()` / `onSettings()` / `onInfo()`, and mutate `.textContent`, `.hidden`, and `.style` in `onSample()`. Never recreate `<canvas>` elements on tick.

---

## 3. Exact `host` API & Data Schemas

### `host` Object Passed to `mount(root, host)`
```ts
interface HostApi {
  HISTORY_LEN: 60;
  fmtBytes(bytes: number | null | undefined): string; // e.g. "1.6 GB" or "420 MB"
  drawGraph(
    canvas: HTMLCanvasElement | null,
    points: Array<number | null>, // length <= 60, values 0..100
    strokeColor: string,
    opts?: {
      lineWidth?: number;
      fillColor?: string;
      gridColor?: string;
      gridDash?: number[];
    }
  ): void;
  getState(): {
    info: InfoPayload | null;
    sample: SamplePayload | null;
    status: { state: "connecting" | "live" | "down"; text: string };
    history: {
      cpu: Array<number | null>;
      ram: Array<number | null>;
      gpu: Array<number | null>;
      vram: Array<number | null>;
    };
    themeId: string;
    settings: UserSettings;
    themes: Array<{ id: string; name: string; bgColor: string }>;
  };
  getSettings(): UserSettings;
  isPinned(name: string): boolean;
  isHidden(name: string): boolean;
  togglePin(name: string): void;
  hideProcess(name: string): void;
  unhideProcess(name: string): void;
  prepareList(entries: AppEntry[], metric?: "cpu" | "mem" | "gpu", sortOverride?: "usage" | "vram"): Array<AppEntry & { _pinned: boolean }>;
  getFocusedProcess(): string | null;
  setFocusedProcess(name: string | null): void;
  getGpuSort(): "usage" | "vram";
  setGpuSort(sort: "usage" | "vram"): void;
  toggleGpuSort(): "usage" | "vram";
  getSecondaryMetrics(): boolean;
  bindProcessRow(el: HTMLElement, getProcessName: () => string | string): void;
  showProcessMenu(x: number, y: number, name: string): void;
  openThemeMenu(anchorEl: HTMLElement, ev?: MouseEvent): void;
  openSettings(ev?: MouseEvent): void;
  switchTheme(nextId?: string): void;
}
```

### Data Schemas
```ts
interface UserSettings {
  port: number;                  // default 8800
  defaultTheme: string;          // "nier" | "doraemon" | "default" | ...
  overviewLimit: number;         // default 8 (1..25)
  pageLimit: number;             // default 24 (1..60)
  pinnedProcesses: string[];     // pinned to top of lists above separator
  hiddenProcesses: string[];     // excluded from lists
  gpuSort: "usage" | "vram";     // default "usage"
  showSecondaryMetrics: boolean; // default false
}

interface InfoPayload {
  v: 1;
  hostname: string; // Windows DNS hostname, e.g. "Kataru"
  cpu: { name: string; logicalCores: number };
  ram: { total: number }; // bytes
  gpu: { name: string; vramTotal: number } | null;
  intervalMs: number; // 1000
}

interface AppEntry {
  name: string;        // e.g. "Rainmeter.exe"
  count: number;       // grouped process instance count (>= 1)
  cpu: number;         // 0..100 (1 decimal)
  mem: number;         // private working set in bytes
  gpu: number | null;  // 0..100 (1 decimal) or null
  vram: number | null; // dedicated VRAM in bytes or null
}

interface SamplePayload {
  ts: number; // unix epoch ms
  cpu: {
    usage: number;   // 0..100
    cores: number[]; // per-logical-core 0..100
  };
  ram: {
    used: number;  // bytes
    total: number; // bytes
  };
  gpu: {
    usage: number;           // 0..100
    vramUsed: number;        // bytes
    vramTotal: number;       // bytes
    tempC: number | null;    // °C
    powerPct: number | null; // % TDP
  } | null;
  top: {
    cpu: AppEntry[];        // sorted desc by cpu
    mem: AppEntry[];        // sorted desc by mem
    gpu: AppEntry[] | null; // sorted desc by gpu, tie-broken by vram
  };
  self: {
    cpu: number; // tele-task-monitor.exe own CPU %
    mem: number; // tele-task-monitor.exe own private working set bytes
  };
}
```

---

## 4. Reference Implementations in Repo

Before writing a new theme, inspect the existing themes for patterns:
- **Complex Multi-Tab Game UI with Central Stack & 4 Oscilloscopes:** `web/themes/nier/` (`Automata`)
- **Illustrated Tabbed Board with Custom SVG Art:** `web/themes/doraemon/` (`Village`)
- **Minimal Single-Screen Dashboard:** `web/themes/default/` (`Classic Dark`)

---

## 5. AI Agent Verification Checklist

Before reporting completion to the user:
1. Verify `web/themes/<id>/template.html`, `style.css`, and `theme.js` exist and use no external network assets.
2. Verify `web/themes/manifest.json` includes `{ "id": "<id>", "name": "<Name>", "bgColor": "<#hex>" }`.
3. If backend files (`src/*.rs`) were modified:
   - Stop any running instance: `Stop-Process -Name "tele-task-monitor" -Force -ErrorAction SilentlyContinue`
   - Run `cargo test --quiet` and confirm all tests pass.
   - Run `cargo build --release --quiet`
   - Copy `target\release\tele-task-monitor.exe` to `.\tele-task-monitor.exe` and relaunch with `Start-Process -FilePath ".\tele-task-monitor.exe"`.
