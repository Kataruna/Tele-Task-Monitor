//! Menu-bar (status bar) item — the macOS counterpart of the Windows tray icon.

/// Runs on the main thread until the user quits.
pub fn run(port: u16, lan_urls: Vec<(String, String)>) {
    // TODO(macos): create an `NSStatusItem` with the same menu as Windows (Open Dashboard,
    // Settings, copy LAN URLs, Quit) and run `NSApplication`'s event loop here. Ship the app as a
    // `.app` bundle with `LSUIElement = true` in Info.plist so it has no Dock icon.
    let _ = lan_urls;
    eprintln!("menu-bar UI not implemented yet; serving on port {port}. Press Ctrl+C to quit.");
    loop {
        std::thread::park();
    }
}

/// Refresh the menu after the port was changed in Settings.
pub fn update_port(new_port: u16) {
    // TODO(macos): rebuild the menu's URLs via `crate::lan::collect_lan_urls(new_port)`.
    let _ = new_port;
}
