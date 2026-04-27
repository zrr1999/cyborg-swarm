mod bottom_bar;
mod confirmation_strip;
mod dev_overlay;
mod shared_visuals;
mod top_bar;

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use swarm_combat::{ArmyCapacityUpgrades, Biomass};
use swarm_model::{Base, GameConfig, Unit};

use super::HudUiAction;
use super::camera::GameHudInsets;
use super::state::HudState;
use bottom_bar::show_bottom_bar;
use top_bar::show_top_bar;

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn hud_system(
    mut contexts: EguiContexts,
    mut cfg: ResMut<GameConfig>,
    biomass: Res<Biomass>,
    upgrades: Res<ArmyCapacityUpgrades>,
    beacon_state: Res<crate::plugins::beacons::BeaconControlState>,
    mut hud_state: ResMut<HudState>,
    mut hud_insets: ResMut<GameHudInsets>,
    unit_positions: Query<(&Transform, &Unit)>,
    base_query: Query<(Entity, &Base, &Transform)>,
    domination: Res<crate::plugins::game_state::DominationState>,
    game_end: Res<crate::plugins::game_state::GameEnd>,
    mut actions: MessageWriter<HudUiAction>,
    time: Res<Time>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    // Always use full-screen viewport — no insets.
    *hud_insets = GameHudInsets::default();

    if let Some(outcome) = game_end.0 {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgba_unmultiplied(6, 8, 14, 238)))
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() * 0.36);
                    let title = if outcome.victory { "Victory" } else { "Defeat" };
                    ui.heading(egui::RichText::new(title).size(30.0).strong());
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(if outcome.victory {
                            "You held total domination over every base."
                        } else {
                            "The enemy held total domination over every base."
                        })
                        .small(),
                    );
                    ui.add_space(20.0);
                    if ui
                        .button(egui::RichText::new("Restart").size(20.0))
                        .clicked()
                    {
                        actions.write(HudUiAction::RestartMatch);
                    }
                });
            });
        return;
    }

    let screen = ctx.content_rect();

    // Top status bar — floating overlay at the top edge.
    egui::Area::new("hud_top_bar".into())
        .fixed_pos(egui::pos2(0.0, 0.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.set_min_width(screen.width());
            super::theme::top_bar_frame().show(ui, |ui| {
                ui.set_min_width(screen.width() - 24.0);
                show_top_bar(
                    ui,
                    &cfg,
                    &biomass,
                    &upgrades,
                    &domination,
                    &beacon_state,
                    &mut hud_state,
                );
            });
        });

    // Bottom action bar — floating overlay at the bottom edge.
    egui::Area::new("hud_bottom_bar".into())
        .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(0.0, 0.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.set_min_width(screen.width());
            super::theme::bottom_bar_frame().show(ui, |ui| {
                ui.set_min_width(screen.width() - 24.0);
                show_bottom_bar(
                    ui,
                    &cfg,
                    &biomass,
                    &upgrades,
                    &unit_positions,
                    &mut hud_state,
                    &base_query,
                    &mut actions,
                );
            });
        });

    // Dev overlay window (floating, toggle via DEV button in top bar).
    if hud_state.dev_overlay_open {
        let mut open = true;
        dev_overlay::show_dev_overlay(
            dev_overlay::DevOverlayArgs {
                ctx,
                cfg: &mut cfg,
                time: &time,
                actions: &mut actions,
                top_height: 48.0,
            },
            &mut open,
        );
        if !open {
            hud_state.dev_overlay_open = false;
        }
    }

    // CentralPanel required by egui to claim the remaining space.
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ctx, |_ui| {});
}
