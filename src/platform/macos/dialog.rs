//! Blocking alert for start-up errors and `--help` (an app launched from Finder has no visible
//! terminal). Uses `osascript`, so no AppKit dependency is needed for this rare path.

use std::process::Command;

pub fn show_message_box(title: &str, text: &str, is_error: bool) {
    eprintln!("{title}: {text}");
    let style = if is_error { "critical" } else { "informational" };
    // Title and text are passed as argv, never spliced into the script, so no quoting issues.
    let script = format!("on run argv\ndisplay alert (item 1 of argv) message (item 2 of argv) as {style}\nend run");
    let _ = Command::new("/usr/bin/osascript").arg("-e").arg(script).arg(title).arg(text).status();
}
