pub mod base;
pub mod unit;

pub use base::{Base, Headquarters};
pub use unit::{
    Evolved, Faction, GeneticTraitBasis, GeneticTraits, MovementSuppression, ReadyToEvolve,
    SpawnedByBase, SteeringInertia, TacticalClass, Unit, UnitFitness, UnitTarget, classify,
    unit_strength,
};
