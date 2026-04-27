#![allow(clippy::too_many_arguments)]

use bevy::prelude::*;
use rand::{Rng, RngExt};
use swarm_growth::WolfPack;
use swarm_model::{
    Faction, GameConfig, GeneticTraits, MovementSuppression, SteeringInertia, Unit, UnitFitness,
};

use crate::plugins::{
    combat::AttackCooldown,
    game_flow::GameplayEntity,
    skills::SkillDraftState,
    squad::{CaptainAnnouncement, HitFlash, attach_unit_visuals},
    world::{MAP_HALF_H, MAP_HALF_W},
};

pub struct EnemiesPlugin;

#[derive(Component)]
pub struct EnemyUnit;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyTier {
    Grunt,
    Elite,
    Leader,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct EnemyBounty {
    pub damage_xp_scale: f32,
    pub kill_xp_scale: f32,
    pub cell_reward: u64,
}

#[derive(Resource)]
pub struct WaveState {
    pub wave: u32,
    pub kills: u32,
    pub next_wave_timer: f32,
    pub enemies_alive: u32,
    pub incoming_edge: Option<SpawnEdge>,
}

impl Default for WaveState {
    fn default() -> Self {
        Self {
            wave: 0,
            kills: 0,
            next_wave_timer: 3.0,
            enemies_alive: 0,
            incoming_edge: Some(SpawnEdge::North),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpawnEdge {
    North,
    South,
    East,
    West,
}

impl SpawnEdge {
    pub fn label(self) -> &'static str {
        match self {
            Self::North => "North",
            Self::South => "South",
            Self::East => "East",
            Self::West => "West",
        }
    }

    pub fn planned_for_wave(wave: u32) -> Self {
        match wave % 4 {
            1 => Self::North,
            2 => Self::East,
            3 => Self::South,
            _ => Self::West,
        }
    }
}

pub const MAX_WAVES_PER_RUN: u32 = 12;
const WAVE_INTERVAL: f32 = 15.0;

impl Plugin for EnemiesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WaveState>()
            .add_systems(Update, wave_spawn_system);
    }
}

fn wave_spawn_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut wave: ResMut<WaveState>,
    mut wolves: ResMut<WolfPack>,
    cfg: Res<GameConfig>,
    game_over: Res<crate::plugins::combat::GameOver>,
    draft: Res<SkillDraftState>,
    mut announcement: ResMut<CaptainAnnouncement>,
    time: Res<Time>,
) {
    if game_over.active {
        return;
    }
    if draft.active {
        return;
    }
    if wave.enemies_alive > 0 {
        return;
    }
    if wave.wave >= MAX_WAVES_PER_RUN {
        return;
    }
    wave.next_wave_timer -= time.delta_secs();
    if wave.next_wave_timer > 0.0 {
        return;
    }

    wave.wave += 1;
    let count = 6 + wave.wave * 4;
    wave.next_wave_timer = WAVE_INTERVAL;
    wave.enemies_alive += count;

    let mut rng = rand::rng();
    let edge = wave
        .incoming_edge
        .unwrap_or_else(|| SpawnEdge::planned_for_wave(wave.wave));
    wave.incoming_edge = Some(edge);
    let leader_count = leader_count_for_wave(wave.wave).min(count);
    let elite_count = elite_count_for_wave(wave.wave).min(count.saturating_sub(leader_count));

    if leader_count > 0 {
        announcement.show(format!(
            "Wave {} · {} breach · hostile leader detected",
            wave.wave,
            edge.label()
        ));
    } else if elite_count > 0 {
        announcement.show(format!(
            "Wave {} · {} breach · elite hostiles incoming",
            wave.wave,
            edge.label()
        ));
    } else {
        announcement.show(format!("Wave {} · {} breach", wave.wave, edge.label()));
    }

    for spawn_index in 0..count {
        let (x, y) = spawn_position(&mut rng, edge);
        let tier = enemy_tier_for_spawn(spawn_index, leader_count, elite_count);
        let archetype_traits = enemy_traits(wave.wave, tier);
        let traits =
            wolves.generate_guided_traits(Faction::Enemy, &archetype_traits, &cfg, &mut rng);
        let bounty = enemy_bounty(tier, wave.wave);
        let entity = commands
            .spawn((
                Transform::from_xyz(x, y, 1.0),
                Visibility::default(),
                GameplayEntity,
                EnemyUnit,
                tier,
                bounty,
                Unit::new(&traits, Faction::Enemy),
                traits.clone(),
                AttackCooldown(0.0),
                SteeringInertia::default(),
                MovementSuppression::default(),
                UnitFitness::default(),
                HitFlash::default(),
            ))
            .id();
        attach_unit_visuals(
            &mut commands,
            meshes.as_mut(),
            materials.as_mut(),
            entity,
            Faction::Enemy,
            &traits,
            &cfg,
            false,
        );
    }
}

fn spawn_position(rng: &mut impl Rng, edge: SpawnEdge) -> (f32, f32) {
    match edge {
        SpawnEdge::North => (rng.random_range(-MAP_HALF_W..MAP_HALF_W), MAP_HALF_H + 20.0),
        SpawnEdge::South => (
            rng.random_range(-MAP_HALF_W..MAP_HALF_W),
            -MAP_HALF_H - 20.0,
        ),
        SpawnEdge::East => (MAP_HALF_W + 20.0, rng.random_range(-MAP_HALF_H..MAP_HALF_H)),
        SpawnEdge::West => (
            -MAP_HALF_W - 20.0,
            rng.random_range(-MAP_HALF_H..MAP_HALF_H),
        ),
    }
}

fn enemy_tier_for_spawn(spawn_index: u32, leader_count: u32, elite_count: u32) -> EnemyTier {
    if spawn_index < leader_count {
        EnemyTier::Leader
    } else if spawn_index < leader_count + elite_count {
        EnemyTier::Elite
    } else {
        EnemyTier::Grunt
    }
}

fn elite_count_for_wave(wave: u32) -> u32 {
    match wave {
        0..=1 => 0,
        2..=3 => 1,
        _ => 1 + wave / 2,
    }
}

fn leader_count_for_wave(wave: u32) -> u32 {
    match wave {
        0..=3 => 0,
        4..=7 => 1,
        _ => 1 + wave / 6,
    }
}

fn enemy_bounty(tier: EnemyTier, wave: u32) -> EnemyBounty {
    let wave_damage_scale = 1.0 + wave.saturating_sub(1) as f32 * 0.10;
    let wave_kill_scale = 1.0 + wave.saturating_sub(1) as f32 * 0.16;
    let wave_cell_bonus = wave.saturating_sub(1) as u64 / 3;
    match tier {
        EnemyTier::Grunt => EnemyBounty {
            damage_xp_scale: 1.0 * wave_damage_scale,
            kill_xp_scale: 1.0 * wave_kill_scale,
            cell_reward: 1 + wave_cell_bonus,
        },
        EnemyTier::Elite => EnemyBounty {
            damage_xp_scale: 1.55 * wave_damage_scale,
            kill_xp_scale: 1.9 * wave_kill_scale,
            cell_reward: 3 + wave_cell_bonus * 2,
        },
        EnemyTier::Leader => EnemyBounty {
            damage_xp_scale: 2.2 * wave_damage_scale,
            kill_xp_scale: 3.0 * wave_kill_scale,
            cell_reward: 6 + wave_cell_bonus * 3,
        },
    }
}

fn enemy_traits(wave: u32, tier: EnemyTier) -> GeneticTraits {
    let wave_index = wave.saturating_sub(1) as f32;
    let scale = 1.0 + wave_index * 0.24 + wave_index * wave_index * 0.012;
    let mut traits = GeneticTraits {
        max_health: 74.0 * scale,
        attack: 10.5 * scale,
        speed: 114.0,
        armor: 1.6 * scale,
        attack_speed: 0.94,
        sensor_range: 88.0,
    };
    match tier {
        EnemyTier::Grunt => {}
        EnemyTier::Elite => {
            traits = traits.scale_by(1.85);
            traits.speed *= 1.01;
            traits.attack_speed *= 1.12;
            traits.armor += 3.0 + wave as f32 * 0.28;
            traits.sensor_range += 16.0;
        }
        EnemyTier::Leader => {
            traits = traits.scale_by(2.75);
            traits.speed *= 0.98;
            traits.attack_speed *= 1.2;
            traits.armor += 5.5 + wave as f32 * 0.4;
            traits.sensor_range += 28.0;
        }
    }
    traits
}
