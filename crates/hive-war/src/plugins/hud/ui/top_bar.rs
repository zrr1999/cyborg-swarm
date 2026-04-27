use bevy_egui::egui;
use swarm_combat::{ArmyCapacityUpgrades, Biomass};
use swarm_model::{Faction, GameConfig};

use super::super::state::HudState;
use super::super::theme::{ACCENT_CYAN, ACCENT_GREEN, TEXT_DIM};
use crate::components::BeaconMode;
use crate::shared_assets::{SharedGlyph, glyph_widget};

pub(super) fn show_top_bar(
    ui: &mut egui::Ui,
    cfg: &GameConfig,
    biomass: &Biomass,
    upgrades: &ArmyCapacityUpgrades,
    domination: &crate::plugins::game_state::DominationState,
    beacon_state: &crate::plugins::beacons::BeaconControlState,
    hud_state: &mut HudState,
) {
    ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
    ui.horizontal(|ui| {
        // Domination status
        status_pill(ui, cfg, domination);

        // Resources
        resource_pill(
            ui,
            "Biomass",
            &format!("{:.0}", biomass.player),
            ACCENT_CYAN,
            Some(SharedGlyph::SupplyCrate),
        );
        let player_cap = upgrades.faction_unit_cap(&Faction::Player, cfg);
        resource_pill(ui, "Cap", &format!("{player_cap}"), ACCENT_GREEN, None);

        // Beacon
        beacon_pill(ui, beacon_state);

        // DEV toggle (right-aligned)
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let dev_label = if hud_state.dev_overlay_open {
                "DEV ●"
            } else {
                "DEV"
            };
            let dev_btn = ui.small_button(egui::RichText::new(dev_label).monospace().color(
                if hud_state.dev_overlay_open {
                    ACCENT_GREEN
                } else {
                    TEXT_DIM
                },
            ));
            if dev_btn.clicked() {
                hud_state.dev_overlay_open = !hud_state.dev_overlay_open;
            }
        });
    });
}

fn status_pill(
    ui: &mut egui::Ui,
    cfg: &GameConfig,
    domination: &crate::plugins::game_state::DominationState,
) {
    use swarm_model::Faction as F;
    let (title, detail, color) = match domination.faction {
        Some(F::Player) => (
            "Player domination",
            format!(
                "{:.1}s to victory",
                domination.remaining(cfg).unwrap_or_default()
            ),
            ACCENT_GREEN,
        ),
        Some(F::Enemy) => (
            "Enemy domination",
            format!(
                "{:.1}s to defeat",
                domination.remaining(cfg).unwrap_or_default()
            ),
            egui::Color32::from_rgb(255, 118, 118),
        ),
        Some(F::Neutral) | None => (
            "Secure every base",
            "Dominate to win.".to_owned(),
            ACCENT_CYAN,
        ),
    };

    egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(4, 12, 20, 140))
        .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.6)))
        .corner_radius(10.0)
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let _ = glyph_widget(ui, SharedGlyph::CommandWaypoint, color, 14.0);
                ui.label(egui::RichText::new(title).strong().color(color));
                ui.label(egui::RichText::new(detail).small().color(TEXT_DIM));
            });
        });
}

fn resource_pill(
    ui: &mut egui::Ui,
    label: &str,
    value: &str,
    accent: egui::Color32,
    glyph: Option<SharedGlyph>,
) {
    egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(7, 20, 31, 96))
        .stroke(egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(124, 234, 255, 28),
        ))
        .corner_radius(999.0)
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some(glyph) = glyph {
                    let _ = glyph_widget(ui, glyph, accent, 13.0);
                }
                ui.label(egui::RichText::new(label).small().color(TEXT_DIM));
                ui.label(
                    egui::RichText::new(value)
                        .strong()
                        .monospace()
                        .color(accent),
                );
            });
        });
}

fn beacon_pill(ui: &mut egui::Ui, beacon_state: &crate::plugins::beacons::BeaconControlState) {
    let mode = match beacon_state.mode {
        BeaconMode::Attract => "Attract",
        BeaconMode::Repel => "Repel",
        BeaconMode::Focus => "Focus",
    };
    let detail = if beacon_state.is_strengthened() {
        format!("{mode} · boost {:.1}s", beacon_state.strengthen_remaining)
    } else if beacon_state.mode_cooldown_remaining > 0.0 {
        format!("{mode} · cool {:.1}s", beacon_state.mode_cooldown_remaining)
    } else {
        format!("{mode} · ready")
    };
    let accent = if beacon_state.is_strengthened() {
        ACCENT_GREEN
    } else {
        ACCENT_CYAN
    };
    egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(7, 20, 31, 104))
        .stroke(egui::Stroke::new(1.0, accent))
        .corner_radius(999.0)
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let _ = glyph_widget(ui, SharedGlyph::CaptureBeacon, accent, 13.0);
                ui.label(egui::RichText::new("Beacon").small().color(TEXT_DIM));
                ui.label(egui::RichText::new(detail).small().strong().color(accent));
            });
        });
}
