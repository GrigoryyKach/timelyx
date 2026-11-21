//! Limits: a daily limit per category or program. Reaching it shows a notification; nothing is ever blocked.

use super::charts;
use super::widgets::{self, Icon};
use super::{page, today_start, App};
use crate::db::LimitRow;
use crate::stats;
use crate::theme::{self, Btn, Pal};
use crate::{cats, fmt};
use eframe::egui::{self, Align, Layout, Margin, RichText, Sense, Ui, vec2};
use std::collections::HashMap;

/// Minutes offered for a limit.
pub const MINUTES: [u32; 12] = [15, 30, 45, 60, 90, 120, 150, 180, 240, 300, 360, 480];

pub struct LimitsState {
    /// 0 category, 1 program.
    pub kind: usize,
    pub target: Option<String>,
    pub minutes: u32,
    used: Option<((u64, i64), HashMap<(String, String), i64>)>,
}

impl Default for LimitsState {
    fn default() -> Self {
        Self { kind: 0, target: None, minutes: 60, used: None }
    }
}

enum Act {
    Add,
    Remove(String, String),
    Set(String, String, u32),
}

fn minutes_label(lang: crate::i18n::Lang, m: u32) -> String {
    fmt::duration(lang, m as i64 * 60)
}

impl App {
    /// Seconds used today by every limit's target.
    fn limit_usage(&mut self) -> HashMap<(String, String), i64> {
        let from = today_start(self.now);
        let key = (self.version, from);
        if let Some((k, m)) = &self.limits_view.used {
            if *k == key {
                return m.clone();
            }
        }
        let rows = self.db.rows(from, self.now + 1).unwrap_or_default();
        let mut m = HashMap::new();
        for l in &self.cat.limits {
            let used = match l.kind.as_str() {
                "category" => stats::sum_where(&rows, from, self.now + 1, |r| r.category == l.target),
                _ => {
                    let id: i64 = l.target.parse().unwrap_or(-1);
                    stats::sum_where(&rows, from, self.now + 1, |r| r.app_id == id)
                }
            };
            m.insert((l.kind.clone(), l.target.clone()), used);
        }
        self.limits_view.used = Some((key, m.clone()));
        m
    }

    pub(super) fn limits_view(&mut self, ui: &mut Ui, p: &Pal) {
        let t = self.s();
        let lang = self.lang;
        let usage = self.limit_usage();
        let limits = self.cat.limits.clone();
        let cat_infos = self.cat.categories(lang);
        let mut apps: Vec<(i64, String)> = self.cat.apps.values().filter(|a| !a.hidden).map(|a| (a.id, a.name.clone())).collect();
        apps.sort_by_key(|a| a.1.to_lowercase());
        let mut act: Option<Act> = None;
        page(ui, p, t.limits_title, Some(t.limits_hint), |_| {}, |ui| {
            theme::card(p).inner_margin(Margin::symmetric(16, 4)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                if limits.is_empty() {
                    ui.add_space(12.0);
                    theme::hint(ui, t.no_limits, p);
                    ui.add_space(12.0);
                }
                for (i, l) in limits.iter().enumerate() {
                    if i > 0 {
                        theme::hairline(ui, p);
                    }
                    let used = usage.get(&(l.kind.clone(), l.target.clone())).copied().unwrap_or(0);
                    let cap = l.minutes as i64 * 60;
                    let reached = used >= cap;
                    ui.horizontal(|ui| {
                        ui.set_min_height(80.0);
                        ui.vertical(|ui| {
                            ui.set_width((ui.available_width() - 250.0).max(180.0));
                            ui.add_space(10.0);
                            ui.spacing_mut().item_spacing.y = 6.0;
                            ui.horizontal(|ui| {
                                if l.kind == "category" {
                                    let info = self.cat.category(lang, &l.target);
                                    let (r, _) = ui.allocate_exact_size(vec2(14.0, 20.0), Sense::hover());
                                    ui.painter().rect_filled(egui::Rect::from_center_size(r.center(), vec2(12.0, 12.0)), 3.0, widgets::rgb(info.color));
                                }
                                ui.label(RichText::new(self.cat.limit_target_name(lang, l)).font(theme::medium(14.0)).color(p.text));
                                if reached {
                                    ui.label(RichText::new(t.limit_reached).font(theme::reg(12.0)).color(p.warning));
                                }
                            });
                            let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), 6.0), Sense::hover());
                            charts::bar(ui.painter(), bar, (used as f32 / cap.max(1) as f32).min(1.0), if reached { p.warning } else { p.accent }, p);
                            theme::meta(ui, &t.used_today.replace("{used}", &fmt::duration(lang, used)).replace("{limit}", &fmt::duration(lang, cap)), p);
                            ui.add_space(8.0);
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::icon_button(ui, Icon::Trash, 32.0, p).on_hover_text(t.remove).clicked() {
                                act = Some(Act::Remove(l.kind.clone(), l.target.clone()));
                            }
                            widgets::combo(("limit", &l.kind, &l.target), &minutes_label(lang, l.minutes), 130.0).show_ui(ui, |ui| {
                                for m in MINUTES {
                                    if ui.selectable_label(m == l.minutes, RichText::new(minutes_label(lang, m)).font(theme::reg(13.5))).clicked() {
                                        act = Some(Act::Set(l.kind.clone(), l.target.clone(), m));
                                    }
                                }
                            });
                            ui.label(RichText::new(t.limit_daily).font(theme::reg(13.0)).color(p.muted));
                        });
                    });
                }
            });
            ui.add_space(24.0);

            theme::section(ui, t.add_limit, p);
            ui.add_space(8.0);
            theme::card(p).inner_margin(Margin::same(16)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
                    if let Some(i) = widgets::segmented(ui, "limit-kind", &[t.limit_kind_category, t.limit_kind_app], self.limits_view.kind, p) {
                        self.limits_view.kind = i;
                        self.limits_view.target = None;
                    }
                    let current = self.limits_view.target.clone();
                    let label = match (self.limits_view.kind, &current) {
                        (0, Some(id)) => self.cat.category(lang, id).name,
                        (1, Some(id)) => id.parse::<i64>().ok().and_then(|i| self.cat.apps.get(&i)).map(|a| a.name.clone()).unwrap_or_default(),
                        _ => t.pick_target.to_string(),
                    };
                    widgets::combo("limit-target", &label, 230.0).show_ui(ui, |ui| {
                        if self.limits_view.kind == 0 {
                            for c in &cat_infos {
                                if ui.selectable_label(current.as_deref() == Some(c.id.as_str()), RichText::new(&c.name).font(theme::reg(13.5))).clicked() {
                                    self.limits_view.target = Some(c.id.clone());
                                }
                            }
                        } else {
                            for (id, name) in &apps {
                                if ui.selectable_label(current.as_deref() == Some(id.to_string().as_str()), RichText::new(name).font(theme::reg(13.5))).clicked() {
                                    self.limits_view.target = Some(id.to_string());
                                }
                            }
                        }
                    });
                    widgets::combo("limit-minutes", &minutes_label(lang, self.limits_view.minutes), 130.0).show_ui(ui, |ui| {
                        for m in MINUTES {
                            if ui.selectable_label(m == self.limits_view.minutes, RichText::new(minutes_label(lang, m)).font(theme::reg(13.5))).clicked() {
                                self.limits_view.minutes = m;
                            }
                        }
                    });
                    let ok = self.limits_view.target.is_some();
                    if widgets::btn_enabled(ui, t.add, Btn::Primary, ok, p).clicked() && ok {
                        act = Some(Act::Add);
                    }
                });
            });
        });
        match act {
            Some(Act::Add) => {
                if let Some(target) = self.limits_view.target.take() {
                    let kind = if self.limits_view.kind == 0 { "category" } else { "app" };
                    let _ = self.db.set_limit(kind, &target, self.limits_view.minutes);
                    self.data_changed();
                    self.tell_tracker("reload");
                }
            }
            Some(Act::Remove(kind, target)) => {
                let _ = self.db.remove_limit(&kind, &target);
                self.data_changed();
            }
            Some(Act::Set(kind, target, m)) => {
                let _ = self.db.set_limit(&kind, &target, m);
                self.data_changed();
            }
            None => {}
        }
        let _: Option<LimitRow> = None;
        let _ = cats::OTHER;
    }
}
