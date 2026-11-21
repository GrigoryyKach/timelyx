//! SQLite storage (WAL mode): sessions, apps, sites, custom categories and limits.
//!
//! The tracker process writes; the window process reads and edits categories. Sessions are appended in batches
//! (once a minute and on exit), never one by one, and a slice that continues the previous row of the same
//! activity extends that row instead of adding a new one.

use chrono::TimeZone;
use rusqlite::{params, Connection, OptionalExtension};
use std::io::Write;
use std::path::Path;

use crate::cats;

#[derive(Clone, Debug, PartialEq)]
pub struct AppRow {
    pub id: i64,
    pub exe: String,
    pub name: String,
    pub category: String,
    /// The user chose the category, so automatic rules (fullscreen means game) leave it alone.
    pub manual: bool,
    pub hidden: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SiteRow {
    pub id: i64,
    pub name: String,
    /// `None`: the site counts in its browser's category.
    pub category: Option<String>,
    pub hidden: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CategoryRow {
    pub id: String,
    pub name: String,
    pub color: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LimitRow {
    /// `category` or `app`.
    pub kind: String,
    /// A category id, or an app id as text.
    pub target: String,
    pub minutes: u32,
}

/// One session with its effective category already resolved (the site's own category wins over the browser's).
#[derive(Clone, Debug, PartialEq)]
pub struct SessionRow {
    pub start: i64,
    pub end: i64,
    pub app_id: i64,
    pub site_id: Option<i64>,
    pub category: String,
}

struct Last {
    id: i64,
    app: i64,
    site: Option<i64>,
    end: i64,
}

pub struct Db {
    conn: Connection,
    last: Option<Last>,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS apps(
    id INTEGER PRIMARY KEY,
    exe TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    category TEXT NOT NULL,
    manual INTEGER NOT NULL DEFAULT 0,
    hidden INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS sites(
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    category TEXT,
    hidden INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS sessions(
    id INTEGER PRIMARY KEY,
    start INTEGER NOT NULL,
    end INTEGER NOT NULL,
    app_id INTEGER NOT NULL REFERENCES apps(id),
    site_id INTEGER REFERENCES sites(id)
);
CREATE INDEX IF NOT EXISTS sessions_start ON sessions(start);
CREATE INDEX IF NOT EXISTS sessions_end ON sessions(end);
CREATE INDEX IF NOT EXISTS sessions_app ON sessions(app_id, start);
CREATE INDEX IF NOT EXISTS sessions_site ON sessions(site_id, start);
CREATE TABLE IF NOT EXISTS categories(
    n INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    color INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS limits(
    kind TEXT NOT NULL,
    target TEXT NOT NULL,
    minutes INTEGER NOT NULL,
    PRIMARY KEY(kind, target)
);
CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
";

type R<T> = rusqlite::Result<T>;

impl Db {
    /// Opens (and creates) `timelyx.db` in `dir`.
    pub fn open(dir: &Path) -> R<Db> {
        let _ = std::fs::create_dir_all(dir);
        Self::init(Connection::open(dir.join("timelyx.db"))?)
    }

    #[cfg(test)]
    pub fn open_memory() -> R<Db> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> R<Db> {
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        // WAL lets the window read while the tracker writes. `journal_mode` answers with a row, so use query_row.
        let _: String = conn.query_row("PRAGMA journal_mode=WAL", [], |r| r.get(0))?;
        conn.execute_batch("PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;")?;
        conn.execute_batch(SCHEMA)?;
        conn.execute_batch("PRAGMA user_version=1;")?;
        Ok(Db { conn, last: None })
    }

    // ───────────── apps and sites ─────────────

    fn app_from(r: &rusqlite::Row) -> R<AppRow> {
        Ok(AppRow { id: r.get(0)?, exe: r.get(1)?, name: r.get(2)?, category: r.get(3)?, manual: r.get::<_, i64>(4)? != 0, hidden: r.get::<_, i64>(5)? != 0 })
    }

    /// The row for an executable, created with `name` and `category` the first time it is seen.
    pub fn app_for(&self, exe: &str, name: &str, category: &str) -> R<AppRow> {
        if let Some(a) = self
            .conn
            .query_row("SELECT id,exe,name,category,manual,hidden FROM apps WHERE exe=?1", [exe], Self::app_from)
            .optional()?
        {
            return Ok(a);
        }
        self.conn.execute("INSERT INTO apps(exe,name,category) VALUES(?1,?2,?3)", params![exe, name, category])?;
        Ok(AppRow { id: self.conn.last_insert_rowid(), exe: exe.into(), name: name.into(), category: category.into(), manual: false, hidden: false })
    }

    pub fn apps(&self) -> R<Vec<AppRow>> {
        let mut st = self.conn.prepare("SELECT id,exe,name,category,manual,hidden FROM apps ORDER BY name COLLATE NOCASE")?;
        let rows = st.query_map([], Self::app_from)?.collect::<R<Vec<_>>>()?;
        Ok(rows)
    }

    /// The user's choice: the automatic rules never change it again.
    pub fn set_app_category(&self, id: i64, category: &str) -> R<()> {
        self.conn.execute("UPDATE apps SET category=?2, manual=1 WHERE id=?1", params![id, category])?;
        Ok(())
    }

    /// An automatic change (fullscreen means a game). Ignored for apps the user has categorised.
    pub fn set_app_auto_category(&self, id: i64, category: &str) -> R<()> {
        self.conn.execute("UPDATE apps SET category=?2 WHERE id=?1 AND manual=0", params![id, category])?;
        Ok(())
    }

    pub fn rename_app(&self, id: i64, name: &str) -> R<()> {
        self.conn.execute("UPDATE apps SET name=?2 WHERE id=?1", params![id, name.trim()])?;
        Ok(())
    }

    pub fn set_app_hidden(&self, id: i64, hidden: bool) -> R<()> {
        self.conn.execute("UPDATE apps SET hidden=?2 WHERE id=?1", params![id, hidden as i64])?;
        Ok(())
    }

    /// The row for a site, created with its shipped default category the first time it is seen.
    pub fn site_for(&self, name: &str, default_category: Option<&str>) -> R<SiteRow> {
        if let Some(s) = self.conn.query_row("SELECT id,name,category,hidden FROM sites WHERE name=?1", [name], Self::site_from).optional()? {
            return Ok(s);
        }
        self.conn.execute("INSERT INTO sites(name,category) VALUES(?1,?2)", params![name, default_category])?;
        Ok(SiteRow { id: self.conn.last_insert_rowid(), name: name.into(), category: default_category.map(str::to_string), hidden: false })
    }

    fn site_from(r: &rusqlite::Row) -> R<SiteRow> {
        Ok(SiteRow { id: r.get(0)?, name: r.get(1)?, category: r.get(2)?, hidden: r.get::<_, i64>(3)? != 0 })
    }

    pub fn sites(&self) -> R<Vec<SiteRow>> {
        let mut st = self.conn.prepare("SELECT id,name,category,hidden FROM sites ORDER BY name COLLATE NOCASE")?;
        let rows = st.query_map([], Self::site_from)?.collect::<R<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn set_site_category(&self, id: i64, category: Option<&str>) -> R<()> {
        self.conn.execute("UPDATE sites SET category=?2 WHERE id=?1", params![id, category])?;
        Ok(())
    }

    pub fn set_site_hidden(&self, id: i64, hidden: bool) -> R<()> {
        self.conn.execute("UPDATE sites SET hidden=?2 WHERE id=?1", params![id, hidden as i64])?;
        Ok(())
    }

    // ───────────── categories ─────────────

    pub fn categories(&self) -> R<Vec<CategoryRow>> {
        let mut st = self.conn.prepare("SELECT n,name,color FROM categories ORDER BY n")?;
        let rows = st
            .query_map([], |r| Ok(CategoryRow { id: format!("c{}", r.get::<_, i64>(0)?), name: r.get(1)?, color: r.get::<_, i64>(2)? as u32 }))?
            .collect::<R<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn add_category(&self, name: &str, color: u32) -> R<String> {
        self.conn.execute("INSERT INTO categories(name,color) VALUES(?1,?2)", params![name.trim(), color as i64])?;
        Ok(format!("c{}", self.conn.last_insert_rowid()))
    }

    pub fn update_category(&self, id: &str, name: &str, color: u32) -> R<()> {
        if let Some(n) = id.strip_prefix('c').and_then(|n| n.parse::<i64>().ok()) {
            self.conn.execute("UPDATE categories SET name=?2,color=?3 WHERE n=?1", params![n, name.trim(), color as i64])?;
        }
        Ok(())
    }

    /// Deletes a custom category. What used it falls back: programs to Other, sites to their browser's category.
    pub fn delete_category(&self, id: &str) -> R<()> {
        let Some(n) = id.strip_prefix('c').and_then(|n| n.parse::<i64>().ok()) else { return Ok(()) };
        self.conn.execute("DELETE FROM categories WHERE n=?1", [n])?;
        self.conn.execute("UPDATE apps SET category=?2 WHERE category=?1", params![id, cats::OTHER])?;
        self.conn.execute("UPDATE sites SET category=NULL WHERE category=?1", [id])?;
        self.conn.execute("DELETE FROM limits WHERE kind='category' AND target=?1", [id])?;
        Ok(())
    }

    // ───────────── limits ─────────────

    pub fn limits(&self) -> R<Vec<LimitRow>> {
        let mut st = self.conn.prepare("SELECT kind,target,minutes FROM limits ORDER BY kind,target")?;
        let rows = st
            .query_map([], |r| Ok(LimitRow { kind: r.get(0)?, target: r.get(1)?, minutes: r.get::<_, i64>(2)? as u32 }))?
            .collect::<R<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn set_limit(&self, kind: &str, target: &str, minutes: u32) -> R<()> {
        self.conn.execute(
            "INSERT INTO limits(kind,target,minutes) VALUES(?1,?2,?3) ON CONFLICT(kind,target) DO UPDATE SET minutes=excluded.minutes",
            params![kind, target, minutes as i64],
        )?;
        Ok(())
    }

    pub fn remove_limit(&self, kind: &str, target: &str) -> R<()> {
        self.conn.execute("DELETE FROM limits WHERE kind=?1 AND target=?2", params![kind, target])?;
        Ok(())
    }

    // ───────────── meta ─────────────

    pub fn meta(&self, key: &str) -> Option<String> {
        self.conn.query_row("SELECT value FROM meta WHERE key=?1", [key], |r| r.get(0)).optional().ok().flatten()
    }

    pub fn set_meta(&self, key: &str, value: &str) -> R<()> {
        self.conn.execute("INSERT INTO meta(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key, value])?;
        Ok(())
    }

    // ───────────── sessions ─────────────

    /// Appends sessions `(start, end, app_id, site_id)` in one transaction. A slice that starts where the previous
    /// row of the same app and site ended (within a second) extends that row.
    pub fn append_sessions(&mut self, list: &[(i64, i64, i64, Option<i64>)]) -> R<()> {
        if list.is_empty() {
            return Ok(());
        }
        let tx = self.conn.transaction()?;
        let mut last = self.last.take();
        for &(start, end, app, site) in list {
            if end <= start {
                continue;
            }
            match &mut last {
                // Never merge across local midnight: a row stays inside one day.
                Some(l) if l.app == app && l.site == site && (start - l.end).abs() <= 1 && crate::engine::day_start(&chrono::Local, start) != start => {
                    tx.execute("UPDATE sessions SET end=?2 WHERE id=?1", params![l.id, end])?;
                    l.end = end;
                }
                _ => {
                    tx.execute("INSERT INTO sessions(start,end,app_id,site_id) VALUES(?1,?2,?3,?4)", params![start, end, app, site])?;
                    last = Some(Last { id: tx.last_insert_rowid(), app, site, end });
                }
            }
        }
        tx.commit()?;
        self.last = last;
        Ok(())
    }

    /// Forgets the last written row (after rows were deleted by another process).
    pub fn forget_last(&mut self) {
        self.last = None;
    }

    /// Sessions that touch `[from, to)`, with their effective category. Hidden apps and sites are left out.
    pub fn rows(&self, from: i64, to: i64) -> R<Vec<SessionRow>> {
        self.rows_inner(from, to, false)
    }

    /// Like [`Db::rows`], but hidden apps and sites are included (for the list that lets you show them again).
    pub fn rows_with_hidden(&self, from: i64, to: i64) -> R<Vec<SessionRow>> {
        self.rows_inner(from, to, true)
    }

    fn rows_inner(&self, from: i64, to: i64, hidden: bool) -> R<Vec<SessionRow>> {
        let mut st = self.conn.prepare(
            "SELECT s.start, s.end, s.app_id, s.site_id,
                    CASE WHEN s.site_id IS NOT NULL AND st.category IS NOT NULL THEN st.category ELSE a.category END
             FROM sessions s
             JOIN apps a ON a.id = s.app_id
             LEFT JOIN sites st ON st.id = s.site_id
             WHERE s.end > ?1 AND s.start < ?2 AND (?3 = 1 OR (a.hidden = 0 AND COALESCE(st.hidden, 0) = 0))
             ORDER BY s.start",
        )?;
        let rows = st
            .query_map(params![from, to, hidden as i64], |r| Ok(SessionRow { start: r.get(0)?, end: r.get(1)?, app_id: r.get(2)?, site_id: r.get(3)?, category: r.get(4)? }))?
            .collect::<R<Vec<_>>>()?;
        Ok(rows)
    }

    /// Time of the very first session, if any.
    pub fn first_start(&self) -> Option<i64> {
        self.conn.query_row("SELECT MIN(start) FROM sessions", [], |r| r.get::<_, Option<i64>>(0)).ok().flatten()
    }

    /// Deletes sessions that ended before `cutoff`.
    pub fn apply_retention(&mut self, cutoff: i64) -> R<usize> {
        self.last = None;
        Ok(self.conn.execute("DELETE FROM sessions WHERE end < ?1", [cutoff])?)
    }

    /// Deletes every tracked session, program and site (custom categories and limits stay).
    pub fn delete_all(&mut self) -> R<()> {
        self.last = None;
        self.conn.execute_batch("DELETE FROM sessions; DELETE FROM sites; DELETE FROM apps; DELETE FROM meta;")?;
        let _ = self.conn.execute_batch("VACUUM;");
        Ok(())
    }

    #[cfg(test)]
    pub fn session_count(&self) -> i64 {
        self.conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0)).unwrap_or(0)
    }

    /// Rows for the CSV export, hidden items included, in time order.
    pub fn export_rows(&self, from: i64, to: i64) -> R<Vec<ExportRow>> {
        let mut st = self.conn.prepare(
            "SELECT s.start, s.end, a.name, st.name,
                    CASE WHEN s.site_id IS NOT NULL AND st.category IS NOT NULL THEN st.category ELSE a.category END
             FROM sessions s
             JOIN apps a ON a.id = s.app_id
             LEFT JOIN sites st ON st.id = s.site_id
             WHERE s.start >= ?1 AND s.start < ?2
             ORDER BY s.start",
        )?;
        let rows = st
            .query_map([from, to], |r| Ok(ExportRow { start: r.get(0)?, end: r.get(1)?, app: r.get(2)?, site: r.get(3)?, category: r.get(4)? }))?
            .collect::<R<Vec<_>>>()?;
        Ok(rows)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExportRow {
    pub start: i64,
    pub end: i64,
    pub app: String,
    pub site: Option<String>,
    pub category: String,
}

fn csv_field(s: &str) -> String {
    // A leading =, +, - or @ would be read as a formula by spreadsheets.
    let needs_guard = s.starts_with(['=', '+', '-', '@']);
    let s = if needs_guard { format!("'{s}") } else { s.to_string() };
    if s.contains([',', '"', '\n', '\r', ';']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s
    }
}

/// Writes the sessions as CSV: date, start, end, seconds, app, site, category. `cat_name` turns a category id into
/// its name; `site_label` turns the stored special site names into text.
pub fn write_csv<Tz: TimeZone, W: Write>(
    rows: &[ExportRow],
    tz: &Tz,
    cat_name: &dyn Fn(&str) -> String,
    site_label: &dyn Fn(&str) -> String,
    out: &mut W,
) -> std::io::Result<()>
where
    Tz::Offset: std::fmt::Display,
{
    writeln!(out, "date,start,end,seconds,app,site,category")?;
    for r in rows {
        let (Some(a), Some(b)) = (tz.timestamp_opt(r.start, 0).single(), tz.timestamp_opt(r.end, 0).single()) else { continue };
        writeln!(
            out,
            "{},{},{},{},{},{},{}",
            a.format("%Y-%m-%d"),
            a.format("%H:%M:%S"),
            b.format("%H:%M:%S"),
            r.end - r.start,
            csv_field(&r.app),
            csv_field(&r.site.as_deref().map(site_label).unwrap_or_default()),
            csv_field(&cat_name(&r.category)),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    fn db() -> Db {
        Db::open_memory().unwrap()
    }

    #[test]
    fn apps_and_sites_are_created_once() {
        let d = db();
        let a = d.app_for(r"C:\x\chrome.exe", "Google Chrome", cats::BROWSER).unwrap();
        let b = d.app_for(r"C:\x\chrome.exe", "Other name", cats::OTHER).unwrap();
        assert_eq!(a.id, b.id);
        assert_eq!(b.name, "Google Chrome");
        let s = d.site_for("YouTube", Some(cats::VIDEO)).unwrap();
        assert_eq!(s.category.as_deref(), Some(cats::VIDEO));
        assert_eq!(d.site_for("YouTube", None).unwrap().id, s.id);
        assert_eq!(d.site_for("The Verge", None).unwrap().category, None);
    }

    #[test]
    fn manual_category_beats_automatic() {
        let d = db();
        let a = d.app_for("g.exe", "G", cats::OTHER).unwrap();
        d.set_app_auto_category(a.id, cats::GAMES).unwrap();
        assert_eq!(d.apps().unwrap()[0].category, cats::GAMES);
        d.set_app_category(a.id, cats::OFFICE).unwrap();
        d.set_app_auto_category(a.id, cats::GAMES).unwrap();
        let row = &d.apps().unwrap()[0];
        assert_eq!(row.category, cats::OFFICE);
        assert!(row.manual);
    }

    #[test]
    fn slices_of_one_activity_extend_one_row() {
        let mut d = db();
        let a = d.app_for("a.exe", "A", cats::CODE).unwrap();
        let b = d.app_for("b.exe", "B", cats::CODE).unwrap();
        d.append_sessions(&[(0, 60, a.id, None)]).unwrap();
        d.append_sessions(&[(60, 120, a.id, None)]).unwrap();
        d.append_sessions(&[(120, 180, b.id, None), (180, 200, a.id, None)]).unwrap();
        assert_eq!(d.session_count(), 3);
        let rows = d.rows(0, 1000).unwrap();
        assert_eq!((rows[0].start, rows[0].end, rows[0].app_id), (0, 120, a.id));
        assert_eq!((rows[1].start, rows[1].end, rows[1].app_id), (120, 180, b.id));
        // A gap starts a new row.
        d.append_sessions(&[(500, 560, a.id, None)]).unwrap();
        assert_eq!(d.session_count(), 4);
    }

    #[test]
    fn rows_resolve_site_category_and_skip_hidden() {
        let mut d = db();
        let chrome = d.app_for("chrome.exe", "Chrome", cats::BROWSER).unwrap();
        let yt = d.site_for("YouTube", Some(cats::VIDEO)).unwrap();
        let plain = d.site_for("The Verge", None).unwrap();
        d.append_sessions(&[(0, 100, chrome.id, Some(yt.id)), (100, 200, chrome.id, Some(plain.id)), (200, 300, chrome.id, None)]).unwrap();
        let rows = d.rows(0, 1000).unwrap();
        assert_eq!(rows.iter().map(|r| r.category.as_str()).collect::<Vec<_>>(), vec![cats::VIDEO, cats::BROWSER, cats::BROWSER]);
        d.set_site_hidden(plain.id, true).unwrap();
        assert_eq!(d.rows(0, 1000).unwrap().len(), 2);
        d.set_app_hidden(chrome.id, true).unwrap();
        assert!(d.rows(0, 1000).unwrap().is_empty());
        // The range filter keeps sessions that overlap the range.
        d.set_app_hidden(chrome.id, false).unwrap();
        d.set_site_hidden(plain.id, false).unwrap();
        assert_eq!(d.rows(150, 250).unwrap().len(), 2);
        assert_eq!(d.rows(300, 400).unwrap().len(), 0);
    }

    #[test]
    fn custom_categories_and_deleting_one() {
        let d = db();
        let id = d.add_category("Study", 0x5b6fbb).unwrap();
        assert_eq!(id, "c1");
        let a = d.app_for("a.exe", "A", cats::OTHER).unwrap();
        d.set_app_category(a.id, &id).unwrap();
        let s = d.site_for("MDN", None).unwrap();
        d.set_site_category(s.id, Some(&id)).unwrap();
        d.set_limit("category", &id, 90).unwrap();
        assert_eq!(d.categories().unwrap(), vec![CategoryRow { id: id.clone(), name: "Study".into(), color: 0x5b6fbb }]);
        d.update_category(&id, "Learning", 0x058778).unwrap();
        assert_eq!(d.categories().unwrap()[0].name, "Learning");
        d.delete_category(&id).unwrap();
        assert!(d.categories().unwrap().is_empty());
        assert_eq!(d.apps().unwrap()[0].category, cats::OTHER);
        assert_eq!(d.sites().unwrap()[0].category, None);
        assert!(d.limits().unwrap().is_empty());
    }

    #[test]
    fn limits_are_upserted() {
        let d = db();
        d.set_limit("category", cats::GAMES, 120).unwrap();
        d.set_limit("category", cats::GAMES, 60).unwrap();
        d.set_limit("app", "7", 30).unwrap();
        let l = d.limits().unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l.iter().find(|x| x.kind == "category").unwrap().minutes, 60);
        d.remove_limit("app", "7").unwrap();
        assert_eq!(d.limits().unwrap().len(), 1);
    }

    #[test]
    fn retention_removes_old_sessions_only() {
        let mut d = db();
        let a = d.app_for("a.exe", "A", cats::CODE).unwrap();
        d.append_sessions(&[(0, 100, a.id, None), (1000, 1100, a.id, None), (5000, 5100, a.id, None)]).unwrap();
        assert_eq!(d.apply_retention(1050).unwrap(), 1);
        assert_eq!(d.session_count(), 2);
        assert_eq!(d.apply_retention(0).unwrap(), 0);
        assert_eq!(d.first_start(), Some(1000));
    }

    #[test]
    fn delete_all_wipes_data_but_keeps_custom_categories() {
        let mut d = db();
        d.add_category("Study", 1).unwrap();
        let a = d.app_for("a.exe", "A", cats::CODE).unwrap();
        d.append_sessions(&[(0, 100, a.id, None)]).unwrap();
        d.delete_all().unwrap();
        assert_eq!(d.session_count(), 0);
        assert!(d.apps().unwrap().is_empty());
        assert_eq!(d.categories().unwrap().len(), 1);
        // And the database still works afterwards.
        let a = d.app_for("a.exe", "A", cats::CODE).unwrap();
        d.append_sessions(&[(0, 100, a.id, None)]).unwrap();
        assert_eq!(d.session_count(), 1);
    }

    #[test]
    fn meta_round_trip() {
        let d = db();
        assert_eq!(d.meta("k"), None);
        d.set_meta("k", "v").unwrap();
        d.set_meta("k", "w").unwrap();
        assert_eq!(d.meta("k").as_deref(), Some("w"));
    }

    #[test]
    fn csv_export_has_header_local_times_and_escaping() {
        let mut d = db();
        let a = d.app_for("a.exe", "My, \"App\"", cats::CODE).unwrap();
        let s = d.site_for("*private", None).unwrap();
        let evil = d.app_for("e.exe", "=cmd()", cats::OTHER).unwrap();
        // 2026-11-20 12:00:00 UTC
        let t0 = 1_795_262_400;
        d.append_sessions(&[(t0, t0 + 90, a.id, None), (t0 + 90, t0 + 100, a.id, Some(s.id)), (t0 + 200, t0 + 260, evil.id, None)]).unwrap();
        let rows = d.export_rows(t0 - 10, t0 + 1000).unwrap();
        assert_eq!(rows.len(), 3);
        let tz = FixedOffset::east_opt(2 * 3600).unwrap();
        let mut out = Vec::new();
        write_csv(&rows, &tz, &|c| c.to_uppercase(), &|s| if s == "*private" { "Private browsing".into() } else { s.into() }, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "date,start,end,seconds,app,site,category");
        assert!(lines[1].starts_with(&format!("{},", chrono::DateTime::from_timestamp(t0, 0).unwrap().with_timezone(&tz).format("%Y-%m-%d"))));
        assert!(lines[1].contains("14:00:00,14:01:30,90,\"My, \"\"App\"\"\",,CODE"), "{}", lines[1]);
        assert!(lines[2].contains("Private browsing"));
        assert!(lines[3].contains(",'=cmd(),"), "{}", lines[3]);
    }

    #[test]
    fn export_respects_the_range() {
        let mut d = db();
        let a = d.app_for("a.exe", "A", cats::CODE).unwrap();
        d.append_sessions(&[(100, 200, a.id, None), (1000, 1100, a.id, None)]).unwrap();
        assert_eq!(d.export_rows(0, 500).unwrap().len(), 1);
        assert_eq!(d.export_rows(0, 5000).unwrap().len(), 2);
    }
}
