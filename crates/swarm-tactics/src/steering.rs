//! Movement steering as weighted **actions** (flow, separation, chase, retreat, rescue, beacon, defend base).
//! All weights live in [`swarm_model::SteeringMovementConfig`] for tuning.

use bevy_math::Vec2;

use swarm_model::SteeringMovementConfig;

/// Inputs for one unit — each vector is already the desired direction for that action (or zero).
#[derive(Clone, Debug)]
pub struct SteeringBlendInput {
    pub flow_dir: Vec2,
    pub separation: Vec2,
    pub chase_dir: Vec2,
    pub retreat_dir: Vec2,
    /// Unit direction toward the chosen distressed ally (if any).
    pub rescue_dir: Vec2,
    /// 0..1 — how urgent rescue is (ally suppression + low HP).
    pub rescue_strength: f32,
    pub beacon_force: Vec2,
    /// Toward a friendly base taking capture damage this frame.
    pub defend_base_dir: Vec2,
    /// 0..1 — scales with capture pressure at that base.
    pub defend_base_strength: f32,
    pub should_retreat: bool,
    pub has_chase: bool,
    pub has_beacon: bool,
    /// True when [`Self::defend_base_strength`] is meaningful (affects flow vs separation weights).
    pub has_defend_base: bool,
    /// Toward nearest friendly base when hurt — encourages guarding / healing near home.
    pub home_rally_dir: Vec2,
    pub home_rally_strength: f32,
    pub has_home_rally: bool,
    pub nearest_enemy_sup: f32,
    /// Toward per-squad peel anchor (built each frame in `units::build_squad_spread_hints`).
    pub squad_spread_dir: Vec2,
    /// 0..1 — lowered while chasing, retreating, defending home, rescuing, or assaulting a base.
    pub squad_spread_strength: f32,
    /// Toward a captain-relative slot for non-captains.
    pub squad_follow_dir: Vec2,
    /// 0..1 — eased off for retreat / rescue / home-rally states so higher-priority behaviors still win.
    pub squad_follow_strength: f32,
}

/// Combine steering actions using [`SteeringMovementConfig`] blend weights.
pub fn blended_steering_direction(
    cfg: &SteeringMovementConfig,
    input: &SteeringBlendInput,
) -> Vec2 {
    let u = input.defend_base_strength.clamp(0.0, 1.0);
    let hr = input.home_rally_strength.clamp(0.0, 1.0);
    let chase_w = if input.has_chase && !input.should_retreat {
        let base = cfg.chase_weight_base
            * (1.0 + cfg.chase_pursuit_suppression_factor * input.nearest_enemy_sup);
        base * (1.0 - u * cfg.defend_base_chase_suppression)
            * (1.0 - hr * cfg.home_rally_chase_suppression)
    } else {
        0.0
    };
    let retreat_w = if input.should_retreat {
        cfg.retreat_blend_weight * (1.0 - cfg.defend_base_retreat_suppression * u)
    } else {
        0.0
    };
    let rescue_w = if input.should_retreat {
        0.0
    } else {
        cfg.rescue_blend_base * input.rescue_strength.clamp(0.0, 1.0)
    };

    let (flow_w, sep_w) = flow_sep_weights(cfg, input);

    let beacon_scale = if input.should_retreat {
        cfg.beacon_blend_retreat_scale
    } else {
        1.0
    };

    let mut acc = input.flow_dir * flow_w + input.separation * sep_w;
    acc += input.chase_dir * chase_w;
    acc += input.retreat_dir * retreat_w;
    acc += input.rescue_dir * rescue_w;
    if input.has_beacon {
        acc += input.beacon_force * cfg.beacon_blend * beacon_scale;
    }
    if u > 1e-6 {
        acc += input.defend_base_dir * (cfg.defend_base_blend * u);
    }
    if hr > 1e-6 {
        acc += input.home_rally_dir * (cfg.home_rally_blend * hr);
    }
    let ss = input.squad_spread_strength.clamp(0.0, 1.0);
    if ss > 1e-4 && input.squad_spread_dir.length_squared() > 1e-10 {
        acc += input.squad_spread_dir.normalize() * (cfg.squad_spread_blend * ss);
    }
    let sf = input.squad_follow_strength.clamp(0.0, 1.0);
    if sf > 1e-4 && input.squad_follow_dir.length_squared() > 1e-10 {
        acc += input.squad_follow_dir.normalize() * (cfg.squad_follow_blend * sf);
    }
    acc.normalize_or_zero()
}

fn flow_sep_weights(cfg: &SteeringMovementConfig, input: &SteeringBlendInput) -> (f32, f32) {
    if input.should_retreat {
        (cfg.flow_w_retreat, cfg.sep_w_retreat)
    } else if input.has_beacon || input.has_defend_base || input.has_home_rally {
        if input.has_chase {
            (cfg.flow_w_beacon_chase, cfg.sep_w_beacon_chase)
        } else {
            (cfg.flow_w_beacon_no_chase, cfg.sep_w_beacon_no_chase)
        }
    } else if input.has_chase {
        (cfg.flow_w_chase_no_beacon, cfg.sep_w_chase_no_beacon)
    } else {
        (cfg.flow_w_default, cfg.sep_w_default)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank_input() -> SteeringBlendInput {
        SteeringBlendInput {
            flow_dir: Vec2::ZERO,
            separation: Vec2::ZERO,
            chase_dir: Vec2::ZERO,
            retreat_dir: Vec2::ZERO,
            rescue_dir: Vec2::ZERO,
            rescue_strength: 0.0,
            beacon_force: Vec2::ZERO,
            defend_base_dir: Vec2::ZERO,
            defend_base_strength: 0.0,
            should_retreat: false,
            has_chase: false,
            has_beacon: false,
            has_defend_base: false,
            home_rally_dir: Vec2::ZERO,
            home_rally_strength: 0.0,
            has_home_rally: false,
            nearest_enemy_sup: 0.0,
            squad_spread_dir: Vec2::ZERO,
            squad_spread_strength: 0.0,
            squad_follow_dir: Vec2::ZERO,
            squad_follow_strength: 0.0,
        }
    }

    #[test]
    fn captain_follow_contributes_to_blend() {
        let cfg = SteeringMovementConfig::default();
        let mut input = blank_input();
        input.squad_follow_dir = Vec2::Y;
        input.squad_follow_strength = 1.0;

        let out = blended_steering_direction(&cfg, &input);
        assert!(out.dot(Vec2::Y) > 0.99);
    }

    #[test]
    fn retreat_still_overrides_follow_when_needed() {
        let cfg = SteeringMovementConfig::default();
        let mut input = blank_input();
        input.should_retreat = true;
        input.retreat_dir = Vec2::X;
        input.squad_follow_dir = -Vec2::X;
        input.squad_follow_strength = 1.0;

        let out = blended_steering_direction(&cfg, &input);
        assert!(out.x > 0.0);
    }
}
