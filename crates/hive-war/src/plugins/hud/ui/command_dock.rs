use bevy::prelude::*;
use bevy_egui::egui;
use swarm_combat::{ArmyCapacityUpgrades, Biomass};
use swarm_model::{Base, GameConfig, Unit};

use super::super::state::HudState;
use super::super::theme::dock_frame;
use super::action_chamber::{show_embedded_resource_cluster, show_embedded_selected_base_panel};
use crate::plugins::hud::HudUiAction;

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn show_command_dock(
    ctx: &egui::Context,
    cfg: &GameConfig,
    biomass: &Biomass,
    upgrades: &ArmyCapacityUpgrades,
    unit_positions: &Query<(&Transform, &Unit)>,
    hud_state: &mut HudState,
    base_query: &Query<(Entity, &Base, &Transform)>,
    top_height: f32,
    game_camera: Option<(&Camera, &GlobalTransform)>,
    actions: &mut MessageWriter<HudUiAction>,
) {
    egui::Area::new("hud_resource_cluster".into())
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(14.0, top_height + 14.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.set_max_width(320.0);
            dock_frame().show(ui, |ui| {
                show_embedded_resource_cluster(
                    ui, cfg, biomass, upgrades, hud_state, base_query, actions,
                );
            });
        });

    let Some(selected_pos) =
        selected_panel_position(ctx, hud_state, base_query, game_camera, top_height)
    else {
        return;
    };

    egui::Area::new("hud_selected_base_panel".into())
        .fixed_pos(selected_pos)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.set_max_width(320.0);
            egui::ScrollArea::vertical()
                .max_height((ctx.content_rect().bottom() - selected_pos.y - 16.0).max(180.0))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    dock_frame().show(ui, |ui| {
                        show_embedded_selected_base_panel(
                            ui,
                            cfg,
                            biomass,
                            unit_positions,
                            hud_state,
                            base_query,
                            actions,
                        );
                    });
                });
        });
}

#[allow(clippy::type_complexity)]
fn selected_panel_position(
    ctx: &egui::Context,
    hud_state: &HudState,
    base_query: &Query<(Entity, &Base, &Transform)>,
    game_camera: Option<(&Camera, &GlobalTransform)>,
    top_height: f32,
) -> Option<egui::Pos2> {
    let selected_base = hud_state.selected_base()?;
    let (camera, camera_transform) = game_camera?;
    let (_, _, base_tf) = base_query.get(selected_base).ok()?;
    let screen = camera
        .world_to_viewport(camera_transform, base_tf.translation)
        .ok()?;
    let content_rect = ctx.content_rect();
    let panel_width = 320.0;
    let x_offset = if screen.x > content_rect.center().x {
        -(panel_width + 28.0)
    } else {
        28.0
    };
    let y_offset = -18.0;

    let x = (screen.x + x_offset).clamp(
        content_rect.left() + 14.0,
        content_rect.right() - panel_width - 14.0,
    );
    let y = (screen.y + y_offset).clamp(top_height + 14.0, content_rect.bottom() - 180.0);
    Some(egui::pos2(x, y))
}
