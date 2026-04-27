use bevy_ecs::prelude::*;
use bevy_math::Vec2;
use swarm_model::{GameConfig, WORLD_HALF_H, WORLD_HALF_W};

/// Cell size in world-units; matches attack_range = UNIT_RADIUS * 4 = 20.
pub const CELL_SIZE: f32 = 20.0;

/// Flat grid of `GRID_W × GRID_H` cells, each holding the entities whose
/// position falls inside that cell.  Rebuilt every frame.
#[derive(Resource)]
pub struct SpatialHash {
    cells: Vec<Vec<Entity>>,
    grid_w: usize,
    grid_h: usize,
    world_half_w: f32,
    world_half_h: f32,
}

impl Default for SpatialHash {
    fn default() -> Self {
        Self::new(WORLD_HALF_W, WORLD_HALF_H)
    }
}

impl SpatialHash {
    pub fn new(world_half_w: f32, world_half_h: f32) -> Self {
        let grid_w = ((world_half_w * 2.0) / CELL_SIZE).ceil().max(1.0) as usize;
        let grid_h = ((world_half_h * 2.0) / CELL_SIZE).ceil().max(1.0) as usize;
        Self {
            cells: vec![Vec::new(); grid_w * grid_h],
            grid_w,
            grid_h,
            world_half_w,
            world_half_h,
        }
    }

    pub fn from_config(cfg: &GameConfig) -> Self {
        Self::new(cfg.world_half_w, cfg.world_half_h)
    }

    /// Remove all entities from every cell (keeps allocations).
    pub fn clear(&mut self) {
        for cell in &mut self.cells {
            cell.clear();
        }
    }

    /// Insert `entity` at the given world position.
    pub fn insert(&mut self, entity: Entity, pos: Vec2) {
        if let Some((gx, gy)) = self.cell_index(pos) {
            self.cells[gy * self.grid_w + gx].push(entity);
        }
    }

    /// Return an iterator over every entity in the 3×3 neighbourhood around
    /// the cell that contains `pos`.
    pub fn query_neighborhood(&self, pos: Vec2) -> impl Iterator<Item = Entity> + '_ {
        let (cx, cy) = self.cell_index(pos).unwrap_or((0, 0));

        let min_x = cx.saturating_sub(1);
        let max_x = (cx + 1).min(self.grid_w - 1);
        let min_y = cy.saturating_sub(1);
        let max_y = (cy + 1).min(self.grid_h - 1);

        (min_y..=max_y).flat_map(move |y| {
            (min_x..=max_x).flat_map(move |x| self.cells[y * self.grid_w + x].iter().copied())
        })
    }

    /// Visit every entity in cells overlapping the axis-aligned square that bounds
    /// the circle of `radius` around `pos`. Callers should apply a precise
    /// distance check (e.g. `dist_sq <= radius * radius`) if a circular window
    /// is required.
    pub fn for_each_in_radius(&self, pos: Vec2, radius: f32, mut f: impl FnMut(Entity)) {
        let Some((cx, cy)) = self.cell_index(pos) else {
            return;
        };
        let extent = ((radius / CELL_SIZE).ceil() as isize).max(1);
        let min_x = (cx as isize - extent).max(0) as usize;
        let max_x = (cx as isize + extent).min(self.grid_w as isize - 1) as usize;
        let min_y = (cy as isize - extent).max(0) as usize;
        let max_y = (cy as isize + extent).min(self.grid_h as isize - 1) as usize;

        for gy in min_y..=max_y {
            for gx in min_x..=max_x {
                for &entity in &self.cells[gy * self.grid_w + gx] {
                    f(entity);
                }
            }
        }
    }

    /// Map a world position to grid coordinates, or `None` if out of bounds.
    fn cell_index(&self, pos: Vec2) -> Option<(usize, usize)> {
        let fx = (pos.x + self.world_half_w) / CELL_SIZE;
        let fy = (pos.y + self.world_half_h) / CELL_SIZE;
        if fx < 0.0 || fy < 0.0 {
            return None;
        }
        let gx = (fx as usize).min(self.grid_w - 1);
        let gy = (fy as usize).min(self.grid_h - 1);
        Some((gx, gy))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::world::World;

    /// Helper: create a real Entity from a World so identifiers are valid.
    fn spawn_entity(world: &mut World) -> Entity {
        world.spawn_empty().id()
    }

    #[test]
    fn test_insert_and_query() {
        let mut world = World::new();
        let mut hash = SpatialHash::default();
        let entity = spawn_entity(&mut world);
        hash.insert(entity, Vec2::new(0.0, 0.0));
        let neighbors: Vec<_> = hash.query_neighborhood(Vec2::new(0.0, 0.0)).collect();
        assert!(neighbors.contains(&entity));
    }

    #[test]
    fn test_query_nearby_cells() {
        let mut world = World::new();
        let mut hash = SpatialHash::default();
        let e1 = spawn_entity(&mut world);
        let e2 = spawn_entity(&mut world);
        hash.insert(e1, Vec2::new(0.0, 0.0));
        hash.insert(e2, Vec2::new(15.0, 0.0)); // same or adjacent cell
        let neighbors: Vec<_> = hash.query_neighborhood(Vec2::new(0.0, 0.0)).collect();
        assert!(neighbors.contains(&e1));
        assert!(neighbors.contains(&e2));
    }

    #[test]
    fn test_far_entities_not_in_neighborhood() {
        let mut world = World::new();
        let mut hash = SpatialHash::default();
        let e1 = spawn_entity(&mut world);
        let e2 = spawn_entity(&mut world);
        hash.insert(e1, Vec2::new(-400.0, -200.0));
        hash.insert(e2, Vec2::new(400.0, 200.0));
        let neighbors: Vec<_> = hash.query_neighborhood(Vec2::new(-400.0, -200.0)).collect();
        assert!(neighbors.contains(&e1));
        assert!(!neighbors.contains(&e2));
    }

    #[test]
    fn test_clear() {
        let mut world = World::new();
        let mut hash = SpatialHash::default();
        hash.insert(spawn_entity(&mut world), Vec2::new(0.0, 0.0));
        hash.clear();
        let neighbors: Vec<_> = hash.query_neighborhood(Vec2::new(0.0, 0.0)).collect();
        assert!(neighbors.is_empty());
    }

    #[test]
    fn test_edge_positions() {
        let mut world = World::new();
        let mut hash = SpatialHash::default();
        let e1 = spawn_entity(&mut world);
        let e2 = spawn_entity(&mut world);
        let _e3 = spawn_entity(&mut world);
        // World corners
        hash.insert(e1, Vec2::new(-480.0, -270.0));
        hash.insert(e2, Vec2::new(479.0, 269.0));
        // Out of bounds position — cell_index returns None, so insert is a no-op
        hash.insert(_e3, Vec2::new(-999.0, -999.0));
        let n: Vec<_> = hash.query_neighborhood(Vec2::new(-480.0, -270.0)).collect();
        assert!(n.contains(&e1));
        // _e3 should NOT appear because it was out of bounds
        assert!(!n.contains(&_e3));
    }
}
