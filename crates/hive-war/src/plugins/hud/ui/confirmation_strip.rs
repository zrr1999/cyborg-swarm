use bevy_egui::egui;

use super::super::theme::{ACCENT_GREEN, TEXT_DIM, show_small_warning};

pub(super) struct ConfirmationStrip<'a> {
    pub(super) pending: bool,
    pub(super) pending_title: &'a str,
    pub(super) pending_detail: &'a str,
    pub(super) prepare_label: &'a str,
    pub(super) confirm_label: &'a str,
    pub(super) disabled_reason: Option<&'a str>,
}

pub(super) enum ConfirmationStripEvent {
    Prepare,
    Confirm,
    Cancel,
}

pub(super) fn show_confirmation_strip(
    ui: &mut egui::Ui,
    strip: ConfirmationStrip<'_>,
) -> Option<ConfirmationStripEvent> {
    if strip.pending {
        let mut event = None;

        egui::Frame::new()
            .fill(egui::Color32::from_rgba_unmultiplied(19, 43, 30, 116))
            .stroke(egui::Stroke::new(
                1.0_f32,
                egui::Color32::from_rgba_unmultiplied(173, 255, 47, 112),
            ))
            .corner_radius(10.0)
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(strip.pending_title)
                                .strong()
                                .color(ACCENT_GREEN),
                        );
                        ui.label(
                            egui::RichText::new(strip.pending_detail)
                                .small()
                                .color(TEXT_DIM),
                        );
                    });

                    ui.add_space(8.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Cancel").clicked() {
                            event = Some(ConfirmationStripEvent::Cancel);
                        }
                        if ui.button(strip.confirm_label).clicked() {
                            event = Some(ConfirmationStripEvent::Confirm);
                        }
                    });
                });
            });

        return event;
    }

    let enabled = strip.disabled_reason.is_none();
    let response = ui.add_enabled(enabled, egui::Button::new(strip.prepare_label));
    if let Some(reason) = strip.disabled_reason {
        ui.add_space(4.0);
        show_small_warning(ui, reason);
    }

    response
        .clicked()
        .then_some(ConfirmationStripEvent::Prepare)
}
