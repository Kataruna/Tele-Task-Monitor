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

  function makeRows(listEl, count = 15) {
    listEl.textContent = "";
    const out = [];
    out.listEl = listEl;
    for (let i = 0; i < count; i++) {
      const li = document.createElement("li");
      const nameWrap = document.createElement("span");
      nameWrap.className = "name";
      const name = document.createElement("span");
      const cnt = document.createElement("span");
      cnt.className = "count";
      nameWrap.append(name, cnt);
      const value = document.createElement("span");
      value.className = "value";
      const meter = document.createElement("div");
      meter.className = "meter";
      const bar = document.createElement("i");
      meter.appendChild(bar);
      li.append(nameWrap, value, meter);
      li.hidden = true;
      listEl.appendChild(li);
      out.push({ li, name, count: cnt, value, bar });
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
      for (const r of slot) r.li.hidden = true;
      slot.empty.textContent = emptyMsg;
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
      r.count.textContent = e.count > 1 ? `×${e.count}` : "";
      r.value.textContent = fmtValue(e);
      r.bar.style.width = `${Math.max(0, Math.min(100, barPct(e))).toFixed(1)}%`;
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
      const pg = Math.max(1, Math.min(60, Number(settings.pageLimit) || 24));
      rows = {
        cpu: makeRows(els.topCpu, pg),
        mem: makeRows(els.topMem, pg),
        gpu: makeRows(els.topGpu, pg),
      };
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

      renderRows(rows.cpu, s.top.cpu, (e) => `${e.cpu.toFixed(1)}%`, (e) => e.cpu, "—");
      renderRows(
        rows.mem,
        s.top.mem,
        (e) => host.fmtBytes(e.mem),
        (e) => (ramTotal > 0 ? (e.mem / ramTotal) * 100 : 0),
        "—"
      );
      if (s.top.gpu === null) {
        renderRows(rows.gpu, [], () => "", () => 0, "GPU metrics unavailable");
      } else {
        renderRows(
          rows.gpu,
          s.top.gpu,
          (e) => `${(e.gpu ?? 0).toFixed(1)}% · ${host.fmtBytes(e.vram ?? 0)}`,
          (e) => e.gpu ?? 0,
          "All apps idle"
        );
      }

      els.selfUsage.textContent = `${s.self.cpu.toFixed(1)}% CPU, ${host.fmtBytes(s.self.mem)}`;
      drawAllGraphs();
    },

    unmount() {
      window.removeEventListener("resize", onResize);
    },
  };
}
