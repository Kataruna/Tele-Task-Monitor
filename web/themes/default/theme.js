// Classic Dark Task Monitor theme

export function mount(root, host) {
  const $ = (id) => root.querySelector(`#${id}`);

  const els = {
    hostname: $("hostname"),
    cpuName: $("cpu-name"),
    cpuValue: $("cpu-value"),
    ramValue: $("ram-value"),
    ramTotal: $("ram-total"),
    ramPct: $("ram-pct"),
    gpuValue: $("gpu-value"),
    gpuTemp: $("gpu-temp"),
    gpuPower: $("gpu-power"),
    vramValue: $("vram-value"),
    vramTotal: $("vram-total"),
    gpuName: $("gpu-name"),
    cores: $("cores"),
    topCpu: $("top-cpu"),
    topMem: $("top-mem"),
    topGpu: $("top-gpu"),
    statusDot: $("status-dot"),
    statusText: $("status-text"),
    selfUsage: $("self-usage"),
    themeBtn: $("theme-btn"),
    gpuSortBtn: $("default-gpu-sort-btn"),
    graphs: {
      cpu: $("cpu-graph"),
      ram: $("ram-graph"),
      gpu: $("gpu-graph"),
      vram: $("vram-graph"),
    },
  };

  const COLORS = {
    cpu: "#4cc2ff",
    ram: "#b48cff",
    gpu: "#5ee08a",
    vram: "#ffb454",
  };

  let coreFills = [];
  let ramTotal = 0;
  let vramTotal = 0;

  if (els.themeBtn) {
    els.themeBtn.addEventListener("click", (ev) => host.openThemeMenu(els.themeBtn, ev));
  }

  function updateDefaultGpuSortBtn() {
    if (!els.gpuSortBtn) return;
    const isVram = host.getGpuSort() === "vram";
    els.gpuSortBtn.textContent = isVram ? "[VRAM]" : "[%]";
  }

  if (els.gpuSortBtn) {
    els.gpuSortBtn.addEventListener("click", (ev) => {
      ev.stopPropagation();
      host.toggleGpuSort();
      updateDefaultGpuSortBtn();
      if (lastSample) renderAllTopLists(lastSample);
    });
  }

  function buildCores(n) {
    els.cores.textContent = "";
    coreFills = [];
    for (let i = 0; i < n; i++) {
      const box = document.createElement("div");
      box.className = "core";
      const fill = document.createElement("i");
      box.appendChild(fill);
      els.cores.appendChild(box);
      coreFills.push({ box, fill });
    }
  }

  let lastSample = null;

  function makeRows(listEl, count = 15) {
    listEl.textContent = "";
    const out = [];
    out.listEl = listEl;
    for (let i = 0; i < count; i++) {
      const li = document.createElement("li");
      const nameWrap = document.createElement("span");
      nameWrap.className = "name";
      const pinBadge = document.createElement("span");
      pinBadge.className = "default-pin-badge";
      pinBadge.innerHTML = `<svg viewBox="0 0 16 16" width="10" height="10" fill="currentColor" aria-hidden="true"><line x1="8" y1="11.5" x2="8" y2="15" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/><path d="M4.5 2h7l-1 4v3.5l1.5 2H4l1.5-2V6L4.5 2z"/></svg>`;
      pinBadge.hidden = true;
      const name = document.createElement("span");
      const cnt = document.createElement("span");
      cnt.className = "count";
      nameWrap.append(pinBadge, name, cnt);

      const rightWrap = document.createElement("span");
      rightWrap.className = "row-right";
      const value = document.createElement("span");
      value.className = "value";
      rightWrap.appendChild(value);

      const meter = document.createElement("div");
      meter.className = "meter";
      const bar = document.createElement("i");
      meter.appendChild(bar);
      li.append(nameWrap, rightWrap, meter);
      li.hidden = true;

      let entryName = null;
      li.addEventListener("click", (ev) => {
        ev.stopPropagation();
        if (!entryName) return;
        const cur = host.getFocusedProcess();
        if (cur && cur.toLowerCase() === entryName.toLowerCase()) {
          host.setFocusedProcess(null);
        } else {
          host.setFocusedProcess(entryName);
        }
        if (lastSample) renderAllTopLists(lastSample);
      });

      host.bindProcessRow(li, () => entryName);

      listEl.appendChild(li);
      out.push({
        li,
        pinBadge,
        name,
        count: cnt,
        value,
        bar,
        setEntryName(n) {
          entryName = n;
        },
      });
    }
    const empty = document.createElement("li");
    empty.className = "empty";
    empty.hidden = true;
    listEl.appendChild(empty);
    out.empty = empty;
    return out;
  }

  let rows = {
    cpu: makeRows(els.topCpu, 24),
    mem: makeRows(els.topMem, 24),
    gpu: makeRows(els.topGpu, 24),
  };

  function renderRows(slot, entries, fmtValue, barPct, emptyMsg) {
    if (!entries || entries.length === 0) {
      for (const r of slot) {
        r.li.hidden = true;
        r.li.classList.remove("focused", "is-pinned", "pinned-separator");
      }
      slot.empty.textContent = emptyMsg;
      slot.empty.hidden = false;
      return;
    }
    slot.empty.hidden = true;
    const focused = (host.getFocusedProcess() || "").toLowerCase();
    let visibleCount = 0;
    let lastPinnedIdx = -1;
    for (let i = 0; i < slot.length; i++) {
      const r = slot[i];
      const e = entries[i];
      if (!e) {
        r.li.hidden = true;
        r.li.classList.remove("focused", "is-pinned", "pinned-separator");
        r.setEntryName(null);
        continue;
      }
      r.li.hidden = false;
      r.setEntryName(e.name);
      const pinned = Boolean(e._pinned);
      r.pinBadge.hidden = !pinned;
      r.name.textContent = e.name;
      r.count.textContent = e.count > 1 ? `×${e.count}` : "";
      r.value.textContent = fmtValue(e);
      r.bar.style.width = `${Math.max(0, Math.min(100, barPct(e))).toFixed(1)}%`;
      r.li.classList.toggle("is-pinned", pinned);
      r.li.classList.toggle("focused", Boolean(focused) && e.name.toLowerCase() === focused);
      if (pinned) lastPinnedIdx = i;
      visibleCount++;
    }
    const hasNormalAfterPinned = lastPinnedIdx >= 0 && lastPinnedIdx < visibleCount - 1;
    for (let i = 0; i < slot.length; i++) {
      slot[i].li.classList.toggle("pinned-separator", hasNormalAfterPinned && i === lastPinnedIdx);
    }
  }

  function renderAllTopLists(s) {
    const isVram = host.getGpuSort() === "vram";
    updateDefaultGpuSortBtn();

    const prepCpu = host.prepareList(s.top.cpu || [], "cpu");
    const prepMem = host.prepareList(s.top.mem || [], "mem");
    const prepGpu = s.top.gpu === null ? null : host.prepareList(s.top.gpu || [], "gpu");

    const focused = host.getFocusedProcess();
    if (focused) {
      const allNames = [...(s.top.cpu || []), ...(s.top.mem || []), ...(s.top.gpu || [])].map((e) => e.name.toLowerCase());
      if (!allNames.includes(focused.toLowerCase()) && prepCpu[0]) {
        host.setFocusedProcess(prepCpu[0].name);
      }
    }

    renderRows(rows.cpu, prepCpu, (e) => `${e.cpu.toFixed(1)}%`, (e) => e.cpu, "—");
    renderRows(rows.mem, prepMem, (e) => host.fmtBytes(e.mem), (e) => (ramTotal > 0 ? (e.mem / ramTotal) * 100 : 0), "—");
    renderRows(
      rows.gpu,
      prepGpu,
      isVram
        ? (e) => `${host.fmtBytes(e.vram ?? 0)} · ${(e.gpu ?? 0).toFixed(1)}%`
        : (e) => `${(e.gpu ?? 0).toFixed(1)}% · ${host.fmtBytes(e.vram ?? 0)}`,
      (e) => (isVram ? (s.gpu && s.gpu.vramTotal > 0 ? ((e.vram ?? 0) / s.gpu.vramTotal) * 100 : 0) : (e.gpu ?? 0)),
      "—"
    );
  }
    renderRows(
      rows.mem,
      prepMem,
      (e) => host.fmtBytes(e.mem),
      (e) => (ramTotal > 0 ? (e.mem / ramTotal) * 100 : 0),
      "—"
    );
    if (prepGpu === null) {
      renderRows(rows.gpu, [], () => "", () => 0, "GPU metrics unavailable");
    } else {
      renderRows(
        rows.gpu,
        prepGpu,
        (e) => `${(e.gpu ?? 0).toFixed(1)}% · ${host.fmtBytes(e.vram ?? 0)}`,
        (e) => e.gpu ?? 0,
        "All apps idle"
      );
    }
  }

  function drawAllGraphs() {
    const h = host.getState().history;
    for (const k of ["cpu", "ram", "gpu", "vram"]) {
      host.drawGraph(els.graphs[k], h[k], COLORS[k]);
    }
  }

  const onResize = () => drawAllGraphs();
  window.addEventListener("resize", onResize);

  return {
    onSettings(settings) {
      if (!settings) return;
      const pinExtra = (settings.pinnedProcesses || []).length;
      const pg = Math.max(1, Math.min(60, Number(settings.pageLimit) || 24)) + pinExtra;
      rows = {
        cpu: makeRows(els.topCpu, pg),
        mem: makeRows(els.topMem, pg),
        gpu: makeRows(els.topGpu, pg),
      };
      if (lastSample) renderAllTopLists(lastSample);
    },

    onInfo(info) {
      els.hostname.textContent = info.hostname;
      els.cpuName.textContent = `${info.cpu.name} (${info.cpu.logicalCores}T)`;
      ramTotal = info.ram.total;
      els.ramTotal.textContent = `/ ${(ramTotal / 1073741824).toFixed(0)} GB`;
      if (info.gpu) {
        vramTotal = info.gpu.vramTotal;
        els.vramTotal.textContent = `/ ${(vramTotal / 1073741824).toFixed(0)} GB`;
        els.gpuName.textContent = info.gpu.name;
      } else {
        els.vramTotal.textContent = "";
        els.gpuName.textContent = "Unavailable";
      }
      buildCores(info.cpu.logicalCores);
    },

    onStatus(st) {
      els.statusDot.className = `dot ${st.state}`;
      els.statusText.textContent = st.text;
    },

    onSample(s) {
      lastSample = s;
      els.cpuValue.textContent = s.cpu.usage.toFixed(1);
      const ramPct = s.ram.total > 0 ? (s.ram.used / s.ram.total) * 100 : 0;
      els.ramValue.textContent = (s.ram.used / 1073741824).toFixed(1);
      els.ramPct.textContent = `${ramPct.toFixed(0)}% used`;

      if (s.gpu) {
        els.gpuValue.textContent = s.gpu.usage.toFixed(1);
        els.gpuTemp.textContent = s.gpu.tempC != null ? `${Math.round(s.gpu.tempC)}°C` : "—";
        els.gpuPower.textContent = s.gpu.powerPct != null ? `${Math.round(s.gpu.powerPct)}% power` : "—";
        els.vramValue.textContent = (s.gpu.vramUsed / 1073741824).toFixed(1);
      } else {
        els.gpuValue.textContent = "—";
        els.gpuTemp.textContent = "—";
        els.gpuPower.textContent = "—";
        els.vramValue.textContent = "—";
      }

      if (coreFills.length !== s.cpu.cores.length) buildCores(s.cpu.cores.length);
      for (let i = 0; i < s.cpu.cores.length; i++) {
        const v = s.cpu.cores[i];
        coreFills[i].fill.style.height = `${v.toFixed(1)}%`;
        coreFills[i].box.classList.toggle("hot", v >= 90);
      }

      renderAllTopLists(s);

      els.selfUsage.textContent = `${s.self.cpu.toFixed(1)}% CPU, ${host.fmtBytes(s.self.mem)}`;
      drawAllGraphs();
    },

    unmount() {
      window.removeEventListener("resize", onResize);
    },
  };
}
