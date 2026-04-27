#![allow(clippy::type_complexity)]

use bevy::prelude::*;
use swarm_model::{
    Faction, GameConfig, GeneticTraitBasis, GeneticTraits, MovementSuppression, ReadyToEvolve,
    SplitTransformationOutcome, SplitTransformationRule, SteeringInertia, TransformationPrereqs,
    TransformationRuleSet, TransformationState, TransformationTrigger, Unit, UnitFitness,
    apply_split_transformation,
};
use swarm_sim::simulation::level_up_system;

use crate::plugins::{
    combat::AttackCooldown,
    effects::PromotionFxEvent,
    game_flow::GameplayEntity,
    meta_progression::MetaProgression,
    squad::{
        CaptainAnnouncement, HitFlash, PromotionFlash, SquadCaptain, SquadFollower, SquadMember,
        attach_unit_visuals,
    },
};

pub struct EvolutionPlugin;

#[derive(Resource, Default, Clone, Copy)]
pub struct EvolutionState {
    pub captain_level: u32,
    pub ready_units: usize,
    pub split_units: usize,
}

impl Plugin for EvolutionPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(squad_strike_transformation_rules())
            .init_resource::<EvolutionState>()
            .add_systems(
                Update,
                (
                    accumulate_survival_fitness,
                    level_up_system,
                    mark_max_level_units,
                    split_system,
                    sync_evolution_state,
                )
                    .chain(),
            );
    }
}

fn squad_strike_transformation_rules() -> TransformationRuleSet {
    TransformationRuleSet {
        self_rules: Vec::new(),
        merge_rules: Vec::new(),
        split_rules: vec![SplitTransformationRule {
            id: "squad-strike-max-level-split",
            trigger: TransformationTrigger::MaxLevel,
            prereqs: TransformationPrereqs {
                requires_ready: true,
                ..Default::default()
            },
            child_count: 2,
            spread_distance_scale: 1.3,
            outcome: SplitTransformationOutcome::DuplicateScaled {
                child_stat_scale: 0.5,
                preserve_fusion_xp_multiplier: false,
            },
        }],
    }
}

fn accumulate_survival_fitness(time: Res<Time>, mut query: Query<&mut UnitFitness, With<Unit>>) {
    let dt = time.delta_secs();
    for mut fitness in &mut query {
        fitness.survival_time += dt;
    }
}

fn mark_max_level_units(
    mut commands: Commands,
    cfg: Res<GameConfig>,
    query: Query<(Entity, &Unit), (With<SquadMember>, Without<ReadyToEvolve>)>,
) {
    for (entity, unit) in &query {
        if unit.is_max_level_with(cfg.max_unit_level) {
            commands.entity(entity).insert(ReadyToEvolve);
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn split_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    cfg: Res<GameConfig>,
    rules: Res<TransformationRuleSet>,
    meta: Res<MetaProgression>,
    mut fx: MessageWriter<PromotionFxEvent>,
    mut announcement: ResMut<CaptainAnnouncement>,
    query: Query<
        (
            Entity,
            &Transform,
            &Unit,
            &GeneticTraits,
            Option<&SquadCaptain>,
            Option<&SquadFollower>,
            Option<&TransformationState>,
        ),
        (With<SquadMember>, With<ReadyToEvolve>),
    >,
) {
    if rules.split_rules.is_empty() {
        return;
    }

    let mut captain_split = false;

    for (entity, tf, unit, traits, captain, follower, state) in &query {
        let state = state.copied().unwrap_or_default();
        let Some(matched_rule) = rules.matching_split_rule(unit, state, true, cfg.max_unit_level)
        else {
            continue;
        };
        let mut effective_rule = matched_rule.clone();
        let SplitTransformationOutcome::DuplicateScaled {
            ref mut child_stat_scale,
            ..
        } = effective_rule.outcome;
        *child_stat_scale = meta.split_child_stat_scale();

        let children =
            apply_split_transformation(&effective_rule, unit, traits, unit.faction, state);
        let center = tf.translation.truncate();
        let split_distance = cfg.unit_body_radius(traits) * effective_rule.spread_distance_scale;

        for (child_index, (child_traits, mut child_unit, child_state)) in
            children.into_iter().enumerate()
        {
            let offset_sign = if child_index % 2 == 0 { -1.0 } else { 1.0 };
            let offset = Vec2::new(offset_sign * split_distance, 0.0);
            child_unit.current_health = child_traits.max_health;
            child_unit.xp =
                child_unit.xp_to_next * meta.split_xp_inheritance_fraction().clamp(0.0, 0.9);
            let basis = GeneticTraitBasis(child_traits.clone());
            let spawned = commands
                .spawn((
                    Transform::from_xyz(center.x + offset.x, center.y + offset.y, 1.0),
                    Visibility::default(),
                    GameplayEntity,
                    SquadMember,
                    child_unit,
                    child_traits.clone(),
                    basis,
                    child_state,
                    AttackCooldown(0.0),
                    SteeringInertia::default(),
                    MovementSuppression::default(),
                    UnitFitness::default(),
                    HitFlash::default(),
                    PromotionFlash(1.0),
                ))
                .id();

            let is_primary = child_index == 0;
            if captain.is_some() && is_primary {
                captain_split = true;
                commands.entity(spawned).insert(SquadCaptain);
            } else {
                let slot_index = if let Some(follower) = follower {
                    follower.slot_index.saturating_add(child_index)
                } else if captain.is_some() && !is_primary {
                    0
                } else {
                    child_index
                };
                commands
                    .entity(spawned)
                    .insert(SquadFollower { slot_index });
            }

            attach_unit_visuals(
                &mut commands,
                meshes.as_mut(),
                materials.as_mut(),
                spawned,
                Faction::Player,
                &child_traits,
                &cfg,
                captain.is_some() && is_primary,
            );
            fx.write(PromotionFxEvent {
                position: center + offset,
            });
        }

        commands.entity(entity).despawn_children().despawn();
    }

    if captain_split {
        announcement.show("Captain split");
    }
}

fn sync_evolution_state(
    mut state: ResMut<EvolutionState>,
    players: Query<
        (
            &Unit,
            Option<&SquadCaptain>,
            Option<&ReadyToEvolve>,
            Option<&TransformationState>,
        ),
        With<SquadMember>,
    >,
) {
    let mut captain_level = 0;
    let mut highest_level = 0;
    let mut ready_units = 0;
    let mut split_units = 0;

    for (unit, captain, ready, transform_state) in &players {
        if unit.is_dead() {
            continue;
        }
        if ready.is_some() {
            ready_units += 1;
        }
        if transform_state
            .copied()
            .unwrap_or_default()
            .split_transforms
            > 0
        {
            split_units += 1;
        }
        if captain.is_some() {
            captain_level = unit.level;
        }
        if unit.level > highest_level {
            highest_level = unit.level;
        }
    }

    if captain_level == 0 {
        captain_level = highest_level;
    }

    *state = EvolutionState {
        captain_level,
        ready_units,
        split_units,
    };
}
