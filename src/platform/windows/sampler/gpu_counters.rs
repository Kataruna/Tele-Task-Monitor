//! GPU usage from the Windows "GPU Engine" / "GPU Process Memory" / "GPU Adapter Memory"
//! performance counters (the same source Task Manager uses).

use std::collections::HashMap;

use windows::Win32::System::Performance::{
    PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_FMT_LARGE, PDH_HCOUNTER, PDH_HQUERY, PDH_MORE_DATA,
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW, PdhOpenQueryW,
};
use windows::core::{PCWSTR, w};

use crate::aggregate::{ProcGpu, round1};
use super::gpu_adapter::GpuAdapter;
use crate::sampler::gpu::GpuReading;

/// `pid_1234_luid_..._eng_0_engtype_3D` -> `Some(1234)`.
pub fn parse_pid(instance: &str) -> Option<u32> {
    instance.strip_prefix("pid_")?.split('_').next()?.parse().ok()
}

/// `pid_1234_luid_0x0_0x4B7D703F_phys_0_eng_3_engtype_Copy` -> `Some("luid_0x0_0x4B7D703F_phys_0_eng_3")`.
/// Identifies one physical engine; summing over PIDs per key gives that engine's utilisation.
pub fn engine_key(instance: &str) -> Option<&str> {
    let start = instance.find("luid_")?;
    let end = instance.find("_engtype_")?;
    (start < end).then(|| &instance[start..end])
}

/// Check whether a PDH instance belongs to `luid_filter` (e.g. `"luid_0x00000000_0x4b7d703f"`).
/// When `luid_filter` is `None`, all instances match.
fn matches_luid(instance: &str, luid_filter: Option<&str>) -> bool {
    let Some(tag) = luid_filter else {
        return true;
    };
    let Some(start) = instance.find("luid_") else {
        return false;
    };
    let rest = &instance[start..];
    rest.get(..tag.len()).is_some_and(|sub| sub.eq_ignore_ascii_case(tag))
        && rest.as_bytes().get(tag.len()).is_none_or(|&b| b == b'_')
}

/// Decodes PDH instance names (UTF-16) into a reused buffer. The names are ASCII
/// (`pid_1234_luid_0x…_eng_0_engtype_3D`), so the common path allocates nothing; a name with any
/// other character takes the exact lossy path.
#[derive(Default)]
pub struct NameBuf {
    ascii: Vec<u8>,
    other: String,
}

impl NameBuf {
    pub fn decode(&mut self, wide: &[u16]) -> &str {
        self.ascii.clear();
        for &u in wide {
            if u >= 0x80 {
                self.other = String::from_utf16_lossy(wide);
                return &self.other;
            }
            self.ascii.push(u as u8);
        }
        std::str::from_utf8(&self.ascii).unwrap_or("")
    }
}

/// Streaming reduction of the three PDH counters into a [`GpuReading`]. Feed it one instance at a
/// time (borrowed name, no per-instance allocation), then call [`Reducer::finish`].
/// - `engine`: `\GPU Engine(*)\Utilization Percentage`
/// - `process_memory`: `\GPU Process Memory(*)\Dedicated Usage`
/// - `adapter_memory`: `\GPU Adapter Memory(*)\Dedicated Usage`
///
/// Instances reporting 0 add nothing to a sum or a maximum, so they are ignored without even
/// parsing their name. Most engine instances are idle, so this is the bulk of the saving.
///
/// - `luid_filter`: optional `"luid_0xHHHHHHHH_0xLLLLLLLL"` of the primary hardware GPU so
///   virtual display drivers and software renderers (WARP) do not pollute GPU metrics.
/// - `vram_cap`: that GPU's dedicated memory capacity in bytes, `0` if unknown. Adapter usage
///   is held at or below it.
pub struct Reducer<'a> {
    luid_filter: Option<&'a str>,
    vram_cap: u64,
    per_engine: HashMap<String, f64>,
    per_pid: HashMap<u32, ProcGpu>,
    vram_used: u64,
    key: String,
}

impl<'a> Reducer<'a> {
    /// `pid_hint`: expected number of PIDs (last tick's count) to size the map once.
    pub fn new(luid_filter: Option<&'a str>, vram_cap: u64, pid_hint: usize) -> Self {
        Self {
            luid_filter,
            vram_cap,
            per_engine: HashMap::new(),
            per_pid: HashMap::with_capacity(pid_hint),
            vram_used: 0,
            key: String::new(),
        }
    }

    pub fn engine(&mut self, name: &str, value: f64) {
        let v = value.max(0.0);
        if v == 0.0 || !matches_luid(name, self.luid_filter) {
            return;
        }
        if let Some(engine) = engine_key(name) {
            // Case-insensitive key built in a reused buffer; only a new engine allocates.
            self.key.clear();
            self.key.extend(engine.chars().map(|c| c.to_ascii_lowercase()));
            match self.per_engine.get_mut(self.key.as_str()) {
                Some(sum) => *sum += v,
                None => {
                    self.per_engine.insert(self.key.clone(), v);
                }
            }
        }
        if let Some(pid) = parse_pid(name) {
            let e = self.per_pid.entry(pid).or_default();
            e.gpu = e.gpu.max(v as f32);
        }
    }

    pub fn process_memory(&mut self, name: &str, value: f64) {
        let bytes = value.max(0.0) as u64;
        if bytes == 0 || !matches_luid(name, self.luid_filter) {
            return;
        }
        if let Some(pid) = parse_pid(name) {
            let e = self.per_pid.entry(pid).or_default();
            e.vram = e.vram.saturating_add(bytes);
        }
    }

    pub fn adapter_memory(&mut self, name: &str, value: f64) {
        let bytes = value.max(0.0) as u64;
        if bytes == 0 || !matches_luid(name, self.luid_filter) {
            return;
        }
        self.vram_used = self.vram_used.saturating_add(bytes);
    }

    pub fn finish(self) -> GpuReading {
        let Self { vram_cap, mut per_pid, per_engine, vram_used, .. } = self;
        for g in per_pid.values_mut() {
            g.gpu = round1(g.gpu.clamp(0.0, 100.0));
        }
        let usage = per_engine.values().copied().fold(0.0f64, f64::max).clamp(0.0, 100.0) as f32;
        let vram_used = if vram_cap > 0 { vram_used.min(vram_cap) } else { vram_used };
        // `vram_used == 0` means the adapter counter returned nothing this tick (a live desktop
        // always holds some VRAM), so it cannot serve as a bound; fall back to the capacity.
        let app_vram_cap = if vram_used > 0 { Some(vram_used) } else { (vram_cap > 0).then_some(vram_cap) };
        GpuReading { usage: round1(usage), vram_used, per_pid, app_vram_cap }
    }
}

/// Reduce whole counter arrays (instance name, value) into a reading; the slice form of
/// [`Reducer`], kept for tests.
#[cfg(test)]
pub fn reduce(
    engine: &[(String, f64)],
    proc_mem: &[(String, f64)],
    adapter_mem: &[(String, f64)],
    luid_filter: Option<&str>,
    vram_cap: u64,
) -> GpuReading {
    let mut r = Reducer::new(luid_filter, vram_cap, 0);
    for (name, v) in engine {
        r.engine(name, *v);
    }
    for (name, v) in proc_mem {
        r.process_memory(name, *v);
    }
    for (name, v) in adapter_mem {
        r.adapter_memory(name, *v);
    }
    r.finish()
}

/// Owns the PDH query. Not `Send`: create and use it on the sampler thread only.
pub struct GpuCounters {
    query: PDH_HQUERY,
    engine: PDH_HCOUNTER,
    proc_mem: PDH_HCOUNTER,
    adapter_mem: PDH_HCOUNTER,
    luid_filter: Option<String>,
    vram_cap: u64,
    /// PDH output buffer, reused by all three counters (each is consumed before the next call).
    buf: Vec<u64>,
    /// Largest `buf` length seen: `release` frees the memory, this pre-sizes it again on wake.
    buf_hint: usize,
    /// PIDs seen last tick, to size the per-PID map once.
    pid_hint: usize,
    names: NameBuf,
}

impl GpuCounters {
    /// `adapter` is the GPU to report on; its LUID filters the counters and its capacity bounds
    /// the readings, so both always describe the same GPU. `None` reads every adapter, unbounded.
    pub fn open(adapter: Option<&GpuAdapter>) -> Result<Self, String> {
        let mut query = PDH_HQUERY::default();
        // SAFETY: valid out-pointer; null data source = live data.
        let s = unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut query) };
        if s != 0 {
            return Err(format!("PdhOpenQueryW failed: {s:#x}"));
        }
        let mut me = Self {
            query,
            engine: PDH_HCOUNTER::default(),
            proc_mem: PDH_HCOUNTER::default(),
            adapter_mem: PDH_HCOUNTER::default(),
            luid_filter: adapter.map(|a| a.luid_tag.clone()),
            vram_cap: adapter.map_or(0, |a| a.vram_cap),
            buf: Vec::new(),
            buf_hint: 0,
            pid_hint: 0,
            names: NameBuf::default(),
        };
        for (path, slot) in [
            (w!("\\GPU Engine(*)\\Utilization Percentage"), &mut me.engine),
            (w!("\\GPU Process Memory(*)\\Dedicated Usage"), &mut me.proc_mem),
            (w!("\\GPU Adapter Memory(*)\\Dedicated Usage"), &mut me.adapter_mem),
        ] {
            // SAFETY: query is open; slot is a valid out-pointer.
            let s = unsafe { PdhAddEnglishCounterW(query, path, 0, slot) };
            if s != 0 {
                return Err(format!("PdhAddEnglishCounterW failed: {s:#x}"));
            }
        }
        // First collection: rate counters need two samples before they have data.
        // SAFETY: query is open.
        unsafe { PdhCollectQueryData(query) };
        Ok(me)
    }

    /// Take a baseline sample without formatting anything. Formatting thousands of instances is
    /// the expensive part of a read, and a baseline only needs the raw collection.
    pub fn collect(&mut self) {
        // SAFETY: query is open.
        unsafe { PdhCollectQueryData(self.query) };
    }

    /// Free the scratch memory (the next `read` re-allocates it at its previous size).
    pub fn release(&mut self) {
        self.buf = Vec::new();
        self.pid_hint = 0;
    }

    /// Collect a new sample. Utilisation covers the time since the previous call.
    pub fn read(&mut self) -> Result<GpuReading, String> {
        // SAFETY: query is open.
        let s = unsafe { PdhCollectQueryData(self.query) };
        if s != 0 {
            return Err(format!("PdhCollectQueryData failed: {s:#x}"));
        }
        if self.buf.is_empty() {
            self.buf.resize(self.buf_hint, 0);
        }
        let mut acc = Reducer::new(self.luid_filter.as_deref(), self.vram_cap, self.pid_hint);
        let names = &mut self.names;
        // Zero-valued instances (idle engines are the large majority) are skipped before the
        // name is decoded.
        Self::visit(&mut self.buf, self.engine, false, |wide, v| {
            if v > 0.0 {
                acc.engine(names.decode(wide), v);
            }
        });
        Self::visit(&mut self.buf, self.proc_mem, true, |wide, v| {
            if v > 0.0 {
                acc.process_memory(names.decode(wide), v);
            }
        });
        Self::visit(&mut self.buf, self.adapter_mem, true, |wide, v| {
            if v > 0.0 {
                acc.adapter_memory(names.decode(wide), v);
            }
        });
        self.buf_hint = self.buf_hint.max(self.buf.len());
        let reading = acc.finish();
        self.pid_hint = reading.per_pid.len();
        Ok(reading)
    }

    /// Format a wildcard counter and call `f(instance name as UTF-16, value)` for every valid
    /// instance. Any error visits nothing (e.g. "no data yet" right after start), never a failure
    /// of the whole tick.
    fn visit(buf: &mut Vec<u64>, counter: PDH_HCOUNTER, large: bool, mut f: impl FnMut(&[u16], f64)) {
        let format = if large { PDH_FMT_LARGE } else { PDH_FMT_DOUBLE };
        loop {
            let mut bytes = (buf.len() * 8) as u32;
            let mut count = 0u32;
            let items = if buf.is_empty() { None } else { Some(buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W) };
            // SAFETY: buffer (if any) is `bytes` long and 8-byte aligned.
            let s = unsafe { PdhGetFormattedCounterArrayW(counter, format, &mut bytes, &mut count, items) };
            if s == PDH_MORE_DATA {
                // Exactly what PDH asked for plus 12.5% for processes that appear meanwhile.
                // (Doubling and zero-filling would commit memory that is never used.)
                let needed = bytes as usize / 8;
                buf.resize((needed + needed / 8 + 1024).max(buf.len() + 1024), 0);
                continue;
            }
            if s != 0 || count == 0 || buf.is_empty() {
                return;
            }
            // SAFETY: PDH wrote `count` items at the start of buf; names point into buf.
            let items = unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const PDH_FMT_COUNTERVALUE_ITEM_W, count as usize) };
            for it in items {
                if it.FmtValue.CStatus > 1 {
                    continue; // only PDH_CSTATUS_VALID_DATA (0) or NEW_DATA (1)
                }
                // SAFETY: union member matches the requested format.
                let v = unsafe { if large { it.FmtValue.Anonymous.largeValue as f64 } else { it.FmtValue.Anonymous.doubleValue } };
                // SAFETY: szName, when non-null, is a NUL-terminated string inside buf.
                let name: &[u16] = if it.szName.is_null() { &[] } else { unsafe { it.szName.as_wide() } };
                f(name, v);
            }
            return;
        }
    }
}

impl Drop for GpuCounters {
    fn drop(&mut self) {
        // SAFETY: query was opened in `open`.
        unsafe {
            PdhCloseQuery(self.query);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregate::ProcGpu;

    fn s(name: &str, v: f64) -> (String, f64) {
        (name.to_string(), v)
    }

    #[test]
    fn parses_pid() {
        assert_eq!(parse_pid("pid_15332_luid_0x00000000_0x4B7D703F_phys_0_eng_0_engtype_3D"), Some(15332));
        assert_eq!(parse_pid("pid_7_luid_0x00000000_0x4B7D703F_phys_0"), Some(7));
        assert_eq!(parse_pid("luid_0x00000000_0x4B7D703F_phys_0"), None);
        assert_eq!(parse_pid("pid_abc_luid"), None);
        assert_eq!(parse_pid(""), None);
    }

    #[test]
    fn extracts_engine_key() {
        assert_eq!(
            engine_key("pid_1_luid_0x00000000_0x4B7D703F_phys_0_eng_3_engtype_Copy"),
            Some("luid_0x00000000_0x4B7D703F_phys_0_eng_3")
        );
        assert_eq!(engine_key("pid_1_luid_0x0_phys_0"), None);
        assert_eq!(engine_key("garbage"), None);
    }

    #[test]
    fn total_is_busiest_engine_summed_over_pids() {
        let engine = vec![
            s("pid_1_luid_0x0_0xA_phys_0_eng_0_engtype_3D", 40.0),
            s("pid_2_luid_0x0_0xA_phys_0_eng_0_engtype_3D", 35.0),
            s("pid_1_luid_0x0_0xA_phys_0_eng_5_engtype_VideoDecode", 60.0),
        ];
        let r = reduce(&engine, &[], &[], None, 0);
        assert_eq!(r.usage, 75.0);
    }

    #[test]
    fn per_pid_gpu_is_its_busiest_engine_and_vram_is_summed() {
        let engine = vec![
            s("pid_1_luid_0x0_0xA_phys_0_eng_0_engtype_3D", 40.0),
            s("pid_1_luid_0x0_0xA_phys_0_eng_5_engtype_VideoDecode", 60.0),
        ];
        let mem = vec![s("pid_1_luid_0x0_0xA_phys_0", 1000.0), s("pid_1_luid_0x0_0xB_phys_0", 24.0), s("pid_9_luid_0x0_0xA_phys_0", 5.0)];
        let r = reduce(&engine, &mem, &[], None, 0);
        assert_eq!(r.per_pid[&1], ProcGpu { gpu: 60.0, vram: 1024 });
        assert_eq!(r.per_pid[&9], ProcGpu { gpu: 0.0, vram: 5 });
    }

    #[test]
    fn usage_is_clamped_and_vram_used_sums_adapters() {
        let engine = vec![s("pid_1_luid_0x0_0xA_phys_0_eng_0_engtype_3D", 80.0), s("pid_2_luid_0x0_0xA_phys_0_eng_0_engtype_3D", 70.0)];
        let adapters = vec![s("luid_0x0_0xA_phys_0", 2_000_000_000.0), s("luid_0x0_0xB_phys_0", 0.0)];
        let r = reduce(&engine, &[], &adapters, None, 0);
        assert_eq!(r.usage, 100.0);
        assert_eq!(r.vram_used, 2_000_000_000);
    }

    #[test]
    fn malformed_instances_are_ignored() {
        let r = reduce(&[s("weird", 50.0)], &[s("also weird", 9.0)], &[], None, 0);
        assert_eq!(r, GpuReading::default());
    }

    #[test]
    fn filters_out_virtual_or_secondary_adapter_luids() {
        let engine = vec![
            s("pid_60324_luid_0x00000000_0x000116c1_phys_0_eng_4_engtype_3d", 82.2),
            s("pid_68176_luid_0x00000000_0x4B7D703F_phys_0_eng_0_engtype_3D", 1.9),
            s("pid_59488_luid_0x00000000_0x4b7d703f_phys_0_eng_0_engtype_3d", 12.1),
        ];
        let proc_mem = vec![
            s("pid_60324_luid_0x00000000_0x000116c1_phys_0", 40_000_000.0),
            s("pid_59488_luid_0x00000000_0x4b7d703f_phys_0", 500_000_000.0),
        ];
        let adapter_mem = vec![
            s("luid_0x00000000_0x000116c1_phys_0", 99_000_000.0),
            s("luid_0x00000000_0x4b7d703f_phys_0", 3_000_000_000.0),
        ];
        let r = reduce(&engine, &proc_mem, &adapter_mem, Some("luid_0x00000000_0x4b7d703f"), 0);
        assert_eq!(r.usage, 14.0);
        assert_eq!(r.vram_used, 3_000_000_000);
        assert!(!r.per_pid.contains_key(&60324), "virtual adapter PID must be excluded");
        assert_eq!(r.per_pid[&59488], ProcGpu { gpu: 12.1, vram: 500_000_000 });
    }

    #[test]
    fn adapter_usage_is_held_at_capacity_and_bounds_every_app() {
        let adapters = vec![s("luid_0x0_0xA_phys_0", 15_000.0)];
        let mem = vec![s("pid_1_luid_0x0_0xA_phys_0", 40_000.0)];
        let r = reduce(&[], &mem, &adapters, None, 12_000);
        assert_eq!(r.vram_used, 12_000, "usage above capacity is pinned to capacity");
        assert_eq!(r.app_vram_cap, Some(12_000));

        // Measured usage below capacity is the tighter bound for any one app.
        let adapters = vec![s("luid_0x0_0xA_phys_0", 3_400.0)];
        let r = reduce(&[], &mem, &adapters, None, 12_000);
        assert_eq!(r.vram_used, 3_400);
        assert_eq!(r.app_vram_cap, Some(3_400));
        assert_eq!(r.per_pid[&1].vram, 40_000, "per-process values stay as reported; the cap applies per app");
    }

    #[test]
    fn app_cap_falls_back_to_capacity_when_adapter_counter_is_empty() {
        let mem = vec![s("pid_1_luid_0x0_0xA_phys_0", 40_000.0)];
        let r = reduce(&[], &mem, &[], None, 12_000);
        assert_eq!(r.vram_used, 0);
        assert_eq!(r.app_vram_cap, Some(12_000), "an empty adapter array must not cap apps at 0");
    }

    #[test]
    fn unknown_capacity_and_no_adapter_data_leaves_everything_unbounded() {
        let mem = vec![s("pid_1_luid_0x0_0xA_phys_0", 40_000.0)];
        let r = reduce(&[], &mem, &[], None, 0);
        assert_eq!(r.app_vram_cap, None);
        let r = reduce(&[], &mem, &[s("luid_0x0_0xA_phys_0", 15_000.0)], None, 0);
        assert_eq!((r.vram_used, r.app_vram_cap), (15_000, Some(15_000)), "no capacity known: usage passes through");
    }

    #[test]
    fn vram_sums_saturate_instead_of_wrapping() {
        let huge = i64::MAX as f64;
        let mem = vec![s("pid_1_luid_0x0_0xA_phys_0", huge), s("pid_1_luid_0x0_0xA_phys_1", huge), s("pid_1_luid_0x0_0xA_phys_2", huge)];
        let adapters = vec![s("luid_0x0_0xA_phys_0", huge), s("luid_0x0_0xA_phys_1", huge), s("luid_0x0_0xA_phys_2", huge)];
        let r = reduce(&[], &mem, &adapters, None, 0);
        assert_eq!(r.vram_used, u64::MAX);
        assert_eq!(r.per_pid[&1].vram, u64::MAX);
    }

    /// The original, allocation-heavy reduction, kept as the reference the streaming `Reducer`
    /// must agree with.
    fn reduce_reference(
        engine: &[(String, f64)],
        proc_mem: &[(String, f64)],
        adapter_mem: &[(String, f64)],
        luid_filter: Option<&str>,
        vram_cap: u64,
    ) -> GpuReading {
        let mut per_engine: HashMap<String, f64> = HashMap::new();
        let mut per_pid: HashMap<u32, ProcGpu> = HashMap::new();
        for (name, value) in engine {
            if !matches_luid(name, luid_filter) {
                continue;
            }
            let v = value.max(0.0);
            if let Some(key) = engine_key(name) {
                *per_engine.entry(key.to_ascii_lowercase()).or_default() += v;
            }
            if let Some(pid) = parse_pid(name) {
                let e = per_pid.entry(pid).or_default();
                e.gpu = e.gpu.max(v as f32);
            }
        }
        for (name, value) in proc_mem {
            if !matches_luid(name, luid_filter) {
                continue;
            }
            if let Some(pid) = parse_pid(name) {
                let e = per_pid.entry(pid).or_default();
                e.vram = e.vram.saturating_add(value.max(0.0) as u64);
            }
        }
        for g in per_pid.values_mut() {
            g.gpu = round1(g.gpu.clamp(0.0, 100.0));
        }
        let usage = per_engine.values().copied().fold(0.0f64, f64::max).clamp(0.0, 100.0) as f32;
        let vram_used = adapter_mem
            .iter()
            .filter(|(name, _)| matches_luid(name, luid_filter))
            .fold(0u64, |acc, (_, v)| acc.saturating_add(v.max(0.0) as u64));
        let vram_used = if vram_cap > 0 { vram_used.min(vram_cap) } else { vram_used };
        let app_vram_cap = if vram_used > 0 { Some(vram_used) } else { (vram_cap > 0).then_some(vram_cap) };
        GpuReading { usage: round1(usage), vram_used, per_pid, app_vram_cap }
    }

    /// Entries that only record "this PID has an idle engine" carry no information
    /// (`group_by_name` treats a missing PID as all zeros), and the streaming reducer never makes them.
    fn without_all_zero_pids(mut r: GpuReading) -> GpuReading {
        r.per_pid.retain(|_, p| *p != ProcGpu::default());
        r
    }

    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            self.0 >> 33
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    #[test]
    fn streaming_reducer_matches_the_reference_on_random_counters() {
        let luids = ["luid_0x00000000_0x4B7D703F", "luid_0x00000000_0x4b7d703f", "luid_0x00000000_0x000116C1", "luid_0x00000001_0x4B7D703F"];
        let types = ["3D", "Copy", "VideoDecode", "3d", "Compute_0"];
        for seed in 0..40u64 {
            let mut r = Lcg(seed * 7919 + 1);
            let mut engine = Vec::new();
            let mut proc_mem = Vec::new();
            let mut adapter = Vec::new();
            for _ in 0..r.below(400) {
                let pid = r.below(60);
                let luid = luids[r.below(4) as usize];
                let name = match r.below(12) {
                    0 => "garbage".to_string(),
                    1 => format!("pid_x{pid}_{luid}_phys_0_eng_1_engtype_3D"),
                    _ => format!("pid_{pid}_{luid}_phys_{}_eng_{}_engtype_{}", r.below(2), r.below(8), types[r.below(5) as usize]),
                };
                let v = match r.below(6) {
                    0 | 1 | 2 => 0.0,
                    3 => -(r.below(50) as f64),
                    4 => r.below(1200) as f64 / 10.0,
                    _ => r.below(100) as f64,
                };
                engine.push((name, v));
            }
            for _ in 0..r.below(120) {
                let pid = r.below(60);
                let luid = luids[r.below(4) as usize];
                let v = if r.below(4) == 0 { 0.0 } else { (r.below(5000) * 1_000_000) as f64 };
                proc_mem.push((format!("pid_{pid}_{luid}_phys_{}", r.below(2)), v));
            }
            for _ in 0..r.below(5) {
                let luid = luids[r.below(4) as usize];
                adapter.push((format!("{luid}_phys_{}", r.below(2)), (r.below(20_000) * 1_000_000) as f64));
            }
            for filter in [None, Some("luid_0x00000000_0x4b7d703f")] {
                for cap in [0u64, 8_000_000_000, 40_000_000_000] {
                    let want = without_all_zero_pids(reduce_reference(&engine, &proc_mem, &adapter, filter, cap));
                    let got = reduce(&engine, &proc_mem, &adapter, filter, cap);
                    assert_eq!(want, got, "seed {seed}, filter {filter:?}, cap {cap}");
                }
            }
        }
    }

    #[test]
    fn idle_instances_are_ignored_without_creating_entries() {
        let engine = vec![s("pid_1_luid_0x0_0xA_phys_0_eng_0_engtype_3D", 0.0), s("pid_2_luid_0x0_0xA_phys_0_eng_0_engtype_3D", 12.5)];
        let r = reduce(&engine, &[], &[], None, 0);
        assert_eq!(r.usage, 12.5);
        assert!(!r.per_pid.contains_key(&1), "an idle engine says nothing about its PID");
        assert_eq!(r.per_pid[&2].gpu, 12.5);
    }

    #[test]
    fn name_buffer_decodes_ascii_and_matches_lossy_decoding_otherwise() {
        let mut nb = NameBuf::default();
        let w = |t: &str| t.encode_utf16().collect::<Vec<u16>>();
        assert_eq!(nb.decode(&w("pid_1_luid_0x0_0xA_phys_0_eng_0_engtype_3D")), "pid_1_luid_0x0_0xA_phys_0_eng_0_engtype_3D");
        // a shorter name after a longer one must not keep the longer one's tail
        assert_eq!(nb.decode(&w("pid_2")), "pid_2");
        assert_eq!(nb.decode(&[]), "");
        // non-ASCII, a surrogate pair, and a lone surrogate behave exactly like the lossy decoder
        for wide in [w("naïve_é.exe"), w("a😀b"), vec![b'a' as u16, 0xD800, b'b' as u16]] {
            assert_eq!(nb.decode(&wide), String::from_utf16_lossy(&wide));
        }
        // and the ASCII path is usable again afterwards
        assert_eq!(nb.decode(&w("ok")), "ok");
    }

    #[test]
    fn live_counters_open_and_read() {
        let adapter = GpuAdapter::open().ok();
        let mut c = GpuCounters::open(adapter.as_ref()).expect("PDH GPU counters available on this PC");
        std::thread::sleep(std::time::Duration::from_millis(300));
        let r = c.read().expect("read");
        assert!((0.0..=100.0).contains(&r.usage));
        assert!(r.vram_used > 0, "desktop compositor always holds some VRAM");
        if let Some(a) = &adapter {
            assert!(r.vram_used <= a.vram_cap, "vram_used {} above capacity {}", r.vram_used, a.vram_cap);
        }
    }
}
