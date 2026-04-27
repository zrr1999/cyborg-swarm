use bevy_egui::egui;
use swarm_combat::Biomass;
use swarm_model::{Faction, GameConfig};

use super::super::theme::{ACCENT_CYAN, ACCENT_GREEN, TEXT_DIM};
use crate::components::BeaconMode;

struct RibbonStatus {
    title: String,
    detail: String,
    color: egui::Color32,
}

pub(super) fn show_objective_ribbon(
    ui: &mut egui::Ui,
    cfg: &GameConfig,
    biomass: &Biomass,
    domination: &crate::plugins::game_state::DominationState,
    beacon_state: &crate::plugins::beacons::BeaconControlState,
) {
    let status = ribbon_status(cfg, domination);

    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);

    ui.horizontal(|ui| {
        status_pill(ui, &status);
        resource_pill(
            ui,
            "Biomass",
            &format!("{:.0}", biomass.player),
            ACCENT_CYAN,
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            beacon_pill(ui, beacon_state);
        });
    });
}

fn ribbon_status(
    cfg: &GameConfig,
    domination: &crate::plugins::game_state::DominationState,
) -> RibbonStatus {
    match domination.faction {
        Some(Faction::Player) => RibbonStatus {
            title: "Player domination".into(),
            detail: format!(
                "{:.1}s to victory",
                domination.remaining(cfg).unwrap_or_default()
            ),
            color: ACCENT_GREEN,
        },
        Some(Faction::Enemy) => RibbonStatus {
            title: "Enemy domination".into(),
            detail: format!(
                "{:.1}s to defeat",
                domination.remaining(cfg).unwrap_or_default()
            ),
            color: egui::Color32::from_rgb(255, 118, 118),
        },
        Some(Faction::Neutral) | None => RibbonStatus {
            title: "Secure every base".into(),
            detail: "Break the enemy before it completes total domination.".into(),
            color: ACCENT_CYAN,
        },
    }
}

fn status_pill(ui: &mut egui::Ui, status: &RibbonStatus) {
    egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(4, 12, 20, 140))
        .stroke(egui::Stroke::new(1.0, status.color.gamma_multiply(0.6)))
        .corner_radius(10.0)
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new(&status.title)
                        .strong()
                        .color(status.color),
                );
                ui.label(egui::RichText::new(&status.detail).small().color(TEXT_DIM));
            });
        });
}

fn resource_pill(ui: &mut egui::Ui, label: &str, value: &str, accent: egui::Color32) {
    egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(7, 20, 31, 96))
        .stroke(egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(124, 234, 255, 28),
        ))
        .corner_radius(999.0)
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
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
        .inner_margin(egui::Margin::symmetric(10, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Beacon").small().color(TEXT_DIM));
                ui.label(egui::RichText::new(detail).small().strong().color(accent));
            });
        });
}
