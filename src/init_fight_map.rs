use std::{
    collections::{HashMap, HashSet},
    fs,
    rc::Rc,
};

use macroquad::rand::ChooseRandom;
use rand::{
    distr::{Distribution, Uniform},
    Rng,
};

use crate::{
    bot::BotBehaviour,
    core::{
        Attributes, BaseAction, Behaviour, Character, CharacterId, Characters, HandType, Position,
    },
    data::{
        BAD_BOW, BAD_DAGGER, BAD_RAPIER, BAD_SMALL_SHIELD, BAD_SWORD, BAD_WAR_HAMMER, CHAIN_MAIL,
        MAGI_HEAL, MAGI_INFLICT_WOUNDS, SHIRT, SWORD,
    },
    pathfind::PathfindGrid,
    textures::{PortraitId, SpriteId, TerrainId},
};

pub fn init_fight_map(player_characters: Vec<Character>, fight_id: FightId) -> GameInitState {
    let mut rng = rand::rng();

    let map_filename = match fight_id {
        FightId::Easy1 => "map1.txt",
        FightId::Easy2 => "map2.txt",
        FightId::Easy3 => "map3.txt",
        FightId::Elite => "map_elite.txt",
        FightId::Elite2 => "map_elite2.txt",
    };
    let map_str = fs::read_to_string(map_filename).unwrap();
    let mut terrain_objects: HashMap<Position, TerrainId> = Default::default();
    let mut water_grid: HashSet<Position> = Default::default();

    let mut player_positions = vec![];
    let mut enemy_positions: HashMap<u32, Vec<Position>> = HashMap::new();

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
                    player_positions.push(pos);
                }
                '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' => {
                    let digit = ch.to_digit(10).unwrap();
                    enemy_positions.entry(digit).or_default().push(pos);
                }
                ' ' => {}
                _ => panic!("Unhandled map object: {}", ch),
            }
        }
        row += 1;
    }

    for character in &player_characters {
        let i = rng.random_range(..player_positions.len());

        character.position.set(player_positions.remove(i));
    }

    let mut characters = player_characters;
    match fight_id {
        FightId::Easy1 => {
            let pos = *enemy_positions[&0].choose().unwrap();
            let melee = Character::new(
                Behaviour::Bot(BotBehaviour::Normal),
                "Thug",
                PortraitId::Skeleton,
                SpriteId::Skeleton,
                Attributes::new(3, 2, 2, 2),
                pos,
            );
            melee.armor_piece.set(Some(SHIRT));
            melee.set_weapon(HandType::MainHand, BAD_DAGGER);

            let pos = *enemy_positions[&1].choose().unwrap();
            let ranged = Character::new(
                Behaviour::Bot(BotBehaviour::Normal),
                "Archer",
                PortraitId::Skeleton,
                SpriteId::Skeleton,
                Attributes::new(1, 3, 3, 1),
                pos,
            );
            ranged.set_weapon(HandType::MainHand, BAD_BOW);

            characters.extend_from_slice(&[melee, ranged]);
        }

        FightId::Easy2 => {
            let pos = *enemy_positions[&0].choose().unwrap();
            let tanky = Character::new(
                Behaviour::Bot(BotBehaviour::Normal),
                "Guard",
                PortraitId::Skeleton,
                SpriteId::Skeleton2,
                Attributes::new(4, 3, 2, 2),
                pos,
            );
            tanky.health.change_max_value_to(20);
            tanky.armor_piece.set(Some(CHAIN_MAIL));
            tanky.set_shield(BAD_SMALL_SHIELD);
            tanky.set_weapon(HandType::MainHand, BAD_RAPIER);

            characters.extend_from_slice(&[tanky]);
        }

        FightId::Easy3 => {
            for i in 0..4 {
                let pos = *enemy_positions[&i].choose().unwrap();
                let enemy = Character::new(
                    Behaviour::Bot(BotBehaviour::Normal),
                    "Ghoul",
                    PortraitId::Skeleton,
                    SpriteId::Ghoul,
                    Attributes::new(2, 1, 1, 1),
                    pos,
                );
                if i % 2 == 0 {
                    enemy.armor_piece.set(Some(SHIRT));
                } else {
                    enemy.set_shield(BAD_SMALL_SHIELD);
                }
                enemy.set_weapon(HandType::MainHand, BAD_DAGGER);
                characters.push(enemy);
            }
        }

        FightId::Elite => {
            let pos = *enemy_positions[&0].choose().unwrap();
            let tanky = Character::new(
                Behaviour::Bot(BotBehaviour::Normal),
                "Ogre",
                PortraitId::Skeleton,
                SpriteId::Ogre,
                Attributes::new(4, 1, 1, 1),
                pos,
            );
            tanky.health.change_max_value_to(25);
            tanky.armor_piece.set(Some(CHAIN_MAIL));
            tanky.set_weapon(HandType::MainHand, BAD_WAR_HAMMER);
            //tanky.base_move_speed.set(0.7);
            characters.push(tanky);

            for i in 1..5 {
                let pos = *enemy_positions[&i].choose().unwrap();
                let archer = Character::new(
                    Behaviour::Bot(BotBehaviour::Normal),
                    "Archer",
                    PortraitId::Ghoul,
                    SpriteId::Ghoul,
                    Attributes::new(1, 1, 2, 1),
                    pos,
                );
                archer.set_weapon(HandType::MainHand, BAD_BOW);
                characters.push(archer);
            }
        }

        FightId::Elite2 => {
            let pos = *enemy_positions[&0].choose().unwrap();
            let mut magi = Character::new(
                Behaviour::Bot(BotBehaviour::Magi(Default::default())),
                "Magi",
                PortraitId::Magi,
                SpriteId::Magi,
                Attributes::new(4, 1, 3, 5),
                pos,
            );
            magi.known_actions.push(BaseAction::CastSpell(MAGI_HEAL));
            magi.armor_piece.set(Some(SHIRT));
            magi.set_weapon(HandType::MainHand, SWORD);
            magi.known_actions
                .push(BaseAction::CastSpell(MAGI_INFLICT_WOUNDS));
            magi.health.change_max_value_to(25);
            characters.push(magi);

            for i in 1..3 {
                let pos = *enemy_positions[&i].choose().unwrap();
                let tanky = Character::new(
                    Behaviour::Bot(BotBehaviour::Normal),
                    "Enslaved",
                    PortraitId::Ghoul,
                    SpriteId::Skeleton2,
                    Attributes::new(3, 1, 1, 1),
                    pos,
                );
                tanky.health.change_max_value_to(20);
                tanky.armor_piece.set(Some(CHAIN_MAIL));
                if i % 2 == 0 {
                    tanky.set_weapon(HandType::MainHand, BAD_SWORD);
                } else {
                    tanky.set_weapon(HandType::MainHand, BAD_RAPIER);
                }
                characters.push(tanky);
            }
        }
    }

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
        active_character_id: 0,
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
    Easy1,
    Easy2,
    Easy3,
    Elite,
    Elite2,
}
