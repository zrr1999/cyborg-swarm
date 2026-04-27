use crate::components::{Base, Faction, GeneticTraits, unit_strength};

/// Hard cap on unit level before theme-specific transformation rules must take over
pub const MAX_UNIT_LEVEL: u32 = 10;

/// Half-width / half-height of the game world in world-units
pub const WORLD_HALF_W: f32 = 480.0;
pub const WORLD_HALF_H: f32 = 270.0;

/// Visual radii
pub const UNIT_RADIUS: f32 = 5.0;
pub const BASE_RADIUS: f32 = 28.0;

/// Legacy compatibility constant from the old level-only radius model.
/// Body size now primarily follows attribute-derived strength.
pub const DEFAULT_LEVEL_RADIUS_BONUS_PER_LEVEL: f32 = 0.11;

/// Default: stats scale as `basis × (1 + bonus × (level - 1))` (additive from spawn baseline).
pub const DEFAULT_LEVEL_TRAIT_BONUS_PER_LEVEL: f32 = 0.06;

/// Tunable weights for unit movement: flow, separation, chase, retreat, rescue, beacon, base defense.
/// Treated as a **strategy parameter block** — gameplay code blends named actions using these values.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct SteeringMovementConfig {
    pub beacon_blend: f32,
    /// Multiplier on `beacon_blend` while retreating (weaker rally pull).
    pub beacon_blend_retreat_scale: f32,
    pub chase_weight_base: f32,
    pub chase_pursuit_suppression_factor: f32,
    pub retreat_blend_weight: f32,
    pub chase_steal_dsq_ratio: f32,
    pub inertia_blend_per_sec: f32,
    pub inertia_zero_decay_per_sec: f32,
    pub retreat_power_ratio: f32,
    pub retreat_count_power_ratio: f32,
    /// Weight on regroup vs withdraw when computing retreat direction.
    pub retreat_regroup_mix: f32,
    pub pursuit_speed_bonus: f32,
    pub retreat_speed_bonus: f32,
    /// Flow field returns zero in a BFS goal cell — scale max speed by this factor so units ease
    /// off after reaching the navigation target. Not applied while chasing, retreating, or when
    /// `rescue_strength` ≥ [`Self::arrival_rescue_override_min`].
    pub goal_arrival_speed_frac: f32,
    /// If `rescue_strength` is at or above this, do not apply [`Self::goal_arrival_speed_frac`].
    pub arrival_rescue_override_min: f32,
    /// Ally suppression above this counts as “under fire” for rescue.
    pub rescue_ally_suppression_threshold: f32,
    pub rescue_urgency_scale: f32,
    /// Max blend strength for rescue action (scaled by `rescue_strength` 0..1).
    pub rescue_blend_base: f32,
    pub flow_w_retreat: f32,
    pub flow_w_beacon_chase: f32,
    pub flow_w_beacon_no_chase: f32,
    pub flow_w_chase_no_beacon: f32,
    pub flow_w_default: f32,
    pub sep_w_retreat: f32,
    pub sep_w_beacon_chase: f32,
    pub sep_w_beacon_no_chase: f32,
    pub sep_w_chase_no_beacon: f32,
    pub sep_w_default: f32,
    /// Blend weight for moving toward an owned base that is taking capture damage (`urgency` 0..1 scales it).
    #[serde(default = "default_defend_base_blend")]
    pub defend_base_blend: f32,
    /// Multiplier on [`Self::retreat_blend_weight`] suppression while `defend_base_strength` is high.
    #[serde(default = "default_defend_base_retreat_suppression")]
    pub defend_base_retreat_suppression: f32,
    /// Reduces chase weight when defending home (`chase *= 1 - urgency * this`).
    #[serde(default = "default_defend_base_chase_suppression")]
    pub defend_base_chase_suppression: f32,
    /// Extra steering weight pulling units toward a per-squad **anchor** (spread captains laterally
    /// toward the enemy so blobs split into lanes). Scaled by per-frame `squad_spread_strength`.
    #[serde(default = "default_squad_spread_blend")]
    pub squad_spread_blend: f32,
    /// Extra steering weight pulling non-captains toward a captain-relative squad slot.
    #[serde(default = "default_squad_follow_blend")]
    pub squad_follow_blend: f32,
    /// Lateral offset between squad anchors as a multiple of mean sensor range.
    #[serde(default = "default_squad_spread_lateral_scale")]
    pub squad_spread_lateral_scale: f32,
    /// Forward offset of anchors from the ally centroid (× mean sensor).
    #[serde(default = "default_squad_spread_forward_scale")]
    pub squad_spread_forward_scale: f32,
    /// Within this distance of any **opposing** HQ base, spread is mostly disabled (assault / base attack).
    #[serde(default = "default_squad_spread_assault_radius")]
    pub squad_spread_assault_radius: f32,
    /// Lateral slot spacing around a captain as a multiple of mean sensor range.
    #[serde(default = "default_squad_follow_lateral_scale")]
    pub squad_follow_lateral_scale: f32,
    /// Back-row slot spacing behind a captain as a multiple of mean sensor range.
    #[serde(default = "default_squad_follow_depth_scale")]
    pub squad_follow_depth_scale: f32,
    /// Distance at which captain-follow steering reaches full strength, as a multiple of mean sensor range.
    #[serde(default = "default_squad_follow_catchup_scale")]
    pub squad_follow_catchup_scale: f32,
    /// Blend weight pulling **low-HP** units toward the nearest friendly base (healing / guard posture).
    #[serde(default = "default_home_rally_blend")]
    pub home_rally_blend: f32,
    /// Below this HP fraction (current / max), [`Self::home_rally_blend`] starts engaging.
    #[serde(default = "default_home_rally_health_threshold")]
    pub home_rally_health_threshold: f32,
    /// Scales down chase weight while a home rally is active (`chase *= 1 - strength * this`).
    #[serde(default = "default_home_rally_chase_suppression")]
    pub home_rally_chase_suppression: f32,
}

fn default_defend_base_blend() -> f32 {
    0.44
}

fn default_defend_base_retreat_suppression() -> f32 {
    0.62
}

fn default_defend_base_chase_suppression() -> f32 {
    0.48
}

fn default_squad_spread_blend() -> f32 {
    0.26
}

fn default_squad_follow_blend() -> f32 {
    0.34
}

fn default_squad_spread_lateral_scale() -> f32 {
    0.72
}

fn default_squad_spread_forward_scale() -> f32 {
    0.38
}

fn default_squad_spread_assault_radius() -> f32 {
    135.0
}

fn default_squad_follow_lateral_scale() -> f32 {
    0.2
}

fn default_squad_follow_depth_scale() -> f32 {
    0.17
}

fn default_squad_follow_catchup_scale() -> f32 {
    0.42
}

fn default_home_rally_blend() -> f32 {
    0.26
}

fn default_home_rally_health_threshold() -> f32 {
    0.58
}

fn default_home_rally_chase_suppression() -> f32 {
    0.52
}

impl Default for SteeringMovementConfig {
    fn default() -> Self {
        Self {
            beacon_blend: 0.048,
            beacon_blend_retreat_scale: 0.35,
            chase_weight_base: 0.38,
            chase_pursuit_suppression_factor: 0.78,
            retreat_blend_weight: 0.92,
            chase_steal_dsq_ratio: 0.58,
            inertia_blend_per_sec: 5.5,
            inertia_zero_decay_per_sec: 4.0,
            retreat_power_ratio: 1.18,
            retreat_count_power_ratio: 1.05,
            retreat_regroup_mix: 0.6,
            pursuit_speed_bonus: 0.28,
            retreat_speed_bonus: 0.18,
            goal_arrival_speed_frac: 0.22,
            arrival_rescue_override_min: 0.08,
            rescue_ally_suppression_threshold: 0.12,
            rescue_urgency_scale: 1.25,
            rescue_blend_base: 0.26,
            flow_w_retreat: 0.12,
            flow_w_beacon_chase: 0.38,
            flow_w_beacon_no_chase: 0.48,
            flow_w_chase_no_beacon: 0.4,
            flow_w_default: 0.68,
            sep_w_retreat: 0.24,
            sep_w_beacon_chase: 0.14,
            sep_w_beacon_no_chase: 0.2,
            sep_w_chase_no_beacon: 0.17,
            sep_w_default: 0.32,
            defend_base_blend: default_defend_base_blend(),
            defend_base_retreat_suppression: default_defend_base_retreat_suppression(),
            defend_base_chase_suppression: default_defend_base_chase_suppression(),
            squad_spread_blend: default_squad_spread_blend(),
            squad_follow_blend: default_squad_follow_blend(),
            squad_spread_lateral_scale: default_squad_spread_lateral_scale(),
            squad_spread_forward_scale: default_squad_spread_forward_scale(),
            squad_spread_assault_radius: default_squad_spread_assault_radius(),
            squad_follow_lateral_scale: default_squad_follow_lateral_scale(),
            squad_follow_depth_scale: default_squad_follow_depth_scale(),
            squad_follow_catchup_scale: default_squad_follow_catchup_scale(),
            home_rally_blend: default_home_rally_blend(),
            home_rally_health_threshold: default_home_rally_health_threshold(),
            home_rally_chase_suppression: default_home_rally_chase_suppression(),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PopulationOptimizerKind {
    #[default]
    GreyWolf,
    ParticleSwarm,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct GreyWolfOptimizerConfig {
    pub initial_a: f32,
    pub a_decay_per_generation: f32,
}

impl Default for GreyWolfOptimizerConfig {
    fn default() -> Self {
        Self {
            initial_a: 2.0,
            a_decay_per_generation: 0.02,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ParticleSwarmOptimizerConfig {
    pub swarm_size: u32,
    pub inertia_weight: f32,
    pub cognitive_weight: f32,
    pub social_weight: f32,
    pub guided_blend_weight: f32,
    pub max_velocity_scale: f32,
}

impl Default for ParticleSwarmOptimizerConfig {
    fn default() -> Self {
        Self {
            swarm_size: 8,
            inertia_weight: 0.62,
            cognitive_weight: 1.35,
            social_weight: 1.45,
            guided_blend_weight: 0.65,
            max_velocity_scale: 0.18,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default)]
pub struct PopulationOptimizerConfig {
    #[serde(default)]
    pub algorithm: PopulationOptimizerKind,
    #[serde(default)]
    pub grey_wolf: GreyWolfOptimizerConfig,
    #[serde(default)]
    pub particle_swarm: ParticleSwarmOptimizerConfig,
}

/// Runtime-configurable parameters loaded from `config.ron`.
/// Falls back to compiled defaults when the file is absent or malformed.
#[derive(bevy_ecs::prelude::Resource, serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct GameConfig {
    /// Hard cap on unit level before evolution/fusion is required
    pub max_unit_level: u32,
    /// Half-width of the game world in world-units
    pub world_half_w: f32,
    /// Half-height of the game world in world-units
    pub world_half_h: f32,
    /// Visual radius of a unit
    pub unit_radius: f32,
    /// Visual radius of a base
    pub base_radius: f32,
    /// σ for player-side mutation (fraction of mean, e.g. 0.10 = ±10%)
    pub player_mutation_sigma: f32,
    /// σ for enemy-side mutation
    pub enemy_mutation_sigma: f32,
    /// α weight: live-pool share in μ_base = α·μ_live + (1-α)·μ_dead
    pub alpha: f32,
    /// Rolling window size N for dead-unit mean
    pub dead_rolling_n: u32,
    /// Seconds between unit spawns from a base
    pub base_spawn_interval: f32,
    /// Base level-0 single-base capacity before per-base upgrades.
    pub base_max_units: u32,
    /// Base health for player / enemy level-0 bases.
    #[serde(default = "default_owned_base_health")]
    pub owned_base_health: f32,
    /// Base health for level-0 neutral bases.
    #[serde(default = "default_neutral_base_health")]
    pub neutral_base_health: f32,
    /// Added max health per base level.
    #[serde(default = "default_base_health_per_level")]
    pub base_health_per_level: f32,
    /// Added income per second per base level.
    #[serde(default = "default_base_income_per_level")]
    pub base_income_per_level: f32,
    /// Level-0 income per second for a controlled base.
    #[serde(default = "default_base_income_per_sec")]
    pub base_income_per_sec: f32,
    /// Added single-base unit capacity per base level.
    #[serde(default = "default_base_capacity_per_level")]
    pub base_capacity_per_level: u32,
    /// Maximum upgrade level for a base.
    #[serde(default = "default_base_max_level")]
    pub base_max_level: u32,
    /// Health fraction restored after a successful capture.
    #[serde(default = "default_base_capture_health_fraction")]
    pub base_capture_health_fraction: f32,
    /// Newly captured bases spawn at reduced efficiency for this many seconds.
    #[serde(default = "default_base_capture_weak_seconds")]
    pub base_capture_weak_seconds: f32,
    /// Spawn efficiency multiplier while a base is in the weak capture state.
    #[serde(default = "default_base_capture_spawn_efficiency")]
    pub base_capture_spawn_efficiency: f32,
    /// Passive HP/sec when a base is not under capture pressure.
    #[serde(default = "default_base_passive_regen_per_sec")]
    pub base_passive_regen_per_sec: f32,
    /// HP restored per repair action.
    #[serde(default = "default_base_repair_amount")]
    pub base_repair_amount: f32,
    /// Biomass cost per repaired HP.
    #[serde(default = "default_base_repair_cost_per_hp")]
    pub base_repair_cost_per_hp: f32,
    /// Cooldown after a manual repair before the same base can be repaired again.
    #[serde(default = "default_base_repair_cooldown_sec")]
    pub base_repair_cooldown_sec: f32,
    /// Base biomass cost for a base upgrade.
    #[serde(default = "default_base_upgrade_cost_base")]
    pub base_upgrade_cost_base: f32,
    /// Extra biomass cost per already-purchased base level.
    #[serde(default = "default_base_upgrade_cost_step")]
    pub base_upgrade_cost_step: f32,
    /// Faction-wide unit cap at expansion tier 0.
    #[serde(default = "default_global_unit_cap_base")]
    pub global_unit_cap_base: u32,
    /// Added faction-wide unit cap per global expansion tier.
    #[serde(default = "default_global_unit_cap_per_tier")]
    pub global_unit_cap_per_tier: u32,
    /// Maximum global expansion tier.
    #[serde(default = "default_global_unit_cap_tiers")]
    pub global_unit_cap_tiers: u32,
    /// Biomass cost for the first global expansion tier.
    #[serde(default = "default_global_unit_cap_cost_base")]
    pub global_unit_cap_cost_base: f32,
    /// Extra biomass cost per already-purchased global expansion tier.
    #[serde(default = "default_global_unit_cap_cost_step")]
    pub global_unit_cap_cost_step: f32,
    /// Domination hold time once one faction controls every base.
    #[serde(default = "default_domination_hold_seconds")]
    pub domination_hold_seconds: f32,
    /// Attacking contribution timeout for kill / capture / defense XP.
    #[serde(default = "default_xp_contribution_window_sec")]
    pub xp_contribution_window_sec: f32,
    /// Assaults fully reset if no attackers remain in a base radius for this long.
    #[serde(default = "default_assault_reset_seconds")]
    pub assault_reset_seconds: f32,
    /// Scales composite unit power into a death XP pool.
    #[serde(default = "default_kill_xp_factor")]
    pub kill_xp_factor: f32,
    /// Small extra weight assigned to the finisher when resolving kill XP.
    #[serde(default = "default_kill_xp_finisher_bonus_weight")]
    pub kill_xp_finisher_bonus_weight: f32,
    /// Base capture XP awarded for a successful flip at level 0.
    #[serde(default = "default_capture_xp_base")]
    pub capture_xp_base: f32,
    /// Added capture XP per pre-capture base level.
    #[serde(default = "default_capture_xp_per_level")]
    pub capture_xp_per_level: f32,
    /// Scales held-defense XP from attacker threat × contest depth.
    #[serde(default = "default_defense_xp_factor")]
    pub defense_xp_factor: f32,
    /// Free beacon mode switches are gated by a short cooldown.
    #[serde(default = "default_beacon_mode_switch_cooldown_sec")]
    pub beacon_mode_switch_cooldown_sec: f32,
    /// Duration of a temporary beacon strengthen spend.
    #[serde(default = "default_beacon_strengthen_duration_sec")]
    pub beacon_strengthen_duration_sec: f32,
    /// Biomass spent to strengthen the beacon.
    #[serde(default = "default_beacon_strengthen_cost")]
    pub beacon_strengthen_cost: f32,
    /// Multiplier applied to beacon behavior while strengthened.
    #[serde(default = "default_beacon_strengthen_multiplier")]
    pub beacon_strengthen_multiplier: f32,
    /// Chase-priority bonus for enemies inside a focus beacon region.
    #[serde(default = "default_beacon_focus_target_bonus")]
    pub beacon_focus_target_bonus: f32,
    /// Extra scale for repel mode to keep it soft.
    #[serde(default = "default_beacon_repel_strength_scale")]
    pub beacon_repel_strength_scale: f32,
    /// Enemy AI keeps at least this much biomass in reserve before non-emergency spending.
    #[serde(default = "default_enemy_macro_reserve_biomass")]
    pub enemy_macro_reserve_biomass: f32,
    /// Enemy AI considers a base urgent to repair above this risk score.
    #[serde(default = "default_enemy_macro_repair_risk_threshold")]
    pub enemy_macro_repair_risk_threshold: f32,
    /// Enemy AI considers global expansion once current units / cap reaches this ratio.
    #[serde(default = "default_enemy_macro_expand_cap_ratio")]
    pub enemy_macro_expand_cap_ratio: f32,
    /// Shared population-optimization algorithm and its parameter blocks.
    #[serde(default)]
    pub population_optimizer: PopulationOptimizerConfig,
    /// Movement steering: action weights and rescue / inertia parameters.
    #[serde(default)]
    pub steering: SteeringMovementConfig,
    /// Legacy compatibility field from the old level-only radius model.
    /// Actual body area now scales from [`crate::components::unit_strength`].
    #[serde(default = "default_level_radius_bonus_per_level")]
    pub level_radius_bonus_per_level: f32,
    /// Traits scale as `basis × (1 + bonus × (level - 1))` from spawn-time baseline.
    #[serde(default = "default_level_trait_bonus_per_level")]
    pub level_trait_bonus_per_level: f32,
    /// Fraction of genetic `max_health` restored per second while regen is active (e.g. `0.02` = 2%/s).
    /// `0` disables regen.
    #[serde(default = "default_unit_hp_regen_fraction_of_max_per_sec")]
    pub unit_hp_regen_fraction_of_max_per_sec: f32,
    /// Seconds without losing HP before % regen applies. Ignored when the fraction is `0`.
    #[serde(default = "default_unit_hp_regen_delay_after_damage_sec")]
    pub unit_hp_regen_delay_after_damage_sec: f32,
    /// Extra fraction of genetic `max_health` restored per second **while inside a friendly base**
    /// capture zone, after the same delay as [`Self::unit_hp_regen_delay_after_damage_sec`].
    /// Stacks with [`Self::unit_hp_regen_fraction_of_max_per_sec`]. `0` disables the bonus.
    #[serde(default = "default_unit_home_base_hp_regen_bonus_fraction_of_max_per_sec")]
    pub unit_home_base_hp_regen_bonus_fraction_of_max_per_sec: f32,
    /// Multiplier on incoming unit-vs-unit DPS while inside a **friendly** base (`Base::capture_radius`).
    /// `1.0` = no bonus; `0.75` ≈ 25% less damage taken.
    #[serde(default = "default_unit_friendly_base_damage_taken_multiplier")]
    pub unit_friendly_base_damage_taken_multiplier: f32,
    /// Which `assets/maps/{map_id}.ron` to load at startup (see [`crate::resources::map_layout::MapDefinition`]).
    #[serde(default = "default_map_id")]
    pub map_id: String,
    /// Flow-field capture target weight for **full-HP** bases (`grid_cost / weight`; higher weight = stronger pull).
    #[serde(default = "default_flow_capture_goal_weight_min")]
    pub flow_capture_goal_weight_min: f32,
    /// Flow-field weight when a base is nearly dead — low HP is “high value” / easier to flip.
    #[serde(default = "default_flow_capture_goal_weight_max")]
    pub flow_capture_goal_weight_max: f32,
    /// Curves `(1 - health_frac)` before mapping between min/max weight.
    #[serde(default = "default_flow_capture_goal_weight_gamma")]
    pub flow_capture_goal_weight_gamma: f32,
}

fn default_level_radius_bonus_per_level() -> f32 {
    DEFAULT_LEVEL_RADIUS_BONUS_PER_LEVEL
}

fn default_level_trait_bonus_per_level() -> f32 {
    DEFAULT_LEVEL_TRAIT_BONUS_PER_LEVEL
}

fn default_unit_hp_regen_fraction_of_max_per_sec() -> f32 {
    0.015
}

fn default_unit_hp_regen_delay_after_damage_sec() -> f32 {
    3.5
}

fn default_unit_home_base_hp_regen_bonus_fraction_of_max_per_sec() -> f32 {
    0.012
}

fn default_unit_friendly_base_damage_taken_multiplier() -> f32 {
    0.78
}

fn default_map_id() -> String {
    "classic".into()
}

fn default_flow_capture_goal_weight_min() -> f32 {
    0.42
}

fn default_flow_capture_goal_weight_max() -> f32 {
    2.35
}

fn default_flow_capture_goal_weight_gamma() -> f32 {
    1.08
}

fn default_owned_base_health() -> f32 {
    500.0
}

fn default_neutral_base_health() -> f32 {
    300.0
}

fn default_base_health_per_level() -> f32 {
    160.0
}

fn default_base_income_per_level() -> f32 {
    2.0
}

fn default_base_income_per_sec() -> f32 {
    5.0
}

fn default_base_capacity_per_level() -> u32 {
    6
}

fn default_base_max_level() -> u32 {
    3
}

fn default_base_capture_health_fraction() -> f32 {
    0.5
}

fn default_base_capture_weak_seconds() -> f32 {
    10.0
}

fn default_base_capture_spawn_efficiency() -> f32 {
    0.5
}

fn default_base_passive_regen_per_sec() -> f32 {
    5.0
}

fn default_base_repair_amount() -> f32 {
    90.0
}

fn default_base_repair_cost_per_hp() -> f32 {
    0.18
}

fn default_base_repair_cooldown_sec() -> f32 {
    6.0
}

fn default_base_upgrade_cost_base() -> f32 {
    55.0
}

fn default_base_upgrade_cost_step() -> f32 {
    28.0
}

fn default_global_unit_cap_base() -> u32 {
    80
}

fn default_global_unit_cap_per_tier() -> u32 {
    20
}

fn default_global_unit_cap_tiers() -> u32 {
    3
}

fn default_global_unit_cap_cost_base() -> f32 {
    45.0
}

fn default_global_unit_cap_cost_step() -> f32 {
    28.0
}

fn default_domination_hold_seconds() -> f32 {
    10.0
}

fn default_xp_contribution_window_sec() -> f32 {
    6.0
}

fn default_assault_reset_seconds() -> f32 {
    3.0
}

fn default_kill_xp_factor() -> f32 {
    0.42
}

fn default_kill_xp_finisher_bonus_weight() -> f32 {
    4.0
}

fn default_capture_xp_base() -> f32 {
    30.0
}

fn default_capture_xp_per_level() -> f32 {
    18.0
}

fn default_defense_xp_factor() -> f32 {
    0.16
}

fn default_beacon_mode_switch_cooldown_sec() -> f32 {
    1.5
}

fn default_beacon_strengthen_duration_sec() -> f32 {
    10.0
}

fn default_beacon_strengthen_cost() -> f32 {
    18.0
}

fn default_beacon_strengthen_multiplier() -> f32 {
    1.6
}

fn default_beacon_focus_target_bonus() -> f32 {
    0.45
}

fn default_beacon_repel_strength_scale() -> f32 {
    0.72
}

fn default_enemy_macro_reserve_biomass() -> f32 {
    30.0
}

fn default_enemy_macro_repair_risk_threshold() -> f32 {
    0.42
}

fn default_enemy_macro_expand_cap_ratio() -> f32 {
    0.85
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            max_unit_level: MAX_UNIT_LEVEL,
            world_half_w: WORLD_HALF_W,
            world_half_h: WORLD_HALF_H,
            unit_radius: UNIT_RADIUS,
            base_radius: BASE_RADIUS,
            player_mutation_sigma: 0.10,
            enemy_mutation_sigma: 0.08,
            alpha: 0.70,
            dead_rolling_n: 50,
            base_spawn_interval: 2.0,
            base_max_units: 40,
            owned_base_health: default_owned_base_health(),
            neutral_base_health: default_neutral_base_health(),
            base_health_per_level: default_base_health_per_level(),
            base_income_per_level: default_base_income_per_level(),
            base_income_per_sec: default_base_income_per_sec(),
            base_capacity_per_level: default_base_capacity_per_level(),
            base_max_level: default_base_max_level(),
            base_capture_health_fraction: default_base_capture_health_fraction(),
            base_capture_weak_seconds: default_base_capture_weak_seconds(),
            base_capture_spawn_efficiency: default_base_capture_spawn_efficiency(),
            base_passive_regen_per_sec: default_base_passive_regen_per_sec(),
            base_repair_amount: default_base_repair_amount(),
            base_repair_cost_per_hp: default_base_repair_cost_per_hp(),
            base_repair_cooldown_sec: default_base_repair_cooldown_sec(),
            base_upgrade_cost_base: default_base_upgrade_cost_base(),
            base_upgrade_cost_step: default_base_upgrade_cost_step(),
            global_unit_cap_base: default_global_unit_cap_base(),
            global_unit_cap_per_tier: default_global_unit_cap_per_tier(),
            global_unit_cap_tiers: default_global_unit_cap_tiers(),
            global_unit_cap_cost_base: default_global_unit_cap_cost_base(),
            global_unit_cap_cost_step: default_global_unit_cap_cost_step(),
            domination_hold_seconds: default_domination_hold_seconds(),
            xp_contribution_window_sec: default_xp_contribution_window_sec(),
            assault_reset_seconds: default_assault_reset_seconds(),
            kill_xp_factor: default_kill_xp_factor(),
            kill_xp_finisher_bonus_weight: default_kill_xp_finisher_bonus_weight(),
            capture_xp_base: default_capture_xp_base(),
            capture_xp_per_level: default_capture_xp_per_level(),
            defense_xp_factor: default_defense_xp_factor(),
            beacon_mode_switch_cooldown_sec: default_beacon_mode_switch_cooldown_sec(),
            beacon_strengthen_duration_sec: default_beacon_strengthen_duration_sec(),
            beacon_strengthen_cost: default_beacon_strengthen_cost(),
            beacon_strengthen_multiplier: default_beacon_strengthen_multiplier(),
            beacon_focus_target_bonus: default_beacon_focus_target_bonus(),
            beacon_repel_strength_scale: default_beacon_repel_strength_scale(),
            enemy_macro_reserve_biomass: default_enemy_macro_reserve_biomass(),
            enemy_macro_repair_risk_threshold: default_enemy_macro_repair_risk_threshold(),
            enemy_macro_expand_cap_ratio: default_enemy_macro_expand_cap_ratio(),
            population_optimizer: PopulationOptimizerConfig::default(),
            steering: SteeringMovementConfig::default(),
            level_radius_bonus_per_level: DEFAULT_LEVEL_RADIUS_BONUS_PER_LEVEL,
            level_trait_bonus_per_level: DEFAULT_LEVEL_TRAIT_BONUS_PER_LEVEL,
            unit_hp_regen_fraction_of_max_per_sec: default_unit_hp_regen_fraction_of_max_per_sec(),
            unit_hp_regen_delay_after_damage_sec: default_unit_hp_regen_delay_after_damage_sec(),
            unit_home_base_hp_regen_bonus_fraction_of_max_per_sec:
                default_unit_home_base_hp_regen_bonus_fraction_of_max_per_sec(),
            unit_friendly_base_damage_taken_multiplier:
                default_unit_friendly_base_damage_taken_multiplier(),
            map_id: default_map_id(),
            flow_capture_goal_weight_min: default_flow_capture_goal_weight_min(),
            flow_capture_goal_weight_max: default_flow_capture_goal_weight_max(),
            flow_capture_goal_weight_gamma: default_flow_capture_goal_weight_gamma(),
        }
    }
}

impl GameConfig {
    /// Collision / separation / visual outer radius.
    ///
    /// Body **area** tracks stable attribute-derived strength, so radius follows the square root of
    /// the strength ratio against the default unit baseline.
    #[inline]
    pub fn unit_body_radius(&self, traits: &GeneticTraits) -> f32 {
        let baseline = unit_strength(&GeneticTraits::default()).max(1e-6);
        let ratio = (unit_strength(traits) / baseline).max(0.05);
        self.unit_radius * ratio.sqrt()
    }

    /// Preferred center-to-center spacing before same-faction units start strongly separating.
    ///
    /// This keeps collision / occupancy behavior tied to the same attribute-derived body size used
    /// for rendering, so upgraded units naturally claim more room.
    #[inline]
    pub fn unit_pair_separation_distance(&self, lhs: &GeneticTraits, rhs: &GeneticTraits) -> f32 {
        self.unit_pair_separation_distance_from_radii(
            self.unit_body_radius(lhs),
            self.unit_body_radius(rhs),
        )
    }

    #[inline]
    pub fn unit_pair_separation_distance_from_radii(
        &self,
        lhs_radius: f32,
        rhs_radius: f32,
    ) -> f32 {
        2.1 * (lhs_radius + rhs_radius)
    }

    /// Load from a RON file, falling back to defaults on any error.
    pub fn load_from_ron(path: &str) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| ron::from_str::<GameConfig>(&s).ok())
            .unwrap_or_default()
    }

    #[inline]
    pub fn base_max_health_for(&self, faction: Faction, level: u32) -> f32 {
        let base = match faction {
            Faction::Neutral => self.neutral_base_health,
            Faction::Player | Faction::Enemy => self.owned_base_health,
        };
        base + self.base_health_per_level * level as f32
    }

    #[inline]
    pub fn base_capacity_for_level(&self, level: u32) -> u32 {
        self.base_max_units
            .saturating_add(self.base_capacity_per_level.saturating_mul(level))
    }

    #[inline]
    pub fn base_income_for_level(&self, level: u32) -> f32 {
        self.base_income_per_sec + self.base_income_per_level * level as f32
    }

    #[inline]
    pub fn base_upgrade_cost(&self, current_level: u32) -> f32 {
        self.base_upgrade_cost_base + self.base_upgrade_cost_step * current_level as f32
    }

    #[inline]
    pub fn base_repair_cost(&self, repaired_hp: f32) -> f32 {
        repaired_hp.max(0.0) * self.base_repair_cost_per_hp
    }

    #[inline]
    pub fn global_unit_cap_for_tier(&self, tier: u32) -> u32 {
        self.global_unit_cap_base
            .saturating_add(self.global_unit_cap_per_tier.saturating_mul(tier))
    }

    #[inline]
    pub fn global_expansion_cost(&self, current_tier: u32) -> f32 {
        self.global_unit_cap_cost_base + self.global_unit_cap_cost_step * current_tier as f32
    }

    #[inline]
    pub fn can_buy_global_expansion(&self, current_tier: u32) -> bool {
        current_tier < self.global_unit_cap_tiers
    }

    #[inline]
    pub fn base_capture_xp_pool(&self, pre_capture_level: u32) -> f32 {
        self.capture_xp_base + self.capture_xp_per_level * pre_capture_level as f32
    }

    /// Flow-field weight for a capture target: **lower HP ⇒ higher weight** (stronger pull).
    #[inline]
    pub fn flow_capture_goal_weight(&self, base: &Base) -> f32 {
        let frac = (base.health / base.max_health.max(1.0)).clamp(0.0, 1.0);
        let vulnerability = 1.0 - frac;
        let g = self.flow_capture_goal_weight_gamma;
        self.flow_capture_goal_weight_min
            + (self.flow_capture_goal_weight_max - self.flow_capture_goal_weight_min)
                * vulnerability.powf(g)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_body_area_tracks_strength_ratio() {
        let cfg = GameConfig::default();
        let base = GeneticTraits::default();
        let stronger = base.scale_by(1.6);

        let base_r = cfg.unit_body_radius(&base);
        let stronger_r = cfg.unit_body_radius(&stronger);
        let area_ratio = (stronger_r / base_r).powi(2);
        let strength_ratio = unit_strength(&stronger) / unit_strength(&base);

        assert!((area_ratio - strength_ratio).abs() < 1e-4);
    }

    #[test]
    fn pair_separation_distance_grows_with_body_radius() {
        let cfg = GameConfig::default();
        let base = GeneticTraits::default();
        let stronger = base.scale_by(1.5);

        let base_sep = cfg.unit_pair_separation_distance(&base, &base);
        let stronger_sep = cfg.unit_pair_separation_distance(&base, &stronger);

        assert!(stronger_sep > base_sep);
    }
}
