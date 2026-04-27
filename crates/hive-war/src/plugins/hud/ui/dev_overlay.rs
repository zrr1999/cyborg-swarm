use bevy::prelude::*;
use bevy_egui::egui;
use swarm_model::GameConfig;

use crate::map::BUILTIN_MAPS;

use super::super::theme::TEXT_DIM;
use crate::plugins::hud::HudUiAction;

pub(super) struct DevOverlayArgs<'a, 'w> {
    pub(super) ctx: &'a egui::Context,
    pub(super) cfg: &'a mut GameConfig,
    pub(super) time: &'a Time,
    pub(super) actions: &'a mut MessageWriter<'w, HudUiAction>,
    pub(super) top_height: f32,
}

pub(super) fn show_dev_overlay(args: DevOverlayArgs<'_, '_>, open: &mut bool) {
    if !*open {
        return;
    }

    egui::Window::new("DEV // control relay")
        .anchor(
            egui::Align2::LEFT_TOP,
            egui::vec2(16.0, args.top_height + 16.0),
        )
        .default_width(272.0)
        .min_width(240.0)
        .resizable(true)
        .collapsible(false)
        .frame(dev_window_frame())
        .open(open)
        .show(args.ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgba_unmultiplied(88, 110, 132, 72))
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("DEV ONLY")
                                .monospace()
                                .size(10.0)
                                .color(egui::Color32::from_rgb(208, 224, 238)),
                        );
                    });
                ui.label(
                    egui::RichText::new(
                        "Map switching, mutation tuning, and runtime metrics stay isolated from the player shell.",
                    )
                    .small()
                    .color(TEXT_DIM),
                );
            });
            ui.add_space(10.0);

            show_dev_section(
                ui,
                "Scenario controls",
                "Keep map changes and restarts available without mixing them into player-facing panels.",
                |ui| {
                    ui.label(egui::RichText::new("Map").small().color(TEXT_DIM));
                    show_map_selector_combo(ui, args.cfg.map_id.as_str(), args.actions);
                    ui.add_space(8.0);
                    if ui.button("Restart current match").clicked() {
                        args.actions.write(HudUiAction::RestartMatch);
                    }
                },
            );

            ui.add_space(8.0);
            show_dev_section(
                ui,
                "Mutation lab",
                "Direct balance tuning lives here for fast iteration during debug passes.",
                |ui| {
                    ui.add(
                        egui::Slider::new(&mut args.cfg.player_mutation_sigma, 0.01..=0.30)
                            .text("Player mutation sigma"),
                    );
                    ui.add(
                        egui::Slider::new(&mut args.cfg.enemy_mutation_sigma, 0.01..=0.30)
                            .text("Enemy mutation sigma"),
                    );
                    ui.add(
                        egui::Slider::new(&mut args.cfg.alpha, 0.0..=1.0)
                            .text("Gene pool alpha"),
                    );
                },
            );

            ui.add_space(8.0);
            show_dev_section(
                ui,
                "Runtime readout",
                "Quick performance checks remain pinned for development ergonomics.",
                |ui| {
                    let dt = args.time.delta_secs();
                    let fps = if dt > 0.0 { 1.0 / dt } else { 0.0 };
                    egui::Grid::new("dev_runtime_grid")
                        .num_columns(2)
                        .spacing([12.0, 4.0])
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("FPS").small().color(TEXT_DIM));
                            ui.monospace(format!("{fps:.0}"));
                            ui.end_row();

                            ui.label(egui::RichText::new("Frame time").small().color(TEXT_DIM));
                            ui.monospace(format!("{:.2} ms", dt * 1000.0));
                            ui.end_row();
                        });
                },
            );

            ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::hover());
        });
}

fn dev_window_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(18, 22, 32, 156))
        .stroke(egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(175, 196, 214, 72),
        ))
        .corner_radius(14.0)
        .inner_margin(egui::Margin::symmetric(12, 10))
}

fn dev_section_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(28, 34, 46, 136))
        .stroke(egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(136, 154, 174, 46),
        ))
        .corner_radius(10.0)
        .inner_margin(egui::Margin::symmetric(10, 8))
}

fn show_dev_section(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: &str,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    dev_section_frame().show(ui, |ui| {
        ui.label(
            egui::RichText::new(title)
                .small()
                .monospace()
                .color(egui::Color32::from_rgb(214, 226, 238)),
        );
        ui.label(egui::RichText::new(subtitle).small().color(TEXT_DIM));
        ui.add_space(6.0);
        add_contents(ui);
    });
}

fn show_map_selector_combo(
    ui: &mut egui::Ui,
    current_map_id: &str,
    actions: &mut MessageWriter<HudUiAction>,
) {
    let map_label = BUILTIN_MAPS
        .iter()
        .find(|(id, _)| *id == current_map_id)
        .map(|(_, label)| *label)
        .unwrap_or("Custom");
    egui::ComboBox::from_id_salt("map_selector")
        .width(ui.available_width())
        .selected_text(format!("{} ({})", map_label, current_map_id))
        .show_ui(ui, |ui| {
            for (id, label) in BUILTIN_MAPS {
                if ui
                    .selectable_label(current_map_id == *id, format!("{label} ({id})"))
                    .clicked()
                {
                    actions.write(HudUiAction::ChangeMap((*id).into()));
                }
            }
        });
}
