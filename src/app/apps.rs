//! Apps and sites: a sortable table with the category editable in place, and a daily chart for the selected row.

use super::charts::{self, BarDay};
use super::data::Catalog;
use super::today::titled_card;
use super::widgets::{self, Icon};
use super::{page, App};
use crate::engine::{day_start, weekday_index};
use crate::i18n::Lang;
use crate::stats::{self, Item, ItemStat};
use crate::theme::{self, Btn, Pal};
use crate::{cats, fmt};
use eframe::egui::{self, Align, Align2, CursorIcon, Layout, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortCol {
    Name,
    Category,
    Time,
    Share,
    Sessions,
    Last,
}

pub struct AppsModel {
    pub total: i64,
    pub from: i64,
    pub to: i64,
    pub items: Vec<ItemStat>,
    pub rows: Vec<crate::db::SessionRow>,
}

pub struct AppsState {
    /// 0 today, 1 seven days, 2 thirty days, 3 all time.
    pub range: usize,
    pub sort: SortCol,
    pub asc: bool,
    pub search: String,
    pub show_hidden: bool,
    pub selected: Option<Item>,
    pub rename: Option<(i64, String)>,
    model: Option<((u64, usize, bool, bool, i64), Arc<AppsModel>)>,
}

impl Default for AppsState {
    fn default() -> Self {
        Self { range: 1, sort: SortCol::Time, asc: false, search: String::new(), show_hidden: false, selected: None, rename: None, model: None }
    }
}

enum Act {
    Sort(SortCol),
    Select(Option<Item>),
    SetCategory(Item, Option<String>),
    SetHidden(Item, bool),
    StartRename(i64, String),
    SaveRename(i64, String),
    CancelRename,
}

/// Sorts a copy of the items for display.
fn sorted(items: &[ItemStat], cat: &Catalog, lang: Lang, col: SortCol, asc: bool) -> Vec<ItemStat> {
    let mut v = items.to_vec();
    v.sort_by(|a, b| {
        let o = match col {
            SortCol::Name => cat.item_name(a.item, lang).to_lowercase().cmp(&cat.item_name(b.item, lang).to_lowercase()),
            SortCol::Category => cat.category(lang, &a.category).name.to_lowercase().cmp(&cat.category(lang, &b.category).name.to_lowercase()),
            SortCol::Time | SortCol::Share => a.secs.cmp(&b.secs),
            SortCol::Sessions => a.sessions.cmp(&b.sessions),
            SortCol::Last => a.last_used.cmp(&b.last_used),
        };
        let o = o.then_with(|| a.item.cmp(&b.item));
        if asc { o } else { o.reverse() }
    });
    v
}

impl App {
    fn apps_model(&mut self) -> Arc<AppsModel> {
        let st = &self.apps_view;
        let today = day_start(&chrono::Local, self.now);
        let key = (self.version, st.range, st.show_hidden, self.cfg.hide_system, today);
        if let Some((k, m)) = &st.model {
            if *k == key {
                return m.clone();
            }
        }
        let to = self.now + 1;
        let from = match st.range {
            0 => today,
            1 => back_days(today, 6),
            2 => back_days(today, 29),
            _ => self.db.first_start().unwrap_or(0),
        };
        let mut rows = if st.show_hidden { self.db.rows_with_hidden(from, to).unwrap_or_default() } else { self.db.rows(from, to).unwrap_or_default() };
        if self.cfg.hide_system {
            rows.retain(|r| r.category != cats::SYSTEM);
        }
        let items = stats::by_item(&rows, from, to);
        let total = items.iter().map(|i| i.secs).sum();
        let m = Arc::new(AppsModel { total, from, to, items, rows });
        self.apps_view.model = Some((key, m.clone()));
        m
    }

    pub(super) fn apps_view(&mut self, ui: &mut Ui, p: &Pal) {
        let t = self.s();
        let m = self.apps_model();
        let lang = self.lang;
        let h24 = self.h24();
        let mut act: Option<Act> = None;
        let q = self.apps_view.search.trim().to_lowercase();
        let mut shown = sorted(&m.items, &self.cat, lang, self.apps_view.sort, self.apps_view.asc);
        if !q.is_empty() {
            shown.retain(|i| self.cat.item_name(i.item, lang).to_lowercase().contains(&q));
        }
        let cat_infos = self.cat.categories(lang);
        page(ui, p, t.apps_title, None, |_| {}, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if let Some(i) = widgets::segmented(ui, "range", &[t.range_today, t.range_7, t.range_30, t.range_all], self.apps_view.range, p) {
                    self.apps_view.range = i;
                }
                ui.add_space(8.0);
                widgets::input(ui, &mut self.apps_view.search, t.search, 200.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    theme::switch(ui, &mut self.apps_view.show_hidden, p);
                    ui.label(RichText::new(t.show_hidden).font(theme::reg(13.0)).color(p.text2));
                });
            });
            ui.add_space(8.0);
            theme::hint(ui, t.apps_hint, p);
            ui.add_space(16.0);

            // The selected row: its days, rename and hide.
            if let Some(sel) = self.apps_view.selected {
                if let Some(stat) = m.items.iter().find(|i| i.item == sel) {
                    let name = self.cat.item_name(sel, lang);
                    titled_card(ui, p, &t.per_day_of.replace("{name}", &name), 0.0, |ui| {
                        let series = stats::item_per_day(&chrono::Local, &m.rows, sel, m.from, m.to);
                        let series = if series.len() > 62 { series[series.len() - 62..].to_vec() } else { series };
                        let color = widgets::rgb(self.cat.category(lang, &stat.category).color);
                        let bars: Vec<BarDay> = series
                            .iter()
                            .map(|(d, secs)| BarDay {
                                label: if series.len() <= 8 { fmt::weekday_short(lang, weekday_index(&chrono::Local, d + 3600)).to_string() } else { String::new() },
                                title: fmt::date_with_weekday(lang, d + 3600),
                                by_cat: vec![(stat.category.clone(), *secs)],
                                total: *secs,
                                highlight: false,
                                clickable: false,
                                date: chrono::DateTime::from_timestamp(d + 3600, 0).map(|u| u.with_timezone(&chrono::Local).date_naive()).unwrap_or_default(),
                            })
                            .collect();
                        charts::stacked_bars(ui, p, lang, 150.0, &bars, &|_| color, &|_| String::new(), "");
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            if let (Item::App(id), Some((rid, text))) = (sel, self.apps_view.rename.as_mut()) {
                                if *rid == id {
                                    let resp = widgets::input(ui, text, t.rename_hint, 220.0);
                                    let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                                    if widgets::btn(ui, t.save, Btn::Primary, p).clicked() || enter {
                                        act = Some(Act::SaveRename(id, text.clone()));
                                    }
                                    if widgets::btn(ui, t.cancel, Btn::Secondary, p).clicked() {
                                        act = Some(Act::CancelRename);
                                    }
                                }
                            }
                            let renaming = self.apps_view.rename.is_some();
                            if !renaming {
                                if let Item::App(id) = sel {
                                    if widgets::btn(ui, t.rename, Btn::Secondary, p).clicked() {
                                        act = Some(Act::StartRename(id, name.clone()));
                                    }
                                }
                                let hidden = self.cat.is_hidden(sel);
                                if widgets::btn(ui, if hidden { t.unhide } else { t.hide }, Btn::Secondary, p).clicked() {
                                    act = Some(Act::SetHidden(sel, !hidden));
                                }
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if widgets::btn(ui, t.close, Btn::Quiet, p).clicked() {
                                        act = Some(Act::Select(None));
                                    }
                                });
                            }
                        });
                    });
                    ui.add_space(24.0);
                }
            }

            if shown.is_empty() {
                ui.add_space(24.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(t.apps_empty).font(theme::medium(16.0)).color(p.text2));
                });
                return;
            }
            self.table(ui, p, &shown, &cat_infos, m.total, h24, &mut act);
        });
        if let Some(a) = act {
            self.apply_apps_act(ui.ctx(), a);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn table(&mut self, ui: &mut Ui, p: &Pal, shown: &[ItemStat], cat_infos: &[super::data::CatInfo], total: i64, h24: bool, act: &mut Option<Act>) {
        let t = self.s();
        let lang = self.lang;
        let w = ui.available_width();
        let wide = w >= 820.0;
        let (w_cat, w_time, w_share, w_sess, w_last) = (170.0, 100.0, 60.0, 76.0, if wide { 130.0 } else { 0.0 });
        let w_name = w - w_cat - w_time - w_share - w_sess - w_last - 16.0;
        let x_name = 16.0;
        let x_cat = x_name + w_name;
        let x_time = x_cat + w_cat;
        let x_share = x_time + w_time;
        let x_sess = x_share + w_share;
        let x_last = x_sess + w_sess;

        // Header: clickable column titles.
        let (head, _) = ui.allocate_exact_size(vec2(w, 32.0), Sense::hover());
        let cols: [(SortCol, &str, f32, bool); 6] = [
            (SortCol::Name, t.col_name, x_name, false),
            (SortCol::Category, t.col_category, x_cat, false),
            (SortCol::Time, t.col_time, x_time + w_time - 12.0, true),
            (SortCol::Share, t.col_share, x_share + w_share - 12.0, true),
            (SortCol::Sessions, t.col_sessions, x_sess + w_sess - 12.0, true),
            (SortCol::Last, t.col_last, x_last, false),
        ];
        for (col, label, x, right) in cols {
            if col == SortCol::Last && !wide {
                continue;
            }
            let on = self.apps_view.sort == col;
            let arrow = if on { if self.apps_view.asc { " ↑" } else { " ↓" } } else { "" };
            let text = format!("{label}{arrow}");
            let g = ui.painter().layout_no_wrap(text.clone(), theme::medium(12.0), p.text2);
            let gw = g.size().x;
            let left = if right { head.left() + x - gw } else { head.left() + x };
            let r = egui::Rect::from_min_size(pos2(left - 4.0, head.top()), vec2(gw + 8.0, 32.0));
            let resp = ui.interact(r, ui.scope_id().with(("sort", label)), Sense::click());
            ui.painter().text(pos2(left, head.center().y), Align2::LEFT_CENTER, text, theme::medium(12.0), if on { p.text } else { theme::mix(p.text2, p.text, theme::fade(ui, resp.id.with("h"), resp.hovered())) });
            if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                *act = Some(Act::Sort(col));
            }
        }
        ui.painter().hline(head.x_range(), head.bottom(), Stroke::new(1.0, p.line));

        for it in shown {
            let (row, resp) = ui.allocate_exact_size(vec2(w, 44.0), Sense::click());
            let selected = self.apps_view.selected == Some(it.item);
            if selected {
                ui.painter().rect(row, theme::R_CONTROL as f32, widgets::selected_fill(p), Stroke::new(1.0, p.line_hi), StrokeKind::Inside);
            } else {
                ui.painter().rect_filled(row, theme::R_CONTROL as f32, widgets::hover_fill(ui, resp.id, resp.hovered(), p));
            }
            let hidden = self.cat.is_hidden(it.item);
            let info = self.cat.category(lang, &it.category);
            let cy = row.center().y;
            let name = self.cat.item_name(it.item, lang);
            let (kind, name_col) = match it.item {
                Item::Site(_) => (t.kind_site, if hidden { p.muted } else { p.text }),
                Item::App(_) => (t.kind_app, if hidden { p.muted } else { p.text }),
            };
            ui.painter().circle_filled(pos2(row.left() + x_name - 2.0, cy), 4.0, widgets::rgb(info.color));
            let nm = widgets::fit_text(ui.painter(), &name, &theme::reg(14.0), w_name - 20.0 - 60.0);
            let g = ui.painter().layout_no_wrap(nm, theme::reg(14.0), name_col);
            let gw = g.size().x;
            ui.painter().galley(pos2(row.left() + x_name + 10.0, cy - g.size().y / 2.0), g, name_col);
            let tag = if hidden { format!("{kind} · {}", t.hidden_tag) } else { kind.to_string() };
            ui.painter().text(pos2(row.left() + x_name + 10.0 + gw + 8.0, cy), Align2::LEFT_CENTER, tag, theme::reg(11.5), p.muted);

            // Category: a dropdown right in the row.
            let cell = egui::Rect::from_min_size(pos2(row.left() + x_cat, cy - 17.0), vec2(w_cat - 12.0, 34.0));
            let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(cell).layout(Layout::left_to_right(Align::Center)));
            let current_label = match (it.item, self.cat_of_item_override(it.item)) {
                (Item::Site(_), None) => t.site_inherit.to_string(),
                _ => info.name.clone(),
            };
            let mut picked: Option<Option<String>> = None;
            widgets::combo(("cat", it.item), &current_label, w_cat - 12.0 - 16.0).show_ui(&mut cui, |ui| {
                if matches!(it.item, Item::Site(_)) && ui.selectable_label(self.cat_of_item_override(it.item).is_none(), RichText::new(t.site_inherit).font(theme::reg(13.5))).clicked() {
                    picked = Some(None);
                }
                for c in cat_infos {
                    let on = self.cat_of_item_override(it.item).as_deref() == Some(c.id.as_str());
                    if ui.selectable_label(on, RichText::new(&c.name).font(theme::reg(13.5))).clicked() {
                        picked = Some(Some(c.id.clone()));
                    }
                }
            });
            if let Some(c) = picked {
                *act = Some(Act::SetCategory(it.item, c));
            }

            let mono = |ui: &Ui, text: String, x_right: f32, col| {
                let g = ui.painter().layout_no_wrap(text, theme::mono(12.5), col);
                ui.painter().galley(pos2(row.left() + x_right - g.size().x, cy - g.size().y / 2.0), g, col);
            };
            mono(ui, fmt::duration(lang, it.secs), x_time + w_time - 12.0, p.text);
            let share = if total > 0 { it.secs as f64 / total as f64 * 100.0 } else { 0.0 };
            mono(ui, format!("{share:.0}%"), x_share + w_share - 12.0, p.text2);
            mono(ui, it.sessions.to_string(), x_sess + w_sess - 12.0, p.text2);
            if wide {
                ui.painter().text(pos2(row.left() + x_last, cy), Align2::LEFT_CENTER, fmt::date_time(lang, it.last_used, h24), theme::reg(12.5), p.text2);
            }
            ui.painter().hline(row.x_range().shrink(8.0), row.bottom(), Stroke::new(1.0, p.line));
            if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() && act.is_none() {
                *act = Some(Act::Select(if selected { None } else { Some(it.item) }));
            }
        }
    }

    /// The category the user (or the shipped defaults) set on the item itself: always set for programs, optional for sites.
    fn cat_of_item_override(&self, item: Item) -> Option<String> {
        match item {
            Item::App(id) => self.cat.apps.get(&id).map(|a| a.category.clone()),
            Item::Site(id) => self.cat.sites.get(&id).and_then(|s| s.category.clone()),
        }
    }

    fn apply_apps_act(&mut self, ctx: &egui::Context, act: Act) {
        match act {
            Act::Sort(col) => {
                if self.apps_view.sort == col {
                    self.apps_view.asc = !self.apps_view.asc;
                } else {
                    self.apps_view.sort = col;
                    self.apps_view.asc = matches!(col, SortCol::Name | SortCol::Category);
                }
            }
            Act::Select(s) => {
                self.apps_view.selected = s;
                self.apps_view.rename = None;
            }
            Act::SetCategory(item, cat) => {
                let _ = match item {
                    Item::App(id) => self.db.set_app_category(id, cat.as_deref().unwrap_or(cats::OTHER)),
                    Item::Site(id) => self.db.set_site_category(id, cat.as_deref()),
                };
                self.data_changed();
                self.tell_tracker("reload");
            }
            Act::SetHidden(item, hidden) => {
                let _ = match item {
                    Item::App(id) => self.db.set_app_hidden(id, hidden),
                    Item::Site(id) => self.db.set_site_hidden(id, hidden),
                };
                if hidden && !self.apps_view.show_hidden {
                    self.apps_view.selected = None;
                }
                self.data_changed();
                self.tell_tracker("reload");
            }
            Act::StartRename(id, name) => self.apps_view.rename = Some((id, name)),
            Act::SaveRename(id, name) => {
                if !name.trim().is_empty() {
                    let _ = self.db.rename_app(id, &name);
                    self.data_changed();
                    self.tell_tracker("reload");
                }
                self.apps_view.rename = None;
            }
            Act::CancelRename => self.apps_view.rename = None,
        }
        ctx.request_repaint();
        let _ = Icon::Check;
    }
}

pub(super) fn back_days(from_day_start: i64, n: i64) -> i64 {
    let mut d = from_day_start;
    for _ in 0..n {
        d = day_start(&chrono::Local, d - 3600);
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::AppRow;

    fn stat(item: Item, secs: i64, sessions: usize, last: i64) -> ItemStat {
        ItemStat { item, category: "code".into(), secs, sessions, last_used: last }
    }

    #[test]
    fn sorting_by_every_column() {
        let mut cat = Catalog::default();
        for (id, name) in [(1, "Zed"), (2, "Atom"), (3, "Mid")] {
            cat.apps.insert(id, AppRow { id, exe: name.into(), name: name.into(), category: "code".into(), manual: false, hidden: false });
        }
        let items = vec![stat(Item::App(1), 100, 5, 10), stat(Item::App(2), 300, 1, 30), stat(Item::App(3), 200, 9, 20)];
        let ids = |v: Vec<ItemStat>| v.into_iter().map(|i| match i.item { Item::App(x) => x, Item::Site(x) => x }).collect::<Vec<_>>();
        assert_eq!(ids(sorted(&items, &cat, Lang::En, SortCol::Time, false)), vec![2, 3, 1]);
        assert_eq!(ids(sorted(&items, &cat, Lang::En, SortCol::Name, true)), vec![2, 3, 1]);
        assert_eq!(ids(sorted(&items, &cat, Lang::En, SortCol::Sessions, false)), vec![3, 1, 2]);
        assert_eq!(ids(sorted(&items, &cat, Lang::En, SortCol::Last, true)), vec![1, 3, 2]);
        assert_eq!(ids(sorted(&items, &cat, Lang::En, SortCol::Share, true)), vec![1, 3, 2]);
    }

    #[test]
    fn back_days_walks_whole_days() {
        let today = day_start(&chrono::Local, 1_795_000_000);
        let six = back_days(today, 6);
        assert!(today - six >= 6 * 86_400 - 3600 && today - six <= 6 * 86_400 + 3600);
        assert_eq!(back_days(today, 0), today);
    }
}
