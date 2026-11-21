//! First run: one screen that says what is recorded, that it all stays on this computer, and that the tracker
//! starts with the computer. Nothing is recorded until "Start tracking" is pressed.

use super::{widgets, App};
use crate::i18n;
use crate::theme::{self, Btn, Pal};
use crate::platform;
use eframe::egui::{self, Align, Layout, Margin, RichText, ScrollArea, Sense, Ui, vec2};

pub struct OnboardingState {
    pub track_sites: bool,
    pub autostart: bool,
}

impl App {
    pub(super) fn onboarding_view(&mut self, ui: &mut Ui, p: &Pal) {
        let ctx = ui.ctx().clone();
        let t = self.s();
        let lang = self.lang;
        let full = ui.available_rect_before_wrap();
        ui.painter().rect_filled(full, 0.0, p.bg);
        let mut start = false;
        ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let avail = ui.available_width();
            let col_w = (avail - 48.0).clamp(320.0, 560.0);
            ui.horizontal_top(|ui| {
                ui.add_space(((avail - col_w) / 2.0).max(0.0));
                ui.vertical(|ui| {
                    ui.set_width(col_w);
                    ui.add_space(28.0);
                    // Language and theme first: a Russian or Ukrainian reader can switch before reading on.
                    ui.horizontal(|ui| {
                        widgets::mark_glyph(ui.painter(), ui.cursor().min + vec2(14.0, 18.0), p.accent);
                        ui.add_space(32.0);
                        ui.label(RichText::new("Timelyx").font(theme::medium(18.0)).color(p.text));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            widgets::combo("ob-language", lang.native_name(), 130.0).show_ui(ui, |ui| {
                                for l in i18n::Lang::ALL {
                                    if ui.selectable_label(l == lang, RichText::new(l.native_name()).font(theme::reg(13.5))).clicked() {
                                        self.lang = l;
                                        self.cfg.language = Some(l.code().to_string());
                                        self.touch_cfg(&ctx);
                                    }
                                }
                            });
                        });
                    });
                    ui.add_space(24.0);
                    ui.label(RichText::new(t.ob_title).font(theme::medium(28.0)).color(p.text));
                    ui.add_space(8.0);
                    ui.add(egui::Label::new(RichText::new(t.ob_lead).font(theme::reg(15.0)).color(p.text2)).wrap());
                    ui.add_space(24.0);

                    theme::section(ui, t.ob_recorded, p);
                    ui.add_space(8.0);
                    theme::card(p).inner_margin(Margin::same(16)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing.y = 10.0;
                        for line in [t.ob_rec_1, t.ob_rec_2, t.ob_rec_3] {
                            ui.horizontal_top(|ui| {
                                let (r, _) = ui.allocate_exact_size(vec2(16.0, 20.0), Sense::hover());
                                widgets::icon(ui.painter(), widgets::Icon::Check, r.center(), 14.0, p.accent);
                                ui.add(egui::Label::new(RichText::new(line).font(theme::reg(14.0)).color(p.text)).wrap());
                            });
                        }
                    });
                    ui.add_space(12.0);
                    ui.add(egui::Label::new(RichText::new(t.ob_local).font(theme::medium(14.0)).color(p.text)).wrap());
                    ui.add_space(24.0);

                    theme::card(p).inner_margin(Margin::symmetric(16, 12)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        switch(ui, p, t.track_sites, t.track_sites_hint, &mut self.onboarding.track_sites);
                        switch(ui, p, if cfg!(windows) { t.autostart } else { t.autostart_os }, "", &mut self.onboarding.autostart);
                        theme::hint(ui, t.ob_autostart_note, p);
                    });
                    ui.add_space(24.0);
                    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::hover());
                    if theme::primary_button(ui, rect, ui.scope_id().with("ob-start"), t.ob_start, None, true, p).clicked() {
                        start = true;
                    }
                    ui.add_space(28.0);
                });
            });
        });
        if start {
            self.finish_onboarding();
        }
    }

    fn finish_onboarding(&mut self) {
        self.cfg.onboarded = true;
        self.cfg.track_sites = self.onboarding.track_sites;
        self.cfg.autostart = self.onboarding.autostart;
        self.settings_view.autostart = {
            let _ = platform::set_autostart(self.onboarding.autostart);
            platform::autostart_enabled()
        };
        self.cfg.save_to(&self.dir);
        if !self.demo && std::env::var_os("TIMELYX_NO_TRACKER").is_none() {
            platform::spawn_background();
        }
        self.refresh_soon();
    }
}

fn switch(ui: &mut Ui, p: &Pal, title: &str, hint: &str, value: &mut bool) {
    let w = ui.available_width();
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width(w - 56.0);
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(RichText::new(title).font(theme::reg(14.0)).color(p.text));
            if !hint.is_empty() {
                theme::hint(ui, hint, p);
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            theme::switch(ui, value, p);
        });
    });
    ui.add_space(8.0);
    let _ = Btn::Primary;
}
