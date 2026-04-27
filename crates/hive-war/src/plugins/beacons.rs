use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;
use swarm_combat::Biomass;
use swarm_model::GameConfig;

use super::markers::GameplayEntity;
use crate::components::{Beacon, BeaconMode};
use crate::shared_assets::SharedGlyph;

pub struct BeaconPlugin;

#[derive(Component)]
struct BeaconDecoration;

impl Plugin for BeaconPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BeaconControlState>();
        app.add_systems(
            Update,
            (
                tick_beacon_state,
                switch_beacon_mode_system,
                strengthen_beacon_system,
                place_beacon_system,
                sync_beacon_visuals,
                draw_beacon_radius,
            )
                .run_if(super::game_state::match_is_active),
        );
    }
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct BeaconControlState {
    pub mode: BeaconMode,
    pub mode_cooldown_remaining: f32,
    pub strengthen_remaining: f32,
}

impl Default for BeaconControlState {
    fn default() -> Self {
        Self {
            mode: BeaconMode::Attract,
            mode_cooldown_remaining: 0.0,
            strengthen_remaining: 0.0,
        }
    }
}

impl BeaconControlState {
    pub fn is_strengthened(&self) -> bool {
        self.strengthen_remaining > 0.0
    }

    pub fn strength_multiplier(&self, cfg: &GameConfig) -> f32 {
        if self.is_strengthened() {
            cfg.beacon_strengthen_multiplier
        } else {
            1.0
        }
    }
}

fn tick_beacon_state(time: Res<Time>, mut state: ResMut<BeaconControlState>) {
    let dt = time.delta_secs();
    state.mode_cooldown_remaining = (state.mode_cooldown_remaining - dt).max(0.0);
    state.strengthen_remaining = (state.strengthen_remaining - dt).max(0.0);
}

fn switch_beacon_mode_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    cfg: Res<GameConfig>,
    mut state: ResMut<BeaconControlState>,
) {
    let requested_mode = if keyboard.just_pressed(KeyCode::Digit1) {
        Some(BeaconMode::Attract)
    } else if keyboard.just_pressed(KeyCode::Digit2) {
        Some(BeaconMode::Repel)
    } else if keyboard.just_pressed(KeyCode::Digit3) {
        Some(BeaconMode::Focus)
    } else {
        None
    };

    let Some(requested_mode) = requested_mode else {
        return;
    };
    if state.mode_cooldown_remaining > 0.0 || state.mode == requested_mode {
        return;
    }

    state.mode = requested_mode;
    state.mode_cooldown_remaining = cfg.beacon_mode_switch_cooldown_sec;
    state.strengthen_remaining = 0.0;
}

fn strengthen_beacon_system(
    keyboard: Res<ButtonInput<KeyCode>>,
    cfg: Res<GameConfig>,
    beacon_query: Query<Entity, With<Beacon>>,
    mut biomass: ResMut<Biomass>,
    mut state: ResMut<BeaconControlState>,
) {
    if !keyboard.just_pressed(KeyCode::KeyG) || beacon_query.is_empty() {
        return;
    }
    if biomass.player < cfg.beacon_strengthen_cost {
        return;
    }

    biomass.player -= cfg.beacon_strengthen_cost;
    state.strengthen_remaining = cfg.beacon_strengthen_duration_sec;
}

fn beacon_color(mode: BeaconMode, strengthened: bool) -> Color {
    let (r, g, b) = match mode {
        BeaconMode::Attract => (0.1_f32, 0.86_f32, 1.0_f32),
        BeaconMode::Repel => (1.0_f32, 0.65_f32, 0.18_f32),
        BeaconMode::Focus => (0.86_f32, 0.95_f32, 1.0_f32),
    };
    let boost = if strengthened { 0.18_f32 } else { 0.0_f32 };
    Color::srgb(
        (r + boost).clamp(0.0, 1.0),
        (g + boost).clamp(0.0, 1.0),
        (b + boost).clamp(0.0, 1.0),
    )
}

/// **F** places or moves the single beacon at the cursor; right-click removes it when the cursor is nearby.
#[allow(clippy::too_many_arguments)] // Bevy `SystemParam` tuple
fn place_beacon_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    beacon_query: Query<(Entity, &Transform), With<Beacon>>,
    mut state: ResMut<BeaconControlState>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((camera, cam_transform)) = camera_q.single() else {
        return;
    };

    let Some(cursor_pos) = window
        .cursor_position()
        .and_then(|p| camera.viewport_to_world_2d(cam_transform, p).ok())
    else {
        return;
    };

    if keyboard.just_pressed(KeyCode::KeyF) {
        let to_remove: Vec<Entity> = beacon_query.iter().map(|(e, _)| e).collect();
        for e in to_remove {
            commands.entity(e).despawn_children().despawn();
        }

        state.strengthen_remaining = 0.0;
        let color = beacon_color(state.mode, false);
        let beacon_entity = commands
            .spawn((
                Mesh2d(meshes.add(RegularPolygon::new(10.0, 4))),
                MeshMaterial2d(materials.add(color)),
                Transform::from_xyz(cursor_pos.x, cursor_pos.y, 5.0),
                Beacon::new(),
                GameplayEntity,
            ))
            .id();
        let glow = beacon_accent_color(color, 0.26);
        let core = beacon_accent_color(color, 0.7);
        commands.entity(beacon_entity).with_children(|parent| {
            parent.spawn((
                Mesh2d(meshes.add(Circle::new(24.0))),
                MeshMaterial2d(materials.add(beacon_accent_color(color, 0.1))),
                Transform::from_xyz(0.0, 0.0, -0.04),
                BeaconDecoration,
            ));
            parent.spawn((
                Mesh2d(meshes.add(Annulus::new(13.0, 19.0))),
                MeshMaterial2d(materials.add(glow)),
                Transform::from_xyz(0.0, 0.0, -0.02),
                BeaconDecoration,
            ));
            parent.spawn((
                Mesh2d(meshes.add(RegularPolygon::new(12.0, 6))),
                MeshMaterial2d(materials.add(glow)),
                Transform::from_xyz(0.0, 0.0, 0.2),
                BeaconDecoration,
                Name::new(format!(
                    "shared:{}",
                    SharedGlyph::CaptureBeacon.asset().relative_path()
                )),
            ));
            parent.spawn((
                Mesh2d(meshes.add(Rectangle::new(4.0, 14.0))),
                MeshMaterial2d(materials.add(core)),
                Transform::from_xyz(0.0, 15.0, 0.25),
                BeaconDecoration,
            ));
            parent.spawn((
                Mesh2d(meshes.add(Circle::new(2.5))),
                MeshMaterial2d(materials.add(core)),
                Transform::from_xyz(0.0, -9.0, 0.25),
                BeaconDecoration,
            ));
        });
    }

    if mouse.just_pressed(MouseButton::Right) {
        const REMOVE_RADIUS: f32 = 36.0;
        let mut closest: Option<(Entity, f32)> = None;
        for (entity, tf) in beacon_query.iter() {
            let dist = (tf.translation.truncate() - cursor_pos).length();
            if dist < REMOVE_RADIUS && (closest.is_none() || dist < closest.unwrap().1) {
                closest = Some((entity, dist));
            }
        }
        if let Some((entity, _)) = closest {
            commands.entity(entity).despawn_children().despawn();
            state.strengthen_remaining = 0.0;
        }
    }
}

fn sync_beacon_visuals(
    state: Res<BeaconControlState>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    root_query: Query<&MeshMaterial2d<ColorMaterial>, With<Beacon>>,
    accent_query: Query<&MeshMaterial2d<ColorMaterial>, With<BeaconDecoration>>,
) {
    let color = beacon_color(state.mode, state.is_strengthened());
    for mesh_mat in root_query.iter() {
        if let Some(mat) = materials.get_mut(mesh_mat) {
            mat.color = color;
        }
    }
    let glow = beacon_accent_color(color, 0.26);
    for mesh_mat in accent_query.iter() {
        if let Some(mat) = materials.get_mut(mesh_mat) {
            mat.color = glow;
        }
    }
}

fn draw_beacon_radius(
    state: Res<BeaconControlState>,
    mut gizmos: Gizmos,
    query: Query<(&Transform, &Beacon)>,
) {
    for (tf, beacon) in query.iter() {
        let mut color = beacon_color(state.mode, state.is_strengthened()).to_srgba();
        color.alpha = if state.is_strengthened() { 0.18 } else { 0.12 };
        let color = Color::srgba(color.red, color.green, color.blue, color.alpha);
        gizmos.circle_2d(tf.translation.truncate(), beacon.radius, color);
    }
}

fn beacon_accent_color(color: Color, alpha: f32) -> Color {
    let color = color.to_srgba();
    Color::srgba(color.red, color.green, color.blue, alpha)
}
