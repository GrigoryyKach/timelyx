//! Small drawing helpers: icons, buttons, segmented control, dropdowns, inputs.

use crate::theme::{self, Btn, Pal};
use eframe::egui::{
    self, epaint::PathStroke, Align2, Color32, CursorIcon, Margin, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui,
    Vec2, pos2, vec2,
};

pub fn rgb(c: u32) -> Color32 {
    theme::hex(c)
}

/// Text colour that reads well on top of `bg`.
pub fn on_color(bg: Color32) -> Color32 {
    let l = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
    if l > 150.0 { theme::hex(0x141415) } else { Color32::WHITE }
}

/// Fill of the selected row on a surface that is not a card: raised on dark, the card white on light.
pub fn selected_fill(p: &Pal) -> Color32 {
    if p.dark { p.raised } else { p.card }
}

/// Hover fill that fades in, for rows drawn by hand.
pub fn hover_fill(ui: &Ui, id: egui::Id, hovered: bool, p: &Pal) -> Color32 {
    theme::alpha(selected_fill(p), (theme::fade(ui, id.with("hover"), hovered) * 255.0) as u8)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Today,
    Calendar,
    Trends,
    Apps,
    Tag,
    Limit,
    Gear,
    Left,
    Right,
    Close,
    Trash,
    Edit,
    Check,
}

/// Points of an arc around `c`, angles in radians measured clockwise from 12 o'clock.
pub fn arc_points(c: Pos2, r: f32, a0: f32, a1: f32, steps: usize) -> Vec<Pos2> {
    (0..=steps)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / steps as f32;
            c + vec2(a.sin(), -a.cos()) * r
        })
        .collect()
}

/// Draws a line icon centred at `c` within a `size` box. Stroke 1.5, never its own colour.
pub fn icon(p: &Painter, ic: Icon, c: Pos2, size: f32, col: Color32) {
    let s = size / 2.0;
    let st = Stroke::new(1.5, col);
    let line = |a: Pos2, b: Pos2| p.line_segment([a, b], st);
    match ic {
        Icon::Today => {
            p.circle_stroke(c, s * 0.82, st);
            line(c, c + vec2(0.0, -s * 0.5));
            line(c, c + vec2(s * 0.38, s * 0.22));
        }
        Icon::Calendar => {
            let r = Rect::from_center_size(c + vec2(0.0, s * 0.1), vec2(s * 1.7, s * 1.5));
            p.rect_stroke(r, 2.5, st, StrokeKind::Inside);
            line(pos2(r.left(), r.top() + s * 0.5), pos2(r.right(), r.top() + s * 0.5));
            line(pos2(r.left() + s * 0.5, r.top() - s * 0.3), pos2(r.left() + s * 0.5, r.top() + s * 0.2));
            line(pos2(r.right() - s * 0.5, r.top() - s * 0.3), pos2(r.right() - s * 0.5, r.top() + s * 0.2));
        }
        Icon::Trends => {
            let base = c.y + s * 0.8;
            line(pos2(c.x - s * 0.85, base), pos2(c.x + s * 0.85, base));
            for (dx, h) in [(-0.55, 0.7), (0.0, 1.3), (0.55, 1.0)] {
                let x = c.x + s * dx;
                p.rect_filled(Rect::from_min_max(pos2(x - s * 0.17, base - s * h), pos2(x + s * 0.17, base - 1.5)), 1.5, col);
            }
        }
        Icon::Apps => {
            for (dx, dy) in [(-0.45, -0.45), (0.45, -0.45), (-0.45, 0.45), (0.45, 0.45)] {
                let r = Rect::from_center_size(c + vec2(s * dx, s * dy), vec2(s * 0.7, s * 0.7));
                p.rect_stroke(r, 2.0, st, StrokeKind::Inside);
            }
        }
        Icon::Tag => {
            let pts = vec![c + vec2(-s * 0.8, -s * 0.1), c + vec2(-s * 0.1, -s * 0.8), c + vec2(s * 0.8, -s * 0.8), c + vec2(s * 0.8, s * 0.1), c + vec2(s * 0.1, s * 0.8)];
            p.add(Shape::closed_line(pts, st));
            p.circle_filled(c + vec2(s * 0.35, -s * 0.35), s * 0.14, col);
        }
        Icon::Limit => {
            let top = c.y - s * 0.8;
            let bot = c.y + s * 0.8;
            line(pos2(c.x - s * 0.6, top), pos2(c.x + s * 0.6, top));
            line(pos2(c.x - s * 0.6, bot), pos2(c.x + s * 0.6, bot));
            p.add(Shape::line(vec![pos2(c.x - s * 0.45, top), pos2(c.x - s * 0.45, top + s * 0.3), pos2(c.x, c.y), pos2(c.x - s * 0.45, bot - s * 0.3), pos2(c.x - s * 0.45, bot)], PathStroke::new(1.5, col)));
            p.add(Shape::line(vec![pos2(c.x + s * 0.45, top), pos2(c.x + s * 0.45, top + s * 0.3), pos2(c.x, c.y), pos2(c.x + s * 0.45, bot - s * 0.3), pos2(c.x + s * 0.45, bot)], PathStroke::new(1.5, col)));
        }
        Icon::Gear => {
            p.circle_stroke(c, s * 0.38, st);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let d = vec2(a.cos(), a.sin());
                line(c + d * (s * 0.68), c + d * (s * 0.98));
            }
            p.circle_stroke(c, s * 0.7, st);
        }
        Icon::Left => {
            line(c + vec2(s * 0.25, -s * 0.6), c + vec2(-s * 0.3, 0.0));
            line(c + vec2(-s * 0.3, 0.0), c + vec2(s * 0.25, s * 0.6));
        }
        Icon::Right => {
            line(c + vec2(-s * 0.25, -s * 0.6), c + vec2(s * 0.3, 0.0));
            line(c + vec2(s * 0.3, 0.0), c + vec2(-s * 0.25, s * 0.6));
        }
        Icon::Close => {
            line(c + vec2(-s * 0.5, -s * 0.5), c + vec2(s * 0.5, s * 0.5));
            line(c + vec2(-s * 0.5, s * 0.5), c + vec2(s * 0.5, -s * 0.5));
        }
        Icon::Trash => {
            line(c + vec2(-s * 0.7, -s * 0.5), c + vec2(s * 0.7, -s * 0.5));
            line(c + vec2(-s * 0.2, -s * 0.5), c + vec2(-s * 0.2, -s * 0.8));
            line(c + vec2(s * 0.2, -s * 0.5), c + vec2(s * 0.2, -s * 0.8));
            line(c + vec2(-s * 0.2, -s * 0.8), c + vec2(s * 0.2, -s * 0.8));
            p.rect_stroke(Rect::from_min_max(c + vec2(-s * 0.5, -s * 0.5), c + vec2(s * 0.5, s * 0.85)), 2.0, st, StrokeKind::Inside);
        }
        Icon::Edit => {
            line(c + vec2(-s * 0.7, s * 0.7), c + vec2(s * 0.6, -s * 0.6));
            line(c + vec2(-s * 0.7, s * 0.7), c + vec2(-s * 0.7, s * 0.3));
            line(c + vec2(-s * 0.7, s * 0.7), c + vec2(-s * 0.3, s * 0.7));
            line(c + vec2(s * 0.2, -s * 0.8), c + vec2(s * 0.8, -s * 0.2));
        }
        Icon::Check => {
            line(c + vec2(-s * 0.55, 0.0), c + vec2(-s * 0.15, s * 0.45));
            line(c + vec2(-s * 0.15, s * 0.45), c + vec2(s * 0.6, -s * 0.45));
        }
    }
}

/// Square icon button; a raised fill fades in on hover.
pub fn icon_button(ui: &mut Ui, ic: Icon, size: f32, p: &Pal) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let t = theme::fade(ui, resp.id.with("hover"), resp.hovered());
    ui.painter().rect_filled(rect, theme::R_CONTROL, theme::alpha(p.raised, (t * 255.0) as u8));
    icon(ui.painter(), ic, rect.center(), size * 0.55, theme::mix(p.muted, p.text, t));
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Like [`icon_button`], but can be shown disabled: muted, no hover, no click.
pub fn icon_button_on(ui: &mut Ui, ic: Icon, size: f32, enabled: bool, p: &Pal) -> Response {
    if enabled {
        return icon_button(ui, ic, size, p);
    }
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    icon(ui.painter(), ic, rect.center(), size * 0.55, theme::alpha(p.muted, 90));
    resp
}

/// The Timelyx mark: a clock with one arc segment, in the accent colour. Flat, no tile.
pub fn mark_glyph(painter: &Painter, c: Pos2, col: Color32) {
    let st = Stroke::new(1.5, col);
    let r = 8.5;
    // 270 degrees of ring, open at the upper left.
    painter.add(Shape::line(arc_points(c + vec2(0.0, 0.5), r, 0.0, std::f32::consts::PI * 1.5, 28), PathStroke::new(st.width, col)));
    painter.line_segment([c + vec2(0.0, -3.0), c + vec2(0.0, 1.5)], st);
    painter.line_segment([c + vec2(0.0, 1.5), c + vec2(3.0, 3.2)], st);
}

/// `theme::button` derives its id from the label alone, so two buttons with the same text in one frame share
/// one id. This wrapper salts the id with the button's position.
pub fn btn(ui: &mut Ui, label: &str, kind: Btn, p: &Pal) -> Response {
    let pos = ui.cursor().min;
    ui.push_id(("btn-at", pos.x as i32, pos.y as i32), |ui| theme::button(ui, label, kind, p)).inner
}

/// Like [`btn`], but can be shown disabled (muted, no hover, no click).
pub fn btn_enabled(ui: &mut Ui, label: &str, kind: Btn, enabled: bool, p: &Pal) -> Response {
    if enabled {
        return btn(ui, label, kind, p);
    }
    let font = theme::medium(14.0);
    let g = ui.painter().layout_no_wrap(label.to_string(), font.clone(), p.muted);
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 24.0, kind.height()), Sense::hover());
    ui.painter().rect(rect, theme::R_CONTROL as f32, Color32::TRANSPARENT, Stroke::new(1.0, p.line), StrokeKind::Inside);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, font, p.muted);
    resp
}

/// A destructive action: an outline and text in the negative colour, only for meaning.
pub fn danger_button(ui: &mut Ui, label: &str, p: &Pal) -> Response {
    let font = theme::medium(14.0);
    let g = ui.painter().layout_no_wrap(label.to_string(), font.clone(), p.negative);
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 32.0, Btn::Secondary.height()), Sense::click());
    let t = theme::fade(ui, resp.id.with("hover"), resp.hovered());
    ui.painter().rect(rect, theme::R_CONTROL as f32, theme::alpha(p.raised, (t * 255.0) as u8), Stroke::new(1.0, theme::mix(theme::alpha(p.negative, 140), p.negative, t)), StrokeKind::Inside);
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, font, p.negative);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Height shared by every control that sits in a row.
pub const ROW_BTN_H: f32 = 34.0;

/// Segmented control, 34 high. Returns the picked index.
pub fn segmented(ui: &mut Ui, id: &str, labels: &[&str], selected: usize, p: &Pal) -> Option<usize> {
    let font = theme::medium(13.0);
    let widths: Vec<f32> = labels.iter().map(|l| ui.painter().layout_no_wrap(l.to_string(), font.clone(), p.text).size().x + 28.0).collect();
    let total: f32 = widths.iter().sum::<f32>() + 8.0;
    let (rect, _) = ui.allocate_exact_size(vec2(total, ROW_BTN_H), Sense::hover());
    ui.painter().rect(rect, 10.0, p.card, Stroke::new(1.0, p.line), StrokeKind::Inside);
    let mut x = rect.left() + 4.0;
    let mut picked = None;
    for (i, l) in labels.iter().enumerate() {
        let r = Rect::from_min_size(pos2(x, rect.top() + 4.0), vec2(widths[i], ROW_BTN_H - 8.0));
        x += widths[i];
        let resp = ui.interact(r, ui.scope_id().with((id, i)), Sense::click());
        let on = i == selected;
        let t = theme::fade(ui, resp.id.with("hover"), resp.hovered() && !on);
        if on {
            ui.painter().rect(r, 6.0, p.raised, Stroke::new(1.0, p.line), StrokeKind::Inside);
            ui.painter().hline(r.x_range().shrink(6.0), r.top() + 0.5, Stroke::new(1.0, p.top_light()));
        } else if t > 0.0 {
            ui.painter().rect_filled(r, 6.0, theme::alpha(p.raised, (t * 160.0) as u8));
        }
        let col = if on { p.text } else { theme::mix(p.text2, p.text, t) };
        ui.painter().text(r.center(), Align2::CENTER_CENTER, l, font.clone(), col);
        if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            picked = Some(i);
        }
    }
    picked
}

/// Single-line text input in the suite style.
pub fn input(ui: &mut Ui, text: &mut String, hint: &str, width: f32) -> Response {
    ui.add(egui::TextEdit::singleline(text).hint_text(hint).desired_width(width).margin(Margin::symmetric(12, 8)).font(theme::reg(14.0)))
}

/// Paints the dropdown chevron, vertically centred on the text row.
fn chevron(ui: &Ui, rect: Rect, color: Color32) {
    theme::icon_chevron(ui.painter(), rect.center(), color, true);
}

/// A dropdown button in the suite style: text on the left, a chevron centred on the same row.
pub fn combo(id: impl std::fmt::Debug, text: &str, width: f32) -> egui::ComboBox<'static> {
    egui::ComboBox::from_id_salt(format!("{id:?}"))
        .width(width)
        .selected_text(egui::RichText::new(text.to_string()).font(theme::reg(14.0)))
        .icon(|ui, rect, visuals, _open| chevron(ui, rect, visuals.fg_stroke.color))
}

/// Shortens `s` with an ellipsis so it fits `max_w` in `font`.
pub fn fit_text(painter: &Painter, s: &str, font: &egui::FontId, max_w: f32) -> String {
    if painter.layout_no_wrap(s.to_string(), font.clone(), Color32::WHITE).size().x <= max_w {
        return s.to_string();
    }
    let mut out: String = s.chars().collect();
    while !out.is_empty() {
        out.pop();
        let cand = format!("{}…", out.trim_end());
        if painter.layout_no_wrap(cand.clone(), font.clone(), Color32::WHITE).size().x <= max_w {
            return cand;
        }
    }
    String::new()
}
