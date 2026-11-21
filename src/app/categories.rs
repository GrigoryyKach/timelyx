//! Categories: the built-in ones with their rules, and the user's own with a name and a colour.

use super::widgets::{self, Icon};
use super::{page, App};
use crate::cats;
use crate::strings;
use crate::theme::{self, Btn, Pal};
use eframe::egui::{self, Align, CursorIcon, Layout, Margin, RichText, Sense, Stroke, Ui, vec2};

pub struct CatsState {
    /// The custom category being edited, with its working copy.
    pub editing: Option<String>,
    pub name: String,
    pub color: u32,
    pub new_name: String,
    pub new_color: u32,
}

impl Default for CatsState {
    fn default() -> Self {
        Self { editing: None, name: String::new(), color: cats::PALETTE[4], new_name: String::new(), new_color: cats::PALETTE[4] }
    }
}

/// Colour choices: the fixed palette as round swatches.
pub fn swatches(ui: &mut Ui, color: &mut u32, p: &Pal) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        for c in cats::PALETTE {
            let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
            ui.painter().circle_filled(r.center(), 11.0, widgets::rgb(c));
            if *color == c {
                ui.painter().circle_stroke(r.center(), 13.0, Stroke::new(1.5, p.text));
                widgets::icon(ui.painter(), Icon::Check, r.center(), 13.0, widgets::on_color(widgets::rgb(c)));
            }
            if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                *color = c;
            }
        }
    });
}

enum Act {
    Add,
    Edit(String, String, u32),
    Save,
    Cancel,
    Delete(String),
}

impl App {
    pub(super) fn categories_view(&mut self, ui: &mut Ui, p: &Pal) {
        let t = self.s();
        let lang = self.lang;
        let mut act: Option<Act> = None;
        page(ui, p, t.categories_title, Some(t.categories_hint), |_| {}, |ui| {
            theme::section(ui, t.builtin_title, p);
            ui.add_space(8.0);
            theme::card(p).inner_margin(Margin::symmetric(16, 4)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                for (i, (id, color)) in cats::BUILTIN.iter().enumerate() {
                    if i > 0 {
                        theme::hairline(ui, p);
                    }
                    ui.horizontal(|ui| {
                        ui.set_min_height(48.0);
                        let (r, _) = ui.allocate_exact_size(vec2(14.0, 48.0), Sense::hover());
                        ui.painter().rect_filled(egui::Rect::from_center_size(r.center(), vec2(12.0, 12.0)), 3.0, widgets::rgb(*color));
                        ui.vertical(|ui| {
                            ui.add_space(8.0);
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.label(RichText::new(strings::builtin_name(lang, id).unwrap_or(id)).font(theme::medium(14.0)).color(p.text));
                            theme::hint(ui, strings::builtin_rule(lang, id).unwrap_or(""), p);
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            theme::meta(ui, &t.items_in.replace("{n}", &self.cat.count_in(id).to_string()), p);
                        });
                    });
                }
            });
            ui.add_space(24.0);

            theme::section(ui, t.custom_title, p);
            ui.add_space(8.0);
            theme::card(p).inner_margin(Margin::symmetric(16, 8)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                if self.cat.customs.is_empty() {
                    theme::hint(ui, t.no_custom, p);
                    ui.add_space(4.0);
                }
                let customs = self.cat.customs.clone();
                for (i, c) in customs.iter().enumerate() {
                    if i > 0 {
                        theme::hairline(ui, p);
                    }
                    let editing = self.cats_view.editing.as_deref() == Some(c.id.as_str());
                    if editing {
                        ui.add_space(8.0);
                        widgets::input(ui, &mut self.cats_view.name, t.category_name, 260.0);
                        ui.add_space(8.0);
                        swatches(ui, &mut self.cats_view.color, p);
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if widgets::btn(ui, t.save, Btn::Primary, p).clicked() {
                                act = Some(Act::Save);
                            }
                            if widgets::btn(ui, t.cancel, Btn::Secondary, p).clicked() {
                                act = Some(Act::Cancel);
                            }
                        });
                        ui.add_space(8.0);
                    } else {
                        ui.horizontal(|ui| {
                            ui.set_min_height(48.0);
                            let (r, _) = ui.allocate_exact_size(vec2(14.0, 48.0), Sense::hover());
                            ui.painter().rect_filled(egui::Rect::from_center_size(r.center(), vec2(12.0, 12.0)), 3.0, widgets::rgb(c.color));
                            ui.label(RichText::new(&c.name).font(theme::medium(14.0)).color(p.text));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if widgets::icon_button(ui, Icon::Trash, 32.0, p).on_hover_text(t.delete).clicked() {
                                    act = Some(Act::Delete(c.id.clone()));
                                }
                                if widgets::icon_button(ui, Icon::Edit, 32.0, p).on_hover_text(t.edit).clicked() {
                                    act = Some(Act::Edit(c.id.clone(), c.name.clone(), c.color));
                                }
                                theme::meta(ui, &t.items_in.replace("{n}", &self.cat.count_in(&c.id).to_string()), p);
                            });
                        });
                    }
                }
                if !self.cat.customs.is_empty() {
                    theme::hairline(ui, p);
                    ui.add_space(8.0);
                    theme::hint(ui, t.delete_category_hint, p);
                    ui.add_space(4.0);
                }
            });
            ui.add_space(24.0);

            theme::section(ui, t.add_category, p);
            ui.add_space(8.0);
            theme::card(p).inner_margin(Margin::same(16)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                widgets::input(ui, &mut self.cats_view.new_name, t.category_name, 260.0);
                ui.add_space(12.0);
                theme::hint(ui, t.colour, p);
                ui.add_space(6.0);
                swatches(ui, &mut self.cats_view.new_color, p);
                ui.add_space(12.0);
                let ok = !self.cats_view.new_name.trim().is_empty();
                if widgets::btn_enabled(ui, t.add, Btn::Primary, ok, p).clicked() && ok {
                    act = Some(Act::Add);
                }
            });
        });
        match act {
            Some(Act::Add) => {
                let name = std::mem::take(&mut self.cats_view.new_name);
                let _ = self.db.add_category(&name, self.cats_view.new_color);
                self.data_changed();
                self.tell_tracker("reload");
            }
            Some(Act::Edit(id, name, color)) => {
                self.cats_view.editing = Some(id);
                self.cats_view.name = name;
                self.cats_view.color = color;
            }
            Some(Act::Save) => {
                if let Some(id) = self.cats_view.editing.take() {
                    if !self.cats_view.name.trim().is_empty() {
                        let _ = self.db.update_category(&id, &self.cats_view.name, self.cats_view.color);
                        self.data_changed();
                    }
                }
            }
            Some(Act::Cancel) => self.cats_view.editing = None,
            Some(Act::Delete(id)) => {
                let _ = self.db.delete_category(&id);
                if self.cats_view.editing.as_deref() == Some(id.as_str()) {
                    self.cats_view.editing = None;
                }
                self.data_changed();
                self.tell_tracker("reload");
            }
            None => {}
        }
    }
}
