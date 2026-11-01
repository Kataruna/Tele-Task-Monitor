//! Whole-machine CPU (total + per logical core), RAM, CPU brand, hostname, core count.
//! Mach `host_processor_info` / `host_statistics64` and `sysctlbyname` — no subprocesses.

use std::ffi::CStr;

use crate::aggregate::round1;
use crate::platform::CpuLoad;
use crate::sampler::cpu::busy_percent;

/// `sysctlbyname` for a string value.
fn sysctl_string(name: &CStr) -> Option<String> {
    let mut len = 0usize;
    // SAFETY: size query with a null buffer.
    if unsafe { libc::sysctlbyname(name.as_ptr(), std::ptr::null_mut(), &mut len, std::ptr::null_mut(), 0) } != 0 || len == 0 {
        return None;
    }
    let mut buf = vec![0u8; len];
    // SAFETY: buf has `len` bytes.
    if unsafe { libc::sysctlbyname(name.as_ptr(), buf.as_mut_ptr().cast(), &mut len, std::ptr::null_mut(), 0) } != 0 {
        return None;
    }
    let s = CStr::from_bytes_until_nul(&buf).ok()?.to_string_lossy().trim().to_string();
    (!s.is_empty()).then_some(s)
}

/// `sysctlbyname` for an integer value (`u64` or smaller, little-endian).
fn sysctl_u64(name: &CStr) -> Option<u64> {
    let mut v = 0u64;
    let mut len = std::mem::size_of::<u64>();
    // SAFETY: v is 8 writable bytes; the kernel writes `len` <= 8 bytes.
    let r = unsafe { libc::sysctlbyname(name.as_ptr(), (&mut v as *mut u64).cast(), &mut len, std::ptr::null_mut(), 0) };
    (r == 0).then_some(v)
}

pub fn logical_cores() -> usize {
    sysctl_u64(c"hw.logicalcpu").map(|n| n as usize).filter(|&n| n > 0).unwrap_or_else(|| {
        std::thread::available_parallelism().map_or(1, |n| n.get())
    })
}

pub fn hostname() -> String {
    let mut buf = [0u8; 256];
    // SAFETY: buf has buf.len() bytes.
    if unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.len()) } != 0 {
        return "unknown".into();
    }
    let s = CStr::from_bytes_until_nul(&buf).map(|c| c.to_string_lossy().into_owned()).unwrap_or_default();
    // "MacBook-Pro.local" -> "MacBook-Pro", matching the short name Windows reports.
    let s = s.strip_suffix(".local").unwrap_or(&s).to_string();
    if s.is_empty() { "unknown".into() } else { s }
}

/// e.g. "Apple M3 Pro" or "Intel(R) Core(TM) i7-9750H CPU @ 2.60GHz".
pub fn cpu_brand() -> String {
    sysctl_string(c"machdep.cpu.brand_string").unwrap_or_else(|| "Unknown CPU".into())
}

fn page_size() -> u64 {
    // SAFETY: plain libc call.
    let p = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if p > 0 { p as u64 } else { 4096 }
}

/// RAM (used, total) in bytes. "Used" matches Activity Monitor's "Memory Used":
/// app memory (internal - purgeable) + wired + compressed.
pub fn ram() -> (u64, u64) {
    let total = sysctl_u64(c"hw.memsize").unwrap_or(0);
    let mut stats: libc::vm_statistics64 = unsafe { std::mem::zeroed() };
    let mut count = libc::HOST_VM_INFO64_COUNT;
    // SAFETY: stats is a vm_statistics64 and count is its size in natural_t units.
    let kr = unsafe {
        libc::host_statistics64(crate::platform::macos::ffi::host(), libc::HOST_VM_INFO64, (&mut stats as *mut libc::vm_statistics64).cast(), &mut count)
    };
    if kr != libc::KERN_SUCCESS {
        return (0, total);
    }
    let pages = (stats.internal_page_count as u64).saturating_sub(stats.purgeable_count as u64)
        + stats.wire_count as u64
        + stats.compressor_page_count as u64;
    ((pages * page_size()).min(total), total)
}

#[derive(Clone, Copy, Default)]
struct Times {
    idle: u64,
    total: u64,
}

/// Per-core `(idle, total)` tick counters, or `None` if the kernel call failed.
fn read_core_ticks(out: &mut Vec<Times>) -> bool {
    let mut cpu_count: libc::natural_t = 0;
    let mut info: libc::processor_info_array_t = std::ptr::null_mut();
    let mut info_count: libc::mach_msg_type_number_t = 0;
    // SAFETY: valid out-pointers; the kernel allocates `info` in our address space.
    let kr = unsafe {
        libc::host_processor_info(crate::platform::macos::ffi::host(), libc::PROCESSOR_CPU_LOAD_INFO, &mut cpu_count, &mut info, &mut info_count)
    };
    if kr != libc::KERN_SUCCESS || info.is_null() {
        return false;
    }
    out.clear();
    let states = libc::CPU_STATE_MAX as usize;
    // SAFETY: the kernel returned `info_count` natural_t values, `states` per CPU.
    let ticks = unsafe { std::slice::from_raw_parts(info as *const libc::natural_t, info_count as usize) };
    for cpu in ticks.chunks_exact(states).take(cpu_count as usize) {
        let user = cpu[libc::CPU_STATE_USER as usize] as u64;
        let system = cpu[libc::CPU_STATE_SYSTEM as usize] as u64;
        let idle = cpu[libc::CPU_STATE_IDLE as usize] as u64;
        let nice = cpu[libc::CPU_STATE_NICE as usize] as u64;
        out.push(Times { idle, total: user + system + idle + nice });
    }
    // SAFETY: frees exactly the region host_processor_info allocated for us.
    unsafe {
        libc::vm_deallocate(
            crate::platform::macos::ffi::task(),
            info as libc::vm_address_t,
            info_count as usize * std::mem::size_of::<libc::natural_t>(),
        );
    }
    true
}

/// Tracks total and per-core CPU busy % between calls.
pub struct CpuTimes {
    prev: Vec<Times>,
    now: Vec<Times>,
}

impl CpuLoad for CpuTimes {
    fn new(logical_cores: usize) -> Self {
        Self { prev: vec![Times::default(); logical_cores], now: Vec::with_capacity(logical_cores) }
    }

    fn update(&mut self) -> (f32, Vec<f32>) {
        if !read_core_ticks(&mut self.now) {
            return (0.0, vec![0.0; self.prev.len()]);
        }
        if self.prev.len() != self.now.len() {
            self.prev.resize(self.now.len(), Times::default());
        }
        let (mut idle_sum, mut total_sum) = (0u64, 0u64);
        let mut cores = Vec::with_capacity(self.now.len());
        for (now, before) in self.now.iter().zip(self.prev.iter()) {
            // 32-bit tick counters can wrap; saturating keeps a wrapped core at 0 % for one tick.
            let idle = now.idle.saturating_sub(before.idle);
            let total = now.total.saturating_sub(before.total);
            idle_sum += idle;
            total_sum += total;
            cores.push(round1(busy_percent(idle, total)));
        }
        std::mem::swap(&mut self.prev, &mut self.now);
        (round1(busy_percent(idle_sum, total_sum)), cores)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_facts_look_sane() {
        assert!(logical_cores() >= 1);
        assert!(!cpu_brand().is_empty());
        assert!(!hostname().is_empty());
        let (used, total) = ram();
        assert!(total > 0 && used > 0 && used <= total);
    }

    #[test]
    fn cpu_times_returns_one_value_per_core_in_range() {
        let cores = logical_cores();
        let mut t = CpuTimes::new(cores);
        t.update();
        std::thread::sleep(std::time::Duration::from_millis(200));
        let (total, per_core) = t.update();
        assert!((0.0..=100.0).contains(&total));
        assert_eq!(per_core.len(), cores);
        assert!(per_core.iter().all(|c| (0.0..=100.0).contains(c)));
    }
}
