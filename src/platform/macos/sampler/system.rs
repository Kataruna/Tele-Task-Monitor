//! Whole-machine CPU (total + per logical core), RAM, CPU brand, hostname, core count.

use std::process::Command;
use std::sync::OnceLock;

use crate::platform::CpuLoad;

/// `sysctl -n <name>`, trimmed. Only for one-off facts read at start-up, never per tick.
fn sysctl(name: &str) -> Option<String> {
    let out = Command::new("/usr/sbin/sysctl").args(["-n", name]).output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !s.is_empty()).then_some(s)
}

pub fn logical_cores() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

pub fn hostname() -> String {
    // TODO(macos): `gethostname` / `SCDynamicStoreCopyLocalHostName` instead of a subprocess.
    Command::new("/bin/hostname")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

pub fn cpu_brand() -> String {
    // TODO(macos): `sysctlbyname("machdep.cpu.brand_string")` instead of a subprocess.
    sysctl("machdep.cpu.brand_string").unwrap_or_else(|| "Unknown CPU".into())
}

/// RAM (used, total) in bytes.
pub fn ram() -> (u64, u64) {
    static TOTAL: OnceLock<u64> = OnceLock::new();
    let total = *TOTAL.get_or_init(|| sysctl("hw.memsize").and_then(|s| s.parse().ok()).unwrap_or(0));
    // TODO(macos): used = (active + wired + compressed) pages from `host_statistics64(HOST_VM_INFO64)`
    // × page size — the "Memory Used" figure in Activity Monitor.
    (0, total)
}

/// Tracks total and per-core CPU busy % between calls.
pub struct CpuTimes {
    logical_cores: usize,
}

impl CpuLoad for CpuTimes {
    fn new(logical_cores: usize) -> Self {
        Self { logical_cores }
    }

    fn update(&mut self) -> (f32, Vec<f32>) {
        // TODO(macos): `host_processor_info(PROCESSOR_CPU_LOAD_INFO)` gives per-core
        // user/system/idle/nice ticks; feed the deltas to `crate::sampler::cpu::busy_percent`
        // (total = user + system + nice + idle).
        (0.0, vec![0.0; self.logical_cores])
    }
}
