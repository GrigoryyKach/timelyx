//! Windows glue: starting the two processes and the autostart registry value.

use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const DETACHED_PROCESS: u32 = 0x0000_0008;

pub const WINDOW_TITLE: &str = "Timelyx";

/// Starts the window process (the interface). It runs on its own and exits when its window closes.
pub fn spawn_ui() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = Command::new(exe).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).creation_flags(CREATE_NO_WINDOW).spawn();
    }
}

/// Starts the background tracker as a separate process.
pub fn spawn_background() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = Command::new(exe)
            .arg("--background")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
            .spawn();
    }
}

/// The launching process may hand the foreground right to the window that is already open.
pub fn allow_foreground() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, ASFW_ANY};
    // SAFETY: plain Win32 call.
    unsafe { AllowSetForegroundWindow(ASFW_ANY) };
}

// ───────────────────────── autostart ─────────────────────────

const REAL_RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const TEST_RUN_KEY: &str = "Software\\Timelyx\\Test\\Run";
const RUN_VALUE: &str = "Timelyx";

/// The registry key that holds the startup value. A test instance (own data folder) and the
/// `TIMELYX_AUTOSTART_KEY` override use a private key, so a test never touches the user's real Run key.
fn run_key() -> String {
    if let Ok(k) = std::env::var("TIMELYX_AUTOSTART_KEY") {
        if !k.is_empty() {
            return k;
        }
    }
    if crate::config::is_test_instance() { TEST_RUN_KEY.to_string() } else { REAL_RUN_KEY.to_string() }
}

/// The command stored in the value: the exe path in quotes and `--background`.
pub fn autostart_command(exe: &std::path::Path) -> String {
    format!("\"{}\" --background", exe.display())
}

/// The real registry state, not a remembered setting.
pub fn autostart_enabled() -> bool {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(run_key())
        .and_then(|k| k.get_value::<String, _>(RUN_VALUE))
        .is_ok_and(|v| !v.is_empty())
}

/// Adds or removes the startup value. Returns whether the operation succeeded.
pub fn set_autostart(on: bool) -> bool {
    use winreg::{enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE}, RegKey};
    let Ok((key, _)) = RegKey::predef(HKEY_CURRENT_USER).create_subkey_with_flags(run_key(), KEY_SET_VALUE | KEY_READ) else {
        return false;
    };
    if on {
        let Ok(exe) = std::env::current_exe() else { return false };
        key.set_value(RUN_VALUE, &autostart_command(&exe)).is_ok()
    } else {
        match key.delete_value(RUN_VALUE) {
            Ok(()) => true,
            Err(e) => e.kind() == std::io::ErrorKind::NotFound,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autostart_goes_to_a_private_key_and_round_trips() {
        let _env = crate::config::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: tests in this module run in one thread each; the variable is only read by `run_key`.
        unsafe { std::env::set_var("TIMELYX_AUTOSTART_KEY", "Software\\Timelyx\\UnitTest\\Run") };
        assert!(!autostart_enabled());
        assert!(set_autostart(true));
        assert!(autostart_enabled());
        assert!(set_autostart(false));
        assert!(!autostart_enabled());
        // Removing what is not there is fine.
        assert!(set_autostart(false));
        // Clean up the private key.
        use winreg::{enums::HKEY_CURRENT_USER, RegKey};
        let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all("Software\\Timelyx\\UnitTest");
        unsafe { std::env::remove_var("TIMELYX_AUTOSTART_KEY") };
    }

    #[test]
    fn command_quotes_the_path() {
        let c = autostart_command(std::path::Path::new(r"C:\Program Files\Timelyx\Timelyx.exe"));
        assert_eq!(c, "\"C:\\Program Files\\Timelyx\\Timelyx.exe\" --background");
    }
}
