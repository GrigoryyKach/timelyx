//! Linux X11 event source: the active window, idle time and the screen saver.
//!
//! Event driven through `x11rb`: a thread blocks in `wait_for_event` and wakes only for `_NET_ACTIVE_WINDOW` on the
//! root window, `_NET_WM_NAME` / `_NET_WM_STATE` on the active window and the screen saver. Idle time comes
//! from the XScreenSaver extension once every few seconds. On Wayland nothing is tracked: the active window
//! cannot be seen there, and the tracker says so instead of recording guesses.

use crate::daemon::{self, Availability};
use crate::tracker::RawWindow;
use crate::x11logic::{is_wayland_session, Out, Translator, XEvent, XQuery};
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::Duration;
use x11rb::connection::Connection;
use x11rb::protocol::screensaver::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{AtomEnum, ChangeWindowAttributesAux, ConnectionExt as _, EventMask, Window};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

/// The foreground as the daemon sees it. Same shape as the Windows source.
pub enum Fg {
    Window(RawWindow),
    None,
    #[allow(dead_code)]
    Ignore,
}

struct Atoms {
    active: u32,
    net_wm_name: u32,
    utf8: u32,
    pid: u32,
    state: u32,
    fullscreen: u32,
}

struct Conn {
    c: RustConnection,
    root: Window,
    atoms: Atoms,
    /// Only the event connection subscribes to property changes.
    watching: bool,
    watched: Option<Window>,
}

impl Conn {
    fn connect(watching: bool) -> Option<Conn> {
        let (c, screen) = RustConnection::connect(None).ok()?;
        let root = c.setup().roots.get(screen)?.root;
        let atom = |name: &[u8]| -> Option<u32> { Some(c.intern_atom(false, name).ok()?.reply().ok()?.atom) };
        let atoms = Atoms {
            active: atom(b"_NET_ACTIVE_WINDOW")?,
            net_wm_name: atom(b"_NET_WM_NAME")?,
            utf8: atom(b"UTF8_STRING")?,
            pid: atom(b"_NET_WM_PID")?,
            state: atom(b"_NET_WM_STATE")?,
            fullscreen: atom(b"_NET_WM_STATE_FULLSCREEN")?,
        };
        Some(Conn { c, root, atoms, watching, watched: None })
    }

    fn prop32(&self, win: Window, prop: u32, ty: impl Into<u32>) -> Option<Vec<u32>> {
        let r = self.c.get_property(false, win, prop, ty, 0, 256).ok()?.reply().ok()?;
        let values: Vec<u32> = r.value32()?.collect();
        Some(values)
    }
}

impl XQuery for Conn {
    fn active_window(&mut self) -> Option<u32> {
        self.prop32(self.root, self.atoms.active, AtomEnum::WINDOW)?.first().copied().filter(|w| *w != 0)
    }

    fn title(&mut self, win: u32) -> Option<String> {
        let r = self.c.get_property(false, win, self.atoms.net_wm_name, self.atoms.utf8, 0, 1024).ok()?.reply().ok()?;
        if !r.value.is_empty() {
            return Some(String::from_utf8_lossy(&r.value).into_owned());
        }
        let r = self.c.get_property(false, win, AtomEnum::WM_NAME, AtomEnum::ANY, 0, 1024).ok()?.reply().ok()?;
        (!r.value.is_empty()).then(|| String::from_utf8_lossy(&r.value).into_owned())
    }

    fn pid(&mut self, win: u32) -> Option<u32> {
        self.prop32(win, self.atoms.pid, AtomEnum::CARDINAL)?.first().copied()
    }

    fn exe_of_pid(&mut self, pid: u32) -> Option<String> {
        std::fs::read_link(format!("/proc/{pid}/exe")).ok().map(|p| p.to_string_lossy().trim_end_matches(" (deleted)").to_string())
    }

    fn fullscreen(&mut self, win: u32) -> bool {
        self.prop32(win, self.atoms.state, AtomEnum::ATOM).is_some_and(|v| v.contains(&self.atoms.fullscreen))
    }

    fn watch(&mut self, win: u32) {
        if !self.watching || self.watched == Some(win) {
            return;
        }
        let aux = ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE);
        let _ = self.c.change_window_attributes(win, &aux);
        let _ = self.c.flush();
        self.watched = Some(win);
    }
}

thread_local! {
    /// The connection of the calling thread for plain reads (the periodic re-look and the idle check).
    static READER: RefCell<Option<(Conn, Translator)>> = const { RefCell::new(None) };
}

fn with_reader<R>(f: impl FnOnce(&mut Conn, &mut Translator) -> R) -> Option<R> {
    READER.with(|r| {
        let mut b = r.borrow_mut();
        if b.is_none() {
            *b = Some((Conn::connect(false)?, Translator::new()));
        }
        let (c, t) = b.as_mut()?;
        Some(f(c, t))
    })
}

/// The foreground window now.
pub fn foreground() -> Fg {
    match with_reader(|c, t| t.current(c)) {
        Some(Some(w)) => Fg::Window(w),
        _ => Fg::None,
    }
}

/// Seconds since the last keyboard or mouse input (XScreenSaver extension).
pub fn idle_secs() -> Option<i64> {
    with_reader(|c, _| {
        let info = c.c.screensaver_query_info(c.root).ok()?.reply().ok()?;
        Some((info.ms_since_user_input / 1000) as i64)
    })
    .flatten()
}

/// Can we track at all? Wayland: no. No X server: an error, shown in the window.
pub fn availability() -> Availability {
    if is_wayland_session(&|k| std::env::var(k).ok()) {
        return Availability::Wayland;
    }
    match Conn::connect(false) {
        Some(_) => Availability::Ok,
        None => Availability::Failed("no X11 display".into()),
    }
}

static QUIT: Mutex<Option<Sender<()>>> = Mutex::new(None);
static SIGNALLED: AtomicBool = AtomicBool::new(false);

/// Asks `run` to return (from any thread).
pub fn request_quit() {
    if let Ok(g) = QUIT.lock() {
        if let Some(tx) = g.as_ref() {
            let _ = tx.send(());
        }
    }
}

extern "C" fn on_signal(_: libc::c_int) {
    SIGNALLED.store(true, Ordering::Relaxed);
}

fn event_loop() {
    let Some(mut conn) = Conn::connect(true) else { return };
    let aux = ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE);
    let _ = conn.c.change_window_attributes(conn.root, &aux);
    // The screen saver tells us when the session is locked or blanked.
    let _ = conn.c.screensaver_select_input(conn.root, screensaver::Event::NOTIFY_MASK);
    let _ = conn.c.flush();
    let mut tr = Translator::new();
    let _ = tr.current(&mut conn);
    loop {
        let ev = match conn.c.wait_for_event() {
            Ok(e) => e,
            Err(_) => return, // the X server went away
        };
        let xe = match ev {
            Event::PropertyNotify(e) if e.window == conn.root && e.atom == conn.atoms.active => XEvent::ActiveWindowChanged,
            Event::PropertyNotify(e) if e.atom == conn.atoms.net_wm_name || e.atom == u32::from(AtomEnum::WM_NAME) => XEvent::TitleChanged(e.window),
            Event::PropertyNotify(e) if e.atom == conn.atoms.state => XEvent::StateChanged(e.window),
            Event::ScreensaverNotify(e) => XEvent::ScreenSaver(e.state != screensaver::State::OFF),
            _ => continue,
        };
        match tr.handle(&mut conn, xe) {
            Some(Out::Focus(w)) => {
                daemon::with(|d| d.on_focus_raw(w));
            }
            Some(Out::Lock) => {
                daemon::with(|d| d.on_lock());
            }
            Some(Out::Unlock) => {
                daemon::with(|d| d.on_unlock());
            }
            None => {}
        }
    }
}

/// Runs the tracker until it is told to quit (the tray's Exit, `request_quit`, SIGTERM or SIGINT).
pub fn run(on_ready: impl FnOnce()) {
    let (tx, rx) = channel::<()>();
    if let Ok(mut g) = QUIT.lock() {
        *g = Some(tx);
    }
    // SAFETY: the handler only stores into an atomic.
    unsafe {
        libc::signal(libc::SIGTERM, on_signal as *const () as usize);
        libc::signal(libc::SIGINT, on_signal as *const () as usize);
    }
    on_ready();
    let available = daemon::with(|d| d.availability == Availability::Ok).unwrap_or(false);
    if available {
        let _ = std::thread::Builder::new().name("timelyx-x11".into()).spawn(event_loop);
        let _ = std::thread::Builder::new().name("timelyx-tick".into()).spawn(|| loop {
            std::thread::sleep(Duration::from_secs(daemon::TICK_SECS));
            daemon::with(|d| d.on_tick());
        });
        daemon::with(|d| d.on_focus());
    }
    loop {
        match rx.recv_timeout(Duration::from_secs(2)) {
            Err(RecvTimeoutError::Timeout) => {
                if SIGNALLED.load(Ordering::Relaxed) {
                    return;
                }
            }
            _ => return,
        }
    }
}
