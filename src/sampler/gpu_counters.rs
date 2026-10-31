//! GPU usage from the Windows "GPU Engine" / "GPU Process Memory" / "GPU Adapter Memory"
//! performance counters (the same source Task Manager uses).

use std::collections::HashMap;

use windows::Win32::System::Performance::{
    PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_FMT_LARGE, PDH_HCOUNTER, PDH_HQUERY, PDH_MORE_DATA,
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW, PdhOpenQueryW,
};
use windows::core::{PCWSTR, w};

use crate::aggregate::{ProcGpu, round1};

/// One GPU reading.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GpuReading {
    /// Busiest engine on the target adapter, 0–100 (Task Manager's headline GPU %).
    pub usage: f32,
    /// Dedicated VRAM in use on the target adapter, bytes.
    pub vram_used: u64,
    /// Per-PID GPU % (busiest engine for that PID) and dedicated VRAM.
    pub per_pid: HashMap<u32, ProcGpu>,
}

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

/// Reduce raw counter arrays (instance name, value) into a reading.
/// - `engine`: `\GPU Engine(*)\Utilization Percentage`
/// - `proc_mem`: `\GPU Process Memory(*)\Dedicated Usage`
/// - `adapter_mem`: `\GPU Adapter Memory(*)\Dedicated Usage`
/// - `luid_filter`: optional `"luid_0xHHHHHHHH_0xLLLLLLLL"` of the primary hardware GPU so
///   virtual display drivers and software renderers (WARP) do not pollute GPU metrics.
pub fn reduce(
    engine: &[(String, f64)],
    proc_mem: &[(String, f64)],
    adapter_mem: &[(String, f64)],
    luid_filter: Option<&str>,
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
            per_pid.entry(pid).or_default().vram += value.max(0.0) as u64;
        }
    }
    for g in per_pid.values_mut() {
        g.gpu = round1(g.gpu.clamp(0.0, 100.0));
    }
    let usage = per_engine.values().copied().fold(0.0f64, f64::max).clamp(0.0, 100.0) as f32;
    let vram_used = adapter_mem
        .iter()
        .filter(|(name, _)| matches_luid(name, luid_filter))
        .map(|(_, v)| v.max(0.0) as u64)
        .sum();
    GpuReading { usage: round1(usage), vram_used, per_pid }
}

/// Owns the PDH query. Not `Send`: create and use it on the sampler thread only.
pub struct GpuCounters {
    query: PDH_HQUERY,
    engine: PDH_HCOUNTER,
    proc_mem: PDH_HCOUNTER,
    adapter_mem: PDH_HCOUNTER,
    luid_filter: Option<String>,
    buf: Vec<u64>,
}

impl GpuCounters {
    pub fn open(luid_filter: Option<String>) -> Result<Self, String> {
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
            luid_filter,
            buf: Vec::new(),
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

    /// Collect a new sample. Utilisation covers the time since the previous call.
    pub fn read(&mut self) -> Result<GpuReading, String> {
        // SAFETY: query is open.
        let s = unsafe { PdhCollectQueryData(self.query) };
        if s != 0 {
            return Err(format!("PdhCollectQueryData failed: {s:#x}"));
        }
        let engine = self.array(self.engine, false);
        let proc_mem = self.array(self.proc_mem, true);
        let adapter_mem = self.array(self.adapter_mem, true);
        Ok(reduce(&engine, &proc_mem, &adapter_mem, self.luid_filter.as_deref()))
    }

    /// Read a wildcard counter as (instance name, value). Any error yields an empty list
    /// (e.g. "no data yet" right after start), never a failure of the whole tick.
    fn array(&mut self, counter: PDH_HCOUNTER, large: bool) -> Vec<(String, f64)> {
        let format = if large { PDH_FMT_LARGE } else { PDH_FMT_DOUBLE };
        loop {
            let mut bytes = (self.buf.len() * 8) as u32;
            let mut count = 0u32;
            let items = if self.buf.is_empty() { None } else { Some(self.buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W) };
            // SAFETY: buffer (if any) is `bytes` long and 8-byte aligned.
            let s = unsafe { PdhGetFormattedCounterArrayW(counter, format, &mut bytes, &mut count, items) };
            if s == PDH_MORE_DATA {
                let next_len = (bytes as usize / 8 + 1024).max(self.buf.len().saturating_mul(2).max(1024));
                self.buf.resize(next_len, 0);
                continue;
            }
            if s != 0 || count == 0 || self.buf.is_empty() {
                return Vec::new();
            }
            // SAFETY: PDH wrote `count` items at the start of buf; names point into buf.
            let items = unsafe { std::slice::from_raw_parts(self.buf.as_ptr() as *const PDH_FMT_COUNTERVALUE_ITEM_W, count as usize) };
            return items
                .iter()
                .filter(|it| it.FmtValue.CStatus <= 1) // PDH_CSTATUS_VALID_DATA or NEW_DATA
                .map(|it| {
                    // SAFETY: union member matches the requested format.
                    let v = unsafe { if large { it.FmtValue.Anonymous.largeValue as f64 } else { it.FmtValue.Anonymous.doubleValue } };
                    let name = if it.szName.is_null() {
                        String::new()
                    } else {
                        // SAFETY: szName is a non-null NUL-terminated string inside buf.
                        unsafe { it.szName.to_string() }.unwrap_or_default()
                    };
                    (name, v)
                })
                .collect();
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
        let r = reduce(&engine, &[], &[], None);
        assert_eq!(r.usage, 75.0);
    }

    #[test]
    fn per_pid_gpu_is_its_busiest_engine_and_vram_is_summed() {
        let engine = vec![
            s("pid_1_luid_0x0_0xA_phys_0_eng_0_engtype_3D", 40.0),
            s("pid_1_luid_0x0_0xA_phys_0_eng_5_engtype_VideoDecode", 60.0),
        ];
        let mem = vec![s("pid_1_luid_0x0_0xA_phys_0", 1000.0), s("pid_1_luid_0x0_0xB_phys_0", 24.0), s("pid_9_luid_0x0_0xA_phys_0", 5.0)];
        let r = reduce(&engine, &mem, &[], None);
        assert_eq!(r.per_pid[&1], ProcGpu { gpu: 60.0, vram: 1024 });
        assert_eq!(r.per_pid[&9], ProcGpu { gpu: 0.0, vram: 5 });
    }

    #[test]
    fn usage_is_clamped_and_vram_used_sums_adapters() {
        let engine = vec![s("pid_1_luid_0x0_0xA_phys_0_eng_0_engtype_3D", 80.0), s("pid_2_luid_0x0_0xA_phys_0_eng_0_engtype_3D", 70.0)];
        let adapters = vec![s("luid_0x0_0xA_phys_0", 2_000_000_000.0), s("luid_0x0_0xB_phys_0", 0.0)];
        let r = reduce(&engine, &[], &adapters, None);
        assert_eq!(r.usage, 100.0);
        assert_eq!(r.vram_used, 2_000_000_000);
    }

    #[test]
    fn malformed_instances_are_ignored() {
        let r = reduce(&[s("weird", 50.0)], &[s("also weird", 9.0)], &[], None);
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
        let r = reduce(&engine, &proc_mem, &adapter_mem, Some("luid_0x00000000_0x4b7d703f"));
        assert_eq!(r.usage, 14.0);
        assert_eq!(r.vram_used, 3_000_000_000);
        assert!(!r.per_pid.contains_key(&60324), "virtual adapter PID must be excluded");
        assert_eq!(r.per_pid[&59488], ProcGpu { gpu: 12.1, vram: 500_000_000 });
    }

    #[test]
    fn live_counters_open_and_read() {
        let luid = crate::sampler::gpu_adapter::GpuAdapter::open().ok().map(|a| a.luid_tag.clone());
        let mut c = GpuCounters::open(luid).expect("PDH GPU counters available on this PC");
        std::thread::sleep(std::time::Duration::from_millis(300));
        let r = c.read().expect("read");
        assert!((0.0..=100.0).contains(&r.usage));
        assert!(r.vram_used > 0, "desktop compositor always holds some VRAM");
    }
}
