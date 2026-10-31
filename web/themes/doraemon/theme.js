// Doraemon Story of Seasons ("Noby Farm") Theme Logic

const TABS = ["status", "cpu", "ram", "gpu"];
const TAB_TITLES = {
  status: "Farm Status",
  cpu: "CPU Crop Field",
  ram: "Memory Barn",
  gpu: "GPU Workshop",
};

export function mount(root, host) {
  const $ = (id) => root.querySelector(`#${id}`);

  let activeTab = "status";
  let hostname = "Noby";
  let ramTotal = 0;
  let vramTotal = 0;
  let overviewCores = [];
  let detailCores = [];

  const tabBtns = Array.from(root.querySelectorAll(".dora-tab"));
  const panes = Array.from(root.querySelectorAll(".dora-pane"));
  const dots = Array.from(root.querySelectorAll(".pdot"));

  const els = {
    farmTitle: $("farm-title"),
    backBtn: $("dora-back-btn"),
    stCpuNum: $("st-cpu-num"),
    stCpuBar: $("st-cpu-bar"),
    stGpuNum: $("st-gpu-num"),
    stGpuBar: $("st-gpu-bar"),
    stRamTxt: $("st-ram-txt"),
    stVramTxt: $("st-vram-txt"),
    stGpuThermals: $("st-gpu-thermals"),
    stCoreCount: $("st-core-count"),
    stCoresRow: $("st-cores-row"),
    midCpuPct: $("mid-cpu-pct"),
    midCpuName: $("mid-cpu-name"),
    midRamPct: $("mid-ram-pct"),
    midGpuPct: $("mid-gpu-pct"),
    midGpuName: $("mid-gpu-name"),
    midVramPct: $("mid-vram-pct"),
    stCpuCanvas: $("st-cpu-canvas"),
    stGpuCanvas: $("st-gpu-canvas"),
    detCpuTitle: $("det-cpu-title"),
    detCpuVal: $("det-cpu-val"),
    detCpuCanvas: $("det-cpu-canvas"),
    detCoresGrid: $("det-cores-grid"),
    detRamVal: $("det-ram-val"),
    detRamCanvas: $("det-ram-canvas"),
    detRamUsed: $("det-ram-used"),
    detRamFree: $("det-ram-free"),
    detRamTotal: $("det-ram-total"),
    detGpuTitle: $("det-gpu-title"),
    detGpuVal: $("det-gpu-val"),
    detGpuCanvas: $("det-gpu-canvas"),
    detVramVal: $("det-vram-val"),
    detVramCanvas: $("det-vram-canvas"),
    detGpuTemp: $("det-gpu-temp"),
    detGpuPower: $("det-gpu-power"),
    doraDot: $("dora-dot"),
    doraStatusText: $("dora-status-text"),
    doraSelf: $("dora-self"),
    themeBtn: $("dora-theme-btn"),
    prevTabBtn: $("dora-prev-tab"),
    nextTabBtn: $("dora-next-tab"),
    tabNameLabel: $("dora-tab-name"),
  };

  function selectTab(tabId) {
    if (!TABS.includes(tabId)) return;
    activeTab = tabId;
    for (const btn of tabBtns) {
      const isAct = btn.dataset.tab === tabId;
      btn.classList.toggle("active", isAct);
      btn.setAttribute("aria-selected", isAct ? "true" : "false");
    }
    for (const p of panes) {
      p.classList.toggle("active", p.dataset.pane === tabId);
    }
    for (const d of dots) {
      d.classList.toggle("active", d.dataset.dot === tabId);
    }
    els.tabNameLabel.textContent = TAB_TITLES[tabId] || tabId;
    els.farmTitle.textContent =
      tabId === "status" ? `${hostname} Farm` : `${hostname} · ${TAB_TITLES[tabId]}`;
    if (els.backBtn) els.backBtn.hidden = tabId === "status";
    drawVisibleGraphs();
  }

  for (const btn of tabBtns) {
    btn.addEventListener("click", () => selectTab(btn.dataset.tab));
  }
  for (const card of root.querySelectorAll("[data-jump]")) {
    card.addEventListener("click", () => selectTab(card.dataset.jump));
  }
  if (els.backBtn) {
    els.backBtn.addEventListener("click", () => selectTab("status"));
  }

  function stepTab(delta) {
    const idx = TABS.indexOf(activeTab);
    const next = TABS[(idx + delta + TABS.length) % TABS.length];
    selectTab(next);
  }

  els.prevTabBtn.addEventListener("click", () => stepTab(-1));
  els.nextTabBtn.addEventListener("click", () => stepTab(1));
  els.themeBtn.addEventListener("click", (ev) => host.openThemeMenu(els.themeBtn, ev));

  const onKeyDown = (ev) => {
    if (ev.key === "Escape" || ev.key === "Backspace") selectTab("status");
    if (ev.key === "ArrowLeft") stepTab(-1);
    if (ev.key === "ArrowRight") stepTab(1);
  };
  window.addEventListener("keydown", onKeyDown);

  function buildCores(n) {
    els.stCoresRow.textContent = "";
    els.detCoresGrid.textContent = "";
    overviewCores = [];
    detailCores = [];

    for (let i = 0; i < n; i++) {
      // Overview Cookware slot (circle silhouette + 5 stars)
      const wrap = document.createElement("div");
      wrap.className = "core-utensil";
      const well = document.createElement("div");
      well.className = "utensil-Well";
      const fill = document.createElement("i");
      fill.className = "utensil-fill";
      const label = document.createElement("span");
      label.className = "utensil-icon";
      label.textContent = `C${i + 1}`;
      well.append(fill, label);

      const stars = document.createElement("div");
      stars.className = "utensil-stars";
      const starSpans = [];
      for (let s = 0; s < 5; s++) {
        const sp = document.createElement("span");
        sp.textContent = "★";
        stars.appendChild(sp);
        starSpans.push(sp);
      }
      wrap.append(well, stars);
      els.stCoresRow.appendChild(wrap);
      overviewCores.push({ fill, starSpans, wrap });

      // Detail Core plot
      const dbox = document.createElement("div");
      dbox.className = "det-core-box";
      const dtop = document.createElement("div");
      dtop.className = "det-core-top";
      const dname = document.createElement("span");
      dname.textContent = `Plot #${i + 1}`;
      const dval = document.createElement("span");
      dval.textContent = "0.0%";
      dtop.append(dname, dval);
      const dbar = document.createElement("div");
      dbar.className = "det-core-bar";
      const dfill = document.createElement("i");
      dbar.appendChild(dfill);
      dbox.append(dtop, dbar);
      els.detCoresGrid.appendChild(dbox);
      detailCores.push({ dval, dfill });
    }
  }

  function makeList(listEl, count = 15) {
    listEl.textContent = "";
    const items = [];
    items.listEl = listEl;
    for (let i = 0; i < count; i++) {
      const li = document.createElement("li");
      const nameWrap = document.createElement("span");
      nameWrap.className = "app-name";
      const name = document.createElement("span");
      const cnt = document.createElement("span");
      cnt.className = "app-count";
      nameWrap.append(name, cnt);

      const val = document.createElement("span");
      val.className = "app-val";

      const bar = document.createElement("div");
      bar.className = "app-bar";
      const fill = document.createElement("i");
      bar.appendChild(fill);

      li.append(nameWrap, val, bar);
      li.hidden = true;
      listEl.appendChild(li);
      items.push({ li, name, cnt, val, fill });
    }
    const empty = document.createElement("li");
    empty.className = "empty";
    empty.hidden = true;
    listEl.appendChild(empty);
    items.empty = empty;
    return items;
  }

  let lists = {
    stCpu: makeList($("st-top-cpu"), 8),
    stMem: makeList($("st-top-mem"), 8),
    stGpu: makeList($("st-top-gpu"), 8),
    detCpu: makeList($("det-top-cpu"), 24),
    detMem: makeList($("det-top-mem"), 24),
    detGpu: makeList($("det-top-gpu"), 24),
  };

  function fillList(slot, entries, fmtVal, pctFn, emptyText) {
    if (!entries || entries.length === 0) {
      for (const r of slot) r.li.hidden = true;
      slot.empty.textContent = emptyText;
      slot.empty.hidden = false;
      return;
    }
    slot.empty.hidden = true;
    for (let i = 0; i < slot.length; i++) {
      const r = slot[i];
      const e = entries[i];
      if (!e) {
        r.li.hidden = true;
        continue;
      }
      r.li.hidden = false;
      r.name.textContent = e.name;
      r.cnt.textContent = e.count > 1 ? `×${e.count}` : "";
      r.val.textContent = fmtVal(e);
      const pct = Math.max(0, Math.min(100, pctFn(e)));
      r.fill.style.width = `${pct.toFixed(1)}%`;
    }

    const boxH = slot.listEl ? slot.listEl.clientHeight : 0;
    if (boxH > 0 && slot.length > 0 && !slot[0].li.hidden) {
      const firstTop = slot[0].li.offsetTop;
      let shown = 0;
      for (let i = 0; i < slot.length; i++) {
        const r = slot[i];
        if (r.li.hidden) continue;
        const bottom = r.li.offsetTop - firstTop + r.li.offsetHeight;
        if (shown > 0 && bottom > boxH + 1) {
          r.li.hidden = true;
        } else {
          shown++;
        }
      }
    }
  }

  function drawVisibleGraphs() {
    const h = host.getState().history;
    const graphOpts = {
      gridColor: "rgba(181, 131, 61, 0.35)",
      lineWidth: 2.2,
    };

    if (activeTab === "status") {
      host.drawGraph(els.stCpuCanvas, h.cpu, "#d96b0c", {
        ...graphOpts,
        fillColor: "rgba(244, 129, 22, 0.25)",
      });
      host.drawGraph(els.stGpuCanvas, h.gpu, "#3e9c35", {
        ...graphOpts,
        fillColor: "rgba(82, 183, 72, 0.25)",
      });
    } else if (activeTab === "cpu") {
      host.drawGraph(els.detCpuCanvas, h.cpu, "#d96b0c", {
        ...graphOpts,
        fillColor: "rgba(244, 129, 22, 0.28)",
      });
    } else if (activeTab === "ram") {
      host.drawGraph(els.detRamCanvas, h.ram, "#0080c8", {
        ...graphOpts,
        fillColor: "rgba(0, 152, 232, 0.24)",
      });
    } else if (activeTab === "gpu") {
      host.drawGraph(els.detGpuCanvas, h.gpu, "#3e9c35", {
        ...graphOpts,
        fillColor: "rgba(82, 183, 72, 0.28)",
      });
      host.drawGraph(els.detVramCanvas, h.vram, "#c87a12", {
        ...graphOpts,
        fillColor: "rgba(244, 162, 54, 0.26)",
      });
    }
  }

  const onResize = () => drawVisibleGraphs();
  window.addEventListener("resize", onResize);

  return {
    onSettings(settings) {
      if (!settings) return;
      const ov = Math.max(1, Math.min(25, Number(settings.overviewLimit) || 8));
      const pg = Math.max(1, Math.min(60, Number(settings.pageLimit) || 24));
      lists = {
        stCpu: makeList($("st-top-cpu"), ov),
        stMem: makeList($("st-top-mem"), ov),
        stGpu: makeList($("st-top-gpu"), ov),
        detCpu: makeList($("det-top-cpu"), pg),
        detMem: makeList($("det-top-mem"), pg),
        detGpu: makeList($("det-top-gpu"), pg),
      };
    },

    onInfo(info) {
      hostname = info.hostname || "Noby";
      ramTotal = info.ram.total || 0;
      vramTotal = info.gpu ? info.gpu.vramTotal : 0;

      els.farmTitle.textContent =
        activeTab === "status" ? `${hostname} Farm` : `${hostname} · ${TAB_TITLES[activeTab]}`;
      els.stCoreCount.textContent = `${info.cpu.logicalCores}T`;
      els.midCpuName.textContent = `· ${info.cpu.name}`;
      els.detCpuTitle.textContent = `${info.cpu.name} (${info.cpu.logicalCores}T)`;

      if (info.gpu) {
        els.midGpuName.textContent = `· ${info.gpu.name}`;
        els.detGpuTitle.textContent = info.gpu.name;
      } else {
        els.midGpuName.textContent = "· Unavailable";
        els.detGpuTitle.textContent = "GPU Unavailable";
      }

      buildCores(info.cpu.logicalCores);
    },

    onStatus(st) {
      els.doraDot.className = `dora-dot ${st.state}`;
      els.doraStatusText.textContent = st.text;
    },

    onSample(s) {
      const cpuRound = Math.round(s.cpu.usage);
      els.stCpuNum.textContent = cpuRound;
      els.stCpuBar.style.width = `${s.cpu.usage.toFixed(1)}%`;
      els.midCpuPct.textContent = `${s.cpu.usage.toFixed(1)}%`;
      els.detCpuVal.textContent = `${s.cpu.usage.toFixed(1)}%`;

      const ramUsedGB = (s.ram.used / 1073741824).toFixed(1);
      const ramTotalGB = (s.ram.total / 1073741824).toFixed(0);
      const ramPct = s.ram.total > 0 ? (s.ram.used / s.ram.total) * 100 : 0;
      els.stRamTxt.textContent = `${ramUsedGB}/${ramTotalGB} GB`;
      els.midRamPct.textContent = `${ramPct.toFixed(0)}%`;
      els.detRamVal.textContent = `${ramUsedGB} / ${ramTotalGB} GB (${ramPct.toFixed(0)}%)`;
      els.detRamUsed.textContent = `${ramUsedGB} GB`;
      els.detRamFree.textContent = `${Math.max(0, (s.ram.total - s.ram.used) / 1073741824).toFixed(1)} GB`;
      els.detRamTotal.textContent = `${ramTotalGB} GB`;

      if (s.gpu) {
        const gpuRound = Math.round(s.gpu.usage);
        const vramUsedGB = (s.gpu.vramUsed / 1073741824).toFixed(1);
        const vramTotalGB = (s.gpu.vramTotal / 1073741824).toFixed(0);
        const vramPct = s.gpu.vramTotal > 0 ? (s.gpu.vramUsed / s.gpu.vramTotal) * 100 : 0;

        els.stGpuNum.textContent = gpuRound;
        els.stGpuBar.style.width = `${s.gpu.usage.toFixed(1)}%`;
        els.stVramTxt.textContent = `${vramUsedGB}/${vramTotalGB} GB`;
        const tStr = s.gpu.tempC != null ? `${Math.round(s.gpu.tempC)}°C` : "—";
        const pStr = s.gpu.powerPct != null ? `${Math.round(s.gpu.powerPct)}% pwr` : "—";
        els.stGpuThermals.textContent = `${tStr} · ${pStr}`;

        els.midGpuPct.textContent = `${s.gpu.usage.toFixed(1)}%`;
        els.midVramPct.textContent = `${vramPct.toFixed(0)}%`;
        els.detGpuVal.textContent = `${s.gpu.usage.toFixed(1)}%`;
        els.detVramVal.textContent = `${vramUsedGB} / ${vramTotalGB} GB (${vramPct.toFixed(0)}%)`;
        els.detGpuTemp.textContent = tStr;
        els.detGpuPower.textContent = pStr;
      } else {
        els.stGpuNum.textContent = "—";
        els.stGpuBar.style.width = "0%";
        els.stVramTxt.textContent = "—";
        els.stGpuThermals.textContent = "Unavailable";
      }

      if (overviewCores.length !== s.cpu.cores.length) {
        buildCores(s.cpu.cores.length);
      }
      for (let i = 0; i < s.cpu.cores.length; i++) {
        const v = s.cpu.cores[i];
        const oc = overviewCores[i];
        oc.fill.style.height = `${v.toFixed(1)}%`;
        oc.fill.classList.toggle("hot", v >= 90);
        oc.wrap.title = `Core #${i + 1}: ${v.toFixed(1)}%`;
        const litStars = Math.ceil((v / 100) * 5);
        for (let st = 0; st < 5; st++) {
          oc.starSpans[st].classList.toggle("lit", st < litStars && v >= 5);
        }

        const dc = detailCores[i];
        dc.dval.textContent = `${v.toFixed(1)}%`;
        dc.dfill.style.width = `${v.toFixed(1)}%`;
      }

      const cpuFmt = (e) => `${e.cpu.toFixed(1)}%`;
      const cpuPct = (e) => e.cpu;
      fillList(lists.stCpu, s.top.cpu, cpuFmt, cpuPct, "All quiet on the farm");
      fillList(
        lists.detCpu,
        s.top.cpu,
        (e) => `${e.cpu.toFixed(1)}% · ${host.fmtBytes(e.mem)}`,
        cpuPct,
        "All quiet on the farm"
      );

      const memFmt = (e) => host.fmtBytes(e.mem);
      const memPct = (e) => (ramTotal > 0 ? (e.mem / ramTotal) * 100 : 0);
      fillList(lists.stMem, s.top.mem, memFmt, memPct, "Barn is empty");
      fillList(
        lists.detMem,
        s.top.mem,
        (e) => `${host.fmtBytes(e.mem)} · ${e.cpu.toFixed(1)}% CPU`,
        memPct,
        "Barn is empty"
      );

      if (s.top.gpu === null) {
        fillList(lists.stGpu, [], () => "", () => 0, "GPU metrics unavailable");
        fillList(lists.detGpu, [], () => "", () => 0, "GPU metrics unavailable");
      } else {
        const gpuFmt = (e) => `${(e.gpu ?? 0).toFixed(1)}% · ${host.fmtBytes(e.vram ?? 0)}`;
        const gpuPct = (e) => e.gpu ?? 0;
        fillList(lists.stGpu, s.top.gpu, gpuFmt, gpuPct, "All gadgets idle");
        fillList(lists.detGpu, s.top.gpu, gpuFmt, gpuPct, "All gadgets idle");
      }

      els.doraSelf.textContent = `${s.self.cpu.toFixed(1)}% CPU, ${host.fmtBytes(s.self.mem)}`;
      drawVisibleGraphs();
    },

    unmount() {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("resize", onResize);
    },
  };
}
