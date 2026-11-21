//! The background process: owns the tracker, answers the window process, and keeps the tray up to date.
//!
//! It draws nothing and renders nothing. The window is a separate process that starts when you open Timelyx
//! and exits when you close it, so the resident part stays small.

use crate::config::Config;
use crate::db::{Db, LimitRow, SessionRow};
use crate::engine::{day_start, State, Stop};
use crate::i18n::{self, Lang};
use crate::stats;
use crate::tracker::{RawWindow, Tracker};
use crate::{fmt, strings, tray};
use serde_json::json;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

#[cfg(unix)]
use crate::x11src as source;
#[cfg(windows)]
use crate::winsrc as source;

pub use source::Fg;

/// How often the idle check runs.
pub const TICK_SECS: u64 = 5;
/// How often finished sessions are written.
const FLUSH_SECS: i64 = 60;

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(windows, allow(dead_code))]
pub enum Availability {
    Ok,
    /// A Wayland session: the active window cannot be seen.
    Wayland,
    Failed(String),
}

pub struct Daemon {
    pub tracker: Tracker,
    pub cfg: Config,
    pub lang: Lang,
    dir: PathBuf,
    pub availability: Availability,
    last_flush: i64,
    last_retention: i64,
    /// Test hook: only windows of programs under this folder are looked at (see `TIMELYX_TRACK_ONLY`).
    only_prefix: Option<String>,
    today_total: i64,
}

static DAEMON: OnceLock<Mutex<Daemon>> = OnceLock::new();

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Runs `f` with the daemon locked. `None` if it is not initialised.
pub fn with<R>(f: impl FnOnce(&mut Daemon) -> R) -> Option<R> {
    let m = DAEMON.get()?;
    let mut g = m.lock().ok()?;
    Some(f(&mut g))
}

/// Like [`with`], but gives up instead of waiting. WinEvent callbacks use it: they run inside the message
/// loop and must never block on a lock held further up the same thread.
pub fn try_with<R>(f: impl FnOnce(&mut Daemon) -> R) -> Option<R> {
    let m = DAEMON.get()?;
    let mut g = m.try_lock().ok()?;
    Some(f(&mut g))
}

/// Asks the platform loop to end (from any thread).
pub fn request_quit() {
    source::request_quit();
}

/// Opens the database and creates the daemon. `false` if that failed.
pub fn init(dir: PathBuf, cfg: Config, availability: Availability) -> bool {
    let db = match Db::open(&dir) {
        Ok(d) => d,
        Err(_) => return false,
    };
    let lang = i18n::resolve(cfg.language.as_deref());
    let only_prefix = std::env::var("TIMELYX_TRACK_ONLY").ok().filter(|s| !s.is_empty()).map(|s| s.to_lowercase());
    let n = now();
    let mut d = Daemon {
        tracker: Tracker::new(db, cfg.clone()),
        cfg,
        lang,
        dir,
        availability,
        last_flush: n,
        last_retention: 0,
        only_prefix,
        today_total: 0,
    };
    d.today_total = d.tracker.today_total(n);
    d.apply_retention(n);
    let _ = DAEMON.set(Mutex::new(d));
    true
}

impl Daemon {
    fn pass_filter(&self, w: &RawWindow) -> bool {
        match &self.only_prefix {
            Some(p) => w.exe.to_lowercase().starts_with(p.as_str()),
            None => true,
        }
    }

    /// The foreground window changed (or may have): ask the platform what it is now.
    pub fn on_focus(&mut self) {
        if self.availability != Availability::Ok {
            return;
        }
        match source::foreground() {
            Fg::Ignore => {}
            Fg::None => self.on_focus_raw(None),
            Fg::Window(w) => self.on_focus_raw(Some(w)),
        }
    }

    /// The foreground is this window (already resolved by the source).
    #[cfg_attr(windows, allow(dead_code))]
    pub fn on_focus_raw(&mut self, raw: Option<RawWindow>) {
        if self.availability != Availability::Ok {
            return;
        }
        let raw = raw.filter(|w| self.pass_filter(w));
        self.tracker.focus(now(), raw);
    }

    /// Every few seconds: idle check, a re-look at the foreground (a missed event, a window gone full screen),
    /// and once a minute the write to disk.
    pub fn on_tick(&mut self) {
        if self.availability != Availability::Ok {
            return;
        }
        let n = now();
        let idle = source::idle_secs().unwrap_or(0).max(0);
        self.tracker.tick(n, n - idle);
        self.on_focus();
        if n - self.last_flush >= FLUSH_SECS {
            self.minute(n);
        }
    }

    fn minute(&mut self, n: i64) {
        self.last_flush = n;
        self.tracker.flush(n);
        self.today_total = self.tracker.today_total(n);
        self.check_limits(n);
        if n - self.last_retention > 86_400 {
            self.apply_retention(n);
        }
        self.push_tray();
    }

    fn apply_retention(&mut self, n: i64) {
        self.last_retention = n;
        if self.cfg.retention_months > 0 {
            let cutoff = n - self.cfg.retention_months as i64 * 30 * 86_400;
            let _ = self.tracker.db_mut().apply_retention(cutoff);
        }
    }

    fn check_limits(&mut self, n: i64) {
        if !self.cfg.limit_notifications {
            return;
        }
        let db = self.tracker.db();
        let limits = db.limits().unwrap_or_default();
        if limits.is_empty() {
            return;
        }
        let from = day_start(&chrono::Local, n);
        let rows = db.rows(from, n + 1).unwrap_or_default();
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let notified = |key: &str| db.meta(key).as_deref() == Some(today.as_str());
        let due = limits_due(&limits, &rows, from, n + 1, &notified);
        for l in due {
            let key = format!("limit:{}:{}", l.kind, l.target);
            let _ = self.tracker.db().set_meta(&key, &today);
            let name = self.limit_target_name(&l);
            let t = strings::TABLE.get(self.lang);
            let body = t.limit_notification_body.replace("{name}", &name).replace("{limit}", &fmt::duration(self.lang, l.minutes as i64 * 60));
            crate::notify::toast(t.limit_notification_title, &body);
        }
    }

    fn limit_target_name(&self, l: &LimitRow) -> String {
        let db = self.tracker.db();
        if l.kind == "app" {
            let id: i64 = l.target.parse().unwrap_or(-1);
            return db.apps().unwrap_or_default().into_iter().find(|a| a.id == id).map(|a| a.name).unwrap_or_default();
        }
        if let Some(n) = strings::builtin_name(self.lang, &l.target) {
            return n.to_string();
        }
        db.categories().unwrap_or_default().into_iter().find(|c| c.id == l.target).map(|c| c.name).unwrap_or_default()
    }

    // ───────────── system events ─────────────

    pub fn on_lock(&mut self) {
        self.tracker.stop(now(), Stop::Lock);
        self.push_tray();
    }

    pub fn on_unlock(&mut self) {
        self.tracker.resume(now());
        self.on_focus();
    }

    pub fn on_suspend(&mut self) {
        self.tracker.stop(now(), Stop::Sleep);
    }

    pub fn on_resume(&mut self) {
        // After a wake-up nothing counts until the first input: a timed wake-up with nobody there stays uncounted.
        self.tracker.engine.await_input(now());
    }

    pub fn on_shutdown(&mut self) {
        self.tracker.shutdown(now());
    }

    // ───────────── pause ─────────────

    pub fn pause(&mut self, secs: Option<i64>) {
        let n = now();
        let until = secs.map(|s| n + s).unwrap_or(i64::MAX);
        self.tracker.pause(n, until);
        self.push_tray();
    }

    pub fn unpause(&mut self) {
        self.tracker.unpause(now());
        self.on_focus();
        self.push_tray();
    }

    pub fn is_paused(&self) -> bool {
        matches!(self.tracker.engine.state(), State::Paused { .. })
    }

    // ───────────── window process requests ─────────────

    /// Re-reads the settings from disk after the window changed them.
    pub fn reload(&mut self) {
        let cfg = Config::load_from(&self.dir);
        self.lang = i18n::resolve(cfg.language.as_deref());
        self.tracker.reload(cfg.clone());
        self.cfg = cfg;
        self.push_tray();
    }

    pub fn state_json(&self) -> String {
        let n = now();
        let engine = &self.tracker.engine;
        let (state, until) = match engine.state() {
            State::Running => ("running", 0),
            State::Idle { .. } => ("idle", 0),
            State::Stopped(_) => ("locked", 0),
            State::Paused { until } => ("paused", until),
        };
        let live = match (engine.open_session(), &self.tracker.live) {
            (Some((start, _)), Some(l)) => json!({"app": l.app, "site": l.site, "category": l.category, "start": start}),
            _ => serde_json::Value::Null,
        };
        let avail = match &self.availability {
            Availability::Ok => "ok".to_string(),
            Availability::Wayland => "wayland".to_string(),
            Availability::Failed(e) => format!("error:{e}"),
        };
        json!({"avail": avail, "state": state, "paused_until": until, "live": live, "now": n}).to_string()
    }

    /// Answers one request line from the window process.
    pub fn handle(&mut self, msg: &str) -> String {
        let mut parts = msg.split_whitespace();
        match parts.next().unwrap_or("") {
            "ping" => "ok".into(),
            "state" => self.state_json(),
            "flush" => {
                let n = now();
                self.last_flush = n;
                self.tracker.flush(n);
                self.today_total = self.tracker.today_total(n);
                "ok".into()
            }
            "reload" => {
                self.reload();
                "ok".into()
            }
            "pause" => {
                match parts.next() {
                    Some("inf") | None => self.pause(None),
                    Some(s) => self.pause(s.parse::<i64>().ok().filter(|v| *v > 0)),
                }
                "ok".into()
            }
            "resume" => {
                self.unpause();
                "ok".into()
            }
            "open" => {
                crate::platform::spawn_ui();
                "ok".into()
            }
            "quit" => {
                source::request_quit();
                "ok".into()
            }
            _ => "error".into(),
        }
    }

    // ───────────── tray ─────────────

    pub fn tray_state(&self) -> tray::TrayState {
        let t = strings::TABLE.get(self.lang);
        let mut tip = format!("Timelyx · {}", t.tray_today.replace("{time}", &fmt::duration(self.lang, self.today_total)));
        match (&self.availability, self.tracker.engine.state()) {
            (Availability::Ok, State::Paused { .. }) => tip.push_str(&format!(" · {}", t.tray_paused)),
            (Availability::Ok, _) => {}
            _ => tip.push_str(&format!(" · {}", t.tray_unavailable)),
        }
        tray::TrayState { lang: self.lang, tooltip: tip, paused: self.is_paused() || self.availability != Availability::Ok }
    }

    pub fn push_tray(&self) {
        tray::update(self.tray_state());
    }
}

/// Which limits were reached today and have not been announced yet.
pub fn limits_due(limits: &[LimitRow], rows: &[SessionRow], from: i64, to: i64, notified: &dyn Fn(&str) -> bool) -> Vec<LimitRow> {
    limits
        .iter()
        .filter(|l| {
            let used = match l.kind.as_str() {
                "category" => stats::sum_where(rows, from, to, |r| r.category == l.target),
                "app" => {
                    let id: i64 = l.target.parse().unwrap_or(-1);
                    stats::sum_where(rows, from, to, |r| r.app_id == id)
                }
                _ => 0,
            };
            l.minutes > 0 && used >= l.minutes as i64 * 60 && !notified(&format!("limit:{}:{}", l.kind, l.target))
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(start: i64, end: i64, app: i64, cat: &str) -> SessionRow {
        SessionRow { start, end, app_id: app, site_id: None, category: cat.into() }
    }

    fn limit(kind: &str, target: &str, minutes: u32) -> LimitRow {
        LimitRow { kind: kind.into(), target: target.into(), minutes }
    }

    #[test]
    fn limits_fire_once_when_reached() {
        let rows = vec![row(0, 3600, 1, "games"), row(3600, 4000, 2, "code")];
        let limits = vec![limit("category", "games", 60), limit("category", "code", 60), limit("app", "2", 5), limit("app", "9", 1)];
        let due = limits_due(&limits, &rows, 0, 10_000, &|_| false);
        let keys: Vec<String> = due.iter().map(|l| format!("{}:{}", l.kind, l.target)).collect();
        assert_eq!(keys, vec!["category:games".to_string(), "app:2".to_string()]);
        // Already announced today: silent.
        let due = limits_due(&limits, &rows, 0, 10_000, &|k| k == "limit:category:games");
        assert_eq!(due.len(), 1);
        // A zero limit never fires.
        assert!(limits_due(&[limit("category", "games", 0)], &rows, 0, 10_000, &|_| false).is_empty());
    }

    #[test]
    fn only_todays_part_counts() {
        let rows = vec![row(0, 7200, 1, "games")];
        assert!(limits_due(&[limit("category", "games", 90)], &rows, 3600, 7200, &|_| false).is_empty());
        assert_eq!(limits_due(&[limit("category", "games", 60)], &rows, 3600, 7200, &|_| false).len(), 1);
    }
}
