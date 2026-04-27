use bevy::prelude::{Component, Vec2};
use swarm_model::Faction;

pub use swarm_model::{
    Base, Evolved, GeneticTraitBasis, GeneticTraits, MovementSuppression, ReadyToEvolve,
    SpawnedByBase, SteeringInertia, TacticalClass, UNIT_RADIUS, Unit, UnitFitness, UnitTarget,
    classify, unit_strength,
};

/// Marks a starting-side anchor for labels / presentation.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Headquarters {
    pub seat: Faction,
}

/// Single player-only tactical beacon.
#[derive(Component, Clone, Copy, Debug)]
pub struct Beacon {
    pub radius: f32,
    pub strength: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeaconMode {
    Attract,
    Repel,
    Focus,
}

pub const BEACON_CROWDING_K: f32 = 0.2;

impl Default for Beacon {
    fn default() -> Self {
        Self {
            radius: 160.0,
            strength: 42.0,
        }
    }
}

impl Beacon {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn influence_force(
        &self,
        mode: BeaconMode,
        beacon_pos: Vec2,
        unit_pos: Vec2,
        units_in_radius: u32,
        strength_multiplier: f32,
    ) -> Vec2 {
        let diff = beacon_pos - unit_pos;
        let dist = diff.length();
        if dist > self.radius || dist < 1e-4 {
            return Vec2::ZERO;
        }
        let dir = diff / dist;
        let crowd_scale = 1.0 / (1.0 + BEACON_CROWDING_K * units_in_radius as f32);
        let strength = self.strength * strength_multiplier * crowd_scale;

        match mode {
            BeaconMode::Attract => {
                let preferred = self.radius * 0.45;
                let band =
                    (1.0_f32 - ((dist - preferred).abs() / preferred.max(1e-4))).clamp(0.0, 1.0);
                dir * strength * band * band
            }
            BeaconMode::Repel => {
                let edge = (1.0_f32 - dist / self.radius).clamp(0.0, 1.0);
                -dir * strength * 0.55 * edge * edge
            }
            BeaconMode::Focus => {
                let preferred = self.radius * 0.55;
                let band =
                    (1.0_f32 - ((dist - preferred).abs() / preferred.max(1e-4))).clamp(0.0, 1.0);
                dir * strength * 0.35 * band
            }
        }
    }

    pub fn contains(&self, beacon_pos: Vec2, point: Vec2) -> bool {
        (point - beacon_pos).length_squared() <= self.radius * self.radius
    }
}
