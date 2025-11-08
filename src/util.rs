use rand::Rng;

use crate::core::Position;

pub fn are_adjacent(a: Position, b: Position) -> bool {
    a != b && (b.0 - a.0).abs() <= 1 && (b.1 - a.1).abs() <= 1
}

pub fn adjacent_positions((x, y): Position) -> Vec<Position> {
    vec![
        (x, y - 1),
        (x + 1, y - 1),
        (x + 1, y),
        (x + 1, y + 1),
        (x, y + 1),
        (x - 1, y + 1),
        (x - 1, y),
        (x - 1, y - 1),
    ]
}

pub fn select_n_random<T: Copy>(mut from: Vec<T>, n: usize) -> Vec<T> {
    let mut selected = vec![];
    let mut rng = rand::rng();
    for _ in 0..n {
        let i = rng.random_range(..from.len());
        selected.push(from.remove(i));
    }
    selected
}
