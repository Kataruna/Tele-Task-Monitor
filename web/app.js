// Tele-Task-Monitor — Theme Host & Telemetry Store
// Maintains one SSE connection + 60s rolling history across theme & tab switches.

const HISTORY_LEN = 60;
const STORAGE_KEY = "ttm.theme";

const state = {
  info: null,
  sample: null,
  status: { state: "connecting", text: "connecting…" },
  history: {
    cpu: [],
    ram: [],
    gpu: [],
    vram: [],
  },
  themeId: "nier",
  settings: {
    port: Number(window.location.port) || 8800,
    defaultTheme: "nier",
    overviewLimit: 8,
    pageLimit: 24,
  },
  themes: [
    { id: "nier", name: "Automata", bgColor: "#cdc8b0" },
    { id: "doraemon", name: "Village", bgColor: "#5a7d3e" },
    { id: "default", name: "Classic Dark", bgColor: "#0b0d12" },
  ],
};

const THEME_BG_FALLBACKS = {
  nier: "#cdc8b0",
  doraemon: "#5a7d3e",
  default: "#0b0d12",
};

let activeThemeInstance = null;
let loadToken = 0;

const rootEl = document.getElementById("theme-root");
const cssLink = document.getElementById("theme-css");
const bannerEl = document.getElementById("banner");
const dropdownEl = document.getElementById("theme-dropdown");
const metaThemeColor = document.getElementById("meta-theme-color");
const safeTopPlate = document.getElementById("safe-top-plate");
const safeBottomPlate = document.getElementById("safe-bottom-plate");

if (
  window.navigator.standalone === true ||
  window.matchMedia("(display-mode: standalone)").matches ||
  window.matchMedia("(display-mode: fullscreen)").matches
) {
  document.documentElement.classList.add("is-standalone");
}

function applyThemeBackgroundColor(themeId) {
  const entry = state.themes.find((t) => t.id === themeId);
  const bg = (entry && entry.bgColor) || THEME_BG_FALLBACKS[themeId] || "#cdc8b0";
  document.documentElement.style.setProperty("--app-bg", bg);
  document.documentElement.style.backgroundColor = bg;
  if (document.body) document.body.style.backgroundColor = bg;
  if (rootEl) rootEl.style.backgroundColor = bg;
  if (safeTopPlate) safeTopPlate.style.backgroundColor = bg;
  if (safeBottomPlate) safeBottomPlate.style.backgroundColor = bg;
  if (metaThemeColor) metaThemeColor.setAttribute("content", bg);
}

function pushHistory(arr, v) {
  arr.push(v);
  while (arr.length > HISTORY_LEN) arr.shift();
}

export function fmtBytes(n) {
  if (n == null || n < 0) return "—";
  const gb = n / 1073741824;
  if (gb >= 1) return `${gb.toFixed(1)} GB`;
  return `${Math.round(n / 1048576)} MB`;
}

/**
 * DPR-aware 60-second graph renderer usable by any theme.
 * `points` may contain numbers (0..100) and `null` gap markers.
 */
export function drawGraph(canvas, points, strokeColor, opts = {}) {
  if (!canvas) return;
  const dpr = window.devicePixelRatio || 1;
  const w = canvas.clientWidth;
  const h = canvas.clientHeight;
  if (w === 0 || h === 0) return;
  if (canvas.width !== Math.round(w * dpr) || canvas.height !== Math.round(h * dpr)) {
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(h * dpr);
  }
  const ctx = canvas.getContext("2d");
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, w, h);

  if (opts.gridColor) {
    ctx.strokeStyle = opts.gridColor;
    ctx.lineWidth = 1;
    ctx.setLineDash(opts.gridDash || [3, 3]);
    for (const frac of [0.25, 0.5, 0.75]) {
      const y = Math.round(h * frac) + 0.5;
      ctx.beginPath();
      ctx.moveTo(0, y);
      ctx.lineTo(w, y);
      ctx.stroke();
    }
    ctx.setLineDash([]);
  }

  if (!points || points.length === 0) return;

  const step = w / Math.max(1, HISTORY_LEN - 1);
  const startIdx = HISTORY_LEN - points.length;
  const yOf = (v) => h - 3 - (Math.max(0, Math.min(100, v)) / 100) * (h - 6);

  if (opts.fillColor) {
    ctx.fillStyle = opts.fillColor;
    let segStart = null;
    for (let i = 0; i <= points.length; i++) {
      const v = i < points.length ? points[i] : null;
      const x = (startIdx + i) * step;
      if (v != null) {
        if (segStart === null) {
          segStart = x;
          ctx.beginPath();
          ctx.moveTo(x, h);
        }
        ctx.lineTo(x, yOf(v));
      } else if (segStart !== null) {
        const prevX = (startIdx + i - 1) * step;
        ctx.lineTo(prevX, h);
        ctx.closePath();
        ctx.fill();
        segStart = null;
      }
    }
  }

  ctx.strokeStyle = strokeColor;
  ctx.lineWidth = opts.lineWidth || 2;
  ctx.lineJoin = "round";
  ctx.lineCap = "round";
  ctx.beginPath();
  let penDown = false;
  for (let i = 0; i < points.length; i++) {
    const v = points[i];
    if (v == null) {
      penDown = false;
      continue;
    }
    const x = (startIdx + i) * step;
    const y = yOf(v);
    if (!penDown) {
      ctx.moveTo(x, y);
      penDown = true;
    } else {
      ctx.lineTo(x, y);
    }
  }
  ctx.stroke();
}

function closeThemeDropdown() {
  if (dropdownEl) dropdownEl.hidden = true;
}

function toggleThemeDropdown(anchorEl) {
  if (!dropdownEl) return;
  if (!dropdownEl.hidden) {
    closeThemeDropdown();
    return;
  }

  dropdownEl.textContent = "";
  const title = document.createElement("div");
  title.className = "dd-title";
  title.textContent = "Select Theme";
  dropdownEl.appendChild(title);

  for (const t of state.themes) {
    const btn = document.createElement("button");
    btn.type = "button";
    const isAct = t.id === state.themeId;
    btn.className = `dd-item${isAct ? " active" : ""}`;
    const label = document.createElement("span");
    label.textContent = t.name;
    const check = document.createElement("span");
    check.className = "dd-check";
    check.textContent = isAct ? "◆" : "";
    btn.append(label, check);
    btn.addEventListener("click", (ev) => {
      ev.stopPropagation();
      closeThemeDropdown();
      if (t.id !== state.themeId) {
        loadTheme(t.id);
      }
    });
    dropdownEl.appendChild(btn);
  }

  const sep = document.createElement("div");
  sep.className = "dd-sep";
  dropdownEl.appendChild(sep);

  const setBtn = document.createElement("button");
  setBtn.type = "button";
  setBtn.className = "dd-item";
  const setLbl = document.createElement("span");
  setLbl.textContent = "⚙ Settings…";
  setBtn.appendChild(setLbl);
  setBtn.addEventListener("click", (ev) => {
    ev.stopPropagation();
    closeThemeDropdown();
    openSettingsModal();
  });
  dropdownEl.appendChild(setBtn);

  dropdownEl.style.left = "0px";
  dropdownEl.style.top = "0px";
  dropdownEl.style.bottom = "auto";
  dropdownEl.style.right = "auto";
  dropdownEl.hidden = false;

  if (anchorEl) {
    const rect = anchorEl.getBoundingClientRect();
    const ddRect = dropdownEl.getBoundingClientRect();
    const vw = window.innerWidth;
    const vh = window.innerHeight;

    let left = rect.right - ddRect.width;
    if (left < 10) left = rect.left;
    if (left + ddRect.width > vw - 10) left = vw - ddRect.width - 10;

    let top = rect.top - ddRect.height - 8;
    if (top < 10) {
      top = rect.bottom + 8;
    }
    if (top + ddRect.height > vh - 10) {
      top = vh - ddRect.height - 10;
    }

    dropdownEl.style.left = `${Math.max(8, left)}px`;
    dropdownEl.style.top = `${Math.max(8, top)}px`;
  }
}

window.addEventListener("click", (ev) => {
  if (!dropdownEl || dropdownEl.hidden) return;
  if (!dropdownEl.contains(ev.target)) {
    closeThemeDropdown();
  }
});

const settingsOverlay = document.getElementById("settings-overlay");
const settingsForm = document.getElementById("settings-form");
const setThemeEl = document.getElementById("set-default-theme");
const setOvLimitEl = document.getElementById("set-overview-limit");
const setPageLimitEl = document.getElementById("set-page-limit");
const setPortEl = document.getElementById("set-port");
const settingsStatusEl = document.getElementById("settings-status");
const settingsCloseBtn = document.getElementById("settings-close-btn");
const settingsCancelBtn = document.getElementById("settings-cancel-btn");
const settingsResetBtn = document.getElementById("settings-reset-btn");

function showSettingsStatus(msg, isError = false) {
  if (!settingsStatusEl) return;
  if (!msg) {
    settingsStatusEl.hidden = true;
    settingsStatusEl.textContent = "";
    settingsStatusEl.classList.remove("error");
    return;
  }
  settingsStatusEl.hidden = false;
  settingsStatusEl.textContent = msg;
  settingsStatusEl.classList.toggle("error", isError);
}

function openSettingsModal() {
  closeThemeDropdown();
  if (!settingsOverlay) return;
  if (setThemeEl) {
    setThemeEl.textContent = "";
    for (const t of state.themes) {
      const opt = document.createElement("option");
      opt.value = t.id;
      opt.textContent = t.name;
      setThemeEl.appendChild(opt);
    }
    setThemeEl.value = state.settings.defaultTheme || state.themeId || "nier";
  }
  if (setOvLimitEl) setOvLimitEl.value = String(state.settings.overviewLimit || 8);
  if (setPageLimitEl) setPageLimitEl.value = String(state.settings.pageLimit || 24);
  if (setPortEl) setPortEl.value = String(state.settings.port || 8800);
  showSettingsStatus("");
  settingsOverlay.hidden = false;
}

function closeSettingsModal() {
  if (settingsOverlay) settingsOverlay.hidden = true;
}

if (settingsCloseBtn) settingsCloseBtn.addEventListener("click", closeSettingsModal);
if (settingsCancelBtn) settingsCancelBtn.addEventListener("click", closeSettingsModal);
if (settingsOverlay) {
  settingsOverlay.addEventListener("click", (ev) => {
    if (ev.target === settingsOverlay) closeSettingsModal();
  });
}

for (const btn of document.querySelectorAll(".step-btn")) {
  btn.addEventListener("click", () => {
    const input = document.getElementById(btn.dataset.stepTarget);
    if (!input) return;
    const step = Number(btn.dataset.step) || 1;
    const min = Number(input.min) || 1;
    const max = Number(input.max) || 60;
    const cur = Number(input.value) || min;
    input.value = String(Math.max(min, Math.min(max, cur + step)));
  });
}

if (settingsResetBtn) {
  settingsResetBtn.addEventListener("click", () => {
    if (setThemeEl) setThemeEl.value = "nier";
    if (setOvLimitEl) setOvLimitEl.value = "8";
    if (setPageLimitEl) setPageLimitEl.value = "24";
    if (setPortEl) setPortEl.value = "8800";
    showSettingsStatus("Defaults restored — click Save & Apply to confirm.");
  });
}

if (settingsForm) {
  settingsForm.addEventListener("submit", async (ev) => {
    ev.preventDefault();
    const prevPort = state.settings.port;
    const payload = {
      port: Math.max(1024, Math.min(65535, Number(setPortEl.value) || 8800)),
      defaultTheme: setThemeEl.value || "nier",
      overviewLimit: Math.max(1, Math.min(25, Number(setOvLimitEl.value) || 8)),
      pageLimit: Math.max(1, Math.min(60, Number(setPageLimitEl.value) || 24)),
    };

    showSettingsStatus("Saving settings…");
    try {
      const res = await fetch("/api/settings", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });
      const data = await res.json();
      if (!res.ok) {
        showSettingsStatus(data.error || "Failed to save settings.", true);
        return;
      }

      state.settings = data;
      try {
        localStorage.setItem(STORAGE_KEY, data.defaultTheme);
      } catch (_) {}

      if (data.port !== prevPort) {
        showSettingsStatus(`Server switched to port ${data.port} — redirecting…`);
        const nextUrl = new URL(window.location.href);
        nextUrl.port = String(data.port);
        nextUrl.searchParams.set("theme", data.defaultTheme);
        nextUrl.searchParams.delete("settings");
        setTimeout(() => {
          window.location.href = nextUrl.toString();
        }, 450);
        return;
      }

      closeSettingsModal();
      if (data.defaultTheme !== state.themeId) {
        await loadTheme(data.defaultTheme);
      } else if (activeThemeInstance && typeof activeThemeInstance.onSettings === "function") {
        activeThemeInstance.onSettings(state.settings);
      }
    } catch (err) {
      showSettingsStatus(`Could not reach server: ${err}`, true);
    }
  });
}

const hostApi = {
  HISTORY_LEN,
  fmtBytes,
  drawGraph,
  getState: () => state,
  getSettings: () => state.settings,
  openSettings: (ev) => {
    if (ev) ev.stopPropagation();
    openSettingsModal();
  },
  switchTheme: (nextId) => {
    closeThemeDropdown();
    if (!nextId) {
      const idx = state.themes.findIndex((t) => t.id === state.themeId);
      const next = state.themes[(idx + 1) % state.themes.length];
      nextId = next ? next.id : "nier";
    }
    loadTheme(nextId);
  },
  openThemeMenu: (anchorEl, ev) => {
    if (ev) ev.stopPropagation();
    toggleThemeDropdown(anchorEl);
  },
};

async function initManifest() {
  try {
    const res = await fetch("themes/manifest.json", { cache: "no-cache" });
    if (res.ok) {
      const data = await res.json();
      if (Array.isArray(data.themes) && data.themes.length > 0) {
        state.themes = data.themes;
      }
      if (data.defaultTheme) {
        state.themeId = data.defaultTheme;
      }
    }
  } catch (_) {}

  try {
    const sRes = await fetch("/api/settings", { cache: "no-cache" });
    if (sRes.ok) {
      const sData = await sRes.json();
      state.settings = { ...state.settings, ...sData };
      if (sData.defaultTheme) {
        state.themeId = sData.defaultTheme;
      }
    }
  } catch (_) {}

  const params = new URLSearchParams(window.location.search);
  const urlTheme = params.get("theme");
  const openSet = params.get("settings");
  const initial = urlTheme || state.settings.defaultTheme || state.themeId || "nier";
  await loadTheme(initial);
  if (openSet === "1" || openSet === "true") {
    openSettingsModal();
  }
}

async function loadTheme(themeId) {
  const token = ++loadToken;
  closeThemeDropdown();
  if (activeThemeInstance && typeof activeThemeInstance.unmount === "function") {
    try {
      activeThemeInstance.unmount();
    } catch (_) {}
  }
  activeThemeInstance = null;

  state.themeId = themeId;
  applyThemeBackgroundColor(themeId);
  try {
    localStorage.setItem(STORAGE_KEY, themeId);
    const url = new URL(window.location.href);
    url.searchParams.set("theme", themeId);
    window.history.replaceState({}, "", url);
  } catch (_) {}

  const base = `themes/${encodeURIComponent(themeId)}`;
  await new Promise((resolve) => {
    cssLink.onload = resolve;
    cssLink.onerror = resolve;
    cssLink.href = `${base}/style.css?t=${Date.now()}`;
  });
  if (token !== loadToken) return;

  const tplRes = await fetch(`${base}/template.html?t=${Date.now()}`, { cache: "no-cache" });
  if (!tplRes.ok) {
    if (themeId !== "nier") return loadTheme("nier");
    return;
  }
  const html = await tplRes.text();
  if (token !== loadToken) return;

  rootEl.innerHTML = html;

  const mod = await import(`./${base}/theme.js?t=${Date.now()}`);
  if (token !== loadToken) return;

  if (typeof mod.mount === "function") {
    activeThemeInstance = mod.mount(rootEl, hostApi) || {};
    if (typeof activeThemeInstance.onSettings === "function") {
      activeThemeInstance.onSettings(state.settings);
    }
    if (state.info && typeof activeThemeInstance.onInfo === "function") {
      activeThemeInstance.onInfo(state.info);
    }
    if (typeof activeThemeInstance.onStatus === "function") {
      activeThemeInstance.onStatus(state.status);
    }
    if (state.sample && typeof activeThemeInstance.onSample === "function") {
      activeThemeInstance.onSample(state.sample);
    }
  }
}

function setStatus(st, text) {
  state.status = { state: st, text };
  bannerEl.hidden = st === "live" || st === "connecting";
  if (activeThemeInstance && typeof activeThemeInstance.onStatus === "function") {
    activeThemeInstance.onStatus(state.status);
  }
}

function connectSse() {
  let everConnected = false;
  const es = new EventSource("/api/stream");

  es.addEventListener("info", (ev) => {
    if (everConnected) {
      for (const k of Object.keys(state.history)) {
        pushHistory(state.history[k], null);
      }
    }
    everConnected = true;
    state.info = JSON.parse(ev.data);
    document.title = `${state.info.hostname} — Task Monitor`;
    setStatus("live", "Live");
    if (activeThemeInstance && typeof activeThemeInstance.onInfo === "function") {
      activeThemeInstance.onInfo(state.info);
    }
  });

  es.addEventListener("sample", (ev) => {
    const s = JSON.parse(ev.data);
    state.sample = s;
    setStatus("live", "Live");

    const ramPct = s.ram.total > 0 ? (s.ram.used / s.ram.total) * 100 : 0;
    pushHistory(state.history.cpu, s.cpu.usage);
    pushHistory(state.history.ram, ramPct);

    if (s.gpu) {
      const vramPct = s.gpu.vramTotal > 0 ? (s.gpu.vramUsed / s.gpu.vramTotal) * 100 : 0;
      pushHistory(state.history.gpu, s.gpu.usage);
      pushHistory(state.history.vram, vramPct);
    } else {
      pushHistory(state.history.gpu, null);
      pushHistory(state.history.vram, null);
    }

    if (activeThemeInstance && typeof activeThemeInstance.onSample === "function") {
      activeThemeInstance.onSample(s);
    }
  });

  es.onerror = () => {
    setStatus("down", "Reconnecting…");
  };
}

initManifest();
connectSse();
