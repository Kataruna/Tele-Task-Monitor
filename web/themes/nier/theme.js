// NieR:Automata YoRHa Theme ("OVERVIEW - All-System Telemetry Brief")

const OVERVIEW_ROWS = 8;
const MAX_ROWS = 24;
const TABS = ["overview", "cpu", "mem", "gpu", "intel"];
const TAB_META = {
  overview: { title: "OVERVIEW", sub: "- Plug-in Chips: All-System Telemetry Brief" },
  cpu: { title: "CPU", sub: "- Logic Virus & Processor Allocation" },
  mem: { title: "MEMORY", sub: "- Pod Working Set & Storage Blocks" },
  gpu: { title: "GPU / VRAM", sub: "- Optical & Shader Unit Telemetry" },
  intel: { title: "INTEL", sub: "- Complete Process Archives" },
};

export function mount(root, host) {
  const $ = (id) => root.querySelector(`#${id}`);

  let activeTab = "overview";
  let selectedIdx = 0;
  let coreSquares = [];
  let lastSample = null;
  let overviewLimit = OVERVIEW_ROWS;
  let pageLimit = MAX_ROWS;

  const tabBtns = Array.from(root.querySelectorAll(".nier-tab[data-tab]"));
  const settingsTabBtn = $("nier-settings-tab");
  if (settingsTabBtn) {
    settingsTabBtn.addEventListener("click", (ev) => host.openSettings(ev));
  }

  const els = {
    mainTitle: $("nier-main-title"),
    subTitle: $("nier-sub-title"),
    titleBack: $("nier-title-back"),
    inlineBack: $("nier-inline-back"),
    backBtn: $("nier-back-btn"),
    storageVal: $("nier-storage-val"),
    mainView: $("nier-main-view"),
    intelView: $("nier-intel-view"),
    overviewLeft: $("nier-overview-left"),
    singleLeft: $("nier-single-left"),
    ovHeadCpu: $("ov-head-cpu"),
    ovHeadMem: $("ov-head-mem"),
    ovHeadGpu: $("ov-head-gpu"),
    ovListCpu: $("ov-list-cpu"),
    ovListMem: $("ov-list-mem"),
    ovListGpu: $("ov-list-gpu"),
    sortName: $("nier-sort-name"),
    chipList: $("nier-chip-list"),
    stackMode: $("nier-stack-mode"),
    stackPct: $("nier-stack-pct"),
    stackPointer: $("nier-stack-pointer"),
    stackBars: $("nier-stack-bars"),
    waveLblCpu: $("wave-lbl-cpu"),
    waveLblRam: $("wave-lbl-ram"),
    waveLblGpu: $("wave-lbl-gpu"),
    waveLblVram: $("wave-lbl-vram"),
    waveCpu: $("wave-cpu"),
    waveRam: $("wave-ram"),
    waveGpu: $("wave-gpu"),
    waveVram: $("wave-vram"),
    hostname: $("nier-hostname"),
    coresLv: $("nier-cores-lv"),
    barCpu: $("nier-bar-cpu"),
    stCpu: $("nier-st-cpu"),
    barRam: $("nier-bar-ram"),
    stRam: $("nier-st-ram"),
    barGpu: $("nier-bar-gpu"),
    stGpu: $("nier-st-gpu"),
    barVram: $("nier-bar-vram"),
    stVram: $("nier-st-vram"),
    stCpuName: $("nier-st-cpuname"),
    stGpuName: $("nier-st-gpuname"),
    stThermals: $("nier-st-thermals"),
    coresCnt: $("nier-cores-cnt"),
    coreSquares: $("nier-core-squares"),
    miniWave: $("nier-mini-wave"),
    diagText: $("nier-diag-text"),
    intelCpu: $("nier-intel-cpu"),
    intelMem: $("nier-intel-mem"),
    intelGpu: $("nier-intel-gpu"),
    descText: $("nier-desc-text"),
    tabBtn: $("nier-tab-btn"),
    themeBtn: $("nier-theme-btn"),
  };

  function setTab(tabId) {
    if (!TABS.includes(tabId)) return;
    activeTab = tabId;
    for (const b of tabBtns) {
      const act = b.dataset.tab === tabId;
      b.classList.toggle("active", act);
      b.setAttribute("aria-selected", act ? "true" : "false");
    }
    const meta = TAB_META[tabId] || TAB_META.overview;
    els.mainTitle.textContent = meta.title;
    els.subTitle.textContent = meta.sub;

    const isIntel = tabId === "intel";
    const isOverview = tabId === "overview";
    els.mainView.hidden = isIntel;
    els.intelView.hidden = !isIntel;
    if (els.titleBack) els.titleBack.hidden = isOverview;
    if (els.backBtn) els.backBtn.hidden = isOverview;

    if (!isIntel) {
      els.overviewLeft.hidden = !isOverview;
      els.singleLeft.hidden = isOverview;
      if (tabId === "cpu") els.sortName.textContent = "CPU";
      else if (tabId === "mem") els.sortName.textContent = "Memory";
      else if (tabId === "gpu") els.sortName.textContent = "GPU / VRAM";
    }

    if (lastSample) renderSample(lastSample);
  }

  for (const b of tabBtns) {
    b.addEventListener("click", () => setTab(b.dataset.tab));
  }
  for (const box of root.querySelectorAll("[data-jump]")) {
    box.addEventListener("click", () => setTab(box.dataset.jump));
  }

  if (els.titleBack) els.titleBack.addEventListener("click", () => setTab("overview"));
  if (els.inlineBack) els.inlineBack.addEventListener("click", () => setTab("overview"));
  if (els.backBtn) els.backBtn.addEventListener("click", () => setTab("overview"));

  els.tabBtn.addEventListener("click", () => {
    const next = TABS[(TABS.indexOf(activeTab) + 1) % TABS.length];
    setTab(next);
  });

  els.themeBtn.addEventListener("click", (ev) => host.openThemeMenu(els.themeBtn, ev));

  const onKeyDown = (ev) => {
    if (ev.key === "Escape" || ev.key === "Backspace") {
      if (activeTab !== "overview") setTab("overview");
    } else if (ev.key === "ArrowUp" || ev.key === "w" || ev.key === "W") {
      selectedIdx = Math.max(0, selectedIdx - 1);
      if (lastSample) renderSample(lastSample);
    } else if (ev.key === "ArrowDown" || ev.key === "s" || ev.key === "S") {
      selectedIdx = Math.min(pageLimit - 1, selectedIdx + 1);
      if (lastSample) renderSample(lastSample);
    } else if (ev.key === "ArrowLeft") {
      const next = TABS[(TABS.indexOf(activeTab) - 1 + TABS.length) % TABS.length];
      setTab(next);
    } else if (ev.key === "ArrowRight") {
      const next = TABS[(TABS.indexOf(activeTab) + 1) % TABS.length];
      setTab(next);
    }
  };
  window.addEventListener("keydown", onKeyDown);

  function buildCoreSquares(n) {
    els.coreSquares.textContent = "";
    coreSquares = [];
    for (let i = 0; i < n; i++) {
      const sq = document.createElement("div");
      sq.className = "nier-core-sq";
      const fill = document.createElement("i");
      sq.appendChild(fill);
      els.coreSquares.appendChild(sq);
      coreSquares.push({ sq, fill });
    }
  }

  function makeChipRows(listEl, count = MAX_ROWS, interactive = false) {
    listEl.textContent = "";
    const out = [];
    out.listEl = listEl;
    for (let i = 0; i < count; i++) {
      const li = document.createElement("li");
      li.className = "nier-chip-item";

      const left = document.createElement("span");
      left.className = "chip-left";
      const icon = document.createElement("span");
      icon.className = "chip-icon";
      const name = document.createElement("span");
      name.className = "chip-name";
      const cnt = document.createElement("span");
      cnt.className = "chip-count";
      const diamond = document.createElement("span");
      diamond.className = "chip-diamond";
      diamond.textContent = "◆";
      left.append(icon, name, cnt, diamond);

      const cost = document.createElement("span");
      cost.className = "chip-cost";

      li.append(left, cost);
      li.hidden = true;
      if (interactive) {
        li.addEventListener("click", (ev) => {
          ev.stopPropagation();
          selectedIdx = i;
          if (lastSample) renderSample(lastSample);
        });
      }
      listEl.appendChild(li);
      out.push({ li, name, count: cnt, cost });
    }
    return out;
  }

  let ovRows = {};
  let singleRows = [];
  let intelRows = {};
  let stackSlices = [];

  function rebuildAllLists() {
    els.ovListCpu.style.gridTemplateRows = `repeat(${overviewLimit}, minmax(0, 1fr))`;
    els.ovListMem.style.gridTemplateRows = `repeat(${overviewLimit}, minmax(0, 1fr))`;
    els.ovListGpu.style.gridTemplateRows = `repeat(${overviewLimit}, minmax(0, 1fr))`;

    els.chipList.style.gridTemplateRows = `repeat(${pageLimit}, minmax(0, 1fr))`;
    els.intelCpu.style.gridTemplateRows = `repeat(${pageLimit}, minmax(0, 1fr))`;
    els.intelMem.style.gridTemplateRows = `repeat(${pageLimit}, minmax(0, 1fr))`;
    els.intelGpu.style.gridTemplateRows = `repeat(${pageLimit}, minmax(0, 1fr))`;

    ovRows = {
      cpu: makeChipRows(els.ovListCpu, overviewLimit, false),
      mem: makeChipRows(els.ovListMem, overviewLimit, false),
      gpu: makeChipRows(els.ovListGpu, overviewLimit, false),
    };
    singleRows = makeChipRows(els.chipList, pageLimit, true);
    intelRows = {
      cpu: makeChipRows(els.intelCpu, pageLimit, false),
      mem: makeChipRows(els.intelMem, pageLimit, false),
      gpu: makeChipRows(els.intelGpu, pageLimit, false),
    };

    els.stackBars.textContent = "";
    stackSlices = [];
    const spacerTop = document.createElement("div");
    spacerTop.style.flex = "1";
    els.stackBars.appendChild(spacerTop);
    for (let i = 0; i < pageLimit + 1; i++) {
      const div = document.createElement("div");
      div.className = "stack-slice c0";
      const lbl = document.createElement("span");
      lbl.className = "slice-lbl";
      div.appendChild(lbl);
      div.addEventListener("click", () => {
        if (div.dataset.jump) {
          setTab(div.dataset.jump);
        } else if (div.dataset.idx != null) {
          selectedIdx = Number(div.dataset.idx);
          if (lastSample) renderSample(lastSample);
        }
      });
      els.stackBars.appendChild(div);
      stackSlices.push({ el: div, lbl });
    }
  }

  rebuildAllLists();

  function fillChipRows(rows, entries, fmtCost, activeIdx = -1) {
    let visibleCount = 0;
    for (let i = 0; i < rows.length; i++) {
      const r = rows[i];
      const e = entries ? entries[i] : null;
      if (!e) {
        r.li.hidden = true;
        continue;
      }
      r.li.hidden = false;
      r.name.textContent = e.name;
      r.count.textContent = e.count > 1 ? `×${e.count}` : "";
      r.cost.textContent = `【${fmtCost(e)}】`;
      visibleCount++;
    }

    const clampedActive = visibleCount > 0 ? Math.min(activeIdx, visibleCount - 1) : -1;
    for (let i = 0; i < rows.length; i++) {
      rows[i].li.classList.toggle("selected", i === clampedActive);
    }
    return visibleCount;
  }

  function renderSample(s) {
    lastSample = s;

    const ramUsedGB = (s.ram.used / 1073741824).toFixed(1);
    const ramTotalGB = (s.ram.total / 1073741824).toFixed(0);
    const ramPct = s.ram.total > 0 ? (s.ram.used / s.ram.total) * 100 : 0;
    const gpuUsage = s.gpu ? s.gpu.usage : 0;
    const vramUsedGB = s.gpu ? (s.gpu.vramUsed / 1073741824).toFixed(1) : "0.0";
    const vramTotalGB = s.gpu ? (s.gpu.vramTotal / 1073741824).toFixed(0) : "0";
    const vramPct = s.gpu && s.gpu.vramTotal > 0 ? (s.gpu.vramUsed / s.gpu.vramTotal) * 100 : 0;

    els.storageVal.textContent = `CPU ${s.cpu.usage.toFixed(0)}% · RAM ${ramUsedGB}/${ramTotalGB}G · GPU ${gpuUsage.toFixed(0)}%`;

    // Overview Left Brief Boxes (shows as many rows as fit each 1/3 box)
    els.ovHeadCpu.textContent = `${s.cpu.usage.toFixed(1)}% Total`;
    els.ovHeadMem.textContent = `${ramUsedGB} / ${ramTotalGB} GB`;
    els.ovHeadGpu.textContent = s.gpu ? `${s.gpu.usage.toFixed(1)}% · ${vramUsedGB}G` : "Unavailable";

    fillChipRows(ovRows.cpu, s.top.cpu || [], (e) => `${e.cpu.toFixed(1)}%`, 0);
    fillChipRows(ovRows.mem, s.top.mem || [], (e) => host.fmtBytes(e.mem), 0);
    fillChipRows(
      ovRows.gpu,
      s.top.gpu || [],
      (e) => `${(e.gpu ?? 0).toFixed(1)}% · ${host.fmtBytes(e.vram ?? 0)}`,
      0
    );

    // Right Status Card
    els.barCpu.style.width = `${s.cpu.usage.toFixed(1)}%`;
    els.stCpu.textContent = `${s.cpu.usage.toFixed(1)} / 100%`;

    els.barRam.style.width = `${ramPct.toFixed(1)}%`;
    els.stRam.textContent = `${ramUsedGB} / ${ramTotalGB} GB`;

    if (s.gpu) {
      els.barGpu.style.width = `${s.gpu.usage.toFixed(1)}%`;
      els.stGpu.textContent = `${s.gpu.usage.toFixed(1)} / 100%`;
      els.barVram.style.width = `${vramPct.toFixed(1)}%`;
      els.stVram.textContent = `${vramUsedGB} / ${vramTotalGB} GB`;
      const t = s.gpu.tempC != null ? `${Math.round(s.gpu.tempC)} °C` : "—";
      const p = s.gpu.powerPct != null ? `${Math.round(s.gpu.powerPct)} % Pwr` : "—";
      els.stThermals.textContent = `${t} · ${p}`;
    } else {
      els.barGpu.style.width = "0%";
      els.stGpu.textContent = "Unavailable";
      els.barVram.style.width = "0%";
      els.stVram.textContent = "—";
      els.stThermals.textContent = "—";
    }

    if (coreSquares.length !== s.cpu.cores.length) {
      buildCoreSquares(s.cpu.cores.length);
    }
    for (let i = 0; i < s.cpu.cores.length; i++) {
      const v = s.cpu.cores[i];
      coreSquares[i].fill.style.height = `${v.toFixed(1)}%`;
      coreSquares[i].sq.classList.toggle("hot", v >= 90);
    }

    // Populate Intel all-3 view (fits as many rows as fit the full column height)
    fillChipRows(intelRows.cpu, s.top.cpu || [], (e) => `${e.cpu.toFixed(1)}%`, 0);
    fillChipRows(intelRows.mem, s.top.mem || [], (e) => host.fmtBytes(e.mem), 0);
    fillChipRows(
      intelRows.gpu,
      s.top.gpu || [],
      (e) => `${(e.gpu ?? 0).toFixed(1)}% · ${host.fmtBytes(e.vram ?? 0)}`,
      0
    );

    // Populate Single-Resource view & Central Capacity Bar
    let stackList = s.top.cpu || [];
    let fmtSingle = (e) => `${e.cpu.toFixed(1)}% · ${host.fmtBytes(e.mem)}`;

    if (activeTab === "mem") {
      stackList = s.top.mem || [];
      fmtSingle = (e) => `${host.fmtBytes(e.mem)} · ${e.cpu.toFixed(1)}%`;
    } else if (activeTab === "gpu") {
      stackList = s.top.gpu || [];
      fmtSingle = (e) => `${(e.gpu ?? 0).toFixed(1)}% · ${host.fmtBytes(e.vram ?? 0)}`;
    }

    const fitCount = fillChipRows(singleRows, stackList, fmtSingle, selectedIdx);
    if (fitCount > 0 && selectedIdx >= fitCount) {
      selectedIdx = fitCount - 1;
    }

    renderCentralStack(s, ramPct, gpuUsage, vramPct, stackList, fitCount);

    const c0 = s.top.cpu && s.top.cpu[0] ? `${s.top.cpu[0].name} (${s.top.cpu[0].cpu.toFixed(1)}%)` : "—";
    const m0 = s.top.mem && s.top.mem[0] ? `${s.top.mem[0].name} (${host.fmtBytes(s.top.mem[0].mem)})` : "—";
    const g0 = s.top.gpu && s.top.gpu[0] ? `${s.top.gpu[0].name} (${(s.top.gpu[0].gpu ?? 0).toFixed(1)}%)` : "—";
    els.descText.textContent = `Top CPU: ${c0} · Top Mem: ${m0} · Top GPU: ${g0}  |  Monitor: ${s.self.cpu.toFixed(1)}% CPU, ${host.fmtBytes(s.self.mem)}`;

    drawWaves(s, ramPct, gpuUsage, vramPct);
  }

  function shortProcName(name) {
    return (name || "").replace(/\.exe$/i, "");
  }

  function renderCentralStack(s, ramPct, gpuUsage, vramPct, stackList, fitCount) {
    // Reset all slices first
    for (const item of stackSlices) {
      item.el.style.height = "0%";
      item.el.classList.remove("active-slice");
      item.lbl.textContent = "";
      delete item.el.dataset.jump;
      delete item.el.dataset.idx;
    }

    let items = [];
    if (activeTab === "overview") {
      const avgSys = (s.cpu.usage + ramPct + gpuUsage + vramPct) / 4;
      if (els.stackMode) els.stackMode.textContent = "SYS";
      if (els.stackPct) els.stackPct.textContent = `${avgSys.toFixed(0)}%`;

      // 4 clearly labeled resource blocks matching the 4 waves next to the bar
      const rawBlocks = [
        { cls: "c0", h: Math.max(4.5, s.cpu.usage * 0.25), lbl: `CPU ${s.cpu.usage.toFixed(0)}%`, jump: "cpu" },
        { cls: "c1", h: Math.max(4.5, ramPct * 0.25), lbl: `RAM ${ramPct.toFixed(0)}%`, jump: "mem" },
        { cls: "c2", h: Math.max(4.5, gpuUsage * 0.25), lbl: `GPU ${gpuUsage.toFixed(0)}%`, jump: "gpu" },
        { cls: "c3", h: Math.max(4.5, vramPct * 0.25), lbl: `VRAM ${vramPct.toFixed(0)}%`, jump: "gpu" },
      ];
      // Highlight the busiest resource block in Overview
      const pcts = [s.cpu.usage, ramPct, gpuUsage, vramPct];
      let busiestIdx = 0;
      for (let i = 1; i < 4; i++) {
        if (pcts[i] > pcts[busiestIdx]) busiestIdx = i;
      }
      items = rawBlocks.map((b, i) => ({
        cls: b.cls,
        h: b.h,
        lbl: b.lbl,
        jump: b.jump,
        active: i === busiestIdx,
      }));
    } else {
      let totalTargetPct = s.cpu.usage;
      let valOf = (e) => e.cpu;
      let lblOf = (e) => `${shortProcName(e.name)} ${e.cpu.toFixed(0)}%`;
      let remLabel = "Other CPU";

      if (activeTab === "mem") {
        totalTargetPct = ramPct;
        valOf = (e) => (s.ram.total > 0 ? (e.mem / s.ram.total) * 100 : 0);
        lblOf = (e) => `${shortProcName(e.name)} ${host.fmtBytes(e.mem)}`;
        remLabel = "OS / Cache";
        if (els.stackMode) els.stackMode.textContent = "RAM";
        if (els.stackPct) els.stackPct.textContent = `${ramPct.toFixed(0)}%`;
      } else if (activeTab === "gpu") {
        const useVram = vramPct > gpuUsage && s.gpu && s.gpu.vramTotal > 0;
        totalTargetPct = useVram ? vramPct : gpuUsage;
        valOf = (e) =>
          useVram
            ? ((e.vram ?? 0) / s.gpu.vramTotal) * 100
            : e.gpu ?? 0;
        lblOf = (e) =>
          useVram
            ? `${shortProcName(e.name)} ${host.fmtBytes(e.vram ?? 0)}`
            : `${shortProcName(e.name)} ${(e.gpu ?? 0).toFixed(0)}%`;
        remLabel = useVram ? "Shared VRAM" : "Other GPU";
        if (els.stackMode) els.stackMode.textContent = useVram ? "VRAM" : "GPU";
        if (els.stackPct) els.stackPct.textContent = `${totalTargetPct.toFixed(0)}%`;
      } else {
        if (els.stackMode) els.stackMode.textContent = "CPU";
        if (els.stackPct) els.stackPct.textContent = `${s.cpu.usage.toFixed(1)}%`;
      }

      const count = Math.min(fitCount || stackList.length, pageLimit);
      const rawVals = [];
      let sumRaw = 0;
      for (let i = 0; i < count; i++) {
        const v = Math.max(0, valOf(stackList[i]));
        rawVals.push(v);
        sumRaw += v;
      }

      const clampedTotal = Math.max(2, Math.min(100, totalTargetPct));
      const scale = sumRaw > clampedTotal && sumRaw > 0 ? clampedTotal / sumRaw : 1;

      let usedSum = 0;
      for (let i = 0; i < count; i++) {
        const h = rawVals[i] * scale;
        if (h < 0.35 && i !== selectedIdx) continue;
        const effH = Math.max(1.2, h);
        usedSum += effH;
        const tier = Math.min(3, Math.floor((i / Math.max(1, count)) * 4));
        items.push({
          cls: `c${tier}`,
          h: effH,
          lbl: effH >= 3.4 ? lblOf(stackList[i]) : "",
          idx: i,
          active: i === selectedIdx,
        });
      }

      const rem = clampedTotal - usedSum;
      if (rem >= 1.5 && items.length < stackSlices.length) {
        items.push({
          cls: "c3",
          h: rem,
          lbl: rem >= 3.8 ? `${remLabel} ${rem.toFixed(0)}%` : "",
          active: false,
        });
      }

      // Normalize if min-heights pushed sum slightly above clampedTotal
      const totalH = items.reduce((acc, it) => acc + it.h, 0);
      if (totalH > clampedTotal && totalH > 0) {
        const norm = clampedTotal / totalH;
        for (const it of items) {
          it.h *= norm;
          if (it.h < 3.4 && !it.active) it.lbl = "";
        }
      }
    }

    // Apply items to DOM slices and position the ◆: pointer at the active slice
    const totalStackPct = items.reduce((acc, it) => acc + it.h, 0);
    let cumFromTopOfStack = 0;
    let pointerTopPct = 100 - totalStackPct / 2;

    for (let i = 0; i < items.length && i < stackSlices.length; i++) {
      const it = items[i];
      const slot = stackSlices[i];
      slot.el.className = `stack-slice ${it.cls}${it.active ? " active-slice" : ""}`;
      slot.el.style.height = `${it.h.toFixed(2)}%`;
      slot.lbl.textContent = it.lbl || "";
      if (it.jump) slot.el.dataset.jump = it.jump;
      if (it.idx != null) slot.el.dataset.idx = String(it.idx);
      if (it.active) {
        pointerTopPct = 100 - totalStackPct + cumFromTopOfStack + it.h / 2;
      }
      cumFromTopOfStack += it.h;
    }

    if (els.stackPointer) {
      els.stackPointer.style.top = `${Math.max(8, Math.min(96, pointerTopPct)).toFixed(1)}%`;
    }
  }

  function drawWaves(s, ramPct, gpuUsage, vramPct) {
    const h = host.getState().history;
    const opts = {
      gridColor: "rgba(78, 75, 66, 0.2)",
      fillColor: "rgba(78, 75, 66, 0.16)",
      lineWidth: 1.7,
    };

    els.waveLblCpu.textContent = `${s.cpu.usage.toFixed(1)}%`;
    els.waveLblRam.textContent = `${ramPct.toFixed(0)}%`;
    els.waveLblGpu.textContent = s.gpu ? `${gpuUsage.toFixed(1)}%` : "—";
    els.waveLblVram.textContent = s.gpu ? `${vramPct.toFixed(0)}%` : "—";

    host.drawGraph(els.waveCpu, h.cpu, "#4e4b42", opts);
    host.drawGraph(els.waveRam, h.ram, "#6b5b45", opts);
    host.drawGraph(els.waveGpu, h.gpu, "#4e4b42", opts);
    host.drawGraph(els.waveVram, h.vram, "#7c5846", opts);
    host.drawGraph(els.miniWave, h.cpu, "#4e4b42", { lineWidth: 1.4 });
  }

  const onResize = () => {
    if (lastSample) renderSample(lastSample);
  };
  window.addEventListener("resize", onResize);

  return {
    onSettings(settings) {
      if (!settings) return;
      const nextOv = Math.max(1, Math.min(25, Number(settings.overviewLimit) || OVERVIEW_ROWS));
      const nextPg = Math.max(1, Math.min(60, Number(settings.pageLimit) || MAX_ROWS));
      if (nextOv !== overviewLimit || nextPg !== pageLimit) {
        overviewLimit = nextOv;
        pageLimit = nextPg;
        rebuildAllLists();
        if (lastSample) renderSample(lastSample);
      }
    },

    onInfo(info) {
      els.hostname.textContent = info.hostname || "YoRHa-2B";
      els.coresLv.textContent = `Lv: ${info.cpu.logicalCores}T`;
      els.coresCnt.textContent = info.cpu.logicalCores;
      els.stCpuName.textContent = info.cpu.name;
      els.stGpuName.textContent = info.gpu ? info.gpu.name : "Unavailable";
      buildCoreSquares(info.cpu.logicalCores);
    },

    onStatus(st) {
      if (st.state === "live") {
        els.diagText.textContent = "NO ERROR";
      } else if (st.state === "down") {
        els.diagText.textContent = "SIGNAL LOST — RECONNECTING";
      } else {
        els.diagText.textContent = "INITIALIZING…";
      }
    },

    onSample(s) {
      renderSample(s);
    },

    unmount() {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("resize", onResize);
    },
  };
}
