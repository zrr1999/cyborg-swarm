use bevy::prelude::*;
use swarm_model::GameConfig;

use crate::map::{ActiveMap, load_map_for_id};

/// Plugin that loads `GameConfig` from `assets/config.ron` at startup,
/// falling back to compiled defaults when the file is missing or malformed.
///
/// Resources are inserted in [`Plugin::build`] so they exist before any `Startup` system runs
/// (e.g. world setup that needs [`ActiveMap`]).
pub struct ConfigPlugin;

impl Plugin for ConfigPlugin {
    fn build(&self, app: &mut App) {
        let config_path = format!("{}/assets/config.ron", env!("CARGO_MANIFEST_DIR"));
        let config = GameConfig::load_from_ron(&config_path);
        let map = load_map_for_id(&config.map_id);
        info!("Loaded GameConfig (map_id={}): {:?}", config.map_id, config);
        info!("Active map: {} — {}", map.name, map.id);
        app.insert_resource(config);
        app.insert_resource(ActiveMap(map));
    }
}
