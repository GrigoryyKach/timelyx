//! Linux glue: starting the two processes and the autostart `.desktop` file.

use std::path::PathBuf;
use std::process::{Command, Stdio};

pub const WINDOW_TITLE: &str = "Timelyx";

fn spawn(args: &[&str]) {
    if let Ok(exe) = std::env::current_exe() {
        let _ = Command::new(exe).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    }
}

/// Starts the window process (the interface). It runs on its own and exits when its window closes.
pub fn spawn_ui() {
    spawn(&[]);
}

/// Starts the background tracker as a separate process.
pub fn spawn_background() {
    spawn(&["--background"]);
}

/// Nothing to hand over on X11 and Wayland: the compositor decides who gets focus.
pub fn allow_foreground() {}

// ───────────────────────── autostart ─────────────────────────

/// `~/.config/autostart/timelyx.desktop`. A test instance (own data folder) writes into that folder instead,
/// so a test never touches the user's real autostart.
fn autostart_file() -> Option<PathBuf> {
    if crate::config::is_test_instance() {
        return Some(crate::config::data_dir().join("autostart-test").join("timelyx.desktop"));
    }
    dirs::config_dir().map(|d| d.join("autostart").join("timelyx.desktop"))
}

/// A `.desktop` `Exec` value: the path in double quotes with the reserved characters escaped.
fn exec_quote(path: &str) -> String {
    let mut s = String::from("\"");
    for c in path.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            s.push('\\');
        }
        s.push(c);
    }
    s.push('"');
    s
}

fn entry(exe: &str) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=Timelyx\nComment=Automatic activity tracker\nExec={} --background\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
        exec_quote(exe)
    )
}

/// The real state of the file, not a remembered setting.
pub fn autostart_enabled() -> bool {
    autostart_file().and_then(|f| std::fs::read_to_string(f).ok()).is_some_and(|t| t.lines().any(|l| l.starts_with("Exec=") && l.ends_with("--background")))
}

/// Writes or removes the autostart file (`"<exe>" --background`). Returns whether it succeeded.
pub fn set_autostart(on: bool) -> bool {
    let Some(file) = autostart_file() else { return false };
    if on {
        let Ok(exe) = std::env::current_exe() else { return false };
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::write(file, entry(&exe.display().to_string())).is_ok()
    } else {
        match std::fs::remove_file(file) {
            Ok(()) => true,
            Err(e) => e.kind() == std::io::ErrorKind::NotFound,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autostart_entry_quotes_the_path() {
        let e = entry("/opt/my apps/Timelyx");
        assert!(e.contains("Exec=\"/opt/my apps/Timelyx\" --background\n"));
        assert_eq!(exec_quote("/a\"b$c"), "\"/a\\\"b\\$c\"");
    }

    #[test]
    fn autostart_round_trips_in_a_test_folder() {
        let _env = crate::config::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("timelyx-autostart-{}", std::process::id()));
        std::env::set_var("TIMELYX_DATA_DIR", &dir);
        assert!(!autostart_enabled());
        assert!(set_autostart(true));
        assert!(autostart_enabled());
        assert!(set_autostart(false));
        assert!(!autostart_enabled());
        assert!(set_autostart(false));
        std::env::remove_var("TIMELYX_DATA_DIR");
        let _ = std::fs::remove_dir_all(dir);
    }
}
