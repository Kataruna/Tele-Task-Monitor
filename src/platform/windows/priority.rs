//! Make this process yield to games: below-normal priority + EcoQoS (efficiency mode), and give
//! its idle memory back.

use windows::Win32::System::Threading::{
    BELOW_NORMAL_PRIORITY_CLASS, GetCurrentProcess, PROCESS_POWER_THROTTLING_CURRENT_VERSION,
    PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_POWER_THROTTLING_STATE, ProcessPowerThrottling, SetPriorityClass,
    SetProcessInformation, SetProcessWorkingSetSize,
};

/// Failures are logged and ignored: the monitor still works at normal priority.
pub fn lower_self() {
    // SAFETY: pseudo-handle for the current process; plain Win32 calls.
    unsafe {
        if let Err(e) = SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS) {
            eprintln!("warning: could not lower priority: {e}");
        }
        let state = PROCESS_POWER_THROTTLING_STATE {
            Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
            ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
            StateMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        };
        if let Err(e) = SetProcessInformation(
            GetCurrentProcess(),
            ProcessPowerThrottling,
            (&state as *const PROCESS_POWER_THROTTLING_STATE).cast(),
            std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
        ) {
            eprintln!("warning: could not enable EcoQoS: {e}");
        }
    }
}

/// Trim this process's working set: its resident pages go back to Windows' standby list, so
/// Task Manager's memory figure drops and other programs can use the RAM. Pages that are touched
/// again simply fault back in (cheap, and they are still in RAM unless something else needed
/// them). Meant for when the monitor has been idle for a while. Failure is ignored.
pub fn trim_working_set() {
    // SAFETY: pseudo-handle for the current process; (-1, -1) is the documented "trim" request.
    unsafe {
        let _ = SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
    }
}
