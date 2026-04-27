use bevy::prelude::*;
use bevy_egui::egui;
use swarm_combat::{ArmyCapacityUpgrades, Biomass};
use swarm_model::{Base, Faction, GameConfig, Unit};

use super::super::selection::{
    base_is_contested, validate_base_repair, validate_base_upgrade, validate_global_expansion,
};
use super::super::state::{HudState, PendingHudAction};
use super::super::theme::{
    ACCENT_CYAN, ACCENT_GREEN, TEXT_DIM, WARNING_TEXT, command_card_frame, faction_color_ui,
    faction_name, show_notice, show_small_warning,
};
use super::confirmation_strip::{
    ConfirmationStrip, ConfirmationStripEvent, show_confirmation_strip,
};
use super::shared_visuals::section_label;
use crate::plugins::hud::HudUiAction;
use crate::shared_assets::{SharedGlyph, glyph_widget};

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn show_bottom_bar(
    ui: &mut egui::Ui,
    cfg: &GameConfig,
    biomass: &Biomass,
    upgrades: &ArmyCapacityUpgrades,
    unit_positions: &Query<(&Transform, &Unit)>,
    hud_state: &mut HudState,
    base_query: &Query<(Entity, &Base, &Transform)>,
    actions: &mut MessageWriter<HudUiAction>,
) {
    let available_width = ui.available_width();
    let col_width = (available_width / 2.0 - 16.0).max(220.0);

    ui.horizontal_top(|ui| {
        // Left: selected base panel
        ui.vertical(|ui| {
            ui.set_width(col_width);
            show_selected_base_column(
                ui,
                cfg,
                biomass,
                unit_positions,
                hud_state,
                base_query,
                actions,
            );
        });

        ui.separator();

        // Right: global expansion + slot status
        ui.vertical(|ui| {
            ui.set_width(col_width);
            show_global_column(ui, cfg, biomass, upgrades, hud_state, actions);
        });
    });
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn show_selected_base_column(
    ui: &mut egui::Ui,
    cfg: &GameConfig,
    biomass: &Biomass,
    unit_positions: &Query<(&Transform, &Unit)>,
    hud_state: &mut HudState,
    base_query: &Query<(Entity, &Base, &Transform)>,
    actions: &mut MessageWriter<HudUiAction>,
) {
    section_label(
        ui,
        SharedGlyph::CommandWaypoint,
        ACCENT_CYAN,
        "Selected Base",
    );
    ui.add_space(2.0);

    let Some(selected_entity) = hud_state.selected_base() else {
        ui.label(
            egui::RichText::new("Click a base in the battlefield to inspect and command it.")
                .small()
                .color(TEXT_DIM),
        );
        if let Some(notice) = hud_state.notice() {
            ui.add_space(4.0);
            show_notice(ui, notice);
        }
        return;
    };

    let Ok((entity, base, base_tf)) = base_query.get(selected_entity) else {
        hud_state.drop_missing_selection();
        ui.label(
            egui::RichText::new("Selection lost.")
                .small()
                .color(TEXT_DIM),
        );
        return;
    };

    let base_pos = base_tf.translation.truncate();
    let contested = base_is_contested(base, base_pos, unit_positions);
    let faction_color = faction_color_ui(base.faction);
    let health_pct = base.health / base.max_health.max(1.0);

    // Base info row
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!(
                "{} · Lvl {} · {:.0}/{:.0} HP",
                faction_name(base.faction),
                base.level,
                base.health,
                base.max_health,
            ))
            .strong()
            .color(faction_color),
        );
        if contested {
            let alert = egui::Color32::from_rgb(255, 186, 100);
            let _ = glyph_widget(ui, SharedGlyph::AlertPing, alert, 12.0);
            ui.label(egui::RichText::new("Contested").small().color(alert));
        }
        if base.is_weak() {
            ui.label(
                egui::RichText::new("⟳ Weak")
                    .small()
                    .color(egui::Color32::from_rgb(220, 180, 100)),
            );
        }
    });

    // HP bar
    ui.add(
        egui::ProgressBar::new(health_pct)
            .desired_width(col_width_hint())
            .fill(egui::Color32::from_rgba_unmultiplied(
                faction_color.r(),
                faction_color.g(),
                faction_color.b(),
                120,
            )),
    );

    if let Some(notice) = hud_state.notice() {
        show_notice(ui, notice);
        return;
    }

    if base.faction != Faction::Player {
        ui.label(
            egui::RichText::new("Enemy/neutral node — no commands available.")
                .small()
                .color(TEXT_DIM),
        );
        return;
    }

    // Action buttons
    let upgrade_action = PendingHudAction::UpgradeBase(entity);
    let repair_action = PendingHudAction::RepairBase(entity);
    let repair_pending = hud_state.pending_action() == Some(repair_action);
    let another_pending = matches!(
        hud_state.pending_action(),
        Some(a) if a != upgrade_action && a != repair_action
    );

    let upgrade_status = validate_base_upgrade(base, base_pos, cfg, biomass, unit_positions);
    let repair_status = validate_base_repair(base, cfg, biomass);

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        // Upgrade button (instant, no confirmation)
        command_card_frame(false).show(ui, |ui| {
            ui.set_min_width(130.0);
            match &upgrade_status {
                Ok(cost) => {
                    ui.label(
                        egui::RichText::new(format!(
                            "Upgrade  {:.0}⚡  → Lvl {}",
                            cost,
                            base.level + 1
                        ))
                        .small()
                        .strong(),
                    );
                    if ui
                        .add_enabled(true, egui::Button::new("Upgrade Node"))
                        .clicked()
                    {
                        actions.write(HudUiAction::ConfirmBaseUpgrade(entity));
                    }
                }
                Err(reason) => {
                    ui.label(egui::RichText::new("Upgrade Node").small().strong());
                    show_small_warning(ui, reason);
                }
            }
        });

        ui.add_space(4.0);

        // Repair section with confirmation
        command_card_frame(repair_pending).show(ui, |ui| {
            ui.set_min_width(130.0);
            match &repair_status {
                Ok(preview) => {
                    ui.label(
                        egui::RichText::new(format!(
                            "Repair  {:.0}⚡  +{:.0} HP",
                            preview.cost, preview.amount
                        ))
                        .small()
                        .strong(),
                    );
                }
                Err(reason) => {
                    ui.label(egui::RichText::new("Repair Node").small().strong());
                    show_small_warning(ui, reason);
                }
            }
            if base.repair_cooldown > 0.0 {
                ui.label(
                    egui::RichText::new(format!("Cooldown {:.1}s", base.repair_cooldown))
                        .small()
                        .color(TEXT_DIM),
                );
            }
            let disabled_reason = repair_status
                .as_ref()
                .err()
                .map(|s| s.as_str())
                .or_else(|| another_pending.then_some("Slot busy — cancel other action first."));
            match show_confirmation_strip(
                ui,
                ConfirmationStrip {
                    pending: repair_pending,
                    pending_title: "Repair primed",
                    pending_detail: "Commit or cancel.",
                    prepare_label: "Repair Node",
                    confirm_label: "Confirm repair",
                    disabled_reason,
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
    });
}

fn show_global_column(
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
    let global_expansion_blocked =
        hud_state.pending_action().is_some() && !global_expansion_pending;
    let current_cap = upgrades.faction_unit_cap(&Faction::Player, cfg);
    let next_tier = upgrades.player_tier.saturating_add(1);
    let next_cap = cfg.global_unit_cap_for_tier(next_tier);

    section_label(
        ui,
        SharedGlyph::SupplyCrate,
        ACCENT_GREEN,
        "Global Expansion",
    );
    ui.add_space(2.0);

    command_card_frame(global_expansion_pending).show(ui, |ui| {
        ui.horizontal(|ui| match &global_expansion_status {
            Ok(cost) => {
                ui.label(
                    egui::RichText::new(format!(
                        "Tier {} → {}  ·  Cap {} → {}  ·  Cost {:.0}⚡",
                        upgrades.player_tier, next_tier, current_cap, next_cap, cost
                    ))
                    .small()
                    .color(TEXT_DIM),
                );
            }
            Err(reason) => {
                ui.label(
                    egui::RichText::new(format!("Cap {current_cap} · {reason}"))
                        .small()
                        .color(WARNING_TEXT),
                );
            }
        });

        ui.add_space(4.0);

        let disabled_reason = global_expansion_status
            .as_ref()
            .err()
            .map(|s| s.as_str())
            .or_else(|| {
                global_expansion_blocked.then_some("Slot busy — cancel other action first.")
            });

        match show_confirmation_strip(
            ui,
            ConfirmationStrip {
                pending: global_expansion_pending,
                pending_title: "Expansion primed",
                pending_detail: "Confirm to boost cap.",
                prepare_label: "Prime Expansion",
                confirm_label: "Confirm expansion",
                disabled_reason,
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

    // Slot status
    ui.add_space(6.0);
    let (slot_text, slot_color) = match hud_state.pending_action() {
        Some(action) => (format!("● Slot: {} queued", action.label()), ACCENT_GREEN),
        None => ("○ Slot: Open".to_owned(), TEXT_DIM),
    };
    ui.label(egui::RichText::new(slot_text).small().color(slot_color));
}

/// Hint width so progress bars don't stretch to full column width.
fn col_width_hint() -> f32 {
    200.0
}
