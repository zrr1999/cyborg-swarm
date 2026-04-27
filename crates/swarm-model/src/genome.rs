use bevy_ecs::prelude::*;

/// Trait for any set of heritable traits that can participate in the evolutionary simulation.
pub trait Genome: Component + Clone + Send + Sync + 'static {
    fn baseline() -> Self;
    fn blend(&self, other: &Self, w_self: f32, w_other: f32) -> Self;
    fn scale(&self, factor: f32) -> Self;
    fn welford_update(&mut self, x: &Self, n: f32);
    fn mutate(&self, rng: &mut impl rand::Rng, sigma: f32) -> Self;
    fn dimension_count() -> usize;
    fn as_slice(&self) -> Vec<f32>;
    fn from_slice(values: &[f32]) -> Self;
}
