use bevy::prelude::*;
use swarm_model::Faction;

use super::markers::GameplayEntity;
use super::{COLOR_BASE_ENEMY, COLOR_BASE_NEUTRAL, COLOR_BASE_PLAYER};

/// Speed at which projectiles travel (world units per second).
const PROJECTILE_SPEED: f32 = 320.0;
/// Projectile despawns when closer than this to its target position.
const PROJECTILE_ARRIVAL_DIST: f32 = 5.0;
/// Maximum lifetime; despawns even if target is never reached.
const PROJECTILE_MAX_AGE: f32 = 1.2;
/// Visual radius of the projectile dot.
const PROJECTILE_RADIUS: f32 = 3.0;

// ── Event ────────────────────────────────────────────────────────────────────

/// Fire a purely visual projectile from `from` toward `to`.
#[derive(Message)]
pub struct ProjectileEvent {
    pub from: Vec2,
    pub to: Vec2,
    pub faction: Faction,
}

// ── Component ────────────────────────────────────────────────────────────────

#[derive(Component)]
struct Projectile {
    to: Vec2,
    age: f32,
}

// ── Plugin ───────────────────────────────────────────────────────────────────

pub struct ProjectilesPlugin;

impl Plugin for ProjectilesPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ProjectileEvent>().add_systems(
            Update,
            (spawn_projectiles, move_projectiles)
                .chain()
                .run_if(super::game_state::match_is_active),
        );
    }
}

// ── Systems ──────────────────────────────────────────────────────────────────

fn spawn_projectiles(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut events: MessageReader<ProjectileEvent>,
) {
    for ev in events.read() {
        let color = match ev.faction {
            Faction::Player => COLOR_BASE_PLAYER,
            Faction::Enemy => COLOR_BASE_ENEMY,
            Faction::Neutral => COLOR_BASE_NEUTRAL,
        };
        // Slightly brighter / more opaque than the base color.
        let sc = color.to_srgba();
        let dot_color = Color::srgba(
            (sc.red * 1.3).clamp(0.0, 1.0),
            (sc.green * 1.3).clamp(0.0, 1.0),
            (sc.blue * 1.3).clamp(0.0, 1.0),
            0.9,
        );
        commands.spawn((
            Mesh2d(meshes.add(Circle::new(PROJECTILE_RADIUS))),
            MeshMaterial2d(materials.add(dot_color)),
            Transform::from_xyz(ev.from.x, ev.from.y, 5.0),
            Projectile {
                to: ev.to,
                age: 0.0,
            },
            GameplayEntity,
        ));
    }
}

fn move_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Transform, &mut Projectile)>,
) {
    let dt = time.delta_secs();
    for (entity, mut tf, mut proj) in query.iter_mut() {
        proj.age += dt;
        if proj.age >= PROJECTILE_MAX_AGE {
            commands.entity(entity).despawn();
            continue;
        }
        let pos = tf.translation.truncate();
        let delta = proj.to - pos;
        let dist = delta.length();
        if dist <= PROJECTILE_ARRIVAL_DIST {
            commands.entity(entity).despawn();
            continue;
        }
        let step = PROJECTILE_SPEED * dt;
        tf.translation += (delta.normalize() * step.min(dist)).extend(0.0);
    }
}
