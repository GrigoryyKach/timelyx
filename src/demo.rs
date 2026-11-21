//! Demo data: a made-up, realistic two weeks of activity for screenshots and manual trials.
//!
//! Used with `--demo` or `TIMELYX_DEMO=1`, and only when `TIMELYX_DATA_DIR` points at a throw-away folder.
//! Nothing here reads real activity.

use crate::config::Config;
use crate::db::Db;
use crate::engine::day_start;
use crate::{cats, rules, sites};
use std::path::Path;

/// A small deterministic generator, so the demo looks the same every time.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// A number in `lo..=hi`.
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.next() % (hi - lo + 1) as u64) as i64
    }

    fn pick<'a, T>(&mut self, items: &'a [(T, u32)]) -> &'a T {
        let total: u32 = items.iter().map(|i| i.1).sum();
        let mut r = (self.next() % total as u64) as u32;
        for (v, w) in items {
            if r < *w {
                return v;
            }
            r -= *w;
        }
        &items[0].0
    }
}

#[derive(Clone, Copy)]
enum Act {
    App(&'static str),
    Site(&'static str),
}

const CHROME: &str = r"C:\Program Files\Google\Chrome\Application\chrome.exe";
const CODE: &str = r"C:\Users\demo\AppData\Local\Programs\Microsoft VS Code\Code.exe";
const TERMINAL: &str = r"C:\Program Files\WindowsApps\Microsoft.WindowsTerminal\WindowsTerminal.exe";
const TELEGRAM: &str = r"C:\Users\demo\AppData\Roaming\Telegram Desktop\Telegram.exe";
const SLACK: &str = r"C:\Users\demo\AppData\Local\slack\slack.exe";
const DISCORD: &str = r"C:\Users\demo\AppData\Local\Discord\app-1.0.9\Discord.exe";
const WORD: &str = r"C:\Program Files\Microsoft Office\root\Office16\WINWORD.EXE";
const EXCEL: &str = r"C:\Program Files\Microsoft Office\root\Office16\EXCEL.EXE";
const OBSIDIAN: &str = r"C:\Users\demo\AppData\Local\Obsidian\Obsidian.exe";
const EXPLORER: &str = r"C:\Windows\explorer.exe";
const FIGMA: &str = r"C:\Users\demo\AppData\Local\Figma\Figma.exe";
const PHOTOSHOP: &str = r"C:\Program Files\Adobe\Adobe Photoshop 2026\Photoshop.exe";
const VLC: &str = r"C:\Program Files\VideoLAN\VLC\vlc.exe";
const SPOTIFY: &str = r"C:\Users\demo\AppData\Roaming\Spotify\Spotify.exe";
const HADES: &str = r"C:\Program Files (x86)\Steam\steamapps\common\Hades\Hades.exe";
const CELESTE: &str = r"C:\Program Files (x86)\Steam\steamapps\common\Celeste\Celeste.exe";
const STEAM: &str = r"C:\Program Files (x86)\Steam\steam.exe";

const WORK: &[(Act, u32)] = &[
    (Act::App(CODE), 42),
    (Act::Site("GitHub"), 9),
    (Act::Site("Stack Overflow"), 6),
    (Act::Site("MDN"), 4),
    (Act::App(TERMINAL), 8),
    (Act::App(SLACK), 7),
    (Act::App(TELEGRAM), 5),
    (Act::Site("Gmail"), 5),
    (Act::App(WORD), 4),
    (Act::App(EXCEL), 3),
    (Act::App(OBSIDIAN), 5),
    (Act::App(FIGMA), 3),
    (Act::App(EXPLORER), 2),
    (Act::Site("YouTube"), 2),
];

const EVENING: &[(Act, u32)] = &[
    (Act::Site("YouTube"), 30),
    (Act::Site("Twitch"), 15),
    (Act::App(TELEGRAM), 12),
    (Act::App(DISCORD), 12),
    (Act::App(SPOTIFY), 8),
    (Act::App(VLC), 8),
    (Act::Site("Reddit"), 8),
    (Act::App(PHOTOSHOP), 4),
];

fn target_of(db: &Db, act: Act) -> Option<(i64, Option<i64>)> {
    match act {
        Act::App(exe) => {
            let a = db.app_for(exe, &rules::display_name(exe), rules::classify(exe)).ok()?;
            Some((a.id, None))
        }
        Act::Site(name) => {
            let a = db.app_for(CHROME, &rules::display_name(CHROME), cats::BROWSER).ok()?;
            let s = db.site_for(name, sites::default_category(name)).ok()?;
            Some((a.id, Some(s.id)))
        }
    }
}

/// Fills `dir` with the demo database and a config that skips the first-run screen.
pub fn fill(dir: &Path) {
    let _ = std::fs::create_dir_all(dir);
    for f in ["timelyx.db", "timelyx.db-wal", "timelyx.db-shm"] {
        let _ = std::fs::remove_file(dir.join(f));
    }
    let Ok(mut db) = Db::open(dir) else { return };
    let tz = chrono::Local;
    let now = chrono::Utc::now().timestamp();
    let today = day_start(&tz, now);
    let mut rows: Vec<(i64, i64, i64, Option<i64>)> = Vec::new();

    for back in (0..14).rev() {
        let day = {
            // Walk back whole local days (a DST day is 23 or 25 hours).
            let mut d = today;
            for _ in 0..back {
                d = day_start(&tz, d - 3600);
            }
            d
        };
        let weekday = crate::engine::weekday_index(&tz, day + 12 * 3600);
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ (day as u64).wrapping_mul(0x2545_F491_4F6C_DD1D));
        let weekend = weekday >= 5;
        // Working hours, with a lunch break; a light, later day at the weekend.
        let spans: Vec<(i64, i64, bool)> = if weekend {
            vec![(11 * 60, 13 * 60 + rng.range(0, 60), false), (15 * 60, 17 * 60 + rng.range(0, 90), false)]
        } else {
            vec![(8 * 60 + 40 + rng.range(0, 25), 12 * 60 + 50 + rng.range(0, 20), true), (14 * 60 + rng.range(0, 10), 18 * 60 + rng.range(0, 50), true)]
        };
        for (from, to, work) in spans {
            let mut t = from;
            while t < to {
                let act = if work { *rng.pick(WORK) } else { *rng.pick(EVENING) };
                let len = match act {
                    Act::App(CODE) => rng.range(18, 70),
                    Act::App(_) => rng.range(2, 14),
                    Act::Site(_) => rng.range(2, 12),
                };
                let end = (t + len).min(to);
                if let Some((a, s)) = target_of(&db, act) {
                    rows.push((day + t * 60, day + end * 60, a, s));
                }
                t = end + rng.range(0, 1);
            }
        }
        // Evening: a game on most days, with some video before it.
        let games_today = [true, false, true, true, false, true, true][weekday] || weekday == 4;
        if games_today {
            let start = 20 * 60 + rng.range(0, 40);
            let game = if rng.range(0, 1) == 0 { HADES } else { CELESTE };
            let len = rng.range(50, 170);
            if let Ok(a) = db.app_for(STEAM, "Steam", cats::GAMES) {
                rows.push((day + (start - 4) * 60, day + start * 60, a.id, None));
            }
            if let Ok(g) = db.app_for(game, &rules::display_name(game), rules::classify(game)) {
                rows.push((day + start * 60, day + (start + len) * 60, g.id, None));
            }
        } else {
            let start = 20 * 60 + rng.range(0, 60);
            let mut t = start;
            while t < start + rng.range(40, 110) {
                let act = *rng.pick(EVENING);
                let end = t + rng.range(5, 30);
                if let Some((a, s)) = target_of(&db, act) {
                    rows.push((day + t * 60, day + end * 60, a, s));
                }
                t = end;
            }
        }
    }
    // Nothing in the future, and today's last session ends a minute ago.
    rows.retain(|r| r.0 < now - 90);
    for r in rows.iter_mut() {
        r.1 = r.1.min(now - 60);
    }
    rows.retain(|r| r.1 > r.0);
    // The window shows "Visual Studio Code, 42 minutes" as the current activity: make it part of today.
    let (cs, ce) = (now - 42 * 60, now - 60);
    rows.retain(|r| r.1 <= cs || r.0 >= ce);
    if let Some((a, s)) = target_of(&db, Act::App(CODE)) {
        rows.push((cs, ce, a, s));
    }
    rows.sort_by_key(|r| r.0);
    let _ = db.append_sessions(&rows);

    // A custom category, a limit or two, and one browser private window for the marker.
    if let Ok(id) = db.add_category("Study", 0x5b6fbb) {
        if let Ok(a) = db.app_for(OBSIDIAN, "Obsidian", cats::OFFICE) {
            let _ = db.set_app_category(a.id, &id);
        }
        if let Ok(s) = db.site_for("MDN", None) {
            let _ = db.set_site_category(s.id, Some(&id));
        }
    }
    let _ = db.set_limit("category", cats::GAMES, 150);
    let _ = db.set_limit("category", cats::VIDEO, 90);

    let cfg = Config { onboarded: true, check_updates: false, language: None, ..Config::default() };
    if !dir.join("config.json").exists() {
        cfg.save_to(dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_fills_two_weeks_of_plausible_data() {
        let dir = std::env::temp_dir().join(format!("timelyx-demo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        fill(&dir);
        let db = Db::open(&dir).unwrap();
        let now = chrono::Utc::now().timestamp();
        let rows = db.rows(now - 15 * 86_400, now + 1).unwrap();
        assert!(rows.len() > 200, "{}", rows.len());
        // No session in the future, none longer than a few hours.
        assert!(rows.iter().all(|r| r.end <= now && r.end - r.start < 4 * 3600));
        let cats_seen: std::collections::HashSet<_> = rows.iter().map(|r| r.category.clone()).collect();
        for c in [cats::CODE, cats::VIDEO, cats::GAMES, cats::COMMUNICATION, cats::OFFICE] {
            assert!(cats_seen.contains(c), "{c}");
        }
        assert!(!db.limits().unwrap().is_empty());
        assert!(Config::load_from(&dir).onboarded);
        // The data never mentions a real user name.
        assert!(db.apps().unwrap().iter().all(|a| !a.exe.to_lowercase().contains("kachu")));
        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
