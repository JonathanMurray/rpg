use std::{
    cell::{Cell, Ref, RefCell},
    collections::{HashMap, HashSet},
    time::Instant,
};

use indexmap::IndexMap;

use crate::core::{
    distance_between, sq_distance_between, within_range_squared, CharacterId, Position,
    MELEE_RANGE_SQUARED,
};

pub const CELLS_PER_ENTITY: u32 = 3;

#[derive(Debug, Copy, Clone)]
pub enum Occupation {
    Character(CharacterId),
    Terrain,
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
struct CacheKey {
    from: Position,
    range: f32,
    target: Option<Target>,
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Target {
    pos: Position,
    proximity_squared: f32,
}

pub struct PathfindGrid {
    dimensions: (u32, u32),
    occupied: RefCell<HashMap<Position, Occupation>>,
    cache_key: Cell<CacheKey>,
    cached_exploration_chart: RefCell<IndexMap<Position, ChartNode>>,
    cached_unexplored: RefCell<Vec<(Position, ChartNode)>>,
}

#[derive(Copy, Clone, Debug)]
pub struct ChartNode {
    pub distance_from_start: f32,
    pub came_from: Position,
}

#[derive(Debug)]
pub struct Path {
    // total distance (walking, not flying) from start to end
    pub total_distance: f32,

    // the positions including the start all the way to the destination, each with a "total distance from start" marker
    pub positions: Vec<(f32, Position)>,
}

impl PathfindGrid {
    pub fn new(dimensions: (u32, u32)) -> Self {
        Self {
            dimensions,
            occupied: Default::default(),
            cache_key: Default::default(),
            cached_exploration_chart: Default::default(),
            cached_unexplored: Default::default(),
        }
    }

    pub fn occupied_positions(&self) -> HashSet<Position> {
        self.occupied.borrow().keys().copied().collect()
    }

    pub fn occupied(&self) -> Ref<HashMap<(i32, i32), Occupation>> {
        self.occupied.borrow()
    }

    pub fn set_occupied(&self, pos: Position, occupation: Option<Occupation>) {
        //dbg!((pos, occupation));
        let mut occupied_cells = self.occupied.borrow_mut();
        if let Some(occupation) = occupation {
            for x in pos.0 - 1..=pos.0 + 1 {
                for y in pos.1 - 1..=pos.1 + 1 {
                    let cell = (x, y);
                    assert!(
                        !occupied_cells.contains_key(&cell),
                        "Cannot occupy {:?} with {:?}. It's already occupied: {:?}, all: {:?}",
                        cell,
                        occupation,
                        occupied_cells[&cell],
                        occupied_cells
                    );
                    occupied_cells.insert(cell, occupation);
                }
            }
        } else {
            for x in pos.0 - 1..=pos.0 + 1 {
                for y in pos.1 - 1..=pos.1 + 1 {
                    let cell = (x, y);
                    assert!(occupied_cells.contains_key(&cell));
                    occupied_cells.remove(&cell);
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

    pub fn find_shortest_path_to_adjacent(
        &self,
        character_id: CharacterId,
        start: Position,
        target: Position,
        exploration_range: f32,
    ) -> Option<Path> {
        self.find_shortest_path_to_proximity(
            character_id,
            start,
            target,
            MELEE_RANGE_SQUARED,
            exploration_range,
        )
    }

    pub fn find_shortest_path_to(
        &self,
        character_id: CharacterId,
        start: Position,
        target: Position,
    ) -> Option<Path> {
        let proximity_sq = 0.0; // i.e. that exact position
        self.find_shortest_path_to_proximity(character_id, start, target, proximity_sq, 20.0)
    }

    pub fn find_shortest_path_to_proximity(
        &self,
        character_id: CharacterId,
        start: Position,
        target: Position,
        proximity_squared: f32,
        exploration_range: f32,
    ) -> Option<Path> {
        let before = Instant::now();

        //println!("find shortest path to proxy ...");
        // TODO: It's wildly inefficient to just "explore outward" in this case. We should instead
        // explore toward the target location using an A* heuristic
        // If the target is to the west, we should search west before east
        // If unable to reach the target westward, we should only explore east as long as the
        // "min distance to goal" heuristic is still below the exploration range
        //
        let chart = self.explore_outward(
            character_id,
            start,
            exploration_range,
            Some(Target {
                pos: target,
                proximity_squared,
            }),
        );
        let mut shortest_path: Option<Path> = None;
        for (pos, chart_node) in chart.iter() {
            if within_range_squared(proximity_squared, *pos, target) {
                //if distance_between(*pos, target) <= proximity {
                //println!("Build path from chart... start={:?}, pos={:?}", start, pos);
                let path = build_path_from_chart(&chart, start, *pos);

                //dbg!(&path);

                if let Some(shortest) = &shortest_path {
                    if chart_node.distance_from_start < shortest.total_distance {
                        shortest_path = Some(path);
                    }
                } else {
                    shortest_path = Some(path);
                }
            }
        }

        //println!("find shortest path to proxy ... DONE");

        shortest_path
    }

    pub fn explore_outward(
        &self,
        character_id: CharacterId,
        start: Position,
        range: f32,
        target: Option<Target>,
    ) -> Ref<IndexMap<Position, ChartNode>> {
        if self.cache_key.get().from == start
            && self.cache_key.get().range >= range
            && self.cache_key.get().target == target
        {
            /*
            println!(
                "explore_outward(char={}, start={:?}, range={} CACHED",
                character_id, start, exploration_range
            );
             */
            return self.cached_exploration_chart.borrow();
        }

        /*
        println!(
            "explore_outward(char={}, start={:?}, range={}, target={:?} ...",
            character_id, start, range, target
        );
         */

        // If we're exploring from a new starting point than previous exploration, we must clear the cached results
        if self.cache_key.get().from != start {
            *self.cached_exploration_chart.borrow_mut() = Default::default();
            self.cached_unexplored.borrow_mut().clear();
        }
        if let Some(Target {
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

        // TODO: This should be a PriorityQueue, so that heuristically good nodes are always explored before others
        let mut next = self.cached_unexplored.take();

        if next.is_empty() {
            next.push((
                start,
                ChartNode {
                    distance_from_start: 0.0,
                    came_from: start,
                },
            ));
        }

        // (start, exploration_range)
        self.cache_key.set(CacheKey {
            from: start,
            range,
            target,
        });

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
            let (pos, chart_node) = next.pop().unwrap();

            assert!(pos.0 >= 0 && pos.1 >= 0);

            if let Some(prev_chart_node) = mut_chart.get(&pos) {
                if prev_chart_node.distance_from_start <= chart_node.distance_from_start {
                    // We already know another shorter route to this node
                    continue;
                }
            }

            mut_chart.insert(pos, chart_node);

            let (x, y) = pos;

            let dist = chart_node.distance_from_start;
            let diagonal: f32 = 2f32.sqrt();
            let mut neighbors = [
                ((x - 1, y - 1), dist + diagonal),
                ((x - 1, y), dist + 1.0),
                ((x - 1, y + 1), dist + diagonal),
                ((x, y - 1), dist + 1.0),
                ((x, y + 1), dist + 1.0),
                ((x + 1, y - 1), dist + diagonal),
                ((x + 1, y), dist + 1.0),
                ((x + 1, y + 1), dist + diagonal),
            ];

            if let Some(Target {
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
                let within_grid = (0..self.dimensions.0 as i32).contains(&neighbor_pos.0)
                    && (0..self.dimensions.1 as i32).contains(&neighbor_pos.1);
                if within_grid && self.is_free_for(character_id, neighbor_pos) {
                    let node = ChartNode {
                        distance_from_start: neighbor_dist,
                        came_from: pos,
                    };

                    let mut should_explore_neighbor = true;

                    if let Some(Target {
                        pos: target_pos,
                        proximity_squared,
                    }) = target
                    {
                        // It's impossible to reach the target within the allowed exploration range, with a path passing this neighbor
                        if neighbor_dist + distance_between(neighbor_pos, target_pos)
                            - proximity_squared.sqrt()
                            > range
                        {
                            should_explore_neighbor = false;
                        }
                    } else {
                        // Visiting this neighbor would exceed the allowed exploration range
                        if neighbor_dist > range {
                            should_explore_neighbor = false;
                        }
                    }

                    if should_explore_neighbor {
                        next.push((neighbor_pos, node));
                    } else {
                        unexplored.push((neighbor_pos, node));
                    }
                }
            }

            if let Some(Target {
                pos: target_pos,
                proximity_squared,
            }) = target
            {
                if sq_distance_between(pos, target_pos) <= proximity_squared {
                    println!(
                        "Found a path to proximity ({:?}), pos={:?}. Chart size={}",
                        target,
                        pos,
                        mut_chart.len()
                    );
                    // We're done!
                    next.clear();
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

        return self.cached_exploration_chart.borrow();
    }

    pub fn is_free_for(&self, character_id: CharacterId, pos: Position) -> bool {
        // A character takes up 9 cells in a square. Check that each cell is free
        for x in pos.0 - 1..=pos.0 + 1 {
            for y in pos.1 - 1..=pos.1 + 1 {
                match self.occupied.borrow().get(&(x, y)) {
                    Some(Occupation::Character(id)) => {
                        if *id != character_id {
                            // This cell is occupied by another character
                            return false;
                        }
                    }
                    Some(Occupation::Terrain) => {
                        // This cell is occupied by terrain
                        return false;
                    }
                    None => {}
                }
            }
        }
        true
    }
}

pub fn build_path_from_chart(
    chart: &Ref<IndexMap<Position, ChartNode>>,
    start: Position,
    destination: Position,
) -> Path {
    let dst_node = chart
        .get(&destination)
        .expect(&format!("chart dest={:?}", destination));
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

    Path {
        total_distance,
        positions,
    }
}
