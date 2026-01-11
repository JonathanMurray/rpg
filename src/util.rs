use macroquad::color::Color;
use rand::Rng;

use crate::core::{sq_distance_between, Position, MELEE_RANGE_SQUARED};

pub fn are_entities_within_melee(a: Position, b: Position) -> bool {
    sq_distance_between(a, b) <= MELEE_RANGE_SQUARED
}

pub fn adjacent_cells((x, y): Position) -> Vec<Position> {
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

pub fn line_collision(from: (i32, i32), to: (i32, i32), mut visitor: impl FnMut(i32, i32)) {
    // Bresenham's line algorithm
    let dx = (to.0 - from.0).abs();
    let dy = (to.1 - from.1).abs();
    let sx = (to.0 - from.0).signum();
    let sy = (to.1 - from.1).signum();
    let mut err = dx - dy;
    let (mut x, mut y) = from;
    loop {
        visitor(x, y);
        if x == to.0 && y == to.1 {
            break;
        }
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }
}

pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::new(r as f32 / 256.0, g as f32 / 256.0, b as f32 / 256.0, 1.0)
}

pub const COL_RED: Color = rgb(223, 53, 81);
pub const COL_GREEN_0: Color = rgb(93 , 146,38);
pub const COL_GREEN_1: Color = rgb(144 , 181,58);
pub const COL_GREEN_2: Color = rgb(201 , 226,118);
pub const COL_BLUE: Color = rgb(58, 139, 194);