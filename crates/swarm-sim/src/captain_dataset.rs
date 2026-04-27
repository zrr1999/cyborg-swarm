//! Versioned feature vectors and labels for captain imitation-learning datasets.

use bevy_math::Vec2;
use serde::{Deserialize, Serialize};

/// Bump when feature layout or semantics change (BC / ONNX contract).
pub const CAPTAIN_SCHEMA_VERSION: u32 = 1;

/// Model input width (MLP placeholder; unused slots are zero).
pub const CAPTAIN_FEATURE_DIM: usize = 60;

/// `dx, dy`, six formation logits or one-hot placeholder, aggression.
pub const CAPTAIN_LABEL_DIM: usize = 10;

/// One JSONL row.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaptainDatasetRow {
    pub schema_version: u32,
    pub seed: u64,
    pub tick: u64,
    /// 0 = player, 1 = enemy (extend as needed).
    pub faction_id: u8,
    /// Stable index within the run (squad slot or captain unit index).
    pub captain_id: u32,
    #[serde(with = "serde_features")]
    pub features: [f32; CAPTAIN_FEATURE_DIM],
    #[serde(with = "serde_labels")]
    pub labels: [f32; CAPTAIN_LABEL_DIM],
    pub meta: CaptainRowMeta,
}

mod serde_features {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::CAPTAIN_FEATURE_DIM;

    pub fn serialize<S>(arr: &[f32; CAPTAIN_FEATURE_DIM], s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        arr.as_slice().serialize(s)
    }

    pub fn deserialize<'de, D>(d: D) -> Result<[f32; CAPTAIN_FEATURE_DIM], D::Error>
    where
        D: Deserializer<'de>,
    {
        let v: Vec<f32> = Vec::deserialize(d)?;
        if v.len() != CAPTAIN_FEATURE_DIM {
            return Err(serde::de::Error::custom("features length mismatch"));
        }
        let mut out = [0f32; CAPTAIN_FEATURE_DIM];
        out.copy_from_slice(&v);
        Ok(out)
    }
}

mod serde_labels {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::CAPTAIN_LABEL_DIM;

    pub fn serialize<S>(arr: &[f32; CAPTAIN_LABEL_DIM], s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        arr.as_slice().serialize(s)
    }

    pub fn deserialize<'de, D>(d: D) -> Result<[f32; CAPTAIN_LABEL_DIM], D::Error>
    where
        D: Deserializer<'de>,
    {
        let v: Vec<f32> = Vec::deserialize(d)?;
        if v.len() != CAPTAIN_LABEL_DIM {
            return Err(serde::de::Error::custom("labels length mismatch"));
        }
        let mut out = [0f32; CAPTAIN_LABEL_DIM];
        out.copy_from_slice(&v);
        Ok(out)
    }
}

/// Extra diagnostics for analysis (not fed to the model unless copied into features).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaptainRowMeta {
    pub bbox_diagonal: f32,
    pub unit_count: u32,
    pub expected_k: u32,
    pub suppression_r: f32,
    pub lambda: f32,
}

/// Abstract inputs for schema v1 feature packing.
#[derive(Clone, Debug)]
pub struct CaptainFeatureInputV1 {
    pub squad_center: Vec2,
    pub squad_alive: u32,
    pub squad_mean_hp: f32,
    pub captain_power: f32,
    pub nearest_enemy_blob_center: Vec2,
    pub nearest_enemy_blob_count: u32,
    pub dist_nearest_enemy_blob: f32,
    pub game_time_s: f32,
    pub friendly_alive: u32,
    pub enemy_alive: u32,
    pub world_diag_hint: f32,
}

impl CaptainFeatureInputV1 {
    /// Pack into `features` (zeros past v1 layout).
    pub fn build_features(&self) -> [f32; CAPTAIN_FEATURE_DIM] {
        let mut f = [0f32; CAPTAIN_FEATURE_DIM];
        let mut i = 0usize;
        f[i] = self.squad_center.x;
        i += 1;
        f[i] = self.squad_center.y;
        i += 1;
        f[i] = self.squad_alive as f32;
        i += 1;
        f[i] = self.squad_mean_hp;
        i += 1;
        f[i] = self.captain_power;
        i += 1;
        f[i] = self.nearest_enemy_blob_center.x;
        i += 1;
        f[i] = self.nearest_enemy_blob_center.y;
        i += 1;
        f[i] = self.nearest_enemy_blob_count as f32;
        i += 1;
        f[i] = self.dist_nearest_enemy_blob;
        i += 1;
        f[i] = self.game_time_s;
        i += 1;
        f[i] = self.friendly_alive as f32;
        i += 1;
        f[i] = self.enemy_alive as f32;
        i += 1;
        f[i] = self.world_diag_hint;
        i += 1;
        debug_assert!(i <= CAPTAIN_FEATURE_DIM);
        f
    }
}

/// Expert / placeholder action for BC (schema v1).
#[derive(Clone, Debug)]
pub struct CaptainLabelInputV1 {
    /// Target move direction scale in world units / s (placeholder).
    pub move_dx: f32,
    pub move_dy: f32,
    /// Six formation channel values (e.g. one-hot or logits).
    pub formation: [f32; 6],
    pub aggression: f32,
}

impl CaptainLabelInputV1 {
    pub fn build_labels(&self) -> [f32; CAPTAIN_LABEL_DIM] {
        let mut l = [0f32; CAPTAIN_LABEL_DIM];
        l[0] = self.move_dx;
        l[1] = self.move_dy;
        l[2..8].copy_from_slice(&self.formation);
        l[8] = self.aggression;
        l
    }
}

/// Heuristic “expert” for Phase 1: move toward enemy blob center, neutral formation, mid aggression.
pub fn placeholder_expert_v1(input: &CaptainFeatureInputV1) -> CaptainLabelInputV1 {
    let to = input.nearest_enemy_blob_center - input.squad_center;
    let n = to.length();
    let dir = if n > 1e-4 { to / n } else { Vec2::Y };
    CaptainLabelInputV1 {
        move_dx: dir.x * 40.0,
        move_dy: dir.y * 40.0,
        formation: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        aggression: 0.5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_layout_fits() {
        let inp = CaptainFeatureInputV1 {
            squad_center: Vec2::new(1.0, 2.0),
            squad_alive: 5,
            squad_mean_hp: 50.0,
            captain_power: 12.0,
            nearest_enemy_blob_center: Vec2::new(10.0, 0.0),
            nearest_enemy_blob_count: 3,
            dist_nearest_enemy_blob: 9.0,
            game_time_s: 1.0,
            friendly_alive: 20,
            enemy_alive: 18,
            world_diag_hint: 100.0,
        };
        let f = inp.build_features();
        assert_eq!(f[0], 1.0);
        assert_eq!(f[12], 100.0);
        let lab = placeholder_expert_v1(&inp).build_labels();
        assert_eq!(lab.len(), CAPTAIN_LABEL_DIM);
    }
}
