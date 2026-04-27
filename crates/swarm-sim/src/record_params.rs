//! CLI / binary parameters shared by synthetic and optional Bevy headless recorders.

use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct SwarmRecordParams {
    pub seed: u64,
    pub ticks: u64,
    pub out: PathBuf,
    pub hz: f32,
    pub units: usize,
    pub mean_sensor_range: f32,
    pub lambda: f32,
}
