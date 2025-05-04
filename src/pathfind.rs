use std::{
    cell::{Ref, RefCell},
    collections::{HashMap, HashSet},
};

use crate::core::Position;

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
            assert!(!positions.contains(&pos));
            positions.insert(pos);
        } else {
            assert!(positions.contains(&pos));
            positions.remove(&pos);
        }
    }

    pub fn dimensions(&self) -> (u32, u32) {
        self.dimensions
    }

    pub fn run(&self, (start_x, start_y): Position, range: f32) -> HashMap<Position, Route> {
        let mut routes: HashMap<Position, Route> = Default::default();

        let mut next: Vec<(Position, Route)> = vec![(
            (start_x, start_y),
            Route {
                distance_from_start: 0.0,
                came_from: (start_x, start_y),
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
