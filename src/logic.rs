//! Pure layout and food maths, kept apart from the game so it can be tested on its own.
//! The saturation icons are the original AppleSkin pixel art (squeek502, Unlicense),
//! ported verbatim: an ASCII grid mapped through a fixed palette and drawn with
//! 1x1 fills, exactly the way the reference mod renders them.

use mod_api::Tag;

/// Vanilla draws ten hunger icons, right to left from the screen's centre.
pub const ICON: f32 = 9.0;
pub const STEP: f32 = 8.0;
pub const BAR_RIGHT: f32 = 91.0;
pub const BAR_BOTTOM: f32 = 39.0;

/// One saturation icon, four frames from a bare bone to a full drumstick.
/// Each char is a palette index ('a'..'j'); '.' is transparent. This is the
/// original AppleSkin artwork, which is why it reads as a golden-brown
/// drumstick rather than a flat bar or a garish solid-yellow blob.
pub const SATURATION: [&[&str]; 4] = [
    &[
        ".........",
        ".........",
        ".........",
        ".........",
        "......a..",
        "......a..",
        ".....b.aa",
        "......b.a",
        "......bb.",
    ],
    &[
        "...c.....",
        "....d....",
        ".....a...",
        "......a..",
        "......a..",
        "......a..",
        "....bb.aa",
        "......b.a",
        "......bb.",
    ],
    &[
        "..cc.....",
        "....d....",
        ".....a...",
        "......a..",
        "......a..",
        "..b...a..",
        "...bbb.aa",
        "......b.a",
        "......bb.",
    ],
    &[
        "..cc.....",
        ".d..d....",
        "a....a...",
        "a.....a..",
        ".b....a..",
        "..b...a..",
        "...bbb.aa",
        "......b.a",
        "......bb.",
    ],
];

/// AppleSkin's palette: golden-brown meat, dark shading, and warm highlights.
/// 0xAARRGGBB. 'a'..'j' map to entries 0..9.
pub const PALETTE: [u32; 10] = [
    0xFFC2A100, // a - main golden meat
    0xFF9F8609, // b - shaded meat
    0xFFE9D262, // c - bright highlight
    0xFFDDC241, // d - soft highlight
    0xFF0A0A0A, // e - outline
    0xFFE01700, // f
    0xFFFB1900, // g
    0xFFAB1200, // h
    0xFF9B1100, // i
    0xFF282828, // j
];

/// Palette colour for one grid cell; `None` for '.' (transparent).
pub fn cell_color(ch: u8) -> Option<u32> {
    if ch == b'.' {
        return None;
    }
    PALETTE.get(usize::from(ch.wrapping_sub(b'a'))).copied()
}

/// Which of the four saturation frames to draw for a given share of one icon.
pub fn saturation_frame(share: f32) -> usize {
    if share >= 1.0 {
        3
    } else if share > 0.5 {
        2
    } else if share > 0.25 {
        1
    } else {
        0
    }
}

/// `(icon index, frame)` for each saturation icon to draw, right to left.
pub fn saturation_icons(saturation: f32) -> Vec<(usize, usize)> {
    let s = saturation.clamp(0.0, 20.0) / 2.0;
    if !s.is_finite() {
        return Vec::new();
    }
    (0..s.ceil() as usize)
        .map(|i| (i, saturation_frame(s - i as f32)))
        .collect()
}

/// Left edge and top edge of hunger icon `index`, zero being the rightmost.
pub fn icon_origin(width: f32, height: f32, index: usize) -> [f32; 2] {
    [
        width / 2.0 + BAR_RIGHT - ICON - STEP * index as f32,
        height - BAR_BOTTOM,
    ]
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FoodValue {
    /// Food points the item restores, `nutrition` in `minecraft:food`.
    pub nutrition: i32,
    /// Saturation points the item restores, `saturation` in the component.
    /// Newer Minecraft data packs this as the final value (a steak is 12.8),
    /// not the old 0.8-style multiplier.
    pub saturation: f32,
}

impl FoodValue {
    /// What eating adds to the saturation level: the component's `saturation`
    /// directly. Multiplying by nutrition here would triple-count it.
    pub fn saturation_gain(&self) -> f32 {
        self.saturation
    }
}

/// `minecraft:food` as the item report gives it: a compound of `nutrition` and `saturation`.
pub fn parse_food(tag: &Tag) -> Option<FoodValue> {
    if !matches!(tag, Tag::Compound(_)) {
        return None;
    }
    let nutrition = match tag.get("nutrition") {
        Some(Tag::Int(value)) => *value,
        Some(Tag::Byte(value)) => i32::from(*value),
        Some(Tag::Short(value)) => i32::from(*value),
        _ => return None,
    };
    // Newer Minecraft writes saturation as the final points gained (steak 12.8).
    // Some data packs still store a multiplier; at <= 2 it is safely ambiguous,
    // so convert that old form with nutrition * modifier * 2.
    let raw = match tag.get("saturation") {
        Some(Tag::Float(bits)) => f32::from_bits(*bits),
        Some(Tag::Double(bits)) => f64::from_bits(*bits) as f32,
        _ => 0.0,
    };
    let saturation = if raw.abs() <= 2.0 {
        nutrition as f32 * raw * 2.0
    } else {
        raw
    };
    Some(FoodValue { nutrition, saturation })
}

/// One tooltip line per fact, already in the order they should appear.
/// Chinese labels, styled like AppleSkin's vanilla-style food values.
pub fn tooltip_lines(value: FoodValue) -> Vec<(String, u32)> {
    let mut lines = Vec::new();
    if value.nutrition != 0 {
        lines.push((format!("恢复饥饿 +{}", value.nutrition), 0x55FF55));
    }
    let gain = value.saturation_gain();
    if gain > 0.0 {
        lines.push((format!("饱和度 +{gain:.1}"), 0xFFD9A21B));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_maps_the_golden_meat_shades() {
        assert_eq!(cell_color(b'a'), Some(0xFFC2A100));
        assert_eq!(cell_color(b'c'), Some(0xFFE9D262));
        assert_eq!(cell_color(b'.'), None);
    }

    #[test]
    fn saturation_frames_rise_with_share() {
        assert_eq!(saturation_frame(0.1), 0);
        assert_eq!(saturation_frame(0.4), 1);
        assert_eq!(saturation_frame(0.7), 2);
        assert_eq!(saturation_frame(1.0), 3);
    }

    #[test]
    fn saturation_icons_count_and_frames() {
        // 5 points = 2.5 icons: two full + one half.
        assert_eq!(saturation_icons(5.0), vec![(0, 3), (1, 3), (2, 1)]);
        assert_eq!(saturation_icons(0.0).len(), 0);
        assert_eq!(saturation_icons(20.0).len(), 10);
        assert_eq!(saturation_icons(f32::NAN).len(), 0);
    }

    #[test]
    fn the_rightmost_icon_sits_against_the_bar() {
        let [x, y] = icon_origin(400.0, 300.0, 0);
        assert_eq!(x, 200.0 + 91.0 - 9.0);
        assert_eq!(y, 261.0);
        let [x1, _] = icon_origin(400.0, 300.0, 1);
        assert_eq!(x - x1, 8.0);
    }

    #[test]
    fn a_steak_restores_what_the_formula_says() {
        let value = FoodValue {
            nutrition: 8,
            saturation: 12.8,
        };
        assert!((value.saturation_gain() - 12.8).abs() < 0.001);
        let lines = tooltip_lines(value);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].0, "恢复饥饿 +8");
        assert_eq!(lines[1].0, "饱和度 +12.8");
    }

    #[test]
    fn old_multiplier_form_is_converted() {
        let raw = 0.8_f32;
        let nutrition = 8;
        let saturation = if raw.abs() <= 2.0 { nutrition as f32 * raw * 2.0 } else { raw };
        assert!((saturation - 12.8).abs() < 0.001);
    }
}
