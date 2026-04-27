use bevy::prelude::*;
use bevy::window::WindowResizeConstraints;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Cyborg Swarm".into(),
                canvas: Some("#bevy".into()),
                fit_canvas_to_parent: true,
                prevent_default_event_handling: false,
                resolution: bevy::window::WindowResolution::new(1280, 720),
                resize_constraints: WindowResizeConstraints {
                    min_width: 640.0,
                    min_height: 360.0,
                    ..default()
                },
                ..default()
            }),
            ..default()
        }))
        .add_plugins((
            hive_war::plugins::config::ConfigPlugin,
            hive_war::plugins::world::WorldPlugin,
            hive_war::plugins::experience::ExperiencePlugin,
            hive_war::plugins::wolf_pack::WolfPackPlugin,
            hive_war::plugins::genetics::GeneticsPlugin,
            hive_war::plugins::capture::CapturePlugin,
            hive_war::plugins::units::UnitPlugin,
            hive_war::plugins::game_flow::GameFlowPlugin,
            hive_war::plugins::beacons::BeaconPlugin,
            hive_war::plugins::particles::ParticlesPlugin,
            hive_war::plugins::projectiles::ProjectilesPlugin,
            hive_war::plugins::HudPlugin,
        ))
        .run();
}
