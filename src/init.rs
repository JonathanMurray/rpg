use std::{
    collections::{HashMap, HashSet},
    fs,
    rc::Rc,
};

use rand::distr::{Distribution, Uniform};

use crate::{
    core::{Attributes, Character, CharacterId, Characters, HandType, Position},
    data::{BAD_DAGGER, BAD_SWORD, SHIRT},
    pathfind::PathfindGrid,
    textures::{PortraitId, SpriteId, TerrainId},
};

pub fn init(player_character: Character, fight_id: FightId) -> GameInitState {
    let active_character_id = 0;

    let skeleton_str = Character::new(
        false,
        "Skeleton",
        PortraitId::Skeleton,
        SpriteId::Skeleton,
        Attributes::new(4, 2, 1, 1),
        (0, 0),
    );
    skeleton_str.armor.set(Some(SHIRT));
    skeleton_str.set_weapon(HandType::MainHand, BAD_SWORD);

    let skeleton_agi = Character::new(
        false,
        "Skeleton",
        PortraitId::Skeleton,
        SpriteId::Skeleton,
        Attributes::new(1, 3, 1, 1),
        (0, 0),
    );
    skeleton_agi.set_weapon(HandType::MainHand, BAD_DAGGER);
    //skeleton2.set_shield(SMALL_SHIELD);

    let map_filename = match fight_id {
        FightId::Easy => "map1.txt",
        FightId::Hard => "map3.txt",
    };

    let map_str = fs::read_to_string(map_filename).unwrap();

    let mut terrain_objects: HashMap<Position, TerrainId> = Default::default();

    let mut water_grid: HashSet<Position> = Default::default();

    let mut player_pos = None;
    let mut enemy_positions = HashMap::new();

    let mut row = 0;
    for line in map_str.lines() {
        if line.starts_with('+') {
            continue;
        }

        for (col, ch) in line.chars().enumerate() {
            let pos = (col as i32, row);
            match ch {
                'W' => {
                    water_grid.insert(pos);
                }
                'B' => {
                    terrain_objects.insert(pos, TerrainId::Bush);
                }
                'R' => {
                    terrain_objects.insert(pos, TerrainId::Boulder2);
                }
                'S' => {
                    terrain_objects.insert(pos, TerrainId::TreeStump);
                }
                'P' => {
                    player_pos = Some(pos);
                }
                '0' => {
                    enemy_positions.insert(0, pos);
                }
                '1' => {
                    enemy_positions.insert(1, pos);
                }
                ' ' => {}
                _ => panic!("Unhandled map object: {}", ch),
            }
        }
        row += 1;
    }

    player_character.position.set(player_pos.unwrap());

    let characters = if fight_id == FightId::Easy {
        skeleton_agi.position.set(enemy_positions[&0]);
        vec![player_character, skeleton_agi]
    } else {
        skeleton_str.position.set(enemy_positions[&0]);
        skeleton_agi.position.set(enemy_positions[&1]);
        vec![player_character, skeleton_str, skeleton_agi]
    };

    for (x, y) in water_grid.iter().copied() {
        let id = match (
            water_grid.contains(&(x, y - 1)),
            water_grid.contains(&(x + 1, y)),
            water_grid.contains(&(x, y + 1)),
            water_grid.contains(&(x - 1, y)),
        ) {
            (false, false, false, _) => TerrainId::WaterBeachNorthEastSouth,
            (_, false, false, false) => TerrainId::WaterBeachEastSouthWest,
            (false, _, false, false) => TerrainId::WaterBeachSouthWestNorth,
            (false, false, _, false) => TerrainId::WaterBeachWestNorthEast,
            (false, false, _, _) => TerrainId::WaterBeachNorthEast,
            (_, false, false, _) => TerrainId::WaterBeachSouthEast,
            (_, _, false, false) => TerrainId::WaterBeachSouthWest,
            (false, _, _, false) => TerrainId::WaterBeachNorthWest,
            (_, false, _, false) => TerrainId::WaterBeachWestEast,
            (false, _, false, _) => TerrainId::WaterBeachNorthSouth,
            (false, _, _, _) => TerrainId::WaterBeachNorth,
            (_, false, _, _) => TerrainId::WaterBeachEast,
            (_, _, false, _) => TerrainId::WaterBeachSouth,
            (_, _, _, false) => TerrainId::WaterBeachWest,

            _ => TerrainId::Water,
        };

        terrain_objects.insert((x, y), id);
    }

    let grid_dimensions: (u32, u32) = (20, 15);

    let mut background: HashMap<Position, TerrainId> = Default::default();
    let grass_variations = [
        TerrainId::Grass,
        TerrainId::Grass2,
        TerrainId::Grass3,
        TerrainId::Grass4,
    ];
    let uniform_distribution = Uniform::new(0, grass_variations.len()).unwrap();
    let mut rng = rand::rng();
    let mut choices = uniform_distribution.sample_iter(&mut rng);

    for x in 0..grid_dimensions.0 {
        for y in 0..grid_dimensions.1 {
            let i = choices.next().unwrap();
            background.insert((x as i32, y as i32), grass_variations[i]);
        }
    }

    let pathfind_grid = PathfindGrid::new(grid_dimensions);
    for ch in &characters {
        pathfind_grid.set_blocked(ch.pos(), true);
    }
    for pos in terrain_objects.keys() {
        pathfind_grid.set_blocked(*pos, true);
    }

    let pathfind_grid = Rc::new(pathfind_grid);

    GameInitState {
        characters: Characters::new(characters),
        active_character_id,
        pathfind_grid,
        background,
        terrain_objects,
    }
}

pub struct GameInitState {
    pub characters: Characters,
    pub active_character_id: CharacterId,
    pub pathfind_grid: Rc<PathfindGrid>,
    pub background: HashMap<Position, TerrainId>,
    pub terrain_objects: HashMap<Position, TerrainId>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum FightId {
    Easy,
    Hard,
}
