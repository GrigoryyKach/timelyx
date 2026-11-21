//! The Today screen: active time, what is running now, the day strip, categories and the top programs and sites.

use super::charts::{self, Seg};
use super::data::Catalog;
use super::widgets::{self, Icon};
use super::{page, ranked, today_start, App};
use crate::db::SessionRow;
use crate::engine::next_midnight;
use crate::stats::{self, Item, ItemStat};
use crate::theme::{self, Btn, Pal};
use crate::{fmt, platform};
use eframe::egui::{self, Align, Align2, CursorIcon, Layout, Margin, RichText, Sense, Ui, pos2, vec2};
use std::sync::Arc;

pub struct TodayModel {
    pub from: i64,
    pub to: i64,
    pub total: i64,
    pub yesterday_same: i64,
    pub sessions: usize,
    pub by_cat: Vec<(String, i64)>,
    pub items: Vec<ItemStat>,
    pub segs: Vec<(SessionRow, String)>,
}

#[derive(Default)]
pub struct TodayState {
    model: Option<((u64, i64, bool), Arc<TodayModel>)>,
}

/// A titled card: a section title over a card. `min_h` keeps neighbouring cards the same height.
pub fn titled_card(ui: &mut Ui, p: &Pal, title: &str, min_h: f32, add: impl FnOnce(&mut Ui)) {
    theme::section(ui, title, p);
    ui.add_space(8.0);
    theme::card(p).inner_margin(Margin::same(16)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.set_min_height(min_h);
        add(ui);
    });
}

/// Draws `left` and `right` side by side with a gap; `ratio` is the left share of the width.
pub fn two_columns(ui: &mut Ui, ratio: f32, left: impl FnOnce(&mut Ui), right: impl FnOnce(&mut Ui)) {
    let gap = 16.0;
    let w = ui.available_width();
    let lw = ((w - gap) * ratio).floor();
    let rw = w - gap - lw;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        ui.allocate_ui_with_layout(vec2(lw, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(lw);
            left(ui);
        });
        ui.allocate_ui_with_layout(vec2(rw, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(rw);
            right(ui);
        });
    });
}

impl App {
    fn today_model(&mut self) -> Arc<TodayModel> {
        let from = today_start(self.now);
        let key = (self.version, from, self.cfg.hide_system);
        if let Some((k, m)) = &self.today_view.model {
            if *k == key {
                return m.clone();
            }
        }
        let to = next_midnight(&chrono::Local, from);
        let y_from = crate::engine::day_start(&chrono::Local, from - 3600);
        let rows = self.load_rows(y_from, to);
        let elapsed = (self.now - from).max(0);
        let items = ranked(self, &rows, from, to);
        let model = TodayModel {
            from,
            to,
            total: stats::total(&rows, from, to),
            yesterday_same: stats::total(&rows, y_from, (y_from + elapsed).min(from)),
            sessions: rows.iter().filter(|r| r.end > from && r.start < to).count(),
            by_cat: stats::by_category(&rows, from, to),
            items,
            segs: rows
                .into_iter()
                .filter(|r| r.end > from && r.start < to)
                .map(|r| {
                    let name = self.cat.item_name(Item::of(&r), self.lang);
                    (r, name)
                })
                .collect(),
        };
        let m = Arc::new(model);
        self.today_view.model = Some((key, m.clone()));
        m
    }

    pub(super) fn today_view(&mut self, ui: &mut Ui, p: &Pal) {
        let t = self.s();
        let m = self.today_model();
        let subtitle = fmt::date_with_weekday(self.lang, self.now);
        let mut start_tracker = false;
        let mut pause: Option<&'static str> = None;
        page(ui, p, t.nav_today, Some(&subtitle), |_| {}, |ui| {
            // Problems first: no tracker, no way to track.
            if self.snap.avail == "wayland" {
                notice(ui, p, t.wayland_title, t.wayland_body, p.warning);
                ui.add_space(16.0);
            } else if self.snap.avail.starts_with("error") {
                notice(ui, p, t.tracker_error, &self.snap.avail, p.negative);
                ui.add_space(16.0);
            } else if !self.snap.running && !self.demo {
                theme::card(p).inner_margin(Margin::same(16)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.label(RichText::new(t.tracker_off).font(theme::medium(15.0)).color(p.text));
                            theme::hint(ui, t.tracker_off_hint, p);
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::btn(ui, t.start_tracking, Btn::Primary, p).clicked() {
                                start_tracker = true;
                            }
                        });
                    });
                });
                ui.add_space(16.0);
            }

            two_columns(
                ui,
                0.5,
                |ui| {
                    titled_card(ui, p, t.active_time, 96.0, |ui| {
                        ui.spacing_mut().item_spacing.y = 6.0;
                        ui.label(RichText::new(fmt::duration(self.lang, m.total)).font(theme::mono(32.0)).color(p.text));
                        let change = stats::change_percent(m.total, m.yesterday_same);
                        let mut line = match change {
                            Some(c) => format!("{}{}% {}", if c > 0 { "+" } else { "" }, c, yesterday_phrase(self)),
                            None => String::new(),
                        };
                        if m.sessions > 0 {
                            if !line.is_empty() {
                                line.push_str("  ·  ");
                            }
                            line.push_str(&t.sessions_n.replace("{n}", &m.sessions.to_string()));
                        }
                        theme::hint(ui, &line, p);
                    });
                },
                |ui| {
                    titled_card(ui, p, t.now_in, 96.0, |ui| {
                        pause = self.now_card(ui, p);
                    });
                },
            );
            ui.add_space(24.0);

            if m.total == 0 && m.segs.is_empty() {
                ui.add_space(24.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(t.nothing_today).font(theme::medium(18.0)).color(p.text));
                    ui.add_space(4.0);
                    theme::hint(ui, t.nothing_today_hint, p);
                });
                return;
            }

            titled_card(ui, p, t.day_timeline, 0.0, |ui| {
                let segs: Vec<Seg> = m
                    .segs
                    .iter()
                    .map(|(r, name)| Seg { start: r.start.max(m.from), end: r.end.min(m.to), color: widgets::rgb(self.cat.category(self.lang, &r.category).color), name: name.clone() })
                    .collect();
                charts::timeline(ui, p, self.lang, self.h24(), m.from, m.to, &segs, Some(self.now));
            });
            ui.add_space(24.0);

            two_columns(
                ui,
                0.5,
                |ui| {
                    titled_card(ui, p, t.by_category, 232.0, |ui| self.category_breakdown(ui, p, &m));
                },
                |ui| {
                    titled_card(ui, p, t.top_items, 232.0, |ui| self.top_items(ui, p, &m));
                },
            );
        });
        if start_tracker {
            platform::spawn_background();
            self.refresh_soon();
        }
        if let Some(msg) = pause {
            self.tell_tracker(msg);
            self.refresh_soon();
        }
    }

    /// The "Now" card. Returns a request for the tracker (pause or resume) when a button was pressed.
    fn now_card(&self, ui: &mut Ui, p: &Pal) -> Option<&'static str> {
        let t = self.s();
        let mut out = None;
        ui.spacing_mut().item_spacing.y = 6.0;
        let snap = &self.snap;
        if !snap.running && !self.demo {
            ui.label(RichText::new(t.tracker_stopped).font(theme::medium(16.0)).color(p.text2));
            return None;
        }
        match (snap.state.as_str(), &snap.live) {
            ("paused", _) => {
                ui.label(RichText::new(t.now_paused).font(theme::medium(16.0)).color(p.text));
                if snap.paused_until > 0 && snap.paused_until < i64::MAX / 2 {
                    theme::hint(ui, &t.now_paused_until.replace("{time}", &fmt::clock(snap.paused_until, self.h24())), p);
                }
                if widgets::btn(ui, t.tray_resume, Btn::Secondary, p).clicked() {
                    out = Some("resume");
                }
            }
            ("idle", _) => {
                ui.label(RichText::new(t.now_idle).font(theme::medium(16.0)).color(p.text2));
            }
            ("locked", _) => {
                ui.label(RichText::new(t.now_locked).font(theme::medium(16.0)).color(p.text2));
            }
            (_, Some(live)) => {
                let cat = self.cat.category(self.lang, &live.category);
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(14.0, 24.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 5.0, widgets::rgb(cat.color));
                    let name = match &live.site {
                        Some(s) => super::data::site_label(self.lang, s),
                        None => live.app.clone(),
                    };
                    ui.label(RichText::new(name).font(theme::medium(16.0)).color(p.text));
                });
                let mins = ((self.now - live.start).max(0) + 30) / 60;
                theme::hint(ui, &format!("{}  ·  {}", cat.name, fmt::duration(self.lang, mins * 60)), p);
                if widgets::btn(ui, t.pause_15, Btn::Secondary, p).clicked() {
                    out = Some("pause 900");
                }
            }
            _ => {
                ui.label(RichText::new(t.now_nothing).font(theme::medium(16.0)).color(p.text2));
            }
        }
        out
    }

    fn category_breakdown(&self, ui: &mut Ui, p: &Pal, m: &TodayModel) {
        let total: i64 = m.by_cat.iter().map(|c| c.1).sum();
        let slices: Vec<(egui::Color32, f64)> = m.by_cat.iter().map(|(c, s)| (widgets::rgb(self.cat.category(self.lang, c).color), *s as f64)).collect();
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            ui.vertical(|ui| {
                charts::donut(ui, p, 150.0, 20.0, &slices, &fmt::duration(self.lang, total), self.s().total);
            });
            ui.vertical(|ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 4.0;
                for (c, secs) in m.by_cat.iter().take(8) {
                    let info = self.cat.category(self.lang, c);
                    let share = if total > 0 { Some(*secs as f64 / total as f64) } else { None };
                    charts::legend_row(ui, p, self.lang, widgets::rgb(info.color), &info.name, *secs, share);
                }
            });
        });
    }

    fn top_items(&self, ui: &mut Ui, p: &Pal, m: &TodayModel) {
        item_rows(ui, p, self.lang, &self.cat, &m.items, 10, m.total);
    }
}

/// "vs yesterday" in the current language.
fn yesterday_phrase(app: &App) -> &'static str {
    app.lang.pick("vs yesterday", "к вчера", "до вчора")
}

fn notice(ui: &mut Ui, p: &Pal, title: &str, body: &str, accent: egui::Color32) {
    theme::card(p).inner_margin(Margin::same(16)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_top(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(10.0, 22.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 4.0, accent);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                ui.label(RichText::new(title).font(theme::medium(15.0)).color(p.text));
                ui.add(egui::Label::new(RichText::new(body).font(theme::reg(13.0)).color(p.text2)).wrap());
            });
        });
    });
}

/// Rows with a colour dot, the name, a share bar and the time. Used for "Top apps and sites".
pub fn item_rows(ui: &mut Ui, p: &Pal, lang: crate::i18n::Lang, cat: &Catalog, items: &[ItemStat], limit: usize, total: i64) {
    let max = items.iter().map(|i| i.secs).max().unwrap_or(1).max(1);
    ui.spacing_mut().item_spacing.y = 2.0;
    for it in items.iter().take(limit) {
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::hover());
        let info = cat.category(lang, &it.category);
        ui.painter().rect_filled(rect, theme::R_CONTROL as f32, widgets::hover_fill(ui, resp.id, resp.hovered(), p));
        ui.painter().circle_filled(pos2(rect.left() + 12.0, rect.center().y), 4.0, widgets::rgb(info.color));
        let time = fmt::duration(lang, it.secs);
        let time_g = ui.painter().layout_no_wrap(time, theme::mono(12.0), p.text);
        let right = rect.right() - 10.0;
        ui.painter().galley(pos2(right - time_g.size().x, rect.center().y - time_g.size().y / 2.0), time_g.clone(), p.text);
        // The bars start at one fixed distance from the right edge, whatever the time text says.
        let bar_w = 72.0;
        let bar = egui::Rect::from_center_size(pos2(right - 96.0 - 12.0 - bar_w / 2.0, rect.center().y), vec2(bar_w, 6.0));
        charts::bar(ui.painter(), bar, it.secs as f32 / max as f32, widgets::rgb(info.color), p);
        let name = cat.item_name(it.item, lang);
        let name_w = (bar.left() - 12.0) - (rect.left() + 26.0);
        let name = widgets::fit_text(ui.painter(), &name, &theme::reg(13.5), name_w);
        ui.painter().text(pos2(rect.left() + 26.0, rect.center().y), Align2::LEFT_CENTER, name, theme::reg(13.5), p.text);
        if total > 0 {
            let _ = resp.on_hover_cursor(CursorIcon::Default);
        }
    }
    let _ = Icon::Check;
}
