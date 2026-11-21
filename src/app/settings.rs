//! The Settings screen and its confirmation dialog.

use super::data::site_label;
use super::{modal, page, today_start, widgets, App};
use crate::db::write_csv;
use crate::i18n;
use crate::nimvoro::{self, UpdateState};
use crate::theme::{self, Btn, Pal};
use crate::{platform, support, update_ui};
use eframe::egui::{self, Align, Layout, Margin, RichText, Sense, Stroke, StrokeKind, Ui, vec2};

pub struct SettingsState {
    pub exclusion: String,
    /// 0 seven days, 1 thirty days, 2 all time.
    pub export_range: usize,
    pub autostart: bool,
    pub msg: Option<String>,
    pub confirm_delete: bool,
}

impl SettingsState {
    pub fn new() -> Self {
        Self { exclusion: String::new(), export_range: 1, autostart: platform::autostart_enabled(), msg: None, confirm_delete: false }
    }
}

/// A titled group: a section title (24 above, 8 below) over a card.
fn group<R>(ui: &mut Ui, p: &Pal, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.add_space(24.0);
    theme::section(ui, title, p);
    ui.add_space(8.0);
    theme::card(p).inner_margin(Margin::symmetric(16, 12)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui)
    })
    .inner
}

fn switch_row(ui: &mut Ui, p: &Pal, title: &str, hint: &str, value: &mut bool) -> bool {
    let mut changed = false;
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
            changed = theme::switch(ui, value, p).changed();
        });
    });
    ui.add_space(8.0);
    changed
}

fn combo_row<T: PartialEq + Copy>(ui: &mut Ui, p: &Pal, title: &str, id: &str, current: T, options: &[(T, String)], width: f32) -> Option<T> {
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).font(theme::reg(14.0)).color(p.text));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let label = options.iter().find(|o| o.0 == current).map(|o| o.1.as_str()).unwrap_or("");
            widgets::combo(id, label, width).show_ui(ui, |ui| {
                for (v, name) in options {
                    if ui.selectable_label(*v == current, RichText::new(name).font(theme::reg(13.5))).clicked() {
                        picked = Some(*v);
                    }
                }
            });
        });
    });
    ui.add_space(8.0);
    picked
}

impl App {
    pub(super) fn settings_view(&mut self, ui: &mut Ui, p: &Pal) {
        let ctx = ui.ctx().clone();
        let t = self.s();
        let lang = self.lang;
        let mut open_delete = false;
        let mut export = false;
        let mut stop = false;
        let mut start = false;
        page(ui, p, t.settings_title, None, |_| {}, |ui| {
            // ── General
            group(ui, p, t.group_general, |ui| {
                let langs: Vec<(i18n::Lang, String)> = i18n::Lang::ALL.iter().map(|l| (*l, l.native_name().to_string())).collect();
                if let Some(l) = combo_row(ui, p, i18n::language_label(lang), "language", lang, &langs, 150.0) {
                    self.lang = l;
                    self.cfg.language = Some(l.code().to_string());
                    self.version += 1;
                    self.touch_cfg(&ctx);
                }
                let themes: Vec<(theme::ThemeMode, String)> = theme::ThemeMode::ALL.iter().map(|m| (*m, i18n::theme_name(*m, lang).to_string())).collect();
                if let Some(m) = combo_row(ui, p, i18n::theme_label(lang), "theme", self.cfg.theme, &themes, 150.0) {
                    self.cfg.theme = m;
                    theme::apply(&ctx, m);
                    self.touch_cfg(&ctx);
                }
                if lang == i18n::Lang::En {
                    let opts = vec![(false, t.monday.to_string()), (true, t.sunday.to_string())];
                    if let Some(v) = combo_row(ui, p, t.week_start, "weekstart", self.cfg.week_starts_sunday, &opts, 150.0) {
                        self.cfg.week_starts_sunday = v;
                        self.version += 1;
                        self.touch_cfg(&ctx);
                    }
                    let opts = vec![(true, "24-hour".to_string()), (false, "12-hour".to_string())];
                    if let Some(v) = combo_row(ui, p, t.time_format, "timefmt", self.cfg.time_24h, &opts, 150.0) {
                        self.cfg.time_24h = v;
                        self.version += 1;
                        self.touch_cfg(&ctx);
                    }
                } else {
                    theme::hint(ui, t.week_fixed_hint, p);
                    ui.add_space(8.0);
                }
                let mut auto = self.settings_view.autostart;
                if switch_row(ui, p, if cfg!(windows) { t.autostart } else { t.autostart_os }, t.autostart_hint, &mut auto) {
                    let ok = platform::set_autostart(auto);
                    self.settings_view.autostart = platform::autostart_enabled();
                    self.cfg.autostart = self.settings_view.autostart;
                    self.settings_view.msg = if ok && self.settings_view.autostart == auto { None } else { Some(t.autostart_failed.to_string()) };
                    self.touch_cfg(&ctx);
                }
                if let Some(m) = &self.settings_view.msg {
                    ui.label(RichText::new(m).font(theme::reg(12.0)).color(p.negative));
                }
            });

            // ── Tracking
            group(ui, p, t.group_tracking, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.label(RichText::new(t.tracker_status).font(theme::reg(14.0)).color(p.text));
                        theme::hint(ui, if self.snap.running { t.tracker_running } else { t.tracker_stopped }, p);
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if self.snap.running {
                            if widgets::btn(ui, t.tracker_stop, Btn::Secondary, p).clicked() {
                                stop = true;
                            }
                        } else if widgets::btn(ui, t.start_tracking, Btn::Primary, p).clicked() {
                            start = true;
                        }
                    });
                });
                theme::divider(ui, p);

                let idle_opts: Vec<(u32, String)> =
                    [0u32, 1, 2, 3, 5, 10, 15, 30, 60].iter().map(|m| (*m, if *m == 0 { t.idle_never.to_string() } else { t.idle_min.replace("{n}", &m.to_string()) })).collect();
                if let Some(v) = combo_row(ui, p, t.idle_after, "idle", self.cfg.idle_minutes, &idle_opts, 150.0) {
                    self.cfg.idle_minutes = v;
                    self.touch_cfg(&ctx);
                }
                theme::hint(ui, t.idle_hint, p);
                ui.add_space(12.0);

                let mut v = self.cfg.track_sites;
                if switch_row(ui, p, t.track_sites, t.track_sites_hint, &mut v) {
                    self.cfg.track_sites = v;
                    self.touch_cfg(&ctx);
                }
                // Excluded sites: chips and an input.
                ui.label(RichText::new(t.exclusions).font(theme::reg(14.0)).color(p.text));
                ui.add_space(2.0);
                theme::hint(ui, t.exclusions_hint, p);
                ui.add_space(8.0);
                let mut remove = None;
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
                    for (i, e) in self.cfg.site_exclusions.iter().enumerate() {
                        let g = ui.painter().layout_no_wrap(e.clone(), theme::reg(13.0), p.text);
                        let (r, resp) = ui.allocate_exact_size(vec2(g.size().x + 40.0, 28.0), Sense::click());
                        ui.painter().rect(r, 8.0, p.raised, Stroke::new(1.0, p.line), StrokeKind::Inside);
                        ui.painter().galley(egui::pos2(r.left() + 12.0, r.center().y - g.size().y / 2.0), g, p.text);
                        widgets::icon(ui.painter(), widgets::Icon::Close, egui::pos2(r.right() - 14.0, r.center().y), 12.0, if resp.hovered() { p.text } else { p.muted });
                        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                            remove = Some(i);
                        }
                    }
                });
                if let Some(i) = remove {
                    self.cfg.site_exclusions.remove(i);
                    self.touch_cfg(&ctx);
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let resp = widgets::input(ui, &mut self.settings_view.exclusion, t.exclusion_placeholder, 220.0);
                    let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let text = self.settings_view.exclusion.trim().to_string();
                    let ok = !text.is_empty();
                    if (widgets::btn_enabled(ui, t.add, Btn::Secondary, ok, p).clicked() || enter) && ok {
                        if !self.cfg.site_exclusions.iter().any(|e| e.eq_ignore_ascii_case(&text)) {
                            self.cfg.site_exclusions.push(text);
                            self.touch_cfg(&ctx);
                        }
                        self.settings_view.exclusion.clear();
                    }
                });
                theme::divider(ui, p);

                let ret_opts: Vec<(u32, String)> =
                    [0u32, 1, 3, 6, 12, 24].iter().map(|m| (*m, if *m == 0 { t.retention_forever.to_string() } else { t.retention_months.replace("{n}", &m.to_string()) })).collect();
                if let Some(v) = combo_row(ui, p, t.retention, "retention", self.cfg.retention_months, &ret_opts, 150.0) {
                    self.cfg.retention_months = v;
                    self.touch_cfg(&ctx);
                }
                let mut v = self.cfg.hide_system;
                if switch_row(ui, p, t.hide_system, "", &mut v) {
                    self.cfg.hide_system = v;
                    self.version += 1;
                    self.touch_cfg(&ctx);
                }
                let mut v = self.cfg.limit_notifications;
                if switch_row(ui, p, t.limit_notifications, "", &mut v) {
                    self.cfg.limit_notifications = v;
                    self.touch_cfg(&ctx);
                }
            });

            // ── Data
            group(ui, p, t.group_data, |ui| {
                ui.label(RichText::new(self.dir.display().to_string()).font(theme::mono(12.0)).color(p.muted));
                ui.add_space(8.0);
                if widgets::btn(ui, t.open_data_folder, Btn::Secondary, p).clicked() {
                    open_folder(&self.dir);
                }
                theme::divider(ui, p);
                theme::section(ui, t.export_csv, p);
                ui.add_space(2.0);
                theme::hint(ui, t.export_hint, p);
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
                    if let Some(i) = widgets::segmented(ui, "export-range", &[t.export_last_7, t.export_last_30, t.export_all], self.settings_view.export_range, p) {
                        self.settings_view.export_range = i;
                    }
                    if widgets::btn(ui, t.export_btn, Btn::Secondary, p).clicked() {
                        export = true;
                    }
                });
                theme::divider(ui, p);
                theme::section(ui, t.delete_all, p);
                ui.add_space(2.0);
                theme::hint(ui, t.delete_all_hint, p);
                ui.add_space(8.0);
                if widgets::danger_button(ui, t.delete_all, p).clicked() {
                    open_delete = true;
                }
            });

            // ── Updates
            let u = nimvoro::strings(lang);
            group(ui, p, u.updates, |ui| {
                let mut v = self.cfg.check_updates;
                if switch_row(ui, p, u.check_on_start, u.check_on_start_hint, &mut v) {
                    self.cfg.check_updates = v;
                    self.touch_cfg(&ctx);
                }
                if !update_ui::progress(ui, &self.updater, lang, p) {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(self.updater.status_text(lang)).font(theme::reg(13.0)).color(p.text2));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if let UpdateState::Available { .. } = self.updater.state() {
                                if widgets::btn(ui, u.update_btn, Btn::Primary, p).clicked() {
                                    let saved = self.flush();
                                    if self.updater.apply_if(saved) {
                                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                    }
                                }
                            }
                            if widgets::btn(ui, u.check_now, Btn::Secondary, p).clicked() {
                                let c = ctx.clone();
                                self.updater.check(move || c.request_repaint());
                            }
                        });
                    });
                }
            });

            // ── Support the author
            ui.add_space(24.0);
            support::support_section(ui, lang);

            // ── About
            group(ui, p, t.about, |ui| {
                ui.horizontal(|ui| {
                    theme::mark(ui, 32.0, widgets::mark_glyph, p);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.label(RichText::new("Timelyx").font(theme::medium(15.0)).color(p.text));
                        theme::meta(ui, &format!("{} {}", t.version, env!("CARGO_PKG_VERSION")), p);
                    });
                });
                ui.add_space(8.0);
                theme::hint(ui, t.privacy_note, p);
                ui.add_space(8.0);
                if widgets::btn(ui, crate::REPO_URL, Btn::Link, p).clicked() {
                    ctx.open_url(egui::OpenUrl::new_tab(crate::REPO_URL));
                }
            });
        });
        if open_delete {
            self.settings_view.confirm_delete = true;
        }
        if export {
            self.export_csv(&ctx);
        }
        if stop {
            self.tell_tracker("quit");
            self.refresh_soon();
        }
        if start {
            platform::spawn_background();
            self.refresh_soon();
        }
    }

    fn export_csv(&mut self, ctx: &egui::Context) {
        let t = self.s();
        let lang = self.lang;
        let today = today_start(self.now);
        let (from, to) = match self.settings_view.export_range {
            0 => (super::apps::back_days(today, 6), self.now + 1),
            1 => (super::apps::back_days(today, 29), self.now + 1),
            _ => (0, self.now + 1),
        };
        let rows = self.db.export_rows(from, to).unwrap_or_default();
        let name = format!("timelyx-{}.csv", chrono::Local::now().format("%Y-%m-%d"));
        let Some(path) = rfd::FileDialog::new().set_file_name(name).add_filter("CSV", &["csv"]).save_file() else { return };
        let mut buf = vec![0xEF, 0xBB, 0xBF];
        let cat = &self.cat;
        let ok = write_csv(&rows, &chrono::Local, &|c| cat.category(lang, c).name, &|s| site_label(lang, s), &mut buf).is_ok() && std::fs::write(&path, &buf).is_ok();
        let msg = if ok { t.exported_n.replace("{n}", &rows.len().to_string()) } else { t.export_failed.to_string() };
        self.notify(ctx, msg);
    }

    pub(super) fn settings_dialogs(&mut self, ctx: &egui::Context, p: &Pal) {
        if !self.settings_view.confirm_delete {
            return;
        }
        let t = self.s();
        let mut confirm = false;
        let mut cancel = false;
        let backdrop = modal(ctx, p, "confirm-delete", self.open_modal_generation(), 420.0, |ui| {
            ui.label(RichText::new(t.delete_confirm_title).font(theme::medium(18.0)).color(p.text));
            ui.add_space(8.0);
            ui.add(egui::Label::new(RichText::new(t.delete_confirm_body).font(theme::reg(14.0)).color(p.text2)).wrap());
            ui.add_space(20.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::danger_button(ui, t.delete_do, p).clicked() {
                        confirm = true;
                    }
                    if widgets::btn(ui, t.cancel, Btn::Secondary, p).clicked() {
                        cancel = true;
                    }
                });
            });
        });
        if confirm {
            let _ = self.db.delete_all();
            self.settings_view.confirm_delete = false;
            self.tell_tracker("reload");
            self.data_changed();
            self.notify(ctx, t.deleted.to_string());
        } else if cancel || backdrop {
            self.settings_view.confirm_delete = false;
        }
    }
}

/// Opens a folder in the file manager.
pub fn open_folder(dir: &std::path::Path) {
    let _ = std::fs::create_dir_all(dir);
    let _ = std::process::Command::new(if cfg!(windows) { "explorer.exe" } else { "xdg-open" }).arg(dir).spawn();
}
