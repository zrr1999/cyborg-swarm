#![allow(clippy::type_complexity)]

use bevy::prelude::*;
use rand::{RngExt, SeedableRng, rngs::SmallRng};

use swarm_growth::CyborgGenePool;
use swarm_model::{
    Evolved, Faction, GameConfig, GeneticTraitBasis, GeneticTraits, MergeTransformationOutcome,
    MergeTransformationRule, MovementSuppression, ReadyToEvolve, SelfTransformationOutcome,
    SelfTransformationRule, SteeringInertia, TransformationPrereqs, TransformationRuleSet,
    TransformationState, TransformationTrigger, Unit, UnitFitness, UnitTarget,
    apply_merge_transformation, apply_self_transformation, classify,
};

use super::markers::GameplayEntity;
use super::units::{
    CaptainEligibilityCooldown, SquadCaptain, apply_evolution_unit_visual,
    captain_eligibility_clear,
};

pub struct GeneticsPlugin;

impl Plugin for GeneticsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CyborgGenePool::default())
            .insert_resource(hive_war_transformation_rules())
            .insert_resource(FusionRng(SmallRng::from_rng(&mut rand::rng())))
            .add_systems(
                Update,
                (
                    captain_eligibility_clear,
                    mark_max_level_units,
                    evolution_system,
                    fusion_system,
                )
                    .chain()
                    .run_if(super::game_state::match_is_active),
            );
    }
}

#[derive(Resource)]
pub struct FusionRng(pub SmallRng);

fn hive_war_transformation_rules() -> TransformationRuleSet {
    TransformationRuleSet {
        self_rules: vec![SelfTransformationRule {
            id: "hive-war-evolution",
            trigger: TransformationTrigger::MaxLevel,
            prereqs: TransformationPrereqs {
                requires_ready: true,
                max_self_transforms: Some(0),
                ..Default::default()
            },
            outcome: SelfTransformationOutcome::ScaleReset {
                stat_scale: 1.2,
                preserve_fusion_xp_multiplier: true,
            },
        }],
        merge_rules: vec![MergeTransformationRule {
            id: "hive-war-fusion",
            trigger: TransformationTrigger::MaxLevel,
            prereqs: TransformationPrereqs {
                requires_ready: true,
                min_self_transforms: 1,
                ..Default::default()
            },
            min_participants: 2,
            max_participants: 2,
            range_scale: 3.0,
            outcome: MergeTransformationOutcome::RecombineWithinRangeThenMutate {
                mutation_sigma_scale: 0.5,
                spawn_as_fused: true,
                preserve_best_fusion_xp_multiplier: true,
            },
        }],
        split_rules: Vec::new(),
    }
}

fn mark_max_level_units(
    mut commands: Commands,
    cfg: Res<GameConfig>,
    query: Query<(Entity, &Unit), Without<ReadyToEvolve>>,
) {
    for (entity, unit) in query.iter() {
        if unit.is_max_level_with(cfg.max_unit_level) {
            commands.entity(entity).insert(ReadyToEvolve);
        }
    }
}

/// When a max-level unit has `ReadyToEvolve` but has NOT already evolved,
/// classify it by dominant trait, boost base stats by 1.2×, reset to level 1,
/// and assign a visual polygon matching its `TacticalClass`.
#[allow(clippy::type_complexity)]
fn evolution_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    cfg: Res<GameConfig>,
    rules: Res<TransformationRuleSet>,
    query: Query<
        (Entity, &Unit, &GeneticTraits, Option<&TransformationState>),
        (With<ReadyToEvolve>, Without<Evolved>),
    >,
) {
    for (entity, unit, traits, state) in query.iter() {
        let state = state.copied().unwrap_or_default();
        let Some(rule) = rules.matching_self_rule(unit, state, true, cfg.max_unit_level) else {
            continue;
        };
        let class = classify(traits);
        let (evolved_traits, evolved_unit, next_state) =
            apply_self_transformation(rule, unit, traits, state, unit.faction);
        let basis = GeneticTraitBasis(evolved_traits.clone());

        // Update the entity's components
        commands
            .entity(entity)
            .insert(evolved_traits.clone())
            .insert(basis)
            .insert(evolved_unit.clone())
            .insert(class)
            .insert(MovementSuppression::default())
            .insert(SteeringInertia::default())
            .insert(next_state)
            .insert(Evolved)
            .remove::<ReadyToEvolve>()
            .remove::<SquadCaptain>()
            .insert(CaptainEligibilityCooldown);

        apply_evolution_unit_visual(
            &mut commands,
            meshes.as_mut(),
            materials.as_mut(),
            &cfg,
            entity,
            &evolved_unit,
            &evolved_traits,
            class,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn fusion_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut gene_pool: ResMut<CyborgGenePool>,
    cfg: Res<GameConfig>,
    rules: Res<TransformationRuleSet>,
    mut rng: ResMut<FusionRng>,
    mut fusion_events: MessageWriter<super::particles::FusionEvent>,
    query: Query<
        (
            Entity,
            &Transform,
            &Unit,
            &GeneticTraits,
            Option<&TransformationState>,
        ),
        With<ReadyToEvolve>,
    >,
) {
    let Some(rule) = rules.merge_rules.first() else {
        return;
    };

    let candidates: Vec<(
        Entity,
        Vec2,
        Faction,
        GeneticTraits,
        Unit,
        TransformationState,
    )> = query
        .iter()
        .filter_map(|(e, tf, u, t, state)| {
            let state = state.copied().unwrap_or_default();
            rules
                .matching_merge_rule(u, state, true, cfg.max_unit_level)
                .map(|_| {
                    (
                        e,
                        tf.translation.truncate(),
                        u.faction,
                        t.clone(),
                        u.clone(),
                        state,
                    )
                })
        })
        .collect();

    let mut consumed: std::collections::HashSet<Entity> = std::collections::HashSet::new();

    for i in 0..candidates.len() {
        if consumed.contains(&candidates[i].0) {
            continue;
        }
        for j in (i + 1)..candidates.len() {
            if consumed.contains(&candidates[j].0) {
                continue;
            }
            let (e_a, pos_a, ref fac_a, ref traits_a, ref unit_a, state_a) = candidates[i];
            let (e_b, pos_b, ref fac_b, ref traits_b, ref unit_b, state_b) = candidates[j];

            if fac_a != fac_b {
                continue;
            }
            let fusion_range = rule.range_scale
                * (cfg.unit_body_radius(traits_a) + cfg.unit_body_radius(traits_b));
            if (pos_a - pos_b).length() > fusion_range {
                continue;
            }

            let pos = (pos_a + pos_b) * 0.5;
            let faction = *fac_a;

            let sigma = match faction {
                Faction::Player => cfg.player_mutation_sigma,
                Faction::Enemy => cfg.enemy_mutation_sigma,
                Faction::Neutral => continue, // neutral units don't fuse
            };
            let merged_state = TransformationState {
                self_transforms: state_a.self_transforms.max(state_b.self_transforms),
                merge_transforms: state_a.merge_transforms.max(state_b.merge_transforms),
                split_transforms: state_a.split_transforms.max(state_b.split_transforms),
            };
            let (bonus_traits, new_unit, next_state) = apply_merge_transformation(
                rule,
                &[unit_a, unit_b],
                &[traits_a, traits_b],
                faction,
                merged_state,
                sigma,
                &mut rng.0,
            );

            let pool = match faction {
                Faction::Player => &mut gene_pool.player,
                Faction::Enemy => &mut gene_pool.enemy,
                Faction::Neutral => continue,
            };
            pool.record_fusion_loss();
            pool.record_fusion_loss();
            pool.record_birth(&bonus_traits);

            let target_x = match faction {
                Faction::Player => cfg.world_half_w * 0.75,
                Faction::Enemy => -cfg.world_half_w * 0.75,
                Faction::Neutral => 0.0,
            };
            let target = Vec2::new(
                target_x,
                rng.0
                    .random_range(-cfg.world_half_h * 0.5..cfg.world_half_h * 0.5),
            );

            let fused_class = classify(&bonus_traits);
            let basis = GeneticTraitBasis(bonus_traits.clone());
            let traits_ref = bonus_traits.clone();
            let spawned = commands
                .spawn((
                    Transform::from_xyz(pos.x, pos.y, 2.5),
                    Visibility::default(),
                    new_unit.clone(),
                    bonus_traits,
                    basis,
                    UnitFitness::default(),
                    fused_class,
                    UnitTarget { position: target },
                    SteeringInertia::default(),
                    MovementSuppression::default(),
                    next_state,
                    GameplayEntity,
                ))
                .id();
            apply_evolution_unit_visual(
                &mut commands,
                meshes.as_mut(),
                materials.as_mut(),
                &cfg,
                spawned,
                &new_unit,
                &traits_ref,
                fused_class,
            );

            fusion_events.write(super::particles::FusionEvent { position: pos });

            consumed.insert(e_a);
            consumed.insert(e_b);
            commands.entity(e_a).despawn_children().despawn();
            commands.entity(e_b).despawn_children().despawn();
            break;
        }
    }
}
