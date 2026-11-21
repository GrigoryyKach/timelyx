//! The window: state, the per-frame logic, navigation and the data it shows. Views live in the sibling modules.
//!
//! The window is a client of the tracker process. It reads the database, edits categories there, and asks the
//! tracker (over `ipc`) to write what it has in memory so the numbers are current.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{Local, NaiveDate};
use eframe::egui::{self, Frame, Id, Key, Margin, Modifiers, Order, Rect, Sense, Stroke, Ui};

use crate::bus::{self, Cmd};
use crate::config::{Config, Window};
use crate::db::{Db, SessionRow};
use crate::engine::day_start;
use crate::i18n::{self, Lang};
use crate::nimvoro::Updater;
use crate::stats::{self, ItemStat};
use crate::strings::{self, S};
use crate::theme::{self, Pal};
use crate::ipc;

mod apps;
mod categories;
mod charts;
mod data;
mod limits;
mod onboarding;
mod settings;
mod today;
mod trends;
mod widgets;

use data::{Catalog, DaemonSnap};

/// The app accent: rose, away from the teal, blue, amber, red and green of the other suite apps.
pub const ACCENT: theme::Accent = theme::Accent::new(0xe8669a, 0xb0306a, 0x141415, 0xffffff);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Nav {
    Today,
    Trends,
    Apps,
    Categories,
    Limits,
    Settings,
}

impl Nav {
    const ALL: [Nav; 6] = [Nav::Today, Nav::Trends, Nav::Apps, Nav::Categories, Nav::Limits, Nav::Settings];

    fn from_name(s: &str) -> Option<Nav> {
        Some(match s {
            "today" => Nav::Today,
            "trends" | "week" => Nav::Trends,
            "apps" => Nav::Apps,
            "categories" => Nav::Categories,
            "limits" => Nav::Limits,
            "settings" => Nav::Settings,
            _ => return None,
        })
    }
}

/// Result of the background refresh (the tracker's state).
#[derive(Default)]
struct Refresh {
    snap: Mutex<Option<DaemonSnap>>,
    ready: AtomicBool,
    busy: AtomicBool,
}

pub struct Toast {
    pub text: String,
    pub until: f64,
}

/// Screenshot driver (set by `TIMELYX_SHOT_DIR`): walks through the screens, saves a PNG of each and quits.
struct Shots {
    dir: PathBuf,
    prefix: String,
    queue: Vec<Nav>,
    wait: u32,
    asked: bool,
    done: Arc<AtomicBool>,
}

pub struct App {
    pub cfg: Config,
    pub lang: Lang,
    pub dir: PathBuf,
    pub db: Db,
    pub cat: Catalog,
    pub snap: DaemonSnap,
    pub nav: Nav,
    pub now: i64,
    pub today: NaiveDate,
    /// Bumped whenever the data may have changed; cached views compare it.
    pub version: u64,
    pub updater: Updater,
    pub demo: bool,
    pub toast: Option<Toast>,
    pub today_view: today::TodayState,
    pub trends_view: trends::TrendsState,
    pub apps_view: apps::AppsState,
    pub cats_view: categories::CatsState,
    pub limits_view: limits::LimitsState,
    pub settings_view: settings::SettingsState,
    pub onboarding: onboarding::OnboardingState,
    refresh: Arc<Refresh>,
    last_refresh: f64,
    cfg_dirty: Option<f64>,
    shots: Option<Shots>,
    modal_generation: u64,
    modal_was_open: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, mut cfg: Config, dir: PathBuf, stop_tracker: fn()) -> Self {
        // Screenshots and tests may force the language and theme without touching the saved settings.
        if crate::config::is_test_instance() {
            if let Ok(l) = std::env::var("TIMELYX_LANG") {
                cfg.language = Some(l);
            }
            if let Some(m) = std::env::var("TIMELYX_THEME").ok().and_then(|t| match t.as_str() {
                "dark" => Some(theme::ThemeMode::Dark),
                "light" => Some(theme::ThemeMode::Light),
                _ => None,
            }) {
                cfg.theme = m;
            }
        }
        theme::set_accent(ACCENT);
        theme::install(&cc.egui_ctx, cfg.theme);
        cc.egui_ctx.all_styles_mut(|s| s.spacing.interact_size.y = widgets::ROW_BTN_H);
        let lang = i18n::resolve(cfg.language.as_deref());
        let mut cfg_dirty = None;
        if cfg.language.as_deref() != Some(lang.code()) {
            cfg.language = Some(lang.code().to_string());
            cfg_dirty = Some(0.0);
        }
        bus::init(cc.egui_ctx.clone());
        let db = Db::open(&dir).unwrap_or_else(|_| {
            // A database that cannot be opened: work with an empty temporary one rather than crash.
            Db::open(&std::env::temp_dir().join("timelyx-fallback")).expect("a temporary database")
        });
        let now = chrono::Utc::now().timestamp();
        let cat = Catalog::load(&db);
        let updater = Updater::new("timelyx", crate::REPO_URL.trim_start_matches("https://github.com/"), env!("CARGO_PKG_VERSION"))
            .relaunch_args(&["--background"])
            .before_restart(stop_tracker);
        if cfg.check_updates && !crate::config::is_test_instance() {
            let ctx = cc.egui_ctx.clone();
            updater.check(move || ctx.request_repaint());
        }
        let demo = std::env::var("TIMELYX_DEMO").is_ok_and(|v| v == "1") || std::env::args().any(|a| a == "--demo");
        let shots = std::env::var_os("TIMELYX_SHOT_DIR").map(|d| {
            let names = std::env::var("TIMELYX_SHOT_NAVS").unwrap_or_else(|_| "today,trends,apps,settings".into());
            Shots {
                dir: PathBuf::from(d),
                prefix: std::env::var("TIMELYX_SHOT_PREFIX").unwrap_or_else(|_| "shot".into()),
                queue: names.split(',').filter_map(Nav::from_name).collect(),
                wait: 0,
                asked: false,
                done: Arc::new(AtomicBool::new(false)),
            }
        });
        let first = shots.as_ref().and_then(|s| s.queue.first().copied()).unwrap_or(Nav::Today);
        let ob_track = cfg.track_sites;
        Self {
            lang,
            dir,
            db,
            cat,
            snap: DaemonSnap::default(),
            nav: first,
            now,
            today: Local::now().date_naive(),
            version: 1,
            updater,
            demo,
            toast: None,
            today_view: Default::default(),
            trends_view: Default::default(),
            apps_view: Default::default(),
            cats_view: Default::default(),
            limits_view: Default::default(),
            settings_view: settings::SettingsState::new(),
            onboarding: onboarding::OnboardingState { track_sites: ob_track, autostart: true },
            refresh: Arc::new(Refresh::default()),
            last_refresh: f64::NEG_INFINITY,
            cfg_dirty,
            shots,
            modal_generation: 0,
            modal_was_open: false,
            cfg,
        }
    }

    pub fn s(&self) -> &'static S {
        strings::TABLE.get(self.lang)
    }

    pub fn h24(&self) -> bool {
        self.cfg.time_24h || self.lang != Lang::En
    }

    pub fn sunday_first(&self) -> bool {
        self.cfg.week_starts_sunday && self.lang == Lang::En
    }

    // ───────────────────────── data ─────────────────────────

    /// Rows that touch `[from, to)`, without what the settings hide.
    pub fn load_rows(&self, from: i64, to: i64) -> Vec<SessionRow> {
        let mut rows = self.db.rows(from, to).unwrap_or_default();
        if self.cfg.hide_system {
            rows.retain(|r| r.category != crate::cats::SYSTEM);
        }
        rows
    }

    /// Re-reads the catalog (programs, sites, categories, limits) and invalidates cached views.
    pub fn data_changed(&mut self) {
        self.cat = Catalog::load(&self.db);
        self.version += 1;
    }

    /// Makes the next frame ask the tracker again, so a pause or start shows up quickly.
    pub fn refresh_soon(&mut self) {
        self.last_refresh = f64::NEG_INFINITY;
    }

    pub fn tell_tracker(&self, msg: &'static str) {
        std::thread::spawn(move || {
            let _ = ipc::request("daemon", msg);
        });
    }

    fn start_refresh(&mut self, ctx: &egui::Context) {
        if self.refresh.busy.swap(true, Ordering::Relaxed) {
            return;
        }
        let r = self.refresh.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let snap = match ipc::request("daemon", "flush") {
                Some(_) => ipc::request("daemon", "state").map(|s| DaemonSnap::parse(&s)).unwrap_or_default(),
                None => DaemonSnap::default(),
            };
            if let Ok(mut g) = r.snap.lock() {
                *g = Some(snap);
            }
            r.ready.store(true, Ordering::Relaxed);
            r.busy.store(false, Ordering::Relaxed);
            ctx.request_repaint();
        });
    }

    /// The tracker's state as the demo wants to show it: running, with a made-up current activity.
    fn demo_snap(&self) -> DaemonSnap {
        DaemonSnap {
            running: true,
            avail: "ok".into(),
            state: "running".into(),
            paused_until: 0,
            live: Some(data::LiveSnap { app: "Visual Studio Code".into(), site: None, category: crate::cats::CODE.into(), start: self.now - 42 * 60 }),
        }
    }

    // ───────────────────────── settings ─────────────────────────

    pub fn touch_cfg(&mut self, ctx: &egui::Context) {
        self.cfg_dirty = Some(ctx.input(|i| i.time));
        ctx.request_repaint_after(Duration::from_millis(450));
    }

    /// Writes the settings and tells the tracker to re-read them.
    pub fn flush(&mut self) -> bool {
        if self.cfg_dirty.take().is_some() {
            let ok = self.cfg.save_to(&self.dir);
            if !self.demo {
                self.tell_tracker("reload");
            }
            return ok;
        }
        true
    }

    pub fn notify(&mut self, ctx: &egui::Context, text: String) {
        let now = ctx.input(|i| i.time);
        self.toast = Some(Toast { text, until: now + 4.0 });
    }

    pub fn go(&mut self, nav: Nav) {
        self.nav = nav;
    }

    fn track_window(&mut self, ctx: &egui::Context) {
        let (inner, outer, minimized, maximized) = ctx.input(|i| (i.viewport().inner_rect, i.viewport().outer_rect, i.viewport().minimized, i.viewport().maximized));
        if minimized == Some(true) || maximized == Some(true) {
            return;
        }
        if let (Some(inner), Some(outer)) = (inner, outer) {
            let w = Window { w: inner.width(), h: inner.height(), x: Some(outer.min.x), y: Some(outer.min.y) };
            let same = self.cfg.window.is_some_and(|c| (c.w - w.w).abs() < 1.0 && (c.h - w.h).abs() < 1.0 && c.x == w.x && c.y == w.y);
            if !same && w.w > 300.0 && w.h > 300.0 && self.shots.is_none() {
                self.cfg.window = Some(w);
                self.touch_cfg(ctx);
            }
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        let ctrl = Modifiers::COMMAND;
        for (i, key) in [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6].into_iter().enumerate() {
            if ctx.input_mut(|inp| inp.consume_key(ctrl, key)) {
                self.go(Nav::ALL[i]);
            }
        }
        if !egui::Popup::is_any_open(ctx) && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            if self.settings_view.confirm_delete {
                self.settings_view.confirm_delete = false;
            } else if self.apps_view.selected.is_some() {
                self.apps_view.selected = None;
            }
        }
    }

    // ───────────────────────── frame ─────────────────────────

    fn drive_screenshots(&mut self, ctx: &egui::Context) {
        let nav_name = self.nav_name();
        let Some(sh) = self.shots.as_mut() else { return };
        ctx.request_repaint();
        if sh.done.swap(false, Ordering::Relaxed) {
            sh.queue.remove(0);
            sh.asked = false;
            sh.wait = 0;
            match sh.queue.first() {
                Some(n) => self.nav = *n,
                None => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            }
            return;
        }
        sh.wait += 1;
        if sh.wait > 25 && !sh.asked {
            sh.asked = true;
            let path = sh.dir.join(format!("{}-{}.png", sh.prefix, nav_name));
            let _ = std::fs::create_dir_all(&sh.dir);
            let done = sh.done.clone();
            ctx.request_screenshot(move |img| {
                let mut raw = Vec::with_capacity(img.pixels.len() * 4);
                for px in &img.pixels {
                    raw.extend_from_slice(&[px.r(), px.g(), px.b(), px.a()]);
                }
                let _ = image::save_buffer(path, &raw, img.size[0] as u32, img.size[1] as u32, image::ColorType::Rgba8);
                done.store(true, Ordering::Relaxed);
            });
        }
    }
    fn nav_name(&self) -> &'static str {
        match self.nav {
            Nav::Today => "today",
            Nav::Trends => "trends",
            Nav::Apps => "apps",
            Nav::Categories => "categories",
            Nav::Limits => "limits",
            Nav::Settings => "settings",
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let t = ctx.input(|i| i.time);
        self.now = chrono::Utc::now().timestamp();
        self.today = Local::now().date_naive();
        for cmd in bus::drain() {
            match cmd {
                Cmd::Show => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
            }
        }
        if self.updater.is_busy() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if self.updater.should_exit() {
            self.flush();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        // The tracker's state and fresh numbers: every few seconds while the window is open.
        if self.cfg.onboarded {
            if self.demo {
                self.snap = self.demo_snap();
            } else {
                if self.refresh.ready.swap(false, Ordering::Relaxed) {
                    if let Some(s) = self.refresh.snap.lock().ok().and_then(|mut g| g.take()) {
                        self.snap = s;
                    }
                    self.data_changed();
                }
                if t - self.last_refresh >= 5.0 {
                    self.last_refresh = t;
                    self.start_refresh(ctx);
                }
            }
            ctx.request_repaint_after(Duration::from_secs(if self.nav == Nav::Today { 1 } else { 5 }));
        }
        if self.cfg_dirty.is_some_and(|d| t - d > 0.4) {
            self.flush();
        }
        if self.toast.as_ref().is_some_and(|x| t > x.until) {
            self.toast = None;
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            self.flush();
        }
        self.track_window(ctx);
        self.drive_screenshots(ctx);
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let p = Pal::of(ui.visuals().dark_mode);
        if !self.cfg.onboarded {
            self.onboarding_view(ui, &p);
            return;
        }
        self.handle_keys(&ctx);

        egui::Panel::left("sidebar")
            .exact_size(232.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(Frame::new().fill(p.sidebar).stroke(Stroke::NONE).inner_margin(Margin { left: 12, right: 12, top: 16, bottom: 12 }))
            .show(ui, |ui| self.sidebar(ui, &p));

        let area = ui.available_rect_before_wrap();
        ui.painter().rect_filled(area, 0.0, p.bg);
        ui.painter().vline(area.left(), area.y_range(), Stroke::new(1.0, p.line));
        let inner = Rect::from_min_max(area.min + egui::vec2(28.0, 24.0), area.max - egui::vec2(28.0, 8.0));
        let mut cui = ui.new_child(egui::UiBuilder::new().id_salt("central").max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
        cui.set_clip_rect(area);
        match self.nav {
            Nav::Today => self.today_view(&mut cui, &p),
            Nav::Trends => self.trends_view(&mut cui, &p),
            Nav::Apps => self.apps_view(&mut cui, &p),
            Nav::Categories => self.categories_view(&mut cui, &p),
            Nav::Limits => self.limits_view(&mut cui, &p),
            Nav::Settings => self.settings_view(&mut cui, &p),
        }
        let modal_open = self.settings_view.confirm_delete;
        if modal_open && !self.modal_was_open {
            self.modal_generation += 1;
        }
        self.modal_was_open = modal_open;
        self.settings_dialogs(&ctx, &p);
        self.toast_ui(&ctx, &p);
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.flush();
    }
}

// ───────────────────────── shared UI pieces ─────────────────────────

/// A page: a 24 px title with an optional subtitle, a right-aligned header, then a scrolling body in a column
/// no wider than 1120 px.
pub fn page(ui: &mut Ui, p: &Pal, title: &str, subtitle: Option<&str>, right: impl FnOnce(&mut Ui), body: impl FnOnce(&mut Ui)) {
    // The title and the body share one centred column, so they line up at every window width.
    let avail = ui.available_width();
    let avail_h = ui.available_height();
    let col_w = (avail - 14.0).clamp(300.0, 1120.0);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(((avail - 14.0 - col_w) / 2.0).max(0.0));
        ui.vertical(|ui| {
            ui.set_width(col_w);
            ui.set_height(avail_h);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(egui::RichText::new(title).font(theme::medium(24.0)).color(p.text));
                    if let Some(s) = subtitle {
                        theme::hint(ui, s, p);
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), right);
            });
            ui.add_space(16.0);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_width(col_w - 12.0);
                body(ui);
                ui.add_space(24.0);
            });
        });
    });
}

/// Dimmed backdrop plus a centred card. Returns `true` when the backdrop was clicked.
pub fn modal(ctx: &egui::Context, p: &Pal, name: &str, generation: u64, width: f32, add: impl FnOnce(&mut Ui)) -> bool {
    let name = format!("{name}-{generation}");
    let screen = ctx.content_rect();
    let dim = egui::Area::new(Id::unique((&name, "dim"))).order(Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
        let r = ui.allocate_rect(screen, Sense::click());
        ui.painter().rect_filled(screen, 0.0, egui::Color32::from_black_alpha(if p.dark { 150 } else { 90 }));
        r
    });
    egui::Area::new(Id::unique(&name)).order(Order::Foreground).anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, -10.0)).show(ctx, |ui| {
        Frame::new()
            .fill(p.card)
            .stroke(Stroke::new(1.0, p.line_hi))
            .corner_radius(theme::R_WINDOW)
            .inner_margin(Margin::same(24))
            .shadow(theme::shadow_for(p))
            .show(ui, |ui| {
                ui.set_width(width);
                add(ui);
            });
    });
    dim.inner.clicked()
}

impl App {
    fn sidebar(&mut self, ui: &mut Ui, p: &Pal) {
        let t = self.s();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let (mark, _) = ui.allocate_exact_size(egui::vec2(28.0, 36.0), Sense::hover());
            widgets::mark_glyph(ui.painter(), mark.center(), p.accent);
            ui.label(egui::RichText::new("Timelyx").font(theme::medium(18.0)).color(p.text));
        });
        ui.add_space(16.0);
        ui.spacing_mut().item_spacing.y = 2.0;
        let rows = [
            (Nav::Today, widgets::Icon::Today, t.nav_today),
            (Nav::Trends, widgets::Icon::Trends, t.nav_trends),
            (Nav::Apps, widgets::Icon::Apps, t.nav_apps),
            (Nav::Categories, widgets::Icon::Tag, t.nav_categories),
            (Nav::Limits, widgets::Icon::Limit, t.nav_limits),
        ];
        for (nav, ic, label) in rows {
            if nav_row(ui, p, ic, label, self.nav == nav).clicked() {
                self.go(nav);
            }
        }
        // Settings and the tracker status sit at the bottom.
        let h = ui.available_height();
        ui.add_space((h - 36.0 - 40.0).max(8.0));
        self.tracker_status(ui, p);
        if nav_row(ui, p, widgets::Icon::Gear, t.nav_settings, self.nav == Nav::Settings).clicked() {
            self.go(Nav::Settings);
        }
    }

    /// One quiet line: a coloured dot and what the tracker is doing.
    fn tracker_status(&self, ui: &mut Ui, p: &Pal) {
        let t = self.s();
        let (color, text) = if !self.snap.running {
            (p.negative, t.tracker_stopped)
        } else if self.snap.avail != "ok" {
            (p.warning, t.tray_unavailable)
        } else if self.snap.state == "paused" {
            (p.warning, t.now_paused)
        } else {
            (p.positive, t.tracker_running)
        };
        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 28.0), Sense::hover());
        ui.painter().circle_filled(egui::pos2(rect.left() + 20.0, rect.center().y), 4.0, color);
        ui.painter().text(egui::pos2(rect.left() + 36.0, rect.center().y), egui::Align2::LEFT_CENTER, text, theme::reg(12.0), p.text2);
    }
}

/// One navigation row.
fn nav_row(ui: &mut Ui, p: &Pal, ic: widgets::Icon, label: &str, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 36.0), Sense::click());
    if selected {
        ui.painter().rect(rect, theme::R_CONTROL as f32, widgets::selected_fill(p), Stroke::new(1.0, p.line_hi), egui::StrokeKind::Inside);
        ui.painter().hline(rect.x_range().shrink(8.0), rect.top() + 0.5, Stroke::new(1.0, p.top_light()));
    } else {
        ui.painter().rect_filled(rect, theme::R_CONTROL as f32, widgets::hover_fill(ui, resp.id, resp.hovered(), p));
    }
    let col = if selected { p.text } else { p.text2 };
    widgets::icon(ui.painter(), ic, egui::pos2(rect.left() + 20.0, rect.center().y), 16.0, if selected { p.text } else { p.muted });
    let font = if selected { theme::medium(14.0) } else { theme::reg(14.0) };
    ui.painter().text(egui::pos2(rect.left() + 36.0, rect.center().y), egui::Align2::LEFT_CENTER, label, font, col);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

impl App {
    fn toast_ui(&mut self, ctx: &egui::Context, p: &Pal) {
        let Some(toast) = &self.toast else { return };
        let text = toast.text.clone();
        let dark = Pal::dark();
        egui::Area::new(Id::unique("toast")).order(Order::Tooltip).anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -22.0)).show(ctx, |ui| {
            Frame::new()
                .fill(dark.raised)
                .stroke(Stroke::new(1.0, dark.line_hi))
                .corner_radius(theme::R_CARD)
                .inner_margin(Margin::symmetric(16, 8))
                .shadow(theme::shadow_for(p))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(&text).font(theme::reg(14.0)).color(dark.text));
                });
        });
    }

    pub fn open_modal_generation(&self) -> u64 {
        self.modal_generation
    }
}

/// Which items (programs and sites) are shown for a set of rows, largest first, with names resolved.
pub fn ranked(app: &App, rows: &[SessionRow], from: i64, to: i64) -> Vec<ItemStat> {
    let mut v = stats::by_item(rows, from, to);
    v.retain(|i| !app.cat.is_hidden(i.item));
    v
}

/// Midnight of today in local time.
pub fn today_start(now: i64) -> i64 {
    day_start(&Local, now)
}
