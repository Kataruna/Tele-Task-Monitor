//! Windows 10/11 backend: Win32 / NT / DXGI / D3DKMT / PDH.

pub mod dialog;
pub mod priority;
pub mod sampler;
pub mod tray;

/// Windows keeps `settings.json` next to the exe / in the working directory (portable app).
pub fn settings_dir(_exe_dir: Option<&std::path::Path>) -> Option<std::path::PathBuf> {
    None
}
