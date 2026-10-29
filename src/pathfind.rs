use std::{
    cell::{Cell, Ref, RefCell},
    cmp::Ordering,
    collections::{BinaryHeap, HashMap, HashSet, VecDeque},
    time::Instant,
};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{
    core::{
        distance_between, sq_distance_between, within_range_squared, CharacterId, Position,
        CENTER_MELEE_RANGE_SQUARED, MOVE_COST_FACTOR_IN_LIQUID,
    },
    grid::ControlPoint,
    textures::{TerrainId, WallOrientation},
    util::line_visitor,
};

// Entities cover 3x3 cells on the grid
pub const CELLS_PER_ENTITY: u32 = 3;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Occupation {
    Character(CharacterId),
    Terrain(TerrainId),
}

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub enum Elevation {
    Wall,
    High,
}

impl Occupation {
    fn occupied_cells(
        &self,
    ) -> Option<(std::ops::RangeInclusive<i32>, std::ops::RangeInclusive<i32>)> {
        let (xs, ys) = match self {
            Occupation::Terrain(terrain_id) => terrain_id.occupied_cells()?,
            Occupation::Character(..) => (-1..=1, -1..=1),
        };

        Some((xs, ys))
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TerrainType {
    ElevationBorder(Elevation, WallOrientation),
    Other,
}

impl TerrainType {
    pub fn can_have_staircase(&self) -> bool {
        matches!(self, TerrainType::ElevationBorder(..))
    }

    pub fn from_elevation(elevation: Elevation, orientation: WallOrientation) -> Self {
        Self::ElevationBorder(elevation, orientation)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
struct CacheKey {
    from: Position,
    range: f32,
    target: Option<PathfindTarget>,
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct PathfindTarget {
    pos: Position,
    proximity_squared: f32,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum Surface {
    Water,
    Poison,
    Spikes,
    Fire,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SurfaceBrushSize {
    ThreeByThree,
    SingleCell,
}

impl Surface {
    pub fn is_liquid(&self) -> bool {
        matches!(self, Surface::Water | Surface::Poison)
    }

    pub fn name(&self) -> &'static str {
        match self {
            Surface::Water => "water",
            Surface::Poison => "poison",
            Surface::Spikes => "spikes",
            Surface::Fire => "fire",
        }
    }
}

pub struct PathfindGrid {
    dimensions: (u32, u32),
    occupied: RefCell<HashMap<Position, Occupation>>,
    surfaces: RefCell<HashMap<Position, Surface>>,
    elevation_centers: RefCell<HashMap<Position, Elevation>>,
    elevation_borders: RefCell<HashMap<Position, (Elevation, WallOrientation)>>,
    staircases: RefCell<HashSet<Position>>,
    pub control_points: RefCell<HashMap<Position, ControlPoint>>,
    cache_key: Cell<CacheKey>,
    cached_exploration_chart: RefCell<IndexMap<Position, ChartNode>>,
    cached_unexplored: RefCell<Vec<ChartNode>>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ChartNode {
    pub distance_from_start: f32,
    pub position: Position,
    pub came_from: Position,
}

impl Ord for ChartNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .distance_from_start
            .total_cmp(&self.distance_from_start)
            .then_with(|| self.position.cmp(&other.position))
    }
}

impl Eq for ChartNode {}

impl PartialOrd for ChartNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug)]
pub struct Path {
    // total distance (walking, not flying) from start to end
    pub total_distance: f32,

    pub nodes: Vec<PathNode>,

    pub is_fallback: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PathNode {
    pub distance_from_start: f32,
    pub position: Position,
    pub surface: Option<Surface>,
}

#[derive(Debug, PartialEq)]
pub enum TraversalType {
    SlowedDownBySurface,
    NotSlowedDownBySurface,
}

impl PathfindGrid {
    pub fn new(dimensions: (u32, u32)) -> Self {
        assert!(
            dimensions.0 % 3 == 0 && dimensions.1 % 3 == 0,
            "Invalid grid dimensions: {:?}",
            dimensions
        );
        Self {
            dimensions,
            occupied: Default::default(),
            surfaces: Default::default(),
            elevation_centers: Default::default(),
            elevation_borders: Default::default(),
            staircases: Default::default(),
            control_points: Default::default(),
            cache_key: Default::default(),
            cached_exploration_chart: Default::default(),
            cached_unexplored: Default::default(),
        }
    }

    pub fn traverse_surface_cells(
        &self,
        start_pos: Position,
        surface_type: Surface,
        convert_to: Option<Surface>,
        mut visitor: impl FnMut(i32, i32),
    ) {
        if self.surfaces.borrow().get(&start_pos) != Some(&surface_type) {
            return;
        }

        let mut next = VecDeque::new();
        let mut seen = HashSet::new();
        next.push_back(start_pos);

        while let Some(pos) = next.pop_front() {
            visitor(pos.0, pos.1);
            if let Some(convert_to) = convert_to {
                self.surfaces.borrow_mut().insert(pos, convert_to);
            }
            seen.insert(pos);
            for x in [pos.0 - 1, pos.0, pos.0 + 1] {
                for y in [pos.1 - 1, pos.1, pos.1 + 1] {
                    if (x, y) != pos
                        && !seen.contains(&(x, y))
                        && self.surfaces.borrow().get(&(x, y)) == Some(&surface_type)
                    {
                        seen.insert((x, y));
                        next.push_back((x, y));
                    }
                }
            }
        }
    }

    pub fn occupied_positions(&self) -> HashSet<Position> {
        self.occupied.borrow().keys().copied().collect()
    }

    pub fn occupation(&self, pos: &Position) -> Option<Occupation> {
        self.occupied.borrow().get(&pos).copied()
    }

    pub fn elevation_border(&self, pos: Position) -> Option<(Elevation, WallOrientation)> {
        self.elevation_borders.borrow().get(&pos).copied()
    }

    pub fn auto_tile_elevation_borders(&self) {
        let mut new_map: HashMap<Position, (Elevation, WallOrientation)> = Default::default();
        for (pos, elevation) in self.elevation_centers.borrow().iter() {
            let mut neighbors = vec![];
            let neighbor_positions = &[
                // west
                (pos.0 - 3, pos.1),
                // east
                (pos.0 + 3, pos.1),
                // north
                (pos.0, pos.1 - 3),
                // south
                (pos.0, pos.1 + 3),
                // north-west
                (pos.0 - 3, pos.1 - 3),
                // north-east
                (pos.0 + 3, pos.1 - 3),
                // south-west
                (pos.0 - 3, pos.1 + 3),
                // south-east
                (pos.0 + 3, pos.1 + 3),
            ];

            #[derive(PartialEq, Debug, Copy, Clone)]
            enum NeighborType {
                // The neighbouring rect is the same type as us (HighGround or Wall)
                Same,
                // We are Wall and the neighbour is HighGround
                Lower,
                // We are HighGround and the neighbor is Wall
                Higher,
                // The neighbor is regular ground
                Ground,
            };
            use NeighborType::*;

            for neighbor_pos in neighbor_positions {
                let neighbor = self.elevation(*neighbor_pos);
                let outside_grid = !self.is_within_grid(*neighbor_pos);
                let t = if neighbor == Some(*elevation) || outside_grid {
                    Same
                } else if neighbor == Some(Elevation::High) && *elevation == Elevation::Wall {
                    Lower
                } else if neighbor == Some(Elevation::Wall) && *elevation == Elevation::High {
                    Higher
                } else {
                    Ground
                };
                neighbors.push(t);
            }
            let orientation = match (neighbors[0], neighbors[1], neighbors[2], neighbors[3]) {
                (Same, Same, Same | Higher, Same) => {
                    if neighbors[4] == Ground {
                        WallOrientation::ConcaveSouthEast
                    } else if neighbors[5] == Ground {
                        WallOrientation::ConcaveSouthWest
                    } else if neighbors[6] == Ground {
                        WallOrientation::ConcaveNorthEast
                    } else if neighbors[7] == Ground {
                        WallOrientation::ConcaveNorthWest
                    } else {
                        WallOrientation::Inner
                    }
                }
                (Ground, Same, Same | Higher, Same) => WallOrientation::East,
                (Ground, Same, Ground, Same) => WallOrientation::ConvexNorthWest,
                (Same, Same, Ground, Same) => WallOrientation::South,
                (Same, Ground, Ground, Same) => WallOrientation::ConvexNorthEast,
                (Same, Ground, Same | Higher, Same) => WallOrientation::West,
                (Same, Ground, Same | Higher, Ground) => WallOrientation::ConvexSouthEast,
                (Same, Same, Same | Higher, Ground) => WallOrientation::North,
                (Ground, Same, Same | Higher, Ground) => WallOrientation::ConvexSouthWest,

                // Wall with highground to the south
                (Same, Same, Same, Lower) => WallOrientation::NorthMezzanine,
                (Same, Ground, Same, Lower) => WallOrientation::ConvexSouthEastMezzanine,
                (Ground, Same, Same, Lower) => WallOrientation::ConvexSouthWestMezzanine,

                neighbors => {
                    println!("WARN: No wall tile fits neighbors: {:?}", neighbors);
                    WallOrientation::Single
                }
            };

            new_map.insert(*pos, (*elevation, orientation));
        }
        self.set_elevation_borders(new_map);
    }

    pub fn set_elevation_borders(&self, borders: HashMap<Position, (Elevation, WallOrientation)>) {
        for (pos, (elevation, orientation)) in self.elevation_borders.borrow().iter() {
            self.remove_occupied(
                *pos,
                Occupation::Terrain(TerrainId::from_elevation(*elevation, *orientation)),
            );
        }

        for (pos, (elevation, orientation)) in borders.iter() {
            if !self.is_staircase(*pos) {
                self.set_occupied(
                    *pos,
                    Occupation::Terrain(TerrainId::from_elevation(*elevation, *orientation)),
                );
            }
        }
        *self.elevation_borders.borrow_mut() = borders;
    }

    pub fn set_elevation_center(&self, pos: Position, elevation: Elevation) -> bool {
        self.elevation_centers
            .borrow_mut()
            .insert(pos, elevation)
            .is_none()
    }

    pub fn remove_elevation_center(&self, pos: Position) -> bool {
        self.elevation_centers.borrow_mut().remove(&pos).is_some()
    }

    pub fn elevation(&self, pos: Position) -> Option<Elevation> {
        let centers = self.elevation_centers.borrow();
        let elevation_borders = self.elevation_borders.borrow();
        for x in pos.0 - 1..=pos.0 + 1 {
            for y in pos.1 - 1..=pos.1 + 1 {
                if let Some(elevation) = centers.get(&(x, y)) {
                    if let Some((_elevation, orientation)) = elevation_borders.get(&(x, y)) {
                        if orientation.is_wall_highground_mezzanine() && y < pos.1 {
                            // Hack: when the 3x3 rect is a Wall-elevation, but mezzanine, the lowest row is Highground, not Wall
                            return Some(Elevation::High);
                        }
                    }
                    return Some(*elevation);
                }
            }
        }
        None
    }

    pub fn is_staircase(&self, pos: Position) -> bool {
        self.staircases.borrow().contains(&pos)
    }

    pub fn control_points(&self) -> Ref<'_, HashMap<Position, ControlPoint>> {
        self.control_points.borrow()
    }

    pub fn set_surface(&self, pos: Position, value: Option<Surface>, size: SurfaceBrushSize) {
        let mut surfaces = self.surfaces.borrow_mut();
        let d = match size {
            SurfaceBrushSize::ThreeByThree => 1,
            SurfaceBrushSize::SingleCell => 0,
        };
        for x in pos.0 - d..=pos.0 + d {
            for y in pos.1 - d..=pos.1 + d {
                if let Some(surface) = value {
                    if surfaces.get(&(x, y)) == Some(&surface) {
                        println!("WARN: {:?} is already marked as {:?}", pos, surface);
                    }
                    surfaces.insert((x, y), surface);
                } else {
                    if surfaces.get(&(x, y)).is_none() {
                        println!(
                            "WARN: Cannot unmark {:?} as a surface. It's not marked",
                            pos
                        );
                    }

                    surfaces.remove(&(x, y));
                }
            }
        }
    }

    pub fn surface(&self, pos: Position) -> Option<Surface> {
        self.surfaces.borrow().get(&pos).copied()
    }

    pub fn set_staircase(&self, center_pos: Position, value: bool) {
        let mut staircases = self.staircases.borrow_mut();
        for dx in -1..=1 {
            for dy in -1..=1 {
                let cell = (center_pos.0 + dx, center_pos.1 + dy);
                if value {
                    staircases.insert(cell);
                } else {
                    staircases.remove(&cell);
                }
            }
        }

        /*
        if value {
            // Characters must be able to occupy this spot - cannot be simultanouesly occupied by elevation border
            if let Some(e) = self.elevation(center_pos) {
                Occupation::Terrain(TerrainType::ElevationBorder((), ()))
            }
            if let Some(occupation) = self.occupation(&center_pos) {
                self.remove_occupied(center_pos, occupation);
            }
        }
         */
    }

    pub fn remove_occupied(&self, center_pos: Position, occupation: Occupation) {
        //println!("Remove occupied {:?}: {:?}", center_pos, occupation);

        if let Some((xs, ys)) = occupation.occupied_cells() {
            let mut occupied_cells = self.occupied.borrow_mut();

            for dx in xs {
                for dy in ys.clone() {
                    let cell = (center_pos.0 + dx, center_pos.1 + dy);
                    if !occupied_cells.contains_key(&cell) {
                        println!("Warn: Cannot free {:?}. It's already free.", cell);
                    }
                    occupied_cells.remove(&cell);
                }
            }
        }

        // The grid changed, so we can't re-use any previous exploration
        self.cache_key.set(Default::default());
        *self.cached_exploration_chart.borrow_mut() = Default::default();
    }

    pub fn set_occupied(&self, center_pos: Position, occupation: Occupation) {
        //dbg!((center_pos, occupation));

        if let Some((xs, ys)) = occupation.occupied_cells() {
            let mut occupied_cells = self.occupied.borrow_mut();
            /*
            println!(
                "Set occupied {:?}: {:?} => {:?}, {:?}",
                center_pos, occupation, xs, ys
            );
             */
            for dx in xs {
                for dy in ys.clone() {
                    let cell = (center_pos.0 + dx, center_pos.1 + dy);
                    assert!(
                        !occupied_cells.contains_key(&cell),
                        "Cannot occupy {:?} with {:?}. It's already occupied: {:?}",
                        cell,
                        occupation,
                        occupied_cells[&cell],
                        //occupied_cells
                    );
                    occupied_cells.insert(cell, occupation);
                }
            }
        }

        // The grid changed, so we can't re-use any previous exploration
        self.cache_key.set(Default::default());
        *self.cached_exploration_chart.borrow_mut() = Default::default();
    }

    pub fn dimensions(&self) -> (u32, u32) {
        self.dimensions
    }

    pub fn is_within_grid(&self, pos: Position) -> bool {
        (0..self.dimensions.0 as i32).contains(&pos.0)
            && (0..self.dimensions.1 as i32).contains(&pos.1)
    }

    pub fn find_shortest_path_to_adjacent(
        &self,
        character_id: CharacterId,
        start: Position,
        target: Position,
        exploration_range: f32,
        accept_fallback_path: bool,
        traversal_type: TraversalType,
    ) -> Option<Path> {
        self.find_shortest_path_to_proximity(
            character_id,
            start,
            target,
            CENTER_MELEE_RANGE_SQUARED,
            exploration_range,
            accept_fallback_path,
            traversal_type,
        )
    }

    /*
    pub fn find_shortest_path_to(
        &self,
        character_id: CharacterId,
        start: Position,
        target: Position,
        traversal_type: TraversalType
    ) -> Option<Path> {
        let proximity_sq = 0.0; // i.e. that exact position
        self.find_shortest_path_to_proximity(character_id, start, target, proximity_sq, 20.0, false,
        traversal_type)
    }
     */

    pub fn find_shortest_path_to_proximity(
        &self,
        character_id: CharacterId,
        start: Position,
        target: Position,
        proximity_squared: f32,
        exploration_range: f32,
        accept_fallback_path: bool,
        traversal_type: TraversalType,
    ) -> Option<Path> {
        let before = Instant::now();

        //println!("find shortest path to proxy ...");
        let chart = self.explore_outward(
            character_id,
            start,
            exploration_range,
            Some(PathfindTarget {
                pos: target,
                proximity_squared,
            }),
            traversal_type,
        );
        let mut shortest_valid_path: Option<Path> = None;
        let mut closest_fallback_path: Option<(f32, Path)> = None;

        /*
        println!("vvvvvvvvvvvvvvvvvvvv");
        dbg!(&chart.keys());
        println!("^^^^^^^^^^^^^^^^^^^^");
         */

        for (pos, chart_node) in chart.iter() {
            let sq_distance = sq_distance_between(*pos, target);
            if sq_distance <= proximity_squared && !self.obstructed_line_of_sight(*pos, target) {
                //if distance_between(*pos, target) <= proximity {
                //println!("Build path from chart... start={:?}, pos={:?}", start, pos);
                let path = self.build_path_from_chart(&chart, start, *pos);

                //dbg!(&path);

                if let Some(shortest) = &shortest_valid_path {
                    if chart_node.distance_from_start < shortest.total_distance {
                        shortest_valid_path = Some(path);
                    }
                } else {
                    shortest_valid_path = Some(path);
                }
            } else if let Some((closest_sq_dist, _)) = closest_fallback_path {
                if sq_distance < closest_sq_dist {
                    let mut path = self.build_path_from_chart(&chart, start, *pos);
                    path.is_fallback = true;
                    closest_fallback_path = Some((sq_distance, path));
                }
            } else {
                let mut path = self.build_path_from_chart(&chart, start, *pos);
                path.is_fallback = true;
                closest_fallback_path = Some((sq_distance, path));
            }
        }

        if let Some(path) = shortest_valid_path {
            //println!("Found valid path");
            Some(path)
        } else if let Some((_, path)) = closest_fallback_path {
            if accept_fallback_path {
                println!("Using fallback path");
                Some(path)
            } else {
                println!("Not accepting fallback path");
                None
            }
        } else {
            println!("Found no path (not even fallback)");
            None
        }
    }

    pub fn explore_outward(
        &self,
        character_id: CharacterId,
        start: Position,
        range: f32,
        target: Option<PathfindTarget>,
        traversal_type: TraversalType,
    ) -> Ref<'_, IndexMap<Position, ChartNode>> {
        /*
        println!(
            "explore_outward(char={}, start={:?}, range={}, target={:?} ...",
            character_id, start, range, target
        );
        println!("cache key: {:?}", self.cache_key);
        println!("cached unexplored: {:?}", self.cached_unexplored);
         */

        if self.cache_key.get().from == start
            && self.cache_key.get().range >= range
            && self.cache_key.get().target == target
        {
            /*
            println!(
                "RETURNING CACHED EXPLORATION ",
            );
             */

            return self.cached_exploration_chart.borrow();
        }

        // If we're exploring from a new starting point than previous exploration, we must clear the cached results
        if self.cache_key.get().from != start {
            //println!("Clearing cache, because cache_key.from={:?}, start={:?}", self.cache_key.get().from, start);
            *self.cached_exploration_chart.borrow_mut() = Default::default();
            self.cached_unexplored.borrow_mut().clear();
        }
        if let Some(PathfindTarget {
            pos: target_pos,
            proximity_squared,
        }) = target
        {
            for (pos, _chart_node) in self.cached_exploration_chart.borrow().iter() {
                if within_range_squared(proximity_squared, *pos, target_pos) {
                    println!(
                        "explore_outward(char={}, start={:?}, range={}, target={:?}. Found target within cache: pos={:?}",
                        character_id, start, range, target, pos
                    );

                    return self.cached_exploration_chart.borrow();
                }
            }
        }

        // Otherwise, we're free to re-use however much was explored previously, and just extend out from it
        let mut mut_chart = self.cached_exploration_chart.borrow_mut();
        //println!("Reusing cached exploration chart: {:?}", mut_chart);

        let mut next = BinaryHeap::from(self.cached_unexplored.take());

        if next.is_empty() {
            next.push(ChartNode {
                distance_from_start: 0.0,
                position: start,
                came_from: start,
            });
        }

        // (start, exploration_range)
        self.cache_key.set(CacheKey {
            from: start,
            range,
            target,
        });

        //println!("Setting cache key = {:?}", self.cache_key);

        let mut unexplored = vec![];

        /*
        println!(
            "explore_outward(char={}, start={:?}, range={}, #next={}",
            character_id,
            start,
            exploration_range,
            next.len()
        );
         */

        while !next.is_empty() {
            let chart_node = next.pop().unwrap();
            //println!("{} nodes left on the stack", next.len());
            //println!("  visiting: {:?}", chart_node.position);

            assert!(chart_node.position.0 >= 0 && chart_node.position.1 >= 0);

            if let Some(prev_chart_node) = mut_chart.get(&chart_node.position) {
                if prev_chart_node.distance_from_start <= chart_node.distance_from_start {
                    // We already know another shorter route to this node
                    //println!("we already know another shorter route to this node");
                    continue;
                }
            }

            mut_chart.insert(chart_node.position, chart_node);

            let (x, y) = chart_node.position;

            let dist = chart_node.distance_from_start;
            let diagonal: f32 = 2f32.sqrt();

            let mut neighbors = vec![];
            for x0 in x - 1..=x + 1 {
                for y0 in y - 1..=y + 1 {
                    if (x0, y0) != (x, y) {
                        let mut local_cost = if (x0 - x).abs() == 1 && (y0 - y).abs() == 1 {
                            diagonal
                        } else {
                            1.0
                        };
                        if traversal_type == TraversalType::SlowedDownBySurface
                            && self.surfaces.borrow().contains_key(&(x0, y0))
                        {
                            local_cost *= MOVE_COST_FACTOR_IN_LIQUID;
                        }
                        neighbors.push(((x0, y0), dist + local_cost));
                    }
                }
            }

            if let Some(PathfindTarget {
                pos: target_pos, ..
            }) = target
            {
                // The neighbors with lower heuristic (i.e. are likely to yield a good path to the target)
                // should be visited before other neighbors
                neighbors.sort_unstable_by(|(a, _), (b, _)| {
                    let a_heuristic = sq_distance_between(*a, target_pos);
                    let b_heuristic = sq_distance_between(*b, target_pos);
                    a_heuristic.total_cmp(&b_heuristic)
                });
            }

            // The neighbors are (potentially) sorted, with the best one first; iterate in reverse
            // order since we push them onto a stack, and therefore want to push the best one last.
            for (neighbor_pos, neighbor_dist) in neighbors.into_iter().rev() {
                if self.is_within_grid(neighbor_pos)
                    && self.is_free(Occupation::Character(character_id), neighbor_pos)
                {
                    let neighbor_node = ChartNode {
                        distance_from_start: neighbor_dist,
                        position: neighbor_pos,
                        came_from: chart_node.position,
                    };

                    let mut should_explore_neighbor = true;

                    if let Some(PathfindTarget {
                        pos: target_pos,
                        proximity_squared,
                    }) = target
                    {
                        // It's impossible to reach the target within the allowed exploration range, with a path passing this neighbor
                        if neighbor_dist + distance_between(neighbor_pos, target_pos)
                            - proximity_squared.sqrt()
                            > range
                        {
                            /*
                            println!(
                                "Shouldn't explore neighbor {:?} (from {:?})",
                                neighbor_pos, chart_node.position
                            );
                             */
                            should_explore_neighbor = false;
                        } else {
                            /*
                            println!(
                                "Should explore neighbor {:?} (from {:?})",
                                neighbor_pos, chart_node.position
                            );
                             */
                        }
                    } else {
                        // Visiting this neighbor would exceed the allowed exploration range
                        if neighbor_dist > range {
                            should_explore_neighbor = false;
                        }
                    }

                    if should_explore_neighbor {
                        next.push(neighbor_node);
                    } else {
                        unexplored.push(neighbor_node);
                    }
                }
            }

            if let Some(PathfindTarget {
                pos: target_pos,
                proximity_squared,
            }) = target
            {
                if sq_distance_between(chart_node.position, target_pos) <= proximity_squared {
                    println!(
                        "Checking if unobstructed line between {:?} and {:?} ...",
                        chart_node.position, target_pos
                    );
                    let obstructed_line_of_sight =
                        self.obstructed_line_of_sight(chart_node.position, target_pos);
                    if !obstructed_line_of_sight {
                        println!(
                            "Found a path to proximity ({:?}), pos={:?}. Chart size={}",
                            target,
                            chart_node.position,
                            mut_chart.len()
                        );
                        //dbg!(&next);
                        // We're done! Empty 'next' to exit the loop, but all the nodes in 'next' that we didn't visit must
                        // be saved as 'unexplored'.
                        // If another exploration reuses our chart, it will need to visit those to continue the
                        // exploration correctly.
                        for unvisited in next.drain() {
                            unexplored.push(unvisited);
                        }
                    } else {
                        println!("Found proximity ({:?}), but does not have unobstructed line-of-sight to target", chart_node.position);
                    }
                }
            }
        }

        /*
        println!(
            "explore_outward(char={}, start={:?}, range={} DONE.",
            character_id, start, range
        );
         */

        // Release the mutable ref, so that we can take a ref to return
        drop(mut_chart);

        *self.cached_unexplored.borrow_mut() = unexplored;

        self.cached_exploration_chart.borrow()
    }

    pub fn obstructed_line_of_sight(&self, from: Position, to: Position) -> bool {
        line_visitor(from, to, |x, y| {
            matches!(
                self.occupied.borrow().get(&(x, y)),
                Some(Occupation::Terrain(TerrainId::StoneWall(..)))
            )
        })
    }

    pub fn is_free(&self, subject: Occupation, pos: Position) -> bool {
        self.check_collision(Some(subject), pos).is_none()
    }

    pub fn check_collision(&self, subject: Option<Occupation>, pos: Position) -> Option<Collision> {
        let mut other_chars: HashSet<CharacterId> = Default::default();
        let mut terrain_collision = false;
        let mut elevation_collision = false;
        let mut prev_elevation = None;

        let (xs, ys) = if let Some(occupation) = subject {
            occupation.occupied_cells().unwrap()
        } else {
            // Just a single cell
            (0..=0, 0..=0)
        };

        for x in xs.map(|dx| pos.0 + dx) {
            for y in ys.clone().map(|dy| pos.1 + dy) {
                if !self.is_within_grid((x, y)) {
                    // This cell is outside of the grid
                    terrain_collision = true;
                }

                let is_staircase = self.is_staircase((x, y));

                match self.occupied.borrow().get(&(x, y)) {
                    Some(Occupation::Character(id)) => {
                        if subject != Some(Occupation::Character(*id)) {
                            // This cell is occupied by another character
                            other_chars.insert(*id);
                        }
                    }
                    Some(Occupation::Terrain(terrain_id)) => {
                        if !is_staircase {
                            if terrain_id.elevation().is_some() {
                                elevation_collision = true;
                            } else {
                                terrain_collision = true;
                            }
                        }
                    }
                    None => {}
                }

                // Check if there's any difference in elevation among the subject cells. Can happen even if there's
                // no occupied cells, since north-facing ledges take up no cells
                let elevation = self.elevation((x, y));
                if !is_staircase {
                    if let Some(prev) = prev_elevation {
                        if elevation != prev {
                            return Some(Collision::Elevation);
                        }
                    } else {
                        prev_elevation = Some(elevation);
                    }
                }
            }
        }

        //dbg!(terrain_collision, elevation_collision);

        if !other_chars.is_empty() {
            Some(Collision::Characters(other_chars))
        } else if terrain_collision {
            Some(Collision::Terrain)
        } else if elevation_collision {
            Some(Collision::Elevation)
        } else {
            None
        }
    }

    pub fn build_path_from_chart(
        &self,
        chart: &Ref<IndexMap<Position, ChartNode>>,
        start: Position,
        destination: Position,
    ) -> Path {
        let dst_node = chart
            .get(&destination)
            .unwrap_or_else(|| panic!("chart dest={:?}", destination));
        let mut dist = dst_node.distance_from_start;

        let mut positions = vec![(dist, destination)];

        // If we seeked to a location that's adjacent to the start position, the path will just consist of the start position
        if dst_node.came_from != destination {
            let mut pos = dst_node.came_from;

            loop {
                let node = chart.get(&pos).unwrap();
                dist = node.distance_from_start;
                positions.insert(0, (dist, pos));
                //path.push((dist, pos));
                if pos == start {
                    break;
                }
                assert!(
                    node.came_from != pos,
                    "Node came from itself: {pos:?} -> {node:?}"
                );
                pos = node.came_from;
            }
            assert!(positions.len() > 1);
            assert!(positions[0] != positions[1]);
        }

        let total_distance = positions.last().unwrap().0;
        //let positions = path.iter().rev.copied().collect();

        let positions = positions
            .into_iter()
            .map(|(dist, pos)| {
                let surface = self.surfaces.borrow().get(&pos).copied();
                PathNode {
                    distance_from_start: dist,
                    position: pos,
                    surface,
                }
            })
            .collect();

        Path {
            total_distance,
            nodes: positions,
            is_fallback: false,
        }
    }
}

#[derive(Debug)]
pub enum Collision {
    Terrain,
    Characters(HashSet<CharacterId>),
    Elevation,
}
