use bevy_ecs::prelude::Resource;
use bevy_math::Vec2;

/// Per-frame signals for “friendly base under capture pressure” — used to pull defenders home.
#[derive(Resource, Clone, Debug)]
pub struct BaseDefenseSignals {
    /// World position of the player-owned base taking the most capture damage this frame.
    pub player_defend_pos: Option<Vec2>,
    /// 0..1 — scales steering pull toward [`Self::player_defend_pos`].
    pub player_urgency: f32,
    pub enemy_defend_pos: Option<Vec2>,
    pub enemy_urgency: f32,
}

impl Default for BaseDefenseSignals {
    fn default() -> Self {
        Self {
            player_defend_pos: None,
            player_urgency: 0.0,
            enemy_defend_pos: None,
            enemy_urgency: 0.0,
        }
    }
}

impl BaseDefenseSignals {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}
