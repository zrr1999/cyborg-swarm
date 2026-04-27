//! Active map selection and loading from `assets/maps/{id}.ron` (relative to this crate).

use bevy::prelude::*;
use swarm_space::MapDefinition;

fn map_ron_path(map_id: &str) -> String {
    format!("{}/assets/maps/{map_id}.ron", env!("CARGO_MANIFEST_DIR"))
}

/// Currently loaded [`MapDefinition`] (bases + display name). Cloned on restart.
#[derive(Resource, Clone)]
pub struct ActiveMap(pub MapDefinition);

/// Built-in map ids and short labels for the HUD selector (ids match `assets/maps/*.ron`).
pub const BUILTIN_MAPS: &[(&str, &str)] = &[
    ("classic", "四象风眼 · 四角中立"),
    ("duel", "南北天堑 · 纵轴单挑"),
    ("skirmish", "碎岛链 · 斜向散落"),
    ("crossroads", "十字要塞 · 五堡路口"),
    ("cluster", "蜂巢环带 · 六边形环绕"),
];

pub fn load_map_for_id(map_id: &str) -> MapDefinition {
    let path = map_ron_path(map_id);
    match MapDefinition::load_from_ron(&path) {
        Some(m) if m.is_valid() => m,
        Some(_) => {
            warn!(
                "Map file {:?} failed validation (need 1 Player + 1 Enemy HQ); using classic",
                path
            );
            MapDefinition::classic_default()
        }
        None => {
            warn!("Could not load {:?}; using built-in classic", path);
            MapDefinition::classic_default()
        }
    }
}
