//! Phase-1 synthetic dataset recorder (no rendering, no full Bevy).
//! With `--features sim_bevy --engine bevy`, runs a minimal ECS world instead.

use std::fs::File;
use std::io::{BufWriter, Write};

use clap::Parser;
use clap::ValueEnum;
use rand::RngExt;
use rand::SeedableRng;
use rand::rngs::SmallRng;

use swarm_model::{WORLD_HALF_H, WORLD_HALF_W};
use swarm_sim::SwarmRecordParams;
use swarm_sim::captain_dataset::{
    CAPTAIN_SCHEMA_VERSION, CaptainDatasetRow, CaptainFeatureInputV1, CaptainRowMeta,
    placeholder_expert_v1,
};
use swarm_tactics::{SquadUnitSnapshot, axis_aligned_bbox_diagonal, elect_and_assign};

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
enum RecordEngine {
    /// Pure Rust random walk + squad formulas (no Bevy `App`).
    #[default]
    Synthetic,
    /// `MinimalPlugins` ECS world (`Transform` + `Unit` + `GeneticTraits`); build with `--features sim_bevy`.
    Bevy,
}

#[derive(Parser, Debug)]
#[command(name = "swarm-record")]
struct Args {
    #[arg(long, default_value_t = 0u64)]
    seed: u64,
    #[arg(long, default_value_t = 100)]
    ticks: u64,
    #[arg(long, default_value = "captain_dataset.jsonl")]
    out: std::path::PathBuf,
    /// Logical record rate (Hz); also advances synthetic motion noise.
    #[arg(long, default_value_t = 10.0)]
    hz: f32,
    #[arg(long, default_value_t = 48)]
    units: usize,
    /// Mean sensor range for K/R heuristics (matches typical `GeneticTraits.sensor_range`).
    #[arg(long, default_value_t = 80.0)]
    mean_sensor_range: f32,
    #[arg(long, default_value_t = 0.5)]
    lambda: f32,
    #[arg(long, value_enum, default_value_t = RecordEngine::Synthetic)]
    engine: RecordEngine,
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();
    let params = SwarmRecordParams {
        seed: args.seed,
        ticks: args.ticks,
        out: args.out.clone(),
        hz: args.hz,
        units: args.units,
        mean_sensor_range: args.mean_sensor_range,
        lambda: args.lambda,
    };

    match args.engine {
        RecordEngine::Synthetic => run_synthetic_record(&params),
        RecordEngine::Bevy => run_bevy_headless(&params),
    }
}

#[cfg(feature = "sim_bevy")]
fn run_bevy_headless(params: &SwarmRecordParams) -> std::io::Result<()> {
    swarm_sim::bevy_headless::run_headless_record(params)
}

#[cfg(not(feature = "sim_bevy"))]
fn run_bevy_headless(_params: &SwarmRecordParams) -> std::io::Result<()> {
    eprintln!(
        "error: --engine bevy requires building swarm-sim with feature `sim_bevy`.\n\
         Example:\n\
           cargo run -p swarm-sim --features sim_bevy --bin swarm-record -- --engine bevy --out out.jsonl"
    );
    std::process::exit(2);
}

fn run_synthetic_record(params: &SwarmRecordParams) -> std::io::Result<()> {
    let out: Box<dyn Write> = if params.out.as_os_str() == "-" {
        Box::new(std::io::stdout())
    } else {
        Box::new(BufWriter::new(File::create(&params.out)?))
    };
    let mut out = BufWriter::new(out);

    let mut rng = SmallRng::seed_from_u64(params.seed);
    let dt = (1.0f32 / params.hz.max(0.001)).min(1.0);

    let n = params.units.max(4);
    let mut positions: Vec<bevy_math::Vec2> = (0..n)
        .map(|_| {
            bevy_math::Vec2::new(
                rng.random_range(-WORLD_HALF_W..WORLD_HALF_W),
                rng.random_range(-WORLD_HALF_H..WORLD_HALF_H),
            )
        })
        .collect();

    let mut velocities: Vec<bevy_math::Vec2> = (0..n)
        .map(|_| {
            bevy_math::Vec2::new(rng.random_range(-1.0..1.0), rng.random_range(-1.0..1.0))
                .normalize_or_zero()
                * 20.0
        })
        .collect();

    let attacks: Vec<f32> = (0..n).map(|_| rng.random_range(5.0f32..25.0)).collect();
    let aps: Vec<f32> = (0..n).map(|_| rng.random_range(0.8f32..1.5)).collect();

    for tick in 0..params.ticks {
        for i in 0..n {
            positions[i] += velocities[i] * dt;
            positions[i].x = positions[i].x.clamp(-WORLD_HALF_W, WORLD_HALF_W);
            positions[i].y = positions[i].y.clamp(-WORLD_HALF_H, WORLD_HALF_H);
            if rng.random_bool(0.05f64) {
                velocities[i] +=
                    bevy_math::Vec2::new(rng.random_range(-1.0..1.0), rng.random_range(-1.0..1.0))
                        * 60.0
                        * dt;
                let m = velocities[i].length();
                if m > 80.0 {
                    velocities[i] *= 80.0 / m;
                }
            }
        }

        let units: Vec<SquadUnitSnapshot> = (0..n)
            .map(|i| SquadUnitSnapshot::new(i, positions[i], attacks[i], aps[i]))
            .collect();

        let positions_ref: Vec<bevy_math::Vec2> = units.iter().map(|u| u.position).collect();
        let bbox = axis_aligned_bbox_diagonal(&positions_ref);
        let (k, r, captain_indices, squads) =
            elect_and_assign(&units, params.mean_sensor_range, params.lambda);

        let world_diag = (WORLD_HALF_W * 2.0).hypot(WORLD_HALF_H * 2.0);
        let friendly = n as u32;
        let enemy = rng.random_range(8u32..40u32);

        for (captain_slot, &captain_u) in captain_indices.iter().enumerate() {
            let mut sum = bevy_math::Vec2::ZERO;
            let mut count = 0u32;
            let mut hp_acc = 0.0f32;
            for u in 0..n {
                if squads[u] == captain_slot {
                    sum += positions[u];
                    count += 1;
                    hp_acc += 50.0 + attacks[u] * 2.0;
                }
            }
            let center = if count > 0 {
                sum / count as f32
            } else {
                positions[captain_u]
            };
            let mean_hp = if count > 0 {
                hp_acc / count as f32
            } else {
                50.0
            };

            let blob = bevy_math::Vec2::new(
                rng.random_range(-WORLD_HALF_W * 0.5..WORLD_HALF_W * 0.5),
                rng.random_range(-WORLD_HALF_H * 0.5..WORLD_HALF_H * 0.5),
            );
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
            writeln!(out, "{}", serde_json::to_string(&row).map_err(to_io)?)?;
        }
    }

    out.flush()?;
    Ok(())
}

fn to_io(e: serde_json::Error) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, e)
}
