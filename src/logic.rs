//! Pure food, HUD offset, tooltip layout, and fallback pixel-art logic.
//! Ported from AppleSkin (squeek502, Unlicense) for Horizon (Minecraft 26.3).

use mod_api::Tag;

pub const ICON: f32 = 9.0;
pub const BAR_RIGHT: f32 = 91.0;
pub const BAR_LEFT: f32 = 91.0;
pub const BAR_BOTTOM: f32 = 39.0;

pub const MAX_EXHAUSTION: f32 = 4.0;
pub const REGEN_EXHAUSTION_INCREMENT: f32 = 6.0;

/// Computes integer-aligned `(left, right, top)` anchors for the vanilla HUD
/// health and hunger bars, matching Minecraft/Horizon's integer `gui_width / 2`
/// division so overlays stay aligned on odd scaled widths (e.g. 1366x768 fullscreen).
pub fn hud_bar_anchors(width: f32, height: f32) -> (f32, f32, f32) {
    let center = (width.max(0.0) / 2.0).floor();
    let top = height.floor() - BAR_BOTTOM;
    (center - BAR_LEFT, center + BAR_RIGHT, top)
}

/// Four HUD saturation icon frames from a sliver to a full drumstick (9x9 each).
/// Used when falling back to pixel fills if the atlas sprite is unavailable.
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

/// Palette mapped from 'a'..'j' (`0xAARRGGBB`); '.' is transparent.
pub const PALETTE: [u32; 10] = [
    0xFFC2_A100, // a - main golden meat
    0xFF9F_8609, // b - shaded meat
    0xFFE9_D262, // c - bright highlight
    0xFFDD_C241, // d - soft highlight
    0xFF0A_0A0A, // e - outline
    0xFFE0_1700, // f
    0xFFFB_1900, // g
    0xFFAB_1200, // h
    0xFF9B_1100, // i
    0xFF28_2828, // j - exhaustion bar pixel
];

/// Returns the ARGB colour for an ASCII pixel-art cell, or `None` for `'.'`.
pub fn cell_color(ch: u8) -> Option<u32> {
    if ch == b'.' {
        return None;
    }
    PALETTE.get(usize::from(ch.wrapping_sub(b'a'))).copied()
}

/// Converts normalized `(r, g, b, a)` floats in `0.0..=1.0` to packed `0xAARRGGBB`.
pub fn argb_from_rgba(r: f32, g: f32, b: f32, a: f32) -> u32 {
    let ch = |v: f32| (v.clamp(0.0, 1.0) * 255.0).floor() as u32;
    (ch(a) << 24) | (ch(r) << 16) | (ch(g) << 8) | ch(b)
}

/// Multiplies a base colour by a normalized alpha in `0.0..=1.0`.
pub fn with_alpha(rgb: u32, alpha: f32) -> u32 {
    let a = ((alpha.clamp(0.0, 1.0) * 255.0).floor() as u32) & 0xFF;
    (a << 24) | (rgb & 0x00FF_FFFF)
}

/// Frame index (`0..=3`) for a given share (`0.0..=1.0+`) of one saturation icon.
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

/// Frame index (`0..=4`) for a 7x7 tooltip saturation icon (`0..=3` partial/full, `4` empty).
pub fn tooltip_saturation_u(share: f32) -> f32 {
    if share >= 1.0 {
        21.0
    } else if share > 0.5 {
        14.0
    } else if share > 0.25 {
        7.0
    } else if share > 0.0 {
        0.0
    } else {
        28.0
    }
}

/// Icon indices and frames to render for a saturation overlay range, matching
/// AppleSkin's `drawSaturationOverlay`.
pub fn saturation_overlay_icons(saturation_level: f32, saturation_gained: f32) -> Vec<(usize, usize)> {
    let total = saturation_level + saturation_gained;
    if !total.is_finite() || total < 0.0 {
        return Vec::new();
    }
    let modified_sat = total.clamp(0.0, 20.0);
    let start_bar = if saturation_gained != 0.0 {
        (saturation_level / 2.0).max(0.0) as usize
    } else {
        0
    };
    let end_bar = (modified_sat / 2.0).ceil() as usize;
    if start_bar >= end_bar {
        return Vec::new();
    }
    (start_bar..end_bar)
        .map(|i| {
            let share = modified_sat / 2.0 - i as f32;
            (i, saturation_frame(share))
        })
        .collect()
}

/// Hunger bars `(icon_index, is_half)` to render when previewing held food.
pub fn hunger_overlay_icons(food_level: i32, hunger_restored: i32) -> Vec<(usize, bool)> {
    if hunger_restored <= 0 {
        return Vec::new();
    }
    let modified_food = (food_level + hunger_restored).clamp(0, 20);
    let start_bar = (food_level / 2).max(0) as usize;
    let end_bar = ((modified_food as f32) / 2.0).ceil() as usize;
    if start_bar >= end_bar {
        return Vec::new();
    }
    (start_bar..end_bar)
        .map(|i| {
            let is_half = i as i32 * 2 + 1 == modified_food;
            (i, is_half)
        })
        .collect()
}

/// Heart indices `(heart_index, is_half)` to render for estimated health restored.
pub fn health_overlay_icons(current_health: f32, modified_health: f32) -> Vec<(usize, bool)> {
    if !current_health.is_finite() || !modified_health.is_finite() || modified_health <= current_health {
        return Vec::new();
    }
    let fixed_modified = modified_health.ceil() as i32;
    let start_bar = (current_health.ceil() / 2.0).max(0.0) as usize;
    let end_bar = (modified_health / 2.0).ceil().max(0.0) as usize;
    if start_bar >= end_bar {
        return Vec::new();
    }
    (start_bar..end_bar)
        .map(|i| {
            let is_half = i as i32 * 2 + 1 == fixed_modified;
            (i, is_half)
        })
        .collect()
}

/// Exhaustion underlay pixel width (`0..=81`).
pub fn exhaustion_width(exhaustion: f32) -> i32 {
    if !exhaustion.is_finite() {
        return 0;
    }
    let ratio = (exhaustion / MAX_EXHAUSTION).clamp(0.0, 1.0);
    (ratio * 81.0) as i32
}

/// Saturation added to the player given current hunger & saturation levels (capped at new food level).
pub fn capped_saturation_gained(
    current_food: i32,
    current_saturation: f32,
    added_food: i32,
    added_saturation: f32,
) -> f32 {
    let new_food = (current_food + added_food) as f32;
    let new_sat = current_saturation + added_saturation;
    if new_sat > new_food {
        new_food - current_saturation
    } else {
        added_saturation
    }
}

/// Parsed `minecraft:food` component values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FoodValue {
    /// Hunger drumsticks in half-shanks (`nutrition`).
    pub nutrition: i32,
    /// Direct saturation points restored (`saturation`).
    pub saturation: f32,
    /// Whether the food can be eaten at full hunger (`can_always_eat`).
    pub can_always_eat: bool,
}

fn unwrap_mixed(mut tag: &Tag) -> &Tag {
    while let Tag::Compound(map) = tag {
        if map.len() == 1
            && let Some(inner) = tag.get("")
        {
            tag = inner;
        } else {
            break;
        }
    }
    tag
}

fn tag_int(tag: &Tag) -> Option<i32> {
    match tag {
        Tag::Int(v) => Some(*v),
        Tag::Byte(v) => Some(i32::from(*v)),
        Tag::Short(v) => Some(i32::from(*v)),
        Tag::Long(v) => i32::try_from(*v).ok(),
        _ => None,
    }
}

fn tag_float(tag: &Tag) -> Option<f32> {
    match tag {
        Tag::Float(bits) => Some(f32::from_bits(*bits)),
        Tag::Double(bits) => Some(f64::from_bits(*bits) as f32),
        Tag::Int(v) => Some(*v as f32),
        Tag::Byte(v) => Some(f32::from(*v)),
        Tag::Short(v) => Some(f32::from(*v)),
        _ => None,
    }
}

fn tag_bool(tag: &Tag) -> Option<bool> {
    match tag {
        Tag::Byte(v) => Some(*v != 0),
        Tag::Int(v) => Some(*v != 0),
        _ => None,
    }
}

fn tag_str(tag: &Tag) -> Option<String> {
    match tag {
        Tag::String(s) => Some(s.to_string_lossy()),
        _ => None,
    }
}

fn normalize_id(raw: &str) -> String {
    if raw.contains(':') {
        raw.to_owned()
    } else {
        format!("minecraft:{raw}")
    }
}

/// Parses `minecraft:food` component NBT into [`FoodValue`].
///
/// In Minecraft 1.20.5+ / 26.3 (and Horizon's synced components), `saturation`
/// already stores the final saturation points (e.g., Cookie = `0.4`, Steak = `12.8`).
pub fn parse_food(tag: &Tag) -> Option<FoodValue> {
    let tag = unwrap_mixed(tag);
    if !matches!(tag, Tag::Compound(_)) {
        return None;
    }
    let nutrition = tag.get("nutrition").and_then(tag_int)?;
    let saturation = tag
        .get("saturation")
        .and_then(tag_float)
        .filter(|v| v.is_finite())
        .unwrap_or(0.0);
    let can_always_eat = tag
        .get("can_always_eat")
        .and_then(tag_bool)
        .unwrap_or(false);
    Some(FoodValue {
        nutrition,
        saturation,
        can_always_eat,
    })
}

/// One status effect applied when consuming an item (`minecraft:apply_effects`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsumeEffectInfo {
    /// Namespaced effect id, e.g. `"minecraft:regeneration"`.
    pub id: String,
    /// Duration in ticks.
    pub duration: i32,
    /// Effect amplifier (`0` = level I).
    pub amplifier: i32,
}

fn parse_effect_instance(tag: &Tag) -> Option<ConsumeEffectInfo> {
    let tag = unwrap_mixed(tag);
    if !matches!(tag, Tag::Compound(_)) {
        return None;
    }
    let raw_id = tag.get("id").and_then(tag_str)?;
    let duration = tag.get("duration").and_then(tag_int).unwrap_or(0);
    let amplifier = tag.get("amplifier").and_then(tag_int).unwrap_or(0);
    Some(ConsumeEffectInfo {
        id: normalize_id(&raw_id),
        duration,
        amplifier,
    })
}

/// Extracts applied status effects from `minecraft:consumable` NBT (`on_consume_effects`).
pub fn parse_consumable_effects(tag: &Tag) -> Vec<ConsumeEffectInfo> {
    let tag = unwrap_mixed(tag);
    let Some(Tag::List(entries)) = tag.get("on_consume_effects") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries {
        let entry = unwrap_mixed(entry);
        let ty = entry
            .get("type")
            .and_then(tag_str)
            .map(|s| normalize_id(&s));
        let is_apply = match ty.as_deref() {
            Some("minecraft:apply_effects") => true,
            Some(_) => false,
            None => entry.get("effects").is_some(),
        };
        if !is_apply {
            continue;
        }
        match entry.get("effects") {
            Some(Tag::List(list)) => {
                for eff in list {
                    if let Some(info) = parse_effect_instance(eff) {
                        out.push(info);
                    }
                }
            }
            Some(single) => {
                if let Some(info) = parse_effect_instance(single) {
                    out.push(info);
                }
            }
            None => {}
        }
    }
    out
}

/// Estimates natural health regeneration from food & saturation levels, matching
/// AppleSkin's `FoodHelper.getEstimatedHealthIncrement(int, float, float)`.
pub fn estimated_natural_regen(
    mut food_level: i32,
    mut saturation_level: f32,
    mut exhaustion_level: f32,
) -> f32 {
    if !exhaustion_level.is_finite() || !saturation_level.is_finite() {
        return 0.0;
    }

    let mut health = 0.0_f32;
    let max_up = f32::from_bits(MAX_EXHAUSTION.to_bits() + 1);
    let mut guard = 0_u32;

    while food_level >= 18 && guard < 10_000 {
        guard += 1;
        while exhaustion_level > MAX_EXHAUSTION {
            exhaustion_level -= MAX_EXHAUSTION;
            if saturation_level > 0.0 {
                saturation_level = (saturation_level - 1.0).max(0.0);
            } else {
                food_level -= 1;
            }
        }
        if food_level >= 20 && saturation_level > f32::MIN_POSITIVE {
            let limited_sat = saturation_level.min(REGEN_EXHAUSTION_INCREMENT);
            let exhaustion_until_above_max = max_up - exhaustion_level;
            let raw_iters = (exhaustion_until_above_max / limited_sat).ceil();
            if !raw_iters.is_finite() {
                break;
            }
            let num_iters = raw_iters.max(1.0) as i32;
            health += (limited_sat / REGEN_EXHAUSTION_INCREMENT) * num_iters as f32;
            exhaustion_level += limited_sat * num_iters as f32;
        } else if food_level >= 18 {
            health += 1.0;
            exhaustion_level += REGEN_EXHAUSTION_INCREMENT;
        }
    }

    health
}

/// Estimates health restored from a regeneration consume effect (`MobEffects.REGENERATION`).
pub fn regeneration_health_from_effects(effects: &[ConsumeEffectInfo]) -> f32 {
    for eff in effects {
        if eff.id == "minecraft:regeneration" && eff.duration > 0 {
            let shift = eff.amplifier.clamp(0, 30) as u32;
            let interval = (50 >> shift).max(1);
            return ((eff.duration / interval) as f32).floor();
        }
    }
    0.0
}

/// Estimates total health restored after eating a food item (`FoodHelper.getEstimatedHealthIncrement`).
pub fn estimated_health_increment(
    current_health: f32,
    max_health: f32,
    current_food: i32,
    current_saturation: f32,
    current_exhaustion: f32,
    food: FoodValue,
    effects: &[ConsumeEffectInfo],
    natural_regeneration: bool,
) -> f32 {
    if !current_health.is_finite() || !max_health.is_finite() || current_health >= max_health {
        return 0.0;
    }
    let food_level = (current_food + food.nutrition).min(20);
    let mut health_increment = 0.0;
    if food_level >= 18 && natural_regeneration {
        let sat_level = (current_saturation + food.saturation).min(food_level as f32);
        health_increment = estimated_natural_regen(food_level, sat_level, current_exhaustion);
    }
    health_increment + regeneration_health_from_effects(effects)
}

/// Oscillating flash alpha controller matching AppleSkin's `HUDOverlayHandler.onClientTick`.
#[derive(Clone, Copy, Debug)]
pub struct FlashState {
    pub unclamped: f32,
    pub alpha: f32,
    pub dir: i8,
}

impl Default for FlashState {
    fn default() -> Self {
        Self {
            unclamped: 0.0,
            alpha: 0.0,
            dir: 1,
        }
    }
}

impl FlashState {
    /// Advances one client tick toward `[0, max_alpha]`.
    pub fn tick(&mut self, max_alpha: f32) {
        self.unclamped += f32::from(self.dir) * 0.125;
        if self.unclamped >= 1.5 {
            self.dir = -1;
        } else if self.unclamped <= -0.5 {
            self.dir = 1;
        }
        self.alpha = self.unclamped.clamp(0.0, 1.0) * max_alpha.clamp(0.0, 1.0);
    }

    /// Resets the flash fade cycle when no edible food is held.
    pub fn reset(&mut self) {
        self.unclamped = 0.0;
        self.alpha = 0.0;
        self.dir = 1;
    }
}

/// Java's 48-bit LCG (`java.util.Random`) used by vanilla HUD icon jitter.
#[derive(Clone, Copy, Debug)]
pub struct JavaRandom {
    seed: u64,
}

impl JavaRandom {
    const MULTIPLIER: u64 = 0x5DEE_CE66D;
    const ADDEND: u64 = 0xB;
    const MASK: u64 = (1_u64 << 48) - 1;

    pub fn new(seed: i64) -> Self {
        Self {
            seed: ((seed as u64) ^ Self::MULTIPLIER) & Self::MASK,
        }
    }

    fn next(&mut self, bits: u32) -> u32 {
        self.seed = (self.seed.wrapping_mul(Self::MULTIPLIER).wrapping_add(Self::ADDEND)) & Self::MASK;
        (self.seed >> (48 - bits)) as u32
    }

    pub fn next_int(&mut self, bound: i32) -> i32 {
        if bound <= 0 {
            return 0;
        }
        let b = bound as u32;
        if (b & b.wrapping_sub(1)) == 0 {
            return (((b as u64) * (self.next(31) as u64)) >> 31) as i32;
        }
        loop {
            let bits = self.next(31) as i32;
            let val = bits % bound;
            if bits.wrapping_sub(val).wrapping_add(bound - 1) >= 0 {
                return val;
            }
        }
    }
}

/// Relative `(x, y)` offsets from `(left, top)` for health hearts and `(right, top)` for food icons.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HudOffsets {
    pub health: Vec<[i32; 2]>,
    pub food: Vec<[i32; 2]>,
}

/// Computes vanilla HUD heart and food offsets (matching Horizon's `HealthHud::layout`
/// PRNG consumption order so low-health jitter and empty-saturation jitter stay in sync).
pub fn compute_hud_offsets(
    gui_ticks: i32,
    health: f32,
    max_health: f32,
    absorption: f32,
    food_level: i32,
    saturation_level: f32,
    has_regeneration: bool,
    animate: bool,
) -> HudOffsets {
    let containers = f64::from(max_health / 2.0).ceil().max(0.0) as i32;
    let absorption_int = absorption.ceil().max(0.0) as i32;
    let absorb_containers = f64::from(absorption_int).ceil() as i32 / 2
        + i32::from(absorption_int % 2 != 0);
    let mut health_bars = containers + absorb_containers;
    if !(0..=1000).contains(&health_bars) {
        health_bars = 0;
    }

    let rows = ((health_bars as f32) / 10.0).ceil().max(0.0) as i32;
    let row_height = (10 - (rows - 2)).max(3);

    let current = health.ceil() as i32;
    let should_animate_health = animate && (current + absorption_int <= 4);
    let regen_denom = max_health.ceil().max(0.0) as i32 + 5;
    let regen_index = if animate && has_regeneration && regen_denom > 0 {
        gui_ticks.rem_euclid(regen_denom)
    } else {
        -1
    };

    let food_divisor = (food_level.max(0) * 3) + 1;
    let should_animate_food = animate && saturation_level <= 0.0 && gui_ticks.rem_euclid(food_divisor) == 0;

    let mut rng = JavaRandom::new(i64::from(gui_ticks.wrapping_mul(312_871)));
    let mut health_offsets = vec![[0, 0]; health_bars as usize];
    for i in (0..health_bars).rev() {
        let row = i / 10;
        let col = i % 10;
        let x = col * 8;
        let mut y = -(row * row_height);
        if should_animate_health {
            y += rng.next_int(2);
        }
        if i < containers && i == regen_index {
            y -= 2;
        }
        health_offsets[i as usize] = [x, y];
    }

    let mut food_offsets = Vec::with_capacity(10);
    for i in 0..10 {
        let x = -(i * 8) - 9;
        let mut y = 0;
        if should_animate_food {
            y += rng.next_int(3) - 1;
        }
        food_offsets.push([x, y]);
    }

    HudOffsets {
        health: health_offsets,
        food: food_offsets,
    }
}

/// Tooltip drumstick outline tint matching `TooltipOverlayHandler.FoodOutline`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoodOutline {
    Negative,
    Extra,
    Normal,
    Partial,
    Missing,
}

impl FoodOutline {
    pub fn argb(self) -> u32 {
        match self {
            Self::Negative => argb_from_rgba(1.0, 1.0, 1.0, 1.0),
            Self::Extra => argb_from_rgba(0.06, 0.32, 0.02, 1.0),
            Self::Normal => argb_from_rgba(0.0, 0.0, 0.0, 1.0),
            Self::Partial => argb_from_rgba(0.53, 0.21, 0.08, 1.0),
            Self::Missing => argb_from_rgba(0.62, 0.0, 0.0, 0.5),
        }
    }

    pub fn classify(modified_hunger: i32, default_hunger: i32, i: i32) -> Self {
        if modified_hunger < 0 {
            Self::Negative
        } else if modified_hunger > default_hunger && default_hunger <= i {
            Self::Extra
        } else if modified_hunger > i + 1 || default_hunger == modified_hunger {
            Self::Normal
        } else if modified_hunger == i + 1 {
            Self::Partial
        } else {
            Self::Missing
        }
    }
}

/// Precomputed AppleSkin tooltip bar layout matching `TooltipOverlayHandler.FoodOverlay`.
#[derive(Clone, Debug, PartialEq)]
pub struct TooltipFoodOverlay {
    pub default_food: FoodValue,
    pub modified_food: FoodValue,
    pub is_rotten: bool,
    pub hunger_bars: i32,
    pub hunger_bars_text: Option<String>,
    pub saturation_bars: i32,
    pub saturation_bars_text: Option<String>,
}

impl TooltipFoodOverlay {
    /// Builds bar counts and `"xN"` overflow labels identically to original `FoodOverlay`.
    pub fn new(default_food: FoodValue, modified_food: FoodValue, is_rotten: bool) -> Self {
        let biggest_hunger = default_food.nutrition.max(modified_food.nutrition);
        let biggest_sat = default_food.saturation.max(modified_food.saturation);

        let mut hunger_bars = (biggest_hunger.abs() as f32 / 2.0).ceil() as i32;
        let mut hunger_bars_text = None;
        if hunger_bars > 10 {
            let sign = if biggest_hunger < 0 { -1 } else { 1 };
            hunger_bars_text = Some(format!("x{}", sign * hunger_bars));
            hunger_bars = 1;
        }

        let mut saturation_bars = (biggest_sat.abs() / 2.0).ceil() as i32;
        let mut saturation_bars_text = None;
        if saturation_bars > 10 || saturation_bars == 0 {
            let sign = if biggest_sat < 0.0 { -1 } else { 1 };
            saturation_bars_text = Some(format!("x{}", sign * saturation_bars));
            saturation_bars = 1;
        }

        Self {
            default_food,
            modified_food,
            is_rotten,
            hunger_bars,
            hunger_bars_text,
            saturation_bars,
            saturation_bars_text,
        }
    }

    pub fn should_render(&self) -> bool {
        self.hunger_bars > 0
    }

    /// Tooltip component height (`9 + 1 + 7 + 3 = 20`), matching `FoodOverlay.getHeight`.
    pub fn height(&self) -> f32 {
        20.0
    }

    /// Tooltip component width given pixel widths for `hunger_bars_text` and `saturation_bars_text`.
    pub fn width(&self, hunger_text_w: f32, sat_text_w: f32) -> f32 {
        let hunger_len = self.hunger_bars as f32 * 9.0
            + if self.hunger_bars_text.is_some() {
                hunger_text_w
            } else {
                0.0
            };
        let sat_len = self.saturation_bars as f32 * 7.0
            + if self.saturation_bars_text.is_some() {
                sat_text_w
            } else {
                0.0
            };
        hunger_len.max(sat_len).max(1.0)
    }
}

/// Horizon's tooltip box layout (`stations/tooltip.rs::layout`), yielding box `(w, h)` and
/// element `y` offsets.
pub fn horizon_tooltip_layout(sizes: &[[f32; 2]]) -> ([f32; 2], Vec<f32>) {
    let extra_space = true;
    let mut height = if extra_space && sizes.len() > 1 {
        0.0
    } else {
        -2.0
    };
    let mut width = 0.0_f32;
    let mut top = 0.0_f32;
    let mut offsets = Vec::with_capacity(sizes.len());
    for (index, size) in sizes.iter().enumerate() {
        offsets.push(top);
        width = width.max(size[0]);
        height += size[1];
        top += size[1] + if index == 0 && extra_space { 2.0 } else { 0.0 };
    }
    ([width, height], offsets)
}

/// Horizon's tooltip positioner (`stations.rs::tooltip_position`).
pub fn horizon_tooltip_position(
    (px, py): (f32, f32),
    (screen_w, screen_h): (f32, f32),
    (w, h): (f32, f32),
) -> (f32, f32) {
    let mut x = (px + 12.0).round();
    let mut y = (py - 12.0).round();
    if x + w > screen_w - 4.0 {
        x = (x - 28.0 - w).max(4.0);
    }
    if x + w > screen_w - 4.0 {
        x = (screen_w - w - 4.0).max(4.0);
    }
    if y + h + 3.0 > screen_h {
        y = screen_h - h - 3.0;
    }
    y = y.max(4.0);
    (x.round(), y.round())
}

/// Formats F3 Debug Screen line matching `DebugInfoHudEntry.java` (`"hunger: 20, sat: 5.00, exh: 0.00/4"`).
pub fn format_debug_food_line(food_level: i32, saturation: f32, exhaustion: f32) -> String {
    format!("hunger: {food_level}, sat: {saturation:.2}, exh: {exhaustion:.2}/4")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn float_tag(v: f32) -> Tag {
        Tag::Float(v.to_bits())
    }

    #[test]
    fn palette_maps_the_golden_meat_shades() {
        assert_eq!(cell_color(b'a'), Some(0xFFC2_A100));
        assert_eq!(cell_color(b'c'), Some(0xFFE9_D262));
        assert_eq!(cell_color(b'.'), None);
    }

    #[test]
    fn parse_food_uses_direct_saturation_even_below_two() {
        let cookie = Tag::compound([
            ("nutrition", Tag::Int(2)),
            ("saturation", float_tag(0.4)),
            ("can_always_eat", Tag::Byte(0)),
        ]);
        let parsed = parse_food(&cookie).unwrap();
        assert_eq!(parsed.nutrition, 2);
        assert!((parsed.saturation - 0.4).abs() < 1e-5);
        assert!(!parsed.can_always_eat);

        let steak = Tag::compound([
            ("nutrition", Tag::Int(8)),
            ("saturation", float_tag(12.8)),
        ]);
        let parsed_steak = parse_food(&steak).unwrap();
        assert_eq!(parsed_steak.nutrition, 8);
        assert!((parsed_steak.saturation - 12.8).abs() < 1e-5);

        let g_apple = Tag::compound([
            ("nutrition", Tag::Int(4)),
            ("saturation", float_tag(9.6)),
            ("can_always_eat", Tag::Byte(1)),
        ]);
        let parsed_ga = parse_food(&g_apple).unwrap();
        assert!(parsed_ga.can_always_eat);
    }

    #[test]
    fn parse_consumable_extracts_apply_effects() {
        let rotten = Tag::compound([(
            "on_consume_effects",
            Tag::List(vec![Tag::compound([
                ("type", Tag::String("minecraft:apply_effects".into())),
                ("probability", float_tag(0.8)),
                (
                    "effects",
                    Tag::List(vec![Tag::compound([
                        ("id", Tag::String("minecraft:hunger".into())),
                        ("duration", Tag::Int(600)),
                        ("amplifier", Tag::Byte(0)),
                    ])]),
                ),
            ])]),
        )]);
        let effects = parse_consumable_effects(&rotten);
        assert_eq!(
            effects,
            vec![ConsumeEffectInfo {
                id: "minecraft:hunger".to_owned(),
                duration: 600,
                amplifier: 0,
            }]
        );
    }

    #[test]
    fn saturation_overlay_matches_appleskin_ranges() {
        assert_eq!(
            saturation_overlay_icons(5.0, 0.0),
            vec![(0, 3), (1, 3), (2, 1)]
        );
        // Held food adds saturation starting from current saturation bar.
        assert_eq!(
            saturation_overlay_icons(4.0, 4.0),
            vec![(2, 3), (3, 3)]
        );
        assert_eq!(saturation_overlay_icons(0.0, 0.0).len(), 0);
        assert_eq!(saturation_overlay_icons(20.0, 0.0).len(), 10);
        assert_eq!(saturation_overlay_icons(f32::NAN, 0.0).len(), 0);
    }

    #[test]
    fn hunger_overlay_highlights_restored_icons() {
        // Food level 13 (6 full + 1 half at index 6), +4 nutrition -> 17 (indices 6, 7, 8).
        assert_eq!(
            hunger_overlay_icons(13, 4),
            vec![(6, false), (7, false), (8, true)]
        );
        assert_eq!(hunger_overlay_icons(20, 4), Vec::new());
    }

    #[test]
    fn estimated_health_matches_vanilla_regen() {
        let apple = FoodValue {
            nutrition: 4,
            saturation: 2.4,
            can_always_eat: false,
        };
        let regen = estimated_health_increment(10.0, 20.0, 18, 0.0, 0.0, apple, &[], true);
        assert!((regen - 2.4).abs() < 0.05);

        let g_apple_eff = vec![ConsumeEffectInfo {
            id: "minecraft:regeneration".to_owned(),
            duration: 100,
            amplifier: 1,
        }];
        assert!((regeneration_health_from_effects(&g_apple_eff) - 4.0).abs() < 1e-5);
    }

    #[test]
    fn flash_state_oscillates_and_resets() {
        let mut flash = FlashState::default();
        flash.tick(0.65);
        assert!((flash.alpha - 0.125 * 0.65).abs() < 1e-5);
        for _ in 0..12 {
            flash.tick(0.65);
        }
        assert_eq!(flash.dir, -1);
        flash.reset();
        assert_eq!(flash.alpha, 0.0);
        assert_eq!(flash.dir, 1);
    }

    #[test]
    fn tooltip_overlay_counts_overflow_and_outlines() {
        let big = FoodValue {
            nutrition: 24,
            saturation: 28.0,
            can_always_eat: false,
        };
        let overlay = TooltipFoodOverlay::new(big, big, false);
        assert_eq!(overlay.hunger_bars, 1);
        assert_eq!(overlay.hunger_bars_text.as_deref(), Some("x12"));
        assert_eq!(overlay.saturation_bars, 1);
        assert_eq!(overlay.saturation_bars_text.as_deref(), Some("x14"));

        assert_eq!(FoodOutline::classify(4, 4, 0), FoodOutline::Normal);
        assert_eq!(FoodOutline::classify(2, 4, 2), FoodOutline::Missing);
        assert_eq!(FoodOutline::classify(3, 4, 2), FoodOutline::Partial);
        assert_eq!(FoodOutline::classify(6, 4, 4), FoodOutline::Extra);
        assert_eq!(FoodOutline::classify(-2, 4, 0), FoodOutline::Negative);
    }

    #[test]
    fn debug_line_matches_original_format() {
        assert_eq!(
            format_debug_food_line(20, 5.0, 0.25),
            "hunger: 20, sat: 5.00, exh: 0.25/4"
        );
    }

    #[test]
    fn hud_bar_anchors_snaps_center_for_odd_fullscreen_widths() {
        assert_eq!(hud_bar_anchors(683.0, 384.0), (250.0, 432.0, 345.0));
        assert_eq!(hud_bar_anchors(640.0, 360.0), (229.0, 411.0, 321.0));
        assert_eq!(hud_bar_anchors(455.3333, 256.0), (136.0, 318.0, 217.0));
        assert_eq!(hud_bar_anchors(853.0, 480.0), (335.0, 517.0, 441.0));
    }
}
