//! Settings (`config.json`) and the data folder. Both the tracker and the window read this file.

use crate::theme::ThemeMode;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// `%APPDATA%\Timelyx` (`~/.config/Timelyx` on Linux), or the folder from `TIMELYX_DATA_DIR` (tests and screenshots).
pub fn data_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("TIMELYX_DATA_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(d);
    }
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("Timelyx")
}

/// Is a data-dir override active? Test instances use their own names for the single-instance locks.
pub fn is_test_instance() -> bool {
    std::env::var_os("TIMELYX_DATA_DIR").is_some_and(|d| !d.is_empty())
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct Window {
    pub w: f32,
    pub h: f32,
    pub x: Option<f32>,
    pub y: Option<f32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Config {
    /// Interface language code (`en`, `ru`, `uk`); `None` until first resolved.
    pub language: Option<String>,
    pub theme: ThemeMode,
    /// The first-run screen has been confirmed. The tracker does nothing before that.
    pub onboarded: bool,
    /// Wanted state of "start with the computer"; the real state is read from the system.
    pub autostart: bool,
    /// Minutes without input before the time stops counting; 0 turns it off.
    pub idle_minutes: u32,
    /// Record the site of every browser tab (and not only "Chrome").
    pub track_sites: bool,
    pub site_exclusions: Vec<String>,
    /// Keep data for this many months; 0 keeps everything.
    pub retention_months: u32,
    /// Notify when a daily limit is reached.
    pub limit_notifications: bool,
    /// Leave the System category out of the charts and totals.
    pub hide_system: bool,
    pub check_updates: bool,
    pub week_starts_sunday: bool,
    pub time_24h: bool,
    pub window: Option<Window>,
    #[serde(skip)]
    pub fresh: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            language: None,
            theme: ThemeMode::System,
            onboarded: false,
            autostart: true,
            idle_minutes: 5,
            track_sites: true,
            site_exclusions: Vec::new(),
            retention_months: 0,
            limit_notifications: true,
            hide_system: false,
            check_updates: true,
            week_starts_sunday: false,
            time_24h: true,
            window: None,
            fresh: false,
        }
    }
}

/// Writes a file in one step: a temporary file next to it, then a rename.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

impl Config {
    pub fn load_from(dir: &Path) -> Self {
        let text = std::fs::read_to_string(dir.join("config.json")).ok();
        let mut cfg: Config = text.as_deref().and_then(|s| serde_json::from_str(s.trim_start_matches('\u{feff}')).ok()).unwrap_or_default();
        cfg.fresh = text.is_none();
        cfg.idle_minutes = cfg.idle_minutes.min(240);
        cfg.retention_months = cfg.retention_months.min(120);
        cfg
    }

    /// Writes the settings. `false` if they could not be written.
    pub fn save_to(&self, dir: &Path) -> bool {
        match serde_json::to_vec_pretty(self) {
            Ok(json) => write_atomic(&dir.join("config.json"), &json).is_ok(),
            Err(_) => false,
        }
    }

    pub fn idle_secs(&self) -> i64 {
        self.idle_minutes as i64 * 60
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{self, Lang};

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("timelyx-cfg-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn missing_config_is_fresh_with_defaults() {
        let dir = tmp("fresh");
        let c = Config::load_from(&dir);
        assert!(c.fresh && !c.onboarded);
        assert_eq!(c.theme, ThemeMode::System);
        assert!(c.autostart && c.track_sites);
        assert_eq!(c.idle_minutes, 5);
        assert_eq!(c.idle_secs(), 300);
        assert_eq!(c.retention_months, 0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = tmp("rt");
        let mut c = Config::default();
        c.language = Some("uk".into());
        c.theme = ThemeMode::Dark;
        c.site_exclusions = vec!["bank".into()];
        c.idle_minutes = 10;
        c.retention_months = 6;
        c.onboarded = true;
        assert!(c.save_to(&dir));
        let l = Config::load_from(&dir);
        assert!(!l.fresh);
        assert_eq!(l.language.as_deref(), Some("uk"));
        assert_eq!(l.theme, ThemeMode::Dark);
        assert_eq!(l.site_exclusions, vec!["bank".to_string()]);
        assert_eq!((l.idle_minutes, l.retention_months, l.onboarded), (10, 6, true));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn bom_unknown_fields_and_out_of_range_values_are_tolerated() {
        let dir = tmp("bom");
        std::fs::write(dir.join("config.json"), "\u{feff}{\"theme\":\"light\",\"future_key\":1,\"idle_minutes\":99999}").unwrap();
        let c = Config::load_from(&dir);
        assert_eq!(c.theme, ThemeMode::Light);
        assert_eq!(c.idle_minutes, 240);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn language_order_config_then_marker_then_english() {
        assert_eq!(i18n::resolve(Some("ru")), Lang::Ru);
        assert_eq!(i18n::resolve(Some("uk")), Lang::Uk);
        if i18n::marker_language().is_none() {
            assert_eq!(i18n::resolve(None), Lang::En);
        }
    }
}
