//! Pure aggregation: group per-process rows by app name and build the top-N lists.
//! No OS calls here, so everything is unit-testable.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::model::{AppEntry, Top};

/// One process after CPU % has been computed.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcRow {
    pub pid: u32,
    pub name: String,
    /// Percent of the whole machine (0–100), already normalised by logical core count.
    pub cpu: f32,
    /// Private working set in bytes (Task Manager's default "Memory" column).
    pub mem: u64,
}

/// Per-process GPU usage from the Windows GPU performance counters.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ProcGpu {
    /// Busiest-engine utilisation for this process, 0–100.
    pub gpu: f32,
    /// Dedicated VRAM in bytes.
    pub vram: u64,
}

pub const TOP_N: usize = 24;

static LIVE_TOP_N: AtomicUsize = AtomicUsize::new(TOP_N);

pub fn current_top_n() -> usize {
    LIVE_TOP_N.load(Ordering::Relaxed)
}

pub fn set_top_n(n: usize) {
    LIVE_TOP_N.store(n.clamp(1, 100), Ordering::Relaxed);
}

/// Round to one decimal place (spec §4: percentages are rounded to 1 decimal).
pub fn round1(v: f32) -> f32 {
    (v * 10.0).round() / 10.0
}

/// Group processes case-insensitively by name, keeping the first spelling seen as display name.
/// Excludes PID 0 ("System Idle Process") and empty names.
/// If `gpu` is `Some`, each app gets the summed GPU % (clamped to 100) and summed VRAM;
/// if `None`, both fields are `None` so they serialize as `null`.
pub fn group_by_name(rows: &[ProcRow], gpu: Option<&HashMap<u32, ProcGpu>>) -> Vec<AppEntry> {
    let mut map: HashMap<String, AppEntry> = HashMap::new();
    for r in rows {
        if r.pid == 0 || r.name.is_empty() {
            continue;
        }
        let key = r.name.to_lowercase();
        let g = gpu.map(|m| m.get(&r.pid).copied().unwrap_or_default());
        let entry = map.entry(key).or_insert_with(|| AppEntry {
            name: r.name.clone(),
            count: 0,
            cpu: 0.0,
            mem: 0,
            gpu: g.map(|_| 0.0),
            vram: g.map(|_| 0),
        });
        entry.count += 1;
        entry.cpu += r.cpu;
        entry.mem += r.mem;
        if let Some(pg) = g {
            *entry.gpu.get_or_insert(0.0) += pg.gpu;
            *entry.vram.get_or_insert(0) += pg.vram;
        }
    }
    let mut out: Vec<AppEntry> = map.into_values().collect();
    for e in &mut out {
        e.cpu = round1(e.cpu.clamp(0.0, 100.0));
        if let Some(g) = &mut e.gpu {
            *g = round1(g.clamp(0.0, 100.0));
        }
    }
    out
}

fn by_name(a: &AppEntry, b: &AppEntry) -> std::cmp::Ordering {
    a.name.to_lowercase().cmp(&b.name.to_lowercase())
}

/// Build the Top-N lists from grouped apps.
/// - `cpu`: top `n` by `cpu` desc, tie-break by lowercase name asc.
/// - `mem`: top `n` by `mem` desc, tie-break by lowercase name asc.
/// - `gpu`: `None` when `!gpu_available`; otherwise apps with `gpu > 0` or `vram > 0`,
///   sorted by `gpu` desc then `vram` desc then name asc, truncated to `n`.
pub fn build_top(apps: &[AppEntry], n: usize, gpu_available: bool) -> Top {
    let mut by_cpu = apps.to_vec();
    by_cpu.sort_by(|a, b| b.cpu.total_cmp(&a.cpu).then_with(|| by_name(a, b)));
    by_cpu.truncate(n);

    let mut by_mem = apps.to_vec();
    by_mem.sort_by(|a, b| b.mem.cmp(&a.mem).then_with(|| by_name(a, b)));
    by_mem.truncate(n);

    let gpu = gpu_available.then(|| {
        let mut by_gpu: Vec<AppEntry> = apps
            .iter()
            .filter(|a| a.gpu.unwrap_or(0.0) > 0.0 || a.vram.unwrap_or(0) > 0)
            .cloned()
            .collect();
        by_gpu.sort_by(|a, b| {
            b.gpu
                .unwrap_or(0.0)
                .total_cmp(&a.gpu.unwrap_or(0.0))
                .then_with(|| b.vram.unwrap_or(0).cmp(&a.vram.unwrap_or(0)))
                .then_with(|| by_name(a, b))
        });
        by_gpu.truncate(n);
        by_gpu
    });

    Top { cpu: by_cpu, mem: by_mem, gpu }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(pid: u32, name: &str, cpu: f32, mem: u64) -> ProcRow {
        ProcRow { pid, name: name.into(), cpu, mem }
    }

    #[test]
    fn groups_case_insensitively_and_sums() {
        let rows = vec![
            row(1, "chrome.exe", 5.24, 100),
            row(2, "Chrome.exe", 3.11, 250),
            row(3, "discord.exe", 1.0, 50),
        ];
        let mut apps = group_by_name(&rows, None);
        apps.sort_by(by_name);
        assert_eq!(apps.len(), 2);
        assert_eq!(
            apps[0],
            AppEntry { name: "chrome.exe".into(), count: 2, cpu: 8.3, mem: 350, gpu: None, vram: None }
        );
    }

    #[test]
    fn excludes_pid_zero() {
        let rows = vec![row(0, "System Idle Process", 94.0, 8192), row(4, "System", 0.5, 4096)];
        let apps = group_by_name(&rows, None);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "System");
    }

    #[test]
    fn merges_gpu_per_pid_and_clamps_group_to_100() {
        let rows = vec![row(10, "a.exe", 1.0, 10), row(11, "a.exe", 1.0, 20), row(12, "b.exe", 1.0, 30)];
        let mut gpu = HashMap::new();
        gpu.insert(10, ProcGpu { gpu: 70.0, vram: 1000 });
        gpu.insert(11, ProcGpu { gpu: 60.0, vram: 500 });
        let apps = group_by_name(&rows, Some(&gpu));
        let a = apps.iter().find(|x| x.name == "a.exe").unwrap();
        assert_eq!((a.gpu, a.vram), (Some(100.0), Some(1500)));
        let b = apps.iter().find(|x| x.name == "b.exe").unwrap();
        assert_eq!((b.gpu, b.vram), (Some(0.0), Some(0)));
    }

    #[test]
    fn rounds_cpu_to_one_decimal() {
        let apps = group_by_name(&[row(1, "x", 1.04, 0), row(2, "x", 1.04, 0)], None);
        assert_eq!(apps[0].cpu, 2.1);
    }

    #[test]
    fn top_lists_sort_desc_with_name_tiebreak_and_truncate() {
        let rows: Vec<ProcRow> = (1..=7).map(|i| row(i, &format!("p{i}"), (i % 3) as f32, i as u64 * 10)).collect();
        let top = build_top(&group_by_name(&rows, None), 5, false);
        let cpu: Vec<&str> = top.cpu.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(cpu, vec!["p2", "p5", "p1", "p4", "p7"]);
        let mem: Vec<&str> = top.mem.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(mem, vec!["p7", "p6", "p5", "p4", "p3"]);
        assert_eq!(top.gpu, None);

        let rows30: Vec<ProcRow> = (1..=30).map(|i| row(i, &format!("p{i}"), i as f32, i as u64)).collect();
        let top_n = build_top(&group_by_name(&rows30, None), TOP_N, false);
        assert_eq!(top_n.cpu.len(), TOP_N);
        assert_eq!(top_n.mem.len(), TOP_N);
    }

    #[test]
    fn fewer_than_n_processes_is_fine() {
        let top = build_top(&group_by_name(&[row(1, "only.exe", 1.0, 1)], None), TOP_N, false);
        assert_eq!(top.cpu.len(), 1);
        assert_eq!(top.mem.len(), 1);
    }

    #[test]
    fn empty_input_gives_empty_lists() {
        let top = build_top(&[], TOP_N, true);
        assert!(top.cpu.is_empty() && top.mem.is_empty());
        assert_eq!(top.gpu, Some(vec![]));
    }

    #[test]
    fn gpu_top_skips_idle_apps_and_breaks_ties_by_vram() {
        let rows = vec![row(1, "idle.exe", 0.0, 1), row(2, "holder.exe", 0.0, 1), row(3, "game.exe", 0.0, 1), row(4, "obs.exe", 0.0, 1)];
        let mut gpu = HashMap::new();
        gpu.insert(2, ProcGpu { gpu: 0.0, vram: 900 });
        gpu.insert(3, ProcGpu { gpu: 95.0, vram: 7000 });
        gpu.insert(4, ProcGpu { gpu: 0.0, vram: 300 });
        let top = build_top(&group_by_name(&rows, Some(&gpu)), TOP_N, true);
        let names: Vec<&str> = top.gpu.as_ref().unwrap().iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["game.exe", "holder.exe", "obs.exe"]);
    }
}
