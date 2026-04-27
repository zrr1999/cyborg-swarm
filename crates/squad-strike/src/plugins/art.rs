use std::f32::consts::{FRAC_PI_4, TAU};

use bevy_egui::egui;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudIcon {
    EnergyCell,
    HealthOrb,
    RepairKit,
    SpeedBoost,
    ShieldEmitter,
    SalvageScrap,
    EliteTarget,
    WaveAlert,
    CaptainBadge,
}

pub fn icon_widget(ui: &mut egui::Ui, icon: Option<HudIcon>, size: f32) {
    let Some(icon) = icon else {
        return;
    };
    let desired = egui::vec2(size, size);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        paint_icon(ui.painter(), rect, icon);
    }
}

fn paint_icon(painter: &egui::Painter, rect: egui::Rect, icon: HudIcon) {
    let c = rect.center();
    let unit = rect.width().min(rect.height());
    let stroke = egui::Stroke::new((unit * 0.1).max(1.35), primary_color(icon));
    let detail = egui::Stroke::new((unit * 0.07).max(1.0), accent_color(icon));
    let fill = primary_color(icon).gamma_multiply(0.16);
    let r = unit * 0.33;

    match icon {
        HudIcon::EnergyCell => {
            painter.add(egui::Shape::convex_polygon(
                regular_polygon(c, r * 1.05, 6, std::f32::consts::FRAC_PI_6),
                fill,
                stroke,
            ));
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(c.x - r * 0.18, c.y - r * 0.78),
                    egui::pos2(c.x - r * 0.48, c.y - r * 0.02),
                    egui::pos2(c.x - r * 0.06, c.y - r * 0.02),
                    egui::pos2(c.x - r * 0.28, c.y + r * 0.78),
                    egui::pos2(c.x + r * 0.48, c.y - r * 0.12),
                    egui::pos2(c.x + r * 0.08, c.y - r * 0.12),
                ],
                accent_color(icon),
                egui::Stroke::NONE,
            ));
        }
        HudIcon::HealthOrb => {
            painter.circle_filled(c, r * 1.02, fill);
            painter.circle_stroke(c, r * 1.02, stroke);
            painter.line_segment(
                [
                    egui::pos2(c.x - r * 0.54, c.y),
                    egui::pos2(c.x + r * 0.54, c.y),
                ],
                detail,
            );
            painter.line_segment(
                [
                    egui::pos2(c.x, c.y - r * 0.54),
                    egui::pos2(c.x, c.y + r * 0.54),
                ],
                detail,
            );
        }
        HudIcon::RepairKit => {
            painter.add(egui::Shape::convex_polygon(
                rect_polygon(c, r * 1.2, r * 0.9),
                fill,
                stroke,
            ));
            painter.line_segment(
                [
                    egui::pos2(c.x - r * 0.55, c.y - r * 1.1),
                    egui::pos2(c.x + r * 0.55, c.y - r * 1.1),
                ],
                egui::Stroke::new(
                    (unit * 0.09).max(1.25),
                    egui::Color32::from_rgb(255, 194, 94),
                ),
            );
            painter.line_segment(
                [
                    egui::pos2(c.x - r * 0.52, c.y),
                    egui::pos2(c.x + r * 0.52, c.y),
                ],
                detail,
            );
            painter.line_segment(
                [
                    egui::pos2(c.x, c.y - r * 0.52),
                    egui::pos2(c.x, c.y + r * 0.52),
                ],
                detail,
            );
        }
        HudIcon::SpeedBoost => {
            painter.circle_stroke(c, r * 1.02, stroke);
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(c.x - r * 0.15, c.y - r * 0.55),
                    egui::pos2(c.x + r * 0.62, c.y),
                    egui::pos2(c.x - r * 0.15, c.y + r * 0.55),
                    egui::pos2(c.x - r * 0.05, c.y + r * 0.18),
                    egui::pos2(c.x - r * 0.62, c.y + r * 0.18),
                    egui::pos2(c.x - r * 0.62, c.y - r * 0.18),
                    egui::pos2(c.x - r * 0.05, c.y - r * 0.18),
                ],
                accent_color(icon),
                egui::Stroke::NONE,
            ));
            painter.line_segment(
                [
                    egui::pos2(c.x - r * 1.05, c.y - r * 0.58),
                    egui::pos2(c.x - r * 0.54, c.y - r * 0.58),
                ],
                egui::Stroke::new((unit * 0.08).max(1.0), primary_color(icon)),
            );
            painter.line_segment(
                [
                    egui::pos2(c.x - r * 1.18, c.y + r * 0.1),
                    egui::pos2(c.x - r * 0.62, c.y + r * 0.1),
                ],
                egui::Stroke::new((unit * 0.08).max(1.0), primary_color(icon)),
            );
        }
        HudIcon::ShieldEmitter => {
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(c.x, c.y - r * 1.22),
                    egui::pos2(c.x + r * 0.92, c.y - r * 0.52),
                    egui::pos2(c.x + r * 0.72, c.y + r * 0.52),
                    egui::pos2(c.x, c.y + r * 1.15),
                    egui::pos2(c.x - r * 0.72, c.y + r * 0.52),
                    egui::pos2(c.x - r * 0.92, c.y - r * 0.52),
                ],
                fill,
                stroke,
            ));
            painter.line_segment(
                [
                    egui::pos2(c.x - r * 0.42, c.y),
                    egui::pos2(c.x + r * 0.42, c.y),
                ],
                detail,
            );
            painter.line_segment(
                [
                    egui::pos2(c.x, c.y - r * 0.42),
                    egui::pos2(c.x, c.y + r * 0.42),
                ],
                detail,
            );
        }
        HudIcon::SalvageScrap => {
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(c.x - r * 0.86, c.y + r * 0.35),
                    egui::pos2(c.x - r * 0.42, c.y - r * 1.02),
                    egui::pos2(c.x + r * 0.52, c.y - r * 0.88),
                    egui::pos2(c.x + r * 0.96, c.y - r * 0.06),
                    egui::pos2(c.x + r * 0.28, c.y + r * 1.0),
                    egui::pos2(c.x - r * 0.64, c.y + r * 0.84),
                ],
                fill,
                stroke,
            ));
            painter.line_segment(
                [
                    egui::pos2(c.x - r * 0.3, c.y + r * 0.62),
                    egui::pos2(c.x + r * 0.5, c.y - r * 0.52),
                ],
                detail,
            );
        }
        HudIcon::EliteTarget => {
            painter.circle_stroke(c, r * 1.02, stroke);
            painter.circle_stroke(
                c,
                r * 0.5,
                egui::Stroke::new(
                    (unit * 0.07).max(1.0),
                    primary_color(icon).gamma_multiply(0.65),
                ),
            );
            painter.line_segment(
                [
                    egui::pos2(c.x, c.y - r * 1.45),
                    egui::pos2(c.x, c.y - r * 0.82),
                ],
                detail,
            );
            painter.line_segment(
                [
                    egui::pos2(c.x, c.y + r * 0.82),
                    egui::pos2(c.x, c.y + r * 1.45),
                ],
                detail,
            );
            painter.line_segment(
                [
                    egui::pos2(c.x - r * 1.45, c.y),
                    egui::pos2(c.x - r * 0.82, c.y),
                ],
                detail,
            );
            painter.line_segment(
                [
                    egui::pos2(c.x + r * 0.82, c.y),
                    egui::pos2(c.x + r * 1.45, c.y),
                ],
                detail,
            );
            painter.circle_filled(c, r * 0.18, accent_color(icon));
        }
        HudIcon::WaveAlert => {
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(c.x, c.y - r * 1.18),
                    egui::pos2(c.x + r * 1.0, c.y + r * 0.86),
                    egui::pos2(c.x - r * 1.0, c.y + r * 0.86),
                ],
                fill,
                stroke,
            ));
            painter.line_segment(
                [
                    egui::pos2(c.x, c.y - r * 0.45),
                    egui::pos2(c.x, c.y + r * 0.16),
                ],
                detail,
            );
            painter.circle_filled(
                egui::pos2(c.x, c.y + r * 0.48),
                r * 0.12,
                accent_color(icon),
            );
        }
        HudIcon::CaptainBadge => {
            painter.add(egui::Shape::convex_polygon(
                regular_polygon(c, r * 1.05, 4, FRAC_PI_4),
                fill,
                stroke,
            ));
            painter.add(egui::Shape::convex_polygon(
                regular_polygon(c, r * 0.42, 4, FRAC_PI_4),
                accent_color(icon),
                egui::Stroke::NONE,
            ));
        }
    }
}

fn primary_color(icon: HudIcon) -> egui::Color32 {
    match icon {
        HudIcon::EnergyCell => egui::Color32::from_rgb(86, 231, 255),
        HudIcon::HealthOrb => egui::Color32::from_rgb(128, 255, 124),
        HudIcon::RepairKit => egui::Color32::from_rgb(92, 224, 255),
        HudIcon::SpeedBoost => egui::Color32::from_rgb(255, 183, 74),
        HudIcon::ShieldEmitter => egui::Color32::from_rgb(114, 214, 255),
        HudIcon::SalvageScrap => egui::Color32::from_rgb(195, 208, 220),
        HudIcon::EliteTarget => egui::Color32::from_rgb(255, 188, 82),
        HudIcon::WaveAlert => egui::Color32::from_rgb(255, 129, 104),
        HudIcon::CaptainBadge => egui::Color32::from_rgb(255, 212, 94),
    }
}

fn accent_color(icon: HudIcon) -> egui::Color32 {
    match icon {
        HudIcon::EnergyCell => egui::Color32::from_rgb(155, 255, 82),
        HudIcon::HealthOrb => egui::Color32::from_rgb(245, 255, 248),
        HudIcon::RepairKit => egui::Color32::from_rgb(155, 255, 82),
        HudIcon::SpeedBoost => egui::Color32::from_rgb(255, 244, 216),
        HudIcon::ShieldEmitter => egui::Color32::from_rgb(245, 255, 248),
        HudIcon::SalvageScrap => egui::Color32::from_rgb(90, 231, 255),
        HudIcon::EliteTarget => egui::Color32::from_rgb(155, 255, 82),
        HudIcon::WaveAlert => egui::Color32::from_rgb(255, 243, 202),
        HudIcon::CaptainBadge => egui::Color32::from_rgb(255, 248, 223),
    }
}

fn regular_polygon(
    center: egui::Pos2,
    radius: f32,
    sides: usize,
    rotation: f32,
) -> Vec<egui::Pos2> {
    (0..sides)
        .map(|i| {
            let angle = rotation + TAU * i as f32 / sides as f32;
            egui::pos2(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            )
        })
        .collect()
}

fn rect_polygon(center: egui::Pos2, half_w: f32, half_h: f32) -> Vec<egui::Pos2> {
    vec![
        egui::pos2(center.x - half_w, center.y - half_h),
        egui::pos2(center.x + half_w, center.y - half_h),
        egui::pos2(center.x + half_w, center.y + half_h),
        egui::pos2(center.x - half_w, center.y + half_h),
    ]
}
