use bevy_ecs::prelude::*;
use rand::Rng;

use swarm_model::{GameConfig, GeneticTraits, Genome};

/// Per-faction gene pool tracking live and dead unit trait distributions.
///
/// Birth formula:
///   μ_base = α · μ_live + (1−α) · μ_dead
///   A_new  = μ_base · (1 + ε),  ε ~ Uniform(−σ, +σ)
///
/// Dead mean uses a capped Welford update with window N:
///   n_eff = min(dead_count, N)
///   μ_dead += (x − μ_dead) / n_eff
#[derive(Resource)]
pub struct GenePool<G: Genome> {
    pub player: FactionPool<G>,
    pub enemy: FactionPool<G>,
}

impl<G: Genome> Default for GenePool<G> {
    fn default() -> Self {
        Self {
            player: FactionPool::default(),
            enemy: FactionPool::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FactionPool<G: Genome> {
    pub live_count: u32,
    pub live_mean: G,
    pub dead_count: u32,
    pub dead_mean: G,
}

impl<G: Genome> Default for FactionPool<G> {
    fn default() -> Self {
        Self {
            live_count: 0,
            live_mean: G::baseline(),
            dead_count: 0,
            dead_mean: G::baseline(),
        }
    }
}

impl<G: Genome> FactionPool<G> {
    /// Compute the expected base traits for a newly spawned unit (blended mean, no mutation).
    pub fn birth_base_traits(&self, alpha: f32) -> G {
        if self.dead_count == 0 && self.live_count == 0 {
            G::baseline()
        } else if self.dead_count == 0 {
            self.live_mean.clone()
        } else if self.live_count == 0 {
            self.dead_mean.clone()
        } else {
            self.live_mean.blend(&self.dead_mean, alpha, 1.0 - alpha)
        }
    }

    /// Generate new trait values by sampling from the blended gene pool.
    pub fn generate_traits(&self, cfg: &GameConfig, rng: &mut impl Rng, sigma: f32) -> G {
        let base = if self.dead_count == 0 && self.live_count == 0 {
            G::baseline()
        } else if self.dead_count == 0 {
            self.live_mean.clone()
        } else if self.live_count == 0 {
            self.dead_mean.clone()
        } else {
            self.live_mean
                .blend(&self.dead_mean, cfg.alpha, 1.0 - cfg.alpha)
        };

        base.mutate(rng, sigma)
    }

    /// Record a new live unit birth.
    pub fn record_birth(&mut self, t: &G) {
        self.live_count += 1;
        self.live_mean.welford_update(t, self.live_count as f32);
    }

    /// Record a unit death (removed from live stats, added to dead rolling window).
    /// Fusion-consumed units must NOT call this — call `record_fusion_loss` instead.
    pub fn record_death(&mut self, t: &G, rolling_n: u32) {
        self.live_count = self.live_count.saturating_sub(1);
        self.dead_count += 1;
        let n_eff = (self.dead_count.min(rolling_n)) as f32;
        self.dead_mean.welford_update(t, n_eff);
    }

    /// Record fusion loss: unit is consumed but NOT added to dead pool
    pub fn record_fusion_loss(&mut self) {
        self.live_count = self.live_count.saturating_sub(1);
    }
}

/// Convenience alias for the cyborg-themed game.
pub type CyborgGenePool = GenePool<GeneticTraits>;
