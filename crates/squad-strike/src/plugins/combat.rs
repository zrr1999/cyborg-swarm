#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use bevy::prelude::*;
use swarm_growth::WolfPack;
use swarm_model::{GeneticTraits, MovementSuppression, Unit, UnitFitness, unit_strength};
use swarm_tactics::select_replacement_captain;

use crate::plugins::{
    effects::{DeathFxEvent, PromotionFxEvent},
    enemies::{EnemyBounty, EnemyUnit, MAX_WAVES_PER_RUN, SpawnEdge},
    meta_progression::{MetaProgression, MetaRunState},
    skills::{
        RunSkills, assimilation_scale, guard_damage_taken_multiplier, rapid_fire_multiplier,
        targeting_damage_multiplier,
    },
    squad::{
        CaptainAnnouncement, HitFlash, PendingCaptainPromotion, SquadCaptain, SquadFollower,
        SquadMember, spawn_player_follower,
    },
};

pub struct CombatPlugin;

#[derive(Component)]
pub struct AttackCooldown(pub f32);

#[derive(Resource, Default)]
pub struct GameOver {
    pub active: bool,
    pub victory: bool,
}

const ATTACK_RANGE: f32 = 50.0;
const SUPPRESSION_PER_DAMAGE: f32 = 0.062;
const XP_PER_DAMAGE: f32 = 1.0;
const XP_PER_KILL_BONUS: f32 = 18.0;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameOver>().add_systems(
            Update,
            (
                tick_cooldowns,
                player_attack_system,
                enemy_attack_system,
                death_system,
                promote_new_captain_system,
                update_game_over_system,
            )
                .chain(),
        );
    }
}

fn tick_cooldowns(mut q: Query<&mut AttackCooldown>, time: Res<Time>) {
    for mut cd in q.iter_mut() {
        cd.0 = (cd.0 - time.delta_secs()).max(0.0);
    }
}

type PlayerFilter = (With<SquadMember>, Without<EnemyUnit>);
type EnemyFilter = (With<EnemyUnit>, Without<SquadMember>);

#[allow(clippy::type_complexity)]
fn player_attack_system(
    skills: Res<RunSkills>,
    mut players: Query<
        (
            &Transform,
            &GeneticTraits,
            &mut AttackCooldown,
            &mut UnitFitness,
            &mut Unit,
        ),
        PlayerFilter,
    >,
    mut enemies: ParamSet<(
        Query<
            (
                Entity,
                &Transform,
                &Unit,
                &GeneticTraits,
                Option<&EnemyBounty>,
            ),
            EnemyFilter,
        >,
        Query<
            (
                &mut Unit,
                &GeneticTraits,
                Option<&EnemyBounty>,
                &mut MovementSuppression,
                &mut UnitFitness,
                &mut HitFlash,
            ),
            EnemyFilter,
        >,
    )>,
) {
    for (ptf, traits, mut cd, mut player_fitness, mut player_unit) in players.iter_mut() {
        if player_unit.is_dead() {
            continue;
        }
        if cd.0 > 0.0 {
            continue;
        }
        let Some(target) = enemies
            .p0()
            .iter()
            .filter(|(_, _, enemy_unit, _, _)| !enemy_unit.is_dead())
            .filter_map(|(entity, etf, enemy_unit, enemy_traits, bounty)| {
                let dist_sq = ptf.translation.distance_squared(etf.translation);
                (dist_sq <= ATTACK_RANGE * ATTACK_RANGE).then_some((
                    entity,
                    player_target_score(dist_sq, enemy_unit, enemy_traits, bounty),
                ))
            })
            .min_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(entity, _)| entity)
        else {
            continue;
        };

        let mut enemy_writer = enemies.p1();
        let Ok((
            mut enemy_unit,
            enemy_traits,
            bounty,
            mut suppression,
            mut enemy_fitness,
            mut hit_flash,
        )) = enemy_writer.get_mut(target)
        else {
            continue;
        };
        let dmg = (traits.attack * targeting_damage_multiplier(skills.targeting_stacks)
            - enemy_traits.armor)
            .max(1.0);
        let lethal = enemy_unit.current_health > 0.0 && enemy_unit.current_health - dmg <= 0.0;
        enemy_unit.current_health -= dmg;
        suppression.level = (suppression.level + dmg * SUPPRESSION_PER_DAMAGE).min(1.0);
        player_fitness.damage_dealt += dmg;
        enemy_fitness.damage_taken_survived += dmg;
        let bounty = bounty.copied().unwrap_or(EnemyBounty {
            damage_xp_scale: 1.0,
            kill_xp_scale: 1.0,
            cell_reward: 1,
        });
        player_unit.gain_xp(dmg * XP_PER_DAMAGE * bounty.damage_xp_scale);
        if lethal {
            player_unit.gain_xp(XP_PER_KILL_BONUS * bounty.kill_xp_scale);
        }
        hit_flash.0 = 1.0;
        cd.0 = 1.0 / (traits.attack_speed * rapid_fire_multiplier(skills.rapid_stacks));
    }
}

#[allow(clippy::type_complexity)]
fn enemy_attack_system(
    skills: Res<RunSkills>,
    mut enemies: Query<
        (
            &Transform,
            &GeneticTraits,
            &mut AttackCooldown,
            &mut UnitFitness,
        ),
        EnemyFilter,
    >,
    mut players: ParamSet<(
        Query<
            (
                Entity,
                &Transform,
                &Unit,
                &GeneticTraits,
                Option<&SquadCaptain>,
            ),
            PlayerFilter,
        >,
        Query<
            (
                &mut Unit,
                &GeneticTraits,
                &mut MovementSuppression,
                &mut UnitFitness,
                &mut HitFlash,
            ),
            PlayerFilter,
        >,
    )>,
) {
    for (etf, traits, mut cd, mut enemy_fitness) in enemies.iter_mut() {
        if cd.0 > 0.0 {
            continue;
        }
        let Some(target) = players
            .p0()
            .iter()
            .filter(|(_, _, player_unit, _, _)| !player_unit.is_dead())
            .filter_map(|(entity, ptf, player_unit, player_traits, captain)| {
                let dist_sq = etf.translation.distance_squared(ptf.translation);
                (dist_sq <= ATTACK_RANGE * ATTACK_RANGE).then_some((
                    entity,
                    enemy_target_score(dist_sq, player_unit, player_traits, captain.is_some()),
                ))
            })
            .min_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(entity, _)| entity)
        else {
            continue;
        };

        let mut player_writer = players.p1();
        let Ok((
            mut player_unit,
            player_traits,
            mut suppression,
            mut player_fitness,
            mut hit_flash,
        )) = player_writer.get_mut(target)
        else {
            continue;
        };
        let dmg = (traits.attack - player_traits.armor).max(1.0)
            * guard_damage_taken_multiplier(skills.guard_stacks);
        player_unit.current_health -= dmg;
        suppression.level = (suppression.level + dmg * SUPPRESSION_PER_DAMAGE).min(1.0);
        enemy_fitness.damage_dealt += dmg;
        player_fitness.damage_taken_survived += dmg;
        hit_flash.0 = 1.0;
        cd.0 = 1.0 / traits.attack_speed;
    }
}

#[allow(clippy::type_complexity)]
fn death_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    units: Query<(
        Entity,
        &Transform,
        &Unit,
        &GeneticTraits,
        Option<&UnitFitness>,
        Option<&EnemyUnit>,
        Option<&EnemyBounty>,
    )>,
    followers: Query<&SquadFollower, With<SquadMember>>,
    mut wave: ResMut<crate::plugins::enemies::WaveState>,
    mut wolves: ResMut<WolfPack>,
    cfg: Res<swarm_model::GameConfig>,
    meta: Res<MetaProgression>,
    skills: Res<RunSkills>,
    mut run: ResMut<MetaRunState>,
    mut announcement: ResMut<CaptainAnnouncement>,
    mut death_fx: MessageWriter<DeathFxEvent>,
    mut game_over: ResMut<GameOver>,
) {
    let mut next_follower_slot = followers
        .iter()
        .map(|follower| follower.slot_index)
        .max()
        .map(|slot| slot + 1)
        .unwrap_or(0);

    for (entity, tf, unit, traits, fitness, enemy_marker, bounty) in units.iter() {
        if !unit.is_dead() {
            continue;
        }
        wolves.record_candidate(
            unit.faction,
            traits,
            fitness.map(UnitFitness::score).unwrap_or_default(),
        );
        if enemy_marker.is_some() {
            let bounty = bounty.copied().unwrap_or(EnemyBounty {
                damage_xp_scale: 1.0,
                kill_xp_scale: 1.0,
                cell_reward: 1,
            });
            wave.kills += 1;
            if wave.enemies_alive > 0 {
                wave.enemies_alive -= 1;
            }
            let elite_bonus = if bounty.kill_xp_scale > 1.0 {
                meta.elite_cell_bonus()
            } else {
                0
            };
            run.add_cells(meta.scaled_cells_gain(
                bounty.cell_reward + skills.salvage_stacks as u64 + elite_bonus,
            ));
            if skills.assimilation_stacks > 0 {
                let summoned_traits =
                    traits.scale_by(assimilation_scale(skills.assimilation_stacks));
                spawn_player_follower(
                    &mut commands,
                    meshes.as_mut(),
                    materials.as_mut(),
                    &cfg,
                    tf.translation.truncate(),
                    next_follower_slot,
                    summoned_traits,
                );
                next_follower_slot += 1;
            }
            if wave.enemies_alive == 0 {
                let wave_bonus = 4 + wave.wave.max(1) as u64 * 2;
                run.add_cells(meta.scaled_cells_gain(wave_bonus));
                if wave.wave >= MAX_WAVES_PER_RUN {
                    game_over.active = true;
                    game_over.victory = true;
                    wave.incoming_edge = None;
                    announcement.show("Run cleared");
                } else {
                    wave.next_wave_timer = 3.0;
                    wave.incoming_edge = Some(SpawnEdge::planned_for_wave(wave.wave + 1));
                    announcement.show(format!("Wave {} cleared", wave.wave));
                }
            }
        }
        death_fx.write(DeathFxEvent {
            position: tf.translation.truncate(),
            is_enemy: enemy_marker.is_some(),
        });
        commands.entity(entity).despawn_children().despawn();
    }
}

fn promote_new_captain_system(
    mut commands: Commands,
    players: Query<
        (
            Entity,
            &Transform,
            &Unit,
            &GeneticTraits,
            Option<&SquadCaptain>,
        ),
        (With<SquadMember>, Without<EnemyUnit>),
    >,
    mut promotion_fx: MessageWriter<PromotionFxEvent>,
    mut announcement: ResMut<CaptainAnnouncement>,
) {
    if players
        .iter()
        .any(|(_, _, unit, _, captain)| !unit.is_dead() && captain.is_some())
    {
        return;
    }

    let replacement =
        select_replacement_captain(players.iter().filter_map(|(entity, _tf, unit, traits, _)| {
            (!unit.is_dead()).then_some((entity, unit_strength(traits)))
        }));
    let Some((entity, tf)) = replacement.and_then(|entity| {
        players
            .iter()
            .find(|(candidate, _, unit, _, _)| *candidate == entity && !unit.is_dead())
            .map(|(_, tf, _, _, _)| (entity, tf))
    }) else {
        return;
    };

    commands
        .entity(entity)
        .insert(SquadCaptain)
        .insert(PendingCaptainPromotion)
        .remove::<SquadFollower>();
    promotion_fx.write(PromotionFxEvent {
        position: tf.translation.truncate(),
    });
    announcement.show("Captain inherited");
}

fn update_game_over_system(
    players: Query<Entity, (With<SquadMember>, Without<EnemyUnit>)>,
    mut game_over: ResMut<GameOver>,
) {
    if players.is_empty() {
        game_over.active = true;
        game_over.victory = false;
    }
}

fn player_target_score(
    dist_sq: f32,
    target: &Unit,
    traits: &GeneticTraits,
    bounty: Option<&EnemyBounty>,
) -> f32 {
    let health = (target.current_health / traits.max_health.max(1.0)).clamp(0.0, 1.0);
    let distance = (dist_sq.sqrt() / ATTACK_RANGE).clamp(0.0, 1.0);
    let reward = bounty
        .map(|b| b.kill_xp_scale)
        .unwrap_or(1.0)
        .clamp(1.0, 3.5);
    health * 0.58 + distance * 0.32 - (reward - 1.0) * 0.04
}

fn enemy_target_score(
    dist_sq: f32,
    target: &Unit,
    traits: &GeneticTraits,
    is_captain: bool,
) -> f32 {
    let health = (target.current_health / traits.max_health.max(1.0)).clamp(0.0, 1.0);
    let distance = (dist_sq.sqrt() / ATTACK_RANGE).clamp(0.0, 1.0);
    health * 0.5 + distance * 0.35 - if is_captain { 0.12 } else { 0.0 }
}
