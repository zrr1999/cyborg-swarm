use bevy::prelude::Color;
use bevy_egui::egui;
use swarm_model::Faction;

pub(super) const ACCENT_CYAN: egui::Color32 = egui::Color32::from_rgb(124, 234, 255);
pub(super) const ACCENT_GREEN: egui::Color32 = egui::Color32::from_rgb(173, 255, 47);
pub(super) const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(151, 188, 196);
pub(super) const WARNING_TEXT: egui::Color32 = egui::Color32::from_rgb(255, 221, 221);

#[allow(dead_code)]
fn panel_alt_bg() -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(11, 24, 36, 148)
}

fn warning_bg() -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(94, 27, 27, 176)
}

pub(super) fn top_bar_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(6, 14, 22, 220))
        .stroke(egui::Stroke::new(
            1.0_f32,
            egui::Color32::from_rgba_unmultiplied(68, 212, 224, 36),
        ))
        .inner_margin(egui::Margin::symmetric(12, 6))
}

pub(super) fn bottom_bar_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(6, 14, 22, 220))
        .stroke(egui::Stroke::new(
            1.0_f32,
            egui::Color32::from_rgba_unmultiplied(68, 212, 224, 36),
        ))
        .inner_margin(egui::Margin::symmetric(12, 8))
}

/// Used by floating overlay windows (intel, dev).
#[allow(dead_code)]
pub(super) fn panel_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(8, 18, 30, 200))
        .stroke(egui::Stroke::new(
            1.0_f32,
            egui::Color32::from_rgba_unmultiplied(68, 212, 224, 50),
        ))
        .corner_radius(14.0)
        .inner_margin(egui::Margin::symmetric(12, 10))
}

#[allow(dead_code)]
pub(super) fn section_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(panel_alt_bg())
        .stroke(egui::Stroke::new(
            1.0_f32,
            egui::Color32::from_rgba_unmultiplied(173, 255, 47, 40),
        ))
        .corner_radius(12.0)
        .inner_margin(egui::Margin::symmetric(12, 10))
}

pub(super) fn command_card_frame(active: bool) -> egui::Frame {
    let stroke = if active {
        egui::Stroke::new(
            1.0_f32,
            egui::Color32::from_rgba_unmultiplied(173, 255, 47, 112),
        )
    } else {
        egui::Stroke::new(
            1.0_f32,
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
        .corner_radius(10.0)
        .inner_margin(egui::Margin::symmetric(10, 8))
}

pub(super) fn show_notice(ui: &mut egui::Ui, notice: &super::state::HudNotice) {
    egui::Frame::new()
        .fill(warning_bg())
        .corner_radius(10.0)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(&notice.text)
                    .color(WARNING_TEXT)
                    .small(),
            );
        });
}

pub(super) fn show_small_warning(ui: &mut egui::Ui, reason: &str) {
    ui.label(
        egui::RichText::new(reason)
            .small()
            .color(egui::Color32::from_rgb(255, 186, 186)),
    );
}

pub(super) fn faction_name(faction: Faction) -> &'static str {
    match faction {
        Faction::Player => "Player",
        Faction::Enemy => "Enemy",
        Faction::Neutral => "Neutral",
    }
}

pub(super) fn faction_color_ui(faction: Faction) -> egui::Color32 {
    match faction {
        Faction::Player => egui::Color32::from_rgb(86, 244, 190),
        Faction::Enemy => egui::Color32::from_rgb(255, 109, 109),
        Faction::Neutral => egui::Color32::from_rgb(167, 177, 186),
    }
}

pub(super) fn faction_color_world(faction: Faction) -> Color {
    match faction {
        Faction::Player => Color::srgba(0.35, 0.92, 0.55, 0.95),
        Faction::Enemy => Color::srgba(0.95, 0.35, 0.35, 0.95),
        Faction::Neutral => Color::srgba(0.75, 0.75, 0.8, 0.95),
    }
}
