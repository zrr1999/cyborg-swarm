#![allow(clippy::too_many_arguments)]

use bevy::prelude::*;

pub struct WorldPlugin;

/// Half-extents of the playfield in world units.
pub const MAP_HALF_W: f32 = 640.0;
pub const MAP_HALF_H: f32 = 360.0;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup);
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);
    spawn_playfield(&mut commands, meshes.as_mut(), materials.as_mut());
}

fn spawn_playfield(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
) {
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(MAP_HALF_W * 2.0, MAP_HALF_H * 2.0))),
        MeshMaterial2d(materials.add(ColorMaterial::from_color(Color::srgb(0.015, 0.025, 0.04)))),
        Transform::from_xyz(0.0, 0.0, -20.0),
    ));

    let grid_major = materials.add(ColorMaterial::from_color(Color::srgba(
        0.18, 0.5, 0.62, 0.16,
    )));
    let grid_minor = materials.add(ColorMaterial::from_color(Color::srgba(
        0.12, 0.24, 0.34, 0.1,
    )));
    for (i, x) in [
        -560.0, -480.0, -400.0, -320.0, -240.0, -160.0, -80.0, 0.0, 80.0, 160.0, 240.0, 320.0,
        400.0, 480.0, 560.0,
    ]
    .into_iter()
    .enumerate()
    {
        commands.spawn((
            Mesh2d(meshes.add(Rectangle::new(
                if i % 4 == 0 { 1.8 } else { 0.9 },
                MAP_HALF_H * 2.0,
            ))),
            MeshMaterial2d(if i % 4 == 0 {
                grid_major.clone()
            } else {
                grid_minor.clone()
            }),
            Transform::from_xyz(x, 0.0, -18.5),
        ));
    }
    for (i, y) in [
        -320.0, -240.0, -160.0, -80.0, 0.0, 80.0, 160.0, 240.0, 320.0,
    ]
    .into_iter()
    .enumerate()
    {
        commands.spawn((
            Mesh2d(meshes.add(Rectangle::new(
                MAP_HALF_W * 2.0,
                if i % 4 == 0 { 1.8 } else { 0.9 },
            ))),
            MeshMaterial2d(if i % 4 == 0 {
                grid_major.clone()
            } else {
                grid_minor.clone()
            }),
            Transform::from_xyz(0.0, y, -18.5),
        ));
    }

    spawn_cover_lane(
        commands,
        meshes,
        materials,
        Vec2::new(-MAP_HALF_W, 0.0),
        Vec2::ZERO,
        10.0,
        Color::srgba(0.1, 0.78, 1.0, 0.18),
        -18.0,
    );
    spawn_cover_lane(
        commands,
        meshes,
        materials,
        Vec2::new(MAP_HALF_W, 0.0),
        Vec2::ZERO,
        10.0,
        Color::srgba(1.0, 0.12, 0.32, 0.18),
        -18.0,
    );
    commands.spawn((
        Mesh2d(meshes.add(Annulus::new(26.0, 42.0))),
        MeshMaterial2d(materials.add(ColorMaterial::from_color(Color::srgba(
            0.82, 0.96, 1.0, 0.2,
        )))),
        Transform::from_xyz(0.0, 0.0, -17.8),
    ));

    let border_material = materials.add(ColorMaterial::from_color(Color::srgba(
        0.58, 0.76, 0.88, 0.34,
    )));
    for (x, y, w, h) in [
        (0.0, MAP_HALF_H, MAP_HALF_W * 2.0, 3.0),
        (0.0, -MAP_HALF_H, MAP_HALF_W * 2.0, 3.0),
        (MAP_HALF_W, 0.0, 3.0, MAP_HALF_H * 2.0),
        (-MAP_HALF_W, 0.0, 3.0, MAP_HALF_H * 2.0),
    ] {
        commands.spawn((
            Mesh2d(meshes.add(Rectangle::new(w, h))),
            MeshMaterial2d(border_material.clone()),
            Transform::from_xyz(x, y, -17.0),
        ));
    }
}

fn spawn_cover_lane(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
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
        MeshMaterial2d(materials.add(ColorMaterial::from_color(color))),
        Transform::from_xyz(mid.x, mid.y, z)
            .with_rotation(Quat::from_rotation_z(delta.y.atan2(delta.x))),
    ));
}
