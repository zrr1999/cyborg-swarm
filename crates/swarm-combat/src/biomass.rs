use bevy_ecs::prelude::*;

/// Global biomass currency for both factions.
/// Earned from kills and base ownership, spent to produce units.
#[derive(Resource, Debug, Clone)]
pub struct Biomass {
    pub player: f32,
    pub enemy: f32,
}

impl Default for Biomass {
    fn default() -> Self {
        Self {
            player: 100.0,
            enemy: 100.0,
        }
    }
}
