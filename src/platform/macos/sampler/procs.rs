//! Process snapshot via libproc (`proc_listpids` + `proc_pid_rusage` + `proc_name`).
//! Per-process CPU % is computed from these snapshots by the portable
//! [`crate::sampler::cpu::CpuTracker`].
//!
//! Limitation: without root, `proc_pid_rusage` is refused for processes of other users (system
//! daemons owned by root, `_windowserver`, …). Those are skipped, so the lists show the user's
//! own apps — which is what the dashboard is mostly about.

use crate::platform::ProcessSource;
use crate::sampler::cpu::RawProc;

const PROC_ALL_PIDS: u32 = 1;

/// Mach absolute time -> 100 ns units.
fn to_100ns(abs: u64) -> i64 {
    let (numer, denom) = crate::platform::macos::ffi::timebase();
    ((abs as u128 * numer as u128 / denom as u128) / 100) as i64
}

/// Display name: `proc_name` (up to 32 bytes, e.g. "Safari", "Google Chrome Helper"), falling
/// back to the executable's file name.
fn process_name(pid: i32, buf: &mut [u8]) -> String {
    // SAFETY: buf has buf.len() writable bytes.
    let n = unsafe { libc::proc_name(pid, buf.as_mut_ptr().cast(), buf.len() as u32) };
    if n > 0 {
        return String::from_utf8_lossy(&buf[..n as usize]).into_owned();
    }
    // SAFETY: as above.
    let n = unsafe { libc::proc_pidpath(pid, buf.as_mut_ptr().cast(), buf.len() as u32) };
    if n > 0 {
        let path = String::from_utf8_lossy(&buf[..n as usize]);
        return path.rsplit('/').next().unwrap_or_default().to_string();
    }
    String::new()
}

pub struct ProcSnapshot {
    pids: Vec<i32>,
    name_buf: Vec<u8>,
}

impl ProcessSource for ProcSnapshot {
    fn new() -> Self {
        Self { pids: Vec::new(), name_buf: Vec::new() }
    }

    fn release(&mut self) {
        self.pids = Vec::new();
        self.name_buf = Vec::new();
    }

    fn take(&mut self) -> Result<Vec<RawProc>, String> {
        if self.name_buf.is_empty() {
            self.name_buf.resize(libc::PROC_PIDPATHINFO_MAXSIZE as usize, 0);
        }
        // Size query, then fill with headroom for processes that start in between.
        // SAFETY: a null buffer asks for the needed size in bytes.
        let needed = unsafe { libc::proc_listpids(PROC_ALL_PIDS, 0, std::ptr::null_mut(), 0) };
        if needed <= 0 {
            return Err("proc_listpids failed".into());
        }
        let slots = needed as usize / std::mem::size_of::<i32>() + 64;
        if self.pids.len() < slots {
            self.pids.resize(slots, 0);
        }
        let bytes = (self.pids.len() * std::mem::size_of::<i32>()) as i32;
        // SAFETY: pids has `bytes` writable bytes.
        let filled = unsafe { libc::proc_listpids(PROC_ALL_PIDS, 0, self.pids.as_mut_ptr().cast(), bytes) };
        if filled <= 0 {
            return Err("proc_listpids failed".into());
        }
        let count = (filled as usize / std::mem::size_of::<i32>()).min(self.pids.len());

        let mut out = Vec::with_capacity(count);
        for &pid in &self.pids[..count] {
            if pid <= 0 {
                continue;
            }
            let mut ri: libc::rusage_info_v2 = unsafe { std::mem::zeroed() };
            // SAFETY: ri is a rusage_info_v2, matching the RUSAGE_INFO_V2 flavor.
            let r = unsafe {
                libc::proc_pid_rusage(pid, libc::RUSAGE_INFO_V2, (&mut ri as *mut libc::rusage_info_v2).cast())
            };
            if r != 0 {
                continue; // exited meanwhile, or another user's process (EPERM)
            }
            out.push(RawProc {
                pid: pid as u32,
                name: process_name(pid, &mut self.name_buf),
                cpu_time: to_100ns(ri.ri_user_time.saturating_add(ri.ri_system_time)),
                create_time: ri.ri_proc_start_abstime as i64,
                mem: ri.ri_phys_footprint,
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_contains_this_process() {
        let mut snap = ProcSnapshot::new();
        let procs = snap.take().expect("snapshot");
        let me = std::process::id();
        let mine = procs.iter().find(|p| p.pid == me).expect("own pid present");
        assert!(!mine.name.is_empty());
        assert!(mine.mem > 0);
        assert!(procs.len() > 5);
    }

    #[test]
    fn released_snapshot_still_works() {
        let mut snap = ProcSnapshot::new();
        snap.take().expect("snapshot");
        snap.release();
        assert!(snap.pids.is_empty());
        assert!(snap.take().expect("snapshot after release").iter().any(|p| p.pid == std::process::id()));
    }
}
