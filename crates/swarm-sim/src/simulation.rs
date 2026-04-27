//! Core simulation helpers and ECS systems shared across frontends.
//!
//! This module intentionally contains logic-only systems that do not depend on
//! theme rendering, particles, or UI composition. The Bevy client wires these
//! systems into its schedule, and future headless runtimes can reuse the same
//! rules without pulling gameplay logic back into the theme crate.

use bevy_ecs::prelude::*;
use bevy_math::Vec2;
use bevy_time::Time;
use bevy_transform::components::Transform;

use swarm_combat::Biomass;
use swarm_model::{
    Base, Faction, GameConfig, GeneticTraitBasis, GeneticTraits, MovementSuppression, Unit,
};
use swarm_space::{FlowFieldSet, SpatialHash};

/// Matches the existing theme-side suppression tuning until this becomes a
/// configurable simulation parameter block.
const SUPPRESSION_DECAY_PER_SEC: f32 = 1.2;

/// `None` while playing; set when the match ends.
#[derive(Resource, Default)]
pub struct GameEnd(pub Option<GameOutcome>);

#[derive(Clone, Copy, Debug)]
pub struct GameOutcome {
    pub victory: bool,
}

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct DominationState {
    pub faction: Option<Faction>,
    pub elapsed: f32,
}

impl DominationState {
    pub fn remaining(&self, cfg: &GameConfig) -> Option<f32> {
        self.faction
            .map(|_| (cfg.domination_hold_seconds - self.elapsed).max(0.0))
    }
}

/// Systems that should freeze while a match result is showing use this condition.
pub fn match_is_active(game_end: Res<GameEnd>) -> bool {
    game_end.0.is_none()
}

/// Decay combat suppression toward zero over time.
pub fn decay_movement_suppression(time: Res<Time>, mut query: Query<&mut MovementSuppression>) {
    let dt = time.delta_secs();
    let k = SUPPRESSION_DECAY_PER_SEC * dt;
    for mut suppression in &mut query {
        suppression.level = (suppression.level * (1.0 - k).max(0.0)).min(1.0);
    }
}

/// Rebuild the per-frame spatial hash from current unit positions.
pub fn update_spatial_hash(
    mut hash: ResMut<SpatialHash>,
    query: Query<(Entity, &Transform), With<Unit>>,
) {
    hash.clear();
    for (entity, tf) in query.iter() {
        hash.insert(entity, tf.translation.truncate());
    }
}

/// Regenerate both attack flow fields from the latest base ownership state.
pub fn sync_flow_fields_each_frame(
    mut flow_fields: ResMut<FlowFieldSet>,
    cfg: Res<GameConfig>,
    base_query: Query<(&Transform, &Base)>,
) {
    sync_flow_fields_to_bases(&mut flow_fields, &cfg, &base_query);
}

/// Pure helper form of [`sync_flow_fields_each_frame`] for callers that need
/// to refresh flow fields outside the main update loop.
pub fn sync_flow_fields_to_bases(
    flow_fields: &mut FlowFieldSet,
    cfg: &GameConfig,
    base_query: &Query<(&Transform, &Base)>,
) {
    let mut player_targets: Vec<(Vec2, f32)> = Vec::new();
    let mut enemy_targets: Vec<(Vec2, f32)> = Vec::new();
    let mut neutral_targets: Vec<(Vec2, f32)> = Vec::new();

    for (tf, base) in base_query.iter() {
        if base.is_destroyed() {
            continue;
        }
        let position = tf.translation.truncate();
        let weight = cfg.flow_capture_goal_weight(base);
        match base.faction {
            Faction::Player => player_targets.push((position, weight)),
            Faction::Enemy => enemy_targets.push((position, weight)),
            Faction::Neutral => neutral_targets.push((position, weight)),
        }
    }

    let mut attack_enemy = enemy_targets;
    attack_enemy.extend_from_slice(&neutral_targets);
    flow_fields.attack_enemy.generate_weighted(&attack_enemy);

    let mut attack_player = player_targets;
    attack_player.extend_from_slice(&neutral_targets);
    flow_fields.attack_player.generate_weighted(&attack_player);
}

/// Accumulate passive biomass income from controlled bases.
pub fn biomass_income_system(
    time: Res<Time>,
    cfg: Res<GameConfig>,
    mut biomass: ResMut<Biomass>,
    base_query: Query<&Base>,
) {
    let dt = time.delta_secs();
    for base in base_query.iter() {
        match base.faction {
            Faction::Player => biomass.player += cfg.base_income_for_level(base.level) * dt,
            Faction::Enemy => biomass.enemy += cfg.base_income_for_level(base.level) * dt,
            Faction::Neutral => {}
        }
    }
}

/// Detect full-map domination and advance the hold timer toward a match result.
pub fn detect_domination_outcome(
    time: Res<Time>,
    cfg: Res<GameConfig>,
    base_query: Query<&Base>,
    mut domination: ResMut<DominationState>,
    mut game_end: ResMut<GameEnd>,
) {
    if game_end.0.is_some() {
        return;
    }

    let mut owner: Option<Faction> = None;
    let mut total_bases = 0u32;
    for base in base_query.iter() {
        total_bases += 1;
        if matches!(base.faction, Faction::Neutral) {
            domination.faction = None;
            domination.elapsed = 0.0;
            return;
        }
        match owner {
            None => owner = Some(base.faction),
            Some(existing) if existing == base.faction => {}
            Some(_) => {
                domination.faction = None;
                domination.elapsed = 0.0;
                return;
            }
        }
    }

    let Some(owner) = owner else {
        domination.faction = None;
        domination.elapsed = 0.0;
        return;
    };
    if total_bases == 0 {
        domination.faction = None;
        domination.elapsed = 0.0;
        return;
    }

    if domination.faction != Some(owner) {
        domination.faction = Some(owner);
        domination.elapsed = 0.0;
    }
    domination.elapsed += time.delta_secs();

    if domination.elapsed >= cfg.domination_hold_seconds {
        game_end.0 = Some(GameOutcome {
            victory: owner == Faction::Player,
        });
        domination.elapsed = cfg.domination_hold_seconds;
    }
}

/// True if `pos` lies inside a friendly base's capture radius.
pub fn unit_inside_own_base(
    pos: Vec2,
    unit_faction: Faction,
    bases: &Query<(Entity, &Transform, &Base)>,
) -> bool {
    own_base_entity_at_position(pos, unit_faction, bases).is_some()
}

/// Return the friendly base whose capture radius currently contains `pos`.
pub fn own_base_entity_at_position(
    pos: Vec2,
    unit_faction: Faction,
    bases: &Query<(Entity, &Transform, &Base)>,
) -> Option<Entity> {
    if unit_faction == Faction::Neutral {
        return None;
    }
    for (entity, base_tf, base) in bases.iter() {
        if base.faction != unit_faction {
            continue;
        }
        if (pos - base_tf.translation.truncate()).length() < base.capture_radius {
            return Some(entity);
        }
    }
    None
}

/// Apply passive and home-base health regeneration once a unit has been out of
/// combat long enough.
pub fn unit_hp_regen_system(
    time: Res<Time>,
    cfg: Res<GameConfig>,
    base_query: Query<(Entity, &Transform, &Base)>,
    mut query: Query<(&mut Unit, &GeneticTraits, &Transform)>,
) {
    let dt = time.delta_secs();
    let rate = cfg.unit_hp_regen_fraction_of_max_per_sec;
    let home_bonus = cfg.unit_home_base_hp_regen_bonus_fraction_of_max_per_sec;
    if dt <= 0.0 {
        return;
    }
    let delay = cfg.unit_hp_regen_delay_after_damage_sec.max(0.0);
    let allow_global = rate > 0.0;
    let allow_home = home_bonus > 0.0;
    if !allow_global && !allow_home {
        return;
    }

    for (mut unit, traits, tf) in query.iter_mut() {
        if unit.is_dead() {
            continue;
        }
        let max_health = traits.max_health.max(1e-6);
        if unit.current_health >= max_health {
            continue;
        }
        if unit.seconds_since_last_damage < delay {
            continue;
        }

        let at_home = unit_inside_own_base(tf.translation.truncate(), unit.faction, &base_query);
        let mut gain = 0.0_f32;
        if allow_global {
            gain += max_health * rate * dt;
        }
        if allow_home && at_home {
            gain += max_health * home_bonus * dt;
        }
        if gain > 0.0 {
            unit.current_health = (unit.current_health + gain).min(max_health);
        }
    }
}

/// Advance unit levels and stat scaling from accumulated experience.
pub fn level_up_system(
    cfg: Res<GameConfig>,
    mut query: Query<(&mut Unit, &mut GeneticTraits, &GeneticTraitBasis)>,
) {
    for (mut unit, mut traits, basis) in query.iter_mut() {
        unit.try_level_up_with(
            &mut traits,
            basis,
            cfg.level_trait_bonus_per_level,
            cfg.max_unit_level,
        );
    }
}
