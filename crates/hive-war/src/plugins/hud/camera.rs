use bevy::camera::Viewport;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

/// Reserved egui margins in logical points. Used to tile the gameplay camera with the HUD shell.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub(crate) struct GameHudInsets {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

pub(super) fn apply_game_camera_viewport(
    window: Query<&Window, With<PrimaryWindow>>,
    insets: Res<GameHudInsets>,
    game_end: Res<super::super::game_state::GameEnd>,
    mut cameras: Query<
        &mut Camera,
        (
            With<Camera2d>,
            With<crate::plugins::markers::GameplayEntity>,
        ),
    >,
) {
    let Ok(window) = window.single() else {
        return;
    };
    let Ok(mut cam) = cameras.single_mut() else {
        return;
    };

    // All-zero insets (the normal gameplay case) or game-over: let the camera use the full window.
    if game_end.0.is_some()
        || (insets.left == 0.0 && insets.right == 0.0 && insets.top == 0.0 && insets.bottom == 0.0)
    {
        cam.viewport = None;
        return;
    }

    let scale_factor = window.scale_factor();
    let physical_width = window.resolution.physical_width();
    let physical_height = window.resolution.physical_height();

    let left_px = (insets.left * scale_factor)
        .round()
        .clamp(0.0, physical_width as f32) as u32;
    let right_px = (insets.right * scale_factor)
        .round()
        .clamp(0.0, physical_width as f32) as u32;
    let top_px = (insets.top * scale_factor)
        .round()
        .clamp(0.0, physical_height as f32) as u32;
    let bottom_px = (insets.bottom * scale_factor)
        .round()
        .clamp(0.0, physical_height as f32) as u32;

    let reserved_width = left_px.saturating_add(right_px);
    let reserved_height = top_px.saturating_add(bottom_px);
    let viewport_width = physical_width.saturating_sub(reserved_width).max(1);
    let viewport_height = physical_height.saturating_sub(reserved_height).max(1);

    cam.viewport = Some(Viewport {
        physical_position: UVec2::new(left_px, top_px),
        physical_size: UVec2::new(viewport_width, viewport_height),
        depth: 0.0..1.0,
    });
}
