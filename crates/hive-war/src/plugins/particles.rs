use bevy::prelude::*;
use bevy_hanabi::Gradient as HanabiGradient;
use bevy_hanabi::prelude::*;

use super::markers::GameplayEntity;

// ── Events ──────────────────────────────────────────────────────────────────

/// Fired when a unit dies; triggers a dissolve-like particle burst.
#[derive(Message)]
pub struct DeathEvent {
    pub position: Vec2,
}

/// Fired when two units fuse; triggers a bright electric-arc burst.
#[derive(Message)]
pub struct FusionEvent {
    pub position: Vec2,
}

// ── Plugin ──────────────────────────────────────────────────────────────────

pub struct ParticlesPlugin;

impl Plugin for ParticlesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HanabiPlugin)
            .add_message::<DeathEvent>()
            .add_message::<FusionEvent>()
            .add_systems(Startup, setup_effects)
            .add_systems(
                Update,
                (
                    spawn_death_particles,
                    spawn_fusion_particles,
                    cleanup_particles,
                )
                    .run_if(super::game_state::match_is_active),
            );
    }
}

// ── Resources ───────────────────────────────────────────────────────────────

#[derive(Resource)]
struct ParticleEffects {
    death_effect: Handle<EffectAsset>,
    fusion_effect: Handle<EffectAsset>,
}

// ── Auto-cleanup component ──────────────────────────────────────────────────

#[derive(Component)]
struct ParticleLifetime(Timer);

// ── Setup ───────────────────────────────────────────────────────────────────

fn setup_effects(mut commands: Commands, mut effects: ResMut<Assets<EffectAsset>>) {
    // ── Death effect ────────────────────────────────────────────────────
    // Burst of 20 particles, expanding outward, orange→fade.
    let mut death_gradient = HanabiGradient::new();
    death_gradient.add_key(0.0, Vec4::new(1.0, 0.3, 0.1, 1.0));
    death_gradient.add_key(1.0, Vec4::new(0.5, 0.1, 0.0, 0.0));

    let mut death_module = Module::default();

    let init_age = SetAttributeModifier::new(Attribute::AGE, death_module.lit(0.0));
    let init_lifetime = SetAttributeModifier::new(Attribute::LIFETIME, death_module.lit(0.6));

    let init_pos = SetPositionCircleModifier {
        center: death_module.lit(Vec3::ZERO),
        axis: death_module.lit(Vec3::Z),
        radius: death_module.lit(3.0),
        dimension: ShapeDimension::Surface,
    };
    let init_vel = SetVelocitySphereModifier {
        center: death_module.lit(Vec3::ZERO),
        speed: death_module.lit(50.0),
    };

    let death_effect = effects.add(
        EffectAsset::new(128, SpawnerSettings::once(20.0.into()), death_module)
            .with_name("death")
            .init(init_pos)
            .init(init_vel)
            .init(init_age)
            .init(init_lifetime)
            .render(ColorOverLifetimeModifier {
                gradient: death_gradient,
                ..default()
            })
            .render(SizeOverLifetimeModifier {
                gradient: {
                    let mut g = HanabiGradient::new();
                    g.add_key(0.0, Vec3::splat(3.0));
                    g.add_key(1.0, Vec3::splat(0.5));
                    g
                },
                screen_space_size: false,
            }),
    );

    // ── Fusion effect ───────────────────────────────────────────────────
    // Brighter, more particles, cyan→white flash→blue fade.
    let mut fusion_gradient = HanabiGradient::new();
    fusion_gradient.add_key(0.0, Vec4::new(0.3, 0.7, 1.0, 1.0));
    fusion_gradient.add_key(0.5, Vec4::new(1.0, 1.0, 1.0, 0.8));
    fusion_gradient.add_key(1.0, Vec4::new(0.1, 0.3, 0.8, 0.0));

    let mut fusion_module = Module::default();

    let init_age2 = SetAttributeModifier::new(Attribute::AGE, fusion_module.lit(0.0));
    let init_lifetime2 = SetAttributeModifier::new(Attribute::LIFETIME, fusion_module.lit(0.8));

    let init_pos2 = SetPositionCircleModifier {
        center: fusion_module.lit(Vec3::ZERO),
        axis: fusion_module.lit(Vec3::Z),
        radius: fusion_module.lit(5.0),
        dimension: ShapeDimension::Surface,
    };
    let init_vel2 = SetVelocitySphereModifier {
        center: fusion_module.lit(Vec3::ZERO),
        speed: fusion_module.lit(80.0),
    };

    let fusion_effect = effects.add(
        EffectAsset::new(256, SpawnerSettings::once(40.0.into()), fusion_module)
            .with_name("fusion")
            .init(init_pos2)
            .init(init_vel2)
            .init(init_age2)
            .init(init_lifetime2)
            .render(ColorOverLifetimeModifier {
                gradient: fusion_gradient,
                ..default()
            })
            .render(SizeOverLifetimeModifier {
                gradient: {
                    let mut g = HanabiGradient::new();
                    g.add_key(0.0, Vec3::splat(4.0));
                    g.add_key(1.0, Vec3::splat(1.0));
                    g
                },
                screen_space_size: false,
            }),
    );

    commands.insert_resource(ParticleEffects {
        death_effect,
        fusion_effect,
    });
}

// ── Event-driven spawners ───────────────────────────────────────────────────

fn spawn_death_particles(
    mut commands: Commands,
    effects: Option<Res<ParticleEffects>>,
    mut events: MessageReader<DeathEvent>,
) {
    let Some(effects) = effects else { return };
    for ev in events.read() {
        commands.spawn((
            ParticleEffect::new(effects.death_effect.clone()),
            Transform::from_xyz(ev.position.x, ev.position.y, 10.0),
            ParticleLifetime(Timer::from_seconds(2.0, TimerMode::Once)),
            GameplayEntity,
        ));
    }
}

fn spawn_fusion_particles(
    mut commands: Commands,
    effects: Option<Res<ParticleEffects>>,
    mut events: MessageReader<FusionEvent>,
) {
    let Some(effects) = effects else { return };
    for ev in events.read() {
        commands.spawn((
            ParticleEffect::new(effects.fusion_effect.clone()),
            Transform::from_xyz(ev.position.x, ev.position.y, 10.0),
            ParticleLifetime(Timer::from_seconds(2.0, TimerMode::Once)),
            GameplayEntity,
        ));
    }
}

// ── Cleanup ─────────────────────────────────────────────────────────────────

fn cleanup_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut ParticleLifetime)>,
) {
    for (entity, mut lifetime) in query.iter_mut() {
        lifetime.0.tick(time.delta());
        if lifetime.0.is_finished()
            && let Ok(mut ec) = commands.get_entity(entity)
        {
            ec.despawn();
        }
    }
}
