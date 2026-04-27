mod camera;
mod selection;
mod state;
mod theme;
mod ui;

use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPostUpdateSet, EguiPrimaryContextPass};

use camera::{GameHudInsets, apply_game_camera_viewport};
use selection::{
    draw_selected_base_gizmo, maintain_hud_state, select_base_system, validate_base_repair,
    validate_base_upgrade, validate_global_expansion,
};
use state::HudState;
use ui::hud_system;

pub struct HudPlugin;

#[derive(Message, Clone, Debug)]
pub(crate) enum HudUiAction {
    RestartMatch,
    ConfirmGlobalExpansion,
    ConfirmBaseUpgrade(Entity),
    ConfirmBaseRepair(Entity),
    ChangeMap(String),
}

fn configured_egui_plugin() -> EguiPlugin {
    #[cfg(target_os = "macos")]
    {
        EguiPlugin {
            bindless_mode_array_size: None,
            ..EguiPlugin::default()
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        EguiPlugin::default()
    }
}

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(configured_egui_plugin())
            .add_message::<HudUiAction>()
            .init_resource::<HudState>()
            .init_resource::<GameHudInsets>()
            .add_systems(Update, (process_hud_ui_actions, maintain_hud_state).chain())
            .add_systems(
                EguiPrimaryContextPass,
                (hud_system, apply_game_camera_viewport).chain(),
            )
            .add_systems(
                PostUpdate,
                select_base_system
                    .after(EguiPostUpdateSet::ProcessOutput)
                    .run_if(super::game_state::match_is_active),
            )
            .add_systems(
                PostUpdate,
                draw_selected_base_gizmo
                    .after(select_base_system)
                    .run_if(super::game_state::match_is_active),
            );
    }
}

#[allow(clippy::too_many_arguments)]
fn process_hud_ui_actions(
    mut actions: MessageReader<HudUiAction>,
    mut cfg: ResMut<swarm_model::GameConfig>,
    mut biomass: ResMut<swarm_combat::Biomass>,
    mut upgrades: ResMut<swarm_combat::ArmyCapacityUpgrades>,
    mut hud_state: ResMut<HudState>,
    game_end: Res<super::game_state::GameEnd>,
    unit_positions: Query<(&Transform, &swarm_model::Unit)>,
    mut base_query: Query<(Entity, &Transform, &mut swarm_model::Base)>,
    mut restart: MessageWriter<super::game_flow::RestartRequested>,
) {
    for action in actions.read() {
        match action {
            HudUiAction::RestartMatch => {
                restart.write(super::game_flow::RestartRequested);
            }
            HudUiAction::ChangeMap(new_id) => {
                cfg.map_id = new_id.clone();
                restart.write(super::game_flow::RestartRequested);
            }
            HudUiAction::ConfirmGlobalExpansion => {
                if game_end.0.is_some() {
                    continue;
                }
                match validate_global_expansion(&cfg, &biomass, &upgrades) {
                    Ok(cost) => {
                        biomass.player -= cost;
                        upgrades.player_tier += 1;
                    }
                    Err(reason) => {
                        hud_state.cancel_action(state::PendingHudAction::GlobalExpansion, reason);
                    }
                }
            }
            HudUiAction::ConfirmBaseUpgrade(entity) => {
                if game_end.0.is_some() {
                    continue;
                }
                let Ok((_, base_tf, mut base)) = base_query.get_mut(*entity) else {
                    hud_state.clear_pending();
                    continue;
                };
                let base_pos = base_tf.translation.truncate();
                match validate_base_upgrade(
                    base.as_ref(),
                    base_pos,
                    &cfg,
                    &biomass,
                    &unit_positions,
                ) {
                    Ok(cost) => {
                        if base.upgrade(&cfg) {
                            biomass.player -= cost;
                        } else {
                            hud_state.show_warning("This base cannot upgrade anymore.");
                        }
                    }
                    Err(reason) => {
                        hud_state.show_warning(reason);
                    }
                }
            }
            HudUiAction::ConfirmBaseRepair(entity) => {
                if game_end.0.is_some() {
                    continue;
                }
                let Ok((_, _, mut base)) = base_query.get_mut(*entity) else {
                    hud_state.clear_pending();
                    continue;
                };
                match validate_base_repair(base.as_ref(), &cfg, &biomass) {
                    Ok(preview) => {
                        let healed = base.repair(preview.amount);
                        let charged = cfg.base_repair_cost(healed);
                        biomass.player -= charged;
                        base.trigger_repair_cooldown(&cfg);
                    }
                    Err(reason) => {
                        hud_state
                            .cancel_action(state::PendingHudAction::RepairBase(*entity), reason);
                    }
                }
            }
        }
    }
}
