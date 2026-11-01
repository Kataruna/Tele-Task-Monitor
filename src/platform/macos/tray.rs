//! Menu-bar (status bar) item — the macOS counterpart of the Windows tray icon.
//! Same menu as Windows: Open Dashboard, Settings…, Copy iPad URL (one per LAN address), Quit.
//!
//! The menu is rebuilt from `STATE` every time it opens (`menuNeedsUpdate:`), so `update_port`,
//! which runs on the HTTP thread, only touches the mutex and never calls AppKit off the main thread.

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Mutex;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSImage, NSMenu, NSMenuDelegate, NSMenuItem, NSStatusBar,
    NSVariableStatusItemLength,
};
use objc2_foundation::NSString;

const TAG_OPEN: isize = 1;
const TAG_SETTINGS: isize = 2;
const TAG_QUIT: isize = 3;
const TAG_COPY_BASE: isize = 100;

struct TrayState {
    port: u16,
    /// `(url, interface_name)` pairs, e.g. `("http://192.168.1.50:8800/", "en0")`.
    lan_urls: Vec<(String, String)>,
}

static STATE: Mutex<TrayState> = Mutex::new(TrayState { port: 0, lan_urls: Vec::new() });

fn snapshot() -> (u16, Vec<(String, String)>) {
    let s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    (s.port, s.lan_urls.clone())
}

fn open_url(url: &str) {
    if let Err(e) = Command::new("/usr/bin/open").arg(url).spawn() {
        eprintln!("warning: could not open {url}: {e}");
    }
}

fn copy_to_clipboard(text: &str) {
    let child = Command::new("/usr/bin/pbcopy").stdin(Stdio::piped()).spawn();
    if let Ok(mut child) = child {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();
    }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `MenuHandler` does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "TeleTaskMonitorMenuHandler"]
    struct MenuHandler;

    impl MenuHandler {
        #[unsafe(method(menuAction:))]
        fn menu_action(&self, sender: &NSMenuItem) {
            handle_action(sender.tag(), MainThreadMarker::from(self));
        }
    }

    unsafe impl NSObjectProtocol for MenuHandler {}

    unsafe impl NSMenuDelegate for MenuHandler {
        #[unsafe(method(menuNeedsUpdate:))]
        fn menu_needs_update(&self, menu: &NSMenu) {
            rebuild_menu(menu, self.as_ref(), MainThreadMarker::from(self));
        }
    }
);

impl MenuHandler {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        // SAFETY: NSObject's designated initializer.
        unsafe { msg_send![super(this), init] }
    }
}

fn handle_action(tag: isize, mtm: MainThreadMarker) {
    let (port, lan_urls) = snapshot();
    match tag {
        TAG_OPEN => open_url(&format!("http://localhost:{port}/")),
        TAG_SETTINGS => open_url(&format!("http://localhost:{port}/?settings=1")),
        TAG_QUIT => NSApplication::sharedApplication(mtm).terminate(None),
        t if t >= TAG_COPY_BASE => {
            if let Some((url, _)) = lan_urls.get((t - TAG_COPY_BASE) as usize) {
                copy_to_clipboard(url);
            }
        }
        _ => {}
    }
}

fn add_item(menu: &NSMenu, mtm: MainThreadMarker, target: &AnyObject, title: &str, tag: Option<isize>) {
    let action = tag.map(|_| sel!(menuAction:));
    // SAFETY: `menuAction:` is implemented by `MenuHandler`, which is the target.
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(title),
            action,
            &NSString::from_str(""),
        )
    };
    match tag {
        Some(tag) => {
            // SAFETY: target outlives the menu (it lives for the whole `run`).
            unsafe { item.setTarget(Some(target)) };
            item.setTag(tag);
        }
        None => item.setEnabled(false),
    }
    menu.addItem(&item);
}

fn rebuild_menu(menu: &NSMenu, target: &AnyObject, mtm: MainThreadMarker) {
    let (port, lan_urls) = snapshot();
    menu.removeAllItems();
    add_item(menu, mtm, target, &format!("Tele-Task-Monitor (Port {port})"), None);
    menu.addItem(&NSMenuItem::separatorItem(mtm));
    add_item(menu, mtm, target, &format!("Open Dashboard (http://localhost:{port}/)"), Some(TAG_OPEN));
    add_item(menu, mtm, target, "Settings…", Some(TAG_SETTINGS));
    if !lan_urls.is_empty() {
        menu.addItem(&NSMenuItem::separatorItem(mtm));
        for (i, (url, iface)) in lan_urls.iter().enumerate() {
            add_item(menu, mtm, target, &format!("Copy iPad URL: {url}  ({iface})"), Some(TAG_COPY_BASE + i as isize));
        }
    }
    menu.addItem(&NSMenuItem::separatorItem(mtm));
    add_item(menu, mtm, target, "Quit Tele-Task-Monitor", Some(TAG_QUIT));
}

/// Runs the AppKit event loop on the main thread until the user picks Quit (never returns).
pub fn run(port: u16, lan_urls: Vec<(String, String)>) {
    {
        let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
        s.port = port;
        s.lan_urls = lan_urls;
    }
    let mtm = MainThreadMarker::new().expect("tray::run must be called on the main thread");
    let app = NSApplication::sharedApplication(mtm);
    // Menu-bar only: no Dock icon and no app menu, even when run outside an .app bundle.
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let status_item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
    if let Some(button) = status_item.button(mtm) {
        let tip = NSString::from_str("Tele-Task-Monitor");
        // SF Symbols (macOS 11+); a template image follows the light/dark menu bar automatically.
        let image = ["gauge.with.dots.needle.33percent", "gauge"]
            .iter()
            .find_map(|name| NSImage::imageWithSystemSymbolName_accessibilityDescription(&NSString::from_str(name), Some(&tip)));
        match image {
            Some(img) => {
                img.setTemplate(true);
                button.setImage(Some(&img));
            }
            None => button.setTitle(&NSString::from_str("TTM")),
        }
        button.setToolTip(Some(&tip));
    }

    let handler = MenuHandler::new(mtm);
    let menu = NSMenu::new(mtm);
    menu.setAutoenablesItems(false);
    menu.setDelegate(Some(ProtocolObject::from_ref(&*handler)));
    status_item.setMenu(Some(&menu));

    app.run();
    // `handler`, `menu` and `status_item` must stay alive while the loop runs; `run` only returns
    // on terminate, which exits the process.
    drop((handler, menu, status_item));
}

/// Refresh the menu after the port was changed in Settings (takes effect next time it opens).
pub fn update_port(new_port: u16) {
    let urls = crate::lan::collect_lan_urls(new_port);
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    s.port = new_port;
    s.lan_urls = urls;
}
