//! What the window knows about the data: names and colours of categories, programs and sites, periods
//! (week, month) and the live state reported by the tracker.

use crate::cats;
use crate::db::{AppRow, CategoryRow, Db, LimitRow, SiteRow};
use crate::i18n::Lang;
use crate::sites;
use crate::stats::Item;
use crate::strings;
use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone, Weekday};
use std::collections::HashMap;

/// A category as the interface needs it.
#[derive(Clone, Debug, PartialEq)]
pub struct CatInfo {
    pub id: String,
    pub name: String,
    pub color: u32,
    pub builtin: bool,
}

#[derive(Default)]
pub struct Catalog {
    pub apps: HashMap<i64, AppRow>,
    pub sites: HashMap<i64, SiteRow>,
    pub customs: Vec<CategoryRow>,
    pub limits: Vec<LimitRow>,
}

impl Catalog {
    pub fn load(db: &Db) -> Catalog {
        Catalog {
            apps: db.apps().unwrap_or_default().into_iter().map(|a| (a.id, a)).collect(),
            sites: db.sites().unwrap_or_default().into_iter().map(|s| (s.id, s)).collect(),
            customs: db.categories().unwrap_or_default(),
            limits: db.limits().unwrap_or_default(),
        }
    }

    /// Built-in categories first, then the user's own.
    pub fn categories(&self, lang: Lang) -> Vec<CatInfo> {
        let mut v: Vec<CatInfo> = cats::BUILTIN
            .iter()
            .map(|(id, color)| CatInfo { id: id.to_string(), name: strings::builtin_name(lang, id).unwrap_or(id).to_string(), color: *color, builtin: true })
            .collect();
        v.extend(self.customs.iter().map(|c| CatInfo { id: c.id.clone(), name: c.name.clone(), color: c.color, builtin: false }));
        v
    }

    pub fn category(&self, lang: Lang, id: &str) -> CatInfo {
        if let Some(n) = strings::builtin_name(lang, id) {
            return CatInfo { id: id.into(), name: n.into(), color: cats::builtin_color(id).unwrap_or(0x71717a), builtin: true };
        }
        match self.customs.iter().find(|c| c.id == id) {
            Some(c) => CatInfo { id: c.id.clone(), name: c.name.clone(), color: c.color, builtin: false },
            None => CatInfo { id: id.into(), name: strings::builtin_name(lang, cats::OTHER).unwrap_or("Other").into(), color: 0x71717a, builtin: true },
        }
    }

    /// The name of a program or site as shown in lists (special site names are translated).
    pub fn item_name(&self, item: Item, lang: Lang) -> String {
        match item {
            Item::App(id) => self.apps.get(&id).map(|a| a.name.clone()).unwrap_or_default(),
            Item::Site(id) => self.sites.get(&id).map(|s| site_label(lang, &s.name)).unwrap_or_default(),
        }
    }

    pub fn is_hidden(&self, item: Item) -> bool {
        match item {
            Item::App(id) => self.apps.get(&id).is_some_and(|a| a.hidden),
            Item::Site(id) => self.sites.get(&id).is_some_and(|s| s.hidden),
        }
    }

    /// Number of programs and sites currently in a category.
    pub fn count_in(&self, category: &str) -> usize {
        self.apps.values().filter(|a| a.category == category).count() + self.sites.values().filter(|s| s.category.as_deref() == Some(category)).count()
    }

    pub fn limit_target_name(&self, lang: Lang, l: &LimitRow) -> String {
        if l.kind == "app" {
            return l.target.parse::<i64>().ok().and_then(|id| self.apps.get(&id)).map(|a| a.name.clone()).unwrap_or_default();
        }
        self.category(lang, &l.target).name
    }
}

/// "Private browsing" and "Other sites" for the two special stored names.
pub fn site_label(lang: Lang, name: &str) -> String {
    let t = strings::TABLE.get(lang);
    match name {
        sites::PRIVATE => t.private_browsing.to_string(),
        sites::OTHER => t.other_sites.to_string(),
        n => n.to_string(),
    }
}

// ───────────────────────── periods ─────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Span {
    Week,
    Month,
}

pub fn local_midnight(d: NaiveDate) -> i64 {
    for h in 0..4 {
        if let Some(t) = Local.from_local_datetime(&d.and_hms_opt(h, 0, 0).unwrap()).earliest() {
            return t.timestamp();
        }
    }
    d.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp()
}

/// First and last-plus-one day of a week or month; `offset` 0 is the current one, -1 the previous.
pub fn period_dates(span: Span, offset: i32, today: NaiveDate, sunday_first: bool) -> (NaiveDate, NaiveDate) {
    match span {
        Span::Week => {
            let first = if sunday_first { Weekday::Sun } else { Weekday::Mon };
            let back = (today.weekday().num_days_from_monday() as i64 - first.num_days_from_monday() as i64).rem_euclid(7);
            let start = today - Duration::days(back) + Duration::days(7 * offset as i64);
            (start, start + Duration::days(7))
        }
        Span::Month => {
            let total = today.year() * 12 + today.month0() as i32 + offset;
            let (y, m) = (total.div_euclid(12), total.rem_euclid(12) as u32 + 1);
            let start = NaiveDate::from_ymd_opt(y, m, 1).unwrap();
            let end = if m == 12 { NaiveDate::from_ymd_opt(y + 1, 1, 1) } else { NaiveDate::from_ymd_opt(y, m + 1, 1) }.unwrap();
            (start, end)
        }
    }
}

// ───────────────────────── live state ─────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub struct LiveSnap {
    pub app: String,
    pub site: Option<String>,
    pub category: String,
    pub start: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DaemonSnap {
    /// The tracker answered.
    pub running: bool,
    /// `ok`, `wayland` or `error:...`.
    pub avail: String,
    /// `running`, `idle`, `locked` or `paused`.
    pub state: String,
    pub paused_until: i64,
    pub live: Option<LiveSnap>,
}

impl DaemonSnap {
    pub fn parse(json: &str) -> DaemonSnap {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else { return DaemonSnap::default() };
        let live = v.get("live").filter(|l| l.is_object()).map(|l| LiveSnap {
            app: l["app"].as_str().unwrap_or("").to_string(),
            site: l["site"].as_str().map(str::to_string),
            category: l["category"].as_str().unwrap_or(cats::OTHER).to_string(),
            start: l["start"].as_i64().unwrap_or(0),
        });
        DaemonSnap {
            running: true,
            avail: v["avail"].as_str().unwrap_or("ok").to_string(),
            state: v["state"].as_str().unwrap_or("running").to_string(),
            paused_until: v["paused_until"].as_i64().unwrap_or(0),
            live,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn weeks_start_on_monday_or_sunday() {
        // 2026-11-21 is a Saturday.
        assert_eq!(period_dates(Span::Week, 0, d(2026, 11, 21), false), (d(2026, 11, 16), d(2026, 11, 23)));
        assert_eq!(period_dates(Span::Week, 0, d(2026, 11, 21), true), (d(2026, 11, 15), d(2026, 11, 22)));
        assert_eq!(period_dates(Span::Week, -1, d(2026, 11, 21), false), (d(2026, 11, 9), d(2026, 11, 16)));
        // A Sunday belongs to the week that started the Monday before.
        assert_eq!(period_dates(Span::Week, 0, d(2026, 11, 22), false).0, d(2026, 11, 16));
    }

    #[test]
    fn months_roll_over_years() {
        assert_eq!(period_dates(Span::Month, 0, d(2026, 11, 21), false), (d(2026, 11, 1), d(2026, 12, 1)));
        assert_eq!(period_dates(Span::Month, 1, d(2026, 11, 21), false), (d(2026, 12, 1), d(2027, 1, 1)));
        assert_eq!(period_dates(Span::Month, -11, d(2026, 11, 21), false).0, d(2025, 12, 1));
        assert_eq!(period_dates(Span::Month, -12, d(2026, 1, 5), false).0, d(2025, 1, 1));
        assert_eq!(period_dates(Span::Month, -1, d(2026, 3, 31), false), (d(2026, 2, 1), d(2026, 3, 1)));
    }

    #[test]
    fn local_midnight_is_before_noon() {
        let m = local_midnight(d(2026, 11, 21));
        let n = local_midnight(d(2026, 11, 22));
        assert!(n - m >= 23 * 3600 && n - m <= 25 * 3600);
    }

    #[test]
    fn daemon_state_parses_and_tolerates_garbage() {
        let s = DaemonSnap::parse(r#"{"avail":"ok","state":"paused","paused_until":99,"live":null,"now":5}"#);
        assert!(s.running && s.state == "paused" && s.paused_until == 99 && s.live.is_none());
        let s = DaemonSnap::parse(r#"{"avail":"wayland","state":"running","paused_until":0,"live":{"app":"Code","site":null,"category":"code","start":10},"now":5}"#);
        assert_eq!(s.avail, "wayland");
        assert_eq!(s.live.unwrap().app, "Code");
        assert!(!DaemonSnap::parse("not json").running);
    }

    #[test]
    fn catalog_names_and_special_sites() {
        let mut c = Catalog::default();
        c.sites.insert(1, SiteRow { id: 1, name: sites::PRIVATE.into(), category: None, hidden: false });
        c.sites.insert(2, SiteRow { id: 2, name: "GitHub".into(), category: Some(cats::CODE.into()), hidden: false });
        c.apps.insert(1, AppRow { id: 1, exe: "x".into(), name: "App".into(), category: cats::CODE.into(), manual: false, hidden: false });
        assert_eq!(c.item_name(Item::Site(1), Lang::En), "Private browsing");
        assert_eq!(c.item_name(Item::Site(1), Lang::Ru), "Приватный просмотр");
        assert_eq!(c.item_name(Item::Site(2), Lang::En), "GitHub");
        assert_eq!(c.count_in(cats::CODE), 2);
        assert_eq!(c.categories(Lang::En).len(), cats::BUILTIN.len());
        assert_eq!(c.category(Lang::En, "c9").name, "Other");
    }
}
