//! Text formatting for durations, clock times and dates in the three interface languages.

use crate::i18n::Lang;
use chrono::{Datelike, Local, TimeZone, Timelike};

/// "3 h 25 min" / "3 ч 25 мин" / "3 год 25 хв". Under a minute: "45 s". Minutes are rounded to the nearest one.
pub fn duration(lang: Lang, secs: i64) -> String {
    let secs = secs.max(0);
    let (h, m, s) = (lang.pick("h", "ч", "год"), lang.pick("min", "мин", "хв"), lang.pick("s", "с", "с"));
    if secs == 0 {
        return format!("0 {m}");
    }
    if secs < 60 {
        return format!("{secs} {s}");
    }
    let total_min = (secs + 30) / 60;
    let (hh, mm) = (total_min / 60, total_min % 60);
    match (hh, mm) {
        (0, mm) => format!("{mm} {m}"),
        (hh, 0) => format!("{hh} {h}"),
        (hh, mm) => format!("{hh} {h} {mm} {m}"),
    }
}

/// A compact duration for axis labels and small cells: "3:25" is not used; "3 h" or "25 min" only.
pub fn duration_short(lang: Lang, secs: i64) -> String {
    let secs = secs.max(0);
    let (h, m) = (lang.pick("h", "ч", "год"), lang.pick("min", "мин", "хв"));
    if secs >= 3600 {
        let tenth = (secs as f64 / 360.0).round() / 10.0;
        if (tenth - tenth.round()).abs() < 0.05 { format!("{} {h}", tenth.round() as i64) } else { format!("{tenth:.1} {h}") }
    } else {
        format!("{} {m}", (secs + 30) / 60)
    }
}

/// Clock time of a Unix timestamp in local time: "14:05" or "2:05 PM".
pub fn clock(ts: i64, h24: bool) -> String {
    let Some(d) = Local.timestamp_opt(ts, 0).single() else { return String::new() };
    clock_hm(d.hour(), d.minute(), h24)
}

pub fn clock_hm(h: u32, m: u32, h24: bool) -> String {
    if h24 {
        format!("{h:02}:{m:02}")
    } else {
        let (h12, ap) = match h {
            0 => (12, "AM"),
            1..=11 => (h, "AM"),
            12 => (12, "PM"),
            _ => (h - 12, "PM"),
        };
        format!("{h12}:{m:02} {ap}")
    }
}

const MONTHS_EN: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const MONTHS_RU: [&str; 12] = ["янв", "фев", "мар", "апр", "мая", "июн", "июл", "авг", "сен", "окт", "ноя", "дек"];
const MONTHS_UK: [&str; 12] = ["січ", "лют", "бер", "кві", "тра", "чер", "лип", "сер", "вер", "жов", "лис", "гру"];
const DAYS_EN: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const DAYS_RU: [&str; 7] = ["Пн", "Вт", "Ср", "Чт", "Пт", "Сб", "Вс"];
const DAYS_UK: [&str; 7] = ["Пн", "Вт", "Ср", "Чт", "Пт", "Сб", "Нд"];

/// Weekday name, Monday = 0.
pub fn weekday_short(lang: Lang, i: usize) -> &'static str {
    let i = i % 7;
    lang.pick(DAYS_EN[i], DAYS_RU[i], DAYS_UK[i])
}

pub fn month_short(lang: Lang, month0: usize) -> &'static str {
    let i = month0 % 12;
    lang.pick(MONTHS_EN[i], MONTHS_RU[i], MONTHS_UK[i])
}

/// "20 Nov" / "20 ноя" / "20 лис".
pub fn date_short(lang: Lang, ts: i64) -> String {
    let Some(d) = Local.timestamp_opt(ts, 0).single() else { return String::new() };
    format!("{} {}", d.day(), month_short(lang, d.month0() as usize))
}

/// "Fri, 20 Nov".
pub fn date_with_weekday(lang: Lang, ts: i64) -> String {
    let Some(d) = Local.timestamp_opt(ts, 0).single() else { return String::new() };
    format!("{}, {} {}", weekday_short(lang, d.weekday().num_days_from_monday() as usize), d.day(), month_short(lang, d.month0() as usize))
}

/// "20 Nov, 14:05" for "last used" columns.
pub fn date_time(lang: Lang, ts: i64, h24: bool) -> String {
    format!("{}, {}", date_short(lang, ts), clock(ts, h24))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_in_three_languages() {
        assert_eq!(duration(Lang::En, 3 * 3600 + 25 * 60), "3 h 25 min");
        assert_eq!(duration(Lang::Ru, 3 * 3600 + 25 * 60), "3 ч 25 мин");
        assert_eq!(duration(Lang::Uk, 3 * 3600 + 25 * 60), "3 год 25 хв");
        assert_eq!(duration(Lang::En, 3600), "1 h");
        assert_eq!(duration(Lang::En, 25 * 60), "25 min");
        assert_eq!(duration(Lang::En, 45), "45 s");
        assert_eq!(duration(Lang::En, 0), "0 min");
        // Rounded to the nearest minute: 59 min 40 s is one hour.
        assert_eq!(duration(Lang::En, 59 * 60 + 40), "1 h");
        assert_eq!(duration(Lang::En, -5), "0 min");
    }

    #[test]
    fn short_durations() {
        assert_eq!(duration_short(Lang::En, 3 * 3600), "3 h");
        assert_eq!(duration_short(Lang::En, 90 * 60), "1.5 h");
        assert_eq!(duration_short(Lang::Ru, 20 * 60), "20 мин");
    }

    #[test]
    fn clock_formats() {
        assert_eq!(clock_hm(9, 5, true), "09:05");
        assert_eq!(clock_hm(0, 7, false), "12:07 AM");
        assert_eq!(clock_hm(12, 0, false), "12:00 PM");
        assert_eq!(clock_hm(15, 30, false), "3:30 PM");
    }

    #[test]
    fn names() {
        assert_eq!(weekday_short(Lang::En, 0), "Mon");
        assert_eq!(weekday_short(Lang::Uk, 6), "Нд");
        assert_eq!(month_short(Lang::Ru, 10), "ноя");
    }
}
