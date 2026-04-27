use bevy::prelude::*;
use bevy_egui::egui;
use swarm_combat::{ArmyCapacityUpgrades, Biomass};
use swarm_model::{Base, Faction, GameConfig, Unit};

use super::super::selection::{
    base_is_contested, validate_base_repair, validate_base_upgrade, validate_global_expansion,
};
use super::super::state::{HudState, PendingHudAction};
use super::super::theme::{
    ACCENT_CYAN, ACCENT_GREEN, TEXT_DIM, faction_color_ui, faction_name, section_frame,
    show_notice, show_small_warning,
};
use super::confirmation_strip::{
    ConfirmationStrip, ConfirmationStripEvent, show_confirmation_strip,
};
use crate::plugins::hud::HudUiAction;

const SHARED_SLOT_BUSY_REASON: &str = "Another action is already using the shared slot.";

#[allow(clippy::type_complexity)]
pub(super) fn show_embedded_resource_cluster(
    ui: &mut egui::Ui,
    cfg: &GameConfig,
    biomass: &Biomass,
    upgrades: &ArmyCapacityUpgrades,
    hud_state: &mut HudState,
    base_query: &Query<(Entity, &Base, &Transform)>,
    actions: &mut MessageWriter<HudUiAction>,
) {
    show_dock_header(ui, cfg, biomass, upgrades, hud_state, base_query);

    if let Some(notice) = hud_state.notice() {
        ui.add_space(8.0);
        show_notice(ui, notice);
    }

    ui.add_space(8.0);
    show_pending_slot_strip(ui, hud_state);
    ui.add_space(10.0);
    show_global_expansion_section(ui, cfg, biomass, upgrades, hud_state, actions);
}

pub(super) fn show_embedded_selected_base_panel(
    ui: &mut egui::Ui,
    cfg: &GameConfig,
    biomass: &Biomass,
    unit_positions: &Query<(&Transform, &Unit)>,
    hud_state: &mut HudState,
    base_query: &Query<(Entity, &Base, &Transform)>,
    actions: &mut MessageWriter<HudUiAction>,
) {
    show_selected_base_section(
        ui,
        cfg,
        biomass,
        unit_positions,
        hud_state,
        base_query,
        actions,
    );
}

fn show_dock_header(
    ui: &mut egui::Ui,
    cfg: &GameConfig,
    biomass: &Biomass,
    upgrades: &ArmyCapacityUpgrades,
    hud_state: &HudState,
    base_query: &Query<(Entity, &Base, &Transform)>,
) {
    let (selection_label, selection_tint) = hud_state
        .selected_base()
        .and_then(|entity| {
            base_query.get(entity).ok().map(|(_, base, _)| {
                (
                    format!("{} lvl {}", faction_name(base.faction), base.level),
                    faction_color_ui(base.faction),
                )
            })
        })
        .unwrap_or_else(|| ("No base".into(), ACCENT_CYAN));
    let slot_label = hud_state
        .pending_action()
        .map(|action| format!("Armed · {}", action.label()))
        .unwrap_or_else(|| "Idle".into());
    let slot_tint = if hud_state.pending_action().is_some() {
        ACCENT_GREEN
    } else {
        ACCENT_CYAN
    };
    let player_cap = upgrades.faction_unit_cap(&Faction::Player, cfg);

    ui.horizontal_wrapped(|ui| {
        status_chip(
            ui,
            "Evolution",
            &format!("{:.0}", biomass.player),
            egui::Color32::from_rgba_unmultiplied(124, 234, 255, 24),
            ACCENT_CYAN,
        );
        status_chip(
            ui,
            "Cap",
            &format!("{}", player_cap),
            egui::Color32::from_rgba_unmultiplied(173, 255, 47, 18),
            ACCENT_GREEN,
        );
        status_chip(
            ui,
            "Selection",
            &selection_label,
            egui::Color32::from_rgba_unmultiplied(90, 244, 190, 24),
            selection_tint,
        );
        status_chip(
            ui,
            "Slot",
            &slot_label,
            egui::Color32::from_rgba_unmultiplied(173, 255, 47, 18),
            slot_tint,
        );
    });
}

fn show_pending_slot_strip(ui: &mut egui::Ui, hud_state: &HudState) {
    let (headline, detail, fill, stroke) = match hud_state.pending_action() {
        Some(action) => (
            "Slot armed",
            format!("{} is queued. Confirm it below or cancel.", action.label()),
            egui::Color32::from_rgba_unmultiplied(19, 43, 30, 132),
            egui::Stroke::new(1.0, ACCENT_GREEN),
        ),
        None => (
            "Slot open",
            "Prime one base action or expansion at a time.".into(),
            egui::Color32::from_rgba_unmultiplied(13, 24, 34, 132),
            egui::Stroke::new(
                1.0,
                egui::Color32::from_rgba_unmultiplied(124, 234, 255, 68),
            ),
        ),
    };

    egui::Frame::new()
        .fill(fill)
        .stroke(stroke)
        .corner_radius(12.0)
        .inner_margin(egui::Margin::symmetric(12, 9))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(headline).strong().color(ACCENT_GREEN));
            ui.label(egui::RichText::new(detail).small().color(TEXT_DIM));
        });
}

fn show_global_expansion_card(
    ui: &mut egui::Ui,
    cfg: &GameConfig,
    biomass: &Biomass,
    upgrades: &ArmyCapacityUpgrades,
    hud_state: &mut HudState,
    actions: &mut MessageWriter<HudUiAction>,
) {
    let global_expansion_action = PendingHudAction::GlobalExpansion;
    let global_expansion_status = validate_global_expansion(cfg, biomass, upgrades);
    let global_expansion_pending = hud_state.pending_action() == Some(global_expansion_action);
    let global_expansion_blocked_by_other =
        hud_state.pending_action().is_some() && !global_expansion_pending;
    let current_cap = upgrades.faction_unit_cap(&Faction::Player, cfg);
    let next_tier = upgrades.player_tier.saturating_add(1);
    let next_cap = cfg.global_unit_cap_for_tier(next_tier);

    command_card_frame(global_expansion_pending).show(ui, |ui| {
        ui.label(egui::RichText::new("Expansion lattice").strong());
        ui.horizontal_wrapped(|ui| {
            status_chip(
                ui,
                "Cap",
                &format!("{}", current_cap),
                egui::Color32::from_rgba_unmultiplied(124, 234, 255, 20),
                ACCENT_CYAN,
            );
            status_chip(
                ui,
                "Next tier",
                &format!("{}", next_tier),
                egui::Color32::from_rgba_unmultiplied(173, 255, 47, 18),
                ACCENT_GREEN,
            );
            if global_expansion_status.is_ok() {
                status_chip(
                    ui,
                    "Projected",
                    &format!("{}", next_cap),
                    egui::Color32::from_rgba_unmultiplied(173, 255, 47, 18),
                    ACCENT_GREEN,
                );
            }
        });
        ui.add_space(6.0);
        match &global_expansion_status {
            Ok(cost) => ui.monospace(format!(
                "Cost: {:.0} biomass · unlocks {} unit cap",
                cost, next_cap
            )),
            Err(reason) => ui.monospace(format!("Requirement: {reason}")),
        };
        ui.add_space(6.0);
        match show_confirmation_strip(
            ui,
            ConfirmationStrip {
                pending: global_expansion_pending,
                pending_title: "Expansion primed",
                pending_detail: "Commit the cap boost or cancel to free the slot.",
                prepare_label: "Prepare expansion",
                confirm_label: "Confirm expansion",
                disabled_reason: action_disabled_reason(
                    &global_expansion_status,
                    global_expansion_blocked_by_other,
                ),
            },
        ) {
            Some(ConfirmationStripEvent::Prepare) => hud_state.arm_action(global_expansion_action),
            Some(ConfirmationStripEvent::Confirm) => {
                actions.write(HudUiAction::ConfirmGlobalExpansion);
                hud_state.resolve_pending();
            }
            Some(ConfirmationStripEvent::Cancel) => hud_state.clear_pending(),
            None => {}
        }
    });
}

fn show_global_expansion_section(
    ui: &mut egui::Ui,
    cfg: &GameConfig,
    biomass: &Biomass,
    upgrades: &ArmyCapacityUpgrades,
    hud_state: &mut HudState,
    actions: &mut MessageWriter<HudUiAction>,
) {
    section_frame().show(ui, |ui| {
        ui.label(egui::RichText::new("Global").strong().color(ACCENT_GREEN));
        ui.add_space(6.0);
        show_global_expansion_card(ui, cfg, biomass, upgrades, hud_state, actions);
    });
}

fn show_selected_base_actions(
    ui: &mut egui::Ui,
    cfg: &GameConfig,
    biomass: &Biomass,
    unit_positions: &Query<(&Transform, &Unit)>,
    hud_state: &mut HudState,
    base_query: &Query<(Entity, &Base, &Transform)>,
    actions: &mut MessageWriter<HudUiAction>,
) {
    let Some(selected_entity) = hud_state.selected_base() else {
        show_empty_selection(ui);
        return;
    };

    let Ok((entity, base, base_tf)) = base_query.get(selected_entity) else {
        hud_state.drop_missing_selection();
        show_empty_selection(ui);
        return;
    };

    let base_pos = base_tf.translation.truncate();
    let contested = base_is_contested(base, base_pos, unit_positions);

    egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(10, 26, 38, 136))
        .corner_radius(12.0)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(format!(
                    "{} node · level {}",
                    faction_name(base.faction),
                    base.level
                ))
                .strong()
                .color(faction_color_ui(base.faction)),
            );
            ui.label(
                egui::RichText::new(format!(
                    "Integrity {:.0}/{:.0} · spawn {:.1}s",
                    base.health, base.max_health, base.spawn_interval
                ))
                .small()
                .color(TEXT_DIM),
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                status_chip(
                    ui,
                    "Contested",
                    if contested { "Yes" } else { "No" },
                    egui::Color32::from_rgba_unmultiplied(124, 234, 255, 18),
                    ACCENT_CYAN,
                );
                status_chip(
                    ui,
                    "Weak",
                    if base.is_weak() {
                        "Recovering"
                    } else {
                        "Stable"
                    },
                    egui::Color32::from_rgba_unmultiplied(173, 255, 47, 18),
                    ACCENT_GREEN,
                );
                if base.repair_cooldown > 0.0 {
                    status_chip(
                        ui,
                        "Repair",
                        &format!("{:.1}s", base.repair_cooldown),
                        egui::Color32::from_rgba_unmultiplied(255, 140, 140, 18),
                        egui::Color32::from_rgb(255, 186, 186),
                    );
                }
            });
        });

    ui.add_space(8.0);

    if base.faction != Faction::Player {
        ui.label(
            egui::RichText::new("Only player bases can route dock commands.")
                .small()
                .color(TEXT_DIM),
        );
        return;
    }

    let upgrade_action = PendingHudAction::UpgradeBase(entity);
    let repair_action = PendingHudAction::RepairBase(entity);
    let repair_pending = hud_state.pending_action() == Some(repair_action);
    let another_action_pending = matches!(
        hud_state.pending_action(),
        Some(action) if action != upgrade_action && action != repair_action
    );

    let upgrade_status = validate_base_upgrade(base, base_pos, cfg, biomass, unit_positions);
    let repair_status = validate_base_repair(base, cfg, biomass);

    command_card_frame(false).show(ui, |ui| {
        ui.label(egui::RichText::new("Fortify node").strong());
        match &upgrade_status {
            Ok(cost) => ui.monospace(format!(
                "Cost: {:.0} biomass · next level {}",
                cost,
                base.level + 1
            )),
            Err(reason) => ui.monospace(format!("Requirement: {reason}")),
        };
        ui.add_space(6.0);
        let disabled_reason = upgrade_status.as_ref().err().map(|reason| reason.as_str());
        let button = ui.add_enabled(
            disabled_reason.is_none(),
            egui::Button::new(egui::RichText::new("Upgrade base").strong()),
        );
        if button.clicked() {
            actions.write(HudUiAction::ConfirmBaseUpgrade(entity));
        }
        if let Some(reason) = disabled_reason {
            show_small_warning(ui, reason);
        }
    });

    ui.add_space(8.0);

    command_card_frame(repair_pending).show(ui, |ui| {
        ui.label(egui::RichText::new("Stabilize node").strong());
        match &repair_status {
            Ok(preview) => {
                ui.monospace(format!(
                    "Cost: {:.0} biomass · restore {:.0} hp",
                    preview.cost, preview.amount
                ));
            }
            Err(reason) => {
                ui.monospace(format!("Requirement: {reason}"));
            }
        };
        ui.add_space(6.0);

        match show_confirmation_strip(
            ui,
            ConfirmationStrip {
                pending: repair_pending,
                pending_title: "Repair primed",
                pending_detail: "Commit the repair pulse or cancel to free the slot.",
                prepare_label: "Prepare repair",
                confirm_label: "Confirm repair",
                disabled_reason: action_disabled_reason(&repair_status, another_action_pending),
            },
        ) {
            Some(ConfirmationStripEvent::Prepare) => hud_state.arm_action(repair_action),
            Some(ConfirmationStripEvent::Confirm) => {
                actions.write(HudUiAction::ConfirmBaseRepair(entity));
                hud_state.resolve_pending();
            }
            Some(ConfirmationStripEvent::Cancel) => hud_state.clear_pending(),
            None => {}
        }
    });
}

fn show_selected_base_section(
    ui: &mut egui::Ui,
    cfg: &GameConfig,
    biomass: &Biomass,
    unit_positions: &Query<(&Transform, &Unit)>,
    hud_state: &mut HudState,
    base_query: &Query<(Entity, &Base, &Transform)>,
    actions: &mut MessageWriter<HudUiAction>,
) {
    section_frame().show(ui, |ui| {
        ui.label(
            egui::RichText::new("Selected base")
                .strong()
                .color(ACCENT_CYAN),
        );
        ui.add_space(6.0);
        show_selected_base_actions(
            ui,
            cfg,
            biomass,
            unit_positions,
            hud_state,
            base_query,
            actions,
        );
    });
}

fn show_empty_selection(ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(9, 21, 31, 210))
        .corner_radius(12.0)
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("No base selected")
                    .strong()
                    .color(ACCENT_CYAN),
            );
            ui.label(
                egui::RichText::new(
                    "Click a battlefield base to inspect it and issue node commands nearby.",
                )
                .small()
                .color(TEXT_DIM),
            );
        });
}

fn status_chip(
    ui: &mut egui::Ui,
    label: &str,
    value: &str,
    fill: egui::Color32,
    text_color: egui::Color32,
) {
    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0, fill.gamma_multiply(2.0)))
        .corner_radius(999.0)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(label).small().color(TEXT_DIM));
                ui.label(egui::RichText::new(value).strong().color(text_color));
            });
        });
}

fn command_card_frame(active: bool) -> egui::Frame {
    let stroke = if active {
        egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(173, 255, 47, 112),
        )
    } else {
        egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(124, 234, 255, 36),
        )
    };

    let fill = if active {
        egui::Color32::from_rgba_unmultiplied(12, 26, 28, 220)
    } else {
        egui::Color32::from_rgba_unmultiplied(10, 22, 32, 220)
    };

    egui::Frame::new()
        .fill(fill)
        .stroke(stroke)
        .corner_radius(12.0)
        .inner_margin(egui::Margin::symmetric(12, 10))
}

fn action_disabled_reason<T>(status: &Result<T, String>, blocked_by_other: bool) -> Option<&str> {
    status
        .as_ref()
        .err()
        .map(|reason| reason.as_str())
        .or_else(|| blocked_by_other.then_some(SHARED_SLOT_BUSY_REASON))
}
