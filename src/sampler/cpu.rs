//! Portable CPU math: whole-machine busy % and per-process CPU % from CPU-time deltas.
//! The platform readers (`crate::platform::sampler`) only supply raw counters; everything that
//! can be computed without the OS lives here so it is shared and unit-tested on every platform.

use std::collections::HashMap;
use std::time::Duration;

use crate::aggregate::ProcRow;

/// Busy % from time deltas. `idle` and `total` are deltas in the same unit, and `total`
/// INCLUDES the idle time (on Windows, kernel time already contains idle; other platforms
/// add idle in themselves).
// Unused on macOS until its CPU reader is implemented (see platform/macos/sampler/system.rs).
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub fn busy_percent(idle: u64, total: u64) -> f32 {
    if total == 0 {
        return 0.0;
    }
    let busy = total.saturating_sub(idle) as f64 / total as f64;
    (busy * 100.0).clamp(0.0, 100.0) as f32
}

/// One process as read from the OS snapshot (`platform::ProcessSource::take`).
#[derive(Debug, Clone, PartialEq)]
pub struct RawProc {
    pub pid: u32,
    /// Display name (e.g. `chrome.exe` on Windows, `Safari` on macOS). Grouping is by name.
    pub name: String,
    /// Total (kernel + user) CPU time, in 100 ns units. Platforms with another native unit
    /// convert (e.g. macOS nanoseconds / 100).
    pub cpu_time: i64,
    /// Any value that is fixed for the life of the process (its start time). Used to detect
    /// PID reuse between snapshots.
    pub create_time: i64,
    /// Private memory in bytes: private working set on Windows, physical footprint on macOS.
    pub mem: u64,
}

/// Turns successive snapshots into per-process CPU % of the whole machine.
#[derive(Default)]
pub struct CpuTracker {
    /// pid -> (create_time, cpu_time) from the previous snapshot.
    prev: HashMap<u32, (i64, i64)>,
}

impl CpuTracker {
    /// Forget every baseline (call while idle: after a pause they are stale anyway). The next
    /// `update` reports 0 % for everything, exactly like the first one.
    pub fn clear(&mut self) {
        self.prev = HashMap::new();
    }

    /// `wall` is the real time elapsed since the previous call. On the first call (or for a
    /// process not seen before / whose PID was reused) CPU is reported as 0.
    pub fn update(&mut self, procs: &[RawProc], wall: Duration, logical_cores: usize) -> Vec<ProcRow> {
        let wall_100ns = wall.as_nanos() as f64 / 100.0;
        let capacity = wall_100ns * logical_cores.max(1) as f64;
        let mut next = HashMap::with_capacity(procs.len());
        let rows = procs
            .iter()
            .map(|p| {
                let cpu = match self.prev.get(&p.pid) {
                    Some(&(created, before)) if created == p.create_time && capacity > 0.0 => {
                        let delta = (p.cpu_time - before).max(0) as f64;
                        ((delta / capacity) * 100.0).clamp(0.0, 100.0) as f32
                    }
                    _ => 0.0,
                };
                next.insert(p.pid, (p.create_time, p.cpu_time));
                ProcRow { pid: p.pid, name: p.name.clone(), cpu, mem: p.mem }
            })
            .collect();
        self.prev = next;
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn busy_percent_basics() {
        assert_eq!(busy_percent(0, 0), 0.0);
        assert_eq!(busy_percent(100, 100), 0.0);
        assert_eq!(busy_percent(25, 100), 75.0);
        assert_eq!(busy_percent(0, 100), 100.0);
        assert_eq!(busy_percent(200, 100), 0.0); // idle > total never goes negative
    }

    fn raw(pid: u32, cpu_time: i64, create_time: i64) -> RawProc {
        RawProc { pid, name: format!("p{pid}"), cpu_time, create_time, mem: 42 }
    }

    #[test]
    fn first_update_reports_zero_cpu() {
        let mut t = CpuTracker::default();
        let rows = t.update(&[raw(1, 5_000_000, 7)], Duration::from_secs(1), 2);
        assert_eq!(rows[0].cpu, 0.0);
        assert_eq!(rows[0].mem, 42);
    }

    #[test]
    fn one_core_busy_for_one_second_on_two_cores_is_fifty_percent() {
        let mut t = CpuTracker::default();
        t.update(&[raw(1, 0, 7)], Duration::from_secs(1), 2);
        let rows = t.update(&[raw(1, 10_000_000, 7)], Duration::from_secs(1), 2);
        assert!((rows[0].cpu - 50.0).abs() < 0.01, "{}", rows[0].cpu);
    }

    #[test]
    fn all_cores_busy_is_one_hundred_percent() {
        let mut t = CpuTracker::default();
        t.update(&[raw(1, 0, 7)], Duration::from_secs(1), 12);
        let rows = t.update(&[raw(1, 120_000_000, 7)], Duration::from_secs(1), 12);
        assert!((rows[0].cpu - 100.0).abs() < 0.01);
    }

    #[test]
    fn cleared_tracker_starts_over() {
        let mut t = CpuTracker::default();
        t.update(&[raw(1, 0, 7)], Duration::from_secs(1), 1);
        t.clear();
        let rows = t.update(&[raw(1, 9_000_000, 7)], Duration::from_secs(1), 1);
        assert_eq!(rows[0].cpu, 0.0, "no baseline after clear");
        let rows = t.update(&[raw(1, 19_000_000, 7)], Duration::from_secs(1), 1);
        assert!((rows[0].cpu - 100.0).abs() < 0.01, "baseline is re-established");
    }

    #[test]
    fn reused_pid_is_treated_as_new_process() {
        let mut t = CpuTracker::default();
        t.update(&[raw(1, 0, 7)], Duration::from_secs(1), 1);
        let rows = t.update(&[raw(1, 9_000_000, 8)], Duration::from_secs(1), 1);
        assert_eq!(rows[0].cpu, 0.0);
    }
}
