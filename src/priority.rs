//! Make this process yield to games: below-normal priority + EcoQoS (efficiency mode).

use windows::Win32::System::Threading::{
    BELOW_NORMAL_PRIORITY_CLASS, GetCurrentProcess, PROCESS_POWER_THROTTLING_CURRENT_VERSION,
    PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_POWER_THROTTLING_STATE, ProcessPowerThrottling, SetPriorityClass,
    SetProcessInformation,
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
