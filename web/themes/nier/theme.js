// Automata Theme — Inspired by NieR:Automata YoRHa UI ("OVERVIEW - All-System Telemetry Brief")

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
    pinFocusedBtn: $("nier-pin-focused-btn"),
    pinFocusedLbl: $("nier-pin-focused-lbl"),
    hideFocusedBtn: $("nier-hide-focused-btn"),
    hiddenBtn: $("nier-hidden-btn"),
    hiddenCnt: $("nier-hidden-cnt"),
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
    ovGpuSortBtn: $("nier-ov-gpu-sort"),
    singleGpuSortBtn: $("nier-single-gpu-sort"),
    intelGpuSortBtn: $("nier-intel-gpu-sort"),
    chipList: $("nier-chip-list"),
    sbThumb: root.querySelector(".nier-scrollbar .sb-thumb"),
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
      else if (tabId === "gpu") els.sortName.textContent = "GPU";
    }

    updateGpuSortLabels();
    if (lastSample) renderSample(lastSample);
  }

  function updateGpuSortLabels(sort = null) {
    const cur = sort || host.getGpuSort();
    const isVram = cur === "vram";
    if (els.ovGpuSortBtn) els.ovGpuSortBtn.textContent = isVram ? "[VRAM]" : "[%]";
    if (els.singleGpuSortBtn) {
      els.singleGpuSortBtn.textContent = isVram ? "[Sort: VRAM]" : "[Sort: %]";
      els.singleGpuSortBtn.hidden = activeTab !== "gpu";
    }
    if (els.intelGpuSortBtn) els.intelGpuSortBtn.textContent = isVram ? "[Sort: VRAM]" : "[Sort: %]";
  }

  const onToggleGpuSort = (ev) => {
    if (ev) {
      ev.stopPropagation();
      ev.preventDefault();
    }
    const next = host.toggleGpuSort();
    updateGpuSortLabels(next);
    if (lastSample) renderSample(lastSample);
  };
  if (els.ovGpuSortBtn) els.ovGpuSortBtn.addEventListener("click", onToggleGpuSort);
  if (els.singleGpuSortBtn) els.singleGpuSortBtn.addEventListener("click", onToggleGpuSort);
  if (els.intelGpuSortBtn) els.intelGpuSortBtn.addEventListener("click", onToggleGpuSort);

  for (const b of tabBtns) {
    b.addEventListener("click", () => setTab(b.dataset.tab));
  }
  for (const box of root.querySelectorAll("[data-jump]")) {
    box.addEventListener("click", () => setTab(box.dataset.jump));
  }

  if (els.titleBack) els.titleBack.addEventListener("click", () => setTab("overview"));
  if (els.inlineBack) els.inlineBack.addEventListener("click", () => setTab("overview"));
  if (els.backBtn) els.backBtn.addEventListener("click", () => setTab("overview"));

  if (els.pinFocusedBtn) {
    els.pinFocusedBtn.addEventListener("click", () => {
      const target = getEffectiveFocusedName();
      if (target) host.togglePin(target);
    });
  }
  if (els.hideFocusedBtn) {
    els.hideFocusedBtn.addEventListener("click", () => {
      const target = getEffectiveFocusedName();
      if (target) host.hideProcess(target);
    });
  }
  if (els.hiddenBtn) {
    els.hiddenBtn.addEventListener("click", (ev) => host.openSettings(ev));
  }

  els.tabBtn.addEventListener("click", () => {
    const next = TABS[(TABS.indexOf(activeTab) + 1) % TABS.length];
    setTab(next);
  });

  els.themeBtn.addEventListener("click", (ev) => host.openThemeMenu(els.themeBtn, ev));

  function updateCustomScrollbar() {
    if (!els.chipList || !els.sbThumb) return;
    const { scrollTop, scrollHeight, clientHeight } = els.chipList;
    if (scrollHeight <= clientHeight || scrollHeight === 0) {
      els.sbThumb.style.height = "100%";
      els.sbThumb.style.top = "0%";
      return;
    }
    const thumbPct = Math.max(15, Math.min(100, (clientHeight / scrollHeight) * 100));
    const maxTop = 100 - thumbPct;
    const scrollRatio = scrollTop / Math.max(1, scrollHeight - clientHeight);
    els.sbThumb.style.height = `${thumbPct.toFixed(1)}%`;
    els.sbThumb.style.top = `${(scrollRatio * maxTop).toFixed(1)}%`;
  }

  if (els.chipList) {
    els.chipList.addEventListener("scroll", updateCustomScrollbar, { passive: true });
  }

  function getActiveStackEntries() {
    if (!lastSample) return [];
    if (activeTab === "mem") return host.prepareList(lastSample.top.mem || [], "mem");
    if (activeTab === "gpu") return host.prepareList(lastSample.top.gpu || [], "gpu");
    return host.prepareList(lastSample.top.cpu || [], "cpu");
  }

  function getEffectiveFocusedName() {
    const explicit = host.getFocusedProcess();
    if (explicit) return explicit;
    const list = getActiveStackEntries();
    return list[0] ? list[0].name : null;
  }

  function stepFocus(delta) {
    const list = getActiveStackEntries();
    if (list.length === 0) return;
    const curName = host.getFocusedProcess();
    let curIdx = curName
      ? list.findIndex((e) => e.name.toLowerCase() === curName.toLowerCase())
      : 0;
    if (curIdx < 0) curIdx = 0;
    const nextIdx = Math.max(0, Math.min(list.length - 1, curIdx + delta));
    const nextEntry = list[nextIdx];
    if (nextEntry) {
      host.setFocusedProcess(nextEntry.name);
      if (lastSample) renderSample(lastSample);
      const rowObj = singleRows[nextIdx];
      if (rowObj && rowObj.li && !rowObj.li.hidden) {
        rowObj.li.scrollIntoView({ block: "nearest" });
      }
    }
  }

  const onKeyDown = (ev) => {
    if (ev.target && (ev.target.tagName === "INPUT" || ev.target.tagName === "SELECT")) return;
    if (ev.key === "Escape" || ev.key === "Backspace") {
      if (activeTab !== "overview") setTab("overview");
    } else if (ev.key === "ArrowUp" || ev.key === "w" || ev.key === "W") {
      ev.preventDefault();
      stepFocus(-1);
    } else if (ev.key === "ArrowDown" || ev.key === "s" || ev.key === "S") {
      ev.preventDefault();
      stepFocus(1);
    } else if (ev.key === "p" || ev.key === "P") {
      const target = getEffectiveFocusedName();
      if (target) host.togglePin(target);
    } else if (ev.key === "h" || ev.key === "H") {
      const target = getEffectiveFocusedName();
      if (target) host.hideProcess(target);
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

  function makeChipRows(listEl, count = MAX_ROWS) {
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
      const pinBadge = document.createElement("span");
      pinBadge.className = "chip-pin-badge";
      pinBadge.innerHTML = `<svg viewBox="0 0 16 16" width="10" height="10" fill="currentColor" aria-hidden="true"><line x1="8" y1="11.5" x2="8" y2="15" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/><path d="M4.5 2h7l-1 4v3.5l1.5 2H4l1.5-2V6L4.5 2z"/></svg>`;
      pinBadge.hidden = true;
      const name = document.createElement("span");
      name.className = "chip-name";
      const cnt = document.createElement("span");
      cnt.className = "chip-count";
      const diamond = document.createElement("span");
      diamond.className = "chip-diamond";
      diamond.textContent = "◆";
      left.append(icon, pinBadge, name, cnt, diamond);

      const right = document.createElement("span");
      right.className = "chip-right";
      const cost = document.createElement("span");
      cost.className = "chip-cost";
      right.append(cost);

      li.append(left, right);
      li.hidden = true;

      let rowEntryName = null;
      li.addEventListener("click", (ev) => {
        ev.stopPropagation();
        if (!rowEntryName) return;
        const cur = host.getFocusedProcess();
        if (cur && cur.toLowerCase() === rowEntryName.toLowerCase()) {
          host.setFocusedProcess(null);
        } else {
          host.setFocusedProcess(rowEntryName);
        }
        if (lastSample) renderSample(lastSample);
      });

      host.bindProcessRow(li, () => rowEntryName);

      listEl.appendChild(li);
      out.push({
        li,
        pinBadge,
        name,
        count: cnt,
        cost,
        setEntryName(n) {
          rowEntryName = n;
        },
      });
    }
    return out;
  }

  let ovRows = {};
  let singleRows = [];
  let intelRows = {};
  let stackSlices = [];

  function rebuildAllLists() {
    const pinnedExtra = (host.getSettings().pinnedProcesses || []).length;
    const ovCap = overviewLimit + pinnedExtra;
    const pgCap = pageLimit + pinnedExtra;

    ovRows = {
      cpu: makeChipRows(els.ovListCpu, ovCap),
      mem: makeChipRows(els.ovListMem, ovCap),
      gpu: makeChipRows(els.ovListGpu, ovCap),
    };
    singleRows = makeChipRows(els.chipList, pgCap);
    intelRows = {
      cpu: makeChipRows(els.intelCpu, pgCap),
      mem: makeChipRows(els.intelMem, pgCap),
      gpu: makeChipRows(els.intelGpu, pgCap),
    };

    els.stackBars.textContent = "";
    stackSlices = [];
    const spacerTop = document.createElement("div");
    spacerTop.style.flex = "1";
    els.stackBars.appendChild(spacerTop);
    for (let i = 0; i < pgCap + 1; i++) {
      const div = document.createElement("div");
      div.className = "stack-slice c0";
      const lbl = document.createElement("span");
      lbl.className = "slice-lbl";
      div.appendChild(lbl);
      div.addEventListener("click", () => {
        if (div.dataset.jump) {
          setTab(div.dataset.jump);
        } else if (div.dataset.procName) {
          host.setFocusedProcess(div.dataset.procName);
          if (lastSample) renderSample(lastSample);
        }
      });
      els.stackBars.appendChild(div);
      stackSlices.push({ el: div, lbl });
    }
  }

  rebuildAllLists();

  function resolveFocusedIndex(entries, visibleCount, isPrimaryActive = false) {
    if (visibleCount <= 0 || !entries || entries.length === 0) return -1;
    const focused = host.getFocusedProcess();
    if (focused) {
      const k = focused.toLowerCase();
      for (let i = 0; i < visibleCount; i++) {
        if (entries[i] && entries[i].name.toLowerCase() === k) {
          return i;
        }
      }
      if (isPrimaryActive && entries[0]) {
        host.setFocusedProcess(entries[0].name);
        return 0;
      }
      return -1;
    }
    return -1;
  }

  function fillChipRows(rows, entries, fmtCost, isPrimaryActive = false) {
    let visibleCount = 0;
    let lastPinnedIdx = -1;
    for (let i = 0; i < rows.length; i++) {
      const r = rows[i];
      const e = entries ? entries[i] : null;
      if (!e) {
        r.li.hidden = true;
        r.li.classList.remove("selected", "is-pinned", "pinned-separator");
        r.setEntryName(null);
        continue;
      }
      r.li.hidden = false;
      r.setEntryName(e.name);
      r.name.textContent = e.name;
      r.count.textContent = e.count > 1 ? `×${e.count}` : "";
      r.cost.textContent = `【${fmtCost(e)}】`;
      const pinned = Boolean(e._pinned);
      r.pinBadge.hidden = !pinned;
      r.li.classList.toggle("is-pinned", pinned);
      if (pinned) lastPinnedIdx = i;
      visibleCount++;
    }

    const hasNormalAfterPinned = lastPinnedIdx >= 0 && lastPinnedIdx < visibleCount - 1;
    const activeIdx = resolveFocusedIndex(entries, visibleCount, isPrimaryActive);
    for (let i = 0; i < rows.length; i++) {
      rows[i].li.classList.toggle("selected", activeIdx >= 0 && i === activeIdx);
      rows[i].li.classList.toggle("pinned-separator", hasNormalAfterPinned && i === lastPinnedIdx);
    }
    return { visibleCount, activeIdx };
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

    const prepCpu = host.prepareList(s.top.cpu || [], "cpu");
    const prepMem = host.prepareList(s.top.mem || [], "mem");
    const prepGpu = host.prepareList(s.top.gpu || [], "gpu");

    const gpuSort = host.getGpuSort();
    const showSecondary = host.getSecondaryMetrics();

    // Overview Left Brief Boxes
    els.ovHeadCpu.textContent = `${s.cpu.usage.toFixed(1)}% Total`;
    els.ovHeadMem.textContent = `${ramUsedGB} / ${ramTotalGB} GB`;
    els.ovHeadGpu.textContent = s.gpu
      ? (gpuSort === "vram" ? `${vramUsedGB} / ${vramTotalGB} GB` : `${s.gpu.usage.toFixed(1)}% · ${vramUsedGB}G`)
      : "Unavailable";

    fillChipRows(ovRows.cpu, prepCpu, (e) => `${e.cpu.toFixed(1)}%`, activeTab === "overview");
    fillChipRows(ovRows.mem, prepMem, (e) => host.fmtBytes(e.mem), false);
    fillChipRows(
      ovRows.gpu,
      prepGpu,
      (e) => (gpuSort === "vram" ? `${host.fmtBytes(e.vram ?? 0)} · ${(e.gpu ?? 0).toFixed(1)}%` : `${(e.gpu ?? 0).toFixed(1)}% · ${host.fmtBytes(e.vram ?? 0)}`),
      false
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

    // Populate Intel all-3 view
    fillChipRows(intelRows.cpu, prepCpu, (e) => `${e.cpu.toFixed(1)}%`, activeTab === "intel");
    fillChipRows(intelRows.mem, prepMem, (e) => host.fmtBytes(e.mem), false);
    fillChipRows(
      intelRows.gpu,
      prepGpu,
      (e) => (gpuSort === "vram" ? `${host.fmtBytes(e.vram ?? 0)} · ${(e.gpu ?? 0).toFixed(1)}%` : `${(e.gpu ?? 0).toFixed(1)}% · ${host.fmtBytes(e.vram ?? 0)}`),
      false
    );

    // Populate Single-Resource view & Central Capacity Bar
    let stackList = prepCpu;
    let fmtSingle = showSecondary
      ? (e) => `${e.cpu.toFixed(1)}% · ${host.fmtBytes(e.mem)}`
      : (e) => `${e.cpu.toFixed(1)}%`;

    if (activeTab === "mem") {
      stackList = prepMem;
      fmtSingle = showSecondary
        ? (e) => `${host.fmtBytes(e.mem)} · ${e.cpu.toFixed(1)}%`
        : (e) => `${host.fmtBytes(e.mem)}`;
    } else if (activeTab === "gpu") {
      stackList = prepGpu;
      fmtSingle = (e) =>
        gpuSort === "vram"
          ? `${host.fmtBytes(e.vram ?? 0)}${showSecondary ? ` · ${(e.gpu ?? 0).toFixed(1)}%` : ""}`
          : `${(e.gpu ?? 0).toFixed(1)}%${showSecondary ? ` · ${host.fmtBytes(e.vram ?? 0)}` : ""}`;
    }

    const isSingleActive = activeTab === "cpu" || activeTab === "mem" || activeTab === "gpu";
    const { visibleCount: fitCount, activeIdx } = fillChipRows(singleRows, stackList, fmtSingle, isSingleActive);
    updateCustomScrollbar();

    renderCentralStack(s, ramPct, gpuUsage, vramPct, stackList, fitCount, activeIdx);

    // Update Footer Focused & Hidden Prompts
    const focusedName = getEffectiveFocusedName();
    if (els.pinFocusedBtn && els.hideFocusedBtn) {
      const hasTarget = Boolean(focusedName);
      els.pinFocusedBtn.hidden = !hasTarget;
      els.hideFocusedBtn.hidden = !hasTarget;
      if (hasTarget && els.pinFocusedLbl) {
        els.pinFocusedLbl.textContent = host.isPinned(focusedName) ? "Unpin" : "Pin";
      }
    }
    if (els.hiddenBtn && els.hiddenCnt) {
      const hidLen = (host.getSettings().hiddenProcesses || []).length;
      els.hiddenBtn.hidden = hidLen === 0;
      els.hiddenCnt.textContent = String(hidLen);
    }

    const c0 = prepCpu[0] ? `${prepCpu[0].name} (${prepCpu[0].cpu.toFixed(1)}%)` : "—";
    const m0 = prepMem[0] ? `${prepMem[0].name} (${host.fmtBytes(prepMem[0].mem)})` : "—";
    const g0 = prepGpu[0] ? `${prepGpu[0].name} (${(prepGpu[0].gpu ?? 0).toFixed(1)}%)` : "—";
    const focusInfo = host.getFocusedProcess() ? `Focused: ${host.getFocusedProcess()}  |  ` : "";
    els.descText.textContent = `${focusInfo}Top CPU: ${c0} · Top Mem: ${m0} · Top GPU: ${g0}  |  Monitor: ${s.self.cpu.toFixed(1)}% CPU, ${host.fmtBytes(s.self.mem)}`;

    drawWaves(s, ramPct, gpuUsage, vramPct);
  }

  function shortProcName(name) {
    return (name || "").replace(/\.exe$/i, "");
  }

  function renderCentralStack(s, ramPct, gpuUsage, vramPct, stackList, fitCount, activeIdx) {
    // Reset all slices first
    for (const item of stackSlices) {
      item.el.style.height = "0%";
      item.el.classList.remove("active-slice");
      item.lbl.textContent = "";
      delete item.el.dataset.jump;
      delete item.el.dataset.procName;
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
        const useVram = host.getGpuSort() === "vram";
        totalTargetPct = useVram ? vramPct : gpuUsage;
        valOf = (e) =>
          useVram
            ? ((e.vram ?? 0) / Math.max(1, s.gpu ? s.gpu.vramTotal : 1)) * 100
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

      const count = Math.min(fitCount || stackList.length, stackSlices.length - 1);
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
        if (h < 0.35 && i !== activeIdx && !stackList[i]._pinned) continue;
        const effH = Math.max(1.2, h);
        usedSum += effH;
        const tier = Math.min(3, Math.floor((i / Math.max(1, count)) * 4));
        items.push({
          cls: `c${tier}`,
          h: effH,
          lbl: effH >= 3.4 ? lblOf(stackList[i]) : "",
          procName: stackList[i].name,
          active: i === activeIdx,
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

    let hasActiveSlice = false;
    for (let i = 0; i < items.length && i < stackSlices.length; i++) {
      const it = items[i];
      const slot = stackSlices[i];
      slot.el.className = `stack-slice ${it.cls}${it.active ? " active-slice" : ""}`;
      slot.el.style.height = `${it.h.toFixed(2)}%`;
      slot.lbl.textContent = it.lbl || "";
      if (it.jump) slot.el.dataset.jump = it.jump;
      if (it.procName) slot.el.dataset.procName = it.procName;
      if (it.active) {
        pointerTopPct = 100 - totalStackPct + cumFromTopOfStack + it.h / 2;
        hasActiveSlice = true;
      }
      cumFromTopOfStack += it.h;
    }

    if (els.stackPointer) {
      if (activeTab === "overview" || hasActiveSlice) {
        els.stackPointer.style.display = "flex";
        els.stackPointer.style.top = `${Math.max(8, Math.min(96, pointerTopPct)).toFixed(1)}%`;
      } else {
        els.stackPointer.style.display = "none";
      }
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
      overviewLimit = Math.max(1, Math.min(25, Number(settings.overviewLimit) || OVERVIEW_ROWS));
      pageLimit = Math.max(1, Math.min(60, Number(settings.pageLimit) || MAX_ROWS));
      rebuildAllLists();
      if (lastSample) renderSample(lastSample);
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
