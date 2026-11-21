//! The tracker core: resolves raw foreground windows into targets (program, site, category), runs the
//! session engine and writes finished sessions to the database. It does not touch the operating system;
//! `winsrc.rs` and `x11src.rs` feed it, and `daemon.rs` owns it.

use crate::cats;
use crate::config::Config;
use crate::db::{AppRow, Db, SiteRow};
use crate::engine::{self, Engine, Session, Stop, Target, Window};
use crate::rules;
use crate::sites;
use std::collections::HashMap;

/// The foreground window as the operating system reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawWindow {
    pub exe: String,
    pub title: String,
    pub fullscreen: bool,
}

/// Programs that are full screen without being a game (remote desktops, virtual machines).
const NEVER_GAMES: &[&str] = &["mstsc", "msrdc", "vmconnect", "anydesk", "teamviewer", "parsec", "vmware", "vmware-vmx", "virtualboxvm", "wfica32", "xfreerdp", "remmina", "vncviewer", "rustdesk"];

pub fn never_game(exe: &str) -> bool {
    NEVER_GAMES.contains(&rules::stem(exe).as_str())
}

/// What decides whether a new event changes anything: the program, the title for browsers, the editor flag
/// for terminals and the fullscreen state. Other title changes (a terminal progress bar) are ignored.
fn change_key(w: &Option<RawWindow>) -> Option<(String, String, bool)> {
    let w = w.as_ref()?;
    let detail = if rules::is_browser(&w.exe) {
        w.title.clone()
    } else if rules::is_terminal(&w.exe) {
        rules::title_looks_like_vim(&w.title).to_string()
    } else {
        String::new()
    };
    Some((w.exe.clone(), detail, w.fullscreen))
}

pub struct Tracker {
    pub engine: Engine,
    db: Db,
    cfg: Config,
    apps: HashMap<String, AppRow>,
    sites: HashMap<String, SiteRow>,
    last_key: Option<Option<(String, String, bool)>>,
    /// The most recent resolved window with its display data, for the state the window process asks for.
    pub live: Option<Live>,
}

/// What the foreground activity is, in terms the interface can show.
#[derive(Clone, Debug, PartialEq)]
pub struct Live {
    pub app: String,
    pub site: Option<String>,
    pub category: String,
}

impl Tracker {
    pub fn new(db: Db, cfg: Config) -> Self {
        let engine = Engine::new(cfg.idle_secs());
        Self { engine, db, cfg, apps: HashMap::new(), sites: HashMap::new(), last_key: None, live: None }
    }

    pub fn db(&self) -> &Db {
        &self.db
    }

    pub fn db_mut(&mut self) -> &mut Db {
        &mut self.db
    }

    /// New settings (after the window changed them) and fresh category data.
    pub fn reload(&mut self, cfg: Config) {
        // The window may have deleted rows: forget what was cached about them.
        self.db.forget_last();
        self.engine.set_idle_secs(cfg.idle_secs());
        self.cfg = cfg;
        self.apps.clear();
        self.sites.clear();
        self.last_key = None;
    }

    fn app_row(&mut self, key: &str, exe: &str, vim: bool) -> Option<AppRow> {
        if let Some(a) = self.apps.get(key) {
            return Some(a.clone());
        }
        let (name, cat) = if vim { (format!("{} (Vim)", rules::display_name(exe)), cats::CODE) } else { (rules::display_name(exe), rules::classify(exe)) };
        let row = self.db.app_for(key, &name, cat).ok()?;
        self.apps.insert(key.to_string(), row.clone());
        Some(row)
    }

    fn site_row(&mut self, name: &str) -> Option<SiteRow> {
        if let Some(s) = self.sites.get(name) {
            return Some(s.clone());
        }
        let default = if sites::is_special(name) { None } else { sites::default_category(name) };
        let row = self.db.site_for(name, default).ok()?;
        self.sites.insert(name.to_string(), row.clone());
        Some(row)
    }

    /// Turns a raw window into an engine window (and creates the database rows it needs).
    fn resolve(&mut self, raw: &RawWindow) -> Option<Window> {
        if raw.exe.is_empty() {
            return None;
        }
        let browser = rules::is_browser(&raw.exe);
        let vim = !browser && rules::is_terminal(&raw.exe) && rules::title_looks_like_vim(&raw.title);
        let key = if vim { format!("{}#vim", raw.exe) } else { raw.exe.clone() };
        let mut app = self.app_row(&key, &raw.exe, vim)?;
        // A program that is full screen and nothing else we know of is a game; remember it.
        if raw.fullscreen && app.category == cats::OTHER && !app.manual && !never_game(&raw.exe) {
            let _ = self.db.set_app_auto_category(app.id, cats::GAMES);
            app.category = cats::GAMES.to_string();
            self.apps.insert(key.clone(), app.clone());
        }
        let mut site = None;
        let mut category = app.category.clone();
        if browser && self.cfg.track_sites && !raw.title.trim().is_empty() {
            let name = sites::parse(&raw.title);
            if !sites::is_excluded(&name, &self.cfg.site_exclusions) {
                if let Some(row) = self.site_row(&name) {
                    if let Some(c) = &row.category {
                        category = c.clone();
                    }
                    site = Some(name);
                }
            }
        }
        self.live = Some(Live { app: app.name.clone(), site: site.clone(), category: category.clone() });
        Some(Window {
            target: Target { exe: key, site },
            fullscreen: raw.fullscreen,
            keep_when_idle: raw.fullscreen || cats::keeps_counting_when_idle(&category),
        })
    }

    pub fn focus(&mut self, now: i64, raw: Option<RawWindow>) {
        let key = change_key(&raw);
        if self.last_key.as_ref() == Some(&key) {
            return;
        }
        self.last_key = Some(key);
        let window = raw.as_ref().and_then(|r| self.resolve(r));
        if window.is_none() {
            self.live = None;
        }
        self.engine.focus(now, window);
    }

    pub fn tick(&mut self, now: i64, last_input: i64) {
        self.engine.tick(now, last_input);
    }

    pub fn stop(&mut self, now: i64, why: Stop) {
        self.engine.stop(now, why);
        self.write(now);
    }

    pub fn resume(&mut self, now: i64) {
        self.engine.resume(now);
    }

    pub fn pause(&mut self, now: i64, until: i64) {
        self.engine.pause(now, until);
        self.write(now);
    }

    pub fn unpause(&mut self, now: i64) {
        self.engine.unpause(now);
    }

    /// Writes finished sessions (and a slice of the open one) to the database.
    pub fn flush(&mut self, now: i64) -> bool {
        let list = self.engine.flush(now);
        self.store(list)
    }

    /// Final flush at exit.
    pub fn shutdown(&mut self, now: i64) -> bool {
        let list = self.engine.finish(now);
        self.store(list)
    }

    fn write(&mut self, now: i64) {
        // After a stop or pause the engine already closed its session; hand out what is finished.
        let list = self.engine.flush(now);
        self.store(list);
    }

    fn store(&mut self, list: Vec<Session>) -> bool {
        if list.is_empty() {
            return true;
        }
        let mut rows: Vec<(i64, i64, i64, Option<i64>)> = Vec::with_capacity(list.len());
        for s in list {
            // The cache may have been cleared by a reload while this session was open: look the program up again.
            let (path, vim) = match s.target.exe.strip_suffix("#vim") {
                Some(p) => (p.to_string(), true),
                None => (s.target.exe.clone(), false),
            };
            let Some(app) = self.app_row(&s.target.exe, &path, vim).map(|a| a.id) else { continue };
            let site = match &s.target.site {
                Some(name) => match self.site_row(name) {
                    Some(r) => Some(r.id),
                    None => continue,
                },
                None => None,
            };
            // Never let one session span two local days.
            for (a, b) in engine::split_days(&chrono::Local, s.start, s.end) {
                rows.push((a, b, app, site));
            }
        }
        self.db.append_sessions(&rows).is_ok()
    }

    /// Total seconds recorded today (already written to the database) plus the open slice.
    pub fn today_total(&self, now: i64) -> i64 {
        let from = engine::day_start(&chrono::Local, now);
        let done = self.db.rows(from, now + 1).map(|r| crate::stats::total(&r, from, now + 1)).unwrap_or(0);
        let open = self.engine.open_session().map(|(s, _)| (now - s.max(from)).max(0)).unwrap_or(0);
        done + open
    }

    #[cfg(test)]
    pub fn local_day_start(now: i64) -> i64 {
        engine::day_start(&chrono::Local, now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracker(cfg: Config) -> Tracker {
        Tracker::new(Db::open_memory().unwrap(), cfg)
    }

    fn raw(exe: &str, title: &str) -> Option<RawWindow> {
        Some(RawWindow { exe: exe.into(), title: title.into(), fullscreen: false })
    }

    const CHROME: &str = r"C:\Program Files\Google\Chrome\Application\chrome.exe";
    const CODE: &str = r"C:\Users\a\AppData\Local\Programs\Microsoft VS Code\Code.exe";

    #[test]
    fn browser_tabs_become_sites_with_default_categories() {
        let mut t = tracker(Config::default());
        t.focus(0, raw(CHROME, "Rust talk - YouTube - Google Chrome"));
        t.focus(100, raw(CHROME, "owner/repo · GitHub - Google Chrome"));
        t.focus(250, raw(CODE, "main.rs - project - Visual Studio Code"));
        assert!(t.shutdown(400));
        let rows = t.db().rows(0, 1000).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].category, cats::VIDEO);
        assert_eq!(rows[1].category, cats::CODE);
        assert_eq!(rows[2].category, cats::CODE);
        assert!(rows[0].site_id.is_some() && rows[2].site_id.is_none());
        let sites = t.db().sites().unwrap();
        assert!(sites.iter().any(|s| s.name == "YouTube") && sites.iter().any(|s| s.name == "GitHub"));
        // The window title of the code editor is never stored anywhere.
        assert!(sites.iter().all(|s| !s.name.contains("main.rs")));
        assert!(t.db().apps().unwrap().iter().all(|a| !a.name.contains("main.rs")));
    }

    #[test]
    fn tracking_sites_off_records_the_browser_only() {
        let mut cfg = Config::default();
        cfg.track_sites = false;
        let mut t = tracker(cfg);
        t.focus(0, raw(CHROME, "Video - YouTube - Google Chrome"));
        t.shutdown(100);
        assert!(t.db().sites().unwrap().is_empty());
        assert_eq!(t.db().rows(0, 1000).unwrap()[0].category, cats::BROWSER);
    }

    #[test]
    fn excluded_sites_are_not_recorded() {
        let mut cfg = Config::default();
        cfg.site_exclusions = vec!["youtube".into()];
        let mut t = tracker(cfg);
        t.focus(0, raw(CHROME, "Video - YouTube - Google Chrome"));
        t.focus(50, raw(CHROME, "Issue · GitHub - Google Chrome"));
        t.shutdown(100);
        let names: Vec<String> = t.db().sites().unwrap().into_iter().map(|s| s.name).collect();
        assert_eq!(names, vec!["GitHub".to_string()]);
        let rows = t.db().rows(0, 1000).unwrap();
        assert_eq!(rows[0].category, cats::BROWSER);
        assert!(rows[0].site_id.is_none());
    }

    #[test]
    fn private_windows_record_no_site_but_the_marker() {
        let mut t = tracker(Config::default());
        t.focus(0, raw(CHROME, "Secret plans - Google Chrome (Incognito)"));
        t.shutdown(60);
        let sites = t.db().sites().unwrap();
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].name, sites::PRIVATE);
        assert!(sites.iter().all(|s| !s.name.to_lowercase().contains("secret")));
    }

    #[test]
    fn title_changes_inside_one_site_do_not_split() {
        let mut t = tracker(Config::default());
        t.focus(0, raw(CHROME, "Video one - YouTube - Google Chrome"));
        t.focus(60, raw(CHROME, "Video two - YouTube - Google Chrome"));
        t.shutdown(120);
        assert_eq!(t.db().session_count(), 1);
        assert_eq!(t.db().rows(0, 1000).unwrap()[0].end, 120);
    }

    #[test]
    fn a_fullscreen_unknown_program_becomes_a_game_and_stays_one() {
        let mut t = tracker(Config::default());
        let exe = r"C:\Tools\mystery.exe";
        t.focus(0, Some(RawWindow { exe: exe.into(), title: "x".into(), fullscreen: false }));
        t.focus(10, Some(RawWindow { exe: exe.into(), title: "x".into(), fullscreen: true }));
        t.focus(70, raw(CODE, "a"));
        t.shutdown(100);
        let app = t.db().apps().unwrap().into_iter().find(|a| a.exe == exe).unwrap();
        assert_eq!(app.category, cats::GAMES);
        assert!(!app.manual);
        // Remembered: a windowed session afterwards is still a game.
        t.focus(100, Some(RawWindow { exe: exe.into(), title: "x".into(), fullscreen: false }));
        t.shutdown(120);
        let rows = t.db().rows(0, 1000).unwrap();
        assert_eq!(rows.last().unwrap().category, cats::GAMES);
    }

    #[test]
    fn a_remote_desktop_in_fullscreen_is_not_a_game() {
        let mut t = tracker(Config::default());
        t.focus(0, Some(RawWindow { exe: r"C:\Windows\System32\mstsc.exe".into(), title: "x".into(), fullscreen: true }));
        t.shutdown(50);
        assert_eq!(t.db().apps().unwrap()[0].category, cats::OTHER);
    }

    #[test]
    fn manual_category_is_never_overridden_by_fullscreen() {
        let mut t = tracker(Config::default());
        let exe = r"C:\Tools\mystery.exe";
        t.focus(0, Some(RawWindow { exe: exe.into(), title: "x".into(), fullscreen: false }));
        let id = t.db().apps().unwrap()[0].id;
        t.db().set_app_category(id, cats::OFFICE).unwrap();
        t.reload(Config::default());
        t.focus(10, Some(RawWindow { exe: exe.into(), title: "x".into(), fullscreen: true }));
        t.shutdown(20);
        assert_eq!(t.db().apps().unwrap()[0].category, cats::OFFICE);
    }

    #[test]
    fn terminal_with_vim_is_counted_as_code() {
        let mut t = tracker(Config::default());
        let wt = r"C:\Program Files\WindowsApps\Terminal\WindowsTerminal.exe";
        t.focus(0, raw(wt, "cargo build"));
        t.focus(100, raw(wt, "nvim src/main.rs"));
        t.focus(200, raw(wt, "cargo test"));
        t.shutdown(300);
        let rows = t.db().rows(0, 1000).unwrap();
        assert_eq!(rows.iter().map(|r| r.category.as_str()).collect::<Vec<_>>(), vec![cats::SYSTEM, cats::CODE, cats::SYSTEM]);
        let apps = t.db().apps().unwrap();
        assert!(apps.iter().any(|a| a.name.ends_with("(Vim)")));
        assert!(apps.iter().all(|a| !a.name.contains("main.rs")));
    }

    #[test]
    fn terminal_progress_titles_do_not_cause_work() {
        let mut t = tracker(Config::default());
        let wt = r"C:\Program Files\WindowsApps\Terminal\WindowsTerminal.exe";
        t.focus(0, raw(wt, "build 1%"));
        t.focus(1, raw(wt, "build 2%"));
        t.focus(2, raw(wt, "build 3%"));
        t.shutdown(10);
        assert_eq!(t.db().session_count(), 1);
    }

    #[test]
    fn a_video_category_keeps_counting_through_idle_time() {
        let mut t = tracker(Config::default());
        t.focus(0, raw(r"C:\Program Files\VideoLAN\VLC\vlc.exe", "movie.mkv - VLC media player"));
        for now in (5..=3600).step_by(5) {
            t.tick(now, 0);
        }
        t.shutdown(3600);
        assert_eq!(t.db().rows(0, 10_000).unwrap()[0].end, 3600);
    }

    #[test]
    fn idle_time_is_not_recorded_for_a_normal_program() {
        let mut t = tracker(Config::default());
        t.focus(0, raw(CODE, "x"));
        for now in (5..=3600).step_by(5) {
            t.tick(now, 100.min(now));
        }
        t.shutdown(3600);
        let rows = t.db().rows(0, 10_000).unwrap();
        assert_eq!((rows[0].start, rows[0].end), (0, 100));
    }

    #[test]
    fn flush_writes_in_slices_that_merge_into_one_row() {
        let mut t = tracker(Config::default());
        t.focus(0, raw(CODE, "x"));
        t.flush(60);
        t.flush(120);
        t.shutdown(150);
        assert_eq!(t.db().session_count(), 1);
        assert_eq!(t.db().rows(0, 1000).unwrap()[0].end, 150);
    }

    #[test]
    fn lock_and_pause_write_immediately() {
        let mut t = tracker(Config::default());
        t.focus(0, raw(CODE, "x"));
        t.stop(100, Stop::Lock);
        assert_eq!(t.db().rows(0, 1000).unwrap()[0].end, 100);
        t.resume(500);
        t.pause(560, i64::MAX);
        assert_eq!(t.db().session_count(), 2);
    }

    #[test]
    fn today_total_includes_the_open_slice() {
        let mut t = tracker(Config::default());
        let now = 1_795_000_000;
        t.focus(now - 600, raw(CODE, "x"));
        assert_eq!(t.today_total(now), 600.min(now - Tracker::local_day_start(now)));
    }

    #[test]
    fn sessions_across_midnight_are_split() {
        let mut t = tracker(Config::default());
        let day = Tracker::local_day_start(1_795_000_000);
        t.focus(day - 600, raw(CODE, "x"));
        t.shutdown(day + 600);
        assert_eq!(t.db().session_count(), 2);
        let rows = t.db().rows(day - 1000, day + 1000).unwrap();
        assert_eq!((rows[0].start, rows[0].end, rows[1].start, rows[1].end), (day - 600, day, day, day + 600));
    }
}
