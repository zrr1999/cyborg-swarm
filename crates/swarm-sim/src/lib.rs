pub mod captain_dataset;
pub mod record_params;
pub mod simulation;

pub use record_params::SwarmRecordParams;

#[cfg(feature = "sim_bevy")]
pub mod bevy_headless;
