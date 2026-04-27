use bevy_ecs::prelude::*;

use swarm_model::{Faction, GameConfig};

/// How many extra faction-wide unit slots each global expansion tier grants.
pub const SLOTS_PER_ARMY_UPGRADE: u32 = 20;

/// Biomass tiers spent on raising faction-wide fielded-unit caps (player / enemy).
#[derive(Resource, Debug, Clone, Default)]
pub struct ArmyCapacityUpgrades {
    pub player_tier: u32,
    pub enemy_tier: u32,
}

impl ArmyCapacityUpgrades {
    pub fn tier(&self, faction: &Faction) -> u32 {
        match faction {
            Faction::Player => self.player_tier,
            Faction::Enemy => self.enemy_tier,
            Faction::Neutral => 0,
        }
    }

    pub fn faction_unit_cap(&self, faction: &Faction, cfg: &GameConfig) -> u32 {
        cfg.global_unit_cap_for_tier(self.tier(faction))
    }
}

/// Cost for the next upgrade at `current_tier` (0 = first purchase).
#[inline]
pub fn army_upgrade_cost(current_tier: u32, cfg: &GameConfig) -> f32 {
    cfg.global_expansion_cost(current_tier)
}
