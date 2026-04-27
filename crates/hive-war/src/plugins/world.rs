#![allow(clippy::too_many_arguments)]

use bevy::prelude::*;
use bevy::text::{TextColor, TextFont};

use swarm_model::{Base, Faction, GameConfig};
use swarm_space::{MapBaseRole, MapDefinition};

use super::markers::GameplayEntity;
use super::{COLOR_BASE_ENEMY, COLOR_BASE_NEUTRAL, COLOR_BASE_PLAYER};
use crate::components::Headquarters;
use crate::shared_assets::SharedGlyph;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_world_startup);
    }
}

fn setup_world_startup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    map: Res<crate::map::ActiveMap>,
    cfg: Res<GameConfig>,
) {
    setup_world(&mut commands, &mut meshes, &mut materials, &map.0, &cfg);
}

/// Builds the playfield (camera, terrain, bases). Used on first load and after “Restart”.
pub fn setup_world(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    map: &MapDefinition,
    cfg: &GameConfig,
) {
    let hw = cfg.world_half_w;
    let hh = cfg.world_half_h;

    commands.spawn((Camera2d, GameplayEntity));

    // Keep the background oversized so taller/wider adaptive windows stay covered.
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(hw * 6.0, hh * 6.0))),
        MeshMaterial2d(materials.add(Color::srgb(0.015, 0.025, 0.04))),
        Transform::from_xyz(0.0, 0.0, -10.0),
        GameplayEntity,
    ));
    spawn_cover_grid(commands, meshes, materials, hw, hh);
    spawn_cover_map_lanes(commands, meshes, materials, map);

    // World border
    let border = 2.0;
    let w = hw * 2.0;
    let h = hh * 2.0;
    let border_color = materials.add(Color::srgba(1.0, 1.0, 1.0, 0.15));
    for (pos, size) in [
        (Vec3::new(0.0, hh, -9.0), Vec2::new(w, border)),
        (Vec3::new(0.0, -hh, -9.0), Vec2::new(w, border)),
        (Vec3::new(-hw, 0.0, -9.0), Vec2::new(border, h)),
        (Vec3::new(hw, 0.0, -9.0), Vec2::new(border, h)),
    ] {
        commands.spawn((
            Mesh2d(meshes.add(Rectangle::new(size.x, size.y))),
            MeshMaterial2d(border_color.clone()),
            Transform::from_translation(pos),
            GameplayEntity,
        ));
    }

    for row in &map.bases {
        let pos = Vec3::new(row.x, row.y, 1.0);
        match row.role {
            MapBaseRole::Player => {
                spawn_base(
                    commands,
                    meshes,
                    materials,
                    pos,
                    Base::player(),
                    cfg,
                    cfg.base_radius,
                    COLOR_BASE_PLAYER,
                    Some(("YOUR BASE", Color::srgba(0.65, 0.98, 0.82, 0.96))),
                    Some(Faction::Player),
                    Some(SharedGlyph::CommandWaypoint),
                );
            }
            MapBaseRole::Enemy => {
                spawn_base(
                    commands,
                    meshes,
                    materials,
                    pos,
                    Base::enemy(),
                    cfg,
                    cfg.base_radius,
                    COLOR_BASE_ENEMY,
                    Some(("ENEMY BASE", Color::srgba(1.0, 0.55, 0.52, 0.96))),
                    Some(Faction::Enemy),
                    Some(SharedGlyph::AlertPing),
                );
            }
            MapBaseRole::Neutral => {
                let hint = if row.hint.is_empty() {
                    "neutral"
                } else {
                    row.hint.as_str()
                };
                spawn_base(
                    commands,
                    meshes,
                    materials,
                    pos,
                    Base::neutral(hint),
                    cfg,
                    cfg.base_radius,
                    COLOR_BASE_NEUTRAL,
                    None,
                    None,
                    Some(SharedGlyph::SupplyCrate),
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_base(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    position: Vec3,
    base: Base,
    cfg: &GameConfig,
    base_radius: f32,
    color: Color,
    world_caption: Option<(&'static str, Color)>,
    hq_seat: Option<Faction>,
    marker: Option<SharedGlyph>,
) {
    let mut base = base;
    base.apply_config_stats(cfg);
    base.health = base.max_health;

    spawn_base_glow(commands, meshes, materials, position, base_radius, color);

    // Main capture body. The component lives on this entity so existing capture and combat
    // systems can keep updating ownership without knowing about the decorative children.
    let mut base_entity = commands.spawn((
        Mesh2d(meshes.add(RegularPolygon::new(base_radius * 0.86, 6))),
        MeshMaterial2d(materials.add(base_shell_color(color))),
        Transform::from_translation(position),
        base,
        GameplayEntity,
    ));
    if let Some(seat) = hq_seat {
        base_entity.insert(Headquarters { seat });
    }
    base_entity.with_children(|parent| {
        let accent = cover_accent(color, 0.92);
        let accent_soft = cover_accent(color, 0.28);
        parent.spawn((
            Mesh2d(meshes.add(Annulus::new(base_radius * 0.94, base_radius * 1.12))),
            MeshMaterial2d(materials.add(accent_soft)),
            Transform::from_xyz(0.0, 0.0, 0.12),
        ));
        parent.spawn((
            Mesh2d(meshes.add(RegularPolygon::new(base_radius * 0.5, 6))),
            MeshMaterial2d(materials.add(accent)),
            Transform::from_xyz(0.0, 0.0, 0.18),
        ));
        parent.spawn((
            Mesh2d(meshes.add(RegularPolygon::new(base_radius * 0.28, 6))),
            MeshMaterial2d(materials.add(Color::srgba(0.97, 1.0, 0.86, 0.92))),
            Transform::from_xyz(0.0, 0.0, 0.22),
        ));
        parent.spawn((
            Mesh2d(meshes.add(Rectangle::new(base_radius * 0.2, base_radius * 1.25))),
            MeshMaterial2d(materials.add(cover_accent(color, 0.16))),
            Transform::from_xyz(0.0, 0.0, 0.1),
        ));
        parent.spawn((
            Mesh2d(meshes.add(Rectangle::new(base_radius * 1.25, base_radius * 0.2))),
            MeshMaterial2d(materials.add(cover_accent(color, 0.16))),
            Transform::from_xyz(0.0, 0.0, 0.1),
        ));

        let y_off = base_radius + 10.0;
        if let Some(marker) = marker {
            let accent = match marker {
                SharedGlyph::CommandWaypoint => Color::srgba(0.38, 0.92, 1.0, 0.92),
                SharedGlyph::AlertPing => Color::srgba(1.0, 0.28, 0.42, 0.92),
                SharedGlyph::SupplyCrate => Color::srgba(1.0, 0.68, 0.24, 0.9),
                SharedGlyph::CaptureBeacon => Color::srgba(0.68, 0.95, 1.0, 0.92),
            };
            let marker_y = y_off + 2.0;
            match marker {
                SharedGlyph::CommandWaypoint => {
                    parent.spawn((
                        Mesh2d(meshes.add(RegularPolygon::new(9.0, 4))),
                        MeshMaterial2d(materials.add(accent)),
                        Transform::from_xyz(0.0, marker_y, 0.32)
                            .with_rotation(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4)),
                        Name::new(format!("shared:{}", marker.asset().relative_path())),
                    ));
                    parent.spawn((
                        Mesh2d(meshes.add(Rectangle::new(3.0, 8.0))),
                        MeshMaterial2d(materials.add(accent)),
                        Transform::from_xyz(0.0, marker_y - 10.0, 0.32),
                    ));
                }
                SharedGlyph::AlertPing => {
                    parent.spawn((
                        Mesh2d(meshes.add(RegularPolygon::new(9.0, 3))),
                        MeshMaterial2d(materials.add(accent)),
                        Transform::from_xyz(0.0, marker_y + 1.0, 0.32),
                        Name::new(format!("shared:{}", marker.asset().relative_path())),
                    ));
                    parent.spawn((
                        Mesh2d(meshes.add(Rectangle::new(2.5, 9.0))),
                        MeshMaterial2d(materials.add(Color::srgba(0.2, 0.1, 0.08, 0.9))),
                        Transform::from_xyz(0.0, marker_y - 1.0, 0.34),
                    ));
                    parent.spawn((
                        Mesh2d(meshes.add(Circle::new(1.5))),
                        MeshMaterial2d(materials.add(Color::srgba(0.2, 0.1, 0.08, 0.9))),
                        Transform::from_xyz(0.0, marker_y - 8.0, 0.34),
                    ));
                }
                SharedGlyph::SupplyCrate => {
                    parent.spawn((
                        Mesh2d(meshes.add(Rectangle::new(14.0, 12.0))),
                        MeshMaterial2d(materials.add(accent)),
                        Transform::from_xyz(0.0, marker_y, 0.32),
                        Name::new(format!("shared:{}", marker.asset().relative_path())),
                    ));
                    parent.spawn((
                        Mesh2d(meshes.add(Rectangle::new(2.0, 10.0))),
                        MeshMaterial2d(materials.add(Color::srgba(0.18, 0.2, 0.26, 0.9))),
                        Transform::from_xyz(0.0, marker_y, 0.34),
                    ));
                    parent.spawn((
                        Mesh2d(meshes.add(Rectangle::new(10.0, 2.0))),
                        MeshMaterial2d(materials.add(Color::srgba(0.18, 0.2, 0.26, 0.9))),
                        Transform::from_xyz(0.0, marker_y, 0.34),
                    ));
                }
                SharedGlyph::CaptureBeacon => {}
            }
        }
        if let Some((caption, cap_color)) = world_caption {
            parent.spawn((
                Text2d::new(caption),
                TextFont::default().with_font_size(13.0),
                TextColor(cap_color),
                Transform::from_xyz(0.0, y_off + 12.0, 0.35),
            ));
        }
    });
}

fn spawn_cover_grid(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    hw: f32,
    hh: f32,
) {
    let major = materials.add(Color::srgba(0.22, 0.5, 0.62, 0.13));
    let minor = materials.add(Color::srgba(0.15, 0.28, 0.38, 0.09));
    let span_w = hw * 6.0;
    let span_h = hh * 6.0;
    let mut x = -span_w * 0.5;
    let mut i = 0;
    while x <= span_w * 0.5 {
        let material = if i % 4 == 0 {
            major.clone()
        } else {
            minor.clone()
        };
        commands.spawn((
            Mesh2d(meshes.add(Rectangle::new(if i % 4 == 0 { 1.8 } else { 0.9 }, span_h))),
            MeshMaterial2d(material),
            Transform::from_xyz(x, 0.0, -9.8),
            GameplayEntity,
        ));
        x += 80.0;
        i += 1;
    }

    let mut y = -span_h * 0.5;
    let mut j = 0;
    while y <= span_h * 0.5 {
        let material = if j % 4 == 0 {
            major.clone()
        } else {
            minor.clone()
        };
        commands.spawn((
            Mesh2d(meshes.add(Rectangle::new(span_w, if j % 4 == 0 { 1.8 } else { 0.9 }))),
            MeshMaterial2d(material),
            Transform::from_xyz(0.0, y, -9.8),
            GameplayEntity,
        ));
        y += 80.0;
        j += 1;
    }
}

fn spawn_cover_map_lanes(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    map: &MapDefinition,
) {
    let center = Vec2::ZERO;
    for row in &map.bases {
        let color = match row.role {
            MapBaseRole::Player => Color::srgba(0.1, 0.78, 1.0, 0.22),
            MapBaseRole::Enemy => Color::srgba(1.0, 0.12, 0.32, 0.2),
            MapBaseRole::Neutral => Color::srgba(1.0, 0.65, 0.18, 0.16),
        };
        let start = Vec2::new(row.x, row.y);
        spawn_cover_lane(commands, meshes, materials, start, center, 8.0, color, -9.2);
        spawn_cover_lane(
            commands,
            meshes,
            materials,
            start,
            center,
            2.0,
            Color::srgba(0.82, 0.96, 1.0, 0.13),
            -9.1,
        );
    }
    commands.spawn((
        Mesh2d(meshes.add(Annulus::new(18.0, 30.0))),
        MeshMaterial2d(materials.add(Color::srgba(0.82, 0.96, 1.0, 0.2))),
        Transform::from_xyz(0.0, 0.0, -9.0),
        GameplayEntity,
    ));
}

fn spawn_cover_lane(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    start: Vec2,
    end: Vec2,
    width: f32,
    color: Color,
    z: f32,
) {
    let delta = end - start;
    let len = delta.length();
    if len < 1.0 {
        return;
    }
    let mid = start + delta * 0.5;
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(len, width))),
        MeshMaterial2d(materials.add(color)),
        Transform::from_xyz(mid.x, mid.y, z)
            .with_rotation(Quat::from_rotation_z(delta.y.atan2(delta.x))),
        GameplayEntity,
    ));
}

fn spawn_base_glow(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    position: Vec3,
    base_radius: f32,
    color: Color,
) {
    commands.spawn((
        Mesh2d(meshes.add(Circle::new(base_radius * 1.55))),
        MeshMaterial2d(materials.add(cover_accent(color, 0.12))),
        Transform::from_translation(position - Vec3::Z * 0.22),
        GameplayEntity,
    ));
    commands.spawn((
        Mesh2d(meshes.add(Annulus::new(base_radius * 1.14, base_radius * 1.38))),
        MeshMaterial2d(materials.add(cover_accent(color, 0.24))),
        Transform::from_translation(position - Vec3::Z * 0.16),
        GameplayEntity,
    ));
}

fn base_shell_color(color: Color) -> Color {
    let c = color.to_srgba();
    Color::srgba(
        (c.red * 0.34 + 0.04).clamp(0.0, 1.0),
        (c.green * 0.34 + 0.06).clamp(0.0, 1.0),
        (c.blue * 0.34 + 0.1).clamp(0.0, 1.0),
        0.98,
    )
}

fn cover_accent(color: Color, alpha: f32) -> Color {
    let c = color.to_srgba();
    Color::srgba(c.red, c.green, c.blue, alpha)
}
