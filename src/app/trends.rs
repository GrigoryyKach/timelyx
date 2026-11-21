//! Week and month: totals, stacked bars per day, the weekday-by-hour heatmap and the longest focus run.

use super::charts::{self, BarDay};
use super::data::{local_midnight, period_dates, Span};
use super::today::titled_card;
use super::widgets::{self, Icon};
use super::{page, App};
use crate::engine::{day_start, weekday_index};
use crate::stats::{self, Focus};
use crate::theme::{self, Btn, Pal};
use crate::fmt;
use chrono::Datelike;
use eframe::egui::{self, Layout, Margin, RichText, Sense, Ui, vec2};
use std::sync::Arc;

/// Gaps up to this long between two sessions of one category still count as one focus run.
const FOCUS_GAP: i64 = 120;

pub struct TrendsModel {
    pub label: String,
    pub bars: Vec<BarDay>,
    pub total: i64,
    pub prev_total: i64,
    pub days_counted: i64,
    pub by_cat: Vec<(String, i64)>,
    pub heat: [[i64; 24]; 7],
    pub focus: Option<Focus>,
    pub busiest: Option<(i64, i64)>,
}

pub struct TrendsState {
    pub span: Span,
    pub offset: i32,
    model: Option<((u64, Span, i32, i64, bool), Arc<TrendsModel>)>,
}

impl Default for TrendsState {
    fn default() -> Self {
        Self { span: Span::Week, offset: 0, model: None }
    }
}

impl App {
    fn trends_model(&mut self) -> Arc<TrendsModel> {
        let (span, offset) = (self.trends_view.span, self.trends_view.offset);
        let key = (self.version, span, offset, day_start(&chrono::Local, self.now), self.cfg.hide_system);
        if let Some((k, m)) = &self.trends_view.model {
            if *k == key {
                return m.clone();
            }
        }
        let sunday = self.sunday_first();
        let (d0, d1) = period_dates(span, offset, self.today, sunday);
        let (p0, p1) = period_dates(span, offset - 1, self.today, sunday);
        let (from, to, prev_from, prev_to) = (local_midnight(d0), local_midnight(d1), local_midnight(p0), local_midnight(p1));
        let rows = self.load_rows(prev_from, to);
        let days = stats::days(&chrono::Local, &rows, from, to);
        let lang = self.lang;
        let month_view = span == Span::Month;
        let today_start = day_start(&chrono::Local, self.now);
        let bars: Vec<BarDay> = days
            .iter()
            .map(|d| {
                let dt = chrono::DateTime::from_timestamp(d.start, 0).map(|u| u.with_timezone(&chrono::Local));
                let dom = dt.map(|x| x.day()).unwrap_or(1);
                let label = if month_view {
                    if dom == 1 || dom % 5 == 0 { dom.to_string() } else { String::new() }
                } else {
                    fmt::weekday_short(lang, weekday_index(&chrono::Local, d.start + 3600)).to_string()
                };
                BarDay { label, title: fmt::date_with_weekday(lang, d.start + 3600), by_cat: d.by_cat.clone(), total: d.total, highlight: d.start == today_start }
            })
            .collect();
        let total = stats::total(&rows, from, to);
        let prev_total = stats::total(&rows, prev_from, prev_to);
        // Days that have started count for the average; a past period counts all of its days.
        let days_counted = days.iter().filter(|d| d.start <= self.now).count().max(1) as i64;
        let busiest = days.iter().filter(|d| d.total > 0).max_by_key(|d| d.total).map(|d| (d.start, d.total));
        let model = TrendsModel {
            label: period_label(lang, span, d0, d1),
            bars,
            total,
            prev_total,
            days_counted,
            by_cat: stats::by_category(&rows, from, to),
            heat: stats::heatmap(&chrono::Local, &rows, from, to),
            focus: stats::longest_focus(&rows, from, to, FOCUS_GAP),
            busiest,
        };
        let m = Arc::new(model);
        self.trends_view.model = Some((key, m.clone()));
        m
    }

    pub(super) fn trends_view(&mut self, ui: &mut Ui, p: &Pal) {
        let t = self.s();
        let m = self.trends_model();
        let mut new_span = None;
        let mut step = 0;
        let mut current = false;
        page(ui, p, t.nav_trends, None, |_| {}, |ui| {
            // Toolbar: week or month, the period, arrows.
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let sel = if self.trends_view.span == Span::Week { 0 } else { 1 };
                if let Some(i) = widgets::segmented(ui, "span", &[t.period_week, t.period_month], sel, p) {
                    new_span = Some(if i == 0 { Span::Week } else { Span::Month });
                }
                ui.add_space(8.0);
                if widgets::icon_button(ui, Icon::Left, 32.0, p).on_hover_text(t.prev).clicked() {
                    step = -1;
                }
                ui.label(RichText::new(&m.label).font(theme::medium(15.0)).color(p.text));
                let at_present = self.trends_view.offset >= 0;
                if !at_present && widgets::icon_button(ui, Icon::Right, 32.0, p).on_hover_text(t.next).clicked() {
                    step = 1;
                }
                if !at_present && widgets::btn(ui, t.this_period, Btn::Secondary, p).clicked() {
                    current = true;
                }
            });
            ui.add_space(16.0);

            // Four figures.
            ui.spacing_mut().item_spacing.x = 16.0;
            ui.columns(4, |cols| {
                let prev_name = if self.trends_view.span == Span::Week { t.vs_prev_week } else { t.vs_prev_month };
                stat_card(&mut cols[0], p, t.total, &fmt::duration(self.lang, m.total), &match stats::change_percent(m.total, m.prev_total) {
                    Some(c) => format!("{}{}% {}", if c > 0 { "+" } else { "" }, c, prev_name),
                    None => t.no_comparison.to_string(),
                });
                let avg = m.total / m.days_counted;
                stat_card(&mut cols[1], p, t.avg_per_day, &fmt::duration(self.lang, avg), &match stats::change_percent(avg, m.prev_total / self.prev_days(&m)) {
                    Some(c) => format!("{}{}% {}", if c > 0 { "+" } else { "" }, c, prev_name),
                    None => t.no_comparison.to_string(),
                });
                match &m.focus {
                    Some(f) => {
                        let cat = self.cat.category(self.lang, &f.category);
                        stat_card(&mut cols[2], p, t.longest_focus, &fmt::duration(self.lang, f.secs()), &format!("{} · {} {}", cat.name, fmt::date_short(self.lang, f.start), fmt::clock(f.start, self.h24())));
                    }
                    None => stat_card(&mut cols[2], p, t.longest_focus, "–", t.focus_none),
                }
                match m.busiest {
                    Some((d, s)) => stat_card(&mut cols[3], p, t.busiest_day, &fmt::duration(self.lang, s), &fmt::date_with_weekday(self.lang, d + 3600)),
                    None => stat_card(&mut cols[3], p, t.busiest_day, "–", t.focus_none),
                }
            });
            ui.add_space(24.0);

            titled_card(ui, p, t.by_day, 0.0, |ui| {
                let color_of = |c: &str| widgets::rgb(self.cat.category(self.lang, c).color);
                let name_of = |c: &str| self.cat.category(self.lang, c).name;
                charts::stacked_bars(ui, p, self.lang, 240.0, &m.bars, &color_of, &name_of);
                ui.add_space(12.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(18.0, 6.0);
                    for (c, secs) in m.by_cat.iter().take(9) {
                        let info = self.cat.category(self.lang, c);
                        charts::legend_chip(ui, p, self.lang, widgets::rgb(info.color), &info.name, *secs);
                    }
                });
            });
            ui.add_space(24.0);

            titled_card(ui, p, t.heatmap_title, 0.0, |ui| {
                let order: [usize; 7] = if self.sunday_first() { [6, 0, 1, 2, 3, 4, 5] } else { [0, 1, 2, 3, 4, 5, 6] };
                charts::heatmap(ui, p, self.lang, self.h24(), &m.heat, order);
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    theme::hint(ui, t.heatmap_hint, p);
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(t.more).font(theme::reg(12.0)).color(p.muted));
                        for k in (0..5).rev() {
                            let (r, _) = ui.allocate_exact_size(vec2(16.0, 12.0), Sense::hover());
                            let col = if k == 0 { p.raised } else { theme::mix(p.raised_hi, p.accent, (k as f32 / 4.0).sqrt()) };
                            ui.painter().rect_filled(r, 3.0, col);
                        }
                        ui.label(RichText::new(t.less).font(theme::reg(12.0)).color(p.muted));
                    });
                });
            });
        });
        if let Some(s) = new_span {
            if s != self.trends_view.span {
                self.trends_view.span = s;
                self.trends_view.offset = 0;
            }
        }
        self.trends_view.offset = (self.trends_view.offset + step).min(0);
        if current {
            self.trends_view.offset = 0;
        }
    }

    /// Number of days of the previous period, for its average.
    fn prev_days(&self, m: &TrendsModel) -> i64 {
        let span = self.trends_view.span;
        let (p0, p1) = period_dates(span, self.trends_view.offset - 1, self.today, self.sunday_first());
        let _ = m;
        (p1 - p0).num_days().max(1)
    }
}

fn period_label(lang: crate::i18n::Lang, span: Span, d0: chrono::NaiveDate, d1: chrono::NaiveDate) -> String {
    let last = d1 - chrono::Duration::days(1);
    match span {
        Span::Week => format!("{} {} – {} {}", d0.day(), fmt::month_short(lang, d0.month0() as usize), last.day(), fmt::month_short(lang, last.month0() as usize)),
        Span::Month => format!("{} {}", fmt::month_short(lang, d0.month0() as usize), d0.year()),
    }
}

fn stat_card(ui: &mut Ui, p: &Pal, title: &str, value: &str, meta: &str) {
    theme::card(p).inner_margin(Margin::same(16)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 4.0;
        theme::hint(ui, title, p);
        ui.label(RichText::new(value).font(theme::mono(22.0)).color(p.text));
        ui.add(egui::Label::new(RichText::new(meta).font(theme::reg(12.0)).color(p.muted)).truncate());
    });
}
