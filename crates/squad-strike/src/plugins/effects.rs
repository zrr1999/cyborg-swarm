#![allow(clippy::too_many_arguments)]

use bevy::prelude::*;
use rand::RngExt;

pub struct EffectsPlugin;

#[derive(Message)]
pub struct DeathFxEvent {
    pub position: Vec2,
    pub is_enemy: bool,
}

#[derive(Message)]
pub struct PromotionFxEvent {
    pub position: Vec2,
}

#[derive(Message)]
pub struct LevelUpFxEvent {
    pub position: Vec2,
    pub is_enemy: bool,
}

#[derive(Component)]
struct FxParticle {
    velocity: Vec2,
    lifetime: Timer,
    start_scale: f32,
    end_scale: f32,
    color: Color,
}

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<DeathFxEvent>()
            .add_message::<PromotionFxEvent>()
            .add_message::<LevelUpFxEvent>()
            .add_systems(
                Update,
                (
                    spawn_death_fx,
                    spawn_promotion_fx,
                    spawn_levelup_fx,
                    tick_particles,
                ),
            );
    }
}

fn spawn_death_fx(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut events: MessageReader<DeathFxEvent>,
) {
    for event in events.read() {
        let color = if event.is_enemy {
            Color::srgb(1.0, 0.34, 0.28)
        } else {
            Color::srgb(0.36, 0.96, 0.76)
        };
        spawn_burst(
            &mut commands,
            meshes.as_mut(),
            materials.as_mut(),
            event.position,
            color,
            6,
            32.0,
            0.45,
        );
    }
}

fn spawn_promotion_fx(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut events: MessageReader<PromotionFxEvent>,
) {
    for event in events.read() {
        spawn_burst(
            &mut commands,
            meshes.as_mut(),
            materials.as_mut(),
            event.position,
            Color::srgb(1.0, 0.91, 0.36),
            8,
            44.0,
            0.55,
        );
    }
}

fn spawn_levelup_fx(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut events: MessageReader<LevelUpFxEvent>,
) {
    for event in events.read() {
        let color = if event.is_enemy {
            Color::srgb(1.0, 0.58, 0.42)
        } else {
            Color::srgb(0.52, 0.86, 1.0)
        };
        spawn_burst(
            &mut commands,
            meshes.as_mut(),
            materials.as_mut(),
            event.position,
            color,
            5,
            26.0,
            0.4,
        );
    }
}

fn spawn_burst(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    position: Vec2,
    color: Color,
    count: usize,
    speed: f32,
    lifetime: f32,
) {
    let mut rng = rand::rng();
    for _ in 0..count {
        let angle = rng.random_range(0.0..std::f32::consts::TAU);
        let velocity = Vec2::new(angle.cos(), angle.sin()) * rng.random_range(speed * 0.45..speed);
        let start_scale = rng.random_range(4.0..7.5);
        let end_scale = rng.random_range(0.5..1.8);
        commands.spawn((
            Mesh2d(meshes.add(Circle::new(1.0))),
            MeshMaterial2d(materials.add(ColorMaterial::from_color(color))),
            Transform::from_xyz(position.x, position.y, 50.0).with_scale(Vec3::splat(start_scale)),
            crate::plugins::game_flow::GameplayEntity,
            FxParticle {
                velocity,
                lifetime: Timer::from_seconds(lifetime, TimerMode::Once),
                start_scale,
                end_scale,
                color,
            },
        ));
    }
}

fn tick_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut particles: Query<(
        Entity,
        &mut Transform,
        &mut FxParticle,
        &MeshMaterial2d<ColorMaterial>,
    )>,
) {
    for (entity, mut tf, mut particle, material_handle) in particles.iter_mut() {
        particle.lifetime.tick(time.delta());
        let progress = particle.lifetime.fraction();
        tf.translation += (particle.velocity * time.delta_secs()).extend(0.0);
        let scale = particle.start_scale + (particle.end_scale - particle.start_scale) * progress;
        tf.scale = Vec3::splat(scale);
        if let Some(material) = materials.get_mut(material_handle) {
            let srgb = particle.color.to_srgba();
            material.color = Color::srgba(srgb.red, srgb.green, srgb.blue, 1.0 - progress);
        }
        if particle.lifetime.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}
