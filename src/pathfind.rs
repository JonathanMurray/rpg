use std::{
    cell::{Ref, RefCell},
    collections::{HashMap, HashSet},
};

use crate::{core::Position, util::are_adjacent};

pub struct PathfindGrid {
    dimensions: (u32, u32),
    blocked_positions: RefCell<HashSet<Position>>,
}

#[derive(Copy, Clone, Debug)]
pub struct Route {
    pub distance_from_start: f32,
    pub came_from: Position,
}

impl PathfindGrid {
    pub fn new(dimensions: (u32, u32)) -> Self {
        Self {
            dimensions,
            blocked_positions: Default::default(),
        }
    }

    pub fn blocked_positions(&self) -> Ref<HashSet<Position>> {
        self.blocked_positions.borrow()
    }

    pub fn set_blocked(&self, pos: Position, blocked: bool) {
        let mut positions = self.blocked_positions.borrow_mut();
        if blocked {
            assert!(!positions.contains(&pos), "{positions:?}, {pos:?}");
            positions.insert(pos);
        } else {
            assert!(positions.contains(&pos));
            positions.remove(&pos);
        }
    }

    pub fn dimensions(&self) -> (u32, u32) {
        self.dimensions
    }

    pub fn find_path_to_adjacent(
        &self,
        start: Position,
        target: Position,
    ) -> Option<(f32, Vec<(f32, Position)>)> {
        let routes = self.explore_outward(start, 20.0);
        let mut shortest_path: Option<Vec<(f32, (i32, i32))>> = None;
        for (end, route) in &routes {
            if are_adjacent(*end, target) {
                let mut path = build_path_from_route(&routes, start, *end);

                //dbg!("before popping start pos from path: ", &path);

                if path[0].1 == start {
                    // The "path" doesn't make it beyond the start position. Discard it.
                    continue;
                }

                // Pop the start position
                path.pop();

                if let Some(shortest) = &shortest_path {
                    if route.distance_from_start < shortest[0].0 {
                        shortest_path = Some(path);
                    }
                } else {
                    shortest_path = Some(path);
                }
            }
        }

        shortest_path.map(|path| {
            let total_dist = path[0].0;
            let positions = path.iter().rev().copied().collect();
            (total_dist, positions)
        })
    }

    pub fn explore_outward(&self, start: Position, range: f32) -> HashMap<Position, Route> {
        let mut routes: HashMap<Position, Route> = Default::default();

        let mut next: Vec<(Position, Route)> = vec![(
            start,
            Route {
                distance_from_start: 0.0,
                came_from: start,
            },
        )];

        while !next.is_empty() {
            let (node, route) = next.remove(0);

            if let Some(prev_route) = routes.get(&node) {
                if prev_route.distance_from_start <= route.distance_from_start {
                    // We already know another shorter route to this node
                    continue;
                }
            }

            assert!(node.0 >= 0 && node.1 >= 0);
            routes.insert(node, route);
            let (x, y) = node;

            let dist = route.distance_from_start;
            let diagonal: f32 = 2f32.sqrt();
            let neighbors = [
                ((x - 1, y - 1), dist + diagonal),
                ((x - 1, y), dist + 1.0),
                ((x - 1, y + 1), dist + diagonal),
                ((x, y - 1), dist + 1.0),
                ((x, y + 1), dist + 1.0),
                ((x + 1, y - 1), dist + diagonal),
                ((x + 1, y), dist + 1.0),
                ((x + 1, y + 1), dist + diagonal),
            ];

            for (neighbor_node, neighbor_dist) in neighbors {
                let within_grid = (0..self.dimensions.0 as i32).contains(&neighbor_node.0)
                    && (0..self.dimensions.1 as i32).contains(&neighbor_node.1);
                if neighbor_dist <= range
                    && within_grid
                    && !self.blocked_positions.borrow().contains(&neighbor_node)
                {
                    next.push((
                        neighbor_node,
                        Route {
                            distance_from_start: neighbor_dist,
                            came_from: node,
                        },
                    ));
                }
            }
        }

        routes
    }
}

pub fn build_path_from_route(
    routes: &HashMap<Position, Route>,
    start: Position,
    destination: Position,
) -> Vec<(f32, Position)> {
    let route = routes.get(&destination).unwrap();
    let mut dist = route.distance_from_start;

    let mut path = vec![(dist, destination)];
    let mut pos = route.came_from;

    loop {
        let route = routes.get(&pos).unwrap();
        dist = route.distance_from_start;
        path.push((dist, pos));
        if pos == start {
            break;
        }
        pos = route.came_from;
    }
    assert!(path.len() > 1);
    path
}
