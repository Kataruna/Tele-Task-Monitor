//! Everything that talks to the operating system lives under `platform/`. The rest of the crate
//! (server, settings, sampling loop, aggregation, JSON model) is portable and only uses what this
//! module re-exports.
//!
//! Exactly one backend is compiled in, picked by the target OS:
//!
//! | Target    | Backend              | Status                         |
//! |-----------|----------------------|--------------------------------|
//! | Windows   | `platform/windows/`  | Complete (Win32 / NT / DXGI / PDH) |
//! | macOS     | `platform/macos/`    | Implemented (Mach / libproc / IOKit / AppKit) |
//!
//! # The contract every backend must provide
//!
//! Like a C# interface that each platform class implements: the stateful sampler parts are the
//! traits below, and the free functions are checked by the `const _` block at the end of this
//! file. A backend that is missing something, or has the wrong signature, fails to compile.
//!
//! ```text
//! platform::
//!   sampler::logical_cores() -> usize                    number of logical CPUs
//!   sampler::hostname()      -> String                   machine name shown in the dashboard
//!   sampler::cpu_brand()     -> String                   e.g. "AMD Ryzen 7 7800X3D" / "Apple M3"
//!   sampler::ram()           -> (used, total) bytes
//!   sampler::CpuTimes   : CpuLoad        whole-machine + per-core CPU %
//!   sampler::ProcSnapshot : ProcessSource  list of processes with CPU time + memory
//!   sampler::Gpu        : GpuSource      GPU identity, usage, memory, per-app GPU, temp/power
//!   priority::lower_self()          make this process yield to games / foreground apps
//!   priority::trim_working_set()    give idle memory back to the OS (may be a no-op)
//!   tray::run(port, lan_urls)       run the tray / menu-bar UI on the main thread until Quit
//!   tray::update_port(port)         refresh the tray UI after the port changed in Settings
//!   dialog::show_message_box(title, text, is_error)   blocking error / info popup
//!   settings_dir(exe_dir) -> Option<PathBuf>   per-user folder for settings.json, or None to
//!                                              keep the portable default (cwd / next to the exe)
//! ```
//!
//! Portable helpers that a backend should reuse instead of re-implementing:
//! [`crate::sampler::cpu`] (busy %, per-process CPU % tracker, [`RawProc`]),
//! [`crate::sampler::gpu::GpuReading`], [`crate::lan::collect_lan_urls`].

use crate::model::GpuInfo;
use crate::sampler::cpu::RawProc;
use crate::sampler::gpu::GpuReading;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::{dialog, priority, sampler, settings_dir, tray};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use self::macos::{dialog, priority, sampler, settings_dir, tray};

#[cfg(not(any(windows, target_os = "macos")))]
compile_error!(
    "Tele-Task-Monitor has no backend for this OS yet. Add one under src/platform/<os>/ \
     implementing the contract documented in src/platform/mod.rs."
);

/// Whole-machine CPU load. Delta-based: each `update` covers the time since the previous call.
pub trait CpuLoad {
    fn new(logical_cores: usize) -> Self;
    /// `(total %, per-core %)`, each 0–100 and rounded to one decimal; `per-core` has one entry
    /// per logical core. The first call may return averages since boot; the sampler discards it.
    fn update(&mut self) -> (f32, Vec<f32>);
}

/// Snapshot of every process the OS lets us see.
pub trait ProcessSource {
    fn new() -> Self;
    /// One snapshot. CPU time in [`RawProc`] is cumulative; the portable
    /// [`crate::sampler::cpu::CpuTracker`] turns successive snapshots into CPU %.
    fn take(&mut self) -> Result<Vec<RawProc>, String>;
    /// Free scratch memory while nobody is watching (the next `take` re-creates it).
    fn release(&mut self);
}

/// The GPU the dashboard reports on. Every part is optional: a backend that cannot read
/// something logs a warning once in `open` and returns `None` for it from then on.
pub trait GpuSource {
    /// Never fails; unavailable parts are disabled internally.
    fn open() -> Self;
    /// Name and total memory, sent once per connection. `None` hides the GPU identity.
    fn info(&self) -> Option<GpuInfo>;
    /// Total GPU memory in bytes for each sample (0 when unknown).
    fn vram_total(&self) -> u64;
    /// Take a baseline for delta-based readings without producing one (cheap).
    fn collect(&mut self);
    /// Usage since the previous `collect`/`read`. `None` hides the GPU panel and column.
    fn read(&mut self) -> Option<GpuReading>;
    /// `(temperature °C, power % of limit)`; either is `None` when not reported.
    fn perf(&self) -> (Option<f32>, Option<f32>);
    /// Free scratch memory while nobody is watching.
    fn release(&mut self);
}

// Compile-time check that the selected backend provides every free function in the contract
// with the expected signature.
const _: () = {
    let _: fn() -> usize = sampler::logical_cores;
    let _: fn() -> String = sampler::hostname;
    let _: fn() -> String = sampler::cpu_brand;
    let _: fn() -> (u64, u64) = sampler::ram;
    let _: fn() = priority::lower_self;
    let _: fn() = priority::trim_working_set;
    let _: fn(u16, Vec<(String, String)>) = tray::run;
    let _: fn(u16) = tray::update_port;
    let _: fn(&str, &str, bool) = dialog::show_message_box;
    let _: fn(Option<&std::path::Path>) -> Option<std::path::PathBuf> = settings_dir;

    fn assert_impls<C: CpuLoad, P: ProcessSource, G: GpuSource>() {}
    let _ = assert_impls::<sampler::CpuTimes, sampler::ProcSnapshot, sampler::Gpu>;
};
