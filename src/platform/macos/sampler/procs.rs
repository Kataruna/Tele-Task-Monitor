//! Process snapshot. Per-process CPU % is computed from these snapshots by the portable
//! [`crate::sampler::cpu::CpuTracker`].

use crate::platform::ProcessSource;
use crate::sampler::cpu::RawProc;

pub struct ProcSnapshot;

impl ProcessSource for ProcSnapshot {
    fn new() -> Self {
        Self
    }

    fn take(&mut self) -> Result<Vec<RawProc>, String> {
        // TODO(macos): `proc_listpids(PROC_ALL_PIDS)`, then per PID:
        //   - name:        `proc_name` (or the bundle name for .app processes)
        //   - cpu_time:    `proc_pid_rusage(RUSAGE_INFO_V2)` ri_user_time + ri_system_time
        //                  (Mach absolute time: convert with `mach_timebase_info`, then ns / 100)
        //   - create_time: `ri_proc_start_abstime`
        //   - mem:         `ri_phys_footprint` (Activity Monitor's "Memory" column)
        // Processes of other users / root are not readable without privileges; skip them.
        Ok(Vec::new())
    }

    fn release(&mut self) {}
}
