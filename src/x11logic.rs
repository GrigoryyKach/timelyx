//! The logic of the Linux X11 event source, with no X11 types in it, so it is tested on every platform with a
//! mocked X server. `x11src.rs` implements [`XQuery`] with the real connection.
//!
//! The source listens for property changes: `_NET_ACTIVE_WINDOW` on the root window (the active window changed)
//! and `_NET_WM_NAME` / `_NET_WM_STATE` on the active window (the title changed, full screen entered or left).

use crate::tracker::RawWindow;

/// What the translator needs to ask the X server.
pub trait XQuery {
    /// The window named by `_NET_ACTIVE_WINDOW` on the root window; `None` when no window is active.
    fn active_window(&mut self) -> Option<u32>;
    /// `_NET_WM_NAME`, falling back to `WM_NAME`.
    fn title(&mut self, win: u32) -> Option<String>;
    /// `_NET_WM_PID`.
    fn pid(&mut self, win: u32) -> Option<u32>;
    /// The program of a process (`/proc/<pid>/exe`).
    fn exe_of_pid(&mut self, pid: u32) -> Option<String>;
    /// Does `_NET_WM_STATE` contain `_NET_WM_STATE_FULLSCREEN`?
    fn fullscreen(&mut self, win: u32) -> bool;
    /// Starts listening to property changes of `win` (and stops for the one watched before).
    fn watch(&mut self, win: u32);
}

/// An X event reduced to what matters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XEvent {
    /// `_NET_ACTIVE_WINDOW` changed on the root window.
    ActiveWindowChanged,
    /// A window's title changed.
    TitleChanged(u32),
    /// A window's `_NET_WM_STATE` changed.
    StateChanged(u32),
    /// The screen saver started (`true`) or stopped (`false`): the session is locked or idle.
    ScreenSaver(bool),
}

/// What the translator tells the tracker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Out {
    /// The foreground is now this window (or none).
    Focus(Option<RawWindow>),
    Lock,
    Unlock,
}

#[derive(Default)]
pub struct Translator {
    active: Option<u32>,
}

impl Translator {
    pub fn new() -> Self {
        Self::default()
    }

    fn resolve<Q: XQuery>(&self, q: &mut Q, win: u32) -> Option<RawWindow> {
        let pid = q.pid(win)?;
        let exe = q.exe_of_pid(pid)?;
        let title = q.title(win).unwrap_or_default();
        Some(RawWindow { exe, title, fullscreen: q.fullscreen(win) })
    }

    /// The active window as it is now (used at start and by the periodic re-look).
    pub fn current<Q: XQuery>(&mut self, q: &mut Q) -> Option<RawWindow> {
        let w = q.active_window();
        if w != self.active {
            if let Some(w) = w {
                q.watch(w);
            }
            self.active = w;
        }
        w.and_then(|w| self.resolve(q, w))
    }

    pub fn handle<Q: XQuery>(&mut self, q: &mut Q, ev: XEvent) -> Option<Out> {
        match ev {
            XEvent::ActiveWindowChanged => Some(Out::Focus(self.current(q))),
            XEvent::TitleChanged(w) | XEvent::StateChanged(w) => {
                if self.active == Some(w) {
                    Some(Out::Focus(self.resolve(q, w)))
                } else {
                    None
                }
            }
            XEvent::ScreenSaver(on) => Some(if on { Out::Lock } else { Out::Unlock }),
        }
    }
}

/// Is this a Wayland session? Then the active window cannot be seen and tracking says so instead of guessing:
/// XWayland shows only the X11 clients, and the root property is not updated for native Wayland windows.
pub fn is_wayland_session(env: &dyn Fn(&str) -> Option<String>) -> bool {
    let non_empty = |k: &str| env(k).filter(|v| !v.is_empty());
    match non_empty("XDG_SESSION_TYPE").as_deref() {
        Some("wayland") => true,
        Some("x11") => false,
        // Unknown session type: a Wayland display without an X display is Wayland; with both, XWayland is likely.
        _ => non_empty("WAYLAND_DISPLAY").is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Mock {
        active: Option<u32>,
        titles: HashMap<u32, String>,
        pids: HashMap<u32, u32>,
        exes: HashMap<u32, String>,
        full: Vec<u32>,
        watched: Vec<u32>,
    }

    impl XQuery for Mock {
        fn active_window(&mut self) -> Option<u32> {
            self.active
        }
        fn title(&mut self, win: u32) -> Option<String> {
            self.titles.get(&win).cloned()
        }
        fn pid(&mut self, win: u32) -> Option<u32> {
            self.pids.get(&win).copied()
        }
        fn exe_of_pid(&mut self, pid: u32) -> Option<String> {
            self.exes.get(&pid).cloned()
        }
        fn fullscreen(&mut self, win: u32) -> bool {
            self.full.contains(&win)
        }
        fn watch(&mut self, win: u32) {
            self.watched.push(win);
        }
    }

    fn mock() -> Mock {
        let mut m = Mock::default();
        for (win, pid, exe, title) in [(10, 100, "/usr/bin/firefox", "Rust - YouTube — Mozilla Firefox"), (20, 200, "/usr/bin/code", "main.rs - Visual Studio Code")] {
            m.pids.insert(win, pid);
            m.exes.insert(pid, exe.to_string());
            m.titles.insert(win, title.to_string());
        }
        m
    }

    #[test]
    fn active_window_changes_become_focus_events_and_are_watched() {
        let mut m = mock();
        let mut t = Translator::new();
        m.active = Some(10);
        match t.handle(&mut m, XEvent::ActiveWindowChanged) {
            Some(Out::Focus(Some(w))) => {
                assert_eq!(w.exe, "/usr/bin/firefox");
                assert_eq!(w.title, "Rust - YouTube — Mozilla Firefox");
                assert!(!w.fullscreen);
            }
            other => panic!("{other:?}"),
        }
        m.active = Some(20);
        t.handle(&mut m, XEvent::ActiveWindowChanged);
        assert_eq!(m.watched, vec![10, 20]);
        // The same window again does not watch twice.
        t.handle(&mut m, XEvent::ActiveWindowChanged);
        assert_eq!(m.watched, vec![10, 20]);
    }

    #[test]
    fn title_changes_count_only_for_the_active_window() {
        let mut m = mock();
        let mut t = Translator::new();
        m.active = Some(10);
        t.handle(&mut m, XEvent::ActiveWindowChanged);
        m.titles.insert(10, "Other - GitHub — Mozilla Firefox".into());
        match t.handle(&mut m, XEvent::TitleChanged(10)) {
            Some(Out::Focus(Some(w))) => assert!(w.title.contains("GitHub")),
            other => panic!("{other:?}"),
        }
        // A background window changing its title is not our business.
        assert_eq!(t.handle(&mut m, XEvent::TitleChanged(20)), None);
        assert_eq!(t.handle(&mut m, XEvent::StateChanged(20)), None);
    }

    #[test]
    fn fullscreen_is_read_from_the_window_state() {
        let mut m = mock();
        let mut t = Translator::new();
        m.active = Some(20);
        t.handle(&mut m, XEvent::ActiveWindowChanged);
        m.full.push(20);
        match t.handle(&mut m, XEvent::StateChanged(20)) {
            Some(Out::Focus(Some(w))) => assert!(w.fullscreen),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn no_active_window_or_unknown_process_gives_no_window() {
        let mut m = mock();
        let mut t = Translator::new();
        m.active = None;
        assert_eq!(t.handle(&mut m, XEvent::ActiveWindowChanged), Some(Out::Focus(None)));
        // A window without a pid (or whose process is gone) cannot be named: no window, no fake data.
        m.active = Some(30);
        assert_eq!(t.handle(&mut m, XEvent::ActiveWindowChanged), Some(Out::Focus(None)));
        m.pids.insert(30, 300);
        assert_eq!(t.handle(&mut m, XEvent::TitleChanged(30)), Some(Out::Focus(None)));
    }

    #[test]
    fn screen_saver_locks_and_unlocks() {
        let mut m = mock();
        let mut t = Translator::new();
        assert_eq!(t.handle(&mut m, XEvent::ScreenSaver(true)), Some(Out::Lock));
        assert_eq!(t.handle(&mut m, XEvent::ScreenSaver(false)), Some(Out::Unlock));
    }

    #[test]
    fn current_resolves_the_window_at_start() {
        let mut m = mock();
        let mut t = Translator::new();
        m.active = Some(20);
        assert_eq!(t.current(&mut m).map(|w| w.exe), Some("/usr/bin/code".to_string()));
        assert_eq!(m.watched, vec![20]);
    }

    #[test]
    fn wayland_detection() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
        };
        assert!(is_wayland_session(&env(&[("XDG_SESSION_TYPE", "wayland"), ("DISPLAY", ":0")])));
        assert!(!is_wayland_session(&env(&[("XDG_SESSION_TYPE", "x11"), ("DISPLAY", ":0"), ("WAYLAND_DISPLAY", "wayland-0")])));
        assert!(is_wayland_session(&env(&[("WAYLAND_DISPLAY", "wayland-0")])));
        assert!(!is_wayland_session(&env(&[("DISPLAY", ":0")])));
        assert!(!is_wayland_session(&env(&[])));
        assert!(!is_wayland_session(&env(&[("XDG_SESSION_TYPE", ""), ("WAYLAND_DISPLAY", "")])));
    }
}
