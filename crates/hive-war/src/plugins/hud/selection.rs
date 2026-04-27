use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use swarm_combat::{ArmyCapacityUpgrades, Biomass, army_upgrade_cost};
use swarm_model::{Base, Faction, GameConfig, Unit};

use super::state::{HudState, PendingHudAction};
use super::theme::faction_color_world;

#[derive(Clone, Copy, Debug)]
pub(super) struct RepairPreview {
    pub(super) amount: f32,
    pub(super) cost: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct LocalBaseInfluence {
    player_count: u32,
    enemy_count: u32,
    player_pull: Vec2,
    enemy_pull: Vec2,
    enemy_in_capture_radius: bool,
}

impl LocalBaseInfluence {
    fn dominant_pull(self, faction: Faction) -> Option<(Vec2, f32)> {
        let (count, pull) = match faction {
            Faction::Player => (self.player_count, self.player_pull),
            Faction::Enemy => (self.enemy_count, self.enemy_pull),
            Faction::Neutral => return None,
        };
        if count == 0 || pull.length_squared() <= f32::EPSILON {
            return None;
        }

        Some((pull.normalize(), ((count as f32) / 5.0).clamp(0.2, 1.0)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BaseStatusCue {
    UpgradePending,
    RepairPending,
}

fn sample_base_influence(
    base: &Base,
    base_pos: Vec2,
    unit_positions: &Query<(&Transform, &Unit)>,
) -> LocalBaseInfluence {
    let mut influence = LocalBaseInfluence::default();
    let influence_radius = base.capture_radius * 1.65;

    for (unit_tf, unit) in unit_positions.iter() {
        if unit.faction == Faction::Neutral {
            continue;
        }

        let offset = unit_tf.translation.truncate() - base_pos;
        let dist = offset.length();
        if dist <= base.capture_radius && base.faction.is_enemy_of(&unit.faction) {
            influence.enemy_in_capture_radius = true;
        }
        if dist > influence_radius {
            continue;
        }

        let weight = 1.0 - (dist / influence_radius);
        let dir = if dist > 0.001 {
            offset / dist
        } else {
            Vec2::ZERO
        };

        match unit.faction {
            Faction::Player => {
                influence.player_count += 1;
                influence.player_pull += dir * weight.max(0.1);
            }
            Faction::Enemy => {
                influence.enemy_count += 1;
                influence.enemy_pull += dir * weight.max(0.1);
            }
            Faction::Neutral => {}
        }
    }

    influence
}

pub(super) fn base_is_contested(
    base: &Base,
    base_pos: Vec2,
    unit_positions: &Query<(&Transform, &Unit)>,
) -> bool {
    if base.faction == Faction::Neutral {
        return false;
    }

    sample_base_influence(base, base_pos, unit_positions).enemy_in_capture_radius
}

pub(super) fn validate_base_upgrade(
    base: &Base,
    base_pos: Vec2,
    cfg: &GameConfig,
    biomass: &Biomass,
    unit_positions: &Query<(&Transform, &Unit)>,
) -> Result<f32, String> {
    if base.faction != Faction::Player {
        return Err("Only player-controlled bases can upgrade.".into());
    }
    if base.level >= cfg.base_max_level {
        return Err("This base is already at max level.".into());
    }
    if base.is_weak() {
        return Err("Captured bases cannot upgrade during weak recovery.".into());
    }
    if base_is_contested(base, base_pos, unit_positions) {
        return Err("This base is contested right now.".into());
    }

    let cost = cfg.base_upgrade_cost(base.level);
    if biomass.player < cost {
        return Err(format!("Need {:.0} biomass to upgrade.", cost));
    }

    Ok(cost)
}

pub(super) fn validate_base_repair(
    base: &Base,
    cfg: &GameConfig,
    biomass: &Biomass,
) -> Result<RepairPreview, String> {
    if base.faction != Faction::Player {
        return Err("Only player-controlled bases can be repaired.".into());
    }
    if base.health >= base.max_health {
        return Err("This base is already fully repaired.".into());
    }
    if base.repair_cooldown > 0.0 {
        return Err(format!(
            "Repair is recharging for {:.1}s.",
            base.repair_cooldown
        ));
    }

    let amount = (base.max_health - base.health).min(cfg.base_repair_amount);
    let cost = cfg.base_repair_cost(amount);
    if biomass.player < cost {
        return Err(format!("Need {:.0} biomass to repair.", cost));
    }

    Ok(RepairPreview { amount, cost })
}

pub(super) fn validate_global_expansion(
    cfg: &GameConfig,
    biomass: &Biomass,
    upgrades: &ArmyCapacityUpgrades,
) -> Result<f32, String> {
    if !cfg.can_buy_global_expansion(upgrades.player_tier) {
        return Err("Global expansion is already at max tier.".into());
    }

    let cost = army_upgrade_cost(upgrades.player_tier, cfg);
    if biomass.player < cost {
        return Err(format!("Need {:.0} biomass for global expansion.", cost));
    }

    Ok(cost)
}

pub(super) fn maintain_hud_state(
    cfg: Res<GameConfig>,
    biomass: Res<Biomass>,
    upgrades: Res<ArmyCapacityUpgrades>,
    game_end: Res<super::super::game_state::GameEnd>,
    base_query: Query<(&Base, &Transform)>,
    unit_positions: Query<(&Transform, &Unit)>,
    mut hud_state: ResMut<HudState>,
) {
    if game_end.0.is_some() || base_query.iter().next().is_none() {
        hud_state.reset_for_match_end();
        return;
    }

    if let Some(selected_base) = hud_state.selected_base()
        && base_query.get(selected_base).is_err()
    {
        hud_state.drop_missing_selection();
    }

    let Some(action) = hud_state.pending_action() else {
        return;
    };

    let invalid_reason = match action {
        PendingHudAction::UpgradeBase(base_entity) => {
            let Ok((base, base_tf)) = base_query.get(base_entity) else {
                hud_state.clear_pending();
                return;
            };
            validate_base_upgrade(
                base,
                base_tf.translation.truncate(),
                &cfg,
                &biomass,
                &unit_positions,
            )
            .err()
        }
        PendingHudAction::RepairBase(base_entity) => {
            let Ok((base, _)) = base_query.get(base_entity) else {
                hud_state.clear_pending();
                return;
            };
            validate_base_repair(base, &cfg, &biomass).err()
        }
        PendingHudAction::GlobalExpansion => {
            validate_global_expansion(&cfg, &biomass, &upgrades).err()
        }
    };

    if let Some(reason) = invalid_reason {
        hud_state.cancel_action(action, reason);
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn select_base_system(
    mouse: Res<ButtonInput<MouseButton>>,
    egui_wants_input: Res<EguiWantsInput>,
    windows: Query<&Window>,
    camera_q: Query<
        (&Camera, &GlobalTransform),
        (
            With<Camera2d>,
            With<crate::plugins::markers::GameplayEntity>,
        ),
    >,
    cfg: Res<GameConfig>,
    base_query: Query<(Entity, &Transform), With<Base>>,
    mut hud_state: ResMut<HudState>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    // Only block selection when egui actually consumed the pointer event (widget clicked/dragged),
    // NOT merely because the cursor is hovering over an egui panel background.
    // `wants_any_pointer_input()` includes `is_pointer_over_area` which would block clicks on
    // world bases that happen to overlap the resource-cluster panel in screen space.
    if egui_wants_input.wants_pointer_input() || egui_wants_input.is_using_pointer() {
        return;
    }

    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((camera, cam_transform)) = camera_q.single() else {
        return;
    };
    let Some(cursor_pos) = window
        .cursor_position()
        .and_then(|pos| camera.viewport_to_world_2d(cam_transform, pos).ok())
    else {
        return;
    };

    let click_radius = cfg.base_radius + 8.0;
    let next_selected = base_query
        .iter()
        .filter_map(|(entity, tf)| {
            let dist = (tf.translation.truncate() - cursor_pos).length();
            (dist <= click_radius).then_some((entity, dist))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(entity, _)| entity);

    hud_state.select_base(next_selected);
}

pub(super) fn draw_selected_base_gizmo(
    cfg: Res<GameConfig>,
    hud_state: Res<HudState>,
    time: Res<Time>,
    base_query: Query<(Entity, &Transform, &Base)>,
    unit_positions: Query<(&Transform, &Unit)>,
    mut gizmos: Gizmos,
) {
    let selected_base = hud_state.selected_base();
    let pending_action = hud_state.pending_action();
    let pulse = 0.5 + 0.5 * (time.elapsed_secs() * 2.2).sin();

    for (entity, base_tf, base) in base_query.iter() {
        let base_pos = base_tf.translation.truncate();
        let influence = sample_base_influence(base, base_pos, &unit_positions);
        let contested = base.faction != Faction::Neutral && influence.enemy_in_capture_radius;
        let health_ratio = (base.health / base.max_health.max(1.0)).clamp(0.0, 1.0);
        let selected = selected_base == Some(entity);
        let pressure = base_pressure(base.faction, influence);
        let under_pressure = contested || pressure > 0.58;
        let pending_cue = pending_base_status_cue(pending_action, entity);
        let show_status_layer = selected
            || contested
            || under_pressure
            || base.is_weak()
            || health_ratio < 1.0
            || base.level > 0;

        if !show_status_layer {
            continue;
        }

        let accent = faction_color_world(base.faction);
        let warning = Color::srgba(1.0, 0.38, 0.42, 0.92);
        let pressure_warning = Color::srgba(1.0, 0.72, 0.36, 0.9);
        let selection_shell = Color::srgba(0.49, 0.92, 1.0, 0.95);
        let recovery = Color::srgba(0.74, 1.0, 0.35, 0.88);
        let health_color = if health_ratio < 0.3 {
            warning
        } else if under_pressure || health_ratio < 0.58 {
            pressure_warning
        } else if base.is_weak() {
            recovery
        } else {
            accent
        };

        draw_progress_arc(
            &mut gizmos,
            base_pos,
            cfg.base_radius + 11.0,
            PI * 0.84,
            PI * 0.16,
            1.0,
            color_with_alpha(accent, if selected { 0.18 } else { 0.08 }),
        );
        draw_progress_arc(
            &mut gizmos,
            base_pos,
            cfg.base_radius + 11.0,
            PI * 0.84,
            PI * 0.16,
            health_ratio.max(0.03),
            if pending_cue == Some(BaseStatusCue::RepairPending) {
                color_with_alpha(selection_shell, 0.68 + 0.18 * pulse)
            } else {
                color_with_alpha(health_color, if selected { 0.96 } else { 0.82 })
            },
        );
        draw_level_pips(
            &mut gizmos,
            base_pos,
            cfg.base_radius + 17.0,
            base.level,
            cfg.base_max_level,
            if pending_cue == Some(BaseStatusCue::UpgradePending) {
                color_with_alpha(selection_shell, 0.78 + 0.16 * pulse)
            } else {
                accent
            },
            selected,
        );

        let weak_ratio = if cfg.base_capture_weak_seconds > 0.0 {
            (base.capture_weak_timer / cfg.base_capture_weak_seconds).clamp(0.0, 1.0)
        } else {
            0.0
        };
        if base.is_weak() || selected {
            draw_progress_arc(
                &mut gizmos,
                base_pos,
                cfg.base_radius + 8.0,
                PI * 1.22,
                PI * 1.48,
                1.0,
                color_with_alpha(recovery, if selected { 0.16 } else { 0.08 }),
            );
            if weak_ratio > 0.0 {
                draw_progress_arc(
                    &mut gizmos,
                    base_pos,
                    cfg.base_radius + 8.0,
                    PI * 1.22,
                    PI * 1.48,
                    weak_ratio,
                    color_with_alpha(recovery, if selected { 0.86 } else { 0.56 }),
                );
            }
        }

        let repair_ratio = if cfg.base_repair_cooldown_sec > 0.0 {
            (base.repair_cooldown / cfg.base_repair_cooldown_sec).clamp(0.0, 1.0)
        } else {
            0.0
        };
        if repair_ratio > 0.0 || selected {
            draw_progress_arc(
                &mut gizmos,
                base_pos,
                cfg.base_radius + 8.0,
                PI * 1.52,
                PI * 1.78,
                1.0,
                color_with_alpha(selection_shell, if selected { 0.16 } else { 0.08 }),
            );
            if repair_ratio > 0.0 {
                draw_progress_arc(
                    &mut gizmos,
                    base_pos,
                    cfg.base_radius + 8.0,
                    PI * 1.52,
                    PI * 1.78,
                    repair_ratio,
                    if pending_cue == Some(BaseStatusCue::RepairPending) {
                        color_with_alpha(selection_shell, 0.72 + 0.16 * pulse)
                    } else {
                        color_with_alpha(selection_shell, if selected { 0.76 } else { 0.48 })
                    },
                );
            }
        }

        if selected {
            draw_selection_brackets(
                &mut gizmos,
                base_pos,
                cfg.base_radius + 13.0 + pulse * 2.0,
                11.0,
                selection_shell,
            );
            gizmos.circle_2d(base_pos, cfg.base_radius + 7.0, accent);
            gizmos.circle_2d(
                base_pos,
                base.capture_radius,
                color_with_alpha(accent, if contested { 0.3 } else { 0.14 }),
            );

            if let Some((dir, strength)) = influence.dominant_pull(Faction::Player) {
                draw_direction_marker(
                    &mut gizmos,
                    base_pos,
                    cfg.base_radius + 10.0,
                    base.capture_radius - 4.0 + 10.0 * strength,
                    dir,
                    color_with_alpha(Color::srgba(0.36, 0.95, 0.72, 0.95), 0.45 + 0.35 * strength),
                );
            }
            if let Some((dir, strength)) = influence.dominant_pull(Faction::Enemy) {
                draw_direction_marker(
                    &mut gizmos,
                    base_pos,
                    cfg.base_radius + 10.0,
                    base.capture_radius - 2.0 + 12.0 * strength,
                    dir,
                    color_with_alpha(warning, 0.55 + 0.25 * strength),
                );
            }
        }

        if contested {
            gizmos.circle_2d(
                base_pos,
                base.capture_radius + 2.0 + pulse * 1.5,
                color_with_alpha(warning, if selected { 0.78 } else { 0.42 }),
            );
            if !selected && let Some((dir, strength)) = influence.dominant_pull(Faction::Enemy) {
                draw_direction_marker(
                    &mut gizmos,
                    base_pos,
                    cfg.base_radius + 8.0,
                    base.capture_radius + 8.0 + 8.0 * strength,
                    dir,
                    color_with_alpha(warning, 0.5 + 0.2 * strength),
                );
            }
        }

        if under_pressure
            && !contested
            && !selected
            && let Some(threat_faction) = threatening_faction(base.faction)
            && let Some((dir, strength)) = influence.dominant_pull(threat_faction)
        {
            draw_direction_marker(
                &mut gizmos,
                base_pos,
                cfg.base_radius + 7.0,
                cfg.base_radius + 17.0 + 8.0 * strength,
                dir,
                color_with_alpha(pressure_warning, 0.38 + 0.2 * strength),
            );
        }

        if base.is_weak() {
            gizmos.circle_2d(
                base_pos,
                cfg.base_radius + 4.0 + pulse,
                color_with_alpha(recovery, if selected { 0.72 } else { 0.35 }),
            );
        }
    }
}

fn pending_base_status_cue(
    pending_action: Option<PendingHudAction>,
    entity: Entity,
) -> Option<BaseStatusCue> {
    match pending_action {
        Some(PendingHudAction::UpgradeBase(target)) if target == entity => {
            Some(BaseStatusCue::UpgradePending)
        }
        Some(PendingHudAction::RepairBase(target)) if target == entity => {
            Some(BaseStatusCue::RepairPending)
        }
        _ => None,
    }
}

fn threatening_faction(base_faction: Faction) -> Option<Faction> {
    match base_faction {
        Faction::Player => Some(Faction::Enemy),
        Faction::Enemy => Some(Faction::Player),
        Faction::Neutral => None,
    }
}

fn base_pressure(base_faction: Faction, influence: LocalBaseInfluence) -> f32 {
    let (friendly, hostile) = match base_faction {
        Faction::Player => (influence.player_count, influence.enemy_count),
        Faction::Enemy => (influence.enemy_count, influence.player_count),
        Faction::Neutral => (
            influence.player_count.max(influence.enemy_count),
            influence.player_count.min(influence.enemy_count),
        ),
    };
    let total = friendly + hostile;
    if total == 0 {
        return 0.0;
    }

    hostile as f32 / total as f32
}

fn draw_selection_brackets(gizmos: &mut Gizmos, center: Vec2, radius: f32, arm: f32, color: Color) {
    for step in 0..4 {
        let angle = FRAC_PI_4 + step as f32 * FRAC_PI_2;
        let dir = Vec2::from_angle(angle);
        let tangent = Vec2::new(-dir.y, dir.x);
        let corner = center + dir * radius;
        let radial_start = center + dir * (radius - arm);
        let tangent_extent = tangent * arm * 0.45;

        gizmos.line_2d(radial_start, corner, color);
        gizmos.line_2d(corner - tangent_extent, corner, color);
        gizmos.line_2d(corner, corner + tangent_extent, color);
    }
}

fn draw_direction_marker(
    gizmos: &mut Gizmos,
    center: Vec2,
    inner_radius: f32,
    outer_radius: f32,
    direction: Vec2,
    color: Color,
) {
    if direction.length_squared() <= f32::EPSILON {
        return;
    }

    let dir = direction.normalize();
    let tip = center + dir * outer_radius;
    let tail = center + dir * inner_radius;
    let wing = Vec2::new(-dir.y, dir.x) * 4.5;

    gizmos.line_2d(tail, tip, color);
    gizmos.line_2d(tip, tip - dir * 7.0 + wing, color);
    gizmos.line_2d(tip, tip - dir * 7.0 - wing, color);
}

fn draw_progress_arc(
    gizmos: &mut Gizmos,
    center: Vec2,
    radius: f32,
    start_angle: f32,
    end_angle: f32,
    progress: f32,
    color: Color,
) {
    let progress = progress.clamp(0.0, 1.0);
    if progress <= 0.0 {
        return;
    }

    let segments = ((14.0 * progress).ceil() as usize).max(1);
    let span_end = start_angle + (end_angle - start_angle) * progress;
    draw_arc(
        gizmos,
        center,
        radius,
        start_angle,
        span_end,
        segments,
        color,
    );
}

fn draw_arc(
    gizmos: &mut Gizmos,
    center: Vec2,
    radius: f32,
    start_angle: f32,
    end_angle: f32,
    segments: usize,
    color: Color,
) {
    let steps = segments.max(1);
    let mut previous = center + Vec2::from_angle(start_angle) * radius;

    for step in 1..=steps {
        let t = step as f32 / steps as f32;
        let angle = start_angle + (end_angle - start_angle) * t;
        let next = center + Vec2::from_angle(angle) * radius;
        gizmos.line_2d(previous, next, color);
        previous = next;
    }
}

fn draw_level_pips(
    gizmos: &mut Gizmos,
    center: Vec2,
    radius: f32,
    level: u32,
    max_level: u32,
    color: Color,
    selected: bool,
) {
    if max_level == 0 {
        return;
    }

    let slots = max_level as usize;
    let spacing = 7.5;
    let start_x = -((slots.saturating_sub(1)) as f32 * spacing * 0.5);
    for slot in 0..slots {
        let filled = slot < level as usize;
        if !filled && !selected {
            continue;
        }

        let pos = center + Vec2::new(start_x + slot as f32 * spacing, radius);
        gizmos.circle_2d(
            pos,
            if filled { 2.3 } else { 1.8 },
            color_with_alpha(color, if filled { 0.95 } else { 0.18 }),
        );
    }
}

fn color_with_alpha(color: Color, alpha: f32) -> Color {
    let rgba = color.to_srgba();
    Color::srgba(rgba.red, rgba.green, rgba.blue, alpha)
}
