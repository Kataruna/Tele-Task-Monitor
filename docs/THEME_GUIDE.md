# Theme Creation Guide (Manual Implementation)

Tele-Task-Monitor uses a **pluggable full-layout theme architecture**. Unlike simple color-variable swaps, each theme owns its **entire HTML structure, CSS stylesheet, and JavaScript rendering lifecycle**.

Because the Rust server serves `web/` directly from disk with `Cache-Control: no-cache`, you can create and edit themes live—**just refresh your browser (`F5`) to see changes immediately without rebuilding the `.exe`**.

---

## 1. Anatomy of a Theme

Every theme lives in its own folder under `web/themes/<theme-id>/` with 3 files:

```text
web/themes/<theme-id>/
├── template.html   # Injected into <main id="theme-root"> when your theme loads
├── style.css       # Loaded into <link id="theme-css"> before template.html mounts
└── theme.js        # ES module exporting `mount(root, host)`
```

And is registered in `web/themes/manifest.json`:

```json
{
  "defaultTheme": "nier",
  "themes": [
    {
      "id": "nier",
      "name": "Automata",
      "bgColor": "#cdc8b0"
    },
    {
      "id": "doraemon",
      "name": "Village",
      "bgColor": "#5a7d3e"
    },
    {
      "id": "default",
      "name": "Classic Dark",
      "bgColor": "#0b0d12"
    },
    {
      "id": "my-theme",
      "name": "My Custom Theme",
      "bgColor": "#141824"
    }
  ]
}
```

> **Why `bgColor` matters:** The host uses `bgColor` to paint `<html>`, `<body>`, `<meta name="theme-color">`, and the iPadOS top/bottom safe-area plates so there are never white edges on iPad Home Screen web apps.

---

## 2. The `theme.js` Lifecycle Contract

Your `theme.js` file is a standard ES module that exports a single `mount(root, host)` function:

```js
export function mount(root, host) {
  // 1. Query your DOM elements inside `root`
  // 2. Wire up click listeners (tabs, theme menu button, settings button)

  return {
    // Called when the theme mounts and whenever user saves Settings
    onSettings(settings) {
      // settings = { port, defaultTheme, overviewLimit, pageLimit }
    },

    // Called once on initial SSE connection (and if the server restarts)
    onInfo(info) {
      // Hardware identity: hostname, CPU name & core count, total RAM, GPU name & VRAM
    },

    // Called whenever connection state changes ("connecting" | "live" | "down")
    onStatus(status) {
      // status = { state: "live", text: "Live" }
    },

    // Called every 1 second with live telemetry
    onSample(sample) {
      // Update text, bars, process lists, and graphs
    },

    // Called right before switching away to another theme
    unmount() {
      // Remove any window event listeners (resize, keydown) or timers
    },
  };
}
```

---

## 3. The `host` API Reference

The second argument passed to `mount(root, host)` provides shared utilities and state so your theme doesn't have to manage network connections or graph buffers:

| Method / Property | Description |
|---|---|
| `host.getState()` | Returns `{ info, sample, status, history: { cpu, ram, gpu, vram }, themeId, settings, themes }`. `history.*` arrays hold up to 60 seconds of `0..100` values (plus `null` gap markers). |
| `host.getSettings()` | Returns `{ port, defaultTheme, overviewLimit, pageLimit }`. |
| `host.fmtBytes(bytes)` | Formats a byte count into `"1.6 GB"` (`>= 1 GiB`) or `"420 MB"` (`< 1 GiB`). |
| `host.drawGraph(canvas, points, strokeColor, opts)` | High-DPI 2D canvas sparkline renderer. Draws `points` (`0..100` array) across a 60-second x-axis. Optional `opts`: `{ lineWidth: 2, fillColor: "rgba(...)", gridColor: "rgba(...)", gridDash: [3, 3] }`. |
| `host.prepareList(entries, metric)` | Filters hidden processes, marks pinned ones with `_pinned`, and sorts pinned to the top. `metric` is `"cpu"`, `"mem"` or `"gpu"` (GPU order follows `host.getGpuSort()`). |
| `host.getFocusedProcess()` / `host.setFocusedProcess(name)` | The selected process, tracked by **name** so it follows the process when ranks change. |
| `host.reconcileFocus(lists)` | Call once per render with the prepared lists on screen: if the focused process is in none of them, focus moves to the top entry of the first non-empty list (a deselected, `null` focus stays `null`). |
| `host.bindProcessRow(rowEl, () => name)` | Makes a row open the pin / hide menu on press-and-hold or right-click. Call it **before** adding the row's own `click` listener. |
| `host.getGpuSort()` / `host.toggleGpuSort()` | `"usage"` or `"vram"`: what the GPU list is ranked by first. |
| `host.getSecondaryMetrics()` | `true` when the user wants the second metric (RAM beside CPU, CPU beside RAM) in detail lists. |
| `host.openThemeMenu(buttonEl, clickEvent)` | Opens the shared **Select Theme** dropdown anchored to `buttonEl` (includes all themes + `⚙ Settings…`). |
| `host.openSettings(clickEvent)` | Opens the **System Settings** modal directly. |
| `host.switchTheme(themeId?)` | Switches to `themeId` (or cycles to the next theme if omitted). |

---

## 4. Telemetry Data Payloads

### `info` (passed to `onInfo(info)`)
```json
{
  "v": 1,
  "hostname": "Kataru",
  "cpu": {
    "name": "AMD Ryzen 5 3600 6-Core Processor",
    "logicalCores": 12
  },
  "ram": {
    "total": 34281504768
  },
  "gpu": {
    "name": "NVIDIA GeForce RTX 5070",
    "vramTotal": 12884901888
  },
  "intervalMs": 1000
}
```
*(Note: `info.gpu` may be `null` on machines without a supported GPU.)*

### `sample` (passed to `onSample(sample)` at 1 Hz)
```json
{
  "ts": 1730370000000,
  "cpu": {
    "usage": 35.2,
    "cores": [42.0, 18.5, 65.1, 12.0, 50.4, 22.1, 38.0, 14.2, 41.0, 19.8, 55.0, 44.3]
  },
  "ram": {
    "used": 19327352832,
    "total": 34281504768
  },
  "gpu": {
    "usage": 14.5,
    "vramUsed": 3758096384,
    "vramTotal": 12884901888,
    "tempC": 46.0,
    "powerPct": 28.0
  },
  "top": {
    "cpu": [
      { "name": "Game.exe", "count": 1, "cpu": 24.5, "mem": 4294967296, "gpu": 12.1, "vram": 2147483648 }
    ],
    "mem": [ ... ],
    "gpu": [ ... ]
  },
  "self": {
    "cpu": 0.0,
    "mem": 7340032
  }
}
```
*(Note: `sample.gpu` and `sample.top.gpu` may be `null` if GPU counters are unavailable; `tempC` and `powerPct` may be `null` on non-NVIDIA/WDDM adapters.)*

---

## 5. Layout & iPadOS Best Practices

1. **Fill the Fixed Viewport Without Scrolling:**
   - The host locks `body` and `#theme-root` to `position: fixed; inset: 0; width: 100%; height: 100%; overflow: hidden;`.
   - Your theme's root container should use `width: 100%; height: 100%; overflow: hidden;` and CSS Grid / Flexbox (`min-height: 0` on flex/grid children) so the page never scrolls on iPad.
2. **Respect iPadOS Standalone Safe-Area Padding (`--top-safe-pad`):**
   - Use `var(--top-safe-pad, 10px)` for your root container's `padding-top`:
     ```css
     .my-theme-shell {
       width: 100%;
       height: 100%;
       overflow: hidden;
       padding: max(10px, var(--top-safe-pad, 10px))
                max(16px, env(safe-area-inset-right, 0px))
                max(10px, env(safe-area-inset-bottom, 0px))
                max(16px, env(safe-area-inset-left, 0px));
     }
     ```
   - This automatically clears iPadOS 27.2's `48px` top status-bar blur band when installed as a Home Screen web app while keeping tight `10px` padding on desktop browsers.
3. **Reuse DOM Nodes in `onSample()`:**
   - `onSample()` runs every second. Pre-create your `<li>` process rows when mounting (or inside `onSettings`) and update `.textContent` / `.style.width` inside `onSample()` rather than rebuilding `innerHTML` every second.

---

## 6. Minimal Starter Theme

To create a new theme called `cyber`:

### Step 1: Create `web/themes/cyber/template.html`
```html
<div class="cyber-shell">
  <header class="cyber-header">
    <div>
      <strong id="cy-host">—</strong>
      <span id="cy-cpu-name"></span>
    </div>
    <div class="cyber-actions">
      <span id="cy-self">0.0% CPU</span>
      <button type="button" id="cy-settings-btn">⚙ Settings</button>
      <button type="button" id="cy-theme-btn">Theme ▾</button>
    </div>
  </header>

  <section class="cyber-metrics">
    <div class="cyber-card">
      <h3>CPU <span id="cy-cpu-val">0%</span></h3>
      <canvas id="cy-cpu-graph"></canvas>
    </div>
    <div class="cyber-card">
      <h3>RAM <span id="cy-ram-val">0 GB</span></h3>
      <canvas id="cy-ram-graph"></canvas>
    </div>
    <div class="cyber-card">
      <h3>GPU <span id="cy-gpu-val">0%</span></h3>
      <canvas id="cy-gpu-graph"></canvas>
    </div>
  </section>

  <section class="cyber-lists">
    <div class="cyber-card"><ol id="cy-list-cpu"></ol></div>
    <div class="cyber-card"><ol id="cy-list-mem"></ol></div>
    <div class="cyber-card"><ol id="cy-list-gpu"></ol></div>
  </section>
</div>
```

### Step 2: Create `web/themes/cyber/style.css`
```css
.cyber-shell {
  width: 100%;
  height: 100%;
  overflow: hidden;
  background: #0d0f18;
  color: #00f0ff;
  font-family: Consolas, monospace;
  display: grid;
  grid-template-rows: auto 150px minmax(0, 1fr);
  gap: 10px;
  padding: max(10px, var(--top-safe-pad, 10px)) 16px max(10px, env(safe-area-inset-bottom, 0px)) 16px;
}
.cyber-header { display: flex; justify-content: space-between; align-items: center; }
.cyber-metrics, .cyber-lists { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 10px; min-height: 0; }
.cyber-card { border: 1px solid #00f0ff44; background: #131726; padding: 8px; display: flex; flex-direction: column; min-height: 0; }
.cyber-card canvas { flex: 1; width: 100%; min-height: 0; }
.cyber-card ol { list-style: none; margin: 0; padding: 0; overflow: hidden; display: grid; gap: 2px; flex: 1; }
.cyber-card li { display: flex; justify-content: space-between; font-size: 12px; }
```

### Step 3: Create `web/themes/cyber/theme.js`
```js
export function mount(root, host) {
  const $ = (id) => root.querySelector(`#${id}`);
  let pageLimit = host.getSettings().pageLimit || 24;

  $("cy-theme-btn").addEventListener("click", (ev) => host.openThemeMenu($("cy-theme-btn"), ev));
  $("cy-settings-btn").addEventListener("click", (ev) => host.openSettings(ev));

  function fillList(ol, entries, fmt) {
    ol.style.gridTemplateRows = `repeat(${pageLimit}, minmax(0, 1fr))`;
    ol.textContent = "";
    for (let i = 0; i < pageLimit && entries && i < entries.length; i++) {
      const li = document.createElement("li");
      li.innerHTML = `<span>${entries[i].name}</span><strong>${fmt(entries[i])}</strong>`;
      ol.appendChild(li);
    }
  }

  return {
    onSettings(s) {
      pageLimit = s.pageLimit || 24;
    },
    onInfo(info) {
      $("cy-host").textContent = info.hostname;
      $("cy-cpu-name").textContent = info.cpu.name;
    },
    onSample(s) {
      $("cy-cpu-val").textContent = `${s.cpu.usage.toFixed(1)}%`;
      $("cy-ram-val").textContent = host.fmtBytes(s.ram.used);
      $("cy-gpu-val").textContent = s.gpu ? `${s.gpu.usage.toFixed(1)}%` : "—";
      $("cy-self").textContent = `${s.self.cpu.toFixed(1)}% CPU`;

      const h = host.getState().history;
      host.drawGraph($("cy-cpu-graph"), h.cpu, "#00f0ff");
      host.drawGraph($("cy-ram-graph"), h.ram, "#ff00a0");
      host.drawGraph($("cy-gpu-graph"), h.gpu, "#00ff88");

      fillList($("cy-list-cpu"), s.top.cpu, (e) => `${e.cpu.toFixed(1)}%`);
      fillList($("cy-list-mem"), s.top.mem, (e) => host.fmtBytes(e.mem));
      fillList($("cy-list-gpu"), s.top.gpu || [], (e) => `${(e.gpu ?? 0).toFixed(1)}%`);
    },
  };
}
```

### Step 4: Register in `web/themes/manifest.json`
Add `{ "id": "cyber", "name": "Cyber", "bgColor": "#0d0f18" }` to `web/themes/manifest.json` and select it from the **`Theme ▾`** menu!
