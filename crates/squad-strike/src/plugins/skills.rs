use bevy::prelude::*;
use swarm_model::{GeneticTraits, Unit};

use crate::plugins::{
    combat::GameOver,
    enemies::WaveState,
    game_flow::RestartRequested,
    meta_progression::MetaProgression,
    squad::{CaptainAnnouncement, SquadMember},
};

pub struct SkillsPlugin;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunSkillKind {
    AssimilationProtocol,
    TriagePulse,
    SalvageProtocol,
    TargetingRelay,
    RapidCycler,
    GuardFormation,
    SplitCatalyst,
    MomentumDrive,
}

impl RunSkillKind {
    const BASE_POOL: [Self; 7] = [
        Self::TargetingRelay,
        Self::TriagePulse,
        Self::RapidCycler,
        Self::GuardFormation,
        Self::MomentumDrive,
        Self::SplitCatalyst,
        Self::SalvageProtocol,
    ];
    const DRAFT_SIZE: usize = 3;

    pub fn available_choices(meta: &MetaProgression) -> Vec<Self> {
        let mut choices = Self::BASE_POOL.to_vec();
        if meta.assimilation_unlocked() {
            choices.insert(0, Self::AssimilationProtocol);
        }
        choices
    }

    pub fn draft_choices(
        wave: u32,
        skills: &RunSkills,
        meta: &MetaProgression,
    ) -> [Option<Self>; Self::DRAFT_SIZE] {
        let mut pool = Self::available_choices(meta);
        let span = pool.len().max(1);
        pool.sort_by_key(|kind| {
            let rotation = (kind.draft_order() + wave as usize * 2) % span;
            (skills.stacks_for(*kind), rotation)
        });

        let mut choices = [None; Self::DRAFT_SIZE];
        for (slot, kind) in choices.iter_mut().zip(pool) {
            *slot = Some(kind);
        }
        choices
    }

    pub fn is_available(self, meta: &MetaProgression) -> bool {
        self != Self::AssimilationProtocol || meta.assimilation_unlocked()
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::AssimilationProtocol => "Assimilation Protocol",
            Self::TriagePulse => "Triage Pulse",
            Self::SalvageProtocol => "Salvage Protocol",
            Self::TargetingRelay => "Targeting Relay",
            Self::RapidCycler => "Rapid Cycler",
            Self::GuardFormation => "Guard Formation",
            Self::SplitCatalyst => "Split Catalyst",
            Self::MomentumDrive => "Momentum Drive",
        }
    }

    pub fn description(self, current_stacks: u32) -> String {
        match self {
            Self::AssimilationProtocol => format!(
                "Enemy kills spawn a new allied unit with {}% of the slain unit's stats.",
                (assimilation_scale(current_stacks + 1) * 100.0).round() as u32
            ),
            Self::TriagePulse => format!(
                "Each cleared wave heals all living allies for {}% max HP.",
                triage_heal_fraction(current_stacks + 1).round() as u32
            ),
            Self::SalvageProtocol => {
                format!("Enemy kills grant +{} extra Cells.", current_stacks + 1)
            }
            Self::TargetingRelay => format!(
                "Squad attacks deal +{}% damage.",
                ((targeting_damage_multiplier(current_stacks + 1) - 1.0) * 100.0).round() as u32
            ),
            Self::RapidCycler => format!(
                "Squad attack cycles run +{}% faster.",
                ((rapid_fire_multiplier(current_stacks + 1) - 1.0) * 100.0).round() as u32
            ),
            Self::GuardFormation => format!(
                "Incoming damage is reduced by {}%.",
                ((1.0 - guard_damage_taken_multiplier(current_stacks + 1)) * 100.0).round() as u32
            ),
            Self::SplitCatalyst => format!(
                "Cleared waves grant {:.0} XP to every living ally.",
                split_catalyst_xp(current_stacks + 1, 1)
            ),
            Self::MomentumDrive => format!(
                "Player squad movement speed +{}%.",
                ((momentum_speed_multiplier(current_stacks + 1) - 1.0) * 100.0).round() as u32
            ),
        }
    }

    pub fn short_pickup(self, next_stacks: u32) -> String {
        format!("{} Lv {}", self.title(), next_stacks)
    }

    fn draft_order(self) -> usize {
        match self {
            Self::TargetingRelay => 0,
            Self::TriagePulse => 1,
            Self::RapidCycler => 2,
            Self::GuardFormation => 3,
            Self::MomentumDrive => 4,
            Self::SplitCatalyst => 5,
            Self::SalvageProtocol => 6,
            Self::AssimilationProtocol => 7,
        }
    }
}

#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct RunSkills {
    pub assimilation_stacks: u32,
    pub triage_stacks: u32,
    pub salvage_stacks: u32,
    pub targeting_stacks: u32,
    pub rapid_stacks: u32,
    pub guard_stacks: u32,
    pub split_catalyst_stacks: u32,
    pub momentum_stacks: u32,
}

impl RunSkills {
    pub fn stacks_for(&self, kind: RunSkillKind) -> u32 {
        match kind {
            RunSkillKind::AssimilationProtocol => self.assimilation_stacks,
            RunSkillKind::TriagePulse => self.triage_stacks,
            RunSkillKind::SalvageProtocol => self.salvage_stacks,
            RunSkillKind::TargetingRelay => self.targeting_stacks,
            RunSkillKind::RapidCycler => self.rapid_stacks,
            RunSkillKind::GuardFormation => self.guard_stacks,
            RunSkillKind::SplitCatalyst => self.split_catalyst_stacks,
            RunSkillKind::MomentumDrive => self.momentum_stacks,
        }
    }

    pub fn gain(&mut self, kind: RunSkillKind) -> u32 {
        let stacks = match kind {
            RunSkillKind::AssimilationProtocol => &mut self.assimilation_stacks,
            RunSkillKind::TriagePulse => &mut self.triage_stacks,
            RunSkillKind::SalvageProtocol => &mut self.salvage_stacks,
            RunSkillKind::TargetingRelay => &mut self.targeting_stacks,
            RunSkillKind::RapidCycler => &mut self.rapid_stacks,
            RunSkillKind::GuardFormation => &mut self.guard_stacks,
            RunSkillKind::SplitCatalyst => &mut self.split_catalyst_stacks,
            RunSkillKind::MomentumDrive => &mut self.momentum_stacks,
        };
        *stacks += 1;
        *stacks
    }
}

#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct SkillDraftState {
    pub active: bool,
    pub cleared_wave: u32,
    pub choices: [Option<RunSkillKind>; RunSkillKind::DRAFT_SIZE],
}

#[derive(Message, Clone, Copy, Debug)]
pub struct ChooseRunSkill(pub RunSkillKind);

impl Plugin for SkillsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RunSkills>()
            .init_resource::<SkillDraftState>()
            .add_message::<ChooseRunSkill>()
            .add_systems(
                Update,
                (
                    process_wave_clear_system,
                    choose_run_skill_system,
                    reset_skills_on_restart,
                ),
            );
    }
}

fn process_wave_clear_system(
    wave: Res<WaveState>,
    game_over: Res<GameOver>,
    mut draft: ResMut<SkillDraftState>,
    skills: Res<RunSkills>,
    meta: Res<MetaProgression>,
    mut announcement: ResMut<CaptainAnnouncement>,
    mut players: Query<(&mut Unit, &GeneticTraits), With<SquadMember>>,
) {
    if game_over.active
        || wave.wave == 0
        || wave.enemies_alive > 0
        || draft.cleared_wave >= wave.wave
    {
        return;
    }

    draft.cleared_wave = wave.wave;
    draft.choices = RunSkillKind::draft_choices(wave.wave, &skills, &meta);

    let heal_percent =
        meta.field_triage_heal_fraction() + triage_heal_fraction(skills.triage_stacks);
    if heal_percent > 0.0 {
        let heal_frac = heal_percent / 100.0;
        for (mut unit, traits) in &mut players {
            if unit.is_dead() {
                continue;
            }
            unit.current_health =
                (unit.current_health + traits.max_health * heal_frac).min(traits.max_health);
        }
    }
    if skills.split_catalyst_stacks > 0 {
        let xp = split_catalyst_xp(skills.split_catalyst_stacks, wave.wave);
        for (mut unit, _) in &mut players {
            if unit.is_dead() {
                continue;
            }
            unit.gain_xp(xp);
        }
    }

    draft.active = true;
    announcement.show(format!("Wave {} cleared · choose an upgrade", wave.wave));
}

fn choose_run_skill_system(
    mut events: MessageReader<ChooseRunSkill>,
    mut draft: ResMut<SkillDraftState>,
    mut skills: ResMut<RunSkills>,
    meta: Res<MetaProgression>,
    mut announcement: ResMut<CaptainAnnouncement>,
) {
    if !draft.active {
        for _ in events.read() {}
        return;
    }

    let Some(ChooseRunSkill(kind)) = events.read().next().copied() else {
        return;
    };
    if !kind.is_available(&meta) || !draft.choices.contains(&Some(kind)) {
        announcement.show("Skill path locked");
        return;
    }
    let next_stacks = skills.gain(kind);
    draft.active = false;
    announcement.show(kind.short_pickup(next_stacks));
}

fn reset_skills_on_restart(
    mut restarts: MessageReader<RestartRequested>,
    mut skills: ResMut<RunSkills>,
    mut draft: ResMut<SkillDraftState>,
) {
    let mut saw_restart = false;
    for _ in restarts.read() {
        saw_restart = true;
    }
    if !saw_restart {
        return;
    }
    *skills = RunSkills::default();
    *draft = SkillDraftState::default();
}

pub fn triage_heal_fraction(stacks: u32) -> f32 {
    22.0 * stacks as f32
}

pub fn assimilation_scale(stacks: u32) -> f32 {
    (0.5 + (stacks.saturating_sub(1)) as f32 * 0.12).min(0.86)
}

pub fn targeting_damage_multiplier(stacks: u32) -> f32 {
    1.0 + stacks as f32 * 0.14
}

pub fn rapid_fire_multiplier(stacks: u32) -> f32 {
    1.0 + stacks as f32 * 0.10
}

pub fn guard_damage_taken_multiplier(stacks: u32) -> f32 {
    (1.0 - stacks as f32 * 0.10).max(0.55)
}

pub fn split_catalyst_xp(stacks: u32, wave: u32) -> f32 {
    stacks as f32 * (14.0 + wave.saturating_sub(1) as f32 * 2.0)
}

pub fn momentum_speed_multiplier(stacks: u32) -> f32 {
    1.0 + stacks as f32 * 0.08
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combat_skill_scalars_progress_with_stacks() {
        assert!(targeting_damage_multiplier(2) > targeting_damage_multiplier(1));
        assert!(rapid_fire_multiplier(2) > rapid_fire_multiplier(1));
        assert!(guard_damage_taken_multiplier(2) < guard_damage_taken_multiplier(1));
        assert!(momentum_speed_multiplier(2) > momentum_speed_multiplier(1));
        assert!(split_catalyst_xp(1, 6) > split_catalyst_xp(1, 1));
    }
}
