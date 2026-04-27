//! Serializable map layouts: HQ and neutral base positions within the arena.
//!
//! World half-extents always come from [`crate::resources::game_config::GameConfig`] so flow fields
//! and spatial hashing stay consistent with the configured playfield size.

use serde::{Deserialize, Serialize};

/// One row in `assets/maps/*.ron`: which base to spawn and where.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MapBaseRow {
    pub role: MapBaseRole,
    pub x: f32,
    pub y: f32,
    /// Set for [`MapBaseRole::Neutral`] spawns (`Base::neutral` hint).
    #[serde(default)]
    pub hint: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum MapBaseRole {
    Player,
    Enemy,
    Neutral,
}

/// A full map: metadata plus an ordered list of bases.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MapDefinition {
    pub id: String,
    pub name: String,
    pub bases: Vec<MapBaseRow>,
}

impl MapDefinition {
    /// Load from a RON file, or `None` if missing or invalid.
    pub fn load_from_ron(path: &str) -> Option<Self> {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| ron::from_str::<MapDefinition>(&s).ok())
    }

    /// Built-in fallback (matches `assets/maps/classic.ron`).
    pub fn classic_default() -> Self {
        Self {
            id: "classic".into(),
            name: "四象风眼".into(),
            bases: vec![
                MapBaseRow {
                    role: MapBaseRole::Player,
                    x: -380.0,
                    y: 0.0,
                    hint: String::new(),
                },
                MapBaseRow {
                    role: MapBaseRole::Enemy,
                    x: 380.0,
                    y: 0.0,
                    hint: String::new(),
                },
                MapBaseRow {
                    role: MapBaseRole::Neutral,
                    x: -165.0,
                    y: 145.0,
                    hint: "nw".into(),
                },
                MapBaseRow {
                    role: MapBaseRole::Neutral,
                    x: 165.0,
                    y: 145.0,
                    hint: "ne".into(),
                },
                MapBaseRow {
                    role: MapBaseRole::Neutral,
                    x: -165.0,
                    y: -145.0,
                    hint: "sw".into(),
                },
                MapBaseRow {
                    role: MapBaseRole::Neutral,
                    x: 165.0,
                    y: -145.0,
                    hint: "se".into(),
                },
            ],
        }
    }

    /// Validate: exactly one player HQ and one enemy HQ.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let mut players = 0u32;
        let mut enemies = 0u32;
        for b in &self.bases {
            match b.role {
                MapBaseRole::Player => players += 1,
                MapBaseRole::Enemy => enemies += 1,
                MapBaseRole::Neutral => {}
            }
        }
        players == 1 && enemies == 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sample_ron_like_assets() {
        let s = r#"
(
    id: "duel",
    name: "Head-to-head",
    bases: [
        (role: Player, x: -380.0, y: 0.0),
        (role: Enemy, x: 380.0, y: 0.0),
    ],
)
"#;
        let m: MapDefinition = ron::from_str(s).expect("RON parse");
        assert!(m.is_valid());
        assert_eq!(m.bases.len(), 2);
    }
}
