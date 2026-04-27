//! Small ONNX captain policy (tract): blends a directional hint into player flow-field steering.
//! Native only — on `wasm32` the hint stays zero.

use bevy::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use swarm_model::Faction;
use swarm_model::{GeneticTraits, Unit};
#[cfg(not(target_arch = "wasm32"))]
use swarm_model::{WORLD_HALF_H, WORLD_HALF_W};
#[cfg(not(target_arch = "wasm32"))]
use swarm_sim::captain_dataset::{CAPTAIN_FEATURE_DIM, CaptainFeatureInputV1};
#[cfg(not(target_arch = "wasm32"))]
use swarm_tactics::{SquadUnitSnapshot, axis_aligned_bbox_diagonal, elect_and_assign};

/// Seconds between ONNX inferences (~10 Hz).
#[cfg(not(target_arch = "wasm32"))]
const CAPTAIN_AI_PERIOD: f32 = 0.1;
/// Blend weight for hint vs sampled flow direction (player only).
pub const CAPTAIN_FLOW_BLEND: f32 = 0.28;

/// Normalized direction blended into player flow in [`super::units::move_units`].
#[derive(Resource, Clone, Default)]
pub struct CaptainAiHint(pub Vec2);

#[derive(Resource, Default)]
pub struct CaptainAiTimer {
    #[cfg(not(target_arch = "wasm32"))]
    accum: f32,
}

pub fn captain_ai_infer(
    time: Res<Time>,
    mut hint: ResMut<CaptainAiHint>,
    mut timer: ResMut<CaptainAiTimer>,
    query: Query<(Entity, &Transform, &Unit, &GeneticTraits)>,
) {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (&time, &mut hint, &mut timer, &query);
        return;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        timer.accum += time.delta_secs();
        if timer.accum < CAPTAIN_AI_PERIOD {
            return;
        }
        timer.accum = 0.0;

        let Some(model) = native::model() else {
            hint.0 = Vec2::ZERO;
            return;
        };

        let mut player: Vec<(Entity, Vec2, &Unit, &GeneticTraits)> = Vec::new();
        let mut enemy_positions: Vec<Vec2> = Vec::new();
        for (e, tf, u, traits) in query.iter() {
            let p = tf.translation.truncate();
            match u.faction {
                Faction::Player => player.push((e, p, u, traits)),
                Faction::Enemy => enemy_positions.push(p),
                Faction::Neutral => {}
            }
        }

        if player.len() < 2 {
            hint.0 = Vec2::ZERO;
            return;
        }

        let mean_sensor: f32 = player
            .iter()
            .map(|(_, _, _, t)| t.sensor_range)
            .sum::<f32>()
            / player.len() as f32;
        let lambda = 0.5_f32;

        let snapshots: Vec<SquadUnitSnapshot> = player
            .iter()
            .enumerate()
            .map(|(i, (_, pos, _, traits))| {
                SquadUnitSnapshot::new(i, *pos, traits.attack, traits.attack_speed)
            })
            .collect();

        let positions: Vec<Vec2> = snapshots.iter().map(|s| s.position).collect();
        let _bbox = axis_aligned_bbox_diagonal(&positions);
        let (_k, _r, captain_indices, squads) = elect_and_assign(&snapshots, mean_sensor, lambda);

        if captain_indices.is_empty() {
            hint.0 = Vec2::ZERO;
            return;
        }

        // First captain only for a cheap global hint (extend per-squad later).
        let cap_idx = captain_indices[0];
        let cap_e = player[cap_idx].0;
        let mut sum = Vec2::ZERO;
        let mut count = 0_u32;
        let mut hp_acc = 0.0_f32;
        for (i, (_, pos, u, _traits)) in player.iter().enumerate() {
            if squads[i] == 0 {
                sum += *pos;
                count += 1;
                hp_acc += u.current_health;
            }
        }
        let squad_center = if count > 0 {
            sum / count as f32
        } else {
            player[cap_idx].1
        };
        let mean_hp = if count > 0 {
            hp_acc / count as f32
        } else {
            50.0
        };

        let enemy_c = if enemy_positions.is_empty() {
            Vec2::new(WORLD_HALF_W * 0.5, 0.0)
        } else {
            enemy_positions.iter().copied().sum::<Vec2>() / enemy_positions.len() as f32
        };
        let dist_blob = squad_center.distance(enemy_c);
        let world_diag = (WORLD_HALF_W * 2.0).hypot(WORLD_HALF_H * 2.0);

        let feat_in = CaptainFeatureInputV1 {
            squad_center,
            squad_alive: count.max(1),
            squad_mean_hp: mean_hp,
            captain_power: snapshots[cap_idx].combat_power,
            nearest_enemy_blob_center: enemy_c,
            nearest_enemy_blob_count: enemy_positions.len().min(64) as u32,
            dist_nearest_enemy_blob: dist_blob,
            game_time_s: 0.0,
            friendly_alive: player.len() as u32,
            enemy_alive: enemy_positions.len().max(1) as u32,
            world_diag_hint: world_diag,
        };

        let features = feat_in.build_features();
        match native::run_model(model, &features) {
            Ok(out) if out.len() >= 2 => {
                let v = Vec2::new(out[0], out[1]);
                hint.0 = v.normalize_or_zero();
            }
            Err(e) => {
                bevy::log::warn!("captain onnx inference failed: {e}");
                hint.0 = Vec2::ZERO;
            }
            _ => hint.0 = Vec2::ZERO,
        }

        let _ = cap_e;
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::sync::OnceLock;

    use tract_onnx::prelude::*;

    use super::CAPTAIN_FEATURE_DIM;

    type CaptainModel =
        RunnableModel<TypedFact, Box<dyn TypedOp>, Graph<TypedFact, Box<dyn TypedOp>>>;

    static MODEL: OnceLock<Option<CaptainModel>> = OnceLock::new();

    pub fn model() -> Option<&'static CaptainModel> {
        MODEL.get_or_init(|| load().ok()).as_ref()
    }

    fn load() -> TractResult<CaptainModel> {
        const BYTES: &[u8] = include_bytes!("../../assets/models/captain_bc.onnx");
        tract_onnx::onnx()
            .model_for_read(&mut std::io::Cursor::new(BYTES))?
            .with_input_fact(0, f32::fact([1, CAPTAIN_FEATURE_DIM as i64]).into())?
            .into_optimized()?
            .into_runnable()
    }

    pub fn run_model(
        model: &CaptainModel,
        features: &[f32; CAPTAIN_FEATURE_DIM],
    ) -> TractResult<Vec<f32>> {
        let input = Tensor::from_shape(&[1, CAPTAIN_FEATURE_DIM], features.as_slice())?;
        let outputs = model.run(tvec!(input.into_tvalue()))?;
        let view = outputs[0].to_array_view::<f32>()?;
        Ok(view.iter().copied().collect())
    }
}
