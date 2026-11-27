//! AppleSkin for Horizon: saturation, hunger/health previews, exhaustion underlay,
//! graphical food tooltips, and F3 debug food info matching original AppleSkin 26.3.

mod logic;

use std::sync::Mutex;

use logic::{
    BAR_BOTTOM, BAR_LEFT, BAR_RIGHT, ConsumeEffectInfo, FlashState, FoodOutline, FoodValue, ICON,
    PALETTE, SATURATION, TooltipFoodOverlay, capped_saturation_gained, cell_color,
    compute_hud_offsets, estimated_health_increment, exhaustion_width, format_debug_food_line,
    health_overlay_icons, horizon_tooltip_layout, horizon_tooltip_position, hunger_overlay_icons,
    parse_consumable_effects, parse_food, saturation_overlay_icons, tooltip_saturation_u,
    with_alpha,
};
use mod_api::{
    ClientHost, ClientTickPostEvent, ComponentId, DebugEntry, DebugEntryLinesEvent, DebugGroup,
    DrawContext, EffectCategory, Flow, GuiDraw, GuiLayerPostEvent, GuiLayerPreEvent, Hand, Hooks,
    Identifier, ItemKey, ItemResource, ItemStack, RegisterDebugEntriesEvent, Registrar,
    RegistrationContext, RenderTooltipGatherComponentsEvent, RenderTooltipPreEvent,
    ScreenRenderPostEvent, SettingSpec, Text, TooltipDisplay, TooltipElement, TooltipImage,
    TooltipImageSource, client::hud_layout::PoseOp,
};

const DEBUG_FOOD_STATS_ID: &str = "appleskin_hzn:food_stats";
const ICONS_SPRITE: &str = "appleskin_hzn:gui/icons";
const OUTLINE_SPRITE: &str = "appleskin_hzn:gui/sprites/tooltip_hunger_outline";
const TOOLTIP_MARKER_ITEM: &str = "minecraft:structure_void";

#[derive(Clone, Debug)]
struct InspectedFood {
    food: FoodValue,
    effects: Vec<ConsumeEffectInfo>,
    is_rotten: bool,
}

#[derive(Clone, Debug)]
struct PendingTooltip {
    x: f32,
    y: f32,
    overlay: TooltipFoodOverlay,
}

#[derive(Default)]
struct ModState {
    gui_ticks: i32,
    flash: FlashState,
    riding_mount: bool,
    marker_item: Option<ItemKey>,
    pending_tooltips: Vec<PendingTooltip>,
}

#[derive(Default)]
struct Mod {
    state: Mutex<ModState>,
}

fn lock_state(mutex: &Mutex<ModState>) -> std::sync::MutexGuard<'_, ModState> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

fn marker_item_key(host: &dyn ClientHost, state: &mut ModState) -> Option<ItemKey> {
    if state.marker_item.is_none()
        && let Ok(id) = Identifier::parse(TOOLTIP_MARKER_ITEM)
    {
        state.marker_item = host.item_key(&id);
    }
    state.marker_item
}

fn food_sprite_name(rotten: bool, kind: FoodSpriteKind) -> &'static str {
    match (rotten, kind) {
        (false, FoodSpriteKind::Empty) => "gui/sprites/hud/food_empty",
        (false, FoodSpriteKind::Half) => "gui/sprites/hud/food_half",
        (false, FoodSpriteKind::Full) => "gui/sprites/hud/food_full",
        (true, FoodSpriteKind::Empty) => "gui/sprites/hud/food_empty_hunger",
        (true, FoodSpriteKind::Half) => "gui/sprites/hud/food_half_hunger",
        (true, FoodSpriteKind::Full) => "gui/sprites/hud/food_full_hunger",
    }
}

#[derive(Clone, Copy)]
enum FoodSpriteKind {
    Empty,
    Half,
    Full,
}

fn heart_sprite_name(kind: HeartSpriteKind) -> &'static str {
    match kind {
        HeartSpriteKind::Container => "gui/sprites/hud/heart/container",
        HeartSpriteKind::Half => "gui/sprites/hud/heart/half",
        HeartSpriteKind::Full => "gui/sprites/hud/heart/full",
    }
}

#[derive(Clone, Copy)]
enum HeartSpriteKind {
    Container,
    Half,
    Full,
}

fn inspect_food(host: &dyn ClientHost, stack: &ItemStack) -> Option<InspectedFood> {
    let food_id = ComponentId::new("minecraft:food")?;
    let consumable_id = ComponentId::new("minecraft:consumable")?;
    let resource = ItemResource::of(stack);

    let food_tag = resource.value(host, &food_id)?;
    let food = parse_food(&food_tag)?;

    let consumable_tag = resource.value(host, &consumable_id);
    let is_consumable = consumable_tag.is_some() || resource.use_duration(host).unwrap_or(0) > 0;
    if !is_consumable {
        return None;
    }

    let mut effects = consumable_tag
        .as_ref()
        .map(parse_consumable_effects)
        .unwrap_or_default();

    // Fallback when a built-in food item exposes use_duration without inline consumable NBT.
    if consumable_tag.is_none() {
        const BUILTIN_EFFECTS: &[(&str, &str, i32, i32)] = &[
            ("minecraft:rotten_flesh", "minecraft:hunger", 600, 0),
            ("minecraft:spider_eye", "minecraft:poison", 100, 0),
            ("minecraft:poisonous_potato", "minecraft:poison", 100, 0),
            ("minecraft:pufferfish", "minecraft:poison", 1200, 1),
            ("minecraft:golden_apple", "minecraft:regeneration", 100, 1),
            ("minecraft:enchanted_golden_apple", "minecraft:regeneration", 400, 1),
        ];
        for &(item_id, eff_id, duration, amplifier) in BUILTIN_EFFECTS {
            if Identifier::parse(item_id)
                .ok()
                .and_then(|ident| host.item_key(&ident))
                == Some(stack.item())
            {
                effects.push(ConsumeEffectInfo {
                    id: eff_id.to_owned(),
                    duration,
                    amplifier,
                });
                break;
            }
        }
    }

    let is_rotten = effects.iter().any(|eff| {
        host.effect_key(&eff.id)
            .and_then(|key| key.category(host))
            == Some(EffectCategory::Harmful)
    });

    Some(InspectedFood {
        food,
        effects,
        is_rotten,
    })
}

fn draw_saturation_icons(
    draw: &mut dyn DrawContext,
    right: f32,
    top: f32,
    offsets: &[[i32; 2]],
    saturation_level: f32,
    saturation_gained: f32,
    alpha: f32,
) {
    let tint = with_alpha(0xFFFFFF, alpha);
    for (i, frame) in saturation_overlay_icons(saturation_level, saturation_gained) {
        let [ox, oy] = offsets.get(i).copied().unwrap_or([-(i as i32 * 8) - 9, 0]);
        let x = right + ox as f32;
        let y = top + oy as f32;
        let src = [frame as f32 * ICON, 0.0, ICON, ICON];
        if !draw.sprite(ICONS_SPRITE, src, [x, y, ICON, ICON], Some(tint)) {
            let a = (alpha.clamp(0.0, 1.0) * 255.0).floor() as u32;
            for (row, line) in SATURATION[frame].iter().enumerate() {
                for (col, ch) in line.bytes().enumerate() {
                    if let Some(argb) = cell_color(ch) {
                        let color = (a << 24) | (argb & 0x00FF_FFFF);
                        draw.fill([x + col as f32, y + row as f32, 1.0, 1.0], color);
                    }
                }
            }
        }
    }
}

fn draw_exhaustion_underlay(draw: &mut dyn DrawContext, right: f32, top: f32, exhaustion: f32) {
    let w = exhaustion_width(exhaustion);
    if w <= 0 {
        return;
    }
    let wf = w as f32;
    let dx = right - wf;
    let tint = with_alpha(0xFFFFFF, 0.75);
    if !draw.sprite(ICONS_SPRITE, [81.0 - wf, 18.0, wf, 9.0], [dx, top, wf, 9.0], Some(tint)) {
        let pixel = with_alpha(PALETTE[9], 0.75);
        for row in 0..9 {
            for col in (81 - w)..81 {
                if (row + col) % 2 == 0 {
                    draw.fill([(right - 81.0) + col as f32, top + row as f32, 1.0, 1.0], pixel);
                }
            }
        }
    }
}

fn draw_scaled_label(draw: &mut dyn DrawContext, text: &str, x: f32, y: f32, dx: i32, dy: i32) {
    let draws = [
        GuiDraw::Push(vec![PoseOp::Translate(x, y), PoseOp::Scale(0.75, 0.75)]),
        GuiDraw::Text {
            text: Text::literal(text),
            x: dx,
            y: dy,
            color: 0xFFAA_AAAA,
            shadow: true,
        },
        GuiDraw::Pop,
    ];
    if draw.gui_paint(&draws).is_err() {
        draw.text(text, [x + dx as f32 * 0.75, y + dy as f32 * 0.75], 0xFFAA_AAAA, true);
    }
}

fn draw_tooltip_saturation_fallback(
    draw: &mut dyn DrawContext,
    x: f32,
    y: f32,
    share: f32,
    negative: bool,
    alpha: f32,
) {
    let rgb = if negative { 0xE01700 } else { 0xC2A100 };
    let border = with_alpha(0x0A0A0A, alpha);
    let fill = with_alpha(rgb, alpha);
    draw.fill([x + 1.0, y + 1.0, 5.0, 5.0], border);
    let cols = if share >= 1.0 {
        4
    } else if share > 0.5 {
        3
    } else if share > 0.25 {
        2
    } else if share > 0.0 {
        1
    } else {
        0
    };
    if cols > 0 {
        draw.fill([x + 1.0, y + 2.0, cols as f32, 3.0], fill);
    }
}

fn draw_food_tooltip(draw: &mut dyn DrawContext, pending: &PendingTooltip) {
    let overlay = &pending.overlay;
    let default_hunger = overlay.default_food.nutrition;
    let modified_hunger = overlay.modified_food.nutrition;

    // Hunger row (right to left so drumsticks face the right way).
    let mut x = pending.x + (overlay.hunger_bars - 1) as f32 * 9.0;
    let y = pending.y;
    let empty_sprite = food_sprite_name(false, FoodSpriteKind::Empty);

    let mut i = 0;
    while i < overlay.hunger_bars * 2 {
        draw.sprite(empty_sprite, [0.0, 0.0, 9.0, 9.0], [x, y, 9.0, 9.0], None);

        let outline = FoodOutline::classify(modified_hunger, default_hunger, i);
        if outline != FoodOutline::Normal {
            draw.sprite(
                OUTLINE_SPRITE,
                [0.0, 0.0, 9.0, 9.0],
                [x, y, 9.0, 9.0],
                Some(outline.argb()),
            );
        }

        let default_half = default_hunger - 1 == i;
        let default_sprite = food_sprite_name(
            overlay.is_rotten,
            if default_half {
                FoodSpriteKind::Half
            } else {
                FoodSpriteKind::Full
            },
        );
        draw.sprite(
            default_sprite,
            [0.0, 0.0, 9.0, 9.0],
            [x, y, 9.0, 9.0],
            Some(with_alpha(0xFFFFFF, 0.25)),
        );

        if modified_hunger > i {
            let modified_half = modified_hunger - 1 == i;
            let modified_sprite = food_sprite_name(
                overlay.is_rotten,
                if modified_half {
                    FoodSpriteKind::Half
                } else {
                    FoodSpriteKind::Full
                },
            );
            draw.sprite(modified_sprite, [0.0, 0.0, 9.0, 9.0], [x, y, 9.0, 9.0], None);
        }

        x -= 9.0;
        i += 2;
    }

    if let Some(ref text) = overlay.hunger_bars_text {
        x += 18.0;
        draw_scaled_label(draw, text, x, y, 2, 2);
    }

    // Saturation row (right to left, 7x7 icons).
    let mut sx = pending.x + (overlay.saturation_bars - 1) as f32 * 7.0;
    let sy = pending.y + 10.0;
    let modified_sat = overlay.modified_food.saturation;
    let abs_sat = modified_sat.abs();
    let v = if modified_sat >= 0.0 { 27.0 } else { 34.0 };

    let mut j = 0;
    while j < overlay.saturation_bars * 2 {
        let share = (abs_sat - j as f32) / 2.0;
        let faded = abs_sat <= j as f32;
        let alpha = if faded { 0.5 } else { 1.0 };
        let u = tooltip_saturation_u(share);
        let color = with_alpha(0xFFFFFF, alpha);
        if !draw.sprite(ICONS_SPRITE, [u, v, 7.0, 7.0], [sx, sy, 7.0, 7.0], Some(color)) {
            draw_tooltip_saturation_fallback(draw, sx, sy, share, modified_sat < 0.0, alpha);
        }
        sx -= 7.0;
        j += 2;
    }

    if let Some(ref text) = overlay.saturation_bars_text {
        sx += 14.0;
        draw_scaled_label(draw, text, sx, sy, 2, 1);
    }
}

fn should_show_tooltip(host: &dyn ClientHost, stack: &ItemStack) -> bool {
    if !TooltipDisplay::for_stack(stack).name_visible(false) {
        return false;
    }
    let show_shift = host
        .setting_bool("show_food_values_in_tooltip")
        .unwrap_or(true);
    let show_always = host
        .setting_bool("show_food_values_in_tooltip_always")
        .unwrap_or(true);
    (show_shift && host.modifiers().shift()) || show_always
}

fn build_tooltip_overlay(host: &dyn ClientHost, stack: &ItemStack) -> Option<TooltipFoodOverlay> {
    if !should_show_tooltip(host, stack) {
        return None;
    }
    let inspected = inspect_food(host, stack)?;
    let overlay = TooltipFoodOverlay::new(inspected.food, inspected.food, inspected.is_rotten);
    overlay.should_render().then_some(overlay)
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _cx: &RegistrationContext) {
        let toggles = [
            (
                "show_food_values_in_tooltip",
                "text.autoconfig.appleskin.option.showFoodValuesInTooltip",
                "text.autoconfig.appleskin.option.showFoodValuesInTooltip.@Tooltip",
                true,
            ),
            (
                "show_food_values_in_tooltip_always",
                "text.autoconfig.appleskin.option.showFoodValuesInTooltipAlways",
                "text.autoconfig.appleskin.option.showFoodValuesInTooltipAlways.@Tooltip",
                true,
            ),
            (
                "show_saturation_hud_overlay",
                "text.autoconfig.appleskin.option.showSaturationHudOverlay",
                "text.autoconfig.appleskin.option.showSaturationHudOverlay.@Tooltip",
                true,
            ),
            (
                "show_food_values_hud_overlay",
                "text.autoconfig.appleskin.option.showFoodValuesHudOverlay",
                "text.autoconfig.appleskin.option.showFoodValuesHudOverlay.@Tooltip",
                true,
            ),
            (
                "show_food_values_hud_overlay_when_offhand",
                "text.autoconfig.appleskin.option.showFoodValuesHudOverlayWhenOffhand",
                "text.autoconfig.appleskin.option.showFoodValuesHudOverlayWhenOffhand.@Tooltip",
                true,
            ),
            (
                "show_food_exhaustion_hud_underlay",
                "text.autoconfig.appleskin.option.showFoodExhaustionHudUnderlay",
                "text.autoconfig.appleskin.option.showFoodExhaustionHudUnderlay.@Tooltip",
                true,
            ),
            (
                "show_food_health_hud_overlay",
                "text.autoconfig.appleskin.option.showFoodHealthHudOverlay",
                "text.autoconfig.appleskin.option.showFoodHealthHudOverlay.@Tooltip",
                true,
            ),
            (
                "show_vanilla_animations_overlay",
                "text.autoconfig.appleskin.option.showVanillaAnimationsOverlay",
                "text.autoconfig.appleskin.option.showVanillaAnimationsOverlay.@Tooltip",
                true,
            ),
        ];
        for (key, label, tooltip, default) in toggles {
            let _ = r.register_setting(SettingSpec::toggle(key, label, default).tooltip(tooltip));
        }
        let _ = r.register_setting(
            SettingSpec::range(
                "max_hud_overlay_flash_alpha",
                "text.autoconfig.appleskin.option.maxHudOverlayFlashAlpha",
                0.0,
                1.0,
                0.05,
                0.65,
            )
            .tooltip("text.autoconfig.appleskin.option.maxHudOverlayFlashAlpha.@Tooltip"),
        );
    }

    fn register_debug_entries(
        &self,
        _cx: &RegistrationContext,
        event: &mut RegisterDebugEntriesEvent<'_>,
    ) {
        event.entries.push(DebugEntry {
            id: DEBUG_FOOD_STATS_ID.to_owned(),
            group: DebugGroup::Position,
        });
    }

    fn debug_entry_lines(&self, host: &dyn ClientHost, event: &mut DebugEntryLinesEvent<'_>) {
        if event.id != DEBUG_FOOD_STATS_ID {
            return;
        }
        let Some(player) = host.local_player() else {
            return;
        };
        let Some(food) = player.food(host) else {
            return;
        };
        event
            .lines
            .push(format_debug_food_line(food.level, food.saturation, food.exhaustion));
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _event: &mut ClientTickPostEvent) {
        let max_alpha = host
            .setting_f64("max_hud_overlay_flash_alpha")
            .unwrap_or(0.65) as f32;
        let paused = host.is_paused();
        let mut state = lock_state(&self.state);
        state.flash.tick(max_alpha);
        if !paused {
            state.gui_ticks = state.gui_ticks.wrapping_add(1);
        }
    }

    fn gui_layer_pre(&self, host: &dyn ClientHost, event: &mut GuiLayerPreEvent<'_>) -> Flow {
        match event.layer {
            "minecraft:hotbar" => {
                lock_state(&self.state).riding_mount = false;
            }
            "minecraft:vehicle_health" => {
                lock_state(&self.state).riding_mount = true;
            }
            "minecraft:player_health" => {
                if host.is_paused()
                    || lock_state(&self.state).riding_mount
                    || !host
                        .setting_bool("show_food_exhaustion_hud_underlay")
                        .unwrap_or(true)
                {
                    return Flow::Continue;
                }
                let Some(player) = host.local_player() else {
                    return Flow::Continue;
                };
                if player.is_creative(host).unwrap_or(false)
                    || player.is_spectator(host).unwrap_or(false)
                {
                    return Flow::Continue;
                }
                let Some(food) = player.food(host) else {
                    return Flow::Continue;
                };
                let [width, height] = event.draw.size();
                let right = width / 2.0 + BAR_RIGHT;
                let top = height - BAR_BOTTOM;
                draw_exhaustion_underlay(event.draw, right, top, food.exhaustion);
            }
            _ => {}
        }
        Flow::Continue
    }

    fn gui_layer_post(&self, host: &dyn ClientHost, event: &mut GuiLayerPostEvent<'_>) {
        if event.layer != "minecraft:player_health" {
            return;
        }
        let (riding_mount, gui_ticks) = {
            let mut state = lock_state(&self.state);
            let riding = state.riding_mount;
            state.riding_mount = false;
            (riding, state.gui_ticks)
        };
        if host.is_paused() {
            return;
        }

        let show_saturation = host
            .setting_bool("show_saturation_hud_overlay")
            .unwrap_or(true);
        let show_food_values = host
            .setting_bool("show_food_values_hud_overlay")
            .unwrap_or(true);
        let show_offhand = host
            .setting_bool("show_food_values_hud_overlay_when_offhand")
            .unwrap_or(true);
        let show_health = host
            .setting_bool("show_food_health_hud_overlay")
            .unwrap_or(true);
        let animate = host
            .setting_bool("show_vanilla_animations_overlay")
            .unwrap_or(true);

        if !show_saturation && !show_food_values && !show_health {
            return;
        }

        let Some(player) = host.local_player() else {
            return;
        };
        if player.is_creative(host).unwrap_or(false) || player.is_spectator(host).unwrap_or(false) {
            return;
        }
        let Some(food) = player.food(host) else {
            return;
        };
        let Some(living) = host.living(player.entity()) else {
            return;
        };
        let health = living.health(host).unwrap_or(20.0);
        let max_health = living.max_health(host).unwrap_or(20.0);
        let absorption = living.absorption(host).unwrap_or(0.0);

        let has_regen = host
            .effect_key("minecraft:regeneration")
            .is_some_and(|k| living.potion_affected(host, k));
        let has_poison = host
            .effect_key("minecraft:poison")
            .is_some_and(|k| living.potion_affected(host, k));
        let has_wither = host
            .effect_key("minecraft:wither")
            .is_some_and(|k| living.potion_affected(host, k));

        let offsets = compute_hud_offsets(
            gui_ticks,
            health,
            max_health,
            absorption,
            food.level,
            food.saturation,
            has_regen,
            animate,
        );

        let [width, height] = event.draw.size();
        let right = width / 2.0 + BAR_RIGHT;
        let left = width / 2.0 - BAR_LEFT;
        let top = height - BAR_BOTTOM;

        if !riding_mount && show_saturation {
            draw_saturation_icons(
                event.draw,
                right,
                top,
                &offsets.food,
                food.saturation,
                0.0,
                1.0,
            );
        }

        let held = [Some(Hand::Main), show_offhand.then_some(Hand::Off)]
            .into_iter()
            .flatten()
            .find_map(|hand| {
                let stack = living.hand_item(host, hand)?;
                let inspected = inspect_food(host, &stack)?;
                let can_eat = player
                    .can_eat(host, inspected.food.can_always_eat)
                    .unwrap_or(false);
                can_eat.then_some(inspected)
            });

        let Some(inspected) = held else {
            lock_state(&self.state).flash.reset();
            return;
        };
        let flash_alpha = lock_state(&self.state).flash.alpha;

        if !riding_mount && show_food_values {
            let empty_sprite = food_sprite_name(inspected.is_rotten, FoodSpriteKind::Empty);
            let bg_tint = with_alpha(0xFFFFFF, flash_alpha * 0.25);
            let fg_tint = with_alpha(0xFFFFFF, flash_alpha);
            for (idx, is_half) in hunger_overlay_icons(food.level, inspected.food.nutrition) {
                let [ox, oy] = offsets
                    .food
                    .get(idx)
                    .copied()
                    .unwrap_or([-(idx as i32 * 8) - 9, 0]);
                let x = right + ox as f32;
                let y = top + oy as f32;
                event.draw.sprite(
                    empty_sprite,
                    [0.0, 0.0, ICON, ICON],
                    [x, y, ICON, ICON],
                    Some(bg_tint),
                );
                let icon_sprite = food_sprite_name(
                    inspected.is_rotten,
                    if is_half {
                        FoodSpriteKind::Half
                    } else {
                        FoodSpriteKind::Full
                    },
                );
                event.draw.sprite(
                    icon_sprite,
                    [0.0, 0.0, ICON, ICON],
                    [x, y, ICON, ICON],
                    Some(fg_tint),
                );
            }

            if show_saturation {
                let gained = capped_saturation_gained(
                    food.level,
                    food.saturation,
                    inspected.food.nutrition,
                    inspected.food.saturation,
                );
                draw_saturation_icons(
                    event.draw,
                    right,
                    top,
                    &offsets.food,
                    food.saturation,
                    gained,
                    flash_alpha,
                );
            }
        }

        if show_health
            && !offsets.health.is_empty()
            && food.level < 18
            && !has_poison
            && !has_wither
            && !has_regen
        {
            let inc = estimated_health_increment(
                health,
                max_health,
                food.level,
                food.saturation,
                food.exhaustion,
                inspected.food,
                &inspected.effects,
                true,
            );
            let modified_health = (health + inc).min(max_health);
            if modified_health > health {
                let bg_tint = with_alpha(0xFFFFFF, flash_alpha * 0.25);
                let fg_tint = with_alpha(0xFFFFFF, flash_alpha);
                for (idx, is_half) in health_overlay_icons(health, modified_health) {
                    let [ox, oy] = offsets
                        .health
                        .get(idx)
                        .copied()
                        .unwrap_or([(idx as i32 % 10) * 8, -(idx as i32 / 10) * 10]);
                    let x = left + ox as f32;
                    let y = top + oy as f32;
                    event.draw.sprite(
                        heart_sprite_name(HeartSpriteKind::Container),
                        [0.0, 0.0, ICON, ICON],
                        [x, y, ICON, ICON],
                        Some(bg_tint),
                    );
                    let heart_sprite = heart_sprite_name(if is_half {
                        HeartSpriteKind::Half
                    } else {
                        HeartSpriteKind::Full
                    });
                    event.draw.sprite(
                        heart_sprite,
                        [0.0, 0.0, ICON, ICON],
                        [x, y, ICON, ICON],
                        Some(fg_tint),
                    );
                }
            }
        }
    }

    fn render_tooltip_gather_components(
        &self,
        host: &dyn ClientHost,
        event: &mut RenderTooltipGatherComponentsEvent<'_>,
    ) -> Flow {
        let Some(overlay) = build_tooltip_overlay(host, event.stack) else {
            return Flow::Continue;
        };
        let marker = {
            let mut state = lock_state(&self.state);
            marker_item_key(host, &mut state)
        };
        let Some(marker_key) = marker else {
            return Flow::Continue;
        };

        let hunger_text_w = overlay
            .hunger_bars_text
            .as_deref()
            .map(|t| host.text_width(&Text::literal(t)).unwrap_or(0) as f32)
            .unwrap_or(0.0);
        let sat_text_w = overlay
            .saturation_bars_text
            .as_deref()
            .map(|t| host.text_width(&Text::literal(t)).unwrap_or(0) as f32)
            .unwrap_or(0.0);

        event.elements.push(TooltipElement::Image(TooltipImage {
            source: TooltipImageSource::ItemPart(ItemStack::of(marker_key)),
            size: [overlay.width(hunger_text_w, sat_text_w), overlay.height()],
        }));
        Flow::Continue
    }

    fn render_tooltip_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut RenderTooltipPreEvent<'_>,
    ) -> Flow {
        let Some(stack) = event.stack else {
            return Flow::Continue;
        };
        let marker_key = {
            let mut state = lock_state(&self.state);
            marker_item_key(host, &mut state)
        };
        let Some(marker_key) = marker_key else {
            return Flow::Continue;
        };
        let Some(marker_idx) = event.elements.iter().position(|elem| {
            matches!(
                elem,
                TooltipElement::Image(TooltipImage {
                    source: TooltipImageSource::ItemPart(item),
                    ..
                }) if item.item() == marker_key
            )
        }) else {
            return Flow::Continue;
        };
        let Some(overlay) = build_tooltip_overlay(host, stack) else {
            return Flow::Continue;
        };

        let sizes: Vec<[f32; 2]> = event
            .elements
            .iter()
            .map(|elem| match elem {
                TooltipElement::Text(text, _) => {
                    [host.text_width(text).unwrap_or(0) as f32, 10.0]
                }
                TooltipElement::Image(img) => img.size,
            })
            .collect();
        let (box_size, offsets) = horizon_tooltip_layout(&sizes);
        let (tx, ty) = horizon_tooltip_position(
            (event.place.x as f32, event.place.y as f32),
            (event.screen_width as f32, event.screen_height as f32),
            (box_size[0].ceil(), box_size[1].ceil()),
        );
        let y_off = offsets.get(marker_idx).copied().unwrap_or(0.0);

        let mut state = lock_state(&self.state);
        if state.pending_tooltips.len() < 8 {
            state.pending_tooltips.push(PendingTooltip {
                x: tx,
                y: ty + y_off,
                overlay,
            });
        }
        Flow::Continue
    }

    fn screen_render_post(&self, _host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        let pending = std::mem::take(&mut lock_state(&self.state).pending_tooltips);
        for tooltip in pending {
            draw_food_tooltip(event.draw, &tooltip);
        }
    }
}
