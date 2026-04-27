use bevy::prelude::*;
use bevy_egui::egui;
use swarm_combat::ArmyCapacityUpgrades;
use swarm_growth::CyborgGenePool;
use swarm_model::{Base, GameConfig, Unit};

use super::super::selection::base_is_contested;
use super::super::state::HudState;
use super::super::theme::{
    ACCENT_CYAN, ACCENT_GREEN, TEXT_DIM, faction_color_ui, faction_name, panel_frame,
};
use super::UnitIntel;

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn show_intel_overlay(
    ctx: &egui::Context,
    cfg: &GameConfig,
    gene_pool: &CyborgGenePool,
    upgrades: &ArmyCapacityUpgrades,
    intel: &UnitIntel,
    hud_state: &mut HudState,
    unit_positions: &Query<(&Transform, &Unit)>,
    base_query: &Query<(Entity, &Base, &Transform)>,
    top_height: f32,
) {
    let mut open = hud_state.intel_overlay_open();
    if !open {
        return;
    }
    let selected_base = hud_state.selected_base();

    egui::Window::new("Intel")
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-8.0, top_height + 8.0))
        .default_width(170.0)
        .min_width(140.0)
        .max_width(180.0)
        .resizable(false)
        .collapsible(false)
        .frame(panel_frame())
        .open(&mut open)
        .show(ctx, |ui| {
            // — Army —
            section_header(ui, "— Army —");
            let player_cap =
                upgrades.faction_unit_cap(&swarm_model::Faction::Player, cfg);
            ui.horizontal_wrapped(|ui| {
                show_chip(ui, &format!("{} units", intel.player_count), ACCENT_GREEN);
                show_chip(ui, &format!("cap {player_cap}"), TEXT_DIM);
            });
            ui.add_space(4.0);
            show_class_bars(
                ui,
                ACCENT_GREEN,
                intel.player_count,
                &intel.player_class_counts,
            );

            ui.add_space(8.0);

            // — Enemy —
            section_header(ui, "— Enemy —");
            let enemy_cap = upgrades.faction_unit_cap(&swarm_model::Faction::Enemy, cfg);
            let enemy_color = egui::Color32::from_rgb(255, 109, 109);
            ui.horizontal_wrapped(|ui| {
                show_chip(ui, &format!("{} units", intel.enemy_count), enemy_color);
                show_chip(ui, &format!("cap {enemy_cap}"), TEXT_DIM);
            });
            ui.add_space(4.0);
            show_class_bars(
                ui,
                enemy_color,
                intel.enemy_count,
                &intel.enemy_class_counts,
            );

            ui.add_space(8.0);

            // — Genome —
            section_header(ui, "— Genome —");
            show_gene_pool_card(ui, "Player", ACCENT_GREEN, &gene_pool.player.live_mean);
            ui.add_space(4.0);
            show_gene_pool_card(ui, "Enemy", enemy_color, &gene_pool.enemy.live_mean);

            ui.add_space(8.0);

            // — Node — (details for the base selected in the world)
            section_header(ui, "— Node —");
            show_selected_node(
                ui,
                cfg,
                selected_base,
                hud_state,
                unit_positions,
                base_query,
            );
        });

    hud_state.set_intel_overlay_open(open);
}

fn show_selected_node(
    ui: &mut egui::Ui,
    cfg: &GameConfig,
    selected_base: Option<Entity>,
    hud_state: &mut HudState,
    unit_positions: &Query<(&Transform, &Unit)>,
    base_query: &Query<(Entity, &Base, &Transform)>,
) {
    let Some(selected_entity) = selected_base else {
        ui.label(
            egui::RichText::new("Click a base in the world to inspect secondary node detail.")
                .small()
                .color(TEXT_DIM),
        );
        return;
    };

    let Ok((_, base, base_tf)) = base_query.get(selected_entity) else {
        hud_state.drop_missing_selection();
        ui.label(
            egui::RichText::new(
                "Selected node drifted out of scope. Click a base to inspect it again.",
            )
            .small()
            .color(TEXT_DIM),
        );
        return;
    };

    let base_pos = base_tf.translation.truncate();
    let contested = base_is_contested(base, base_pos, unit_positions);
    let faction_color = faction_color_ui(base.faction);
    let health_pct = (base.health / base.max_health.max(1.0)) * 100.0;

    ui.label(
        egui::RichText::new(format!(
            "{} base @ ({:.0}, {:.0})",
            faction_name(base.faction),
            base_pos.x,
            base_pos.y
        ))
        .color(faction_color)
        .strong(),
    );
    ui.horizontal_wrapped(|ui| {
        show_chip(
            ui,
            &format!("Level {}/{}", base.level, cfg.base_max_level),
            ACCENT_CYAN,
        );
        show_chip(ui, &format!("{health_pct:.0}% integrity"), ACCENT_GREEN);
        show_chip(
            ui,
            if contested { "Contested" } else { "Stable" },
            if contested {
                egui::Color32::from_rgb(255, 140, 140)
            } else {
                faction_color
            },
        );
    });
    ui.add_space(6.0);

    egui::Grid::new("intel_spine_selected_base_grid")
        .num_columns(2)
        .spacing(egui::vec2(14.0, 4.0))
        .show(ui, |ui| {
            detail_row(ui, "Owner", faction_name(base.faction));
            detail_row(
                ui,
                "Health",
                format!("{:.0} / {:.0}", base.health, base.max_health),
            );
            detail_row(
                ui,
                "Income",
                format!("+{:.1} biomass/s", cfg.base_income_for_level(base.level)),
            );
            detail_row(ui, "Local cap", base.max_units);
            detail_row(
                ui,
                "Spawn efficiency",
                format!("{:.0}%", base.spawn_efficiency(cfg) * 100.0),
            );
            detail_row(ui, "Contested", if contested { "yes" } else { "no" });
            detail_row(
                ui,
                "Weak recovery",
                if base.is_weak() {
                    format!("{:.1}s", base.capture_weak_timer)
                } else {
                    "stable".to_owned()
                },
            );
            detail_row(
                ui,
                "Repair cycle",
                if base.repair_cooldown > 0.0 {
                    format!("{:.1}s", base.repair_cooldown)
                } else {
                    "ready".to_owned()
                },
            );
        });
}

fn section_header(ui: &mut egui::Ui, title: &str) {
    ui.label(egui::RichText::new(title).small().strong().color(TEXT_DIM));
    ui.add_space(2.0);
}

fn show_class_bars(
    ui: &mut egui::Ui,
    color: egui::Color32,
    total: u32,
    class_counts: &std::collections::BTreeMap<String, u32>,
) {
    if total == 0 || class_counts.is_empty() {
        ui.label(egui::RichText::new("none").small().color(TEXT_DIM));
        return;
    }
    for (class, count) in class_counts {
        let share = (*count as f32 / total.max(1) as f32).clamp(0.0, 1.0);
        ui.add(
            egui::ProgressBar::new(share)
                .desired_width(f32::INFINITY)
                .fill(soft_fill(color, 140))
                .text(format!("{class} {count} ({:.0}%)", share * 100.0)),
        );
        ui.add_space(2.0);
    }
}

fn show_gene_pool_card(
    ui: &mut egui::Ui,
    title: &str,
    color: egui::Color32,
    genes: &swarm_model::GeneticTraits,
) {
    egui::Frame::new()
        .fill(soft_fill(color, 22))
        .stroke(egui::Stroke::new(1.0, soft_fill(color, 56)))
        .corner_radius(10.0)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(title).small().strong().color(color));
            ui.add_space(2.0);
            egui::Grid::new(title)
                .num_columns(2)
                .spacing(egui::vec2(8.0, 2.0))
                .show(ui, |ui| {
                    detail_row(ui, "HP", format!("{:.1}", genes.max_health));
                    detail_row(ui, "ATK", format!("{:.1}", genes.attack));
                    detail_row(ui, "SPD", format!("{:.1}", genes.speed));
                    detail_row(ui, "RNG", format!("{:.1}", genes.sensor_range));
                    detail_row(ui, "ARM", format!("{:.1}", genes.armor));
                    detail_row(ui, "APS", format!("{:.2}", genes.attack_speed));
                });
        });
}

fn detail_row(ui: &mut egui::Ui, label: &str, value: impl std::fmt::Display) {
    ui.label(egui::RichText::new(label).small().color(TEXT_DIM));
    ui.monospace(value.to_string());
    ui.end_row();
}

fn show_chip(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
    egui::Frame::new()
        .fill(soft_fill(color, 28))
        .stroke(egui::Stroke::new(1.0, soft_fill(color, 64)))
        .corner_radius(999.0)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).small().strong().color(color));
        });
}

fn soft_fill(color: egui::Color32, alpha: u8) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}
