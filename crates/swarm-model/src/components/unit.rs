use bevy_ecs::prelude::*;
use rand::RngExt;

use crate::config::MAX_UNIT_LEVEL;

/// The six heritable genetic traits that drive the evolutionary simulation.
/// All values are subject to mutation on each new birth from the GenePool.
#[derive(Component, Clone, Debug)]
pub struct GeneticTraits {
    /// Base maximum health points
    pub max_health: f32,
    /// Damage dealt per hit
    pub attack: f32,
    /// World-units per second movement speed
    pub speed: f32,
    /// Radius within which the unit detects enemies/allies
    pub sensor_range: f32,
    /// Flat damage reduction per hit
    pub armor: f32,
    /// Attacks per second multiplier (DPS = attack × attack_speed)
    pub attack_speed: f32,
}

/// Snapshot of genetic traits at spawn / transformation boundaries — used to apply
/// **linear-in-level** scaling: `traits = basis × (1 + k × (level - 1))`.
#[derive(Component, Clone, Debug)]
pub struct GeneticTraitBasis(pub GeneticTraits);

impl Default for GeneticTraits {
    fn default() -> Self {
        Self {
            max_health: 100.0,
            attack: 10.0,
            speed: 60.0,
            sensor_range: 80.0,
            armor: 5.0,
            attack_speed: 1.0,
        }
    }
}

impl GeneticTraits {
    /// Weighted blend: self * w_self + other * w_other
    pub fn blend_with(&self, other: &Self, w_self: f32, w_other: f32) -> Self {
        Self {
            max_health: self.max_health * w_self + other.max_health * w_other,
            attack: self.attack * w_self + other.attack * w_other,
            speed: self.speed * w_self + other.speed * w_other,
            sensor_range: self.sensor_range * w_self + other.sensor_range * w_other,
            armor: self.armor * w_self + other.armor * w_other,
            attack_speed: self.attack_speed * w_self + other.attack_speed * w_other,
        }
    }

    /// Incremental Welford update: move mean toward x with step 1/n
    pub fn welford_step(&mut self, x: &Self, n: f32) {
        let inv_n = 1.0 / n;
        self.max_health += (x.max_health - self.max_health) * inv_n;
        self.attack += (x.attack - self.attack) * inv_n;
        self.speed += (x.speed - self.speed) * inv_n;
        self.sensor_range += (x.sensor_range - self.sensor_range) * inv_n;
        self.armor += (x.armor - self.armor) * inv_n;
        self.attack_speed += (x.attack_speed - self.attack_speed) * inv_n;
    }

    /// Scale all traits by a factor (used for level-up bonuses)
    pub fn scale_by(&self, factor: f32) -> Self {
        Self {
            max_health: self.max_health * factor,
            attack: self.attack * factor,
            speed: self.speed * factor,
            sensor_range: self.sensor_range * factor,
            armor: self.armor * factor,
            attack_speed: self.attack_speed * factor,
        }
    }
}

impl crate::Genome for GeneticTraits {
    fn baseline() -> Self {
        Self::default()
    }

    fn blend(&self, other: &Self, w_self: f32, w_other: f32) -> Self {
        self.blend_with(other, w_self, w_other)
    }

    fn scale(&self, factor: f32) -> Self {
        self.scale_by(factor)
    }

    fn welford_update(&mut self, x: &Self, n: f32) {
        self.welford_step(x, n)
    }

    fn mutate(&self, rng: &mut impl rand::Rng, sigma: f32) -> Self {
        Self {
            max_health: (self.max_health * (1.0 + rng.random_range(-sigma..sigma)))
                .clamp(10.0, 2000.0),
            attack: (self.attack * (1.0 + rng.random_range(-sigma..sigma))).clamp(1.0, 500.0),
            speed: (self.speed * (1.0 + rng.random_range(-sigma..sigma))).clamp(15.0, 180.0),
            sensor_range: (self.sensor_range * (1.0 + rng.random_range(-sigma..sigma)))
                .clamp(20.0, 300.0),
            armor: (self.armor * (1.0 + rng.random_range(-sigma..sigma))).clamp(0.0, 100.0),
            attack_speed: (self.attack_speed * (1.0 + rng.random_range(-sigma..sigma)))
                .clamp(0.1, 5.0),
        }
    }

    fn dimension_count() -> usize {
        6
    }

    fn as_slice(&self) -> Vec<f32> {
        vec![
            self.max_health,
            self.attack,
            self.speed,
            self.sensor_range,
            self.armor,
            self.attack_speed,
        ]
    }

    fn from_slice(values: &[f32]) -> Self {
        Self {
            max_health: values[0],
            attack: values[1],
            speed: values[2],
            sensor_range: values[3],
            armor: values[4],
            attack_speed: values[5],
        }
    }
}

/// Stable attribute-derived strength used as the common basis for body area, tactical strength
/// comparison, and unit value calculations.
pub fn unit_strength(traits: &GeneticTraits) -> f32 {
    let dps = traits.attack * traits.attack_speed;
    let durability = traits.max_health + traits.armor * 12.0;
    let reach = traits.sensor_range * 0.25;
    dps * 0.55 + durability * 0.08 + traits.speed * 0.14 + reach * 0.06
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Faction {
    Player,
    Enemy,
    Neutral,
}

impl Faction {
    pub fn is_enemy_of(&self, other: &Faction) -> bool {
        match (self, other) {
            (Faction::Neutral, _) | (_, Faction::Neutral) => false,
            (a, b) => a != b,
        }
    }
}

/// Live unit state (not inherited, resets each lifecycle)
#[derive(Component, Clone, Debug)]
pub struct Unit {
    pub level: u32,
    pub xp: f32,
    pub xp_to_next: f32,
    pub current_health: f32,
    pub faction: Faction,
    /// Fusion offspring get accelerated XP gain
    pub fusion_xp_multiplier: f32,
    /// Seconds since this unit last lost HP from damage (used for delayed % regen).
    pub seconds_since_last_damage: f32,
}

/// Initial value so new spawns are treated as "out of combat" for regen delay.
const SPAWN_REGEN_TIMER_READY: f32 = 1_000.0;

impl Unit {
    pub fn new(traits: &GeneticTraits, faction: Faction) -> Self {
        Self {
            level: 1,
            xp: 0.0,
            xp_to_next: xp_threshold(1),
            current_health: traits.max_health,
            faction,
            fusion_xp_multiplier: 1.0,
            seconds_since_last_damage: SPAWN_REGEN_TIMER_READY,
        }
    }

    pub fn new_fused(traits: &GeneticTraits, faction: Faction) -> Self {
        Self {
            fusion_xp_multiplier: 1.5,
            ..Self::new(traits, faction)
        }
    }

    pub fn is_max_level(&self) -> bool {
        self.is_max_level_with(MAX_UNIT_LEVEL)
    }

    pub fn is_max_level_with(&self, max_unit_level: u32) -> bool {
        self.level >= max_unit_level
    }

    pub fn is_dead(&self) -> bool {
        self.current_health <= 0.0
    }

    pub fn gain_xp(&mut self, amount: f32) {
        self.xp += amount * self.fusion_xp_multiplier;
    }

    /// Try to level up. Returns true if level increased.
    ///
    /// `level_trait_bonus_per_level` is the per-level fraction of **baseline** stats added for each
    /// level after 1 (e.g. `0.06` → at level 3, multiplier `1 + 0.06×2` on `basis`).
    pub fn try_level_up(
        &mut self,
        traits: &mut GeneticTraits,
        basis: &GeneticTraitBasis,
        level_trait_bonus_per_level: f32,
    ) -> bool {
        self.try_level_up_with(traits, basis, level_trait_bonus_per_level, MAX_UNIT_LEVEL)
    }

    pub fn try_level_up_with(
        &mut self,
        traits: &mut GeneticTraits,
        basis: &GeneticTraitBasis,
        level_trait_bonus_per_level: f32,
        max_unit_level: u32,
    ) -> bool {
        if self.xp >= self.xp_to_next && !self.is_max_level_with(max_unit_level) {
            self.level += 1;
            self.xp -= self.xp_to_next;
            self.xp_to_next = xp_threshold(self.level);
            let f = 1.0 + level_trait_bonus_per_level * (self.level - 1) as f32;
            *traits = basis.0.scale_by(f);
            self.current_health = traits.max_health;
            self.seconds_since_last_damage = SPAWN_REGEN_TIMER_READY;
            true
        } else {
            false
        }
    }
}

/// Lifecycle fitness signals used by theme-level swarm algorithms.
#[derive(Component, Clone, Debug, Default)]
pub struct UnitFitness {
    pub damage_dealt: f32,
    pub damage_taken_survived: f32,
    pub survival_time: f32,
}

impl UnitFitness {
    pub fn score(&self) -> f32 {
        self.damage_dealt + self.damage_taken_survived * 0.5 + self.survival_time * 0.3
    }
}

/// Smoothed steering and sticky chase target to reduce jitter when blending movement.
#[derive(Component, Clone, Debug)]
pub struct SteeringInertia {
    /// Exponentially blended desired direction (used for actual motion).
    pub smoothed_dir: bevy_math::Vec2,
    /// Locked enemy to pursue until invalid or a clearly better target appears.
    pub chase_target: Option<Entity>,
}

impl Default for SteeringInertia {
    fn default() -> Self {
        Self {
            smoothed_dir: bevy_math::Vec2::ZERO,
            chase_target: None,
        }
    }
}

/// XP needed to reach next level from current level
pub fn xp_threshold(level: u32) -> f32 {
    // Quadratic: cheap early levels, steeply increasing cost later.
    // Lv1→2: 50 XP  Lv3→4: 450  Lv5→6: 1250  Lv9→10: 4050
    50.0 * (level as f32).powi(2)
}

/// Current movement target for a unit
#[derive(Component, Clone, Debug)]
pub struct UnitTarget {
    pub position: bevy_math::Vec2,
}

/// Which [`super::base::Base`] entity spawned this unit; used for per-base spawn caps.
#[derive(Component, Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct SpawnedByBase(pub Entity);

/// Buildup while taking damage; reduces movement speed and marks targets for pursuit.
#[derive(Component, Clone, Debug)]
pub struct MovementSuppression {
    /// 0 = no suppression, 1 = fully suppressed (max slow).
    pub level: f32,
}

impl Default for MovementSuppression {
    fn default() -> Self {
        Self { level: 0.0 }
    }
}

/// Marker: this unit has reached max level and is eligible for theme-specific transformation rules.
#[derive(Component)]
pub struct ReadyToEvolve;

/// Legacy marker for one-shot self transformation flows used by some themes.
#[derive(Component)]
pub struct Evolved;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum TacticalClass {
    #[default]
    Drone, // balanced — no dominant trait
    Siege,    // attack dominant
    Vanguard, // max_health or armor dominant
    Scout,    // speed dominant
    Sentinel, // sensor_range dominant
    Hive,     // attack_speed dominant
}

impl TacticalClass {
    /// Number of polygon sides for this class's visual representation.
    /// Returns 0 for Drone (use Circle mesh).
    pub fn polygon_sides(&self) -> u32 {
        match self {
            Self::Drone => 0,    // circle
            Self::Siege => 5,    // pentagon
            Self::Vanguard => 6, // hexagon
            Self::Scout => 3,    // triangle
            Self::Sentinel => 4, // diamond/square
            Self::Hive => 8,     // octagon
        }
    }
}

/// Classify a unit based on which trait deviates most from default.
/// Returns `Drone` if no single trait dominates by >30% above the mean ratio.
pub fn classify(traits: &GeneticTraits) -> TacticalClass {
    let defaults = GeneticTraits::default();
    let ratios = [
        ("max_health", traits.max_health / defaults.max_health),
        ("attack", traits.attack / defaults.attack),
        ("speed", traits.speed / defaults.speed),
        ("sensor_range", traits.sensor_range / defaults.sensor_range),
        ("armor", traits.armor / defaults.armor),
        ("attack_speed", traits.attack_speed / defaults.attack_speed),
    ];

    let mean_ratio: f32 = ratios.iter().map(|(_, r)| r).sum::<f32>() / ratios.len() as f32;

    // Find the trait with highest ratio relative to the mean
    let (dominant_name, dominant_ratio) = ratios
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .unwrap();

    // Must exceed mean by 30% to specialize
    if *dominant_ratio < mean_ratio * 1.3 {
        return TacticalClass::Drone;
    }

    match *dominant_name {
        "attack" => TacticalClass::Siege,
        "max_health" | "armor" => TacticalClass::Vanguard,
        "speed" => TacticalClass::Scout,
        "sensor_range" => TacticalClass::Sentinel,
        "attack_speed" => TacticalClass::Hive,
        _ => TacticalClass::Drone,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_traits() {
        let t = GeneticTraits::default();
        assert_eq!(t.max_health, 100.0);
        assert_eq!(t.attack_speed, 1.0);
    }

    #[test]
    fn test_scale() {
        let t = GeneticTraits::default().scale_by(2.0);
        assert_eq!(t.max_health, 200.0);
        assert_eq!(t.attack, 20.0);
    }

    #[test]
    fn test_blend_equal_weights() {
        let a = GeneticTraits::default();
        let b = GeneticTraits {
            max_health: 200.0,
            ..Default::default()
        };
        let blended = a.blend_with(&b, 0.5, 0.5);
        assert!((blended.max_health - 150.0).abs() < 0.01);
    }

    #[test]
    fn test_classify_balanced_is_drone() {
        let t = GeneticTraits::default();
        assert_eq!(classify(&t), TacticalClass::Drone);
    }

    #[test]
    fn test_classify_high_attack_is_siege() {
        let t = GeneticTraits {
            attack: 30.0,
            ..Default::default()
        };
        assert_eq!(classify(&t), TacticalClass::Siege);
    }

    #[test]
    fn test_classify_high_speed_is_scout() {
        let t = GeneticTraits {
            speed: 180.0,
            ..Default::default()
        };
        assert_eq!(classify(&t), TacticalClass::Scout);
    }

    #[test]
    fn test_level_up() {
        let mut traits = GeneticTraits::default();
        let basis = GeneticTraitBasis(traits.clone());
        let mut unit = Unit::new(&traits, Faction::Player);
        unit.xp = 999.0; // way above threshold
        let bonus = crate::config::DEFAULT_LEVEL_TRAIT_BONUS_PER_LEVEL;
        assert!(unit.try_level_up(&mut traits, &basis, bonus));
        assert_eq!(unit.level, 2);
        let expected = 100.0 * (1.0 + bonus * 1.0);
        assert!((traits.max_health - expected).abs() < 0.01);
    }

    #[test]
    fn test_fusion_xp_multiplier() {
        let traits = GeneticTraits::default();
        let mut unit = Unit::new_fused(&traits, Faction::Player);
        assert_eq!(unit.fusion_xp_multiplier, 1.5);
        unit.gain_xp(10.0);
        assert!((unit.xp - 15.0).abs() < 0.01);
    }

    #[test]
    fn test_xp_threshold_increases() {
        assert!(xp_threshold(2) > xp_threshold(1));
        assert!(xp_threshold(3) > xp_threshold(2));
    }

    #[test]
    fn test_faction_enemy_of() {
        assert!(Faction::Player.is_enemy_of(&Faction::Enemy));
        assert!(!Faction::Player.is_enemy_of(&Faction::Player));
        assert!(!Faction::Neutral.is_enemy_of(&Faction::Player));
        assert!(!Faction::Player.is_enemy_of(&Faction::Neutral));
    }

    #[test]
    fn test_unit_strength_increases_with_scaled_traits() {
        let base = GeneticTraits::default();
        let leveled = base.scale_by(1.4);
        assert!(unit_strength(&leveled) > unit_strength(&base));
    }

    #[test]
    fn test_unit_fitness_score_combines_signals() {
        let fitness = UnitFitness {
            damage_dealt: 10.0,
            damage_taken_survived: 6.0,
            survival_time: 5.0,
        };
        assert!((fitness.score() - 14.5).abs() < 0.01);
    }
}
