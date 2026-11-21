#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Timelyx: an automatic, local activity tracker.
//!
//! One executable, two processes:
//! - the **tracker** (`--background`, started with the computer) records the foreground program and, in browsers, the
//!   site. It draws nothing, owns the tray icon and uses almost no memory;
//! - the **window** (the default) shows the charts. It starts when you open Timelyx and ends when you close it.

mod app;
mod bus;
mod cats;
mod config;
mod daemon;
mod db;
mod demo;
mod engine;
mod fmt;
#[allow(dead_code)]
mod i18n;
mod ipc;
#[allow(dead_code)]
mod nimvoro;
mod notify;
mod rules;
#[allow(dead_code)]
mod selfupdate;
mod sites;
mod stats;
mod strings;
mod support;
#[allow(dead_code)]
mod theme;
mod tracker;
#[allow(dead_code)]
mod x11logic;
#[cfg(unix)]
mod x11src;
#[cfg(windows)]
mod winsrc;
#[allow(dead_code)]
mod update_ui;
mod tray_common;
#[cfg(windows)]
mod platform;
#[cfg(not(windows))]
#[path = "platform_linux.rs"]
mod platform;
#[cfg(windows)]
mod tray;
#[cfg(not(windows))]
#[path = "tray_linux.rs"]
mod tray;

use config::Config;
use eframe::egui;

/// The repository, kept in one place (About page and the update check).
pub const REPO_URL: &str = "https://github.com/GrigoryyKach/timelyx";
pub const ICON_PNG: &[u8] = include_bytes!("../assets/icon.png");

fn main() {
    selfupdate::startup("timelyx");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = config::data_dir();
    // `--ipc <request>`: asks the running tracker and prints its answer (used by the tests in tools/).
    if let Some(i) = args.iter().position(|a| a == "--ipc") {
        match ipc::request("daemon", &args[i + 1..].join(" ")) {
            Some(r) => println!("{r}"),
            None => println!("not running"),
        }
        return;
    }
    if args.iter().any(|a| a == "--background" || a == "--minimized") {
        run_tracker(dir);
    } else if let Err(e) = run_window(dir, &args) {
        eprintln!("timelyx: {e}");
    }
}

/// The resident part: tracker, tray and the channel the window talks to.
fn run_tracker(dir: std::path::PathBuf) {
    let cfg = Config::load_from(&dir);
    // Nothing is recorded before the first-run screen has been confirmed.
    if !cfg.onboarded {
        return;
    }
    // The endpoint is also the single-instance lock.
    if !ipc::serve("daemon", |m| daemon::with(|d| d.handle(m)).unwrap_or_else(|| "error".into())) {
        return;
    }
    #[cfg(windows)]
    let availability = daemon::Availability::Ok;
    #[cfg(unix)]
    let availability = x11src::availability();
    if !daemon::init(dir.clone(), cfg, availability) {
        let _ = std::fs::write(dir.join("tracker.log"), "could not open the database\n");
        return;
    }
    notify::register_identity();
    let state = daemon::with(|d| d.tray_state());
    #[cfg(windows)]
    {
        winsrc::run(|| {
            if let Some(s) = &state {
                tray::create(s, ICON_PNG);
            }
        });
        daemon::with(|d| d.on_shutdown());
        tray::remove();
    }
    #[cfg(unix)]
    {
        x11src::run(|| {
            if let Some(s) = &state {
                tray::create(s, ICON_PNG);
            }
        });
        daemon::with(|d| d.on_shutdown());
        tray::remove();
    }
}

/// Asks the tracker to quit and waits a moment for it to leave (before an update replaces the files).
fn stop_tracker() {
    if ipc::request("daemon", "quit").is_some() {
        for _ in 0..30 {
            if !ipc::is_running("daemon") {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}

fn run_window(dir: std::path::PathBuf, args: &[String]) -> eframe::Result<()> {
    // One window per data folder: a second launch brings the first one forward.
    let served = ipc::serve("ui", |m| {
        if m == "show" {
            bus::push(bus::Cmd::Show);
        }
        "ok".into()
    });
    if !served {
        platform::allow_foreground();
        ipc::request("ui", "show");
        return Ok(());
    }

    let demo = args.iter().any(|a| a == "--demo") || std::env::var("TIMELYX_DEMO").is_ok_and(|v| v == "1");
    // Demo data only ever goes into a test data folder, never into the real one.
    if demo && config::is_test_instance() {
        demo::fill(&dir);
    }
    let cfg = Config::load_from(&dir);
    // The window starts the tracker if it is not running (never for demo data or a test run without one).
    if cfg.onboarded && !demo && std::env::var_os("TIMELYX_NO_TRACKER").is_none() && !ipc::is_running("daemon") {
        platform::spawn_background();
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title(platform::WINDOW_TITLE)
        .with_app_id("local.timelyx")
        .with_min_inner_size([960.0, 620.0])
        .with_icon(std::sync::Arc::new(app_icon()));
    let mut centered = true;
    match cfg.window {
        Some(w) if w.w >= 960.0 && w.h >= 620.0 => {
            viewport = viewport.with_inner_size([w.w, w.h]);
            if let (Some(x), Some(y)) = (w.x, w.y) {
                if x > -20000.0 && y > -20000.0 && x < 20000.0 && y < 20000.0 {
                    viewport = viewport.with_position([x, y]);
                    centered = false;
                }
            }
        }
        _ => viewport = viewport.with_inner_size([1240.0, 800.0]),
    }
    let options = eframe::NativeOptions { viewport, centered, ..Default::default() };
    let updater_stop: fn() = stop_tracker;
    eframe::run_native(platform::WINDOW_TITLE, options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, cfg, dir, updater_stop)))))
}

/// Window icon from `assets/icon.png` (the same artwork that is embedded in the exe).
fn app_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(ICON_PNG).unwrap_or_default()
}
