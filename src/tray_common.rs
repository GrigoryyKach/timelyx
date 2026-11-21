//! What the tray menu does, and the state the daemon hands to the tray. Shared by the Windows and Linux trays.

use crate::i18n::Lang;

/// Everything the tray needs to draw itself.
#[derive(Clone, Debug, PartialEq)]
pub struct TrayState {
    pub lang: Lang,
    pub tooltip: String,
    /// Tracking is paused (or unavailable): the icon shows it.
    pub paused: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Open,
    /// Pause for this many seconds; `None` until resumed.
    Pause(Option<i64>),
    Resume,
    Support,
    Exit,
}

pub fn perform(a: Action) {
    match a {
        Action::Open => crate::platform::spawn_ui(),
        Action::Pause(secs) => {
            crate::daemon::with(|d| d.pause(secs));
        }
        Action::Resume => {
            crate::daemon::with(|d| d.unpause());
        }
        Action::Support => crate::support::open(),
        Action::Exit => crate::daemon::request_quit(),
    }
}

/// The tray icon while tracking is paused: the same tile, dimmed, with a pause mark in the middle.
pub fn paused_rgba(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
    let mut out = rgba.to_vec();
    for px in out.chunks_exact_mut(4) {
        let l = (0.299 * px[0] as f32 + 0.587 * px[1] as f32 + 0.114 * px[2] as f32) * 0.5;
        px[0] = l as u8;
        px[1] = l as u8;
        px[2] = l as u8;
    }
    // Two bars, each about 1/8 of the width, in the middle.
    let (bar_w, bar_h) = ((w / 8).max(2), (h / 2).max(4));
    let top = (h - bar_h) / 2;
    for (i, left) in [w / 2 - bar_w - bar_w / 2, w / 2 + bar_w / 2].into_iter().enumerate() {
        let _ = i;
        for y in top..top + bar_h {
            for x in left..(left + bar_w).min(w) {
                let o = ((y * w + x) * 4) as usize;
                if o + 3 < out.len() && out[o + 3] > 0 {
                    out[o] = 0xed;
                    out[o + 1] = 0xed;
                    out[o + 2] = 0xed;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paused_icon_keeps_size_and_alpha_and_draws_bars() {
        let (w, h) = (32u32, 32u32);
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for px in rgba.chunks_exact_mut(4) {
            px.copy_from_slice(&[0xe8, 0x66, 0x9a, 0xff]);
        }
        let p = paused_rgba(&rgba, w, h);
        assert_eq!(p.len(), rgba.len());
        assert!(p.chunks_exact(4).all(|px| px[3] == 0xff));
        // The corners are dimmed grey, the middle has the light bar.
        assert!(p[0] < 0x80 && p[0] == p[1] && p[1] == p[2]);
        let bars = p.chunks_exact(4).filter(|px| px[0] == 0xed).count();
        assert!(bars > 20);
    }
}
