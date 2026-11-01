//! Raw macOS declarations that `libc` lacks or marks deprecated (it points at the `mach2` crate,
//! but these are stable Apple ABI and two functions don't justify another dependency).

use std::sync::OnceLock;

unsafe extern "C" {
    #[link_name = "mach_host_self"]
    fn mach_host_self_raw() -> libc::mach_port_t;
    #[link_name = "mach_task_self_"]
    static MACH_TASK_SELF: libc::mach_port_t;
    #[link_name = "mach_timebase_info"]
    fn mach_timebase_info_raw(info: *mut Timebase) -> libc::c_int;
}

#[repr(C)]
struct Timebase {
    numer: u32,
    denom: u32,
}

/// Mach absolute time -> nanoseconds factor `(numer, denom)`. Intel Macs tick in ns (1/1);
/// Apple Silicon at 24 MHz (125/3).
pub fn timebase() -> (u64, u64) {
    static TB: OnceLock<(u64, u64)> = OnceLock::new();
    *TB.get_or_init(|| {
        let mut tb = Timebase { numer: 0, denom: 0 };
        // SAFETY: valid out-pointer.
        unsafe { mach_timebase_info_raw(&mut tb) };
        if tb.numer == 0 || tb.denom == 0 { (1, 1) } else { (tb.numer as u64, tb.denom as u64) }
    })
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
