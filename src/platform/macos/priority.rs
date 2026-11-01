//! Make this process yield to games and foreground apps — the macOS counterpart of Windows'
//! BELOW_NORMAL priority + EcoQoS.

/// Called on the main thread before any other thread is spawned, so the sampler and HTTP threads
/// inherit the lower QoS. Failures are logged and ignored.
pub fn lower_self() {
    // Utility QoS: the scheduler prefers efficiency cores on Apple Silicon and lets
    // user-interactive work (games, the foreground app) run first.
    if !crate::platform::macos::ffi::set_utility_qos() {
        eprintln!("warning: could not lower QoS class");
    }
    // nice +10 for the whole process.
    // SAFETY: plain libc call on our own process.
    if unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, 10) } != 0 {
        eprintln!("warning: could not lower priority: {}", std::io::Error::last_os_error());
    }
}

/// macOS has no working-set trim; the kernel reclaims idle pages on its own. Intentionally a no-op.
pub fn trim_working_set() {}
