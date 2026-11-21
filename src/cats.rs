//! Categories: the built-in set, their colours, and the fixed palette offered for custom categories.
//!
//! A category is identified by a short string: one of the built-in ids below, or `c<number>` for a custom one
//! (see `db::CategoryRow`). Names of the built-in ones live in `strings.rs`.

pub const BROWSER: &str = "browser";
pub const CODE: &str = "code";
pub const GAMES: &str = "games";
pub const COMMUNICATION: &str = "communication";
pub const VIDEO: &str = "video";
pub const OFFICE: &str = "office";
pub const DESIGN: &str = "design";
pub const SYSTEM: &str = "system";
pub const OTHER: &str = "other";

/// Built-in categories in display order, with their chart colour.
pub const BUILTIN: [(&str, u32); 9] = [
    (CODE, 0x058778),
    (BROWSER, 0x1d7cb4),
    (COMMUNICATION, 0x33884f),
    (VIDEO, 0xb05647),
    (GAMES, 0x8162af),
    (OFFICE, 0xa66119),
    (DESIGN, 0x9c5894),
    (SYSTEM, 0x71717a),
    (OTHER, 0x8d7102),
];

/// Colours a custom category can take: twelve steps of one lightness (OKLCH L 0.56) plus a neutral, the same set the
/// other suite apps use for dots and segments. Labels in white reach 4.4:1 on all of them.
pub const PALETTE: [u32; 13] = [
    0xb05647, 0x058778, 0x9c5894, 0x6a7f20, 0x5b6fbb, 0xa66119, 0x078394, 0xac5370, 0x33884f, 0x8162af, 0x8d7102,
    0x1d7cb4, 0x71717a,
];

#[cfg(test)]
pub fn is_builtin(id: &str) -> bool {
    BUILTIN.iter().any(|(b, _)| *b == id)
}

pub fn builtin_color(id: &str) -> Option<u32> {
    BUILTIN.iter().find(|(b, _)| *b == id).map(|(_, c)| *c)
}

/// Is this id one the code knows how to treat specially: Video and Games keep counting through idle time.
pub fn keeps_counting_when_idle(id: &str) -> bool {
    id == VIDEO || id == GAMES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_ids_are_unique_and_known() {
        for (i, (a, _)) in BUILTIN.iter().enumerate() {
            assert!(is_builtin(a));
            assert!(BUILTIN.iter().skip(i + 1).all(|(b, _)| a != b));
        }
        assert!(!is_builtin("c1"));
        assert_eq!(builtin_color(CODE), Some(0x058778));
        assert!(keeps_counting_when_idle(VIDEO) && keeps_counting_when_idle(GAMES) && !keeps_counting_when_idle(CODE));
    }
}
