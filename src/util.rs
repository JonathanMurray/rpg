use crate::core::Position;

pub fn are_adjacent(a: Position, b: Position) -> bool {
    (b.0 - a.0).abs() == 1 && (b.1 - a.1).abs() == 1
}
