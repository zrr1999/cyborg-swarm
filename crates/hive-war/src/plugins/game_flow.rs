use bevy::prelude::*;
use rand::{SeedableRng, rngs::SmallRng};

use swarm_combat::{ArmyCapacityUpgrades, Biomass};
use swarm_growth::CyborgGenePool;
use swarm_model::{Base, GameConfig};
use swarm_sim::simulation::{detect_domination_outcome, sync_flow_fields_to_bases};
use swarm_space::{FlowFieldSet, SpatialHash};

use crate::map::ActiveMap;

use super::capture::EnemyArmyUpgradeTimer;
use super::experience::{BaseAssaultLedger, CombatContributionLedger};
use super::game_state::{DominationState, GameEnd};
use super::genetics::FusionRng;
use super::markers::GameplayEntity;
use super::units::RngResource;
use super::wolf_pack::WolfPack;
use super::world::setup_world;

pub struct GameFlowPlugin;

impl Plugin for GameFlowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameEnd>()
            .init_resource::<DominationState>()
            .init_resource::<RestartFlow>()
            .add_message::<RestartRequested>();

        app.add_systems(Update, detect_domination_outcome)
            .add_systems(Update, restart_session_begin)
            .add_systems(Update, ApplyDeferred.after(restart_session_begin))
            .add_systems(Update, restart_session_spawn_world.after(ApplyDeferred))
            .add_systems(
                Update,
                restart_session_finish_flow_fields.after(restart_session_spawn_world),
            );
    }
}

#[derive(Message)]
pub struct RestartRequested;

#[derive(Resource, Default)]
struct RestartFlow {
    pending_world_spawn: bool,
    pending_flow_regen: bool,
}

#[allow(clippy::too_many_arguments)]
fn restart_session_begin(
    mut events: MessageReader<RestartRequested>,
    mut commands: Commands,
    gameplay: Query<Entity, With<GameplayEntity>>,
    mut game_end: ResMut<GameEnd>,
    mut domination: ResMut<DominationState>,
    mut restart_flow: ResMut<RestartFlow>,
    mut gene_pool: ResMut<CyborgGenePool>,
    mut biomass: ResMut<Biomass>,
    mut upgrades: ResMut<ArmyCapacityUpgrades>,
    mut spatial: ResMut<SpatialHash>,
    mut flow_fields: ResMut<FlowFieldSet>,
    mut combat_ledger: ResMut<CombatContributionLedger>,
    mut assault_ledger: ResMut<BaseAssaultLedger>,
    (cfg, mut wolf_pack): (Res<GameConfig>, ResMut<WolfPack>),
    (mut rng_unit, mut rng_fusion): (ResMut<RngResource>, ResMut<FusionRng>),
    mut enemy_army_timer: ResMut<EnemyArmyUpgradeTimer>,
) {
    for _ in events.read() {
        for e in gameplay.iter() {
            commands.entity(e).despawn_children().despawn();
        }

        *game_end = GameEnd(None);
        *domination = DominationState::default();
        *gene_pool = CyborgGenePool::default();
        *biomass = Biomass::default();
        *upgrades = ArmyCapacityUpgrades::default();
        *spatial = SpatialHash::default();
        *flow_fields = FlowFieldSet::default();
        *combat_ledger = CombatContributionLedger::default();
        *assault_ledger = BaseAssaultLedger::default();
        *wolf_pack = WolfPack::from_config(cfg.population_optimizer.clone());
        rng_unit.0 = SmallRng::from_rng(&mut rand::rng());
        rng_fusion.0 = SmallRng::from_rng(&mut rand::rng());
        enemy_army_timer.0 = Timer::from_seconds(1.5, TimerMode::Repeating);

        restart_flow.pending_world_spawn = true;
    }
}

fn restart_session_spawn_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    map: Res<ActiveMap>,
    cfg: Res<GameConfig>,
    mut restart_flow: ResMut<RestartFlow>,
) {
    if !restart_flow.pending_world_spawn {
        return;
    }
    restart_flow.pending_world_spawn = false;
    setup_world(&mut commands, &mut meshes, &mut materials, &map.0, &cfg);
    restart_flow.pending_flow_regen = true;
}

fn restart_session_finish_flow_fields(
    mut flow_fields: ResMut<FlowFieldSet>,
    cfg: Res<GameConfig>,
    base_query: Query<(&Transform, &Base)>,
    mut restart_flow: ResMut<RestartFlow>,
) {
    if !restart_flow.pending_flow_regen {
        return;
    }
    restart_flow.pending_flow_regen = false;
    sync_flow_fields_to_bases(&mut flow_fields, &cfg, &base_query);
}
