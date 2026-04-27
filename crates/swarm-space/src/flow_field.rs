use bevy_ecs::prelude::*;
use bevy_math::Vec2;
use std::collections::VecDeque;
use swarm_model::{GameConfig, WORLD_HALF_H, WORLD_HALF_W};

/// Size of each grid cell in world-units.
pub const FLOW_CELL_SIZE: f32 = 20.0;

/// Pair of flow fields — one per attack direction.
#[derive(Resource)]
pub struct FlowFieldSet {
    /// Directs **Player** units toward the **Enemy** base(s).
    pub attack_enemy: FlowField,
    /// Directs **Enemy** units toward the **Player** base(s).
    pub attack_player: FlowField,
}

impl Default for FlowFieldSet {
    fn default() -> Self {
        Self::new(WORLD_HALF_W, WORLD_HALF_H)
    }
}

impl FlowFieldSet {
    pub fn new(world_half_w: f32, world_half_h: f32) -> Self {
        Self {
            attack_enemy: FlowField::empty(world_half_w, world_half_h),
            attack_player: FlowField::empty(world_half_w, world_half_h),
        }
    }

    pub fn from_config(cfg: &GameConfig) -> Self {
        Self::new(cfg.world_half_w, cfg.world_half_h)
    }
}

/// A BFS-based flow field over a fixed 2-D grid.
///
/// Each cell stores a unit-length direction vector pointing toward the nearest
/// goal cell (or `Vec2::ZERO` if the cell is unreachable / is itself a goal).
pub struct FlowField {
    directions: Vec<Vec2>,
    grid_w: usize,
    grid_h: usize,
    half_w: f32,
    half_h: f32,
}

impl FlowField {
    /// Create a field filled with `Vec2::ZERO`.
    pub fn empty(world_half_w: f32, world_half_h: f32) -> Self {
        let grid_w = ((world_half_w * 2.0) / FLOW_CELL_SIZE).ceil().max(1.0) as usize;
        let grid_h = ((world_half_h * 2.0) / FLOW_CELL_SIZE).ceil().max(1.0) as usize;
        Self {
            directions: vec![Vec2::ZERO; grid_w * grid_h],
            grid_w,
            grid_h,
            half_w: world_half_w,
            half_h: world_half_h,
        }
    }

    /// (Re-)generate the field via BFS from the given goal world-positions (uniform weight `1.0`).
    pub fn generate(&mut self, goals: &[Vec2]) {
        let weighted: Vec<(Vec2, f32)> = goals.iter().map(|p| (*p, 1.0)).collect();
        self.generate_weighted(&weighted);
    }

    /// Multi-goal flow field: each cell points down the **weighted** cost gradient
    /// `min_g (grid_steps(cell, g) / attractiveness_g)`.
    ///
    /// Higher `attractiveness` (e.g. low-HP bases = easier / higher-value captures) makes that goal
    /// effectively “closer”, so more of the map steers toward it.
    pub fn generate_weighted(&mut self, goals: &[(Vec2, f32)]) {
        let total = self.grid_w * self.grid_h;

        for d in self.directions.iter_mut() {
            *d = Vec2::ZERO;
        }

        if goals.is_empty() {
            return;
        }

        const NEIGHBORS: [(i32, i32); 8] = [
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ];

        let mut combined: Vec<f32> = vec![f32::INFINITY; total];

        for &(world_pos, attract) in goals {
            if attract <= 1e-6 {
                continue;
            }
            let (gx, gy) = self.world_to_grid(world_pos);
            let start = gy * self.grid_w + gx;

            let mut dist: Vec<u32> = vec![u32::MAX; total];
            let mut queue: VecDeque<usize> = VecDeque::new();
            dist[start] = 0;
            queue.push_back(start);

            while let Some(cur) = queue.pop_front() {
                let dcur = dist[cur];
                let cx = (cur % self.grid_w) as i32;
                let cy = (cur / self.grid_w) as i32;

                for &(dx, dy) in &NEIGHBORS {
                    let nx = cx + dx;
                    let ny = cy + dy;
                    if nx < 0 || ny < 0 || nx >= self.grid_w as i32 || ny >= self.grid_h as i32 {
                        continue;
                    }
                    let nidx = ny as usize * self.grid_w + nx as usize;
                    if dist[nidx] != u32::MAX {
                        continue;
                    }
                    dist[nidx] = dcur + 1;
                    queue.push_back(nidx);
                }
            }

            for i in 0..total {
                if dist[i] != u32::MAX {
                    let eff = dist[i] as f32 / attract;
                    combined[i] = combined[i].min(eff);
                }
            }
        }

        for i in 0..total {
            if !combined[i].is_finite() {
                continue;
            }
            let my = combined[i];
            let cx = i % self.grid_w;
            let cy = i / self.grid_w;
            let mut best_j: Option<usize> = None;
            let mut best_cost = my;

            for &(dx, dy) in &NEIGHBORS {
                let nx = cx as i32 + dx;
                let ny = cy as i32 + dy;
                if nx < 0 || ny < 0 || nx >= self.grid_w as i32 || ny >= self.grid_h as i32 {
                    continue;
                }
                let nidx = ny as usize * self.grid_w + nx as usize;
                let nc = combined[nidx];
                if nc.is_finite() && nc < best_cost {
                    best_cost = nc;
                    best_j = Some(nidx);
                }
            }

            if let Some(j) = best_j {
                let jx = j % self.grid_w;
                let jy = j / self.grid_w;
                let my_c = self.cell_center(cx, cy);
                let n_c = self.cell_center(jx, jy);
                self.directions[i] = (n_c - my_c).normalize_or_zero();
            }
        }
    }

    /// Sample the flow direction at the given world position.
    ///
    /// Returns a unit-length vector (or `Vec2::ZERO` at goals / unreachable cells).
    pub fn sample(&self, pos: Vec2) -> Vec2 {
        let (gx, gy) = self.world_to_grid(pos);
        self.directions[gy * self.grid_w + gx]
    }

    #[inline]
    fn world_to_grid(&self, pos: Vec2) -> (usize, usize) {
        let gx = ((pos.x + self.half_w) / FLOW_CELL_SIZE) as i32;
        let gy = ((pos.y + self.half_h) / FLOW_CELL_SIZE) as i32;
        let gx = gx.clamp(0, self.grid_w as i32 - 1) as usize;
        let gy = gy.clamp(0, self.grid_h as i32 - 1) as usize;
        (gx, gy)
    }

    #[inline]
    fn cell_center(&self, gx: usize, gy: usize) -> Vec2 {
        Vec2::new(
            gx as f32 * FLOW_CELL_SIZE + FLOW_CELL_SIZE * 0.5 - self.half_w,
            gy as f32 * FLOW_CELL_SIZE + FLOW_CELL_SIZE * 0.5 - self.half_h,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_single_goal() {
        let mut field = FlowField::empty(WORLD_HALF_W, WORLD_HALF_H);
        field.generate(&[Vec2::new(0.0, 0.0)]);
        // A cell far from center should point toward center
        let dir = field.sample(Vec2::new(-200.0, 0.0));
        assert!(
            dir.x > 0.0,
            "Should point right toward center, got {:?}",
            dir
        );
    }

    #[test]
    fn test_goal_cell_is_zero() {
        let mut field = FlowField::empty(WORLD_HALF_W, WORLD_HALF_H);
        field.generate(&[Vec2::new(0.0, 0.0)]);
        let dir = field.sample(Vec2::new(0.0, 0.0));
        assert_eq!(dir, Vec2::ZERO);
    }

    #[test]
    fn test_empty_goals() {
        let mut field = FlowField::empty(WORLD_HALF_W, WORLD_HALF_H);
        field.generate(&[]);
        // All cells should be ZERO (no goals)
        let dir = field.sample(Vec2::new(100.0, 100.0));
        assert_eq!(dir, Vec2::ZERO);
    }
}
