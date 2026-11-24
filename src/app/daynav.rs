//! Browsing days on the Day tab: the date maths, the navigation buttons and the date picker modal.

use super::data::local_midnight;
use super::widgets::{self, Icon};
use super::{modal, App, Nav};
use crate::i18n::Lang;
use crate::stats;
use crate::theme::{self, Pal};
use crate::{db::SessionRow, fmt};
use chrono::{Datelike, NaiveDate, TimeZone};
use eframe::egui::{self, Align, Align2, CursorIcon, Layout, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use std::collections::HashMap;
use std::sync::Arc;

// ───────────────────────── date maths ─────────────────────────

/// The day before `d`.
pub fn prev_day(d: NaiveDate) -> NaiveDate {
    d.pred_opt().unwrap_or(d)
}

/// The day after `d`, or `None` when that would be in the future.
pub fn next_day(d: NaiveDate, today: NaiveDate) -> Option<NaiveDate> {
    if d >= today {
        None
    } else {
        d.succ_opt()
    }
}

/// The weeks of a month, each as seven cells starting on Monday or Sunday; cells of other months are `None`.
pub fn month_grid(year: i32, month: u32, sunday_first: bool) -> Vec<[Option<NaiveDate>; 7]> {
    let Some(first) = NaiveDate::from_ymd_opt(year, month, 1) else { return Vec::new() };
    let lead = if sunday_first { first.weekday().num_days_from_sunday() } else { first.weekday().num_days_from_monday() } as usize;
    let mut weeks = Vec::new();
    let mut row = [None; 7];
    let mut col = lead;
    let mut d = first;
    while d.month() == month {
        row[col] = Some(d);
        col += 1;
        if col == 7 {
            weeks.push(row);
            row = [None; 7];
            col = 0;
        }
        d = d.succ_opt().unwrap();
    }
    if col > 0 {
        weeks.push(row);
    }
    weeks
}

/// First and last year the picker offers: from the year of the first recorded data (the current year when there is
/// none) up to the current year.
pub fn year_range(first_data: Option<i64>, today: NaiveDate) -> (i32, i32) {
    let first = first_data
        .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
        .map(|u| u.with_timezone(&chrono::Local).year())
        .unwrap_or(today.year());
    (first.min(today.year()), today.year())
}

/// `(year, month)` moved by `delta` months.
pub fn step_month(year: i32, month: u32, delta: i32) -> (i32, u32) {
    let total = year * 12 + month as i32 - 1 + delta;
    (total.div_euclid(12), total.rem_euclid(12) as u32 + 1)
}

/// Keeps a month inside what the picker offers: not before January of `first_year`, not after the current month.
pub fn clamp_month(year: i32, month: u32, first_year: i32, today: NaiveDate) -> (i32, u32) {
    let v = year * 12 + month as i32 - 1;
    let lo = first_year * 12;
    let hi = today.year() * 12 + today.month() as i32 - 1;
    step_month(0, 1, v.clamp(lo, hi.max(lo)))
}

/// The local days of `[from, to)` that have recorded time, with their totals in seconds.
pub fn days_with_data<Tz: TimeZone>(tz: &Tz, rows: &[SessionRow], from: i64, to: i64) -> Vec<(NaiveDate, i64)> {
    stats::days(tz, rows, from, to)
        .into_iter()
        .filter(|d| d.total > 0)
        .filter_map(|d| tz.timestamp_opt(d.start + 3600, 0).single().map(|dt| (dt.date_naive(), d.total)))
        .collect()
}

// ───────────────────────── state ─────────────────────────

/// The open date picker: the month it shows and the first year it offers.
#[derive(Clone, Copy)]
pub struct Picker {
    pub year: i32,
    pub month: u32,
    pub first_year: i32,
}

type Marks = Arc<HashMap<NaiveDate, i64>>;

#[derive(Default)]
pub struct DayNav {
    pub picker: Option<Picker>,
    marks: Option<((u64, i32, u32, bool), Marks)>,
}

/// What the buttons above the Day tab asked for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DayAct {
    Prev,
    Next,
    Today,
    Pick,
}

impl App {
    /// The day shown on the Day tab. Follows the current day until the user picks another one.
    pub fn selected_day(&self) -> NaiveDate {
        self.today_view.day.unwrap_or(self.today).min(self.today)
    }

    /// Shows `d` on the Day tab (never a future day) and closes the picker.
    pub fn select_day(&mut self, d: NaiveDate) {
        let d = d.min(self.today);
        self.today_view.day = if d == self.today { None } else { Some(d) };
        self.day_nav.picker = None;
    }

    /// Opens the Day tab on `d` (the click-through from the week and month charts).
    pub fn open_day(&mut self, d: NaiveDate) {
        self.select_day(d);
        self.go(Nav::Today);
    }

    pub fn open_picker(&mut self) {
        let sel = self.selected_day();
        let (first_year, _) = year_range(self.db.first_start(), self.today);
        self.day_nav.picker = Some(Picker { year: sel.year(), month: sel.month(), first_year });
    }

    pub fn apply_day_act(&mut self, act: DayAct) {
        let sel = self.selected_day();
        match act {
            DayAct::Prev => self.select_day(prev_day(sel)),
            DayAct::Next => {
                if let Some(d) = next_day(sel, self.today) {
                    self.select_day(d);
                }
            }
            DayAct::Today => self.select_day(self.today),
            DayAct::Pick => self.open_picker(),
        }
    }

    /// Days with recorded time in a month, cached until the data changes.
    fn month_marks(&mut self, year: i32, month: u32) -> Marks {
        let key = (self.edit_version, year, month, self.cfg.hide_system);
        if let Some((k, m)) = &self.day_nav.marks {
            if *k == key {
                return m.clone();
            }
        }
        let (Some(a), Some(b)) = (NaiveDate::from_ymd_opt(year, month, 1), NaiveDate::from_ymd_opt(step_month(year, month, 1).0, step_month(year, month, 1).1, 1)) else {
            return Arc::default();
        };
        let (from, to) = (local_midnight(a), local_midnight(b));
        let rows = self.load_rows(from, to);
        let m: Marks = Arc::new(days_with_data(&chrono::Local, &rows, from, to).into_iter().collect());
        self.day_nav.marks = Some((key, m.clone()));
        m
    }

    /// The date picker modal. Does nothing while it is closed.
    pub(super) fn day_picker(&mut self, ctx: &egui::Context, p: &Pal) {
        let Some(mut pk) = self.day_nav.picker else { return };
        let (lang, today, sel, sunday) = (self.lang, self.today, self.selected_day(), self.sunday_first());
        let t = self.s();
        let marks = self.month_marks(pk.year, pk.month);
        let max = marks.values().copied().max().unwrap_or(1).max(1);
        let mut picked: Option<NaiveDate> = None;
        let mut close = false;
        let cur = (today.year(), today.month());
        let backdrop = modal(ctx, p, "day-picker", self.open_modal_generation(), 336.0, |ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            ui.horizontal(|ui| {
                let can_prev = (pk.year, pk.month) > (pk.first_year, 1);
                let can_next = (pk.year, pk.month) < cur;
                if widgets::icon_button_on(ui, Icon::Left, 32.0, can_prev, p).on_hover_text(t.prev).clicked() {
                    (pk.year, pk.month) = step_month(pk.year, pk.month, -1);
                }
                widgets::combo("pk-year", &pk.year.to_string(), 62.0).show_ui(ui, |ui| {
                    for y in (pk.first_year..=today.year()).rev() {
                        if ui.selectable_label(y == pk.year, RichText::new(y.to_string()).font(theme::reg(13.5))).clicked() {
                            pk.year = y;
                        }
                    }
                });
                widgets::combo("pk-month", fmt::month_full(lang, pk.month as usize - 1), 112.0).show_ui(ui, |ui| {
                    let last = if pk.year == today.year() { today.month() } else { 12 };
                    for m in 1..=last {
                        if ui.selectable_label(m == pk.month, RichText::new(fmt::month_full(lang, m as usize - 1)).font(theme::reg(13.5))).clicked() {
                            pk.month = m;
                        }
                    }
                });
                if widgets::icon_button_on(ui, Icon::Right, 32.0, can_next, p).on_hover_text(t.next).clicked() {
                    (pk.year, pk.month) = step_month(pk.year, pk.month, 1);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::icon_button(ui, Icon::Close, 32.0, p).on_hover_text(t.close).clicked() {
                        close = true;
                    }
                });
            });
            // A year change can leave the month after the current one.
            (pk.year, pk.month) = clamp_month(pk.year, pk.month, pk.first_year, today);
            ui.add_space(4.0);
            if let Some(d) = grid(ui, p, lang, &pk, sunday, today, sel, &marks, max) {
                picked = Some(d);
            }
        });
        if let Some(d) = picked {
            self.select_day(d);
        } else if close || backdrop {
            self.day_nav.picker = None;
        } else {
            self.day_nav.picker = Some(pk);
        }
    }
}

/// The weekday header and the month grid. Returns the clicked day.
#[allow(clippy::too_many_arguments)]
fn grid(ui: &mut Ui, p: &Pal, lang: Lang, pk: &Picker, sunday: bool, today: NaiveDate, sel: NaiveDate, marks: &HashMap<NaiveDate, i64>, max: i64) -> Option<NaiveDate> {
    let weeks = month_grid(pk.year, pk.month, sunday);
    let (cw, ch) = (336.0 / 7.0, 40.0);
    let (rect, _) = ui.allocate_exact_size(vec2(336.0, 24.0 + ch * 6.0), Sense::hover());
    let order: [usize; 7] = if sunday { [6, 0, 1, 2, 3, 4, 5] } else { [0, 1, 2, 3, 4, 5, 6] };
    for (c, wd) in order.iter().enumerate() {
        let x = rect.left() + cw * (c as f32 + 0.5);
        ui.painter().text(pos2(x, rect.top() + 10.0), Align2::CENTER_CENTER, fmt::weekday_short(lang, *wd), theme::reg(12.0), p.muted);
    }
    let mut out = None;
    for (r, week) in weeks.iter().enumerate() {
        for (c, day) in week.iter().enumerate() {
            let Some(d) = day else { continue };
            let cell = egui::Rect::from_min_size(pos2(rect.left() + cw * c as f32, rect.top() + 24.0 + ch * r as f32), vec2(cw, ch)).shrink(2.0);
            let future = *d > today;
            let secs = marks.get(d).copied().unwrap_or(0);
            let selected = *d == sel;
            let resp = ui.interact(cell, ui.scope_id().with(("pk-day", *d)), if future { Sense::hover() } else { Sense::click() });
            let fill = if selected {
                p.accent
            } else if secs > 0 {
                theme::mix(p.raised, p.accent, 0.08 + 0.22 * (secs as f32 / max as f32))
            } else if resp.hovered() && !future {
                p.raised
            } else {
                egui::Color32::TRANSPARENT
            };
            let painter = ui.painter();
            painter.rect_filled(cell, theme::R_CONTROL as f32, fill);
            if *d == today {
                painter.rect_stroke(cell, theme::R_CONTROL as f32, Stroke::new(1.5, p.accent), StrokeKind::Inside);
            }
            let col = if selected {
                p.on_accent
            } else if future {
                theme::alpha(p.muted, 110)
            } else {
                p.text
            };
            painter.text(cell.center() + vec2(0.0, if secs > 0 { -2.0 } else { 0.0 }), Align2::CENTER_CENTER, d.day().to_string(), theme::reg(14.0), col);
            if secs > 0 {
                painter.circle_filled(pos2(cell.center().x, cell.bottom() - 6.0), 2.0, if selected { p.on_accent } else { p.accent });
            }
            if !future {
                let resp = if secs > 0 { resp.on_hover_text(fmt::duration(lang, secs)) } else { resp };
                if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    out = Some(*d);
                }
            }
        }
    }
    out
}

/// The row of buttons next to the Day heading, laid out for a right-to-left container: calendar, "Today", `›`, `‹`.
pub fn nav_bar(ui: &mut Ui, p: &Pal, t: &crate::strings::S, is_today: bool, can_next: bool) -> Option<DayAct> {
    let mut act = None;
    ui.spacing_mut().item_spacing.x = 8.0;
    if widgets::icon_button(ui, Icon::Calendar, 32.0, p).on_hover_text(t.pick_date).clicked() {
        act = Some(DayAct::Pick);
    }
    if !is_today && widgets::btn(ui, t.today_label, theme::Btn::Secondary, p).clicked() {
        act = Some(DayAct::Today);
    }
    if widgets::icon_button_on(ui, Icon::Right, 32.0, can_next, p).on_hover_text(t.next).clicked() {
        act = Some(DayAct::Next);
    }
    if widgets::icon_button(ui, Icon::Left, 32.0, p).on_hover_text(t.prev).clicked() {
        act = Some(DayAct::Prev);
    }
    act
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::SessionRow;
    use chrono::FixedOffset;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn previous_and_next_stop_at_today() {
        let today = d(2026, 11, 24);
        assert_eq!(prev_day(d(2026, 3, 1)), d(2026, 2, 28));
        assert_eq!(prev_day(d(2026, 1, 1)), d(2025, 12, 31));
        assert_eq!(next_day(d(2026, 11, 23), today), Some(today));
        assert_eq!(next_day(today, today), None);
        assert_eq!(next_day(d(2025, 12, 31), today), Some(d(2026, 1, 1)));
        assert_eq!(next_day(d(2027, 1, 1), today), None);
    }

    fn flat(g: &[[Option<NaiveDate>; 7]]) -> Vec<NaiveDate> {
        g.iter().flatten().flatten().copied().collect()
    }

    #[test]
    fn month_grid_monday_first() {
        // November 2026 starts on a Sunday: six empty cells before the 1st in a Monday-first grid.
        let g = month_grid(2026, 11, false);
        assert_eq!(g[0], [None, None, None, None, None, None, Some(d(2026, 11, 1))]);
        assert_eq!(g.len(), 6);
        assert_eq!(g[5][0], Some(d(2026, 11, 30)));
        assert_eq!(flat(&g).len(), 30);
        // Sunday-first: the 1st opens the first row.
        let g = month_grid(2026, 11, true);
        assert_eq!(g[0][0], Some(d(2026, 11, 1)));
        assert_eq!(g.len(), 5);
        assert_eq!(g[4][1], Some(d(2026, 11, 30)));
    }

    #[test]
    fn month_grid_for_a_month_starting_on_every_weekday() {
        // The 1st of these months falls on Monday ... Sunday in turn.
        let months = [(2026, 6), (2026, 9), (2026, 4), (2026, 1), (2026, 5), (2026, 8), (2026, 2)];
        for (i, (y, m)) in months.into_iter().enumerate() {
            let first = d(y, m, 1);
            assert_eq!(first.weekday().num_days_from_monday() as usize, i, "{y}-{m}");
            let g = month_grid(y, m, false);
            assert_eq!(g[0][i], Some(first));
            assert!(g[0][..i].iter().all(Option::is_none));
            let (ny, nm) = step_month(y, m, 1);
            assert_eq!(flat(&g).len() as i64, (d(ny, nm, 1) - first).num_days());
            // Sunday-first puts the 1st in column (i + 1) % 7.
            let g = month_grid(y, m, true);
            assert_eq!(g[0][(i + 1) % 7], Some(first), "{y}-{m} sunday");
        }
    }

    #[test]
    fn leap_years_have_a_29th_of_february() {
        assert_eq!(flat(&month_grid(2028, 2, false)).len(), 29);
        assert_eq!(flat(&month_grid(2026, 2, false)).len(), 28);
        assert_eq!(flat(&month_grid(1900, 2, false)).len(), 28);
        assert_eq!(flat(&month_grid(2000, 2, true)).len(), 29);
        assert_eq!(flat(&month_grid(2028, 2, true)).last(), Some(&d(2028, 2, 29)));
        assert!(month_grid(2026, 13, false).is_empty());
    }

    #[test]
    fn months_step_across_years() {
        assert_eq!(step_month(2026, 1, -1), (2025, 12));
        assert_eq!(step_month(2026, 12, 1), (2027, 1));
        assert_eq!(step_month(2026, 11, -11), (2025, 12));
        assert_eq!(step_month(2026, 5, 0), (2026, 5));
    }

    #[test]
    fn picker_year_range_and_month_limits() {
        let today = d(2026, 11, 24);
        let ts = |y, m, day| local_midnight(d(y, m, day)) + 3600;
        assert_eq!(year_range(Some(ts(2024, 3, 5)), today), (2024, 2026));
        assert_eq!(year_range(None, today), (2026, 2026));
        assert_eq!(year_range(Some(ts(2026, 1, 2)), today), (2026, 2026));
        // Data from the future (a wrong clock) never extends the range past the current year.
        assert_eq!(year_range(Some(ts(2029, 1, 2)), today), (2026, 2026));
        // The month cannot leave the offered range.
        assert_eq!(clamp_month(2026, 12, 2024, today), (2026, 11));
        assert_eq!(clamp_month(2023, 6, 2024, today), (2024, 1));
        assert_eq!(clamp_month(2025, 7, 2024, today), (2025, 7));
        assert_eq!(clamp_month(2026, 3, 2026, d(2026, 1, 10)), (2026, 1));
    }

    #[test]
    fn days_with_data_lists_only_days_with_time() {
        let tz = FixedOffset::east_opt(3 * 3600).unwrap();
        let day = |date: NaiveDate| tz.from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap()).unwrap().timestamp();
        let row = |start: i64, end: i64| SessionRow { start, end, app_id: 1, site_id: None, category: "code".into() };
        let (from, to) = (day(d(2026, 11, 1)), day(d(2026, 12, 1)));
        let rows = vec![
            row(day(d(2026, 11, 3)) + 3600, day(d(2026, 11, 3)) + 7200),
            // Crosses midnight: counts for both days.
            row(day(d(2026, 11, 10)) - 1800, day(d(2026, 11, 10)) + 600),
            // Before the month: ignored.
            row(day(d(2026, 10, 31)) + 100, day(d(2026, 10, 31)) + 900),
        ];
        let got = days_with_data(&tz, &rows, from, to);
        assert_eq!(got, vec![(d(2026, 11, 3), 3600), (d(2026, 11, 9), 1800), (d(2026, 11, 10), 600)]);
        assert!(days_with_data(&tz, &[], from, to).is_empty());
    }
}
