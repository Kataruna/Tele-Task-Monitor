//! Make this process yield to games and foreground apps, and give idle memory back.

/// Failures are logged and ignored: the monitor still works at normal priority.
pub fn lower_self() {
    // TODO(macos): `setpriority(PRIO_PROCESS, 0, 10)` and/or
    // `pthread_set_qos_class_self_np(QOS_CLASS_UTILITY, 0)` on the sampler thread — the macOS
    // counterpart of Windows' BELOW_NORMAL priority + EcoQoS.
}

/// macOS has no working-set trim; the kernel reclaims idle pages on its own. Intentionally a no-op.
pub fn trim_working_set() {}
