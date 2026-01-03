use macroquad::texture::{draw_texture, Texture2D};
use rand::Rng;

use crate::{
    base_ui::draw_text_rounded,
    core::{sq_distance_between, Position, MELEE_RANGE_SQUARED},
    textures::DICE_SYMBOL,
};

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
