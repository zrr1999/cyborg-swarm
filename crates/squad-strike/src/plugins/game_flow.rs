use bevy::prelude::*;
use swarm_growth::WolfPack;
use swarm_model::GameConfig;

use crate::plugins::{
    combat::GameOver,
    enemies::WaveState,
    evolution::EvolutionState,
    meta_progression::MetaProgression,
    skills::{RunSkills, SkillDraftState},
    squad::{CaptainAnnouncement, CaptainTarget, spawn_initial_squad},
};

pub struct GameFlowPlugin;

#[derive(Component)]
pub struct GameplayEntity;

#[derive(Message)]
pub struct RestartRequested;

impl Plugin for GameFlowPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<RestartRequested>()
            .add_systems(Update, (request_restart_on_keyboard, restart_session));
    }
}

fn request_restart_on_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    game_over: Res<GameOver>,
    mut restart: MessageWriter<RestartRequested>,
) {
    if game_over.active && keys.just_pressed(KeyCode::KeyR) {
        restart.write(RestartRequested);
    }
}

#[allow(clippy::too_many_arguments)]
fn restart_session(
    mut events: MessageReader<RestartRequested>,
    mut commands: Commands,
    gameplay: Query<Entity, With<GameplayEntity>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    cfg: Res<GameConfig>,
    meta: Res<MetaProgression>,
    mut wolves: ResMut<WolfPack>,
    mut wave: ResMut<WaveState>,
    mut evo: ResMut<EvolutionState>,
    mut game_over: ResMut<GameOver>,
    mut run_skills: ResMut<RunSkills>,
    mut skill_draft: ResMut<SkillDraftState>,
    mut captain_target: ResMut<CaptainTarget>,
    mut announcement: ResMut<CaptainAnnouncement>,
) {
    let mut should_restart = false;
    for _ in events.read() {
        should_restart = true;
    }
    if !should_restart {
        return;
    }

    for entity in gameplay.iter() {
        commands.entity(entity).despawn_children().despawn();
    }

    *wolves = WolfPack::from_config(cfg.population_optimizer.clone());
    *wave = WaveState::default();
    *evo = EvolutionState::default();
    *game_over = GameOver::default();
    *run_skills = RunSkills::default();
    *skill_draft = SkillDraftState::default();
    *captain_target = CaptainTarget::default();
    *announcement = CaptainAnnouncement::default();

    spawn_initial_squad(
        &mut commands,
        &cfg,
        &meta,
        meshes.as_mut(),
        materials.as_mut(),
    );
}
