//! Raw macOS declarations that `libc` lacks or marks deprecated (it points at the `mach2` crate,
//! but these are stable Apple ABI and two functions don't justify another dependency).

use std::sync::OnceLock;

unsafe extern "C" {
    #[link_name = "mach_host_self"]
    fn mach_host_self_raw() -> libc::mach_port_t;
    #[link_name = "mach_task_self_"]
    static MACH_TASK_SELF: libc::mach_port_t;
}

/// Host port, fetched once: every `mach_host_self()` call adds a send right that is never freed.
pub fn host() -> libc::mach_port_t {
    static HOST: OnceLock<libc::mach_port_t> = OnceLock::new();
    // SAFETY: plain Mach call with no arguments.
    *HOST.get_or_init(|| unsafe { mach_host_self_raw() })
}

/// This task's port (what the `mach_task_self()` C macro reads).
pub fn task() -> libc::mach_port_t {
    // SAFETY: set by the kernel at process start and never written afterwards.
    unsafe { MACH_TASK_SELF }
}
