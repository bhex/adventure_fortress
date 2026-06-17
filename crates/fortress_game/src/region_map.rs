//! Procedural terrain + roads for the regional overworld map. Pure presentation
//! derived deterministically from `run_seed` (the `world.rs` philosophy — cheap
//! to recompute, never serialized). Terrain is value-noise biomes rendered at
//! double vertical resolution via half-block glyphs; roads are least-cost paths
//! that lead from every hold to the fortress, so they bend logically around
//! lakes and peaks. Rebuilt only when the set of standing sites changes.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};

use bevy::prelude::*;

use fortress_core::{Coord, GameState, FORTRESS_POS, REGION_H, REGION_W};

/// Terrain at sub-pixel (half-block) resolution: full width, double height.
pub const SUB_W: i32 = REGION_W as i32;
pub const SUB_H: i32 = REGION_H as i32 * 2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Biome {
    Water,
    Plains,
    Forest,
    Hills,
    Mountains,
}

impl Biome {
    /// Base map colour. The render dithers brightness per tile for texture.
    pub fn color(self) -> Color {
        match self {
            Biome::Water => Color::srgb(0.16, 0.30, 0.52),
            Biome::Plains => Color::srgb(0.52, 0.60, 0.31),
            Biome::Forest => Color::srgb(0.17, 0.39, 0.21),
            Biome::Hills => Color::srgb(0.46, 0.40, 0.26),
            Biome::Mountains => Color::srgb(0.52, 0.50, 0.52),
        }
    }

    /// Travel cost for road-finding — water and peaks are dear, plains cheap.
    fn cost(self) -> i32 {
        match self {
            Biome::Plains => 1,
            Biome::Forest => 3,
            Biome::Hills => 6,
            Biome::Mountains => 14,
            Biome::Water => 22,
        }
    }
}

#[derive(Resource, Default)]
pub struct RegionMap {
    /// Sub-pixel biome grid, `SUB_W * SUB_H`, row-major.
    pub terrain: Vec<Biome>,
    /// Road tiles in logical region coordinates.
    pub roads: HashSet<(i16, i16)>,
    /// Key over the standing sites; the map rebuilds when it changes.
    revision: u64,
}

impl RegionMap {
    pub fn biome_at(&self, x: i32, sy: i32) -> Biome {
        self.terrain
            .get((sy * SUB_W + x) as usize)
            .copied()
            .unwrap_or(Biome::Plains)
    }

    pub fn is_road(&self, x: i16, y: i16) -> bool {
        self.roads.contains(&(x, y))
    }

    /// Rebuild from the game state if the standing sites have changed. Cheap
    /// no-op otherwise.
    pub fn ensure(&mut self, gs: &GameState) {
        let rev = revision(gs);
        if rev == self.revision && !self.terrain.is_empty() {
            return;
        }
        self.revision = rev;
        self.terrain = generate_terrain(gs.run_seed);
        carve_land_under_settlements(&mut self.terrain, gs);
        self.roads = build_roads(&self.terrain, gs);
    }
}

/// A change key over the holds (count, names, positions) so the roads follow the
/// living region. The terrain itself only depends on `run_seed`.
fn revision(gs: &GameState) -> u64 {
    let mut h: u64 = gs.run_seed ^ 0x1234_5678;
    for s in &gs.region.sites {
        for b in s.name.as_bytes() {
            h = h.wrapping_mul(31).wrapping_add(*b as u64);
        }
        h = h.wrapping_mul(131).wrapping_add(s.pos.x as u64);
        h = h.wrapping_mul(131).wrapping_add(s.pos.y as u64);
    }
    h
}

// ---------------------------------------------------------------------------
// value noise
// ---------------------------------------------------------------------------

fn hashf(seed: u64, x: i32, y: i32) -> f32 {
    let mut h = seed
        ^ (x as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (y as i64 as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 33;
    h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h ^= h >> 33;
    (h & 0xFF_FFFF) as f32 / 0xFF_FFFF as f32
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn value_noise(seed: u64, x: f32, y: f32) -> f32 {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let (sx, sy) = (smooth(x - x0 as f32), smooth(y - y0 as f32));
    let v00 = hashf(seed, x0, y0);
    let v10 = hashf(seed, x0 + 1, y0);
    let v01 = hashf(seed, x0, y0 + 1);
    let v11 = hashf(seed, x0 + 1, y0 + 1);
    let a = v00 + (v10 - v00) * sx;
    let b = v01 + (v11 - v01) * sx;
    a + (b - a) * sy
}

/// Fractal (multi-octave) value noise, normalised to 0..1.
fn fbm(seed: u64, x: f32, y: f32) -> f32 {
    let mut f = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    let mut norm = 0.0;
    for o in 0..4u64 {
        f += amp * value_noise(seed.wrapping_add(o.wrapping_mul(1009)), x * freq, y * freq);
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    f / norm
}

fn generate_terrain(seed: u64) -> Vec<Biome> {
    // Sub-pixels are half as tall as wide, so squash y to keep features round.
    let elev_scale = 11.0;
    let moist_scale = 8.0;
    let mut terrain = Vec::with_capacity((SUB_W * SUB_H) as usize);
    for sy in 0..SUB_H {
        for x in 0..SUB_W {
            let fx = x as f32 / elev_scale;
            let fy = (sy as f32 / 2.0) / elev_scale;
            let elevation = fbm(seed, fx, fy);
            let moisture = fbm(
                seed ^ 0xA5A5_5A5A,
                x as f32 / moist_scale,
                (sy as f32 / 2.0) / moist_scale,
            );
            let biome = if elevation < 0.30 {
                Biome::Water
            } else if elevation > 0.78 {
                Biome::Mountains
            } else if elevation > 0.62 {
                Biome::Hills
            } else if moisture > 0.55 {
                Biome::Forest
            } else {
                Biome::Plains
            };
            terrain.push(biome);
        }
    }
    terrain
}

/// Sites and the fortress sit on dry, buildable ground — stamp a little plains
/// patch under each so they never float in a lake and roads start on land.
fn carve_land_under_settlements(terrain: &mut [Biome], gs: &GameState) {
    let mut stamp = |c: Coord| {
        for dy in -2..=2i32 {
            for dx in -1..=1i32 {
                let x = c.x as i32 + dx;
                let sy = c.y as i32 * 2 + dy;
                if (0..SUB_W).contains(&x) && (0..SUB_H).contains(&sy) {
                    terrain[(sy * SUB_W + x) as usize] = Biome::Plains;
                }
            }
        }
    };
    stamp(FORTRESS_POS);
    for s in &gs.region.sites {
        stamp(s.pos);
    }
}

// ---------------------------------------------------------------------------
// roads (least-cost paths)
// ---------------------------------------------------------------------------

/// Logical-cell travel cost, sampled from the terrain at the cell's middle.
fn cell_cost(terrain: &[Biome], x: i16, y: i16) -> i32 {
    let sy = y as i32 * 2;
    let i = (sy * SUB_W + x as i32) as usize;
    terrain.get(i).copied().unwrap_or(Biome::Plains).cost()
}

/// Roads: hub-and-spoke from the fortress to every hold, plus a link from each
/// hold to its nearest neighbour — a network that bends around the terrain.
fn build_roads(terrain: &[Biome], gs: &GameState) -> HashSet<(i16, i16)> {
    let mut roads = HashSet::new();
    let sites: Vec<Coord> = gs.region.sites.iter().map(|s| s.pos).collect();

    for &site in &sites {
        for tile in astar(terrain, FORTRESS_POS, site) {
            roads.insert(tile);
        }
        // link to the nearest other hold for a connected web
        if let Some(&near) = sites
            .iter()
            .filter(|&&o| o != site)
            .min_by_key(|&&o| site.dist(o))
        {
            for tile in astar(terrain, site, near) {
                roads.insert(tile);
            }
        }
    }
    roads
}

fn astar(terrain: &[Biome], start: Coord, goal: Coord) -> Vec<(i16, i16)> {
    let w = REGION_W as i32;
    let h = REGION_H as i32;
    let idx = |x: i16, y: i16| (y as i32 * w + x as i32) as usize;
    let n = (w * h) as usize;

    let mut g = vec![i32::MAX; n];
    let mut came: Vec<Option<(i16, i16)>> = vec![None; n];
    let mut heap: BinaryHeap<Reverse<(i32, i16, i16)>> = BinaryHeap::new();

    let heur = |x: i16, y: i16| Coord::new(x, y).dist(goal);
    g[idx(start.x, start.y)] = 0;
    heap.push(Reverse((heur(start.x, start.y), start.x, start.y)));

    while let Some(Reverse((_, cx, cy))) = heap.pop() {
        if (cx, cy) == (goal.x, goal.y) {
            break;
        }
        let here = g[idx(cx, cy)];
        for (dx, dy) in [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (1, -1),
            (-1, 1),
            (-1, -1),
        ] {
            let nx = cx + dx;
            let ny = cy + dy;
            if nx < 0 || ny < 0 || nx as i32 >= w || ny as i32 >= h {
                continue;
            }
            // diagonals a touch dearer so straight roads are preferred
            let step = cell_cost(terrain, nx, ny) * if dx != 0 && dy != 0 { 3 } else { 2 };
            let tentative = here.saturating_add(step);
            let ni = idx(nx, ny);
            if tentative < g[ni] {
                g[ni] = tentative;
                came[ni] = Some((cx, cy));
                heap.push(Reverse((tentative.saturating_add(heur(nx, ny)), nx, ny)));
            }
        }
    }

    // walk the path back from the goal
    let mut path = Vec::new();
    let mut cur = (goal.x, goal.y);
    if came[idx(cur.0, cur.1)].is_none() && cur != (start.x, start.y) {
        return path; // unreachable (shouldn't happen on a connected grid)
    }
    loop {
        path.push(cur);
        if cur == (start.x, start.y) {
            break;
        }
        match came[idx(cur.0, cur.1)] {
            Some(prev) => cur = prev,
            None => break,
        }
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_map_builds_and_connects_every_hold() {
        for seed in [1u64, 2, 7, 42, 100, 12345] {
            let gs = GameState::new(seed);
            let mut map = RegionMap::default();
            map.ensure(&gs);

            assert_eq!(map.terrain.len(), (SUB_W * SUB_H) as usize, "terrain fully filled");
            assert!(!map.roads.is_empty(), "roads exist when holds stand");
            // all roads radiate from the fortress, so its tile is on the network
            assert!(
                map.roads.contains(&(FORTRESS_POS.x, FORTRESS_POS.y)),
                "the fortress sits on the road network (seed {seed})"
            );
            // every standing hold is reached by a road
            for s in &gs.region.sites {
                assert!(
                    map.roads.contains(&(s.pos.x, s.pos.y)),
                    "{} is joined to the roads (seed {seed})",
                    s.name
                );
            }
        }
    }
}
