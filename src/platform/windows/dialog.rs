//! Blocking message box (`MessageBoxW`), used for start-up errors and `--help` since the app has
//! no console window.

pub fn show_message_box(title: &str, text: &str, is_error: bool) {
    use windows::Win32::UI::WindowsAndMessaging::{
        MB_ICONERROR, MB_ICONINFORMATION, MB_OK, MessageBoxW,
    };
    use windows::core::PCWSTR;
    let wtitle: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let wtext: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let flags = if is_error { MB_OK | MB_ICONERROR } else { MB_OK | MB_ICONINFORMATION };
    unsafe {
        let _ = MessageBoxW(None, PCWSTR(wtext.as_ptr()), PCWSTR(wtitle.as_ptr()), flags);
    }
}
