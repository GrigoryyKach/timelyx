//! Charts painted by hand in the suite style: hairlines, flat fills, no gradients or shadows,
//! figures in Plex Mono. Colours are always a meaning (a category), never decoration.

use super::widgets::arc_points;
use crate::fmt;
use crate::i18n::Lang;
use crate::theme::{self, Pal};
use eframe::egui::{epaint::PathStroke, Align2, Color32, CornerRadius, Painter, Rect, RichText, Sense, Shape, Stroke, StrokeKind, Ui, pos2, vec2};

/// One block of the day timeline.
pub struct Seg {
    pub start: i64,
    pub end: i64,
    pub color: Color32,
    pub name: String,
}

/// A "nice" axis maximum and step for `max` seconds: at most four steps, steps of 15 min, 30 min, 1 h, 2 h ...
pub fn nice_axis(max: i64) -> (i64, i64) {
    const STEPS: [i64; 10] = [900, 1800, 3600, 7200, 10_800, 14_400, 21_600, 28_800, 43_200, 86_400];
    let max = max.max(1);
    for s in STEPS {
        let n = (max + s - 1) / s;
        if n <= 4 {
            return (n.max(1) * s, s);
        }
    }
    (86_400, 21_600)
}

fn tip_title(ui: &mut Ui, text: &str, p: &Pal) {
    ui.label(RichText::new(text).font(theme::medium(13.0)).color(p.text));
}

fn tip_meta(ui: &mut Ui, text: &str, p: &Pal) {
    ui.label(RichText::new(text).font(theme::mono(12.0)).color(p.text2));
}

/// The 24 hour strip of one day: a block per session in its category colour, hairline ticks every three hours.
#[allow(clippy::too_many_arguments)]
pub fn timeline(ui: &mut Ui, p: &Pal, lang: Lang, h24: bool, day_start: i64, day_end: i64, segs: &[Seg], now: Option<i64>) {
    let w = ui.available_width();
    let strip_h = 44.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, strip_h + 22.0), Sense::hover());
    let strip = Rect::from_min_size(rect.min, vec2(w, strip_h));
    let painter = ui.painter();
    painter.rect(strip, 6.0, p.raised, Stroke::new(1.0, p.line), StrokeKind::Inside);
    let span = (day_end - day_start).max(1) as f32;
    let x_of = |t: i64| strip.left() + (t - day_start) as f32 / span * strip.width();
    for h in 0..=8 {
        let x = strip.left() + strip.width() * h as f32 / 8.0;
        if h > 0 && h < 8 {
            painter.vline(x, strip.y_range(), Stroke::new(1.0, p.line));
        }
        let label = format!("{}", h * 3);
        let (anchor, lx) = match h {
            0 => (Align2::LEFT_TOP, x),
            8 => (Align2::RIGHT_TOP, x),
            _ => (Align2::CENTER_TOP, x),
        };
        painter.text(pos2(lx, strip.bottom() + 6.0), anchor, label, theme::mono(11.0), p.muted);
    }
    let inner = strip.shrink2(vec2(0.0, 1.0));
    for s in segs {
        let (x0, x1) = (x_of(s.start).max(strip.left()), x_of(s.end).min(strip.right()));
        if x1 <= x0 {
            continue;
        }
        let r = Rect::from_min_max(pos2(x0, inner.top()), pos2(x0.max(x1 - 0.0).max(x0 + 1.0), inner.bottom()));
        painter.rect_filled(r, 0.0, s.color);
    }
    if let Some(n) = now {
        if n >= day_start && n <= day_end {
            let x = x_of(n);
            painter.vline(x, strip.y_range().expand(3.0), Stroke::new(1.5, p.text));
        }
    }
    if let Some(pos) = resp.hover_pos() {
        if strip.contains(pos) {
            let hit = segs.iter().find(|s| pos.x >= x_of(s.start) - 1.5 && pos.x <= x_of(s.end).max(x_of(s.start) + 1.0) + 1.5);
            if let Some(s) = hit {
                painter.vline(pos.x, strip.y_range(), Stroke::new(1.0, p.line_hi));
                resp.on_hover_ui_at_pointer(|ui| {
                    tip_title(ui, &s.name, p);
                    tip_meta(ui, &format!("{} – {} · {}", fmt::clock(s.start, h24), fmt::clock(s.end, h24), fmt::duration(lang, s.end - s.start)), p);
                });
            }
        }
    }
}

/// A ring of category shares. `fracs` add up to at most 1. The two lines go in the middle.
pub fn donut(ui: &mut Ui, p: &Pal, size: f32, thickness: f32, slices: &[(Color32, f64)], top: &str, sub: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let c = rect.center();
    let r = size / 2.0 - thickness / 2.0 - 1.0;
    let painter = ui.painter();
    painter.add(Shape::line(arc_points(c, r, 0.0, std::f32::consts::TAU, 96), PathStroke::new(thickness, p.raised)));
    let total: f64 = slices.iter().map(|s| s.1).sum();
    if total > 0.0 {
        let gap = if slices.len() > 1 { 0.035 } else { 0.0 };
        let mut a = 0.0f32;
        for (col, frac) in slices {
            let sweep = (*frac / total) as f32 * std::f32::consts::TAU;
            let (a0, a1) = (a + gap / 2.0, a + sweep - gap / 2.0);
            a += sweep;
            if a1 <= a0 {
                continue;
            }
            let steps = ((a1 - a0) * 24.0).ceil().max(2.0) as usize;
            painter.add(Shape::line(arc_points(c, r, a0, a1, steps), PathStroke::new(thickness, *col)));
        }
    }
    painter.text(c + vec2(0.0, -7.0), Align2::CENTER_CENTER, top, theme::mono(14.0), p.text);
    painter.text(c + vec2(0.0, 12.0), Align2::CENTER_CENTER, sub, theme::reg(12.0), p.muted);
}

/// One day (or one bucket) of a stacked bar chart.
pub struct BarDay {
    /// Under the bar: a weekday name or a day number; empty to skip.
    pub label: String,
    /// First line of the tooltip.
    pub title: String,
    pub by_cat: Vec<(String, i64)>,
    pub total: i64,
    pub highlight: bool,
}

/// Stacked bars per day with a seconds axis, hairline grid and a hover tooltip with the split by category.
#[allow(clippy::too_many_arguments)]
pub fn stacked_bars(
    ui: &mut Ui,
    p: &Pal,
    lang: Lang,
    height: f32,
    days: &[BarDay],
    color_of: &dyn Fn(&str) -> Color32,
    name_of: &dyn Fn(&str) -> String,
) {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, height), Sense::hover());
    let gutter = 44.0;
    let plot = Rect::from_min_max(pos2(rect.left() + gutter, rect.top() + 6.0), pos2(rect.right(), rect.bottom() - 22.0));
    let painter = ui.painter();
    let max = days.iter().map(|d| d.total).max().unwrap_or(0);
    let (axis_max, step) = nice_axis(max);
    let y_of = |s: i64| plot.bottom() - s as f32 / axis_max as f32 * plot.height();
    let mut g = 0;
    while g <= axis_max {
        let y = y_of(g);
        painter.hline(plot.x_range(), y, Stroke::new(1.0, if g == 0 { p.line_hi } else { p.line }));
        let label = if g == 0 { String::new() } else { fmt::duration_short(lang, g) };
        painter.text(pos2(plot.left() - 8.0, y), Align2::RIGHT_CENTER, label, theme::mono(11.0), p.muted);
        g += step;
    }
    let n = days.len().max(1);
    let slot = plot.width() / n as f32;
    let bar_w = (slot * 0.62).clamp(3.0, 56.0);
    let hover_idx = resp.hover_pos().filter(|pos| plot.expand2(vec2(0.0, 6.0)).contains(*pos)).map(|pos| (((pos.x - plot.left()) / slot) as usize).min(n - 1));
    for (i, d) in days.iter().enumerate() {
        let cx = plot.left() + slot * (i as f32 + 0.5);
        if Some(i) == hover_idx {
            painter.rect_filled(Rect::from_min_max(pos2(cx - slot / 2.0 + 1.0, plot.top()), pos2(cx + slot / 2.0 - 1.0, plot.bottom())), 4.0, theme::alpha(p.raised, 200));
        }
        let mut y = plot.bottom();
        let last = d.by_cat.iter().rposition(|c| c.1 > 0);
        for (k, (cat, secs)) in d.by_cat.iter().enumerate() {
            if *secs <= 0 {
                continue;
            }
            let h = (*secs as f32 / axis_max as f32 * plot.height()).max(1.0);
            let r = Rect::from_min_max(pos2(cx - bar_w / 2.0, y - h), pos2(cx + bar_w / 2.0, y));
            let radius = if Some(k) == last { CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 } } else { CornerRadius::ZERO };
            painter.rect_filled(r, radius, color_of(cat));
            y -= h;
        }
        if !d.label.is_empty() {
            let col = if d.highlight { p.text } else { p.muted };
            painter.text(pos2(cx, plot.bottom() + 6.0), Align2::CENTER_TOP, &d.label, theme::mono(11.0), col);
        }
    }
    if let Some(i) = hover_idx {
        let d = &days[i];
        resp.on_hover_ui_at_pointer(|ui| {
            tip_title(ui, &d.title, p);
            tip_meta(ui, &fmt::duration(lang, d.total), p);
            for (cat, secs) in d.by_cat.iter().filter(|c| c.1 > 0).take(6) {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(8.0, 14.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 4.0, color_of(cat));
                    ui.label(RichText::new(name_of(cat)).font(theme::reg(12.0)).color(p.text2));
                    ui.label(RichText::new(fmt::duration(lang, *secs)).font(theme::mono(12.0)).color(p.text));
                });
            }
        });
    }
}

/// Weekday by hour: darker means more time. `order` lists the weekday rows to draw (Monday = 0).
pub fn heatmap(ui: &mut Ui, p: &Pal, lang: Lang, h24: bool, grid: &[[i64; 24]; 7], order: [usize; 7]) {
    let w = ui.available_width();
    let gutter = 40.0;
    let gap = 2.0;
    let cell_w = ((w - gutter) / 24.0 - gap).max(4.0);
    let cell_h = 20.0;
    let total_h = 16.0 + 7.0 * (cell_h + gap);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, total_h), Sense::hover());
    let painter = ui.painter();
    let max = grid.iter().flatten().copied().max().unwrap_or(0).max(1);
    for hh in (0..24).step_by(3) {
        let x = rect.left() + gutter + hh as f32 * (cell_w + gap);
        painter.text(pos2(x, rect.top()), Align2::LEFT_TOP, format!("{hh}"), theme::mono(11.0), p.muted);
    }
    let mut hover = None;
    for (row, wd) in order.iter().enumerate() {
        let y = rect.top() + 16.0 + row as f32 * (cell_h + gap);
        painter.text(pos2(rect.left(), y + cell_h / 2.0), Align2::LEFT_CENTER, fmt::weekday_short(lang, *wd), theme::reg(12.0), p.muted);
        for hh in 0..24 {
            let x = rect.left() + gutter + hh as f32 * (cell_w + gap);
            let cell = Rect::from_min_size(pos2(x, y), vec2(cell_w, cell_h));
            let v = grid[*wd][hh];
            let t = (v as f32 / max as f32).powf(0.85);
            painter.rect_filled(cell, 3.0, if v > 0 { theme::mix(p.raised_hi, p.accent, t) } else { p.raised });
            if resp.hover_pos().is_some_and(|pos| cell.expand(gap / 2.0).contains(pos)) {
                hover = Some((*wd, hh, v));
                painter.rect_stroke(cell, 3.0, Stroke::new(1.0, p.text), StrokeKind::Outside);
            }
        }
    }
    if let Some((wd, hh, v)) = hover {
        resp.on_hover_ui_at_pointer(|ui| {
            tip_title(ui, &format!("{} {}–{}", fmt::weekday_short(lang, wd), fmt::clock_hm(hh as u32, 0, h24), fmt::clock_hm(((hh + 1) % 24) as u32, 0, h24)), p);
            tip_meta(ui, &fmt::duration(lang, v), p);
        });
    }
}

/// A flat horizontal bar on a track.
pub fn bar(painter: &Painter, rect: Rect, frac: f32, color: Color32, p: &Pal) {
    painter.rect_filled(rect, 2.0, p.raised);
    let w = (rect.width() * frac.clamp(0.0, 1.0)).max(if frac > 0.0 { 2.0 } else { 0.0 });
    if w > 0.0 {
        painter.rect_filled(Rect::from_min_size(rect.min, vec2(w, rect.height())), 2.0, color);
    }
}

/// Legend row: dot, name on the left; time and share in fixed columns on the right.
pub fn legend_row(ui: &mut Ui, p: &Pal, lang: Lang, color: Color32, name: &str, secs: i64, share: Option<f64>) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
    let painter = ui.painter();
    painter.circle_filled(pos2(rect.left() + 6.0, rect.center().y), 4.0, color);
    let mut right = rect.right();
    if let Some(s) = share {
        painter.text(pos2(right, rect.center().y), Align2::RIGHT_CENTER, format!("{:.0}%", s * 100.0), theme::mono(12.0), p.muted);
    }
    right -= 40.0;
    let time = painter.layout_no_wrap(fmt::duration(lang, secs), theme::mono(12.0), p.text);
    let tw = time.size().x.max(76.0);
    painter.galley(pos2(right - time.size().x, rect.center().y - time.size().y / 2.0), time, p.text);
    let name_w = (right - tw - 8.0) - (rect.left() + 18.0);
    let name = super::widgets::fit_text(painter, name, &theme::reg(13.0), name_w);
    painter.text(pos2(rect.left() + 18.0, rect.center().y), Align2::LEFT_CENTER, name, theme::reg(13.0), p.text2);
}

/// A single wrappable legend item: dot, name and time in one allocation, so `horizontal_wrapped` can break lines.
pub fn legend_chip(ui: &mut Ui, p: &Pal, lang: Lang, color: Color32, name: &str, secs: i64) {
    let g_name = ui.painter().layout_no_wrap(name.to_string(), theme::reg(12.5), p.text2);
    let g_time = ui.painter().layout_no_wrap(fmt::duration(lang, secs), theme::mono(12.0), p.muted);
    let w = 16.0 + g_name.size().x + 8.0 + g_time.size().x;
    let (rect, _) = ui.allocate_exact_size(vec2(w, 20.0), Sense::hover());
    let painter = ui.painter();
    painter.circle_filled(pos2(rect.left() + 4.0, rect.center().y), 4.0, color);
    painter.galley(pos2(rect.left() + 16.0, rect.center().y - g_name.size().y / 2.0), g_name.clone(), p.text2);
    painter.galley(pos2(rect.left() + 16.0 + g_name.size().x + 8.0, rect.center().y - g_time.size().y / 2.0), g_time, p.muted);
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_is_nice_and_has_at_most_four_steps() {
        assert_eq!(nice_axis(0), (900, 900));
        assert_eq!(nice_axis(600), (900, 900));
        assert_eq!(nice_axis(3600), (3600, 900));
        assert_eq!(nice_axis(2 * 3600 + 1), (3 * 3600, 3600).min((3 * 3600, 3600)));
        let (m, s) = nice_axis(7 * 3600 + 30 * 60);
        assert_eq!((m, s), (8 * 3600, 7200));
        for max in [1, 500, 1799, 3600, 10_000, 30_000, 70_000, 200_000] {
            let (m, s) = nice_axis(max);
            assert!(m >= max.min(86_400) && m / s <= 4 && m % s == 0, "{max}: {m} {s}");
        }
    }
}
