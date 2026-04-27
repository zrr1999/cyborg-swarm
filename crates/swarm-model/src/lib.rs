pub mod components;
pub mod config;
pub mod genome;
pub mod transform;

pub use components::{
    Base, Evolved, Faction, GeneticTraitBasis, GeneticTraits, Headquarters, MovementSuppression,
    ReadyToEvolve, SpawnedByBase, SteeringInertia, TacticalClass, Unit, UnitFitness, UnitTarget,
    classify, unit_strength,
};
pub use config::{
    BASE_RADIUS, DEFAULT_LEVEL_RADIUS_BONUS_PER_LEVEL, DEFAULT_LEVEL_TRAIT_BONUS_PER_LEVEL,
    GameConfig, GreyWolfOptimizerConfig, MAX_UNIT_LEVEL, ParticleSwarmOptimizerConfig,
    PopulationOptimizerConfig, PopulationOptimizerKind, SteeringMovementConfig, UNIT_RADIUS,
    WORLD_HALF_H, WORLD_HALF_W,
};
pub use genome::Genome;
pub use transform::{
    MergeTransformationOutcome, MergeTransformationRule, SelfTransformationOutcome,
    SelfTransformationRule, SplitTransformationOutcome, SplitTransformationRule,
    TransformationPrereqs, TransformationRuleSet, TransformationState, TransformationTrigger,
    apply_merge_transformation, apply_self_transformation, apply_split_transformation,
};
