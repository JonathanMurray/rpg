use std::collections::{HashMap, HashSet};

pub struct PathfindGrid {
    pub blocked_positions: HashSet<(i32, i32)>,
    // pos -> (dist, enter_from)
    pub distances: HashMap<(i32, i32), (f32, (i32, i32))>,
}

impl Default for PathfindGrid {
    fn default() -> Self {
        Self::new()
    }
}

impl PathfindGrid {
    pub fn new() -> Self {
        Self {
            blocked_positions: Default::default(),
            distances: Default::default(),
        }
    }

    pub fn run(&mut self, (start_x, start_y): (i32, i32), range: f32) {
        self.distances.clear();

        // (pos, dist, enter_from)
        let mut next: Vec<((i32, i32), f32, (i32, i32))> =
            vec![((start_x, start_y), 0.0, (start_x, start_y))];
        let sqrt_2 = 2f32.sqrt();

        while !next.is_empty() {
            let (node, dist, enter_from) = next.remove(0);

            if let Some((prev_dist, _prev_enter_from)) = self.distances.get(&node) {
                if *prev_dist <= dist {
                    // We already know another shorter route to this node
                    continue;
                }
            }

            self.distances.insert(node, (dist, enter_from));
            let (x, y) = node;

            let neighbors = [
                ((x - 1, y - 1), dist + sqrt_2),
                ((x - 1, y), dist + 1.0),
                ((x - 1, y + 1), dist + sqrt_2),
                ((x, y - 1), dist + 1.0),
                ((x, y + 1), dist + 1.0),
                ((x + 1, y - 1), dist + sqrt_2),
                ((x + 1, y), dist + 1.0),
                ((x + 1, y + 1), dist + sqrt_2),
            ];

            for (neighbor_pos, neighbor_dist) in neighbors {
                if neighbor_dist <= range && (!self.blocked_positions.contains(&neighbor_pos)) {
                    next.push((neighbor_pos, neighbor_dist, node));
                }
            }
        }
    }
}
