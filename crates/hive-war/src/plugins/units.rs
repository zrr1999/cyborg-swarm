use std::collections::HashMap;

use bevy::prelude::*;
use rand::{RngExt, SeedableRng, rngs::SmallRng};

use swarm_combat::ArmyCapacityUpgrades;
use swarm_growth::CyborgGenePool;
use swarm_model::{
    Base, Faction, GameConfig, GeneticTraitBasis, GeneticTraits, MovementSuppression,
    SpawnedByBase, SteeringInertia, TacticalClass, UNIT_RADIUS, Unit, UnitFitness, UnitTarget,
    unit_strength,
};
use swarm_sim::simulation::{
    decay_movement_suppression, level_up_system, own_base_entity_at_position,
    sync_flow_fields_each_frame, unit_hp_regen_system, unit_inside_own_base, update_spatial_hash,
};
use swarm_space::{FlowFieldSet, SpatialHash};
use swarm_tactics::{
    BaseDefenseSignals, SquadUnitSnapshot, assign_units_to_squads, elect_and_assign,
};

use super::beacons::BeaconControlState;
use super::captain_ai::{CAPTAIN_FLOW_BLEND, CaptainAiHint, CaptainAiTimer};
use super::experience::{
    BaseAssaultLedger, CombatContributionLedger, award_xp_shares, distribute_pool_with_main_share,
};
use super::markers::GameplayEntity;
use super::wolf_pack::WolfPack;
use super::{COLOR_ENEMY, COLOR_PLAYER};
use crate::components::{Beacon, BeaconMode, Headquarters};
use crate::steering::{SteeringBlendInput, blended_steering_direction};

/// Weapon reach derived from genetics (ties combat range to `sensor_range`).
#[inline]
pub(crate) fn hit_range_from_traits(traits: &GeneticTraits) -> f32 {
    (traits.sensor_range * 0.32).clamp(UNIT_RADIUS * 2.0, 128.0)
}

/// True if any hostile **unit** is within this unit's weapon range (prioritize unit combat over base).
pub(crate) fn hostile_unit_in_weapon_range(
    entity: Entity,
    pos: Vec2,
    faction: Faction,
    weapon_range: f32,
    hash: &SpatialHash,
    unit_q: &Query<(Entity, &Transform, &Unit, &GeneticTraits)>,
) -> bool {
    let mut found = false;
    hash.for_each_in_radius(pos, weapon_range, |other_e| {
        if found || other_e == entity {
            return;
        }
        let Ok((_, tf, u, _)) = unit_q.get(other_e) else {
            return;
        };
        if !faction.is_enemy_of(&u.faction) {
            return;
        }
        let d = (pos - tf.translation.truncate()).length();
        if d < weapon_range && d > 1e-4 {
            found = true;
        }
    });
    found
}

/// Snapshot-based variant for [`combat_system`] (avoids a second `Query<&Unit>`).
pub(crate) fn hostile_unit_in_weapon_range_snapshot(
    entity: Entity,
    pos: Vec2,
    faction: Faction,
    weapon_range: f32,
    hash: &SpatialHash,
    snapshot: &[(Entity, Vec2, Faction, f32, f32, f32)],
    index_map: &std::collections::HashMap<Entity, usize>,
) -> bool {
    let mut found = false;
    hash.for_each_in_radius(pos, weapon_range, |other_e| {
        if found || other_e == entity {
            return;
        }
        let Some(&i) = index_map.get(&other_e) else {
            return;
        };
        let (_, op, of, _, _, _) = snapshot[i];
        if !faction.is_enemy_of(&of) {
            return;
        }
        let d = (pos - op).length();
        if d < weapon_range && d > 1e-4 {
            found = true;
        }
    });
    found
}

fn nearest_friendly_base_world_pos(
    pos: Vec2,
    faction: Faction,
    bases: &Query<(&Transform, &Base, Option<&Headquarters>), Without<Unit>>,
) -> Option<Vec2> {
    if faction == Faction::Neutral {
        return None;
    }
    let mut best: Option<(Vec2, f32)> = None;
    for (tf, b, _) in bases.iter() {
        if b.faction != faction || b.is_destroyed() {
            continue;
        }
        let p = tf.translation.truncate();
        let d2 = pos.distance_squared(p);
        if best.is_none_or(|(_, bd2)| d2 < bd2) {
            best = Some((p, d2));
        }
    }
    best.map(|(p, _)| p)
}

#[inline]
fn home_rally_steering(
    pos: Vec2,
    faction: Faction,
    health_frac: f32,
    has_chase: bool,
    nearest_enemy_sup: f32,
    st: &swarm_model::SteeringMovementConfig,
    bases: &Query<(&Transform, &Base, Option<&Headquarters>), Without<Unit>>,
) -> (Vec2, f32, bool) {
    if faction == Faction::Neutral {
        return (Vec2::ZERO, 0.0, false);
    }
    let thr = st.home_rally_health_threshold.clamp(0.12, 0.95);
    if health_frac >= thr {
        return (Vec2::ZERO, 0.0, false);
    }
    let Some(target) = nearest_friendly_base_world_pos(pos, faction, bases) else {
        return (Vec2::ZERO, 0.0, false);
    };
    let deficit = ((thr - health_frac) / thr.max(1e-3)).clamp(0.0, 1.0);
    let mut hr = deficit.sqrt();
    if has_chase && nearest_enemy_sup > 0.32 {
        hr *= (1.0 - nearest_enemy_sup).clamp(0.0, 1.0);
    }
    if hr < 0.035 {
        return (Vec2::ZERO, 0.0, false);
    }
    let dir = (target - pos).normalize_or_zero();
    (dir, hr, true)
}

pub(crate) fn in_assaultable_base_capture_radius(
    pos: Vec2,
    unit_faction: Faction,
    bases: &Query<(Entity, &Transform, &Base)>,
) -> bool {
    for (_, base_tf, base) in bases.iter() {
        if unit_faction == base.faction || unit_faction == Faction::Neutral {
            continue;
        }
        if (pos - base_tf.translation.truncate()).length() < base.capture_radius {
            return true;
        }
    }
    false
}

/// When true, this unit only applies **base capture** pressure this frame (no outgoing unit-vs-unit damage).
#[allow(clippy::too_many_arguments)]
pub(crate) fn base_assault_suppresses_unit_combat_outgoing_snapshot(
    entity: Entity,
    pos: Vec2,
    faction: Faction,
    weapon_range: f32,
    hash: &SpatialHash,
    snapshot: &[(Entity, Vec2, Faction, f32, f32, f32)],
    index_map: &std::collections::HashMap<Entity, usize>,
    bases: &Query<(Entity, &Transform, &Base)>,
) -> bool {
    !hostile_unit_in_weapon_range_snapshot(
        entity,
        pos,
        faction,
        weapon_range,
        hash,
        snapshot,
        index_map,
    ) && in_assaultable_base_capture_radius(pos, faction, bases)
}

/// Upper bound for `hit_range_from_traits`; spatial queries must cover at least this radius.
const COMBAT_LOOKUP_RADIUS: f32 = 128.0;

/// Annulus inner radius / outer radius in mesh space (scaled by world radius). Defines ring thickness.
const UNIT_RING_WIDTH_FRAC: f32 = 0.2;

#[derive(Component)]
struct UnitVisualFill;

#[derive(Component)]
struct UnitVisualRing;

/// Elected squad captain — sticky until death, evolution, or fusion replaces the unit.
#[derive(Component)]
pub struct SquadCaptain;

/// Evolution strips [`SquadCaptain`] and inserts this for one frame so the same unit is not
/// immediately re-elected; cleared at the start of the next frame by [`captain_eligibility_clear`].
#[derive(Component)]
pub struct CaptainEligibilityCooldown;

/// Child mesh: small center dot on [`SquadCaptain`] units.
#[derive(Component)]
struct CaptainBadgeGraphic;

/// 0..1 intensity; set to 1.0 when the unit deals damage this frame, decays over time.
#[derive(Component, Default)]
struct AttackFlash(f32);

/// 0..1 intensity; set to 1.0 when the unit takes mitigated damage, decays over time.
#[derive(Component, Default)]
struct HitFlash(f32);

/// Seconds remaining before this unit can fire another visual projectile.
#[derive(Component, Default)]
struct ProjectileCooldown(f32);

/// Semi-transparent disc child showing the stat-boost area when the unit is inside its own base.
#[derive(Component)]
struct UnitBoostAura;

/// Per-unit boost multiplier from being inside a friendly base (1.0 = no boost).
#[derive(Component, Default, Clone, Copy)]
pub struct BaseBoost {
    pub multiplier: f32,
}

const ATTACK_FLASH_DURATION: f32 = 0.18;
const HIT_FLASH_DURATION: f32 = 0.25;
/// Fill briefly brightens toward white when hit — preserves faction hue so enemies stay red
/// and allies stay blue, while still giving clear damage feedback.
const HIT_FLASH_COLOR: Color = Color::WHITE;

/// Stat multiplier per base level while a unit is inside its own base.
const BASE_BOOST_PER_LEVEL: f32 = 0.12;

/// Same λ as [`super::captain_ai::captain_ai_infer`] squad assignment.
const SQUAD_CAPTAIN_LAMBDA: f32 = 0.5;
/// λ for assigning non-captains to a captain when building spread anchors (formula 4 style).
const SQUAD_SPREAD_ASSIGN_LAMBDA: f32 = 0.5;

/// Slightly higher overall damage to make engagements resolve faster.
const COMBAT_DAMAGE_SCALE: f32 = 1.12;

const SUPPRESSION_PER_DAMAGE: f32 = 0.062;
/// At suppression 1.0, speed is multiplied by `1.0 - MAX_SLOW_FROM_SUPPRESSION`.
const MAX_SLOW_FROM_SUPPRESSION: f32 = 0.55;
const MIN_SPEED_FRAC: f32 = 0.34;
pub struct UnitPlugin;

impl Plugin for UnitPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(RngResource(SmallRng::from_rng(&mut rand::rng())))
            .insert_resource(SpatialHash::default())
            .insert_resource(FlowFieldSet::default())
            .insert_resource(CaptainAiHint::default())
            .insert_resource(CaptainAiTimer::default())
            .init_resource::<SquadSpreadCache>()
            .add_systems(
                Update,
                (
                    decay_movement_suppression,
                    tick_base_spawn,
                    sync_flow_fields_each_frame,
                    super::captain_ai::captain_ai_infer,
                    build_squad_spread_hints,
                    update_base_boost,
                    move_units,
                    fill_squad_captain_vacancies,
                    sync_captain_badge_meshes,
                    update_spatial_hash,
                    decay_combat_flash,
                    combat_system,
                    unit_hp_regen_system,
                    level_up_system,
                    update_unit_circle_hp_visuals,
                    update_polygon_unit_hp_fill,
                    reap_dead_units,
                )
                    .chain()
                    .run_if(super::game_state::match_is_active),
            );
    }
}

#[derive(Resource)]
pub struct RngResource(pub SmallRng);

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let a = a.to_srgba();
    let b = b.to_srgba();
    Color::srgba(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
        a.alpha,
    )
}

fn faction_base_color(faction: Faction) -> Color {
    match faction {
        Faction::Player => COLOR_PLAYER,
        Faction::Enemy => COLOR_ENEMY,
        Faction::Neutral => Color::WHITE,
    }
}

fn tint_color(base: Color, t: f32) -> Color {
    let srgba = base.to_srgba();
    Color::srgba(
        (srgba.red + t * 0.3).clamp(0.0, 1.0),
        (srgba.green + t * 0.1).clamp(0.0, 1.0),
        (srgba.blue + t * 0.2).clamp(0.0, 1.0),
        1.0,
    )
}

fn ring_tint(base: Color) -> Color {
    let s = base.to_srgba();
    Color::srgba(
        (s.red * 1.08).clamp(0.0, 1.0),
        (s.green * 1.08).clamp(0.0, 1.0),
        (s.blue * 1.08).clamp(0.0, 1.0),
        0.36,
    )
}

fn unit_visual_sides(faction: Faction) -> u32 {
    match faction {
        Faction::Player => 4,
        Faction::Enemy => 3,
        Faction::Neutral => 6,
    }
}

fn unit_visual_rotation(faction: Faction) -> Quat {
    match faction {
        Faction::Player => Quat::from_rotation_z(std::f32::consts::FRAC_PI_4),
        Faction::Enemy => Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2),
        Faction::Neutral => Quat::IDENTITY,
    }
}

fn cover_core_color(faction: Faction) -> Color {
    match faction {
        Faction::Player => Color::srgb(1.0, 0.82, 0.28),
        Faction::Enemy => Color::srgb(1.0, 0.82, 0.72),
        Faction::Neutral => Color::srgb(1.0, 0.72, 0.34),
    }
}

fn unit_glow_color(faction: Faction) -> Color {
    let c = faction_base_color(faction).to_srgba();
    Color::srgba(c.red, c.green, c.blue, 0.12)
}

fn update_base_boost(
    mut unit_query: Query<(&Transform, &Unit, &mut BaseBoost)>,
    base_query: Query<(Entity, &Transform, &Base)>,
) {
    for (tf, unit, mut boost) in &mut unit_query {
        let pos = tf.translation.truncate();
        if let Some(base_entity) = own_base_entity_at_position(pos, unit.faction, &base_query) {
            if let Ok((_, _, base)) = base_query.get(base_entity) {
                boost.multiplier = 1.0 + base.level as f32 * BASE_BOOST_PER_LEVEL;
            }
        } else {
            boost.multiplier = 1.0;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn tick_base_spawn(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut gene_pool: ResMut<CyborgGenePool>,
    mut wolf_pack: ResMut<WolfPack>,
    cfg: Res<GameConfig>,
    time: Res<Time>,
    mut rng: ResMut<RngResource>,
    upgrades: Res<ArmyCapacityUpgrades>,
    mut base_query: Query<(Entity, &mut Base, &Transform)>,
    spawned_units: Query<(&Unit, &SpawnedByBase)>,
) {
    let mut units_by_base: HashMap<Entity, u32> = HashMap::new();
    let mut player_total = 0u32;
    let mut enemy_total = 0u32;
    for (unit, home) in spawned_units.iter() {
        *units_by_base.entry(home.0).or_default() += 1;
        match unit.faction {
            Faction::Player => player_total += 1,
            Faction::Enemy => enemy_total += 1,
            Faction::Neutral => {}
        }
    }

    for (base_entity, mut base, base_tf) in base_query.iter_mut() {
        // Neutral bases do not spawn units
        if base.faction == Faction::Neutral {
            continue;
        }

        base.spawn_timer += time.delta_secs();
        let spawn_efficiency = base.spawn_efficiency(&cfg).max(0.01);
        let spawn_interval = base.spawn_interval / spawn_efficiency;
        if base.spawn_timer < spawn_interval {
            continue;
        }
        base.spawn_timer = 0.0;

        let units_from_this_base = units_by_base.get(&base_entity).copied().unwrap_or(0);
        if units_from_this_base >= base.max_units {
            continue;
        }

        let (faction_total, faction_cap) = match base.faction {
            Faction::Player => (
                &mut player_total,
                upgrades.faction_unit_cap(&base.faction, &cfg),
            ),
            Faction::Enemy => (
                &mut enemy_total,
                upgrades.faction_unit_cap(&base.faction, &cfg),
            ),
            Faction::Neutral => continue,
        };
        if *faction_total >= faction_cap {
            continue;
        }

        let (sigma, pool_ref) = match base.faction {
            Faction::Player => (cfg.player_mutation_sigma, &mut gene_pool.player),
            Faction::Enemy => (cfg.enemy_mutation_sigma, &mut gene_pool.enemy),
            Faction::Neutral => continue,
        };

        let base_traits = pool_ref.generate_traits(&cfg, &mut rng.0, sigma);
        let traits = wolf_pack.generate_guided_traits(base.faction, &base_traits, &cfg, &mut rng.0);
        pool_ref.record_birth(&traits);

        let offset = Vec2::new(
            rng.0.random_range(-30.0..30.0),
            rng.0.random_range(-30.0..30.0),
        );
        let pos = base_tf.translation.truncate() + offset;

        let target_x = match base.faction {
            Faction::Player => cfg.world_half_w * 0.75,
            Faction::Enemy => -cfg.world_half_w * 0.75,
            Faction::Neutral => continue,
        };
        let target = Vec2::new(
            target_x,
            rng.0
                .random_range(-cfg.world_half_h * 0.5..cfg.world_half_h * 0.5),
        );

        let color = match base.faction {
            Faction::Player => COLOR_PLAYER,
            Faction::Enemy => COLOR_ENEMY,
            Faction::Neutral => continue,
        };
        let speed_t = ((traits.speed - 15.0) / 165.0).clamp(0.0, 1.0);
        let tinted = tint_color(color, speed_t);
        let ring_color = ring_tint(tinted);

        let basis = GeneticTraitBasis(traits.clone());
        let unit = Unit::new(&traits, base.faction);
        let unit_target = UnitTarget { position: target };

        let r = cfg.unit_body_radius(&traits);
        let inner = 1.0 - UNIT_RING_WIDTH_FRAC;
        let hp = (unit.current_health / traits.max_health.max(1e-6)).clamp(0.0, 1.0);
        let fill_scale = r * inner * hp.sqrt();
        let sides = unit_visual_sides(base.faction);
        let rotation = unit_visual_rotation(base.faction);

        commands
            .spawn((
                Transform::from_xyz(pos.x, pos.y, 2.0),
                Visibility::default(),
                unit,
                traits,
                basis,
                UnitFitness::default(),
                SteeringInertia::default(),
                unit_target,
                SpawnedByBase(base_entity),
                MovementSuppression::default(),
                AttackFlash::default(),
                HitFlash::default(),
                BaseBoost::default(),
                ProjectileCooldown::default(),
                GameplayEntity,
            ))
            .with_children(|parent| {
                parent.spawn((
                    Mesh2d(meshes.add(Circle::new(1.0))),
                    MeshMaterial2d(
                        materials.add(ColorMaterial::from_color(unit_glow_color(base.faction))),
                    ),
                    Transform::from_xyz(0.0, 0.0, -0.01).with_scale(Vec3::splat(r * 1.48)),
                ));
                // Boost aura behind everything (z=0.0); starts invisible, sized/tinted by update_unit_circle_hp_visuals.
                let aura_base = faction_base_color(base.faction).to_srgba();
                parent.spawn((
                    Mesh2d(meshes.add(Circle::new(1.0))),
                    MeshMaterial2d(materials.add(ColorMaterial::from_color(Color::srgba(
                        aura_base.red,
                        aura_base.green,
                        aura_base.blue,
                        0.0,
                    )))),
                    Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3::ZERO),
                    UnitBoostAura,
                ));
                parent.spawn((
                    Mesh2d(meshes.add(RegularPolygon::new(1.0, sides))),
                    MeshMaterial2d(materials.add(tinted)),
                    Transform::from_xyz(0.0, 0.0, 0.01)
                        .with_rotation(rotation)
                        .with_scale(Vec3::splat(fill_scale)),
                    UnitVisualFill,
                ));
                parent.spawn((
                    Mesh2d(meshes.add(RegularPolygon::new(1.0, sides))),
                    MeshMaterial2d(materials.add(ring_color)),
                    Transform::from_xyz(0.0, 0.0, 0.02)
                        .with_rotation(rotation)
                        .with_scale(Vec3::splat(r * 1.16)),
                    UnitVisualRing,
                ));
            });

        *units_by_base.entry(base_entity).or_default() += 1;
        *faction_total += 1;
    }
}

/// Clears [`CaptainEligibilityCooldown`] so evolved units can be captains again next frame.
/// Runs at the **start** of the frame in [`super::genetics::GeneticsPlugin`] before evolution.
pub fn captain_eligibility_clear(
    mut commands: Commands,
    q: Query<Entity, With<CaptainEligibilityCooldown>>,
) {
    for e in &q {
        commands.entity(e).remove::<CaptainEligibilityCooldown>();
    }
}

/// Greedy NMS: pick up to `need` new captains among `candidates`, staying `suppression_radius`
/// away from `blocked` positions and from each other (same rule as [`swarm_tactics::elect_captains_nms`]).
fn elect_fill_captains(
    candidates: &[SquadUnitSnapshot],
    candidate_entities: &[Entity],
    blocked: &[Vec2],
    suppression_radius: f32,
    need: usize,
) -> Vec<Entity> {
    if need == 0 || candidates.is_empty() {
        return Vec::new();
    }
    let mut order: Vec<usize> = (0..candidates.len()).collect();
    order.sort_by(|a, b| {
        candidates[*b]
            .combat_power
            .partial_cmp(&candidates[*a].combat_power)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let r = suppression_radius;
    let mut picked_idx: Vec<usize> = Vec::new();
    let mut blocked_pos: Vec<Vec2> = blocked.to_vec();
    for &idx in &order {
        if picked_idx.len() >= need {
            break;
        }
        let p = candidates[idx].position;
        let far_from_blocked = blocked_pos.iter().all(|bp| p.distance(*bp) > r);
        let far_from_picked = picked_idx
            .iter()
            .all(|&j| p.distance(candidates[j].position) > r);
        if far_from_blocked && far_from_picked {
            picked_idx.push(idx);
            blocked_pos.push(p);
        }
    }
    picked_idx
        .into_iter()
        .map(|i| candidate_entities[i])
        .collect()
}

/// Fills vacant captain slots only: existing [`SquadCaptain`] stay until death, evolution, or fusion.
fn fill_squad_captain_vacancies(
    mut commands: Commands,
    query: Query<(Entity, &Transform, &Unit, &GeneticTraits), With<GameplayEntity>>,
    captains: Query<&SquadCaptain>,
    cooldown: Query<&CaptainEligibilityCooldown>,
) {
    for faction in [Faction::Player, Faction::Enemy] {
        let mut rows: Vec<(Entity, Vec2, f32, f32, f32)> = Vec::new();
        for (e, tf, u, traits) in query.iter() {
            if u.is_dead() || u.faction != faction {
                continue;
            }
            let p = tf.translation.truncate();
            rows.push((
                e,
                p,
                traits.attack,
                traits.attack_speed,
                traits.sensor_range,
            ));
        }

        if rows.len() < 2 {
            for (e, _, u, _) in query.iter() {
                if u.faction == faction {
                    commands.entity(e).remove::<SquadCaptain>();
                }
            }
            continue;
        }

        rows.sort_by_key(|(entity, ..)| *entity);
        let mean_sensor: f32 =
            rows.iter().map(|(_, _, _, _, s)| *s).sum::<f32>() / rows.len() as f32;
        let snapshots: Vec<SquadUnitSnapshot> = rows
            .iter()
            .enumerate()
            .map(|(i, (_, pos, atk, asp, _))| SquadUnitSnapshot::new(i, *pos, *atk, *asp))
            .collect();
        let (k, r, _, _) = elect_and_assign(&snapshots, mean_sensor, SQUAD_CAPTAIN_LAMBDA);
        // Formula 1: desired captain count is min(K, N), not merely “whatever NMS returned”.
        let target_n = (k as usize).min(rows.len());

        let mut blocked: Vec<Vec2> = Vec::new();
        let mut cand_entities: Vec<Entity> = Vec::new();
        let mut cand_snaps: Vec<SquadUnitSnapshot> = Vec::new();

        for (e, pos, atk, asp, _) in &rows {
            if captains.get(*e).is_ok() {
                blocked.push(*pos);
            } else if cooldown.get(*e).is_ok() {
                // Evolved this frame: not a candidate until cooldown clears next frame.
            } else {
                let i = cand_snaps.len();
                cand_entities.push(*e);
                cand_snaps.push(SquadUnitSnapshot::new(i, *pos, *atk, *asp));
            }
        }

        let current_n = blocked.len();
        let need = target_n.saturating_sub(current_n);
        let picked = elect_fill_captains(&cand_snaps, &cand_entities, &blocked, r, need);
        for e in picked {
            commands.entity(e).insert(SquadCaptain);
        }
    }
}

fn sync_captain_badge_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    cfg: Res<GameConfig>,
    units: Query<(Entity, &Unit, &GeneticTraits, Option<&Children>), With<GameplayEntity>>,
    squad_captain: Query<&SquadCaptain>,
    badge_q: Query<(), With<CaptainBadgeGraphic>>,
) {
    for (entity, unit, traits, children_opt) in units.iter() {
        if unit.is_dead() || unit.faction == Faction::Neutral {
            continue;
        }
        let is_captain = squad_captain.get(entity).is_ok();
        let has_badge = children_opt
            .map(|ch| ch.iter().any(|c| badge_q.get(c).is_ok()))
            .unwrap_or(false);

        if is_captain && !has_badge {
            let r = cfg.unit_body_radius(traits);
            let dot_scale = (r * 0.3).max(0.07);
            let dot_color = match unit.faction {
                Faction::Player => cover_core_color(Faction::Player),
                Faction::Enemy => cover_core_color(Faction::Enemy),
                Faction::Neutral => cover_core_color(Faction::Neutral),
            };
            commands.entity(entity).with_children(|parent| {
                parent.spawn((
                    Mesh2d(meshes.add(Circle::new(1.0))),
                    MeshMaterial2d(materials.add(ColorMaterial::from_color(dot_color))),
                    Transform::from_xyz(0.0, 0.0, 0.04).with_scale(Vec3::splat(dot_scale)),
                    CaptainBadgeGraphic,
                ));
            });
        } else if !is_captain
            && has_badge
            && let Some(children) = children_opt
        {
            for c in children.iter() {
                if badge_q.get(c).is_ok()
                    && let Ok(mut ec) = commands.get_entity(c)
                {
                    ec.despawn();
                }
            }
        }
    }
}

/// Per-frame rally points so each [`SquadCaptain`] lane pulls toward a distinct anchor ahead of the
/// swarm (reduces “one blob” theatre). Strength is modulated in [`move_units`] during combat / defense.
#[derive(Resource, Default)]
pub(crate) struct SquadSpreadCache {
    pub(crate) anchor_for_unit: HashMap<Entity, Vec2>,
    pub(crate) follow_for_unit: HashMap<Entity, SquadFollowHint>,
}

#[derive(Clone, Copy)]
pub(crate) struct SquadFollowHint {
    pub(crate) target: Vec2,
    pub(crate) catchup_radius: f32,
}

#[inline]
fn squad_follow_slot_offset(
    index: usize,
    forward: Vec2,
    right: Vec2,
    lateral: f32,
    depth: f32,
) -> Vec2 {
    const COLUMN_PATTERN: [f32; 3] = [-1.0, 1.0, 0.0];
    let row = index / COLUMN_PATTERN.len() + 1;
    let column = COLUMN_PATTERN[index % COLUMN_PATTERN.len()];
    let lateral_scale = 1.0 + (row.saturating_sub(1)) as f32 * 0.28;
    right * (column * lateral * lateral_scale) - forward * (depth * row as f32)
}

fn build_squad_spread_hints(
    mut cache: ResMut<SquadSpreadCache>,
    cfg: Res<GameConfig>,
    units: Query<(Entity, &Transform, &Unit, &GeneticTraits), With<GameplayEntity>>,
    captains_q: Query<Entity, With<SquadCaptain>>,
) {
    cache.anchor_for_unit.clear();
    cache.follow_for_unit.clear();
    let st = &cfg.steering;

    for faction in [Faction::Player, Faction::Enemy] {
        let mut members: Vec<(Entity, Vec2, f32, f32)> = Vec::new();
        let mut cap_list: Vec<(Entity, Vec2, f32)> = Vec::new();

        for (e, tf, u, traits) in units.iter() {
            if u.is_dead() || u.faction != faction {
                continue;
            }
            let p = tf.translation.truncate();
            let pow = traits.attack * traits.attack_speed;
            let sr = traits.sensor_range;
            members.push((e, p, pow, sr));
            if captains_q.get(e).is_ok() {
                cap_list.push((e, p, pow));
            }
        }

        if members.len() < 2 || cap_list.is_empty() {
            continue;
        }

        members.sort_by_key(|(e, _, _, _)| *e);
        cap_list.sort_by_key(|(e, _, _)| *e);

        let ally_c: Vec2 =
            members.iter().map(|(_, p, _, _)| *p).sum::<Vec2>() / members.len() as f32;

        let mut enemy_c = Vec2::ZERO;
        let mut n_enemy = 0u32;
        for (_, tf, u, _) in units.iter() {
            if u.is_dead() || !faction.is_enemy_of(&u.faction) {
                continue;
            }
            enemy_c += tf.translation.truncate();
            n_enemy += 1;
        }
        if n_enemy > 0 {
            enemy_c /= n_enemy as f32;
        } else {
            enemy_c = ally_c + Vec2::new(cfg.world_half_w * 0.35, 0.0);
        }

        let mean_sensor: f32 =
            members.iter().map(|(_, _, _, s)| *s).sum::<f32>() / members.len() as f32;

        let forward = (enemy_c - ally_c).normalize_or_zero();
        let forward = if forward.length_squared() < 1e-6 {
            Vec2::X
        } else {
            forward
        };
        let right = forward.perp();

        let k = cap_list.len() as f32;
        let mut anchors: HashMap<Entity, Vec2> = HashMap::new();
        for (i, (ce, _, _)) in cap_list.iter().enumerate() {
            let lateral =
                (i as f32 - (k - 1.0) * 0.5) * mean_sensor * st.squad_spread_lateral_scale;
            let fwd_off = mean_sensor * st.squad_spread_forward_scale;
            let anchor = ally_c + forward * fwd_off + right * lateral;
            anchors.insert(*ce, anchor);
        }

        let mut member_index_by_entity: HashMap<Entity, usize> = HashMap::new();
        let snapshots: Vec<SquadUnitSnapshot> = members
            .iter()
            .enumerate()
            .map(|(i, (e, pos, pow, _))| {
                member_index_by_entity.insert(*e, i);
                SquadUnitSnapshot {
                    id: i,
                    position: *pos,
                    combat_power: *pow,
                }
            })
            .collect();
        let captain_indices: Vec<usize> = cap_list
            .iter()
            .filter_map(|(e, _, _)| member_index_by_entity.get(e).copied())
            .collect();
        if captain_indices.is_empty() {
            continue;
        }
        let assignments =
            assign_units_to_squads(&snapshots, &captain_indices, SQUAD_SPREAD_ASSIGN_LAMBDA);
        let mut squads: Vec<Vec<(Entity, Vec2)>> = vec![Vec::new(); captain_indices.len()];
        for ((entity, pos, _, _), squad_idx) in members.iter().zip(assignments) {
            squads[squad_idx].push((*entity, *pos));
        }

        let lateral_spacing = mean_sensor * st.squad_follow_lateral_scale;
        let depth_spacing = mean_sensor * st.squad_follow_depth_scale;
        let catchup_radius =
            (mean_sensor * st.squad_follow_catchup_scale).max(lateral_spacing * 1.25);

        for (slot, squad_members) in squads.into_iter().enumerate() {
            let (cap_e, cap_pos, _) = cap_list[slot];
            let anchor = anchors.get(&cap_e).copied().unwrap_or(cap_pos);
            for (entity, _) in &squad_members {
                cache.anchor_for_unit.insert(*entity, anchor);
            }
            let squad_forward = (anchor - cap_pos).normalize_or_zero();
            let squad_forward = if squad_forward.length_squared() < 1e-6 {
                forward
            } else {
                squad_forward
            };
            let squad_right = squad_forward.perp();

            let mut followers: Vec<(Entity, Vec2)> = squad_members
                .into_iter()
                .filter(|(entity, _)| *entity != cap_e)
                .collect();
            followers.sort_by_key(|(entity, _)| *entity);

            for (member_index, (entity, _member_pos)) in followers.into_iter().enumerate() {
                let slot_offset = squad_follow_slot_offset(
                    member_index,
                    squad_forward,
                    squad_right,
                    lateral_spacing,
                    depth_spacing,
                );
                cache.follow_for_unit.insert(
                    entity,
                    SquadFollowHint {
                        target: cap_pos + slot_offset,
                        catchup_radius,
                    },
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn move_units(
    time: Res<Time>,
    cfg: Res<GameConfig>,
    flow_fields: Res<FlowFieldSet>,
    hash: Res<SpatialHash>,
    defense_signals: Res<BaseDefenseSignals>,
    captain_hint: Res<CaptainAiHint>,
    beacon_state: Res<BeaconControlState>,
    spread_cache: Res<SquadSpreadCache>,
    all_bases: Query<(&Transform, &Base, Option<&Headquarters>), Without<Unit>>,
    mut query: Query<(
        Entity,
        &mut Transform,
        &Unit,
        &GeneticTraits,
        &MovementSuppression,
        &mut SteeringInertia,
        Option<&SquadCaptain>,
        Option<&BaseBoost>,
    )>,
    beacon_query: Query<(&Transform, &Beacon), Without<Unit>>,
) {
    let dt = time.delta_secs();

    // Snapshot: position, faction, suppression, combat power, health fraction, body radius (separation).
    let positions: Vec<(Entity, Vec2, Faction, f32, f32, f32, f32)> = query
        .iter()
        .map(|(e, tf, u, traits, sup, _, _, _)| {
            let health_frac = (u.current_health / traits.max_health.max(1.0)).clamp(0.0, 1.0);
            let body_r = cfg.unit_body_radius(traits);
            (
                e,
                tf.translation.truncate(),
                u.faction,
                sup.level,
                local_combat_power(u, traits, sup),
                health_frac,
                body_r,
            )
        })
        .collect();

    let index_map: std::collections::HashMap<Entity, usize> = positions
        .iter()
        .enumerate()
        .map(|(i, (e, ..))| (*e, i))
        .collect();

    let beacon_context = beacon_query.iter().next().map(|(btf, beacon)| {
        let bp = btf.translation.truncate();
        let r_sq = beacon.radius * beacon.radius;
        let player_units_in_radius = positions
            .iter()
            .filter(|(_, p, fac, _, _, _, _)| {
                *fac == Faction::Player && (*p - bp).length_squared() <= r_sq
            })
            .count() as u32;
        (
            bp,
            *beacon,
            player_units_in_radius,
            beacon_state.mode,
            beacon_state.strength_multiplier(&cfg),
        )
    });

    for (entity, mut tf, unit, traits, my_sup, mut steering, squad_captain, base_boost) in
        query.iter_mut()
    {
        let pos = tf.translation.truncate();
        let st = &cfg.steering;

        // 1. Flow-field steering (+ optional ONNX captain hint for player)
        let mut flow_dir = match unit.faction {
            Faction::Player => flow_fields.attack_enemy.sample(pos),
            Faction::Enemy => flow_fields.attack_player.sample(pos),
            Faction::Neutral => Vec2::ZERO, // neutral units don't move
        };
        if unit.faction == Faction::Player {
            let h = captain_hint.0;
            if h.length_squared() > 1e-10 {
                flow_dir = (flow_dir * (1.0 - CAPTAIN_FLOW_BLEND) + h * CAPTAIN_FLOW_BLEND)
                    .normalize_or_zero();
            }
        }

        // 2. Separation from same-faction neighbours
        let my_r = cfg.unit_body_radius(traits);
        let mut separation = Vec2::ZERO;
        for (other_e, other_pos, other_faction, _, _, _, other_r) in &positions {
            if *other_e == entity {
                continue;
            }
            if *other_faction != unit.faction {
                continue;
            }
            let diff = pos - *other_pos;
            let dist = diff.length();
            let sep_threshold = cfg.unit_pair_separation_distance_from_radii(my_r, *other_r);
            if dist > 0.0 && dist < sep_threshold {
                separation += diff / dist * (1.0 - dist / sep_threshold);
            }
        }

        // 3. Chase hostile within sensor_range — sticky target + hysteresis to reduce flip-flop
        let mut chase_dir = Vec2::ZERO;
        let mut nearest_enemy_sup = 0.0_f32;
        let mut retreat_dir = Vec2::ZERO;
        let mut allied_power = local_combat_power(unit, traits, my_sup);
        let mut allied_count = 1_u32;
        let mut allied_center = pos;
        let mut enemy_power = 0.0_f32;
        let mut enemy_count = 0_u32;
        let mut enemy_center_acc = Vec2::ZERO;

        let locked = steering.chase_target;
        let mut nearest_dsq = f32::MAX;
        let mut nearest_score = f32::MAX;
        let mut nearest_e: Option<Entity> = None;
        let mut nearest_chase_dir = Vec2::ZERO;
        let mut nearest_sup = 0.0_f32;
        let mut locked_ok = false;
        let mut locked_dsq = f32::MAX;
        let mut locked_score = f32::MAX;
        let mut locked_dir = Vec2::ZERO;
        let mut locked_sup = 0.0_f32;
        let mut rescue_dir = Vec2::ZERO;
        let mut rescue_strength = 0.0_f32;
        let mut best_rescue_score = 0.0_f32;

        let (defend_base_dir, defend_base_strength, has_defend_base) = match unit.faction {
            Faction::Player => {
                defense_signals
                    .player_defend_pos
                    .map_or((Vec2::ZERO, 0.0, false), |p| {
                        let to = p - pos;
                        let d = to.length();
                        if d > 1e-3 {
                            let u = defense_signals.player_urgency;
                            (to / d, u, u > 0.02)
                        } else {
                            (Vec2::ZERO, 0.0, false)
                        }
                    })
            }
            Faction::Enemy => {
                defense_signals
                    .enemy_defend_pos
                    .map_or((Vec2::ZERO, 0.0, false), |p| {
                        let to = p - pos;
                        let d = to.length();
                        if d > 1e-3 {
                            let u = defense_signals.enemy_urgency;
                            (to / d, u, u > 0.02)
                        } else {
                            (Vec2::ZERO, 0.0, false)
                        }
                    })
            }
            Faction::Neutral => (Vec2::ZERO, 0.0, false),
        };

        if unit.faction != Faction::Neutral {
            let sensor = traits.sensor_range;
            let sensor_sq = sensor * sensor;
            hash.for_each_in_radius(pos, sensor, |other_e| {
                if other_e == entity {
                    return;
                }
                let idx = match index_map.get(&other_e) {
                    Some(&i) => i,
                    None => return,
                };
                let row = &positions[idx];
                let other_pos = row.1;
                let other_faction = row.2;
                let other_sup = row.3;
                let other_power = row.4;

                if other_faction == unit.faction {
                    allied_power += other_power;
                    allied_count += 1;
                    allied_center += other_pos;
                    if other_e != entity && other_sup > st.rescue_ally_suppression_threshold {
                        let to = other_pos - pos;
                        let dsq = to.length_squared();
                        if dsq < sensor_sq && dsq > 1e-6 {
                            let hf = row.5;
                            let urgency =
                                (other_sup * st.rescue_urgency_scale) * (1.0 + (1.0 - hf));
                            let score = urgency / (1.0 + dsq / sensor_sq.max(1.0));
                            if score > best_rescue_score {
                                best_rescue_score = score;
                                rescue_dir = to.normalize_or_zero();
                                rescue_strength = (other_sup * (1.2 - 0.4 * hf)).clamp(0.0, 1.0);
                            }
                        }
                    }
                    return;
                }

                if !unit.faction.is_enemy_of(&other_faction) {
                    return;
                }

                enemy_power += other_power;
                enemy_count += 1;
                enemy_center_acc += other_pos;

                let to = other_pos - pos;
                let dsq = to.length_squared();
                if dsq >= sensor_sq || dsq <= 1e-6 {
                    return;
                }
                let mut score = dsq;
                if unit.faction == Faction::Player
                    && let Some((bp, beacon, _, mode, strength_multiplier)) = beacon_context
                    && mode == BeaconMode::Focus
                {
                    let is_local_focus_unit = beacon.contains(bp, pos)
                        || (pos - bp).length_squared() <= (beacon.radius * beacon.radius * 1.44);
                    if is_local_focus_unit && beacon.contains(bp, other_pos) {
                        score /= 1.0 + cfg.beacon_focus_target_bonus * strength_multiplier;
                    }
                }
                let dir = to.normalize_or_zero();
                if Some(other_e) == locked {
                    locked_ok = true;
                    locked_score = score;
                    locked_dsq = dsq;
                    locked_dir = dir;
                    locked_sup = other_sup;
                }
                if score < nearest_score {
                    nearest_score = score;
                    nearest_dsq = dsq;
                    nearest_e = Some(other_e);
                    nearest_chase_dir = dir;
                    nearest_sup = other_sup;
                }
            });

            if enemy_count > 0 {
                let allied_center = allied_center / allied_count as f32;
                let enemy_center = enemy_center_acc / enemy_count as f32;
                let regroup_dir = (allied_center - pos).normalize_or_zero();
                let withdraw_dir = (pos - enemy_center).normalize_or_zero();
                retreat_dir =
                    (withdraw_dir + regroup_dir * st.retreat_regroup_mix).normalize_or_zero();
            }

            // Resolve chase direction and update sticky target
            if locked_ok {
                let steal = nearest_e.is_some_and(|ne| {
                    Some(ne) != locked && nearest_score < locked_score * st.chase_steal_dsq_ratio
                });
                if steal {
                    chase_dir = nearest_chase_dir;
                    nearest_enemy_sup = nearest_sup;
                    steering.chase_target = nearest_e;
                } else {
                    chase_dir = locked_dir;
                    nearest_enemy_sup = locked_sup;
                    steering.chase_target = locked;
                }
            } else {
                chase_dir = nearest_chase_dir;
                nearest_enemy_sup = nearest_sup;
                steering.chase_target = nearest_e;
            }
        }

        // 4. Beacon — player units only; crowding counts allies in radius (`Beacon::attraction_force`)
        let mut beacon_force = Vec2::ZERO;
        if unit.faction == Faction::Player
            && let Some((bp, beacon, ally_count, mode, strength_multiplier)) = beacon_context
        {
            let strength_multiplier = if mode == BeaconMode::Repel {
                strength_multiplier * cfg.beacon_repel_strength_scale
            } else {
                strength_multiplier
            };
            beacon_force = beacon.influence_force(mode, bp, pos, ally_count, strength_multiplier);
        }

        // 5. Blend flow + separation + optional chase/retreat + optional beacon
        let has_chase = chase_dir.length_squared() > 1e-6;
        let enemy_advantage = enemy_power > allied_power * st.retreat_power_ratio;
        let count_disadvantage =
            enemy_count > allied_count && enemy_power > allied_power * st.retreat_count_power_ratio;
        let should_retreat = enemy_count > 0
            && retreat_dir.length_squared() > 1e-6
            && (enemy_advantage || count_disadvantage);
        if should_retreat {
            steering.chase_target = None;
        }
        let has_beacon = beacon_force.length_squared() > 1e-10;

        let health_frac_move = (unit.current_health / traits.max_health.max(1.0)).clamp(0.0, 1.0);
        let (home_rally_dir, home_rally_strength, has_home_rally) = home_rally_steering(
            pos,
            unit.faction,
            health_frac_move,
            has_chase,
            nearest_enemy_sup,
            st,
            &all_bases,
        );

        let squad_spread_dir = spread_cache
            .anchor_for_unit
            .get(&entity)
            .map(|anchor| (*anchor - pos).normalize_or_zero())
            .unwrap_or(Vec2::ZERO);

        let r_assault_sq = st.squad_spread_assault_radius * st.squad_spread_assault_radius;
        let assaulting_hq = all_bases.iter().any(|(tf, b, hq)| {
            hq.is_some()
                && unit.faction.is_enemy_of(&b.faction)
                && (tf.translation.truncate() - pos).length_squared() < r_assault_sq
        });

        let mut squad_spread_strength = if squad_spread_dir.length_squared() > 1e-10 {
            1.0_f32
        } else {
            0.0_f32
        };
        if has_chase {
            squad_spread_strength *= 1.0 - (0.22 + 0.68 * nearest_enemy_sup).clamp(0.0, 1.0);
        }
        if should_retreat {
            squad_spread_strength *= 0.12;
        }
        if has_defend_base {
            squad_spread_strength *= 1.0 - 0.9 * defend_base_strength.clamp(0.0, 1.0);
        }
        if has_home_rally {
            squad_spread_strength *= 1.0 - 0.48 * home_rally_strength.clamp(0.0, 1.0);
        }
        if rescue_strength > 0.06 {
            squad_spread_strength *= 1.0 - 0.5 * rescue_strength.clamp(0.0, 1.0);
        }
        if assaulting_hq {
            squad_spread_strength *= 0.18;
        }
        if unit.faction == Faction::Neutral {
            squad_spread_strength = 0.0;
        }

        let (squad_follow_dir, squad_follow_distance, squad_follow_catchup_radius) = spread_cache
            .follow_for_unit
            .get(&entity)
            .map(|hint| {
                let to = hint.target - pos;
                (to.normalize_or_zero(), to.length(), hint.catchup_radius)
            })
            .unwrap_or((Vec2::ZERO, 0.0, 1.0));
        let mut squad_follow_strength =
            if squad_captain.is_none() && squad_follow_dir.length_squared() > 1e-10 {
                (squad_follow_distance / squad_follow_catchup_radius.max(1e-3)).clamp(0.0, 1.0)
            } else {
                0.0
            };
        if has_chase {
            squad_follow_strength *= 0.88 - 0.26 * nearest_enemy_sup.clamp(0.0, 1.0);
        }
        if should_retreat {
            squad_follow_strength *= 0.2;
        }
        if has_home_rally {
            squad_follow_strength *= 1.0 - 0.72 * home_rally_strength.clamp(0.0, 1.0);
        }
        if rescue_strength > 0.06 {
            squad_follow_strength *= 1.0 - 0.6 * rescue_strength.clamp(0.0, 1.0);
        }
        if has_defend_base {
            squad_follow_strength *= 0.8 + 0.2 * defend_base_strength.clamp(0.0, 1.0);
        }
        if assaulting_hq {
            squad_follow_strength = (squad_follow_strength * 1.12).min(1.0);
        }
        if unit.faction == Faction::Neutral {
            squad_follow_strength = 0.0;
        }

        let final_dir = blended_steering_direction(
            st,
            &SteeringBlendInput {
                flow_dir,
                separation,
                chase_dir,
                retreat_dir,
                rescue_dir,
                rescue_strength,
                beacon_force,
                defend_base_dir,
                defend_base_strength,
                should_retreat,
                has_chase,
                has_beacon,
                has_defend_base,
                home_rally_dir,
                home_rally_strength,
                has_home_rally,
                nearest_enemy_sup,
                squad_spread_dir,
                squad_spread_strength,
                squad_follow_dir,
                squad_follow_strength,
            },
        );

        let k = (st.inertia_blend_per_sec * dt).min(1.0);
        if final_dir.length_squared() > 1e-10 {
            steering.smoothed_dir = if steering.smoothed_dir.length_squared() < 1e-10 {
                final_dir
            } else {
                (steering.smoothed_dir * (1.0 - k) + final_dir * k).normalize_or_zero()
            };
        } else {
            let decay = (1.0 - dt * st.inertia_zero_decay_per_sec).max(0.0);
            steering.smoothed_dir *= decay;
        }
        let move_dir = if steering.smoothed_dir.length_squared() > 1e-10 {
            steering.smoothed_dir.normalize()
        } else {
            Vec2::ZERO
        };

        let slow = 1.0 - MAX_SLOW_FROM_SUPPRESSION * my_sup.level;
        let slow = slow.max(MIN_SPEED_FRAC);
        let pursuit_speed = if has_chase {
            1.0 + st.pursuit_speed_bonus * nearest_enemy_sup
        } else {
            1.0
        };
        let retreat_speed = if should_retreat {
            1.0 + st.retreat_speed_bonus
        } else {
            1.0
        };
        let boost_mult = base_boost.map(|b| b.multiplier).unwrap_or(1.0);
        let mut move_speed = traits.speed * slow * pursuit_speed * retreat_speed * boost_mult;

        // In a flow-field goal cell, `flow_dir` is zero — treat as arrived and throttle speed
        // unless combat or rescue still needs full movement.
        let at_flow_goal = matches!(unit.faction, Faction::Player | Faction::Enemy)
            && flow_dir.length_squared() < 1e-10;
        let urgent_home = rescue_strength >= st.arrival_rescue_override_min
            || defend_base_strength >= st.arrival_rescue_override_min;
        if at_flow_goal && !has_chase && !should_retreat && !urgent_home {
            move_speed *= st.goal_arrival_speed_frac;
        }

        let new_pos = pos + move_dir * move_speed * dt;
        let clamped = new_pos.clamp(
            Vec2::new(-cfg.world_half_w + 6.0, -cfg.world_half_h + 6.0),
            Vec2::new(cfg.world_half_w - 6.0, cfg.world_half_h - 6.0),
        );
        tf.translation = clamped.extend(tf.translation.z);
    }
}

fn local_combat_power(
    unit: &Unit,
    traits: &GeneticTraits,
    suppression: &MovementSuppression,
) -> f32 {
    let health_frac = (unit.current_health / traits.max_health.max(1.0)).clamp(0.0, 1.0);
    let durability = 0.45 + health_frac * 0.55;
    let suppression_penalty = 1.0 - suppression.level * 0.4;
    unit_strength(traits) * durability * suppression_penalty.max(0.35)
}

fn decay_combat_flash(
    time: Res<Time>,
    mut query: Query<(&mut AttackFlash, &mut HitFlash, &mut ProjectileCooldown)>,
) {
    let dt = time.delta_secs();
    for (mut atk, mut hit, mut pc) in &mut query {
        atk.0 = (atk.0 - dt / ATTACK_FLASH_DURATION).max(0.0);
        hit.0 = (hit.0 - dt / HIT_FLASH_DURATION).max(0.0);
        pc.0 = (pc.0 - dt).max(0.0);
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::collapsible_if
)]
fn combat_system(
    time: Res<Time>,
    cfg: Res<GameConfig>,
    hash: Res<SpatialHash>,
    mut combat_ledger: ResMut<CombatContributionLedger>,
    mut assault_ledger: ResMut<BaseAssaultLedger>,
    base_query: Query<(Entity, &Transform, &Base)>,
    mut projectile_events: MessageWriter<super::projectiles::ProjectileEvent>,
    mut unit_query: Query<(
        Entity,
        &Transform,
        &mut Unit,
        &GeneticTraits,
        &mut MovementSuppression,
        &mut UnitFitness,
        Option<&mut AttackFlash>,
        Option<&mut HitFlash>,
        Option<&BaseBoost>,
        Option<&mut ProjectileCooldown>,
    )>,
) {
    let dt = time.delta_secs();

    // Snapshot only the data we need for read-only neighbour lookups.
    // Boost multiplier scales attack, armor, and hit range for units inside their own base.
    let snapshot: Vec<(Entity, Vec2, Faction, f32, f32, f32)> = unit_query
        .iter()
        .map(|(e, tf, u, t, _, _, _, _, boost, _)| {
            let boost_m = boost.map(|b| b.multiplier).unwrap_or(1.0);
            let hr = hit_range_from_traits(t) * boost_m.sqrt();
            (
                e,
                tf.translation.truncate(),
                u.faction,
                t.attack * boost_m,
                t.armor * boost_m,
                hr,
            )
        })
        .collect();

    // Build a fast Entity → index map so neighbour lookups are O(1).
    let index_map: std::collections::HashMap<Entity, usize> = snapshot
        .iter()
        .enumerate()
        .map(|(i, (e, ..))| (*e, i))
        .collect();

    let mut total_damage_by_target: std::collections::HashMap<Entity, f32> =
        std::collections::HashMap::new();
    let mut damage_dealt_by_attacker: std::collections::HashMap<Entity, f32> =
        std::collections::HashMap::new();
    // Track one representative target position per attacker for projectile visuals.
    let mut attacker_target_pos: std::collections::HashMap<Entity, Vec2> =
        std::collections::HashMap::new();

    for (entity, pos, faction, _attack, _armor, _) in &snapshot {
        let mut total_damage = 0.0_f32;
        hash.for_each_in_radius(*pos, COMBAT_LOOKUP_RADIUS, |other_e| {
            if *entity == other_e {
                return;
            }
            let idx = match index_map.get(&other_e) {
                Some(i) => *i,
                None => return,
            };
            let (_, other_pos, other_faction, other_attack, _, other_hit_range) = &snapshot[idx];
            if !faction.is_enemy_of(other_faction) {
                return;
            }
            let dist = (*pos - *other_pos).length();
            if dist < *other_hit_range {
                // Units assaulting a hostile base with no enemy in weapon range only deal base
                // capture damage, not unit-vs-unit damage (see `base_capture_system`).
                if base_assault_suppresses_unit_combat_outgoing_snapshot(
                    other_e,
                    *other_pos,
                    *other_faction,
                    *other_hit_range,
                    &hash,
                    &snapshot,
                    &index_map,
                    &base_query,
                ) {
                    return;
                }
                let falloff = 1.0 - (dist / *other_hit_range);
                let dealt = other_attack * falloff * dt * COMBAT_DAMAGE_SCALE;
                total_damage += dealt;
                *damage_dealt_by_attacker.entry(other_e).or_default() += dealt;
                // Record attacker → target position (first target wins per frame).
                attacker_target_pos.entry(other_e).or_insert(*pos);
                combat_ledger.record_damage(other_e, *entity, dealt);
                if let Some(base_entity) =
                    own_base_entity_at_position(*other_pos, *other_faction, &base_query)
                {
                    assault_ledger.record_defense_contribution(base_entity, other_e, dealt);
                }
            }
        });
        total_damage_by_target.insert(*entity, total_damage);
    }

    let home_dmg_scale = cfg
        .unit_friendly_base_damage_taken_multiplier
        .clamp(0.0, 2.0);

    for (entity, tf, mut unit, _, mut sup, mut fitness, mut atk_flash, mut hit_flash, _, mut pc) in
        &mut unit_query
    {
        if let Some(damage_dealt) = damage_dealt_by_attacker.get(&entity) {
            fitness.damage_dealt += *damage_dealt;
            if let Some(ref mut flash) = atk_flash {
                flash.0 = 1.0;
            }
            // Fire a projectile visual if cooldown has expired.
            let can_fire = pc.as_ref().map(|c| c.0 <= 0.0).unwrap_or(false);
            if can_fire {
                if let Some(target_pos) = attacker_target_pos.get(&entity) {
                    projectile_events.write(super::projectiles::ProjectileEvent {
                        from: tf.translation.truncate(),
                        to: *target_pos,
                        faction: unit.faction,
                    });
                    if let Some(ref mut c) = pc {
                        c.0 = 0.38;
                    }
                }
            }
        }
        let mut total_damage = total_damage_by_target
            .get(&entity)
            .copied()
            .unwrap_or_default();
        if unit_inside_own_base(tf.translation.truncate(), unit.faction, &base_query) {
            total_damage *= home_dmg_scale;
        }
        let armor = snapshot[index_map[&entity]].4;
        let mitigated = (total_damage - armor * dt).max(0.0);
        if mitigated > 0.0 {
            unit.current_health -= mitigated;
            sup.level = (sup.level + mitigated * SUPPRESSION_PER_DAMAGE).min(1.0);
            fitness.damage_taken_survived += mitigated;
            unit.seconds_since_last_damage = 0.0;
            if let Some(ref mut flash) = hit_flash {
                flash.0 = 1.0;
            }
        } else {
            unit.seconds_since_last_damage += dt;
        }
    }
}

#[allow(clippy::type_complexity)]
fn reap_dead_units(
    mut commands: Commands,
    mut gene_pool: ResMut<CyborgGenePool>,
    mut combat_ledger: ResMut<CombatContributionLedger>,
    mut wolf_pack: ResMut<WolfPack>,
    cfg: Res<GameConfig>,
    mut death_events: MessageWriter<super::particles::DeathEvent>,
    mut unit_queries: ParamSet<(
        Query<(Entity, &Transform, &Unit, &GeneticTraits, &UnitFitness)>,
        Query<&mut Unit>,
    )>,
) {
    let dead_units: Vec<_> = unit_queries
        .p0()
        .iter()
        .filter(|(_, _, unit, _, _)| unit.is_dead())
        .map(|(entity, tf, unit, traits, fitness)| {
            (
                entity,
                tf.translation.truncate(),
                unit.clone(),
                traits.clone(),
                fitness.clone(),
            )
        })
        .collect();

    for (entity, position, unit, traits, fitness) in dead_units {
        if unit.is_dead() {
            match unit.faction {
                Faction::Player => gene_pool.player.record_death(&traits, cfg.dead_rolling_n),
                Faction::Enemy => gene_pool.enemy.record_death(&traits, cfg.dead_rolling_n),
                Faction::Neutral => {} // neutral units have no gene pool
            }
            if let Some(record) = combat_ledger.remove_target(entity) {
                let pool =
                    super::experience::unit_composite_power(&unit, &traits) * cfg.kill_xp_factor;
                let shares = distribute_pool_with_main_share(
                    pool,
                    combat_ledger.time_s,
                    cfg.xp_contribution_window_sec,
                    &record.contributors,
                    record.last_hitter,
                    cfg.kill_xp_finisher_bonus_weight,
                );
                award_xp_shares(&shares, &mut unit_queries.p1());
            }
            wolf_pack.record_candidate(unit.faction, &traits, fitness.score());
            death_events.write(super::particles::DeathEvent { position });
            // Unit visuals are parented (fill + ring); remove the hierarchy before despawning.
            commands.entity(entity).despawn_children().despawn();
        }
    }
}

/// Child meshes: boost aura (z=0), fill circle (z=0.01), ring annulus (z=0.02).
/// Updates scale, flash colors, and boost aura visibility each frame.
#[allow(clippy::type_complexity, clippy::collapsible_if)]
fn update_unit_circle_hp_visuals(
    cfg: Res<GameConfig>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    units: Query<
        (
            &Unit,
            &GeneticTraits,
            Option<&AttackFlash>,
            Option<&HitFlash>,
            Option<&BaseBoost>,
        ),
        With<Unit>,
    >,
    mut visual_q: Query<
        (
            &ChildOf,
            &mut Transform,
            Option<&UnitBoostAura>,
            Option<&UnitVisualFill>,
            Option<&UnitVisualRing>,
            Option<&CaptainBadgeGraphic>,
            Option<&MeshMaterial2d<ColorMaterial>>,
        ),
        Or<(
            With<UnitBoostAura>,
            With<UnitVisualFill>,
            With<UnitVisualRing>,
            With<CaptainBadgeGraphic>,
        )>,
    >,
) {
    let inner = 1.0 - UNIT_RING_WIDTH_FRAC;
    for (parent, mut tf, aura_m, fill_m, ring_m, badge_m, mat_h) in visual_q.iter_mut() {
        let is_aura = aura_m.is_some();
        let is_fill = fill_m.is_some();
        let is_ring = ring_m.is_some();
        let is_badge = badge_m.is_some();
        if !is_aura && !is_fill && !is_ring && !is_badge {
            continue;
        }
        let Ok((unit, traits, _atk_flash, hit_flash, base_boost)) = units.get(parent.parent())
        else {
            continue;
        };
        let r = cfg.unit_body_radius(traits);
        let boost_m = base_boost.map(|b| b.multiplier).unwrap_or(1.0).max(1.0);
        let speed_t = ((traits.speed - 15.0) / 165.0).clamp(0.0, 1.0);
        let base_col = faction_base_color(unit.faction);

        if is_aura {
            // Semi-transparent disc showing the boosted radius beyond the unit body.
            let boosted_r = r * boost_m.sqrt();
            tf.scale = Vec3::splat(boosted_r);
            if let Some(h) = mat_h {
                if let Some(mat) = materials.get_mut(h) {
                    let alpha = ((boost_m - 1.0) * 2.5).clamp(0.0, 0.35);
                    let c = base_col.to_srgba();
                    mat.color = Color::srgba(c.red, c.green, c.blue, alpha);
                }
            }
        } else if is_fill {
            let hp = (unit.current_health / traits.max_health.max(1e-6)).clamp(0.0, 1.0);
            let fill_scale = r * inner * hp.sqrt();
            tf.scale = Vec3::splat(fill_scale);
            if let Some(h) = mat_h {
                if let Some(mat) = materials.get_mut(h) {
                    let normal = tint_color(base_col, speed_t);
                    // Blend up to 45% toward white — visible pop without washing out faction color.
                    let t = hit_flash.map(|f| f.0 * 0.45).unwrap_or(0.0);
                    mat.color = lerp_color(normal, HIT_FLASH_COLOR, t);
                }
            }
        } else if is_ring {
            tf.scale = Vec3::splat(r * 1.16);
            if let Some(h) = mat_h {
                if let Some(mat) = materials.get_mut(h) {
                    mat.color = ring_tint(tint_color(base_col, speed_t));
                }
            }
        } else if is_badge {
            let dot_scale = (r * 0.3).max(0.07);
            tf.scale = Vec3::splat(dot_scale);
        }
    }
}

/// Polygon class shapes (single mesh): alpha tracks HP so damaged units read lighter / more "outline-like".
#[allow(clippy::type_complexity)]
fn update_polygon_unit_hp_fill(
    mut materials: ResMut<Assets<ColorMaterial>>,
    q: Query<
        (
            &Unit,
            &GeneticTraits,
            &TacticalClass,
            &MeshMaterial2d<ColorMaterial>,
        ),
        (With<Unit>, Without<UnitVisualFill>, With<Mesh2d>),
    >,
) {
    for (unit, traits, class, mat_h) in q.iter() {
        if class.polygon_sides() == 0 {
            continue;
        }
        let hp = (unit.current_health / traits.max_health.max(1e-6)).clamp(0.0, 1.0);
        let a = 0.3 + 0.7 * hp;
        if let Some(m) = materials.get_mut(mat_h) {
            let s = m.color.to_srgba();
            m.color = Color::srgba(s.red, s.green, s.blue, a);
        }
    }
}

/// Replace unit mesh(es) after evolution: Drone → circle + ring children; other classes → one polygon mesh.
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_evolution_unit_visual(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    cfg: &GameConfig,
    entity: Entity,
    unit: &Unit,
    traits: &GeneticTraits,
    class: TacticalClass,
) {
    // One hierarchy pass avoids double-despawn (B0003) vs. per-child despawn + later systems.
    let mut ec = commands.entity(entity);
    ec.despawn_children();
    ec.remove::<Mesh2d>();
    ec.remove::<MeshMaterial2d<ColorMaterial>>();

    let color = match unit.faction {
        Faction::Player => COLOR_PLAYER,
        Faction::Enemy => COLOR_ENEMY,
        Faction::Neutral => Color::srgb(0.5, 0.5, 0.5),
    };
    let speed_t = ((traits.speed - 15.0) / 165.0).clamp(0.0, 1.0);
    let tinted = tint_color(color, speed_t);
    let ring_col = ring_tint(tinted);

    if class.polygon_sides() == 0 {
        let r = cfg.unit_body_radius(traits);
        let inner = 1.0 - UNIT_RING_WIDTH_FRAC;
        let hp = (unit.current_health / traits.max_health.max(1e-6)).clamp(0.0, 1.0);
        let fill_scale = r * inner * hp.sqrt();
        let aura_base = color.to_srgba();
        let sides = unit_visual_sides(unit.faction);
        let rotation = unit_visual_rotation(unit.faction);
        ec.with_children(|parent| {
            parent.spawn((
                Mesh2d(meshes.add(Circle::new(1.0))),
                MeshMaterial2d(
                    materials.add(ColorMaterial::from_color(unit_glow_color(unit.faction))),
                ),
                Transform::from_xyz(0.0, 0.0, -0.01).with_scale(Vec3::splat(r * 1.48)),
            ));
            parent.spawn((
                Mesh2d(meshes.add(Circle::new(1.0))),
                MeshMaterial2d(materials.add(ColorMaterial::from_color(Color::srgba(
                    aura_base.red,
                    aura_base.green,
                    aura_base.blue,
                    0.0,
                )))),
                Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3::ZERO),
                UnitBoostAura,
            ));
            parent.spawn((
                Mesh2d(meshes.add(RegularPolygon::new(1.0, sides))),
                MeshMaterial2d(materials.add(tinted)),
                Transform::from_xyz(0.0, 0.0, 0.01)
                    .with_rotation(rotation)
                    .with_scale(Vec3::splat(fill_scale)),
                UnitVisualFill,
            ));
            parent.spawn((
                Mesh2d(meshes.add(RegularPolygon::new(1.0, sides))),
                MeshMaterial2d(materials.add(ring_col)),
                Transform::from_xyz(0.0, 0.0, 0.02)
                    .with_rotation(rotation)
                    .with_scale(Vec3::splat(r * 1.16)),
                UnitVisualRing,
            ));
        });
    } else {
        let r = cfg.unit_body_radius(traits) * 1.15;
        ec.insert(Mesh2d(
            meshes.add(RegularPolygon::new(r, class.polygon_sides())),
        ))
        .insert(MeshMaterial2d(materials.add(tinted)));
    }
}
