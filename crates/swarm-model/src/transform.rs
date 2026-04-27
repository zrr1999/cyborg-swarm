use bevy_ecs::prelude::{Component, Resource};
use rand::{Rng, RngExt};

use crate::components::{Faction, GeneticTraits, Unit};

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct TransformationState {
    pub self_transforms: u32,
    pub merge_transforms: u32,
    pub split_transforms: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TransformationPrereqs {
    pub requires_ready: bool,
    pub min_self_transforms: u32,
    pub max_self_transforms: Option<u32>,
    pub min_merge_transforms: u32,
    pub max_merge_transforms: Option<u32>,
    pub min_split_transforms: u32,
    pub max_split_transforms: Option<u32>,
}

impl TransformationPrereqs {
    pub fn matches(&self, state: TransformationState, ready: bool) -> bool {
        if self.requires_ready && !ready {
            return false;
        }
        state.self_transforms >= self.min_self_transforms
            && self
                .max_self_transforms
                .is_none_or(|max| state.self_transforms <= max)
            && state.merge_transforms >= self.min_merge_transforms
            && self
                .max_merge_transforms
                .is_none_or(|max| state.merge_transforms <= max)
            && state.split_transforms >= self.min_split_transforms
            && self
                .max_split_transforms
                .is_none_or(|max| state.split_transforms <= max)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransformationTrigger {
    MaxLevel,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct TransformationRuleSet {
    pub self_rules: Vec<SelfTransformationRule>,
    pub merge_rules: Vec<MergeTransformationRule>,
    pub split_rules: Vec<SplitTransformationRule>,
}

impl TransformationRuleSet {
    pub fn matching_self_rule<'a>(
        &'a self,
        unit: &Unit,
        state: TransformationState,
        ready: bool,
        max_unit_level: u32,
    ) -> Option<&'a SelfTransformationRule> {
        self.self_rules.iter().find(|rule| {
            rule.trigger == TransformationTrigger::MaxLevel
                && unit.is_max_level_with(max_unit_level)
                && rule.prereqs.matches(state, ready)
        })
    }

    pub fn matching_merge_rule<'a>(
        &'a self,
        unit: &Unit,
        state: TransformationState,
        ready: bool,
        max_unit_level: u32,
    ) -> Option<&'a MergeTransformationRule> {
        self.merge_rules.iter().find(|rule| {
            rule.trigger == TransformationTrigger::MaxLevel
                && unit.is_max_level_with(max_unit_level)
                && rule.prereqs.matches(state, ready)
        })
    }

    pub fn matching_split_rule<'a>(
        &'a self,
        unit: &Unit,
        state: TransformationState,
        ready: bool,
        max_unit_level: u32,
    ) -> Option<&'a SplitTransformationRule> {
        self.split_rules.iter().find(|rule| {
            rule.trigger == TransformationTrigger::MaxLevel
                && unit.is_max_level_with(max_unit_level)
                && rule.prereqs.matches(state, ready)
        })
    }
}

#[derive(Clone, Debug)]
pub struct SelfTransformationRule {
    pub id: &'static str,
    pub trigger: TransformationTrigger,
    pub prereqs: TransformationPrereqs,
    pub outcome: SelfTransformationOutcome,
}

#[derive(Clone, Copy, Debug)]
pub enum SelfTransformationOutcome {
    ScaleReset {
        stat_scale: f32,
        preserve_fusion_xp_multiplier: bool,
    },
}

#[derive(Clone, Debug)]
pub struct MergeTransformationRule {
    pub id: &'static str,
    pub trigger: TransformationTrigger,
    pub prereqs: TransformationPrereqs,
    pub min_participants: usize,
    pub max_participants: usize,
    pub range_scale: f32,
    pub outcome: MergeTransformationOutcome,
}

#[derive(Clone, Copy, Debug)]
pub enum MergeTransformationOutcome {
    RecombineWithinRangeThenMutate {
        mutation_sigma_scale: f32,
        spawn_as_fused: bool,
        preserve_best_fusion_xp_multiplier: bool,
    },
}

#[derive(Clone, Debug)]
pub struct SplitTransformationRule {
    pub id: &'static str,
    pub trigger: TransformationTrigger,
    pub prereqs: TransformationPrereqs,
    pub child_count: usize,
    pub spread_distance_scale: f32,
    pub outcome: SplitTransformationOutcome,
}

#[derive(Clone, Copy, Debug)]
pub enum SplitTransformationOutcome {
    DuplicateScaled {
        child_stat_scale: f32,
        preserve_fusion_xp_multiplier: bool,
    },
}

pub fn apply_self_transformation(
    rule: &SelfTransformationRule,
    unit: &Unit,
    traits: &GeneticTraits,
    state: TransformationState,
    faction: Faction,
) -> (GeneticTraits, Unit, TransformationState) {
    match rule.outcome {
        SelfTransformationOutcome::ScaleReset {
            stat_scale,
            preserve_fusion_xp_multiplier,
        } => {
            let new_traits = traits.scale_by(stat_scale);
            let mut new_unit = Unit::new(&new_traits, faction);
            if preserve_fusion_xp_multiplier {
                new_unit.fusion_xp_multiplier = unit.fusion_xp_multiplier;
            }
            let mut new_state = state;
            new_state.self_transforms += 1;
            (new_traits, new_unit, new_state)
        }
    }
}

pub fn apply_merge_transformation(
    rule: &MergeTransformationRule,
    units: &[&Unit],
    traits: &[&GeneticTraits],
    faction: Faction,
    state: TransformationState,
    mutation_sigma: f32,
    rng: &mut impl Rng,
) -> (GeneticTraits, Unit, TransformationState) {
    let blended = recombine_trait_ranges(traits, rng);
    match rule.outcome {
        MergeTransformationOutcome::RecombineWithinRangeThenMutate {
            mutation_sigma_scale,
            spawn_as_fused,
            preserve_best_fusion_xp_multiplier,
        } => {
            let new_traits =
                apply_bonus_mutation(blended, mutation_sigma * mutation_sigma_scale, rng);
            let mut new_unit = if spawn_as_fused {
                Unit::new_fused(&new_traits, faction)
            } else {
                Unit::new(&new_traits, faction)
            };
            if preserve_best_fusion_xp_multiplier {
                new_unit.fusion_xp_multiplier = units
                    .iter()
                    .map(|unit| unit.fusion_xp_multiplier)
                    .fold(new_unit.fusion_xp_multiplier, f32::max);
            }
            let mut new_state = state;
            new_state.merge_transforms += 1;
            (new_traits, new_unit, new_state)
        }
    }
}

pub fn apply_split_transformation(
    rule: &SplitTransformationRule,
    unit: &Unit,
    traits: &GeneticTraits,
    faction: Faction,
    state: TransformationState,
) -> Vec<(GeneticTraits, Unit, TransformationState)> {
    match rule.outcome {
        SplitTransformationOutcome::DuplicateScaled {
            child_stat_scale,
            preserve_fusion_xp_multiplier,
        } => (0..rule.child_count.max(1))
            .map(|_| {
                let new_traits = traits.scale_by(child_stat_scale);
                let mut new_unit = Unit::new(&new_traits, faction);
                if preserve_fusion_xp_multiplier {
                    new_unit.fusion_xp_multiplier = unit.fusion_xp_multiplier;
                }
                let mut new_state = state;
                new_state.split_transforms += 1;
                (new_traits, new_unit, new_state)
            })
            .collect(),
    }
}

fn recombine_trait_ranges(traits: &[&GeneticTraits], rng: &mut impl Rng) -> GeneticTraits {
    debug_assert!(!traits.is_empty());
    let bounds = |selector: fn(&GeneticTraits) -> f32| {
        traits.iter().fold(
            (f32::INFINITY, f32::NEG_INFINITY),
            |(min_v, max_v), traits| {
                let value = selector(traits);
                (min_v.min(value), max_v.max(value))
            },
        )
    };

    let (min_health, max_health) = bounds(|traits| traits.max_health);
    let (min_attack, max_attack) = bounds(|traits| traits.attack);
    let (min_speed, max_speed) = bounds(|traits| traits.speed);
    let (min_sensor, max_sensor) = bounds(|traits| traits.sensor_range);
    let (min_armor, max_armor) = bounds(|traits| traits.armor);
    let (min_aps, max_aps) = bounds(|traits| traits.attack_speed);

    GeneticTraits {
        max_health: rng.random_range(min_health..=max_health),
        attack: rng.random_range(min_attack..=max_attack),
        speed: rng.random_range(min_speed..=max_speed),
        sensor_range: rng.random_range(min_sensor..=max_sensor),
        armor: rng.random_range(min_armor..=max_armor),
        attack_speed: rng.random_range(min_aps..=max_aps),
    }
}

fn apply_bonus_mutation(base: GeneticTraits, sigma: f32, rng: &mut impl Rng) -> GeneticTraits {
    let sigma = sigma.abs();
    let hi = sigma.max(0.0001);
    GeneticTraits {
        max_health: base.max_health * (1.0 + sigma + rng.random_range(0.0..hi)).max(1.0),
        attack: base.attack * (1.0 + sigma + rng.random_range(0.0..hi)).max(1.0),
        speed: (base.speed * (1.0 + rng.random_range(-sigma..sigma.max(0.0001))))
            .clamp(15.0, 180.0),
        sensor_range: base.sensor_range * (1.0_f32 + rng.random_range(0.0..(hi * 0.5))).max(1.0),
        armor: base.armor * (1.0 + sigma + rng.random_range(0.0..hi)).max(0.0),
        attack_speed: base.attack_speed * (1.0_f32 + rng.random_range(0.0..(hi * 0.5))).max(1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_rule_scales_children() {
        let traits = GeneticTraits::default();
        let unit = Unit::new(&traits, Faction::Player);
        let rule = SplitTransformationRule {
            id: "split",
            trigger: TransformationTrigger::MaxLevel,
            prereqs: TransformationPrereqs::default(),
            child_count: 2,
            spread_distance_scale: 1.0,
            outcome: SplitTransformationOutcome::DuplicateScaled {
                child_stat_scale: 0.5,
                preserve_fusion_xp_multiplier: false,
            },
        };

        let children = apply_split_transformation(
            &rule,
            &unit,
            &traits,
            Faction::Player,
            TransformationState::default(),
        );

        assert_eq!(children.len(), 2);
        assert!((children[0].0.max_health - traits.max_health * 0.5).abs() < 0.001);
        assert_eq!(children[0].2.split_transforms, 1);
    }
}
