//! Notification-area icon (StatusNotifier / AppIndicator): Open, Pause tracking, Support the author, Exit.
//!
//! `tray-icon` needs a GTK main loop, so the icon lives on its own GTK thread and the tracker talks to it through
//! a channel. The AppIndicator protocol has no click events: the icon only opens its menu. GNOME needs the
//! "AppIndicator and KStatusNotifierItem Support" extension; without a tray host the tracker still runs, only
//! without an icon.

use crate::strings;
use crate::support;
use crate::tray_common::{paused_rgba, perform, Action};
pub use crate::tray_common::TrayState;
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Sender};
use std::sync::Mutex;
use std::time::Duration;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIconBuilder};

enum Msg {
    State(TrayState),
    Remove,
}

static TX: Mutex<Option<Sender<Msg>>> = Mutex::new(None);

/// Is a StatusNotifier host running? Without one the icon would be invisible.
fn tray_host_present() -> bool {
    let out = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.freedesktop.DBus",
            "--object-path",
            "/org/freedesktop/DBus",
            "--method",
            "org.freedesktop.DBus.NameHasOwner",
            "org.kde.StatusNotifierWatcher",
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).contains("true"),
        // gdbus is missing or the bus is unreachable: assume the best.
        _ => true,
    }
}

/// Creates the icon on its own GTK thread. `false` if there is no tray to show it in.
pub fn create(state: &TrayState, icon_png: &[u8]) -> bool {
    let (tx, rx) = channel::<Msg>();
    let (ready_tx, ready_rx) = channel::<bool>();
    let png = icon_png.to_vec();
    let state = state.clone();
    if std::thread::Builder::new().name("timelyx-tray".into()).spawn(move || run(state, png, rx, ready_tx)).is_err() {
        return false;
    }
    match ready_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(true) => {
            if let Ok(mut g) = TX.lock() {
                *g = Some(tx);
            }
            true
        }
        _ => false,
    }
}

/// Hands a new state to the tray (any thread).
pub fn update(state: TrayState) {
    if let Ok(g) = TX.lock() {
        if let Some(tx) = g.as_ref() {
            let _ = tx.send(Msg::State(state));
        }
    }
}

pub fn remove() {
    if let Ok(g) = TX.lock() {
        if let Some(tx) = g.as_ref() {
            let _ = tx.send(Msg::Remove);
        }
    }
}

fn run(state: TrayState, icon_png: Vec<u8>, rx: std::sync::mpsc::Receiver<Msg>, ready: Sender<bool>) {
    let built = (|| {
        gtk::init().ok()?;
        let data = eframe::icon_data::from_png_bytes(&icon_png).ok()?;
        let normal = Icon::from_rgba(data.rgba.clone(), data.width, data.height).ok()?;
        let paused = Icon::from_rgba(paused_rgba(&data.rgba, data.width, data.height), data.width, data.height).ok()?;
        let t = strings::TABLE.get(state.lang);
        let open = MenuItem::new(t.tray_open, true, None);
        let p15 = MenuItem::new(t.tray_pause_15, true, None);
        let p60 = MenuItem::new(t.tray_pause_60, true, None);
        let pinf = MenuItem::new(t.tray_pause_until, true, None);
        let pause = Submenu::with_items(t.tray_pause, true, &[&p15, &p60, &pinf]).ok()?;
        let resume = MenuItem::new(t.tray_resume, state.paused, None);
        let support_item = MenuItem::new(support::tray_label(state.lang), true, None);
        let exit = MenuItem::new(t.tray_exit, true, None);
        let menu = Menu::new();
        menu.append_items(&[&open, &PredefinedMenuItem::separator(), &pause, &resume, &PredefinedMenuItem::separator(), &support_item, &exit]).ok()?;
        let tray = TrayIconBuilder::new()
            .with_icon(if state.paused { paused.clone() } else { normal.clone() })
            .with_tooltip(&state.tooltip)
            .with_menu(Box::new(menu))
            .build()
            .ok()?;
        Some((tray, normal, paused, open, pause, p15, p60, pinf, resume, support_item, exit))
    })();
    let Some((tray, normal, paused, open, pause, p15, p60, pinf, resume, support_item, exit)) = built else {
        let _ = ready.send(false);
        return;
    };
    if !tray_host_present() {
        let _ = ready.send(false);
        return;
    }

    let ids = (open.id().clone(), p15.id().clone(), p60.id().clone(), pinf.id().clone(), resume.id().clone(), support_item.id().clone(), exit.id().clone());
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        let a = if e.id == ids.0 {
            Action::Open
        } else if e.id == ids.1 {
            Action::Pause(Some(15 * 60))
        } else if e.id == ids.2 {
            Action::Pause(Some(60 * 60))
        } else if e.id == ids.3 {
            Action::Pause(None)
        } else if e.id == ids.4 {
            Action::Resume
        } else if e.id == ids.5 {
            Action::Support
        } else if e.id == ids.6 {
            Action::Exit
        } else {
            return;
        };
        perform(a);
    }));
    let _ = ready.send(true);

    let mut last = Some(state);
    gtk::glib::timeout_add_local(Duration::from_millis(500), move || {
        while let Ok(msg) = rx.try_recv() {
            match msg {
                Msg::State(s) => {
                    if last.as_ref() == Some(&s) {
                        continue;
                    }
                    let t = strings::TABLE.get(s.lang);
                    if last.as_ref().is_none_or(|l| l.lang != s.lang) {
                        open.set_text(t.tray_open);
                        pause.set_text(t.tray_pause);
                        p15.set_text(t.tray_pause_15);
                        p60.set_text(t.tray_pause_60);
                        pinf.set_text(t.tray_pause_until);
                        resume.set_text(t.tray_resume);
                        support_item.set_text(support::tray_label(s.lang));
                        exit.set_text(t.tray_exit);
                    }
                    resume.set_enabled(s.paused);
                    let _ = tray.set_tooltip(Some(&s.tooltip));
                    let _ = tray.set_icon(Some(if s.paused { paused.clone() } else { normal.clone() }));
                    last = Some(s);
                }
                Msg::Remove => {
                    let _ = tray.set_visible(false);
                    gtk::main_quit();
                    return gtk::glib::ControlFlow::Break;
                }
            }
        }
        gtk::glib::ControlFlow::Continue
    });
    gtk::main();
}
