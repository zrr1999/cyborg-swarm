use bevy_egui::egui;

use crate::shared_assets::{SharedGlyph, glyph_widget};

pub(super) fn section_label(
    ui: &mut egui::Ui,
    glyph: SharedGlyph,
    tint: egui::Color32,
    text: &str,
) {
    ui.horizontal(|ui| {
        let _ = glyph_widget(ui, glyph, tint, 14.0);
        ui.label(egui::RichText::new(text).small().strong().color(tint));
    });
}
