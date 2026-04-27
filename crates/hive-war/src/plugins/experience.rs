use std::collections::HashMap;

use bevy::prelude::*;

use swarm_model::{Faction, GameConfig, GeneticTraits, Unit, unit_strength};

pub struct ExperiencePlugin;

impl Plugin for ExperiencePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CombatContributionLedger>()
            .init_resource::<BaseAssaultLedger>()
            .add_systems(
                Update,
                tick_experience_ledgers.run_if(super::game_state::match_is_active),
            );
    }
}

fn tick_experience_ledgers(
    time: Res<Time>,
    mut combat: ResMut<CombatContributionLedger>,
    mut assaults: ResMut<BaseAssaultLedger>,
) {
    let dt = time.delta_secs();
    combat.time_s += dt;
    assaults.time_s += dt;
}

#[derive(Clone, Debug, Default)]
pub struct ContributionEntry {
    pub amount: f32,
    pub last_time: f32,
}

#[derive(Clone, Debug, Default)]
pub struct UnitContributionRecord {
    pub contributors: HashMap<Entity, ContributionEntry>,
    pub last_hitter: Option<Entity>,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct CombatContributionLedger {
    pub time_s: f32,
    pub unit_targets: HashMap<Entity, UnitContributionRecord>,
}

impl CombatContributionLedger {
    pub fn record_damage(&mut self, attacker: Entity, target: Entity, amount: f32) {
        if amount <= 0.0 {
            return;
        }
        let target_record = self.unit_targets.entry(target).or_default();
        let entry = target_record.contributors.entry(attacker).or_default();
        entry.amount += amount;
        entry.last_time = self.time_s;
        target_record.last_hitter = Some(attacker);
    }

    pub fn remove_target(&mut self, target: Entity) -> Option<UnitContributionRecord> {
        self.unit_targets.remove(&target)
    }
}

#[derive(Clone, Debug)]
pub struct BaseAssaultRecord {
    pub defender: Faction,
    pub attacker: Faction,
    pub pre_capture_level: u32,
    pub max_contest_depth: f32,
    pub peak_attacker_threat: f32,
    pub no_attacker_time: f32,
    pub attackers: HashMap<Entity, ContributionEntry>,
    pub defenders: HashMap<Entity, ContributionEntry>,
}

impl BaseAssaultRecord {
    fn new(defender: Faction, attacker: Faction, pre_capture_level: u32) -> Self {
        Self {
            defender,
            attacker,
            pre_capture_level,
            max_contest_depth: 0.0,
            peak_attacker_threat: 0.0,
            no_attacker_time: 0.0,
            attackers: HashMap::new(),
            defenders: HashMap::new(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AttackPressureSample {
    pub defender: Faction,
    pub attacker: Faction,
    pub pre_capture_level: u32,
    pub contributor: Entity,
    pub pressure: f32,
    pub attacker_threat: f32,
    pub contest_depth: f32,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct BaseAssaultLedger {
    pub time_s: f32,
    pub assaults: HashMap<Entity, BaseAssaultRecord>,
}

impl BaseAssaultLedger {
    pub fn record_attack_pressure(&mut self, base_entity: Entity, sample: AttackPressureSample) {
        if sample.pressure <= 0.0 {
            return;
        }

        let should_reset = self.assaults.get(&base_entity).is_none_or(|record| {
            record.attacker != sample.attacker || record.defender != sample.defender
        });
        if should_reset {
            self.assaults.insert(
                base_entity,
                BaseAssaultRecord::new(sample.defender, sample.attacker, sample.pre_capture_level),
            );
        }

        let record = self
            .assaults
            .get_mut(&base_entity)
            .expect("assault just inserted");
        record.pre_capture_level = sample.pre_capture_level;
        record.max_contest_depth = record.max_contest_depth.max(sample.contest_depth);
        record.peak_attacker_threat = record.peak_attacker_threat.max(sample.attacker_threat);
        record.no_attacker_time = 0.0;

        let entry = record.attackers.entry(sample.contributor).or_default();
        entry.amount += sample.pressure;
        entry.last_time = self.time_s;
    }

    pub fn record_defense_contribution(
        &mut self,
        base_entity: Entity,
        defender_unit: Entity,
        amount: f32,
    ) {
        if amount <= 0.0 {
            return;
        }
        let Some(record) = self.assaults.get_mut(&base_entity) else {
            return;
        };
        let entry = record.defenders.entry(defender_unit).or_default();
        entry.amount += amount;
        entry.last_time = self.time_s;
    }

    pub fn mark_attackers_absent(&mut self, base_entity: Entity, dt: f32) {
        if let Some(record) = self.assaults.get_mut(&base_entity) {
            record.no_attacker_time += dt;
        }
    }

    pub fn clear_assault(&mut self, base_entity: Entity) -> Option<BaseAssaultRecord> {
        self.assaults.remove(&base_entity)
    }

    pub fn take_held_defense_if_resolved(
        &mut self,
        base_entity: Entity,
        cfg: &GameConfig,
    ) -> Option<BaseAssaultRecord> {
        let should_resolve = self
            .assaults
            .get(&base_entity)
            .is_some_and(|record| record.no_attacker_time >= cfg.assault_reset_seconds);
        if should_resolve {
            self.assaults.remove(&base_entity)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FixedXpShare {
    pub entity: Entity,
    pub xp: f32,
}

pub fn unit_composite_power(_unit: &Unit, traits: &GeneticTraits) -> f32 {
    unit_strength(traits)
}

pub fn distribute_pool_with_main_share(
    pool: f32,
    now: f32,
    contribution_window_sec: f32,
    contributors: &HashMap<Entity, ContributionEntry>,
    main_bonus_target: Option<Entity>,
    main_bonus_weight: f32,
) -> Vec<FixedXpShare> {
    if pool <= 0.0 {
        return Vec::new();
    }

    let mut eligible: Vec<(Entity, f32, f32)> = contributors
        .iter()
        .filter_map(|(entity, entry)| {
            (now - entry.last_time <= contribution_window_sec).then_some((
                *entity,
                entry.amount.max(0.0),
                entry.amount.max(0.0),
            ))
        })
        .collect();
    if eligible.is_empty() {
        return Vec::new();
    }

    if let Some(main_bonus_target) = main_bonus_target
        && let Some((_, _, score)) = eligible
            .iter_mut()
            .find(|(entity, _, _)| *entity == main_bonus_target)
    {
        *score += main_bonus_weight;
    }

    let main_index = eligible
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(index, _)| index)
        .expect("non-empty eligible list");

    if eligible.len() == 1 {
        return vec![FixedXpShare {
            entity: eligible[0].0,
            xp: pool,
        }];
    }

    let main_entity = eligible[main_index].0;
    let main_share = pool * 0.60;
    let share_pool = pool - main_share;
    let others_total: f32 = eligible
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != main_index)
        .map(|(_, (_, amount, _))| *amount)
        .sum();

    let mut shares = vec![FixedXpShare {
        entity: main_entity,
        xp: main_share,
    }];
    if others_total <= 1e-6 {
        if let Some(main) = shares.first_mut() {
            main.xp = pool;
        }
        return shares;
    }

    for (index, (entity, amount, _)) in eligible.into_iter().enumerate() {
        if index == main_index {
            continue;
        }
        shares.push(FixedXpShare {
            entity,
            xp: share_pool * (amount / others_total),
        });
    }
    shares
}

pub fn award_xp_shares(shares: &[FixedXpShare], unit_query: &mut Query<&mut Unit>) {
    for share in shares {
        if share.xp <= 0.0 {
            continue;
        }
        if let Ok(mut unit) = unit_query.get_mut(share.entity) {
            unit.gain_xp(share.xp);
        }
    }
}
