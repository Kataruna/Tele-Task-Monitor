//! Native Win32 system tray icon and popup menu (`Shell_NotifyIconW`).
//! Zero external GUI dependencies and 0% CPU when idle (blocks in `GetMessageW`).

use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::{null, null_mut};
use std::sync::{Mutex, OnceLock};

use windows::Win32::Foundation::{FALSE, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS,
    DeleteObject, HDC, HGDIOBJ,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW, ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CW_USEDEFAULT, CreateIconIndirect, CreatePopupMenu, CreateWindowExW,
    DefWindowProcW, DestroyIcon, DestroyMenu, DispatchMessageW, GetCursorPos, GetMessageW, HICON,
    ICONINFO, MF_GRAYED, MF_SEPARATOR, MF_STRING, MSG, PostQuitMessage, RegisterClassW,
    RegisterWindowMessageW, SW_SHOWNORMAL, SetForegroundWindow, TPM_BOTTOMALIGN, TPM_LEFTALIGN,
    TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WINDOW_EX_STYLE, WM_COMMAND, WM_DESTROY,
    WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_RBUTTONUP, WM_USER, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};
use windows::core::PCWSTR;

const WM_TRAYICON: u32 = WM_USER + 1;
const TRAY_UID: u32 = 1001;

const IDM_OPEN_LOCAL: usize = 2001;
const IDM_OPEN_SETTINGS: usize = 2002;
const IDM_QUIT: usize = 2099;
const IDM_COPY_BASE: usize = 2100;

const CF_UNICODETEXT: u32 = 13;

struct TrayConfig {
    hwnd: isize,
    port: u16,
    /// `(url, interface_name)` pairs, e.g. `("http://192.168.1.50:8800/", "Wi-Fi")`.
    lan_urls: Vec<(String, String)>,
    taskbar_created_msg: u32,
    hicon: isize,
}

static TRAY_CFG: OnceLock<Mutex<TrayConfig>> = OnceLock::new();

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn collect_lan_urls(port: u16) -> Vec<(String, String)> {
    let addrs = if_addrs::get_if_addrs().unwrap_or_default();
    let mut out = Vec::new();
    for a in addrs.iter().filter(|a| !a.is_loopback()) {
        if let std::net::IpAddr::V4(ip) = a.ip() {
            if ip.is_link_local() {
                continue;
            }
            out.push((format!("http://{ip}:{port}/"), a.name.clone()));
        }
    }
    out
}

/// Update the tray icon tooltip and popup menu URLs when the port is changed in Settings.
pub fn update_port(new_port: u16) {
    let Some(lock) = TRAY_CFG.get() else { return };
    let Ok(mut cfg) = lock.lock() else { return };
    cfg.port = new_port;
    cfg.lan_urls = collect_lan_urls(new_port);
    if cfg.hwnd != 0 {
        let tip = if let Some((url, _)) = cfg.lan_urls.first() {
            format!("Tele-Task-Monitor — {url}")
        } else {
            format!("Tele-Task-Monitor — http://localhost:{}/", cfg.port)
        };
        let nid = fill_nid(
            HWND(cfg.hwnd as *mut c_void),
            HICON(cfg.hicon as *mut c_void),
            &tip,
        );
        unsafe {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    }
}

/// Generate a crisp 32×32 ARGB icon showing a dark rounded monitor badge with a cyan/gold pulse wave.
fn create_pulse_icon() -> HICON {
    const W: i32 = 32;
    const H: i32 = 32;

    let mut bmi: BITMAPINFO = unsafe { std::mem::zeroed() };
    bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
    bmi.bmiHeader.biWidth = W;
    bmi.bmiHeader.biHeight = -H; // top-down DIB
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB.0;

    let mut bits: *mut c_void = null_mut();
    let hbm_color = match unsafe {
        CreateDIBSection(Some(HDC(null_mut())), &bmi, DIB_RGB_COLORS, &mut bits, None, 0)
    } {
        Ok(b) if !bits.is_null() => b,
        _ => return HICON::default(),
    };

    let pixels = unsafe { std::slice::from_raw_parts_mut(bits as *mut u32, (W * H) as usize) };

    // Pulse wave y-coordinates across x = 4..27
    let wave_y: [i32; 32] = [
        17, 17, 17, 17, 17, 17, 17, 17, 17, 17, 15, 11, 7, 13, 21, 24, 19, 13, 15, 17, 17, 17, 17,
        17, 17, 17, 17, 17, 17, 17, 17, 17,
    ];

    for y in 0..H {
        for x in 0..W {
            let idx = (y * W + x) as usize;
            // Rounded corner mask (radius 6)
            let dx = if x < 6 { 6 - x } else if x > 25 { x - 25 } else { 0 };
            let dy = if y < 6 { 6 - y } else if y > 25 { y - 25 } else { 0 };
            if dx * dx + dy * dy > 36 {
                pixels[idx] = 0x00000000;
                continue;
            }

            let is_border = dx * dx + dy * dy >= 22 || x <= 1 || x >= 30 || y <= 1 || y >= 30;
            let mut color = if is_border {
                0xFF_CDC8B0
            } else {
                0xFF_262520
            };

            if !is_border && (y == 10 || y == 17 || y == 24) && x % 2 == 0 {
                color = 0xFF_38362F;
            }

            if (4..=27).contains(&x) {
                let wy = wave_y[x as usize];
                let prev_y = wave_y[(x - 1) as usize];
                let min_y = wy.min(prev_y);
                let max_y = wy.max(prev_y);
                if (y >= min_y - 1 && y <= max_y + 1) || (y - wy).abs() <= 1 {
                    color = 0xFF_4CC2FF;
                } else if (y - wy).abs() == 2 {
                    color = 0xFF_2A6B8C;
                }
            }

            pixels[idx] = color;
        }
    }

    let mask_bytes = [0u8; ((W * H) / 8) as usize];
    let hbm_mask = unsafe { CreateBitmap(W, H, 1, 1, Some(mask_bytes.as_ptr() as *const c_void)) };

    let ii = ICONINFO {
        fIcon: true.into(),
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: hbm_mask,
        hbmColor: hbm_color,
    };

    let hicon = unsafe { CreateIconIndirect(&ii).unwrap_or_default() };
    unsafe {
        let _ = DeleteObject(HGDIOBJ(hbm_color.0));
        let _ = DeleteObject(HGDIOBJ(hbm_mask.0));
    }
    hicon
}

fn fill_nid(hwnd: HWND, hicon: HICON, tip: &str) -> NOTIFYICONDATAW {
    let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_UID;
    nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    nid.uCallbackMessage = WM_TRAYICON;
    nid.hIcon = hicon;

    let wide = to_wide(tip);
    let max_len = nid.szTip.len().saturating_sub(1);
    let copy_len = wide.len().min(max_len);
    nid.szTip[..copy_len].copy_from_slice(&wide[..copy_len]);
    nid
}

fn add_tray_icon(hwnd: HWND) {
    let Some(lock) = TRAY_CFG.get() else { return };
    let Ok(mut cfg) = lock.lock() else { return };
    cfg.hwnd = hwnd.0 as isize;
    let tip = if let Some((url, _)) = cfg.lan_urls.first() {
        format!("Tele-Task-Monitor — {url}")
    } else {
        format!("Tele-Task-Monitor — http://localhost:{}/", cfg.port)
    };
    let nid = fill_nid(hwnd, HICON(cfg.hicon as *mut c_void), &tip);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);
    }
}

fn remove_tray_icon(hwnd: HWND) {
    let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_UID;
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

fn open_url(url: &str) {
    let op = to_wide("open");
    let target = to_wide(url);
    unsafe {
        let _ = ShellExecuteW(
            None,
            PCWSTR(op.as_ptr()),
            PCWSTR(target.as_ptr()),
            PCWSTR(null()),
            PCWSTR(null()),
            SW_SHOWNORMAL,
        );
    }
}

fn copy_to_clipboard(hwnd: HWND, text: &str) {
    let wide = to_wide(text);
    let byte_len = wide.len() * size_of::<u16>();
    unsafe {
        if OpenClipboard(Some(hwnd)).is_err() {
            return;
        }
        let _ = EmptyClipboard();
        if let Ok(hmem) = GlobalAlloc(GMEM_MOVEABLE, byte_len) {
            let ptr = GlobalLock(hmem);
            if !ptr.is_null() {
                std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, ptr as *mut u8, byte_len);
                let _ = GlobalUnlock(hmem);
                let _ = SetClipboardData(CF_UNICODETEXT, Some(HANDLE(hmem.0)));
            }
        }
        let _ = CloseClipboard();
    }
}

fn show_context_menu(hwnd: HWND) {
    let Some(lock) = TRAY_CFG.get() else { return };
    let (port, lan_urls) = match lock.lock() {
        Ok(c) => (c.port, c.lan_urls.clone()),
        Err(_) => return,
    };
    unsafe {
        let Ok(hmenu) = CreatePopupMenu() else { return };

        let title = to_wide(&format!("Tele-Task-Monitor (Port {port})"));
        let _ = AppendMenuW(hmenu, MF_STRING | MF_GRAYED, 0, PCWSTR(title.as_ptr()));
        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR(null()));

        let open_lbl = to_wide(&format!("Open Dashboard (http://localhost:{port}/)"));
        let _ = AppendMenuW(hmenu, MF_STRING, IDM_OPEN_LOCAL, PCWSTR(open_lbl.as_ptr()));

        let set_lbl = to_wide("Settings…");
        let _ = AppendMenuW(hmenu, MF_STRING, IDM_OPEN_SETTINGS, PCWSTR(set_lbl.as_ptr()));

        if !lan_urls.is_empty() {
            let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR(null()));
            for (i, (url, iface)) in lan_urls.iter().enumerate() {
                let lbl = to_wide(&format!("Copy iPad URL: {url}  ({iface})"));
                let _ = AppendMenuW(hmenu, MF_STRING, IDM_COPY_BASE + i, PCWSTR(lbl.as_ptr()));
            }
        }

        let _ = AppendMenuW(hmenu, MF_SEPARATOR, 0, PCWSTR(null()));
        let quit_lbl = to_wide("Quit Tele-Task-Monitor");
        let _ = AppendMenuW(hmenu, MF_STRING, IDM_QUIT, PCWSTR(quit_lbl.as_ptr()));

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(
            hmenu,
            TPM_BOTTOMALIGN | TPM_LEFTALIGN | TPM_RIGHTBUTTON,
            pt.x,
            pt.y,
            Some(0),
            hwnd,
            None,
        );
        let _ = DestroyMenu(hmenu);
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if let Some(lock) = TRAY_CFG.get() {
        if let Ok(cfg) = lock.lock() {
            if msg == cfg.taskbar_created_msg && cfg.taskbar_created_msg != 0 {
                drop(cfg);
                add_tray_icon(hwnd);
                return LRESULT(0);
            }
        }
    }

    match msg {
        WM_TRAYICON => {
            let event = (lparam.0 & 0xFFFF) as u32;
            match event {
                WM_RBUTTONUP | WM_LBUTTONUP => {
                    show_context_menu(hwnd);
                }
                WM_LBUTTONDBLCLK => {
                    if let Some(lock) = TRAY_CFG.get() {
                        if let Ok(cfg) = lock.lock() {
                            open_url(&format!("http://localhost:{}/", cfg.port));
                        }
                    }
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let cmd = wparam.0 & 0xFFFF;
            if let Some(lock) = TRAY_CFG.get() {
                let (port, lan_urls) = match lock.lock() {
                    Ok(c) => (c.port, c.lan_urls.clone()),
                    Err(_) => return LRESULT(0),
                };
                if cmd == IDM_OPEN_LOCAL {
                    open_url(&format!("http://localhost:{port}/"));
                } else if cmd == IDM_OPEN_SETTINGS {
                    open_url(&format!("http://localhost:{port}/?settings=1"));
                } else if cmd == IDM_QUIT {
                    remove_tray_icon(hwnd);
                    unsafe { PostQuitMessage(0) };
                } else if cmd >= IDM_COPY_BASE && cmd < IDM_COPY_BASE + lan_urls.len() {
                    let (url, _) = &lan_urls[cmd - IDM_COPY_BASE];
                    copy_to_clipboard(hwnd, url);
                }
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            remove_tray_icon(hwnd);
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// Run the Win32 system tray icon message loop on the calling thread until the user selects "Quit".
pub fn run(port: u16, lan_urls: Vec<(String, String)>) {
    unsafe {
        let hinstance: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
        let class_name = to_wide("TeleTaskMonitorTrayClass");
        let taskbar_created_msg = RegisterWindowMessageW(PCWSTR(to_wide("TaskbarCreated").as_ptr()));
        let hicon = create_pulse_icon();

        let _ = TRAY_CFG.set(Mutex::new(TrayConfig {
            hwnd: 0,
            port,
            lan_urls,
            taskbar_created_msg,
            hicon: hicon.0 as isize,
        }));

        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(wnd_proc);
        wc.hInstance = hinstance;
        wc.lpszClassName = PCWSTR(class_name.as_ptr());
        let _ = RegisterClassW(&wc);

        let wnd_title = to_wide("Tele-Task-Monitor");
        let Ok(hwnd) = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class_name.as_ptr()),
            PCWSTR(wnd_title.as_ptr()),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            None,
            None,
            Some(hinstance),
            None,
        ) else {
            return;
        };

        add_tray_icon(hwnd);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, None, 0, 0) != FALSE {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        remove_tray_icon(hwnd);
        if !hicon.is_invalid() {
            let _ = DestroyIcon(hicon);
        }
    }
}
