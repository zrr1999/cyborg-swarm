use bevy::prelude::*;
use rand::{Rng, RngExt};

use swarm_model::{
    Faction, GameConfig, GeneticTraits, Genome, ParticleSwarmOptimizerConfig,
    PopulationOptimizerConfig, PopulationOptimizerKind, Unit, UnitFitness,
};

pub struct PopulationOptimizerPlugin;

impl Plugin for PopulationOptimizerPlugin {
    fn build(&self, app: &mut App) {
        let config = app
            .world()
            .get_resource::<GameConfig>()
            .map(|cfg| cfg.population_optimizer.clone())
            .unwrap_or_default();
        app.insert_resource(PopulationOptimizer::from_config(config))
            .add_systems(Update, accumulate_survival_fitness);
    }
}

fn accumulate_survival_fitness(time: Res<Time>, mut query: Query<&mut UnitFitness, With<Unit>>) {
    let dt = time.delta_secs();
    for mut fitness in &mut query {
        fitness.survival_time += dt;
    }
}

#[derive(Resource, Clone, Debug)]
pub struct PopulationOptimizer {
    pub generation: u32,
    pub config: PopulationOptimizerConfig,
    player: FactionOptimizerState,
    enemy: FactionOptimizerState,
}

impl Default for PopulationOptimizer {
    fn default() -> Self {
        Self::from_config(PopulationOptimizerConfig::default())
    }
}

impl PopulationOptimizer {
    pub fn from_config(config: PopulationOptimizerConfig) -> Self {
        Self {
            generation: 0,
            config,
            player: FactionOptimizerState::default(),
            enemy: FactionOptimizerState::default(),
        }
    }

    pub fn record_candidate(&mut self, faction: Faction, traits: &GeneticTraits, fitness: f32) {
        let algorithm = self.config.algorithm;
        let particle_swarm_cfg = self.config.particle_swarm.clone();
        let Some(state) = self.state_mut(faction) else {
            return;
        };
        match algorithm {
            PopulationOptimizerKind::GreyWolf => state.grey_wolf.update(traits, fitness),
            PopulationOptimizerKind::ParticleSwarm => {
                state
                    .particle_swarm
                    .record_candidate(traits, fitness, &particle_swarm_cfg)
            }
        }
    }

    pub fn generate_guided_traits(
        &mut self,
        faction: Faction,
        base: &GeneticTraits,
        cfg: &GameConfig,
        rng: &mut impl Rng,
    ) -> GeneticTraits {
        let algorithm = self.config.algorithm;
        let generation = self.generation;
        let grey_wolf_cfg = self.config.grey_wolf.clone();
        let particle_swarm_cfg = cfg.population_optimizer.particle_swarm.clone();
        let Some(state) = self.state_mut(faction) else {
            return base.clone();
        };
        let guided =
            match algorithm {
                PopulationOptimizerKind::GreyWolf => {
                    let a = (grey_wolf_cfg.initial_a
                        - grey_wolf_cfg.a_decay_per_generation * generation as f32)
                        .max(0.0);
                    state.grey_wolf.generate_guided_traits(base, a, rng)
                }
                PopulationOptimizerKind::ParticleSwarm => state
                    .particle_swarm
                    .generate_guided_traits(base, &particle_swarm_cfg, generation, rng),
            };
        self.generation = self.generation.saturating_add(1);
        guided
    }

    fn state_mut(&mut self, faction: Faction) -> Option<&mut FactionOptimizerState> {
        match faction {
            Faction::Player => Some(&mut self.player),
            Faction::Enemy => Some(&mut self.enemy),
            Faction::Neutral => None,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct FactionOptimizerState {
    grey_wolf: GreyWolfFactionState,
    particle_swarm: ParticleSwarmFactionState,
}

#[derive(Clone, Debug, Default)]
pub struct GreyWolfFactionState {
    pub alpha: Option<(GeneticTraits, f32)>,
    pub beta: Option<(GeneticTraits, f32)>,
    pub delta: Option<(GeneticTraits, f32)>,
}

impl GreyWolfFactionState {
    pub fn update(&mut self, traits: &GeneticTraits, fitness: f32) {
        let mut leaders = [
            self.alpha.clone(),
            self.beta.clone(),
            self.delta.clone(),
            Some((traits.clone(), fitness)),
        ];
        leaders.sort_by(|a, b| {
            let a_fit = a
                .as_ref()
                .map(|(_, score)| *score)
                .unwrap_or(f32::NEG_INFINITY);
            let b_fit = b
                .as_ref()
                .map(|(_, score)| *score)
                .unwrap_or(f32::NEG_INFINITY);
            b_fit
                .partial_cmp(&a_fit)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        self.alpha = leaders[0].clone();
        self.beta = leaders[1].clone();
        self.delta = leaders[2].clone();
    }

    pub fn generate_guided_traits(
        &self,
        base: &GeneticTraits,
        a: f32,
        rng: &mut impl Rng,
    ) -> GeneticTraits {
        let (Some((alpha, _)), Some((beta, _)), Some((delta, _))) =
            (&self.alpha, &self.beta, &self.delta)
        else {
            return base.clone();
        };

        let base_values = base.as_slice();
        let x1 = gwo_step(&base_values, &alpha.as_slice(), a, rng);
        let x2 = gwo_step(&base_values, &beta.as_slice(), a, rng);
        let x3 = gwo_step(&base_values, &delta.as_slice(), a, rng);
        let guided: Vec<f32> = x1
            .iter()
            .zip(x2.iter())
            .zip(x3.iter())
            .map(|((a, b), c)| (a + b + c) / 3.0)
            .collect();
        let guided = clamp_traits(&guided);

        // Preserve the current mean as the anchor and bias it toward the strongest leaders.
        base.blend_with(&guided, 0.35, 0.65)
    }
}

fn gwo_step(base: &[f32], leader: &[f32], a: f32, rng: &mut impl Rng) -> Vec<f32> {
    leader
        .iter()
        .zip(base.iter())
        .map(|(leader_value, base_value)| {
            let r1: f32 = rng.random_range(0.0..1.0);
            let r2: f32 = rng.random_range(0.0..1.0);
            let a_coeff = 2.0 * a * r1 - a;
            let c_coeff = 2.0 * r2;
            leader_value - a_coeff * (c_coeff * leader_value - base_value).abs()
        })
        .collect()
}

#[derive(Clone, Debug)]
struct SwarmParticle {
    position: GeneticTraits,
    velocity: Vec<f32>,
    personal_best: GeneticTraits,
    personal_best_fitness: f32,
}

#[derive(Clone, Debug, Default)]
struct ParticleSwarmFactionState {
    particles: Vec<SwarmParticle>,
    global_best: Option<(GeneticTraits, f32)>,
}

impl ParticleSwarmFactionState {
    fn record_candidate(
        &mut self,
        traits: &GeneticTraits,
        fitness: f32,
        cfg: &ParticleSwarmOptimizerConfig,
    ) {
        let size = cfg.swarm_size.max(1) as usize;
        if self.particles.len() < size {
            self.particles.push(SwarmParticle {
                position: traits.clone(),
                velocity: vec![0.0; <GeneticTraits as Genome>::dimension_count()],
                personal_best: traits.clone(),
                personal_best_fitness: fitness,
            });
        } else if let Some((index, _)) =
            self.particles
                .iter()
                .enumerate()
                .min_by(|(_, lhs), (_, rhs)| {
                    genetic_distance(&lhs.position, traits)
                        .partial_cmp(&genetic_distance(&rhs.position, traits))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        {
            let particle = &mut self.particles[index];
            particle.position = traits.clone();
            if fitness > particle.personal_best_fitness {
                particle.personal_best = traits.clone();
                particle.personal_best_fitness = fitness;
            }
        }

        if self
            .global_best
            .as_ref()
            .is_none_or(|(_, best_fitness)| fitness > *best_fitness)
        {
            self.global_best = Some((traits.clone(), fitness));
        }
    }

    fn generate_guided_traits(
        &mut self,
        base: &GeneticTraits,
        cfg: &ParticleSwarmOptimizerConfig,
        generation: u32,
        rng: &mut impl Rng,
    ) -> GeneticTraits {
        let Some((global_best, _)) = self.global_best.clone() else {
            self.seed_swarm(base, cfg);
            return base.clone();
        };
        self.seed_swarm(base, cfg);

        let index = generation as usize % self.particles.len();
        let particle = &mut self.particles[index];
        let current_values = particle.position.as_slice();
        let personal_best = particle.personal_best.as_slice();
        let global_best = global_best.as_slice();

        let next_velocity: Vec<f32> = current_values
            .iter()
            .zip(personal_best.iter())
            .zip(global_best.iter())
            .zip(particle.velocity.iter())
            .map(|(((current, personal), global), velocity)| {
                let current_scale = current.abs().max(1.0);
                let max_velocity = current_scale * cfg.max_velocity_scale.max(0.0);
                let inertia = cfg.inertia_weight * *velocity;
                let cognitive =
                    cfg.cognitive_weight * rng.random_range(0.0..1.0) * (personal - current);
                let social = cfg.social_weight * rng.random_range(0.0..1.0) * (global - current);
                (inertia + cognitive + social).clamp(-max_velocity, max_velocity)
            })
            .collect();

        let candidate: Vec<f32> = current_values
            .iter()
            .zip(next_velocity.iter())
            .map(|(current, velocity)| current + velocity)
            .collect();
        let candidate = clamp_traits(&candidate);
        let blended = base.blend_with(
            &candidate,
            1.0 - cfg.guided_blend_weight,
            cfg.guided_blend_weight,
        );

        particle.position = blended.clone();
        particle.velocity = next_velocity;
        blended
    }

    fn seed_swarm(&mut self, base: &GeneticTraits, cfg: &ParticleSwarmOptimizerConfig) {
        let target_size = cfg.swarm_size.max(1) as usize;
        while self.particles.len() < target_size {
            self.particles.push(SwarmParticle {
                position: base.clone(),
                velocity: vec![0.0; <GeneticTraits as Genome>::dimension_count()],
                personal_best: base.clone(),
                personal_best_fitness: f32::NEG_INFINITY,
            });
        }
    }
}

fn genetic_distance(lhs: &GeneticTraits, rhs: &GeneticTraits) -> f32 {
    lhs.as_slice()
        .iter()
        .zip(rhs.as_slice().iter())
        .map(|(lhs, rhs)| (lhs - rhs).powi(2))
        .sum::<f32>()
        .sqrt()
}

fn clamp_traits(values: &[f32]) -> GeneticTraits {
    GeneticTraits {
        max_health: values[0].clamp(10.0, 2000.0),
        attack: values[1].clamp(1.0, 500.0),
        speed: values[2].clamp(15.0, 180.0),
        sensor_range: values[3].clamp(20.0, 300.0),
        armor: values[4].clamp(0.0, 100.0),
        attack_speed: values[5].clamp(0.1, 5.0),
    }
}

#[cfg(test)]
mod tests {
    use rand::{SeedableRng, rngs::SmallRng};
    use swarm_model::{GreyWolfOptimizerConfig, ParticleSwarmOptimizerConfig};

    use super::*;

    #[test]
    fn grey_wolf_keeps_top_three_leaders_by_fitness() {
        let mut wolves = GreyWolfFactionState::default();
        let base = GeneticTraits::default();

        wolves.update(&base, 1.0);
        wolves.update(&base.scale_by(1.1), 5.0);
        wolves.update(&base.scale_by(1.2), 3.0);
        wolves.update(&base.scale_by(1.3), 4.0);

        assert_eq!(wolves.alpha.as_ref().map(|(_, f)| *f), Some(5.0));
        assert_eq!(wolves.beta.as_ref().map(|(_, f)| *f), Some(4.0));
        assert_eq!(wolves.delta.as_ref().map(|(_, f)| *f), Some(3.0));
    }

    #[test]
    fn particle_swarm_can_nudge_traits_toward_recorded_best() {
        let mut optimizer = PopulationOptimizer::from_config(PopulationOptimizerConfig {
            algorithm: PopulationOptimizerKind::ParticleSwarm,
            grey_wolf: GreyWolfOptimizerConfig::default(),
            particle_swarm: ParticleSwarmOptimizerConfig {
                swarm_size: 2,
                inertia_weight: 0.0,
                cognitive_weight: 0.0,
                social_weight: 1.8,
                guided_blend_weight: 1.0,
                max_velocity_scale: 1.0,
            },
        });
        let cfg = GameConfig {
            population_optimizer: optimizer.config.clone(),
            ..GameConfig::default()
        };
        let base = GeneticTraits::default();
        let best = base.scale_by(1.4);
        let mut rng = SmallRng::seed_from_u64(7);

        let _ = optimizer.generate_guided_traits(Faction::Player, &base, &cfg, &mut rng);
        optimizer.record_candidate(Faction::Player, &best, 12.0);
        let guided = optimizer.generate_guided_traits(Faction::Player, &base, &cfg, &mut rng);

        assert!(guided.max_health > base.max_health);
        assert!(guided.attack > base.attack);
    }
}
