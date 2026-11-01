//! macOS backend — SKELETON.
//!
//! Every item the contract in `src/platform/mod.rs` requires exists here, so the project builds
//! for macOS, but most readers still return placeholder data. Each `TODO(macos)` says which macOS
//! API is the intended replacement for the Windows one. Suggested order of work:
//!
//! 1. `sampler/system.rs` — CPU % (`host_processor_info`) and RAM used (`host_statistics64`)
//! 2. `sampler/procs.rs`  — process list (`proc_listpids` + `proc_pid_rusage`)
//! 3. `tray.rs`           — menu-bar item (`NSStatusItem`, via the `objc2-app-kit` crate)
//! 4. `priority.rs`       — background QoS / `setpriority`
//! 5. `sampler/gpu.rs`    — overall GPU % from IOKit `IOAccelerator` "PerformanceStatistics"
//!
//! Put macOS-only crates under `[target.'cfg(target_os = "macos")'.dependencies]` in Cargo.toml.

pub mod dialog;
pub mod priority;
pub mod sampler;
pub mod tray;
