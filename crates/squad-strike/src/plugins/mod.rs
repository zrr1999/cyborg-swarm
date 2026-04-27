pub mod art;
pub mod combat;
pub mod effects;
pub mod enemies;
pub mod evolution;
pub mod game_flow;
pub mod hud;
pub mod meta_progression;
pub mod skills;
pub mod squad;
pub mod world;

use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use swarm_model::GameConfig;

pub struct SquadStrikePlugin;

impl Plugin for SquadStrikePlugin {
    fn build(&self, app: &mut App) {
        let cfg = GameConfig {
            max_unit_level: 5,
            world_half_w: world::MAP_HALF_W,
            world_half_h: world::MAP_HALF_H,
            steering: swarm_model::SteeringMovementConfig {
                inertia_blend_per_sec: 7.0,
                inertia_zero_decay_per_sec: 5.0,
                ..Default::default()
            },
            ..Default::default()
        };

        app.insert_resource(cfg)
            .add_plugins(EguiPlugin::default())
            .add_plugins((
                swarm_growth::WolfPackPlugin,
                effects::EffectsPlugin,
                meta_progression::MetaProgressionPlugin,
                skills::SkillsPlugin,
                world::WorldPlugin,
                game_flow::GameFlowPlugin,
                squad::SquadPlugin,
                enemies::EnemiesPlugin,
                combat::CombatPlugin,
                evolution::EvolutionPlugin,
                hud::HudPlugin,
            ));
    }
}
