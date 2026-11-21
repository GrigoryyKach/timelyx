//! Aggregations over session rows: totals, categories, top items, days, the weekday-by-hour heatmap and the
//! longest focus run. Everything is a pure function of the rows, so it is tested without a database.

use crate::db::SessionRow;
use crate::engine::{day_start, next_midnight, weekday_index};
use chrono::{TimeZone, Timelike};
use std::collections::HashMap;

/// Seconds of `row` that fall inside `[from, to)`.
pub fn clip(row: &SessionRow, from: i64, to: i64) -> i64 {
    (row.end.min(to) - row.start.max(from)).max(0)
}

pub fn total(rows: &[SessionRow], from: i64, to: i64) -> i64 {
    rows.iter().map(|r| clip(r, from, to)).sum()
}

/// Seconds per category, largest first.
pub fn by_category(rows: &[SessionRow], from: i64, to: i64) -> Vec<(String, i64)> {
    let mut m: HashMap<&str, i64> = HashMap::new();
    for r in rows {
        let c = clip(r, from, to);
        if c > 0 {
            *m.entry(r.category.as_str()).or_default() += c;
        }
    }
    let mut v: Vec<(String, i64)> = m.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v
}

/// What a row counts for in the lists: the site when there is one, otherwise the program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Item {
    App(i64),
    Site(i64),
}

impl Item {
    pub fn of(r: &SessionRow) -> Item {
        match r.site_id {
            Some(s) => Item::Site(s),
            None => Item::App(r.app_id),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ItemStat {
    pub item: Item,
    pub category: String,
    pub secs: i64,
    pub sessions: usize,
    pub last_used: i64,
}

/// Per-item totals, largest first. The category is the one of the item's latest session.
pub fn by_item(rows: &[SessionRow], from: i64, to: i64) -> Vec<ItemStat> {
    let mut m: HashMap<Item, ItemStat> = HashMap::new();
    for r in rows {
        let c = clip(r, from, to);
        if c <= 0 {
            continue;
        }
        let e = m.entry(Item::of(r)).or_insert_with(|| ItemStat { item: Item::of(r), category: r.category.clone(), secs: 0, sessions: 0, last_used: 0 });
        e.secs += c;
        e.sessions += 1;
        if r.end >= e.last_used {
            e.last_used = r.end;
            e.category = r.category.clone();
        }
    }
    let mut v: Vec<ItemStat> = m.into_values().collect();
    v.sort_by(|a, b| b.secs.cmp(&a.secs).then_with(|| a.item.cmp(&b.item)));
    v
}

/// One day of a period: its start and the seconds per category.
#[derive(Clone, Debug, PartialEq)]
pub struct Day {
    pub start: i64,
    pub end: i64,
    pub by_cat: Vec<(String, i64)>,
    pub total: i64,
}

/// The local days from the one containing `from` up to (not including) `to`.
pub fn days<Tz: TimeZone>(tz: &Tz, rows: &[SessionRow], from: i64, to: i64) -> Vec<Day> {
    let mut out = Vec::new();
    let mut d = day_start(tz, from);
    while d < to {
        let n = next_midnight(tz, d);
        let by_cat = by_category(rows, d, n);
        let total = by_cat.iter().map(|c| c.1).sum();
        out.push(Day { start: d, end: n, by_cat, total });
        d = n;
    }
    out
}

/// Weekday (Monday = 0) by hour of day, in seconds. DST days simply have one hour more or less.
pub fn heatmap<Tz: TimeZone>(tz: &Tz, rows: &[SessionRow], from: i64, to: i64) -> [[i64; 24]; 7] {
    let mut h = [[0i64; 24]; 7];
    for r in rows {
        let mut cur = r.start.max(from);
        let end = r.end.min(to);
        while cur < end {
            let Some(dt) = tz.timestamp_opt(cur, 0).single() else { break };
            let into_hour = (dt.minute() * 60 + dt.second()) as i64;
            let next = (cur + 3600 - into_hour).min(end);
            h[weekday_index(tz, cur)][dt.hour() as usize] += next - cur;
            cur = next;
        }
    }
    h
}

/// The longest run in one category without a switch to another one.
#[derive(Clone, Debug, PartialEq)]
pub struct Focus {
    pub start: i64,
    pub end: i64,
    pub category: String,
}

impl Focus {
    pub fn secs(&self) -> i64 {
        self.end - self.start
    }
}

/// Consecutive sessions of one category that follow each other with gaps of at most `max_gap` seconds form a run.
pub fn longest_focus(rows: &[SessionRow], from: i64, to: i64, max_gap: i64) -> Option<Focus> {
    let mut best: Option<Focus> = None;
    let mut cur: Option<Focus> = None;
    let mut sorted: Vec<&SessionRow> = rows.iter().filter(|r| clip(r, from, to) > 0).collect();
    sorted.sort_by_key(|r| r.start);
    for r in sorted {
        let (s, e) = (r.start.max(from), r.end.min(to));
        cur = match cur.take() {
            Some(mut c) if c.category == r.category && s - c.end <= max_gap => {
                c.end = c.end.max(e);
                Some(c)
            }
            other => {
                if let Some(c) = other {
                    if best.as_ref().is_none_or(|b| c.secs() > b.secs()) {
                        best = Some(c);
                    }
                }
                Some(Focus { start: s, end: e, category: r.category.clone() })
            }
        };
    }
    if let Some(c) = cur {
        if best.as_ref().is_none_or(|b| c.secs() > b.secs()) {
            best = Some(c);
        }
    }
    best
}

/// Change against the previous period in percent; `None` when there is nothing to compare with.
pub fn change_percent(now: i64, before: i64) -> Option<i32> {
    if before <= 0 {
        return None;
    }
    Some((((now - before) as f64 / before as f64) * 100.0).round() as i32)
}

/// Seconds per day of one item, for the days between `from` and `to`.
pub fn item_per_day<Tz: TimeZone>(tz: &Tz, rows: &[SessionRow], item: Item, from: i64, to: i64) -> Vec<(i64, i64)> {
    let mut out = Vec::new();
    let mut d = day_start(tz, from);
    while d < to {
        let n = next_midnight(tz, d);
        let secs = rows.iter().filter(|r| Item::of(r) == item).map(|r| clip(r, d, n)).sum();
        out.push((d, secs));
        d = n;
    }
    out
}

/// Seconds spent in `rows` that match `pred`, between `from` and `to` (used for limits).
pub fn sum_where(rows: &[SessionRow], from: i64, to: i64, pred: impl Fn(&SessionRow) -> bool) -> i64 {
    rows.iter().filter(|r| pred(r)).map(|r| clip(r, from, to)).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, Utc};

    fn row(start: i64, end: i64, app: i64, site: Option<i64>, cat: &str) -> SessionRow {
        SessionRow { start, end, app_id: app, site_id: site, category: cat.into() }
    }

    fn utc() -> FixedOffset {
        FixedOffset::east_opt(0).unwrap()
    }

    // Monday 2026-11-16 00:00 UTC
    fn monday() -> i64 {
        Utc.with_ymd_and_hms(2026, 11, 16, 0, 0, 0).unwrap().timestamp()
    }

    #[test]
    fn totals_clip_to_the_range() {
        let rows = vec![row(0, 100, 1, None, "code"), row(90, 200, 2, None, "video")];
        assert_eq!(total(&rows, 0, 1000), 210);
        assert_eq!(total(&rows, 50, 150), 50 + 60);
        assert_eq!(total(&rows, 300, 400), 0);
    }

    #[test]
    fn categories_sorted_by_time() {
        let rows = vec![row(0, 100, 1, None, "code"), row(100, 400, 2, None, "video"), row(400, 450, 1, None, "code")];
        assert_eq!(by_category(&rows, 0, 1000), vec![("video".to_string(), 300), ("code".to_string(), 150)]);
    }

    #[test]
    fn items_prefer_the_site_and_count_sessions() {
        let rows = vec![
            row(0, 100, 1, Some(10), "video"),
            row(100, 150, 1, None, "browser"),
            row(150, 250, 1, Some(10), "video"),
            row(250, 260, 2, None, "code"),
        ];
        let items = by_item(&rows, 0, 1000);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].item, Item::Site(10));
        assert_eq!((items[0].secs, items[0].sessions, items[0].last_used), (200, 2, 250));
        assert_eq!(items[1].item, Item::App(1));
        assert_eq!(items[2].item, Item::App(2));
    }

    #[test]
    fn days_split_and_sum() {
        let m = monday();
        let rows = vec![row(m + 23 * 3600, m + 25 * 3600, 1, None, "code"), row(m + 30 * 3600, m + 31 * 3600, 1, None, "video")];
        let d = days(&utc(), &rows, m, m + 3 * 86_400);
        assert_eq!(d.len(), 3);
        assert_eq!(d[0].total, 3600);
        assert_eq!(d[1].total, 3600 + 3600);
        assert_eq!(d[1].by_cat.len(), 2);
        assert_eq!(d[2].total, 0);
        // A range that starts mid-day still starts at that day's midnight.
        assert_eq!(days(&utc(), &rows, m + 5 * 3600, m + 86_400)[0].start, m);
    }

    #[test]
    fn heatmap_buckets_by_weekday_and_hour() {
        let m = monday();
        // Monday 09:30 - 11:15, Wednesday 23:30 - Thursday 00:30.
        let rows = vec![row(m + 9 * 3600 + 1800, m + 11 * 3600 + 900, 1, None, "code"), row(m + 2 * 86_400 + 23 * 3600 + 1800, m + 3 * 86_400 + 1800, 1, None, "code")];
        let h = heatmap(&utc(), &rows, m, m + 7 * 86_400);
        assert_eq!(h[0][9], 1800);
        assert_eq!(h[0][10], 3600);
        assert_eq!(h[0][11], 900);
        assert_eq!(h[2][23], 1800);
        assert_eq!(h[3][0], 1800);
        let sum: i64 = h.iter().flatten().sum();
        assert_eq!(sum, total(&rows, m, m + 7 * 86_400));
    }

    #[test]
    fn heatmap_with_a_half_hour_zone_uses_local_hours() {
        let tz = FixedOffset::east_opt(5 * 3600 + 1800).unwrap(); // UTC+5:30
        let m = Utc.with_ymd_and_hms(2026, 11, 16, 0, 0, 0).unwrap().timestamp(); // 05:30 local Monday
        let rows = vec![row(m, m + 3600, 1, None, "code")];
        let h = heatmap(&tz, &rows, m - 86_400, m + 86_400);
        assert_eq!(h[0][5], 1800);
        assert_eq!(h[0][6], 1800);
    }

    #[test]
    fn longest_focus_merges_same_category_runs() {
        let rows = vec![
            row(0, 600, 1, None, "code"),
            row(600, 1200, 2, None, "code"), // another app, same category: one run
            row(1200, 1300, 3, None, "video"),
            row(1300, 1500, 1, None, "code"),
            row(5000, 7000, 1, None, "code"), // after a long gap: a new run, the longest
        ];
        let f = longest_focus(&rows, 0, 10_000, 120).unwrap();
        assert_eq!((f.start, f.end, f.category.as_str()), (5000, 7000, "code"));
        // With a huge gap allowance the first two runs do not join across the video session.
        let f2 = longest_focus(&rows[..4], 0, 10_000, 120).unwrap();
        assert_eq!((f2.start, f2.end), (0, 1200));
        assert_eq!(f2.secs(), 1200);
        assert!(longest_focus(&[], 0, 10, 60).is_none());
    }

    #[test]
    fn change_percent_rules() {
        assert_eq!(change_percent(150, 100), Some(50));
        assert_eq!(change_percent(50, 100), Some(-50));
        assert_eq!(change_percent(10, 0), None);
        assert_eq!(change_percent(0, 100), Some(-100));
    }

    #[test]
    fn per_day_series_for_one_item() {
        let m = monday();
        let rows = vec![row(m + 3600, m + 7200, 1, Some(5), "video"), row(m + 86_400, m + 86_400 + 600, 1, Some(5), "video"), row(m, m + 100, 2, None, "code")];
        let s = item_per_day(&utc(), &rows, Item::Site(5), m, m + 3 * 86_400);
        assert_eq!(s.iter().map(|x| x.1).collect::<Vec<_>>(), vec![3600, 600, 0]);
    }

    #[test]
    fn sum_where_filters() {
        let rows = vec![row(0, 100, 1, None, "games"), row(100, 150, 2, None, "code")];
        assert_eq!(sum_where(&rows, 0, 1000, |r| r.category == "games"), 100);
        assert_eq!(sum_where(&rows, 0, 120, |r| r.app_id == 2), 20);
    }
}
