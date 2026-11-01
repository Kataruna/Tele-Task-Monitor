//! Whole-machine CPU (total + per logical core), RAM, CPU brand, hostname, core count.

use windows::Wdk::System::SystemInformation::{NtQuerySystemInformation, SystemProcessorPerformanceInformation};
use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::SystemInformation::{
    ComputerNameDnsHostname, GetComputerNameExW, GetSystemInfo, GlobalMemoryStatusEx, MEMORYSTATUSEX, SYSTEM_INFO,
};
use windows::Win32::System::Threading::GetSystemTimes;
use windows::core::PWSTR;

use crate::platform::CpuLoad;
use crate::sampler::cpu::busy_percent;

/// Layout of SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION (48 bytes on x64).
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct ProcessorPerf {
    idle_time: i64,
    kernel_time: i64,
    user_time: i64,
    dpc_time: i64,
    interrupt_time: i64,
    interrupt_count: u32,
}

#[derive(Clone, Copy, Default, Debug, PartialEq)]
struct Times {
    idle: u64,
    total: u64,
}

fn delta(now: Times, before: Times) -> f32 {
    busy_percent(now.idle.saturating_sub(before.idle), now.total.saturating_sub(before.total))
}

fn filetime(f: FILETIME) -> u64 {
    ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64
}

pub fn logical_cores() -> usize {
    let mut info = SYSTEM_INFO::default();
    // SAFETY: valid out-pointer.
    unsafe { GetSystemInfo(&mut info) };
    info.dwNumberOfProcessors.max(1) as usize
}

pub fn hostname() -> String {
    let mut buf = [0u16; 256];
    let mut len = buf.len() as u32;
    // SAFETY: buf has `len` u16 slots.
    match unsafe { GetComputerNameExW(ComputerNameDnsHostname, Some(PWSTR(buf.as_mut_ptr())), &mut len) } {
        Ok(()) => String::from_utf16_lossy(&buf[..len as usize]),
        Err(_) => "unknown".into(),
    }
}

/// CPU brand string from CPUID leaves 0x80000002..=0x80000004.
pub fn cpu_brand() -> String {
    #[cfg(target_arch = "x86_64")]
    {
        use std::arch::x86_64::__cpuid;
        #[allow(unused_unsafe)]
        let max = unsafe { __cpuid(0x8000_0000) }.eax;
        if max >= 0x8000_0004 {
            let mut bytes = Vec::with_capacity(48);
            for leaf in 0x8000_0002u32..=0x8000_0004 {
                #[allow(unused_unsafe)]
                let r = unsafe { __cpuid(leaf) };
                for v in [r.eax, r.ebx, r.ecx, r.edx] {
                    bytes.extend_from_slice(&v.to_le_bytes());
                }
            }
            let s = String::from_utf8_lossy(&bytes);
            let s = s.trim_matches(|c: char| c == '\0' || c.is_whitespace());
            if !s.is_empty() {
                return s.to_string();
            }
        }
    }
    "Unknown CPU".into()
}

/// RAM (used, total) in bytes.
pub fn ram() -> (u64, u64) {
    let mut m = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    // SAFETY: dwLength set as required.
    match unsafe { GlobalMemoryStatusEx(&mut m) } {
        Ok(()) => (m.ullTotalPhys.saturating_sub(m.ullAvailPhys), m.ullTotalPhys),
        Err(_) => (0, 0),
    }
}

/// Tracks total and per-core CPU busy % between calls.
pub struct CpuTimes {
    total_prev: Times,
    cores_prev: Vec<Times>,
    raw: Vec<ProcessorPerf>,
}

impl CpuLoad for CpuTimes {
    fn new(logical_cores: usize) -> Self {
        Self {
            total_prev: Times::default(),
            cores_prev: vec![Times::default(); logical_cores],
            raw: vec![ProcessorPerf::default(); logical_cores],
        }
    }

    /// Returns (total %, per-core %) since the previous call. The first call returns
    /// averages since boot; callers discard it by priming.
    fn update(&mut self) -> (f32, Vec<f32>) {
        let total_now = {
            let (mut i, mut k, mut u) = (FILETIME::default(), FILETIME::default(), FILETIME::default());
            // SAFETY: valid out-pointers.
            match unsafe { GetSystemTimes(Some(&mut i), Some(&mut k), Some(&mut u)) } {
                Ok(()) => Times { idle: filetime(i), total: filetime(k) + filetime(u) },
                Err(_) => self.total_prev,
            }
        };
        let total = delta(total_now, self.total_prev);
        self.total_prev = total_now;

        let mut ret = 0u32;
        let size = (self.raw.len() * std::mem::size_of::<ProcessorPerf>()) as u32;
        // SAFETY: raw has room for `size` bytes.
        let status = unsafe {
            NtQuerySystemInformation(SystemProcessorPerformanceInformation, self.raw.as_mut_ptr().cast(), size, &mut ret)
        };
        let mut cores = Vec::with_capacity(self.raw.len());
        if status.is_ok() {
            for (i, p) in self.raw.iter().enumerate() {
                let now = Times { idle: p.idle_time as u64, total: (p.kernel_time + p.user_time) as u64 };
                cores.push(crate::aggregate::round1(delta(now, self.cores_prev[i])));
                self.cores_prev[i] = now;
            }
        }
        (crate::aggregate::round1(total), cores)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_facts_look_sane() {
        let cores = logical_cores();
        assert!(cores >= 1);
        assert!(!cpu_brand().is_empty());
        assert!(!hostname().is_empty());
        let (used, total) = ram();
        assert!(total > 0 && used <= total);
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
