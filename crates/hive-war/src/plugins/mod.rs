pub mod beacons;
pub mod captain_ai;
pub mod capture;
pub mod config;
pub mod experience;
pub mod game_flow;
pub mod game_state;
pub mod genetics;
pub mod hud;
pub mod markers;
pub mod particles;
pub mod projectiles;
pub mod units;
pub mod wolf_pack;
pub mod world;

pub use hud::HudPlugin;

use bevy::prelude::Color;

/// Theme colors (sRGB)
pub const COLOR_PLAYER: Color = Color::srgb(0.16, 0.86, 1.0);
pub const COLOR_ENEMY: Color = Color::srgb(1.0, 0.16, 0.34);
pub const COLOR_BASE_PLAYER: Color = Color::srgb(0.1, 0.78, 1.0);
pub const COLOR_BASE_ENEMY: Color = Color::srgb(1.0, 0.12, 0.32);
pub const COLOR_BASE_NEUTRAL: Color = Color::srgb(1.0, 0.65, 0.18);
