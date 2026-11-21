//! Windows notification-area icon: Open, Pause tracking, Support the author, Exit.
//!
//! The icon lives on the tracker's main thread (the one with the message loop). Other threads change it through
//! `update`, which stores the new state and posts a message; the loop then calls `apply_pending`.

use crate::strings;
use crate::support;
use crate::tray_common::{paused_rgba, perform, Action};
pub use crate::tray_common::TrayState;
use std::cell::RefCell;
use std::sync::Mutex;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

struct Tray {
    icon: TrayIcon,
    normal: Icon,
    paused: Icon,
    open: MenuItem,
    pause: Submenu,
    p15: MenuItem,
    p60: MenuItem,
    pinf: MenuItem,
    resume: MenuItem,
    support: MenuItem,
    exit: MenuItem,
    last: Option<TrayState>,
}

thread_local! {
    static TRAY: RefCell<Option<Tray>> = const { RefCell::new(None) };
}

static PENDING: Mutex<Option<TrayState>> = Mutex::new(None);

/// Creates the icon. Call on the message-loop thread.
pub fn create(state: &TrayState, icon_png: &[u8]) -> bool {
    let Ok(data) = eframe::icon_data::from_png_bytes(icon_png) else { return false };
    let Ok(normal) = Icon::from_rgba(data.rgba.clone(), data.width, data.height) else { return false };
    let Ok(paused) = Icon::from_rgba(paused_rgba(&data.rgba, data.width, data.height), data.width, data.height) else { return false };
    let t = strings::TABLE.get(state.lang);
    let open = MenuItem::new(t.tray_open, true, None);
    let p15 = MenuItem::new(t.tray_pause_15, true, None);
    let p60 = MenuItem::new(t.tray_pause_60, true, None);
    let pinf = MenuItem::new(t.tray_pause_until, true, None);
    let pause = Submenu::with_items(t.tray_pause, true, &[&p15, &p60, &pinf]).unwrap_or_else(|_| Submenu::new(t.tray_pause, true));
    let resume = MenuItem::new(t.tray_resume, state.paused, None);
    let support_item = MenuItem::new(support::tray_label(state.lang), true, None);
    let exit = MenuItem::new(t.tray_exit, true, None);
    let menu = Menu::new();
    if menu.append_items(&[&open, &PredefinedMenuItem::separator(), &pause, &resume, &PredefinedMenuItem::separator(), &support_item, &exit]).is_err() {
        return false;
    }
    let Ok(icon) = TrayIconBuilder::new()
        .with_icon(if state.paused { paused.clone() } else { normal.clone() })
        .with_tooltip(&state.tooltip)
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
    else {
        return false;
    };

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
    TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
            perform(Action::Open);
        }
    }));
    TRAY.with(|t| {
        *t.borrow_mut() = Some(Tray { icon, normal, paused, open, pause, p15, p60, pinf, resume, support: support_item, exit, last: Some(state.clone()) });
    });
    true
}

/// Stores a new state and asks the loop to draw it. Safe to call from any thread.
pub fn update(state: TrayState) {
    if let Ok(mut p) = PENDING.lock() {
        *p = Some(state);
    }
    crate::winsrc::request_tray_refresh();
}

/// Draws the state stored by `update`. Call on the message-loop thread.
pub fn apply_pending() {
    let Some(state) = PENDING.lock().ok().and_then(|mut p| p.take()) else { return };
    TRAY.with(|t| {
        let mut b = t.borrow_mut();
        let Some(tr) = b.as_mut() else { return };
        if tr.last.as_ref() == Some(&state) {
            return;
        }
        let t = strings::TABLE.get(state.lang);
        let lang_changed = tr.last.as_ref().is_none_or(|l| l.lang != state.lang);
        if lang_changed {
            tr.open.set_text(t.tray_open);
            tr.pause.set_text(t.tray_pause);
            tr.p15.set_text(t.tray_pause_15);
            tr.p60.set_text(t.tray_pause_60);
            tr.pinf.set_text(t.tray_pause_until);
            tr.resume.set_text(t.tray_resume);
            tr.support.set_text(support::tray_label(state.lang));
            tr.exit.set_text(t.tray_exit);
        }
        tr.resume.set_enabled(state.paused);
        let _ = tr.icon.set_tooltip(Some(&state.tooltip));
        let _ = tr.icon.set_icon(Some(if state.paused { tr.paused.clone() } else { tr.normal.clone() }));
        tr.last = Some(state);
    });
}

pub fn remove() {
    TRAY.with(|t| {
        if let Some(tr) = t.borrow().as_ref() {
            let _ = tr.icon.set_visible(false);
        }
        *t.borrow_mut() = None;
    });
}
