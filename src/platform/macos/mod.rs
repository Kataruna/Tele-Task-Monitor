//! macOS backend (Apple Silicon and Intel, macOS 11+).
//!
//! - `sampler/system.rs` — CPU % (`host_processor_info`), RAM (`host_statistics64`), `sysctl`
//! - `sampler/procs.rs`  — processes (`proc_listpids` + `proc_pid_rusage`)
//! - `sampler/gpu.rs`    — GPU usage / memory (IOKit `IOAccelerator` "PerformanceStatistics")
//! - `tray.rs`           — menu-bar item (`NSStatusItem` via objc2-app-kit)
//! - `priority.rs`       — utility QoS + nice 10
//! - `dialog.rs`         — alerts via `osascript`
//!
//! Known gaps versus Windows (no public macOS API): per-app GPU %, GPU temperature / power, and
//! CPU / memory of other users' processes (root daemons) without running as root.
//!
//! Package as an app with `scripts/bundle-macos.sh`.

mod ffi;

pub mod dialog;
pub mod priority;
pub mod sampler;
pub mod tray;

use std::path::{Path, PathBuf};

/// Inside an `.app` bundle the working directory is `/` and the bundle should stay unmodified
/// (it is code-signed), so settings live in `~/Library/Application Support/Tele-Task-Monitor/`.
/// A bare binary run from a terminal keeps the portable behaviour (cwd / next to the binary).
pub fn settings_dir(exe_dir: Option<&Path>) -> Option<PathBuf> {
    let in_bundle = exe_dir.is_some_and(|d| d.ends_with("Contents/MacOS"));
    if !in_bundle {
        return None;
    }
    let dir = PathBuf::from(std::env::var_os("HOME")?).join("Library/Application Support/Tele-Task-Monitor");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("warning: cannot create {}: {e}", dir.display());
        return None;
    }
    Some(dir)
}
