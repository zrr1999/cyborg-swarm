mod plugins;

use bevy::prelude::*;
use bevy::window::WindowResizeConstraints;
use plugins::SquadStrikePlugin;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(bevy::asset::AssetPlugin {
                    file_path: format!("{}/assets", env!("CARGO_MANIFEST_DIR")),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Cyborg Swarm · 赛博蜂群".into(),
                        resolution: (1280u32, 720u32).into(),
                        resize_constraints: WindowResizeConstraints {
                            min_width: 960.0,
                            min_height: 540.0,
                            ..default()
                        },
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(SquadStrikePlugin)
        .run();
}
