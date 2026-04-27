//! Squad captain election and weighted assignment (formula-driven).
//!
//! ## Combat power
//!
//! Sorting and λ-term use **`attack * attack_speed`** (DPS proxy). This matches
//! [`GeneticTraits`] fields and stays stable for dataset / BC replay.

use bevy_math::Vec2;

/// DPS-style combat score for ordering and assignment (`ln(power + 1)` term).
#[inline]
pub fn combat_power_from_traits(attack: f32, attack_speed: f32) -> f32 {
    attack * attack_speed
}

/// Snapshot of one unit for squad algorithms (index-stable `id` is the caller’s entity index).
#[derive(Clone, Debug)]
pub struct SquadUnitSnapshot {
    pub id: usize,
    pub position: Vec2,
    pub combat_power: f32,
}

impl SquadUnitSnapshot {
    pub fn new(id: usize, position: Vec2, attack: f32, attack_speed: f32) -> Self {
        Self {
            id,
            position,
            combat_power: combat_power_from_traits(attack, attack_speed),
        }
    }
}

/// Axis-aligned bounding box diagonal length for spatial factor (formula 1).
pub fn axis_aligned_bbox_diagonal(positions: &[Vec2]) -> f32 {
    if positions.is_empty() {
        return 0.0;
    }
    let mut min_x = positions[0].x;
    let mut max_x = positions[0].x;
    let mut min_y = positions[0].y;
    let mut max_y = positions[0].y;
    for p in positions.iter().skip(1) {
        min_x = min_x.min(p.x);
        max_x = max_x.max(p.x);
        min_y = min_y.min(p.y);
        max_y = max_y.max(p.y);
    }
    let w = max_x - min_x;
    let h = max_y - min_y;
    (w * w + h * h).sqrt()
}

/// Mean Euclidean distance to the nearest *other* point (global over the slice).
///
/// For `len < 2`, returns `fallback` (e.g. mean sensor range) so formula 3 stays usable.
pub fn mean_nearest_neighbor_distance(positions: &[Vec2], fallback: f32) -> f32 {
    let n = positions.len();
    if n < 2 {
        return fallback.max(1e-3);
    }
    let mut sum = 0.0f32;
    for i in 0..n {
        let mut best = f32::MAX;
        for j in 0..n {
            if i == j {
                continue;
            }
            let d = positions[i].distance(positions[j]);
            if d < best {
                best = d;
            }
        }
        sum += best;
    }
    (sum / n as f32).max(0.0)
}

/// Formula 1 (raw): `K_raw = max(1, ceil(sqrt(N) * (diag / r_mean)))`.
///
/// **Cap:** `K = min(K_raw, ceil(CAPTAIN_SQRT_N_CAP * sqrt(N)))` (at least 1). Keeps leaders sparse;
/// without a cap, map-wide spreads make `K_raw ≈ N`. The multiplier is intentionally \< 1.5 so
/// typical battles have fewer simultaneous captains.
const CAPTAIN_SQRT_N_CAP: f32 = 0.8;

pub fn expected_squad_count(n: usize, bbox_diagonal: f32, mean_sensor_range: f32) -> u32 {
    let n = n.max(1);
    let r = mean_sensor_range.max(1e-3);
    let diag = bbox_diagonal.max(1e-3);
    let spatial_factor = diag / r;
    let k_raw = ((n as f32).sqrt() * spatial_factor).ceil().max(1.0) as u32;
    let k_cap = (CAPTAIN_SQRT_N_CAP * (n as f32).sqrt()).ceil().max(1.0) as u32;
    k_raw.min(k_cap).max(1)
}

/// Formula 3: `R = d_nn_mean * (1 + ln(N) / 10)`.
pub fn suppression_radius(d_nn_mean: f32, unit_count: usize) -> f32 {
    let n = unit_count.max(1) as f32;
    d_nn_mean.max(0.0) * (1.0 + n.ln() / 10.0)
}

/// Formula 2: greedy NMS by descending [`SquadUnitSnapshot::combat_power`], capped at **formula 1’s
/// `K`** (and at `N` units).
///
/// Stops once `max_captains` are chosen (typically `min(K, N)` from formula 1).
///
/// Returns **unit indices** into `units` (not necessarily contiguous ids).
pub fn elect_captains_nms(
    units: &[SquadUnitSnapshot],
    suppression_radius: f32,
    max_captains: usize,
) -> Vec<usize> {
    if units.is_empty() || max_captains == 0 {
        return Vec::new();
    }
    let mut order: Vec<usize> = (0..units.len()).collect();
    order.sort_by(|a, b| {
        units[*b]
            .combat_power
            .partial_cmp(&units[*a].combat_power)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut captains: Vec<usize> = Vec::new();
    for &idx in &order {
        if captains.len() >= max_captains {
            break;
        }
        let p = units[idx].position;
        let far_enough = captains
            .iter()
            .all(|&cidx| p.distance(units[cidx].position) > suppression_radius);
        if far_enough {
            captains.push(idx);
        }
    }
    captains
}

/// Formula 4: each unit joins the captain minimizing `dist - λ ln(power+1)`.
///
/// `captain_indices` are indices into `units`. Returns **squad slot** `0..captain_indices.len()`
/// for every unit row (same order as `units`).
pub fn assign_units_to_squads(
    units: &[SquadUnitSnapshot],
    captain_indices: &[usize],
    lambda: f32,
) -> Vec<usize> {
    let c = captain_indices.len();
    let mut out = vec![0usize; units.len()];
    if c == 0 {
        return out;
    }

    let cap_powers: Vec<f32> = captain_indices
        .iter()
        .map(|&i| units[i].combat_power)
        .collect();

    for (u, row) in units.iter().enumerate() {
        let mut best_squad = 0usize;
        let mut best_score = f32::MAX;
        for (s, &ci) in captain_indices.iter().enumerate() {
            let d = row.position.distance(units[ci].position);
            let score = d - lambda * (cap_powers[s] + 1.0).ln();
            if score < best_score {
                best_score = score;
                best_squad = s;
            }
        }
        out[u] = best_squad;
    }
    out
}

/// Convenience: `d_nn` → `R` → elect → assign with default λ.
pub fn elect_and_assign(
    units: &[SquadUnitSnapshot],
    mean_sensor_range: f32,
    lambda: f32,
) -> (u32, f32, Vec<usize>, Vec<usize>) {
    let positions: Vec<Vec2> = units.iter().map(|u| u.position).collect();
    let bbox = axis_aligned_bbox_diagonal(&positions);
    let k = expected_squad_count(units.len(), bbox, mean_sensor_range);
    let d_nn = mean_nearest_neighbor_distance(&positions, mean_sensor_range);
    let r = suppression_radius(d_nn, units.len());
    let max_captains = (k as usize).min(units.len());
    let captains = elect_captains_nms(units, r, max_captains);
    let squads = assign_units_to_squads(units, &captains, lambda);
    (k, r, captains, squads)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(id: usize, x: f32, y: f32, atk: f32, aps: f32) -> SquadUnitSnapshot {
        SquadUnitSnapshot::new(id, Vec2::new(x, y), atk, aps)
    }

    #[test]
    fn expected_k_matches_examples() {
        let r = 80.0f32;
        // Use **unit spread** AABB diagonal (not full map): sqrt(N) * (diag/r) ≈ K.
        // N=9, diag ≈ r → K ≈ ceil(3 * 1) = 3.
        let diag = 80.0f32;
        let k = expected_squad_count(9, diag, r);
        assert_eq!(k, 3);

        let k100 = expected_squad_count(100, diag, r);
        // k_raw = 10, capped by ceil(0.8 * 10) = 8
        assert_eq!(k100, 8);
    }

    #[test]
    fn expected_k_caps_when_swarm_spans_most_of_map() {
        let r = 80.0f32;
        let n = 50usize;
        // Huge diagonal (e.g. units from corner to corner): uncapped K_raw would be ~N.
        let diag = 2000.0f32;
        let k = expected_squad_count(n, diag, r);
        let k_cap = (CAPTAIN_SQRT_N_CAP * (n as f32).sqrt()).ceil() as u32;
        assert_eq!(k, k_cap);
        assert!(k < n as u32);
    }

    #[test]
    fn pile_elects_single_captain() {
        let units = vec![
            u(0, 0.0, 0.0, 5.0, 2.0),
            u(1, 0.0, 0.0, 20.0, 1.0),
            u(2, 0.0, 0.0, 3.0, 1.0),
        ];
        let d_nn = mean_nearest_neighbor_distance(
            &units.iter().map(|x| x.position).collect::<Vec<_>>(),
            80.0,
        );
        let r = suppression_radius(d_nn, units.len());
        let caps = elect_captains_nms(&units, r, usize::MAX);
        assert_eq!(caps.len(), 1);
        assert_eq!(caps[0], 1);
    }

    #[test]
    fn nms_respects_suppression_radius() {
        let units = vec![
            u(0, 0.0, 0.0, 100.0, 1.0),
            u(1, 50.0, 0.0, 50.0, 1.0),
            u(2, 100.0, 0.0, 40.0, 1.0),
        ];
        let caps = elect_captains_nms(&units, 40.0, usize::MAX);
        assert_eq!(caps.len(), 3);
        let caps2 = elect_captains_nms(&units, 60.0, usize::MAX);
        assert!(caps2.len() < 3);
    }

    #[test]
    fn nms_respects_max_captains_even_when_geometry_allows_more() {
        let units = vec![
            u(0, 0.0, 0.0, 10.0, 1.0),
            u(1, 150.0, 0.0, 10.0, 1.0),
            u(2, 300.0, 0.0, 10.0, 1.0),
        ];
        assert_eq!(elect_captains_nms(&units, 50.0, 2).len(), 2);
        assert_eq!(elect_captains_nms(&units, 50.0, usize::MAX).len(), 3);
    }

    #[test]
    fn assignment_prefers_stronger_captain_when_equidistant() {
        let units = vec![
            u(0, 0.0, 0.0, 10.0, 1.0),
            u(1, 100.0, 0.0, 100.0, 1.0),
            u(2, 50.0, 0.0, 5.0, 1.0),
        ];
        let caps = vec![0usize, 1];
        let sq = assign_units_to_squads(&units, &caps, 0.5);
        // Unit 2 is midway: equal distance; higher captain power lowers score (formula 4).
        assert_eq!(sq[2], 1);
    }
}
