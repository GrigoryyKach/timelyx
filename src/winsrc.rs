//! Windows event source: the foreground window, idle time, sleep, lock and shutdown.
//!
//! Event driven. A WinEvent hook reports every foreground change and every title change of the foreground window;
//! nothing polls. The only timer is the idle check every few seconds (GetLastInputInfo). The message loop runs on
//! the tracker's main thread together with the hidden window that receives power and session notifications.

use crate::daemon;
use crate::tracker::RawWindow;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::RemoteDesktop::{WTSRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION};
use windows_sys::Win32::System::SystemInformation::GetTickCount;
use windows_sys::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION};
use windows_sys::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClassNameW, GetForegroundWindow, GetMessageW, GetWindowRect,
    GetWindowTextW, GetWindowThreadProcessId, KillTimer, PostMessageW, PostQuitMessage, RegisterClassW, SetTimer, TranslateMessage, MSG,
    WNDCLASSW,
};

const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;
const EVENT_OBJECT_NAMECHANGE: u32 = 0x800C;
const OBJID_WINDOW: i32 = 0;
const WINEVENT_OUTOFCONTEXT: u32 = 0;

const WM_TIMER: u32 = 0x0113;
const WM_POWERBROADCAST: u32 = 0x0218;
const WM_WTSSESSION_CHANGE: u32 = 0x02B1;
const WM_QUERYENDSESSION: u32 = 0x0011;
const WM_ENDSESSION: u32 = 0x0016;
const WM_APP: u32 = 0x8000;
/// Posted from other threads: leave the loop.
pub const WM_QUIT_REQUEST: u32 = WM_APP + 1;
/// Posted from other threads: the tray should redraw (see `tray.rs`).
pub const WM_TRAY_REFRESH: u32 = WM_APP + 2;
const PBT_APMSUSPEND: usize = 0x0004;
const PBT_APMRESUMESUSPEND: usize = 0x0007;
const PBT_APMRESUMEAUTOMATIC: usize = 0x0012;
const WTS_SESSION_LOCK: usize = 0x7;
const WTS_SESSION_UNLOCK: usize = 0x8;
const WTS_CONSOLE_DISCONNECT: usize = 0x2;
const WTS_REMOTE_DISCONNECT: usize = 0x4;
const WTS_CONSOLE_CONNECT: usize = 0x1;
const TIMER_ID: usize = 1;

static MAIN_HWND: AtomicIsize = AtomicIsize::new(0);
/// A focus event arrived while the daemon was busy; the next tick looks again.
static FOCUS_PENDING: AtomicBool = AtomicBool::new(false);

/// What the foreground looks like right now.
pub enum Fg {
    Window(RawWindow),
    /// No window in focus (lock screen, between windows).
    None,
    /// A transient shell window (Alt+Tab, the taskbar): keep what we had.
    Ignore,
}

const IGNORED_CLASSES: &[&str] = &["XamlExplorerHostIslandWindow", "MultitaskingViewFrame", "ForegroundStaging", "TaskSwitcherWnd", "Shell_TrayWnd", "NotifyIconOverflowWindow", "TopLevelWindowForOverflowXamlIsland"];
const SHELL_DESKTOP_CLASSES: &[&str] = &["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd"];

fn wide_to_string(buf: &[u16], len: i32) -> String {
    String::from_utf16_lossy(&buf[..(len.max(0) as usize).min(buf.len())])
}

struct ExeCache {
    map: HashMap<u32, (String, Instant)>,
}

static EXE_CACHE: Mutex<Option<ExeCache>> = Mutex::new(None);
const EXE_TTL: Duration = Duration::from_secs(600);

/// Full path of the program of a process. Resolved once and cached for ten minutes (a process id can be reused).
fn exe_of_pid(pid: u32) -> Option<String> {
    if let Ok(mut g) = EXE_CACHE.lock() {
        let c = g.get_or_insert_with(|| ExeCache { map: HashMap::new() });
        if let Some((p, at)) = c.map.get(&pid) {
            if at.elapsed() < EXE_TTL {
                return Some(p.clone());
            }
        }
    }
    // SAFETY: documented Win32 calls with valid buffers.
    let path = unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len);
        CloseHandle(h);
        if ok == 0 {
            return None;
        }
        String::from_utf16_lossy(&buf[..len as usize])
    };
    if let Ok(mut g) = EXE_CACHE.lock() {
        let c = g.get_or_insert_with(|| ExeCache { map: HashMap::new() });
        if c.map.len() > 256 {
            c.map.clear();
        }
        c.map.insert(pid, (path.clone(), Instant::now()));
    }
    Some(path)
}

/// Does the window cover its whole monitor (a game or a full screen video)? The desktop and the taskbar do not count.
fn is_fullscreen(hwnd: HWND, class: &str) -> bool {
    if SHELL_DESKTOP_CLASSES.contains(&class) {
        return false;
    }
    // SAFETY: documented Win32 calls with valid out parameters.
    unsafe {
        let mut r: RECT = std::mem::zeroed();
        if GetWindowRect(hwnd, &mut r) == 0 {
            return false;
        }
        let mon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(mon, &mut mi) == 0 {
            return false;
        }
        let m = mi.rcMonitor;
        let tol = 2;
        r.left <= m.left + tol && r.top <= m.top + tol && r.right >= m.right - tol && r.bottom >= m.bottom - tol
    }
}

/// The foreground window: program, title and whether it is full screen.
pub fn foreground() -> Fg {
    // SAFETY: documented Win32 calls with valid buffers.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return Fg::None;
        }
        let mut cls = [0u16; 128];
        let n = GetClassNameW(hwnd, cls.as_mut_ptr(), cls.len() as i32);
        let class = wide_to_string(&cls, n);
        if IGNORED_CLASSES.contains(&class.as_str()) {
            return Fg::Ignore;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return Fg::None;
        }
        let Some(exe) = exe_of_pid(pid) else { return Fg::None };
        let mut title = [0u16; 1024];
        let n = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32);
        Fg::Window(RawWindow { exe, title: wide_to_string(&title, n), fullscreen: is_fullscreen(hwnd, &class) })
    }
}

/// Seconds since the last keyboard or mouse input.
pub fn idle_secs() -> Option<i64> {
    // SAFETY: `info` is initialised with its size as the API requires.
    unsafe {
        let mut info = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
        if GetLastInputInfo(&mut info) == 0 {
            return None;
        }
        let ms = GetTickCount().wrapping_sub(info.dwTime);
        Some((ms / 1000) as i64)
    }
}

unsafe extern "system" fn on_event(_hook: HWINEVENTHOOK, event: u32, hwnd: HWND, id_object: i32, _id_child: i32, _thread: u32, _time: u32) {
    if event == EVENT_OBJECT_NAMECHANGE {
        // Only the title of the window in front matters.
        // SAFETY: plain Win32 call.
        if id_object != OBJID_WINDOW || hwnd.is_null() || hwnd != unsafe { GetForegroundWindow() } {
            return;
        }
    }
    if daemon::try_with(|d| d.on_focus()).is_none() {
        FOCUS_PENDING.store(true, Ordering::Relaxed);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_TIMER => {
            if FOCUS_PENDING.swap(false, Ordering::Relaxed) {
                daemon::with(|d| d.on_focus());
            }
            daemon::with(|d| d.on_tick());
            0
        }
        WM_POWERBROADCAST => {
            match wp {
                PBT_APMSUSPEND => {
                    daemon::with(|d| d.on_suspend());
                }
                PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => {
                    daemon::with(|d| d.on_resume());
                }
                _ => {}
            }
            1
        }
        WM_WTSSESSION_CHANGE => {
            match wp {
                WTS_SESSION_LOCK | WTS_CONSOLE_DISCONNECT | WTS_REMOTE_DISCONNECT => {
                    daemon::with(|d| d.on_lock());
                }
                WTS_SESSION_UNLOCK | WTS_CONSOLE_CONNECT => {
                    daemon::with(|d| d.on_unlock());
                }
                _ => {}
            }
            0
        }
        WM_QUERYENDSESSION => 1,
        WM_ENDSESSION => {
            if wp != 0 {
                daemon::with(|d| d.on_shutdown());
            }
            0
        }
        WM_TRAY_REFRESH => {
            crate::tray::apply_pending();
            0
        }
        WM_QUIT_REQUEST => {
            // SAFETY: plain Win32 calls on our own window.
            unsafe {
                KillTimer(hwnd, TIMER_ID);
                DestroyWindow(hwnd);
                PostQuitMessage(0);
            }
            0
        }
        // SAFETY: forwarding to the default procedure with the original arguments.
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Asks the message loop to end (from any thread).
pub fn request_quit() {
    let h = MAIN_HWND.load(Ordering::Relaxed);
    if h != 0 {
        // SAFETY: posting to a window handle of this process.
        unsafe { PostMessageW(h as HWND, WM_QUIT_REQUEST, 0, 0) };
    }
}

/// Asks the message loop to redraw the tray (from any thread).
pub fn request_tray_refresh() {
    let h = MAIN_HWND.load(Ordering::Relaxed);
    if h != 0 {
        // SAFETY: posting to a window handle of this process.
        unsafe { PostMessageW(h as HWND, WM_TRAY_REFRESH, 0, 0) };
    }
}

/// Runs the tracker until it is told to quit: hidden window, hooks, timer and the message loop.
/// `on_ready` runs once the window exists (the tray is created there, on the same thread).
pub fn run(on_ready: impl FnOnce()) {
    // SAFETY: standard window class registration and message loop; every pointer outlives its use.
    unsafe {
        let hinst = GetModuleHandleW(std::ptr::null());
        let class = wide("TimelyxTrackerWindow");
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class.as_ptr(),
        };
        RegisterClassW(&wc);
        let title = wide("Timelyx tracker");
        // A top-level window that is never shown: broadcast messages (power, end of session) reach only those.
        let hwnd = CreateWindowExW(0, class.as_ptr(), title.as_ptr(), 0, 0, 0, 0, 0, std::ptr::null_mut(), std::ptr::null_mut(), hinst, std::ptr::null());
        if hwnd.is_null() {
            return;
        }
        MAIN_HWND.store(hwnd as isize, Ordering::Relaxed);
        WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION);
        let h1 = SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, std::ptr::null_mut(), Some(on_event), 0, 0, WINEVENT_OUTOFCONTEXT);
        let h2 = SetWinEventHook(EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_NAMECHANGE, std::ptr::null_mut(), Some(on_event), 0, 0, WINEVENT_OUTOFCONTEXT);
        SetTimer(hwnd, TIMER_ID, (daemon::TICK_SECS * 1000) as u32, None);
        on_ready();
        daemon::with(|d| d.on_focus());

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        if !h1.is_null() {
            UnhookWinEvent(h1);
        }
        if !h2.is_null() {
            UnhookWinEvent(h2);
        }
    }
}
