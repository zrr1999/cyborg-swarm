#![allow(clippy::too_many_arguments)]

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use swarm_model::{
    Faction, GameConfig, GeneticTraitBasis, GeneticTraits, MovementSuppression, SteeringInertia,
    TransformationState, Unit, UnitFitness,
};
use swarm_sim::simulation::decay_movement_suppression;
use swarm_tactics::{SteeringBlendInput, blended_steering_direction};

use crate::plugins::{
    combat::{AttackCooldown, GameOver},
    enemies::EnemyUnit,
    game_flow::GameplayEntity,
    meta_progression::{MetaProgression, MetaStatKind},
    skills::{RunSkills, momentum_speed_multiplier},
    world::{MAP_HALF_H, MAP_HALF_W},
};

pub struct SquadPlugin;

#[derive(Resource, Default)]
pub struct CaptainTarget {
    pub position: Vec2,
    pub active: bool,
}

#[derive(Resource, Default)]
pub struct CaptainAnnouncement {
    text: Option<String>,
    ttl: f32,
}

#[derive(Resource)]
pub struct CaptainHeadingState {
    dir: Vec2,
}

impl Default for CaptainHeadingState {
    fn default() -> Self {
        Self { dir: Vec2::Y }
    }
}

impl CaptainAnnouncement {
    pub fn show(&mut self, text: impl Into<String>) {
        self.text = Some(text.into());
        self.ttl = 2.2;
    }

    pub fn active_text(&self) -> Option<&str> {
        (self.ttl > 0.0).then_some(self.text.as_deref()).flatten()
    }
}

#[derive(Component)]
pub struct SquadMember;

#[derive(Component)]
pub struct SquadCaptain;

#[derive(Component)]
pub struct PendingCaptainPromotion;

#[derive(Component)]
pub struct SquadFollower {
    pub slot_index: usize,
}

#[derive(Component)]
pub struct UnitVisualFill;

#[derive(Component)]
pub struct UnitVisualRing;

#[derive(Component)]
pub struct CaptainCoreGraphic;

#[derive(Component, Default)]
pub struct HitFlash(pub f32);

#[derive(Component, Default)]
pub struct PromotionFlash(pub f32);

pub const SQUAD_SIZE: usize = 8;
const UNIT_RING_WIDTH_FRAC: f32 = 0.2;
const HIT_FLASH_DURATION: f32 = 0.18;
const PROMOTION_FLASH_DURATION: f32 = 0.48;
const PLAYER_SEPARATION_SCALE: f32 = 0.52;
const CLOSE_COMBAT_LOCK_DISTANCE: f32 = 58.0;
const CAPTAIN_COMMAND_OVERRIDE_DISTANCE: f32 = 76.0;
const CAPTAIN_TARGET_STOP_RADIUS_FACTOR: f32 = 1.45;
const CAPTAIN_HEADING_TARGET_DEADZONE_FACTOR: f32 = 2.35;
const CAPTAIN_HEADING_BLEND_PER_SEC: f32 = 8.5;
const FOLLOWER_FORMATION_BREAK_SCALE: f32 = 0.22;
const FOLLOWER_CATCHUP_SPEED_BONUS: f32 = 0.55;
const FOLLOWER_SLOT_SETTLE_RADIUS_FACTOR: f32 = 0.72;
const FORMATION_FILE_SPACING_FACTOR: f32 = 0.72;
const FORMATION_RANK_SPACING_FACTOR: f32 = 0.76;

impl Plugin for SquadPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_squad)
            .init_resource::<CaptainTarget>()
            .init_resource::<CaptainAnnouncement>()
            .init_resource::<CaptainHeadingState>()
            .add_systems(
                Update,
                (
                    update_captain_target_system,
                    steering_move_system,
                    decay_movement_suppression,
                    consume_pending_promotions,
                    tick_announcement_system,
                    decay_visual_flash_system,
                    sync_captain_core_graphics,
                    update_unit_circle_hp_visuals,
                )
                    .chain(),
            );
    }
}

fn spawn_squad(
    mut commands: Commands,
    cfg: Res<GameConfig>,
    meta: Res<MetaProgression>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    spawn_initial_squad(
        &mut commands,
        &cfg,
        &meta,
        meshes.as_mut(),
        materials.as_mut(),
    );
}

pub(crate) fn spawn_initial_squad(
    commands: &mut Commands,
    cfg: &GameConfig,
    meta: &MetaProgression,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
) {
    let traits = base_traits(meta);

    let captain = commands
        .spawn((
            Transform::from_xyz(0.0, 0.0, 1.0),
            Visibility::default(),
            GameplayEntity,
            SquadMember,
            SquadCaptain,
            Unit::new(&traits, Faction::Player),
            traits.clone(),
            GeneticTraitBasis(traits.clone()),
            TransformationState::default(),
            AttackCooldown(0.0),
            SteeringInertia::default(),
            MovementSuppression::default(),
            UnitFitness::default(),
            HitFlash::default(),
            PromotionFlash::default(),
        ))
        .id();
    attach_unit_visuals(
        commands,
        meshes,
        materials,
        captain,
        Faction::Player,
        &traits,
        cfg,
        true,
    );

    for i in 0..SQUAD_SIZE {
        let follower_traits = base_traits(meta);
        let offset = formation_slot_offset(cfg, i, &traits, &follower_traits);
        let entity = commands
            .spawn((
                Transform::from_xyz(offset.x, offset.y, 1.0),
                Visibility::default(),
                GameplayEntity,
                SquadMember,
                SquadFollower { slot_index: i },
                Unit::new(&follower_traits, Faction::Player),
                follower_traits.clone(),
                GeneticTraitBasis(follower_traits.clone()),
                TransformationState::default(),
                AttackCooldown(0.0),
                SteeringInertia::default(),
                MovementSuppression::default(),
                UnitFitness::default(),
                HitFlash::default(),
                PromotionFlash::default(),
            ))
            .id();
        attach_unit_visuals(
            commands,
            meshes,
            materials,
            entity,
            Faction::Player,
            &follower_traits,
            cfg,
            false,
        );
    }
}

pub(crate) fn spawn_player_follower(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    cfg: &GameConfig,
    position: Vec2,
    slot_index: usize,
    traits: GeneticTraits,
) -> Entity {
    let entity = commands
        .spawn((
            Transform::from_xyz(position.x, position.y, 1.0),
            Visibility::default(),
            GameplayEntity,
            SquadMember,
            SquadFollower { slot_index },
            Unit::new(&traits, Faction::Player),
            traits.clone(),
            GeneticTraitBasis(traits.clone()),
            TransformationState::default(),
            AttackCooldown(0.0),
            SteeringInertia::default(),
            MovementSuppression::default(),
            UnitFitness::default(),
            HitFlash::default(),
            PromotionFlash::default(),
        ))
        .id();
    attach_unit_visuals(
        commands,
        meshes,
        materials,
        entity,
        Faction::Player,
        &traits,
        cfg,
        false,
    );
    entity
}

pub(crate) fn attach_unit_visuals(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    entity: Entity,
    faction: Faction,
    traits: &GeneticTraits,
    cfg: &GameConfig,
    is_captain: bool,
) {
    let color = faction_base_color(faction);
    let speed_t = ((traits.speed - 15.0) / 165.0).clamp(0.0, 1.0);
    let tinted = tint_color(color, speed_t);
    let ring = ring_tint(tinted);
    let radius = cfg.unit_body_radius(traits);
    let sides = unit_visual_sides(faction);
    let rotation = unit_visual_rotation(faction);

    commands.entity(entity).with_children(|parent| {
        parent.spawn((
            Mesh2d(meshes.add(Circle::new(1.0))),
            MeshMaterial2d(materials.add(ColorMaterial::from_color(unit_glow_color(faction)))),
            Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3::splat(radius * 1.45)),
        ));
        parent.spawn((
            Mesh2d(meshes.add(RegularPolygon::new(1.0, sides))),
            MeshMaterial2d(materials.add(ColorMaterial::from_color(tinted))),
            Transform::from_xyz(0.0, 0.0, 0.01)
                .with_rotation(rotation)
                .with_scale(Vec3::splat(radius * (1.0 - UNIT_RING_WIDTH_FRAC))),
            UnitVisualFill,
        ));
        parent.spawn((
            Mesh2d(meshes.add(RegularPolygon::new(1.0, sides))),
            MeshMaterial2d(materials.add(ColorMaterial::from_color(ring))),
            Transform::from_xyz(0.0, 0.0, 0.02)
                .with_rotation(rotation)
                .with_scale(Vec3::splat(radius * 1.16)),
            UnitVisualRing,
        ));
        if is_captain {
            parent.spawn((
                Mesh2d(meshes.add(Circle::new(1.0))),
                MeshMaterial2d(
                    materials.add(ColorMaterial::from_color(Color::srgb(1.0, 0.95, 0.72))),
                ),
                Transform::from_xyz(0.0, 0.0, 0.03)
                    .with_scale(Vec3::splat((radius * 0.24).max(2.4))),
                CaptainCoreGraphic,
            ));
        }
    });
}

fn formation_slot_shape(i: usize) -> Vec2 {
    let row = i / 2;
    let side = if i.is_multiple_of(2) { -1.0 } else { 1.0 };
    let lateral_scale = match row {
        0 => 0.9,
        1 => 1.15,
        2 => 0.95,
        _ => 0.65,
    };
    Vec2::new(side * lateral_scale, -(row as f32 + 1.0))
}

fn formation_slot_offset(
    cfg: &GameConfig,
    slot_index: usize,
    captain_traits: &GeneticTraits,
    follower_traits: &GeneticTraits,
) -> Vec2 {
    let shape = formation_slot_shape(slot_index);
    let spacing = cfg.unit_pair_separation_distance(captain_traits, follower_traits);
    Vec2::new(
        shape.x * spacing * FORMATION_FILE_SPACING_FACTOR,
        shape.y * spacing * FORMATION_RANK_SPACING_FACTOR,
    )
}

pub fn base_traits(meta: &MetaProgression) -> GeneticTraits {
    GeneticTraits {
        max_health: meta.boosted_stat(80.0, MetaStatKind::MaxHealth),
        attack: meta.boosted_stat(12.0, MetaStatKind::Attack),
        speed: meta.boosted_stat(180.0, MetaStatKind::Speed),
        armor: meta.boosted_stat(2.0, MetaStatKind::Armor),
        attack_speed: meta.boosted_stat(1.2, MetaStatKind::AttackSpeed),
        sensor_range: meta.boosted_stat(120.0, MetaStatKind::SensorRange),
    }
}

fn update_captain_target_system(
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    mut target: ResMut<CaptainTarget>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((cam, cam_tf)) = camera.single() else {
        return;
    };
    let Some(cursor): Option<Vec2> = window.cursor_position() else {
        target.active = false;
        return;
    };
    let Ok(world) = cam.viewport_to_world_2d(cam_tf, cursor) else {
        target.active = false;
        return;
    };

    target.position = Vec2::new(
        world.x.clamp(-MAP_HALF_W, MAP_HALF_W),
        world.y.clamp(-MAP_HALF_H, MAP_HALF_H),
    );
    target.active = true;
}

#[derive(Clone, Copy)]
struct MotionSnapshot {
    entity: Entity,
    position: Vec2,
    faction: Faction,
    suppression: f32,
    health_fraction: f32,
    body_radius: f32,
    motion_dir: Vec2,
    is_captain: bool,
}

#[allow(clippy::type_complexity)]
fn steering_move_system(
    cfg: Res<GameConfig>,
    captain_target: Res<CaptainTarget>,
    mut captain_heading_state: ResMut<CaptainHeadingState>,
    game_over: Res<GameOver>,
    skills: Res<RunSkills>,
    mut units: ParamSet<(
        Query<(
            Entity,
            &Transform,
            &Unit,
            &GeneticTraits,
            &MovementSuppression,
            &SteeringInertia,
            Option<&SquadCaptain>,
        )>,
        Query<(
            Entity,
            &mut Transform,
            &Unit,
            &GeneticTraits,
            &MovementSuppression,
            &mut SteeringInertia,
            Option<&SquadCaptain>,
            Option<&SquadFollower>,
            Option<&EnemyUnit>,
        )>,
    )>,
    time: Res<Time>,
) {
    if game_over.active {
        return;
    }

    let dt = time.delta_secs();
    let snapshots: Vec<MotionSnapshot> = units
        .p0()
        .iter()
        .map(
            |(entity, tf, unit, traits, suppression, inertia, captain)| MotionSnapshot {
                entity,
                position: tf.translation.truncate(),
                faction: unit.faction,
                suppression: suppression.level,
                health_fraction: (unit.current_health / traits.max_health.max(1.0)).clamp(0.0, 1.0),
                body_radius: cfg.unit_body_radius(traits),
                motion_dir: inertia.smoothed_dir,
                is_captain: captain.is_some(),
            },
        )
        .collect();
    if snapshots.is_empty() {
        return;
    }

    let captain_position = snapshots
        .iter()
        .find(|snapshot| snapshot.is_captain)
        .map(|snapshot| snapshot.position);
    let captain_body_radius = snapshots
        .iter()
        .find(|snapshot| snapshot.is_captain)
        .map(|snapshot| snapshot.body_radius);
    let captain_motion_dir = snapshots
        .iter()
        .find(|snapshot| snapshot.is_captain)
        .map(|snapshot| snapshot.motion_dir);
    let captain_heading = blended_captain_heading(
        captain_position,
        captain_body_radius,
        captain_motion_dir,
        captain_target.as_ref(),
        &snapshots,
        captain_heading_state.dir,
        dt,
    );
    captain_heading_state.dir = captain_heading;
    let player_positions: Vec<Vec2> = snapshots
        .iter()
        .filter(|snapshot| snapshot.faction == Faction::Player)
        .map(|snapshot| snapshot.position)
        .collect();
    let player_centroid = if player_positions.is_empty() {
        Vec2::ZERO
    } else {
        player_positions.iter().copied().sum::<Vec2>() / player_positions.len() as f32
    };
    let steering_cfg = &cfg.steering;

    for (entity, mut transform, unit, traits, suppression, mut inertia, captain, follower, enemy) in
        units.p1().iter_mut()
    {
        let pos = transform.translation.truncate();
        let (chase_dir, nearest_enemy_distance, nearest_enemy_sup) =
            nearest_enemy_motion(entity, pos, unit.faction, &snapshots);
        let has_chase = chase_dir.length_squared() > 1e-6
            && nearest_enemy_distance <= traits.sensor_range * 1.15;
        let mut separation = ally_separation(entity, pos, unit.faction, traits, &cfg, &snapshots);
        if enemy.is_none() {
            separation *= PLAYER_SEPARATION_SCALE;
        }

        let (mut squad_follow_dir, mut squad_follow_strength, squad_follow_distance) =
            follower_motion(
                pos,
                follower,
                captain_position,
                captain_body_radius,
                captain_heading,
                cfg.unit_body_radius(traits),
                traits.sensor_range,
                &cfg,
                &cfg.steering,
            );
        let mut goal_dir = Vec2::ZERO;
        let mut goal_distance = 0.0;
        if captain.is_some() && captain_target.active {
            let to_goal = captain_target.position - pos;
            goal_dir = to_goal.normalize_or_zero();
            goal_distance = to_goal.length();
        } else if enemy.is_some() {
            let to_goal = player_centroid - pos;
            goal_dir = to_goal.normalize_or_zero();
            goal_distance = to_goal.length();
        } else if follower.is_some() && squad_follow_distance > 1e-4 {
            goal_dir = squad_follow_dir;
            goal_distance = squad_follow_distance;
        } else if captain_target.active {
            let to_goal = captain_target.position - pos;
            goal_dir = to_goal.normalize_or_zero();
            goal_distance = to_goal.length();
        }
        let formation_break_distance =
            (traits.sensor_range * FOLLOWER_FORMATION_BREAK_SCALE).max(24.0);
        let is_out_of_formation = squad_follow_distance > formation_break_distance;
        let captain_holding_line = captain.is_some()
            && captain_target.active
            && goal_distance > CAPTAIN_COMMAND_OVERRIDE_DISTANCE
            && nearest_enemy_distance > CLOSE_COMBAT_LOCK_DISTANCE;
        let allow_chase = if enemy.is_some() || nearest_enemy_distance <= CLOSE_COMBAT_LOCK_DISTANCE
        {
            has_chase
        } else if follower.is_some() && captain_target.active && is_out_of_formation {
            false
        } else {
            has_chase && !captain_holding_line
        };

        let retreat_dir = if allow_chase { -chase_dir } else { Vec2::ZERO };
        let should_retreat = enemy.is_none()
            && captain.is_none()
            && allow_chase
            && nearest_enemy_distance < traits.sensor_range * 0.75
            && snapshots
                .iter()
                .find(|snapshot| snapshot.entity == entity)
                .is_some_and(|snapshot| snapshot.health_fraction < 0.28);

        let captain_stop_radius =
            (cfg.unit_body_radius(traits) * CAPTAIN_TARGET_STOP_RADIUS_FACTOR).max(10.0);
        let captain_holding_target = captain.is_some()
            && captain_target.active
            && !allow_chase
            && goal_distance <= captain_stop_radius;
        if captain_holding_target {
            goal_dir = Vec2::ZERO;
            goal_distance = 0.0;
        }

        let follower_settle_radius =
            (cfg.unit_body_radius(traits) * FOLLOWER_SLOT_SETTLE_RADIUS_FACTOR).max(6.0);
        let follower_settled =
            follower.is_some() && squad_follow_distance <= follower_settle_radius;
        if follower_settled && !allow_chase && !should_retreat {
            let settle_t = (squad_follow_distance / follower_settle_radius).clamp(0.0, 1.0);
            squad_follow_strength *= settle_t;
            if goal_distance <= follower_settle_radius {
                goal_dir = Vec2::ZERO;
                goal_distance = 0.0;
            }
            if squad_follow_distance <= follower_settle_radius * 0.5 {
                squad_follow_dir = Vec2::ZERO;
            }
        }

        let final_dir = blended_steering_direction(
            steering_cfg,
            &SteeringBlendInput {
                flow_dir: goal_dir,
                separation,
                chase_dir: if allow_chase { chase_dir } else { Vec2::ZERO },
                retreat_dir,
                rescue_dir: Vec2::ZERO,
                rescue_strength: 0.0,
                beacon_force: Vec2::ZERO,
                defend_base_dir: Vec2::ZERO,
                defend_base_strength: 0.0,
                should_retreat,
                has_chase: allow_chase,
                has_beacon: false,
                has_defend_base: false,
                home_rally_dir: Vec2::ZERO,
                home_rally_strength: 0.0,
                has_home_rally: false,
                nearest_enemy_sup,
                squad_spread_dir: Vec2::ZERO,
                squad_spread_strength: 0.0,
                squad_follow_dir,
                squad_follow_strength,
            },
        );

        let blend = (steering_cfg.inertia_blend_per_sec * dt).min(1.0);
        if final_dir.length_squared() > 1e-10 {
            inertia.smoothed_dir = if inertia.smoothed_dir.length_squared() < 1e-10 {
                final_dir
            } else {
                (inertia.smoothed_dir * (1.0 - blend) + final_dir * blend).normalize_or_zero()
            };
        } else if captain_holding_target || (follower_settled && !allow_chase && !should_retreat) {
            inertia.smoothed_dir = Vec2::ZERO;
        } else {
            let decay = (1.0 - dt * steering_cfg.inertia_zero_decay_per_sec).max(0.0);
            inertia.smoothed_dir *= decay;
        }

        let move_dir = inertia.smoothed_dir.normalize_or_zero();
        if move_dir.length_squared() < 1e-10 {
            continue;
        }

        let slow = (1.0 - 0.55 * suppression.level).max(0.34);
        let pursuit_bonus = if has_chase {
            1.0 + steering_cfg.pursuit_speed_bonus * 0.5
        } else {
            1.0
        };
        let retreat_bonus = if should_retreat {
            1.0 + steering_cfg.retreat_speed_bonus
        } else {
            1.0
        };
        let arrival_scale = if !has_chase && !should_retreat && goal_distance > 0.0 {
            let radius = (traits.sensor_range * 0.35).max(18.0);
            let t = (goal_distance / radius).clamp(0.0, 1.0);
            steering_cfg.goal_arrival_speed_frac + (1.0 - steering_cfg.goal_arrival_speed_frac) * t
        } else {
            1.0
        };
        let catchup_radius =
            (traits.sensor_range * steering_cfg.squad_follow_catchup_scale).max(16.0);
        let catchup_bonus = if follower.is_some() {
            1.0 + FOLLOWER_CATCHUP_SPEED_BONUS
                * (squad_follow_distance / catchup_radius).clamp(0.0, 1.0)
        } else {
            1.0
        };

        let skill_speed = if enemy.is_none() {
            momentum_speed_multiplier(skills.momentum_stacks)
        } else {
            1.0
        };
        let step = traits.speed
            * skill_speed
            * slow
            * pursuit_bonus
            * retreat_bonus
            * catchup_bonus
            * arrival_scale
            * dt;
        transform.translation += (move_dir * step).extend(0.0);
        transform.translation.x = transform.translation.x.clamp(-MAP_HALF_W, MAP_HALF_W);
        transform.translation.y = transform.translation.y.clamp(-MAP_HALF_H, MAP_HALF_H);
    }
}

fn nearest_enemy_motion(
    entity: Entity,
    pos: Vec2,
    faction: Faction,
    snapshots: &[MotionSnapshot],
) -> (Vec2, f32, f32) {
    let Some(nearest) = snapshots
        .iter()
        .filter(|snapshot| snapshot.entity != entity && faction.is_enemy_of(&snapshot.faction))
        .min_by(|a, b| {
            pos.distance_squared(a.position)
                .partial_cmp(&pos.distance_squared(b.position))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    else {
        return (Vec2::ZERO, f32::INFINITY, 0.0);
    };

    let to_enemy = nearest.position - pos;
    (
        to_enemy.normalize_or_zero(),
        to_enemy.length(),
        nearest.suppression.clamp(0.0, 1.0),
    )
}

fn ally_separation(
    entity: Entity,
    pos: Vec2,
    faction: Faction,
    traits: &GeneticTraits,
    cfg: &GameConfig,
    snapshots: &[MotionSnapshot],
) -> Vec2 {
    let mut separation = Vec2::ZERO;
    let my_radius = cfg.unit_body_radius(traits);
    for snapshot in snapshots {
        if snapshot.entity == entity || snapshot.faction != faction {
            continue;
        }
        let delta = pos - snapshot.position;
        let distance = delta.length();
        let radius = cfg.unit_pair_separation_distance_from_radii(my_radius, snapshot.body_radius);
        if distance > 1e-3 && distance < radius {
            separation += delta.normalize() * (1.0 - distance / radius);
        }
    }
    separation.normalize_or_zero()
}

fn follower_motion(
    pos: Vec2,
    follower: Option<&SquadFollower>,
    captain_position: Option<Vec2>,
    captain_body_radius: Option<f32>,
    captain_heading: Vec2,
    follower_body_radius: f32,
    sensor_range: f32,
    cfg: &GameConfig,
    steering_cfg: &swarm_model::SteeringMovementConfig,
) -> (Vec2, f32, f32) {
    let Some(follower) = follower else {
        return (Vec2::ZERO, 0.0, 0.0);
    };
    let Some(captain_position) = captain_position else {
        return (Vec2::ZERO, 0.0, 0.0);
    };
    let Some(captain_body_radius) = captain_body_radius else {
        return (Vec2::ZERO, 0.0, 0.0);
    };
    let forward = if captain_heading.length_squared() > 1e-6 {
        captain_heading.normalize()
    } else {
        Vec2::Y
    };
    let right = Vec2::new(forward.y, -forward.x);
    let slot_offset = formation_slot_offset_from_radii(
        cfg,
        follower.slot_index,
        captain_body_radius,
        follower_body_radius,
    );
    let target = captain_position + right * slot_offset.x + forward * slot_offset.y;
    let to_target = target - pos;
    let distance = to_target.length();
    let catchup_radius = (sensor_range * steering_cfg.squad_follow_catchup_scale).max(16.0);
    (
        to_target.normalize_or_zero(),
        (distance / catchup_radius).clamp(0.0, 1.0),
        distance,
    )
}

fn formation_slot_offset_from_radii(
    cfg: &GameConfig,
    slot_index: usize,
    captain_body_radius: f32,
    follower_body_radius: f32,
) -> Vec2 {
    let shape = formation_slot_shape(slot_index);
    let spacing =
        cfg.unit_pair_separation_distance_from_radii(captain_body_radius, follower_body_radius);
    Vec2::new(
        shape.x * spacing * FORMATION_FILE_SPACING_FACTOR,
        shape.y * spacing * FORMATION_RANK_SPACING_FACTOR,
    )
}

fn blended_captain_heading(
    captain_position: Option<Vec2>,
    captain_body_radius: Option<f32>,
    captain_motion_dir: Option<Vec2>,
    captain_target: &CaptainTarget,
    snapshots: &[MotionSnapshot],
    previous_heading: Vec2,
    dt: f32,
) -> Vec2 {
    let previous_heading = if previous_heading.length_squared() > 1e-6 {
        previous_heading.normalize()
    } else {
        Vec2::Y
    };
    let Some(captain_position) = captain_position else {
        return previous_heading;
    };
    let desired_heading = if let Some(motion_dir) = captain_motion_dir {
        if motion_dir.length_squared() > 1e-6 {
            motion_dir.normalize()
        } else {
            Vec2::ZERO
        }
    } else {
        Vec2::ZERO
    };
    let desired_heading = if desired_heading.length_squared() > 1e-6 {
        desired_heading
    } else if captain_target.active {
        let to_goal = captain_target.position - captain_position;
        let deadzone = captain_body_radius.unwrap_or(6.0) * CAPTAIN_HEADING_TARGET_DEADZONE_FACTOR;
        if to_goal.length() > deadzone.max(8.0) {
            to_goal.normalize_or_zero()
        } else {
            Vec2::ZERO
        }
    } else {
        Vec2::ZERO
    };
    let desired_heading = if desired_heading.length_squared() > 1e-6 {
        desired_heading
    } else {
        let (enemy_dir, _, _) = nearest_enemy_motion(
            Entity::PLACEHOLDER,
            captain_position,
            Faction::Player,
            snapshots,
        );
        if enemy_dir.length_squared() > 1e-6 {
            enemy_dir
        } else {
            previous_heading
        }
    };
    let blend = (CAPTAIN_HEADING_BLEND_PER_SEC * dt).min(1.0);
    (previous_heading * (1.0 - blend) + desired_heading * blend).normalize_or_zero()
}

fn consume_pending_promotions(
    mut commands: Commands,
    mut announcement: ResMut<CaptainAnnouncement>,
    mut query: Query<(Entity, &mut PromotionFlash), With<PendingCaptainPromotion>>,
) {
    for (entity, mut promotion_flash) in query.iter_mut() {
        promotion_flash.0 = 1.0;
        commands.entity(entity).remove::<PendingCaptainPromotion>();
        if announcement.active_text().is_none() {
            announcement.show("Captain inherited");
        }
    }
}

fn tick_announcement_system(mut announcement: ResMut<CaptainAnnouncement>, time: Res<Time>) {
    if announcement.ttl > 0.0 {
        announcement.ttl = (announcement.ttl - time.delta_secs()).max(0.0);
        if announcement.ttl <= 0.0 {
            announcement.text = None;
        }
    }
}

fn decay_visual_flash_system(
    time: Res<Time>,
    mut hits: Query<&mut HitFlash>,
    mut promotions: Query<&mut PromotionFlash>,
) {
    let hit_decay = time.delta_secs() / HIT_FLASH_DURATION;
    for mut flash in &mut hits {
        flash.0 = (flash.0 - hit_decay).max(0.0);
    }
    let promotion_decay = time.delta_secs() / PROMOTION_FLASH_DURATION;
    for mut flash in &mut promotions {
        flash.0 = (flash.0 - promotion_decay).max(0.0);
    }
}

fn sync_captain_core_graphics(
    mut commands: Commands,
    cfg: Res<GameConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    units: Query<(Entity, &Unit, &GeneticTraits, Option<&Children>), Without<EnemyUnit>>,
    captains: Query<(), With<SquadCaptain>>,
    core_q: Query<(), With<CaptainCoreGraphic>>,
) {
    for (entity, unit, traits, children_opt) in units.iter() {
        if unit.is_dead() {
            continue;
        }
        let is_captain = captains.get(entity).is_ok();
        let has_core = children_opt
            .map(|children| children.iter().any(|child| core_q.get(child).is_ok()))
            .unwrap_or(false);
        if is_captain && !has_core {
            let radius = cfg.unit_body_radius(traits);
            commands.entity(entity).with_children(|parent| {
                parent.spawn((
                    Mesh2d(meshes.add(Circle::new(1.0))),
                    MeshMaterial2d(
                        materials.add(ColorMaterial::from_color(Color::srgb(1.0, 0.95, 0.72))),
                    ),
                    Transform::from_xyz(0.0, 0.0, 0.03)
                        .with_scale(Vec3::splat((radius * 0.24).max(2.4))),
                    CaptainCoreGraphic,
                ));
            });
        } else if !is_captain
            && has_core
            && let Some(children) = children_opt
        {
            for child in children.iter() {
                if core_q.get(child).is_ok() {
                    commands.entity(child).despawn();
                }
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn update_unit_circle_hp_visuals(
    cfg: Res<GameConfig>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    units: Query<(
        &Unit,
        &GeneticTraits,
        &HitFlash,
        &PromotionFlash,
        Option<&SquadCaptain>,
    )>,
    mut visual_q: Query<
        (
            &ChildOf,
            &mut Transform,
            Option<&UnitVisualFill>,
            Option<&UnitVisualRing>,
            &MeshMaterial2d<ColorMaterial>,
        ),
        Without<CaptainCoreGraphic>,
    >,
    mut core_q: Query<
        (&ChildOf, &mut Transform, &MeshMaterial2d<ColorMaterial>),
        With<CaptainCoreGraphic>,
    >,
) {
    let inner = 1.0 - UNIT_RING_WIDTH_FRAC;
    for (parent, mut tf, fill_m, ring_m, mat_h) in visual_q.iter_mut() {
        let Ok((unit, traits, hit_flash, _promotion_flash, _captain)) = units.get(parent.parent())
        else {
            continue;
        };
        let radius = cfg.unit_body_radius(traits);
        let speed_t = ((traits.speed - 15.0) / 165.0).clamp(0.0, 1.0);
        let base_color = faction_base_color(unit.faction);
        if fill_m.is_some() {
            let hp = (unit.current_health / traits.max_health.max(1e-6)).clamp(0.0, 1.0);
            let fill_scale = radius * inner * hp.sqrt();
            tf.scale = Vec3::splat(fill_scale);
            if let Some(mat) = materials.get_mut(mat_h) {
                let tinted = tint_color(base_color, speed_t);
                mat.color = lerp_color(tinted, Color::WHITE, hit_flash.0 * 0.45);
            }
        } else if ring_m.is_some() {
            tf.scale = Vec3::splat(radius * 1.16);
            if let Some(mat) = materials.get_mut(mat_h) {
                mat.color = ring_tint(tint_color(base_color, speed_t));
            }
        }
    }

    for (parent, mut tf, mat_h) in &mut core_q {
        let Ok((_unit, traits, _hit_flash, promotion_flash, captain)) = units.get(parent.parent())
        else {
            continue;
        };
        let radius = cfg.unit_body_radius(traits);
        let pulse = if captain.is_some() {
            1.0 + promotion_flash.0 * 0.16
        } else {
            1.0
        };
        tf.translation = Vec3::new(0.0, 0.0, 0.03);
        tf.rotation = Quat::IDENTITY;
        tf.scale = Vec3::splat((radius * 0.24).max(2.4) * pulse.max(0.0));
        if let Some(mat) = materials.get_mut(mat_h) {
            mat.color = lerp_color(
                Color::srgb(1.0, 0.95, 0.72),
                Color::WHITE,
                promotion_flash.0 * 0.28,
            );
        }
    }
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let a = a.to_srgba();
    let b = b.to_srgba();
    Color::srgba(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
        a.alpha + (b.alpha - a.alpha) * t,
    )
}

fn faction_base_color(faction: Faction) -> Color {
    match faction {
        Faction::Player => Color::srgb(0.16, 0.86, 1.0),
        Faction::Enemy => Color::srgb(1.0, 0.16, 0.34),
        Faction::Neutral => Color::srgb(1.0, 0.65, 0.18),
    }
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

fn unit_glow_color(faction: Faction) -> Color {
    let c = faction_base_color(faction).to_srgba();
    Color::srgba(c.red, c.green, c.blue, 0.14)
}

fn tint_color(base: Color, t: f32) -> Color {
    let base = base.to_srgba();
    let warm = Color::srgb(1.0, 0.88, 0.64).to_srgba();
    Color::srgba(
        base.red + (warm.red - base.red) * t * 0.35,
        base.green + (warm.green - base.green) * t * 0.2,
        base.blue + (warm.blue - base.blue) * t * 0.08,
        1.0,
    )
}

fn ring_tint(base: Color) -> Color {
    let base = base.to_srgba();
    Color::srgba(
        (base.red * 1.06).clamp(0.0, 1.0),
        (base.green * 1.06).clamp(0.0, 1.0),
        (base.blue * 1.06).clamp(0.0, 1.0),
        0.38,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formation_slots_stay_tight_and_trailing() {
        let cfg = GameConfig::default();
        let captain = GeneticTraits::default();
        let follower = GeneticTraits::default();
        let front_left = formation_slot_offset(&cfg, 0, &captain, &follower);
        let front_right = formation_slot_offset(&cfg, 1, &captain, &follower);
        let rear_left = formation_slot_offset(&cfg, 6, &captain, &follower);
        let rear_right = formation_slot_offset(&cfg, 7, &captain, &follower);

        assert!(front_left.y < 0.0);
        assert!(rear_left.y < front_left.y);
        assert!((front_left.x + front_right.x).abs() < f32::EPSILON);
        assert!((rear_left.x + rear_right.x).abs() < f32::EPSILON);
    }

    #[test]
    fn follower_slots_rotate_with_captain_heading() {
        let follower = SquadFollower { slot_index: 0 };
        let cfg = swarm_model::SteeringMovementConfig::default();
        let game_cfg = GameConfig::default();
        let (_, _, dist_up) = follower_motion(
            Vec2::ZERO,
            Some(&follower),
            Some(Vec2::ZERO),
            Some(5.0),
            Vec2::Y,
            5.0,
            120.0,
            &game_cfg,
            &cfg,
        );
        let (dir_right, _, dist_right) = follower_motion(
            Vec2::ZERO,
            Some(&follower),
            Some(Vec2::ZERO),
            Some(5.0),
            Vec2::X,
            5.0,
            120.0,
            &game_cfg,
            &cfg,
        );

        assert!(dist_up > 0.0);
        assert!((dist_right - dist_up).abs() < 0.001);
        assert!(dir_right.x < -0.4);
        assert!(dir_right.y > 0.4);
    }

    #[test]
    fn captain_heading_holds_previous_direction_inside_deadzone() {
        let heading = blended_captain_heading(
            Some(Vec2::ZERO),
            Some(10.0),
            None,
            &CaptainTarget {
                position: Vec2::new(8.0, 0.0),
                active: true,
            },
            &[],
            Vec2::Y,
            1.0 / 60.0,
        );

        assert!(heading.dot(Vec2::Y) > 0.98);
    }

    #[test]
    fn captain_heading_prefers_recent_motion_over_cursor_flip() {
        let heading = blended_captain_heading(
            Some(Vec2::ZERO),
            Some(10.0),
            Some(Vec2::Y),
            &CaptainTarget {
                position: Vec2::new(-80.0, 0.0),
                active: true,
            },
            &[],
            Vec2::X,
            1.0,
        );

        assert!(heading.dot(Vec2::Y) > 0.98);
    }

    #[test]
    fn upgraded_units_claim_more_formation_space() {
        let cfg = GameConfig::default();
        let captain = GeneticTraits::default();
        let upgraded = captain.scale_by(1.6);
        let base_offset = formation_slot_offset(&cfg, 0, &captain, &captain);
        let upgraded_offset = formation_slot_offset(&cfg, 0, &upgraded, &upgraded);

        assert!(upgraded_offset.length() > base_offset.length());
    }
}
