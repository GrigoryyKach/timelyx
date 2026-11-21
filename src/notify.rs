//! Desktop notifications for daily limits. A toast never blocks anything and has no buttons.
//!
//! Windows shows toasts only for an AppUserModelID that belongs to a Start-menu shortcut. `register_identity` tags
//! (or creates) `Start Menu\Programs\Timelyx.lnk` with the ID; the NIMVORO launcher creates the same shortcut at
//! install time. A test instance (`TIMELYX_DATA_DIR` set) never touches the Start menu and shows no toasts.

#[cfg(windows)]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(windows)]
const AUMID: &str = "Timelyx.App";

#[cfg(windows)]
static IDENTITY_OK: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
pub fn register_identity() -> bool {
    if crate::config::is_test_instance() {
        return false;
    }
    let ok = tag_shortcut().is_ok();
    IDENTITY_OK.store(ok, Ordering::Relaxed);
    ok
}

#[cfg(windows)]
fn shortcut_path() -> Option<std::path::PathBuf> {
    Some(dirs::config_dir()?.join("Microsoft").join("Windows").join("Start Menu").join("Programs").join("Timelyx.lnk"))
}

#[cfg(windows)]
fn tag_shortcut() -> windows::core::Result<()> {
    use windows::core::{Interface, PCWSTR};
    use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
    use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, STGM_READWRITE};
    use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    let path = shortcut_path().ok_or_else(windows::core::Error::empty)?;
    let exe = std::env::current_exe().map_err(|_| windows::core::Error::empty())?;
    let wide = |p: &std::path::Path| -> Vec<u16> { p.as_os_str().to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect() };
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        let file: IPersistFile = link.cast()?;
        let wpath = wide(&path);
        let existed = path.exists();
        if existed {
            file.Load(PCWSTR(wpath.as_ptr()), STGM_READWRITE)?;
        } else {
            let wexe = wide(&exe);
            link.SetPath(PCWSTR(wexe.as_ptr()))?;
            link.SetIconLocation(PCWSTR(wexe.as_ptr()), 0)?;
            let desc = wide(std::path::Path::new("Timelyx"));
            link.SetDescription(PCWSTR(desc.as_ptr()))?;
        }
        let store: IPropertyStore = link.cast()?;
        if existed {
            if let Ok(v) = store.GetValue(&PKEY_AppUserModel_ID) {
                if v.to_string() == AUMID {
                    return Ok(());
                }
            }
        }
        store.SetValue(&PKEY_AppUserModel_ID, &PROPVARIANT::from(AUMID))?;
        store.Commit()?;
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        file.Save(PCWSTR(wpath.as_ptr()), true)?;
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn register_identity() -> bool {
    true
}

/// Shows one notification on its own thread, so the caller (the message loop) never waits for the system.
pub fn toast(title: &str, body: &str) {
    if crate::config::is_test_instance() {
        return;
    }
    let (title, body) = (title.to_string(), body.to_string());
    let _ = std::thread::Builder::new().name("timelyx-toast".into()).spawn(move || {
        let _ = show(&title, &body);
    });
}

#[cfg(windows)]
fn show(title: &str, body: &str) -> Result<(), String> {
    use tauri_winrt_notification::Toast;
    let id = if IDENTITY_OK.load(Ordering::Relaxed) { AUMID } else { Toast::POWERSHELL_APP_ID };
    Toast::new(id).title(title).text1(body).show().map_err(|e| e.to_string())
}

#[cfg(not(windows))]
fn show(title: &str, body: &str) -> Result<(), String> {
    notify_rust::Notification::new().appname("Timelyx").summary(title).body(body).show().map(|_| ()).map_err(|e| e.to_string())
}
