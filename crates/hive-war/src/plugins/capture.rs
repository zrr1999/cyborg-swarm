use bevy::prelude::*;

use swarm_combat::{ArmyCapacityUpgrades, Biomass, army_upgrade_cost};
use swarm_model::{Base, Faction, GameConfig, GeneticTraits, Unit};
use swarm_sim::simulation::biomass_income_system;
use swarm_space::SpatialHash;
use swarm_tactics::BaseDefenseSignals;

use super::experience::{
    AttackPressureSample, BaseAssaultLedger, award_xp_shares, distribute_pool_with_main_share,
    unit_composite_power,
};
use super::units::{hit_range_from_traits, hostile_unit_in_weapon_range};
use super::{COLOR_BASE_ENEMY, COLOR_BASE_NEUTRAL, COLOR_BASE_PLAYER};

pub struct CapturePlugin;

impl Plugin for CapturePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(BaseDefenseSignals::default())
            .insert_resource(Biomass::default())
            .insert_resource(ArmyCapacityUpgrades::default())
            .insert_resource(EnemyArmyUpgradeTimer(Timer::from_seconds(
                1.5,
                TimerMode::Repeating,
            )))
            .add_systems(
                Update,
                (
                    base_capture_system,
                    base_tower_attack_system,
                    biomass_income_system,
                    enemy_army_upgrade_system,
                )
                    .run_if(super::game_state::match_is_active),
            );
    }
}

/// Periodic enemy macro-decision timer (repair / expansion / upgrade).
#[derive(Resource)]
pub struct EnemyArmyUpgradeTimer(pub Timer);

#[derive(Clone, Copy, Debug)]
struct BaseRiskSnapshot {
    entity: Entity,
    risk: f32,
    pressure: f32,
    frontline_score: f32,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn base_capture_system(
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    time: Res<Time>,
    cfg: Res<GameConfig>,
    hash: Res<SpatialHash>,
    mut defense: ResMut<BaseDefenseSignals>,
    mut assaults: ResMut<BaseAssaultLedger>,
    mut unit_queries: ParamSet<(
        Query<(Entity, &Transform, &Unit, &GeneticTraits)>,
        Query<&mut Unit>,
    )>,
    mut base_query: Query<(Entity, &mut Base, &Transform)>,
) {
    let dt = time.delta_secs();
    defense.clear();
    let mut best_player = (0.0_f32, Vec2::ZERO);
    let mut best_enemy = (0.0_f32, Vec2::ZERO);

    for (base_entity, mut base, base_tf) in base_query.iter_mut() {
        base.tick_weak_state(dt);
        base.tick_repair_cooldown(dt);

        let base_pos = base_tf.translation.truncate();
        let pre_capture_level = base.level;
        let mut player_pressure = 0.0f32;
        let mut enemy_pressure = 0.0f32;
        let mut player_threat = 0.0f32;
        let mut enemy_threat = 0.0f32;
        let mut player_near = 0u32;
        let mut enemy_near = 0u32;
        let mut player_attackers = Vec::new();
        let mut enemy_attackers = Vec::new();

        {
            let unit_query = unit_queries.p0();
            for (unit_entity, unit_tf, unit, traits) in unit_query.iter() {
                if unit.faction == Faction::Neutral || unit.faction == base.faction {
                    continue;
                }
                let unit_pos = unit_tf.translation.truncate();
                let dist = (unit_pos - base_pos).length();
                if dist >= base.capture_radius {
                    continue;
                }
                let wr = hit_range_from_traits(traits);
                if hostile_unit_in_weapon_range(
                    unit_entity,
                    unit_pos,
                    unit.faction,
                    wr,
                    &hash,
                    &unit_query,
                ) {
                    continue;
                }

                let pressure = traits.attack * 0.1 * dt;
                let threat = unit_composite_power(unit, traits);
                match unit.faction {
                    Faction::Player => {
                        player_pressure += pressure;
                        player_threat += threat;
                        player_near += 1;
                        player_attackers.push((unit_entity, pressure));
                    }
                    Faction::Enemy => {
                        enemy_pressure += pressure;
                        enemy_threat += threat;
                        enemy_near += 1;
                        enemy_attackers.push((unit_entity, pressure));
                    }
                    Faction::Neutral => {}
                }
            }
        }

        let dominant_attack = if player_pressure <= 0.0 && enemy_pressure <= 0.0 {
            None
        } else if player_pressure >= enemy_pressure {
            Some((
                Faction::Player,
                player_pressure,
                player_threat,
                player_attackers,
            ))
        } else {
            Some((
                Faction::Enemy,
                enemy_pressure,
                enemy_threat,
                enemy_attackers,
            ))
        };

        if let Some((attacker_faction, pressure, attacker_threat, contributors)) = dominant_attack {
            let contest_depth = (1.0
                - ((base.health - pressure).max(0.0) / base.max_health.max(1.0)))
            .clamp(0.0, 1.0);
            for (contributor, contribution) in contributors {
                assaults.record_attack_pressure(
                    base_entity,
                    AttackPressureSample {
                        defender: base.faction,
                        attacker: attacker_faction,
                        pre_capture_level,
                        contributor,
                        pressure: contribution,
                        attacker_threat,
                        contest_depth,
                    },
                );
            }

            match base.faction {
                Faction::Player => {
                    if pressure > best_player.0 {
                        best_player = (pressure, base_pos);
                    }
                }
                Faction::Enemy => {
                    if pressure > best_enemy.0 {
                        best_enemy = (pressure, base_pos);
                    }
                }
                Faction::Neutral => {}
            }

            base.health -= pressure;
            if base.health <= 0.0 {
                let new_faction = if player_near > enemy_near {
                    Faction::Player
                } else if enemy_near > player_near {
                    Faction::Enemy
                } else {
                    attacker_faction
                };

                base.faction = new_faction;
                base.spawn_timer = 0.0;
                base.downgrade_on_capture(&cfg);

                let color = match new_faction {
                    Faction::Player => COLOR_BASE_PLAYER,
                    Faction::Enemy => COLOR_BASE_ENEMY,
                    Faction::Neutral => COLOR_BASE_NEUTRAL,
                };
                commands
                    .entity(base_entity)
                    .insert(MeshMaterial2d(materials.add(color)));

                if let Some(record) = assaults.clear_assault(base_entity) {
                    let pool = cfg.base_capture_xp_pool(record.pre_capture_level);
                    let shares = distribute_pool_with_main_share(
                        pool,
                        assaults.time_s,
                        cfg.xp_contribution_window_sec,
                        &record.attackers,
                        None,
                        0.0,
                    );
                    award_xp_shares(&shares, &mut unit_queries.p1());
                }
            }
        } else {
            assaults.mark_attackers_absent(base_entity, dt);
            base.health = (base.health + cfg.base_passive_regen_per_sec * dt).min(base.max_health);

            if base.faction != Faction::Neutral
                && let Some(record) = assaults.take_held_defense_if_resolved(base_entity, &cfg)
                && record.defender == base.faction
                && !record.defenders.is_empty()
            {
                let pool = cfg.defense_xp_factor
                    * record.peak_attacker_threat
                    * record.max_contest_depth.max(0.1);
                let shares = distribute_pool_with_main_share(
                    pool,
                    assaults.time_s,
                    cfg.xp_contribution_window_sec,
                    &record.defenders,
                    None,
                    0.0,
                );
                award_xp_shares(&shares, &mut unit_queries.p1());
            }
        }
    }

    if best_player.0 > 0.0 {
        defense.player_defend_pos = Some(best_player.1);
        defense.player_urgency = (1.0 - (-best_player.0 * 18.0).exp()).clamp(0.0, 1.0);
    }
    if best_enemy.0 > 0.0 {
        defense.enemy_defend_pos = Some(best_enemy.1);
        defense.enemy_urgency = (1.0 - (-best_enemy.0 * 18.0).exp()).clamp(0.0, 1.0);
    }
}

/// Spend enemy biomass on urgent repairs first, then expansion, then stable frontline upgrades.
#[allow(clippy::type_complexity)]
fn enemy_army_upgrade_system(
    time: Res<Time>,
    cfg: Res<GameConfig>,
    mut timer: ResMut<EnemyArmyUpgradeTimer>,
    mut biomass: ResMut<Biomass>,
    mut upgrades: ResMut<ArmyCapacityUpgrades>,
    unit_query: Query<(&Transform, &Unit, &GeneticTraits)>,
    mut base_queries: ParamSet<(Query<(Entity, &Base, &Transform)>, Query<&mut Base>)>,
) {
    timer.0.tick(time.delta());
    if !timer.0.just_finished() {
        return;
    }

    let mut enemy_base_infos = Vec::new();
    let enemy_unit_count = unit_query
        .iter()
        .filter(|(_, unit, _)| unit.faction == Faction::Enemy)
        .count() as u32;

    {
        let base_view = base_queries.p0();
        for (entity, base, base_tf) in base_view.iter() {
            if matches!(base.faction, Faction::Enemy) {
                let mut attacker_threat = 0.0f32;
                let mut defender_threat = 1.0f32;
                let mut dist_to_front = f32::MAX;
                for (unit_tf, unit, traits) in unit_query.iter() {
                    match unit.faction {
                        Faction::Enemy => {
                            let d = (unit_tf.translation.truncate()
                                - base_tf.translation.truncate())
                            .length();
                            if d < base.capture_radius * 1.5 {
                                defender_threat += unit_composite_power(unit, traits);
                            }
                        }
                        Faction::Player => {
                            let d = (unit_tf.translation.truncate()
                                - base_tf.translation.truncate())
                            .length();
                            dist_to_front = dist_to_front.min(d);
                            if d < base.capture_radius * 1.5 {
                                attacker_threat += unit_composite_power(unit, traits);
                            }
                        }
                        Faction::Neutral => {}
                    }
                }

                let health_deficit = (1.0 - base.health / base.max_health.max(1.0)).clamp(0.0, 1.0);
                let pressure_ratio = attacker_threat / (attacker_threat + defender_threat);
                let risk = (health_deficit * 0.55
                    + pressure_ratio * 0.45
                    + if base.is_weak() { 0.1 } else { 0.0 })
                .clamp(0.0, 1.25);
                let frontline_score = (1.0 / (1.0 + dist_to_front)).clamp(0.0, 1.0);
                enemy_base_infos.push((
                    entity,
                    base.level,
                    base.can_repair(),
                    base.is_weak(),
                    BaseRiskSnapshot {
                        entity,
                        risk,
                        pressure: attacker_threat,
                        frontline_score,
                    },
                ));
            }
        }
    }

    if let Some(urgent) = enemy_base_infos
        .iter()
        .map(|(_, _, _, _, snapshot)| snapshot)
        .filter(|snapshot| snapshot.risk >= cfg.enemy_macro_repair_risk_threshold)
        .max_by(|a, b| {
            a.risk
                .partial_cmp(&b.risk)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .copied()
        && let Some((_, _, can_repair, _, _)) = enemy_base_infos
            .iter()
            .find(|(entity, ..)| *entity == urgent.entity)
        && *can_repair
        && let Ok(mut base) = base_queries.p1().get_mut(urgent.entity)
    {
        let heal_amount = (base.max_health - base.health).min(cfg.base_repair_amount);
        let cost = cfg.base_repair_cost(heal_amount);
        if biomass.enemy >= cost && heal_amount > 0.0 {
            biomass.enemy -= cost;
            base.repair(heal_amount);
            base.trigger_repair_cooldown(&cfg);
            return;
        }
    }

    let reserve = cfg.enemy_macro_reserve_biomass.max(0.0);
    let discretionary = (biomass.enemy - reserve).max(0.0);
    if discretionary <= 0.0 {
        return;
    }

    let current_cap = upgrades.faction_unit_cap(&Faction::Enemy, &cfg).max(1);
    let cap_ratio = enemy_unit_count as f32 / current_cap as f32;
    if cap_ratio >= cfg.enemy_macro_expand_cap_ratio
        && cfg.can_buy_global_expansion(upgrades.enemy_tier)
    {
        let cost = army_upgrade_cost(upgrades.enemy_tier, &cfg);
        if biomass.enemy >= reserve + cost {
            biomass.enemy -= cost;
            upgrades.enemy_tier += 1;
            return;
        }
    }

    let upgrade_target = enemy_base_infos
        .iter()
        .filter(|(_, level, _, is_weak, snapshot)| {
            *level < cfg.base_max_level && !*is_weak && snapshot.pressure <= 1.0
        })
        .max_by(|a, b| {
            a.4.frontline_score
                .partial_cmp(&b.4.frontline_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(entity, level, _, _, _)| (*entity, *level));

    if let Some((entity, level)) = upgrade_target {
        let cost = cfg.base_upgrade_cost(level);
        if biomass.enemy >= reserve + cost
            && let Ok(mut base) = base_queries.p1().get_mut(entity)
        {
            let _ = base.upgrade(&cfg);
            biomass.enemy -= cost;
        }
    }
}

// ── Base tower attack ─────────────────────────────────────────────────────────

/// Base level required before the tower becomes active.
const BASE_TOWER_MIN_LEVEL: u32 = 2;
/// Fire interval in seconds (all levels use the same cadence; damage scales with level).
const BASE_TOWER_FIRE_INTERVAL: f32 = 1.5;
/// Base attack range (world units). Scales by level × 15.
const BASE_TOWER_RANGE_BASE: f32 = 80.0;
const BASE_TOWER_RANGE_PER_LEVEL: f32 = 15.0;
/// Flat damage per shot × level.  At level 2 this yields ~107 DPS — roughly 10× a baseline unit.
const BASE_TOWER_DAMAGE_PER_LEVEL: f32 = 80.0;

/// Each non-neutral base at level ≥ [`BASE_TOWER_MIN_LEVEL`] fires at the nearest enemy unit
/// within range, dealing flat damage and spawning a visual projectile.
#[allow(clippy::too_many_arguments)]
fn base_tower_attack_system(
    time: Res<Time>,
    hash: Res<SpatialHash>,
    mut projectile_events: MessageWriter<super::projectiles::ProjectileEvent>,
    mut base_query: Query<(&mut Base, &Transform)>,
    mut unit_query: Query<(&mut Unit, &Transform, &GeneticTraits)>,
) {
    let dt = time.delta_secs();
    for (mut base, base_tf) in base_query.iter_mut() {
        if base.faction == Faction::Neutral {
            continue;
        }
        if base.level < BASE_TOWER_MIN_LEVEL {
            continue;
        }
        base.tower_fire_timer -= dt;
        if base.tower_fire_timer > 0.0 {
            continue;
        }

        let base_pos = base_tf.translation.truncate();
        let range = BASE_TOWER_RANGE_BASE + base.level as f32 * BASE_TOWER_RANGE_PER_LEVEL;
        let damage = base.level as f32 * BASE_TOWER_DAMAGE_PER_LEVEL;
        let base_faction = base.faction;

        // Find the nearest enemy unit in range.
        let mut best: Option<(Entity, f32)> = None;
        hash.for_each_in_radius(base_pos, range, |unit_e| {
            let Ok((unit, unit_tf, _)) = unit_query.get(unit_e) else {
                return;
            };
            if !base_faction.is_enemy_of(&unit.faction) {
                return;
            }
            let d = (base_pos - unit_tf.translation.truncate()).length();
            if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                best = Some((unit_e, d));
            }
        });

        if let Some((target_e, _)) = best {
            if let Ok((mut unit, unit_tf, _)) = unit_query.get_mut(target_e) {
                unit.current_health -= damage;
                unit.seconds_since_last_damage = 0.0;
                let target_pos = unit_tf.translation.truncate();
                projectile_events.write(super::projectiles::ProjectileEvent {
                    from: base_pos,
                    to: target_pos,
                    faction: base_faction,
                });
            }
            // Reset fire timer.
            base.tower_fire_timer = BASE_TOWER_FIRE_INTERVAL;
        } else {
            // No target; reset so we check again next interval.
            base.tower_fire_timer = BASE_TOWER_FIRE_INTERVAL;
        }
    }
}
