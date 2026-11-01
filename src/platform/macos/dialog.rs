//! Blocking message box for start-up errors and `--help`.

pub fn show_message_box(title: &str, text: &str, is_error: bool) {
    // TODO(macos): show a real alert (`NSAlert` via objc2-app-kit, or the `rfd` crate). An app
    // launched from Finder has no visible stderr, so start-up errors are invisible until then.
    let kind = if is_error { "error" } else { "info" };
    eprintln!("[{kind}] {title}: {text}");
}
