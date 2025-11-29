use std::{
    cell::{Ref, RefCell},
    collections::{HashMap, HashSet},
};

use indexmap::IndexMap;

use crate::core::{distance_between, Position};

pub struct PathfindGrid {
    dimensions: (u32, u32),
    blocked_positions: RefCell<HashSet<Position>>,
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
            blocked_positions: Default::default(),
        }
    }

    pub fn blocked_positions(&self) -> Ref<HashSet<Position>> {
        self.blocked_positions.borrow()
    }

    pub fn set_blocked(&self, pos: Position, blocked: bool) {
        let mut positions = self.blocked_positions.borrow_mut();
        if blocked {
            assert!(
                !positions.contains(&pos),
                "{pos:?} is already blocked: {positions:?}"
            );
            positions.insert(pos);
        } else {
            assert!(positions.contains(&pos));
            positions.remove(&pos);
        }
    }

    pub fn dimensions(&self) -> (u32, u32) {
        self.dimensions
    }

    pub fn find_shortest_path_to_adjacent(
        &self,
        start: Position,
        target: Position,
        exploration_range: f32,
    ) -> Option<Path> {
        let proximity = 1.5; // adjacent = one diaginal away (sqrt(2)) is allowed, but 2 away is not
        self.find_shortest_path_to_proximity(start, target, proximity, exploration_range)
    }

    pub fn find_shortest_path_to(&self, start: Position, target: Position) -> Option<Path> {
        let proximity = 0.0; // i.e. that exact position
        self.find_shortest_path_to_proximity(start, target, proximity, 20.0)
    }

    pub fn find_shortest_path_to_proximity(
        &self,
        start: Position,
        target: Position,
        proximity: f32,
        exploration_range: f32,
    ) -> Option<Path> {
        let routes = self.explore_outward(start, exploration_range);
        let mut shortest_path: Option<Path> = None;
        for (end, chart_node) in &routes {
            if distance_between(*end, target) <= proximity {
                let path = build_path_from_chart(&routes, start, *end);

                if let Some(shortest) = &shortest_path {
                    if chart_node.distance_from_start < shortest.total_distance {
                        shortest_path = Some(path);
                    }
                } else {
                    shortest_path = Some(path);
                }
            }
        }

        shortest_path
    }

    pub fn explore_outward(&self, start: Position, range: f32) -> IndexMap<Position, ChartNode> {
        let mut routes: IndexMap<Position, ChartNode> = Default::default();

        let mut next: Vec<(Position, ChartNode)> = vec![(
            start,
            ChartNode {
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
                        ChartNode {
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

pub fn build_path_from_chart(
    chart: &IndexMap<Position, ChartNode>,
    start: Position,
    destination: Position,
) -> Path {
    let dst_node = chart.get(&destination).unwrap();
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
