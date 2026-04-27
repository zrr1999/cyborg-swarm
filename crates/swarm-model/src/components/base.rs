use bevy_ecs::prelude::*;

use crate::components::unit::Faction;
use crate::config::GameConfig;

/// Marks a starting-side anchor for labels / presentation.
/// The match result is no longer tied to headquarters ownership.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Headquarters {
    pub seat: Faction,
}

#[derive(Component, Clone, Debug)]
pub struct Base {
    pub faction: Faction,
    /// Linear base upgrade level. All bases begin at 0.
    pub level: u32,
    pub health: f32,
    pub max_health: f32,
    /// Countdown timer between unit spawns (seconds)
    pub spawn_timer: f32,
    /// Interval between unit spawns (seconds)
    pub spawn_interval: f32,
    /// Maximum number of units this base will maintain on the field
    pub max_units: u32,
    /// Radius within which enemy units can capture this base
    pub capture_radius: f32,
    /// Remaining time where spawning is suppressed after a capture.
    pub capture_weak_timer: f32,
    /// Cooldown before another manual repair can be applied to this base.
    pub repair_cooldown: f32,
    /// Countdown to next tower shot. Only active when `level >= BASE_TOWER_MIN_LEVEL`.
    pub tower_fire_timer: f32,
}

impl Base {
    pub fn player() -> Self {
        Self {
            faction: Faction::Player,
            level: 0,
            health: 500.0,
            max_health: 500.0,
            spawn_timer: 0.0,
            spawn_interval: 2.0,
            max_units: 40,
            capture_radius: 60.0,
            capture_weak_timer: 0.0,
            repair_cooldown: 0.0,
            tower_fire_timer: 0.0,
        }
    }

    pub fn enemy() -> Self {
        Self {
            faction: Faction::Enemy,
            level: 0,
            health: 500.0,
            max_health: 500.0,
            spawn_timer: 0.0,
            spawn_interval: 2.0,
            max_units: 40,
            capture_radius: 60.0,
            capture_weak_timer: 0.0,
            repair_cooldown: 0.0,
            tower_fire_timer: 0.0,
        }
    }

    pub fn neutral(_pos_hint: &str) -> Self {
        Self {
            faction: Faction::Neutral,
            level: 0,
            health: 300.0,
            max_health: 300.0,
            spawn_timer: 0.0,
            spawn_interval: 3.0,
            max_units: 20,
            capture_radius: 50.0,
            capture_weak_timer: 0.0,
            repair_cooldown: 0.0,
            tower_fire_timer: 0.0,
        }
    }

    pub fn is_destroyed(&self) -> bool {
        self.health <= 0.0
    }

    pub fn is_weak(&self) -> bool {
        self.capture_weak_timer > 0.0
    }

    pub fn tick_weak_state(&mut self, dt: f32) {
        self.capture_weak_timer = (self.capture_weak_timer - dt).max(0.0);
    }

    pub fn tick_repair_cooldown(&mut self, dt: f32) {
        self.repair_cooldown = (self.repair_cooldown - dt).max(0.0);
    }

    pub fn spawn_efficiency(&self, cfg: &GameConfig) -> f32 {
        if self.is_weak() {
            cfg.base_capture_spawn_efficiency
        } else {
            1.0
        }
    }

    pub fn apply_config_stats(&mut self, cfg: &GameConfig) {
        self.max_health = cfg.base_max_health_for(self.faction, self.level);
        self.max_units = cfg.base_capacity_for_level(self.level);
        self.spawn_interval = if self.faction == Faction::Neutral {
            cfg.base_spawn_interval * 1.5
        } else {
            cfg.base_spawn_interval
        };
        if self.faction == Faction::Neutral {
            self.max_units = (self.max_units / 2).max(1);
        }
        self.health = self.health.clamp(0.0, self.max_health);
    }

    pub fn upgrade(&mut self, cfg: &GameConfig) -> bool {
        if self.level >= cfg.base_max_level {
            return false;
        }
        let old_max = self.max_health;
        self.level += 1;
        self.apply_config_stats(cfg);
        self.health = (self.health + (self.max_health - old_max)).min(self.max_health);
        true
    }

    pub fn downgrade_on_capture(&mut self, cfg: &GameConfig) {
        self.level = self.level.saturating_sub(1);
        self.capture_weak_timer = cfg.base_capture_weak_seconds;
        self.repair_cooldown = 0.0;
        self.apply_config_stats(cfg);
        self.health = self.max_health * cfg.base_capture_health_fraction;
    }

    pub fn repair(&mut self, amount: f32) -> f32 {
        let healed = (self.max_health - self.health).min(amount).max(0.0);
        self.health += healed;
        healed
    }

    pub fn can_repair(&self) -> bool {
        self.repair_cooldown <= 0.0 && self.health < self.max_health
    }

    pub fn trigger_repair_cooldown(&mut self, cfg: &GameConfig) {
        self.repair_cooldown = cfg.base_repair_cooldown_sec;
    }
}
