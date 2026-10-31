//! Pure aggregation: group per-process rows by app name and build the top-N lists.
//! No OS calls here, so everything is unit-testable.

use std::collections::{HashMap, HashSet};
use std::sync::{OnceLock, RwLock};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::{Deserialize, Serialize};

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

/// How the GPU list is ranked: by engine utilisation first, or by dedicated VRAM first.
/// Anything other than `"vram"` in stored/posted settings reads as `Usage`, so a stale or
/// hand-edited `settings.json` can never make the whole file fail to load.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", from = "String")]
pub enum GpuSort {
    #[default]
    Usage,
    Vram,
}

impl From<String> for GpuSort {
    fn from(s: String) -> Self {
        if s == "vram" { Self::Vram } else { Self::Usage }
    }
}

#[derive(Clone, Default)]
struct ProcPrefs {
    pinned: HashSet<String>,
    hidden: HashSet<String>,
    gpu_sort: GpuSort,
}

static LIVE_PREFS: OnceLock<RwLock<ProcPrefs>> = OnceLock::new();

fn prefs_lock() -> &'static RwLock<ProcPrefs> {
    LIVE_PREFS.get_or_init(|| RwLock::new(ProcPrefs::default()))
}

pub fn set_process_prefs(pinned: &[String], hidden: &[String], gpu_sort: GpuSort) {
    let mut guard = prefs_lock().write().expect("prefs lock");
    guard.pinned = pinned.iter().map(|s| s.to_lowercase()).collect();
    guard.hidden = hidden.iter().map(|s| s.to_lowercase()).collect();
    guard.gpu_sort = gpu_sort;
}

/// Round to one decimal place (spec §4: percentages are rounded to 1 decimal).
pub fn round1(v: f32) -> f32 {
    (v * 10.0).round() / 10.0
}

/// Group processes case-insensitively by name, keeping the first spelling seen as display name.
/// Excludes PID 0 ("System Idle Process") and empty names.
/// If `gpu` is `Some`, each app gets the summed GPU % (clamped to 100) and summed VRAM
/// (clamped to `vram_cap` when known); if `None`, both fields are `None` so they serialize
/// as `null`.
///
/// The VRAM cap is applied to the per-app *sum*: that is the figure the dashboard shows, and
/// per-process counters can be inflated, or count shared allocations once per process, so a
/// multi-process app (browsers, WebView2 hosts) can add up to more than the adapter holds.
pub fn group_by_name(rows: &[ProcRow], gpu: Option<&HashMap<u32, ProcGpu>>, vram_cap: Option<u64>) -> Vec<AppEntry> {
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
            let v = entry.vram.get_or_insert(0);
            *v = v.saturating_add(pg.vram);
        }
    }
    let mut out: Vec<AppEntry> = map.into_values().collect();
    for e in &mut out {
        e.cpu = round1(e.cpu.clamp(0.0, 100.0));
        if let Some(g) = &mut e.gpu {
            *g = round1(g.clamp(0.0, 100.0));
        }
        if let (Some(v), Some(cap)) = (&mut e.vram, vram_cap) {
            *v = (*v).min(cap);
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
///   sorted by `gpu` desc then `vram` desc then name asc (`vram` first when `gpu_sort` is
///   [`GpuSort::Vram`]), truncated to `n`.
pub fn build_top(apps: &[AppEntry], n: usize, gpu_available: bool) -> Top {
    let prefs = prefs_lock().read().expect("prefs lock").clone();
    build_top_with_prefs(apps, n, gpu_available, &prefs.pinned, &prefs.hidden, prefs.gpu_sort)
}

pub fn build_top_with_prefs(
    apps: &[AppEntry],
    n: usize,
    gpu_available: bool,
    pinned: &HashSet<String>,
    hidden: &HashSet<String>,
    gpu_sort: GpuSort,
) -> Top {
    let visible: Vec<AppEntry> = apps
        .iter()
        .filter(|a| !hidden.contains(&a.name.to_lowercase()))
        .cloned()
        .collect();
    let is_pin = |a: &AppEntry| pinned.contains(&a.name.to_lowercase());
    let cap = n.saturating_add(pinned.len());

    let mut by_cpu = visible.clone();
    by_cpu.sort_by(|a, b| {
        is_pin(b)
            .cmp(&is_pin(a))
            .then_with(|| b.cpu.total_cmp(&a.cpu))
            .then_with(|| by_name(a, b))
    });
    by_cpu.truncate(cap);

    let mut by_mem = visible.clone();
    by_mem.sort_by(|a, b| {
        is_pin(b)
            .cmp(&is_pin(a))
            .then_with(|| b.mem.cmp(&a.mem))
            .then_with(|| by_name(a, b))
    });
    by_mem.truncate(cap);

    let gpu = gpu_available.then(|| {
        let mut by_gpu: Vec<AppEntry> = visible
            .into_iter()
            .filter(|a| is_pin(a) || a.gpu.unwrap_or(0.0) > 0.0 || a.vram.unwrap_or(0) > 0)
            .collect();
        by_gpu.sort_by(|a, b| {
            let by_usage = || b.gpu.unwrap_or(0.0).total_cmp(&a.gpu.unwrap_or(0.0));
            let by_vram = || b.vram.unwrap_or(0).cmp(&a.vram.unwrap_or(0));
            let ranked = is_pin(b).cmp(&is_pin(a));
            let ranked = match gpu_sort {
                GpuSort::Usage => ranked.then_with(by_usage).then_with(by_vram),
                GpuSort::Vram => ranked.then_with(by_vram).then_with(by_usage),
            };
            ranked.then_with(|| by_name(a, b))
        });
        by_gpu.truncate(cap);
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
        let mut apps = group_by_name(&rows, None, None);
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
        let apps = group_by_name(&rows, None, None);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "System");
    }

    #[test]
    fn merges_gpu_per_pid_and_clamps_group_to_100() {
        let rows = vec![row(10, "a.exe", 1.0, 10), row(11, "a.exe", 1.0, 20), row(12, "b.exe", 1.0, 30)];
        let mut gpu = HashMap::new();
        gpu.insert(10, ProcGpu { gpu: 70.0, vram: 1000 });
        gpu.insert(11, ProcGpu { gpu: 60.0, vram: 500 });
        let apps = group_by_name(&rows, Some(&gpu), None);
        let a = apps.iter().find(|x| x.name == "a.exe").unwrap();
        assert_eq!((a.gpu, a.vram), (Some(100.0), Some(1500)));
        let b = apps.iter().find(|x| x.name == "b.exe").unwrap();
        assert_eq!((b.gpu, b.vram), (Some(0.0), Some(0)));
    }

    #[test]
    fn rounds_cpu_to_one_decimal() {
        let apps = group_by_name(&[row(1, "x", 1.04, 0), row(2, "x", 1.04, 0)], None, None);
        assert_eq!(apps[0].cpu, 2.1);
    }

    #[test]
    fn top_lists_sort_desc_with_name_tiebreak_and_truncate() {
        let rows: Vec<ProcRow> = (1..=7).map(|i| row(i, &format!("p{i}"), (i % 3) as f32, i as u64 * 10)).collect();
        let top = build_top(&group_by_name(&rows, None, None), 5, false);
        let cpu: Vec<&str> = top.cpu.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(cpu, vec!["p2", "p5", "p1", "p4", "p7"]);
        let mem: Vec<&str> = top.mem.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(mem, vec!["p7", "p6", "p5", "p4", "p3"]);
        assert_eq!(top.gpu, None);

        let rows30: Vec<ProcRow> = (1..=30).map(|i| row(i, &format!("p{i}"), i as f32, i as u64)).collect();
        let top_n = build_top(&group_by_name(&rows30, None, None), TOP_N, false);
        assert_eq!(top_n.cpu.len(), TOP_N);
        assert_eq!(top_n.mem.len(), TOP_N);
    }

    #[test]
    fn fewer_than_n_processes_is_fine() {
        let top = build_top(&group_by_name(&[row(1, "only.exe", 1.0, 1)], None, None), TOP_N, false);
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
        let top = build_top(&group_by_name(&rows, Some(&gpu), None), TOP_N, true);
        let names: Vec<&str> = top.gpu.as_ref().unwrap().iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["game.exe", "holder.exe", "obs.exe"]);
    }

    #[test]
    fn build_top_with_prefs_pins_to_top_and_filters_hidden() {
        let rows = vec![
            row(1, "heavy.exe", 50.0, 5000),
            row(2, "noisy.exe", 40.0, 4000),
            row(3, "normal.exe", 20.0, 2000),
            row(4, "favorite.exe", 0.5, 100),
        ];
        let mut gpu = HashMap::new();
        gpu.insert(1, ProcGpu { gpu: 80.0, vram: 2000 });
        gpu.insert(3, ProcGpu { gpu: 10.0, vram: 500 });
        let apps = group_by_name(&rows, Some(&gpu), None);

        let pinned: HashSet<String> = ["favorite.exe".to_string()].into_iter().collect();
        let hidden: HashSet<String> = ["noisy.exe".to_string()].into_iter().collect();

        let top = build_top_with_prefs(&apps, 3, true, &pinned, &hidden, GpuSort::Usage);
        let cpu_names: Vec<&str> = top.cpu.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(cpu_names, vec!["favorite.exe", "heavy.exe", "normal.exe"]);

        let mem_names: Vec<&str> = top.mem.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(mem_names, vec!["favorite.exe", "heavy.exe", "normal.exe"]);

        let gpu_names: Vec<&str> = top.gpu.as_ref().unwrap().iter().map(|e| e.name.as_str()).collect();
        assert_eq!(gpu_names, vec!["favorite.exe", "heavy.exe", "normal.exe"]);
    }

    #[test]
    fn build_top_with_prefs_sorts_gpu_by_vram_when_requested() {
        let rows = vec![
            row(1, "engine_heavy.exe", 10.0, 1000),
            row(2, "vram_heavy.exe", 1.0, 1000),
            row(3, "idle.exe", 0.1, 1000),
        ];
        let mut gpu = HashMap::new();
        gpu.insert(1, ProcGpu { gpu: 85.0, vram: 500 });
        gpu.insert(2, ProcGpu { gpu: 2.0, vram: 9000 });
        gpu.insert(3, ProcGpu { gpu: 0.0, vram: 0 });
        let apps = group_by_name(&rows, Some(&gpu), None);

        let empty: HashSet<String> = HashSet::new();

        let top_usage = build_top_with_prefs(&apps, 5, true, &empty, &empty, GpuSort::Usage);
        let usage_names: Vec<&str> = top_usage.gpu.as_ref().unwrap().iter().map(|e| e.name.as_str()).collect();
        assert_eq!(usage_names, vec!["engine_heavy.exe", "vram_heavy.exe"]);

        let top_vram = build_top_with_prefs(&apps, 5, true, &empty, &empty, GpuSort::Vram);
        let vram_names: Vec<&str> = top_vram.gpu.as_ref().unwrap().iter().map(|e| e.name.as_str()).collect();
        assert_eq!(vram_names, vec!["vram_heavy.exe", "engine_heavy.exe"]);
    }

    #[test]
    fn caps_summed_vram_per_app() {
        // Two same-name processes (e.g. WebView2 hosts) that each look plausible but add up to
        // more than the adapter holds; and an app whose single process is wildly inflated.
        let rows = vec![
            row(1, "edge.exe", 0.0, 1),
            row(2, "edge.exe", 0.0, 1),
            row(3, "word.exe", 0.0, 1),
            row(4, "small.exe", 0.0, 1),
        ];
        let mut gpu = HashMap::new();
        gpu.insert(1, ProcGpu { gpu: 0.0, vram: 8_000 });
        gpu.insert(2, ProcGpu { gpu: 0.0, vram: 7_000 });
        gpu.insert(3, ProcGpu { gpu: 0.0, vram: 1_000_000 });
        gpu.insert(4, ProcGpu { gpu: 0.0, vram: 500 });
        let apps = group_by_name(&rows, Some(&gpu), Some(10_000));
        let vram_of = |n: &str| apps.iter().find(|a| a.name == n).unwrap().vram;
        assert_eq!(vram_of("edge.exe"), Some(10_000), "15_000 summed must be capped");
        assert_eq!(vram_of("word.exe"), Some(10_000), "inflated single process must be capped");
        assert_eq!(vram_of("small.exe"), Some(500), "values under the cap are untouched");

        let uncapped = group_by_name(&rows, Some(&gpu), None);
        let raw = |n: &str| uncapped.iter().find(|a| a.name == n).unwrap().vram;
        assert_eq!(raw("edge.exe"), Some(15_000));
        assert_eq!(raw("word.exe"), Some(1_000_000));
    }

    #[test]
    fn vram_cap_leaves_unavailable_gpu_as_null() {
        let apps = group_by_name(&[row(1, "a.exe", 1.0, 1)], None, Some(100));
        assert_eq!(apps[0].vram, None);
    }

    #[test]
    fn gpu_sort_reads_leniently_and_serializes_lowercase() {
        assert_eq!(serde_json::from_str::<GpuSort>("\"vram\"").unwrap(), GpuSort::Vram);
        assert_eq!(serde_json::from_str::<GpuSort>("\"usage\"").unwrap(), GpuSort::Usage);
        assert_eq!(serde_json::from_str::<GpuSort>("\"bogus\"").unwrap(), GpuSort::Usage);
        assert_eq!(serde_json::to_string(&GpuSort::Vram).unwrap(), "\"vram\"");
        assert_eq!(serde_json::to_string(&GpuSort::default()).unwrap(), "\"usage\"");
    }
}
