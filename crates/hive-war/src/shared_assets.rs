use bevy_egui::egui;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SharedAssetKind {
    EnergyCell,
    RepairKit,
    HealthOrb,
    ShieldEmitter,
    SpeedBoost,
    ScrapShard,
    CaptureBeacon,
    CommandWaypoint,
    AlertPing,
    TargetReticle,
    HazardWarning,
    CaptainBadge,
    SupplyCrate,
}

impl SharedAssetKind {
    pub const fn id(self) -> &'static str {
        match self {
            Self::EnergyCell => "energy_cell",
            Self::RepairKit => "repair_kit",
            Self::HealthOrb => "health_orb",
            Self::ShieldEmitter => "shield_emitter",
            Self::SpeedBoost => "speed_boost",
            Self::ScrapShard => "scrap_shard",
            Self::CaptureBeacon => "capture_beacon",
            Self::CommandWaypoint => "command_waypoint",
            Self::AlertPing => "alert_ping",
            Self::TargetReticle => "target_reticle",
            Self::HazardWarning => "hazard_warning",
            Self::CaptainBadge => "captain_badge",
            Self::SupplyCrate => "supply_crate",
        }
    }

    pub const fn relative_path(self) -> &'static str {
        match self {
            Self::EnergyCell => "shared/icons/shared_icon_energy_cell.svg",
            Self::RepairKit => "shared/icons/shared_icon_repair_kit.svg",
            Self::HealthOrb => "shared/icons/shared_icon_health_orb.svg",
            Self::ShieldEmitter => "shared/icons/shared_icon_shield_emitter.svg",
            Self::SpeedBoost => "shared/icons/shared_icon_speed_boost.svg",
            Self::ScrapShard => "shared/icons/shared_icon_scrap_shard.svg",
            Self::CaptureBeacon => "shared/markers/shared_marker_capture_beacon.svg",
            Self::CommandWaypoint => "shared/markers/shared_marker_command_waypoint.svg",
            Self::AlertPing => "shared/markers/shared_marker_alert_ping.svg",
            Self::TargetReticle => "shared/markers/shared_marker_target_reticle.svg",
            Self::HazardWarning => "shared/markers/shared_marker_hazard_warning.svg",
            Self::CaptainBadge => "shared/markers/shared_marker_captain_badge.svg",
            Self::SupplyCrate => "shared/icons/shared_prop_supply_crate.svg",
        }
    }
}

pub const ALL_SHARED_ASSETS: [SharedAssetKind; 13] = [
    SharedAssetKind::EnergyCell,
    SharedAssetKind::RepairKit,
    SharedAssetKind::HealthOrb,
    SharedAssetKind::ShieldEmitter,
    SharedAssetKind::SpeedBoost,
    SharedAssetKind::ScrapShard,
    SharedAssetKind::CaptureBeacon,
    SharedAssetKind::CommandWaypoint,
    SharedAssetKind::AlertPing,
    SharedAssetKind::TargetReticle,
    SharedAssetKind::HazardWarning,
    SharedAssetKind::CaptainBadge,
    SharedAssetKind::SupplyCrate,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SharedGlyph {
    CaptureBeacon,
    CommandWaypoint,
    AlertPing,
    SupplyCrate,
}

impl SharedGlyph {
    pub const fn asset(self) -> SharedAssetKind {
        match self {
            Self::CaptureBeacon => SharedAssetKind::CaptureBeacon,
            Self::CommandWaypoint => SharedAssetKind::CommandWaypoint,
            Self::AlertPing => SharedAssetKind::AlertPing,
            Self::SupplyCrate => SharedAssetKind::SupplyCrate,
        }
    }

    pub const fn hover_text(self) -> &'static str {
        self.asset().relative_path()
    }
}

pub fn glyph_widget(
    ui: &mut egui::Ui,
    glyph: SharedGlyph,
    tint: egui::Color32,
    size: f32,
) -> egui::Response {
    let desired = egui::vec2(size, size);
    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        paint_glyph(ui.painter(), rect, glyph, tint);
    }
    response.on_hover_text(format!("Shared asset: {}", glyph.hover_text()))
}

fn paint_glyph(painter: &egui::Painter, rect: egui::Rect, glyph: SharedGlyph, tint: egui::Color32) {
    let stroke = egui::Stroke::new((rect.width() * 0.08).max(1.5), tint);
    let fill = tint.gamma_multiply(0.18);
    let c = rect.center();
    let r = rect.width().min(rect.height()) * 0.34;

    match glyph {
        SharedGlyph::CaptureBeacon => {
            painter.circle_stroke(c, r * 1.05, stroke);
            painter.circle_filled(c, r * 0.32, tint);
            painter.line_segment([egui::pos2(c.x, c.y - r), egui::pos2(c.x, c.y + r)], stroke);
            painter.circle_filled(egui::pos2(c.x, c.y + r * 1.2), r * 0.17, tint);
        }
        SharedGlyph::CommandWaypoint => {
            let pts = vec![
                egui::pos2(c.x, c.y - r * 1.1),
                egui::pos2(c.x + r, c.y),
                egui::pos2(c.x, c.y + r * 1.1),
                egui::pos2(c.x - r, c.y),
            ];
            painter.add(egui::Shape::convex_polygon(pts, fill, stroke));
            painter.line_segment(
                [
                    egui::pos2(c.x, c.y + r * 1.15),
                    egui::pos2(c.x, c.y + r * 1.7),
                ],
                stroke,
            );
        }
        SharedGlyph::AlertPing => {
            let pts = vec![
                egui::pos2(c.x, c.y - r * 1.15),
                egui::pos2(c.x + r, c.y + r * 0.9),
                egui::pos2(c.x - r, c.y + r * 0.9),
            ];
            painter.add(egui::Shape::convex_polygon(pts, fill, stroke));
            painter.line_segment(
                [
                    egui::pos2(c.x, c.y - r * 0.55),
                    egui::pos2(c.x, c.y + r * 0.1),
                ],
                stroke,
            );
            painter.circle_filled(egui::pos2(c.x, c.y + r * 0.48), r * 0.11, tint);
        }
        SharedGlyph::SupplyCrate => {
            let left = c.x - r;
            let right = c.x + r;
            let top = c.y - r * 0.85;
            let bottom = c.y + r * 0.85;
            let tl = egui::pos2(left, top);
            let tr = egui::pos2(right, top);
            let br = egui::pos2(right, bottom);
            let bl = egui::pos2(left, bottom);
            painter.add(egui::Shape::convex_polygon(
                vec![tl, tr, br, bl],
                fill,
                stroke,
            ));
            painter.line_segment([tl, br], stroke);
            painter.line_segment([tr, bl], stroke);
            painter.line_segment(
                [
                    egui::pos2(c.x, top - r * 0.25),
                    egui::pos2(c.x, bottom + r * 0.25),
                ],
                egui::Stroke::new((rect.width() * 0.05).max(1.0), tint.gamma_multiply(0.7)),
            );
        }
    }
}
