//! Minimal Bevy `App` (no window) to validate ECS-driven snapshots use the same dataset row format.
//!
//! Enabled with `--features sim_bevy` and `swarm-record --engine bevy`.

use std::fs::File;
use std::io::{BufWriter, Write};

use bevy::prelude::*;
use rand::RngExt;
use rand::SeedableRng;
use rand::rngs::SmallRng;

use crate::captain_dataset::{
    CAPTAIN_SCHEMA_VERSION, CaptainDatasetRow, CaptainFeatureInputV1, CaptainRowMeta,
    placeholder_expert_v1,
};
use crate::record_params::SwarmRecordParams;
use swarm_model::{Faction, GeneticTraits, Unit, WORLD_HALF_H, WORLD_HALF_W};
use swarm_tactics::{SquadUnitSnapshot, axis_aligned_bbox_diagonal, elect_and_assign};

/// Entry from `swarm-record` when built with `sim_bevy`.
pub fn run_headless_record(params: &SwarmRecordParams) -> std::io::Result<()> {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(TransformPlugin);

    let n = params.units.max(4);
    let mut rng = SmallRng::seed_from_u64(params.seed);

    {
        let world = app.world_mut();
        for _ in 0..n {
            let x = rng.random_range(-WORLD_HALF_W..WORLD_HALF_W);
            let y = rng.random_range(-WORLD_HALF_H..WORLD_HALF_H);
            let traits = GeneticTraits {
                max_health: rng.random_range(80.0..120.0),
                attack: rng.random_range(5.0..25.0),
                speed: 60.0,
                sensor_range: params.mean_sensor_range,
                armor: 5.0,
                attack_speed: rng.random_range(0.8..1.5),
            };
            world.spawn((
                Transform::from_xyz(x, y, 0.0),
                traits.clone(),
                Unit::new(&traits, Faction::Player),
            ));
        }
    }

    let out: Box<dyn Write> = if params.out.as_os_str() == "-" {
        Box::new(std::io::stdout())
    } else {
        Box::new(BufWriter::new(File::create(&params.out)?))
    };
    let mut out = BufWriter::new(out);

    let dt = (1.0f32 / params.hz.max(0.001)).min(1.0);
    let world_diag = (WORLD_HALF_W * 2.0).hypot(WORLD_HALF_H * 2.0);

    for tick in 0..params.ticks {
        {
            let world = app.world_mut();
            let mut walk_rng =
                SmallRng::seed_from_u64(params.seed.wrapping_add(tick.wrapping_mul(1_000_003)));
            let mut q = world.query_filtered::<&mut Transform, With<Unit>>();
            for mut tf in q.iter_mut(world) {
                tf.translation.x += walk_rng.random_range(-20.0..20.0) * dt;
                tf.translation.y += walk_rng.random_range(-20.0..20.0) * dt;
                tf.translation.x = tf.translation.x.clamp(-WORLD_HALF_W, WORLD_HALF_W);
                tf.translation.y = tf.translation.y.clamp(-WORLD_HALF_H, WORLD_HALF_H);
            }
        }

        let units: Vec<SquadUnitSnapshot> = {
            let world = app.world_mut();
            let mut q = world.query_filtered::<(Entity, &Transform, &GeneticTraits), With<Unit>>();
            let mut rows: Vec<(Entity, Vec2, f32, f32)> = q
                .iter(world)
                .map(|(e, tf, traits)| {
                    (
                        e,
                        tf.translation.truncate(),
                        traits.attack,
                        traits.attack_speed,
                    )
                })
                .collect();
            rows.sort_by_key(|(e, _, _, _)| *e);
            rows.into_iter()
                .enumerate()
                .map(|(i, (_, pos, atk, aps))| SquadUnitSnapshot::new(i, pos, atk, aps))
                .collect()
        };

        let positions_ref: Vec<Vec2> = units.iter().map(|u| u.position).collect();
        let bbox = axis_aligned_bbox_diagonal(&positions_ref);
        let (k, r, captain_indices, squads) =
            elect_and_assign(&units, params.mean_sensor_range, params.lambda);

        let friendly = n as u32;
        let enemy = 20u32;

        for (captain_slot, &captain_u) in captain_indices.iter().enumerate() {
            let mut sum = Vec2::ZERO;
            let mut count = 0u32;
            let mut hp_acc = 0.0f32;
            for u in 0..n {
                if squads[u] == captain_slot {
                    sum += units[u].position;
                    count += 1;
                    hp_acc += 50.0 + units[u].combat_power;
                }
            }
            let center = if count > 0 {
                sum / count as f32
            } else {
                units[captain_u].position
            };
            let mean_hp = if count > 0 {
                hp_acc / count as f32
            } else {
                50.0
            };

            let blob = Vec2::new(120.0, -40.0);
            let dist_blob = center.distance(blob);

            let feat_in = CaptainFeatureInputV1 {
                squad_center: center,
                squad_alive: count,
                squad_mean_hp: mean_hp,
                captain_power: units[captain_u].combat_power,
                nearest_enemy_blob_center: blob,
                nearest_enemy_blob_count: enemy / 4,
                dist_nearest_enemy_blob: dist_blob,
                game_time_s: tick as f32 * dt,
                friendly_alive: friendly,
                enemy_alive: enemy,
                world_diag_hint: world_diag,
            };
            let labels = placeholder_expert_v1(&feat_in).build_labels();

            let row = CaptainDatasetRow {
                schema_version: CAPTAIN_SCHEMA_VERSION,
                seed: params.seed,
                tick,
                faction_id: 0,
                captain_id: captain_slot as u32,
                features: feat_in.build_features(),
                labels,
                meta: CaptainRowMeta {
                    bbox_diagonal: bbox,
                    unit_count: n as u32,
                    expected_k: k,
                    suppression_r: r,
                    lambda: params.lambda,
                },
            };
            writeln!(out, "{}", serde_json::to_string(&row).map_err(json_io)?)?;
        }
    }

    out.flush()?;
    Ok(())
}

fn json_io(e: serde_json::Error) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, e)
}
