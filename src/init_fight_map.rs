use std::{
    collections::{
        hash_map::{self, Entry},
        HashMap, HashSet,
    },
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
        Attributes, BaseAction, Bot, Character, CharacterId, CharacterKind, Characters, HandType,
        Position,
    },
    data::{
        PassiveSkill, BAD_BOW, BAD_DAGGER, BAD_RAPIER, BAD_SMALL_SHIELD, BAD_SWORD, BAD_WAR_HAMMER,
        CHAIN_MAIL, ENEMY_BRACE, ENEMY_INSPIRE, ENEMY_TACKLE, GOOD_CHAIN_MAIL, LEATHER_ARMOR,
        MAGI_HEAL, MAGI_INFLICT_WOUNDS, SHIRT, SWORD,
    },
    pathfind::{Occupation, PathfindGrid, CELLS_PER_ENTITY},
    textures::{PortraitId, SpriteId, TerrainId},
};

fn bot(behaviour: BotBehaviour, move_speed: f32) -> CharacterKind {
    CharacterKind::Bot(Bot {
        behaviour,
        base_movement: move_speed,
    })
}

pub fn init_fight_map(player_characters: Vec<Character>, fight_id: FightId) -> GameInitState {
    let mut rng = rand::rng();

    let map_filename = match fight_id {
        FightId::EasyPair => "map_easy_pair.txt",
        FightId::EasyGuard => "map_easy_guard.txt",
        FightId::EasyCluster => "map_easy_cluster.txt",
        FightId::EasySurrounded => "map_easy_surrounded.txt",
        FightId::EasyRiver => "map_easy_river.txt",
        FightId::EliteOgre => "map_elite.txt",
        FightId::EliteMagi => "map_elite2.txt",
        FightId::Test => "map_test.txt",
        FightId::VerticalSlice => "map_vertical_slice.txt",
    };
    let map_str = fs::read_to_string(map_filename).unwrap();
    let mut terrain_objects: HashMap<Position, TerrainId> = Default::default();
    let mut terrain_center_positions: HashSet<Position> = Default::default();
    let mut water_grid: HashSet<Position> = Default::default();

    let mut player_positions = vec![];
    let mut enemy_positions: HashMap<u32, Vec<Position>> = HashMap::new();

    let terrain_object_chars: HashMap<char, TerrainId> = [
        ('Y', TerrainId::StoneWallConcaveNorthWest),
        ('U', TerrainId::StoneWallNorth),
        ('I', TerrainId::StoneWallConcaveNorthEast),
        ('H', TerrainId::StoneWallWest),
        ('K', TerrainId::StoneWallEast),
        ('N', TerrainId::StoneWallConcaveSouthWest),
        ('M', TerrainId::StoneWallSouth),
        (',', TerrainId::StoneWallConcaveSouthEast),
        ('y', TerrainId::NewWaterNorthWest),
        ('u', TerrainId::NewWaterNorth),
        ('i', TerrainId::NewWaterNorthEast),
        ('j', TerrainId::NewWaterWest),
        ('k', TerrainId::NewWater),
        ('l', TerrainId::NewWaterEast),
        ('n', TerrainId::NewWaterSouthWest),
        ('m', TerrainId::NewWaterSouth),
        (';', TerrainId::NewWaterSouthEast),
        ('O', TerrainId::StoneWallConvexSouthWest),
        ('L', TerrainId::StoneWallConvexSouthEast),
        ('C', TerrainId::StoneWallConvexNorthWest),
        ('V', TerrainId::StoneWallConvexNorthEast),
        ('B', TerrainId::StoneWallInner),
    ]
    .into_iter()
    .collect();

    // Start on cell with index 1, since all entities are 3x3; if the middle is on index 1, the edge will be on index 0.
    let mut row = 1;
    for line in map_str.lines() {
        if line.starts_with('+') {
            continue;
        }

        let mut col = 1;
        for ch in line.chars() {
            let pos = (col, row);
            if let Some(terrain_id) = terrain_object_chars.get(&ch) {
                terrain_objects.insert(pos, *terrain_id);
                terrain_center_positions.insert(pos);
            } else {
                match ch {
                    'W' => {
                        water_grid.insert(pos);
                        terrain_center_positions.insert(pos);
                    }
                    'X' => {
                        terrain_objects.insert(pos, TerrainId::StoneWall);
                        terrain_center_positions.insert(pos);
                    }
                    'B' => {
                        terrain_objects.insert(pos, TerrainId::Bush);
                        terrain_center_positions.insert(pos);
                    }
                    'R' => {
                        terrain_objects.insert(pos, TerrainId::Boulder2);
                        terrain_center_positions.insert(pos);
                    }
                    'S' => {
                        terrain_objects.insert(pos, TerrainId::Table);
                        terrain_center_positions.insert(pos);
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

            col += CELLS_PER_ENTITY as i32;
        }
        row += CELLS_PER_ENTITY as i32;
    }

    for character in &player_characters {
        let i = rng.random_range(..player_positions.len());
        let pos = player_positions.remove(i);
        character.position.set(pos);
    }

    let mut characters = player_characters;
    match fight_id {
        FightId::EasyPair => {
            let pos = *enemy_positions[&0].choose().unwrap();
            let melee = Character::new(
                bot(BotBehaviour::Normal, 12.0),
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
                bot(BotBehaviour::Normal, 12.0),
                "Archer",
                PortraitId::Skeleton,
                SpriteId::Skeleton,
                Attributes::new(1, 3, 3, 1),
                pos,
            );
            ranged.set_weapon(HandType::MainHand, BAD_BOW);

            characters.extend_from_slice(&[melee, ranged]);
        }
        FightId::EasyGuard => {
            let pos = *enemy_positions[&0].choose().unwrap();
            let tanky = Character::new(
                bot(BotBehaviour::Normal, 12.0),
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
        FightId::EasyCluster => {
            for i in 0..4 {
                let pos = *enemy_positions[&i].choose().unwrap();
                let enemy = Character::new(
                    bot(BotBehaviour::Normal, 12.0),
                    "Ghoul",
                    PortraitId::Skeleton,
                    SpriteId::Ghoul,
                    Attributes::new(2, 1, 1, 1),
                    pos,
                );
                if i % 2 == 0 {
                    enemy.armor_piece.set(Some(SHIRT));
                    enemy.set_shield(BAD_SMALL_SHIELD);
                }
                enemy.set_weapon(HandType::MainHand, BAD_DAGGER);
                characters.push(enemy);
            }
        }
        FightId::EasySurrounded => {
            for i in 0..4 {
                let pos = *enemy_positions[&i].choose().unwrap();
                let enemy = Character::new(
                    bot(BotBehaviour::Normal, 12.0),
                    "Ghoul",
                    PortraitId::Skeleton,
                    SpriteId::Ghoul,
                    Attributes::new(2, 1, 1, 1),
                    pos,
                );
                enemy.armor_piece.set(Some(SHIRT));
                enemy.set_weapon(HandType::MainHand, BAD_DAGGER);
                characters.push(enemy);
            }
        }
        FightId::EasyRiver => {
            for i in 0..6 {
                let pos = *enemy_positions[&i].choose().unwrap();
                let enemy = Character::new(
                    bot(BotBehaviour::Normal, 12.0),
                    "Ghoul",
                    PortraitId::Skeleton,
                    SpriteId::Ghoul,
                    Attributes::new(1, 2, 1, 1),
                    pos,
                );
                if i < 5 {
                    enemy.armor_piece.set(Some(SHIRT));
                    enemy.set_shield(BAD_SMALL_SHIELD);
                    enemy.set_weapon(HandType::MainHand, BAD_DAGGER);
                } else {
                    enemy.set_weapon(HandType::MainHand, BAD_BOW);
                }
                characters.push(enemy);
            }
        }
        FightId::EliteOgre => {
            let pos = *enemy_positions[&0].choose().unwrap();
            let ogre = Character::new(
                bot(BotBehaviour::Normal, 12.0),
                "Ogre",
                PortraitId::Ogre,
                SpriteId::Ogre,
                Attributes::new(4, 1, 1, 1),
                pos,
            );
            ogre.health.change_max_value_to(25);
            ogre.armor_piece.set(Some(CHAIN_MAIL));
            ogre.set_weapon(HandType::MainHand, BAD_WAR_HAMMER);
            characters.push(ogre);

            for i in 1..5 {
                let pos = *enemy_positions[&i].choose().unwrap();
                let archer = Character::new(
                    bot(BotBehaviour::Normal, 12.0),
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
        FightId::EliteMagi => {
            let pos = *enemy_positions[&0].choose().unwrap();
            let magi = Character::new(
                bot(BotBehaviour::Magi(Default::default()), 9.0),
                "Magi",
                PortraitId::Magi,
                SpriteId::Magi,
                Attributes::new(4, 1, 3, 5),
                pos,
            );
            magi.known_actions
                .borrow_mut()
                .push(BaseAction::UseAbility(MAGI_HEAL));
            magi.armor_piece.set(Some(SHIRT));
            magi.set_weapon(HandType::MainHand, BAD_SWORD);
            magi.known_actions
                .borrow_mut()
                .push(BaseAction::UseAbility(MAGI_INFLICT_WOUNDS));
            magi.health.change_max_value_to(25);
            characters.push(magi);

            for i in 1..3 {
                let pos = *enemy_positions[&i].choose().unwrap();
                let tanky = Character::new(
                    bot(BotBehaviour::Normal, 12.0),
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
        FightId::Test => {
            let mut enemies = vec![];

            let mut e1 = Character::new(
                bot(BotBehaviour::Fighter(Default::default()), 10.0),
                "Enemy 1",
                PortraitId::Skeleton,
                SpriteId::Skeleton,
                Attributes::new(1, 10, 1, 1),
                *enemy_positions[&0].choose().unwrap(),
            );
            e1.armor_piece.set(Some(GOOD_CHAIN_MAIL));
            e1.learn_ability(ENEMY_TACKLE);
            e1.known_passive_skills.push(PassiveSkill::BloodRage);
            e1.health.change_max_value_to(300);
            enemies.push(e1);

            enemies.push(Character::new(
                bot(BotBehaviour::Fighter(Default::default()), 12.0),
                "Enemy 2",
                PortraitId::Skeleton,
                SpriteId::Skeleton2,
                Attributes::new(1, 1, 1, 1),
                *enemy_positions[&1].choose().unwrap(),
            ));
            /*

            enemies.push(Character::new(
                bot(BotBehaviour::Normal, 12.0),
                "Enemy 1",
                PortraitId::Ghoul,
                SpriteId::Ghoul,
                Attributes::new(1, 1, 1, 1),
                *enemy_positions[&2].choose().unwrap(),
            ));
            enemies.push(Character::new(
                bot(BotBehaviour::Normal, 12.0),
                "Enemy 1",
                PortraitId::Skeleton,
                SpriteId::Ogre,
                Attributes::new(1, 1, 1, 1),
                *enemy_positions[&3].choose().unwrap(),
            ));
            enemies.push(Character::new(
                bot(BotBehaviour::Normal, 12.0),
                "Enemy 1",
                PortraitId::Skeleton,
                SpriteId::Magi,
                Attributes::new(1, 1, 1, 1),
                *enemy_positions[&4].choose().unwrap(),
            ));
             */

            for e in &enemies {
                e.set_weapon(HandType::MainHand, BAD_DAGGER);
            }

            characters.extend_from_slice(&enemies);
        }
        FightId::VerticalSlice => {
            for i in 0..=2 {
                let pos = *enemy_positions[&i].choose().unwrap();
                let ghoul = Character::new(
                    bot(BotBehaviour::Fighter(Default::default()), 12.0),
                    "Ghoul",
                    PortraitId::Ghoul,
                    SpriteId::Ghoul,
                    Attributes::new(2, 3, 1, 1),
                    pos,
                );

                if i == 2 {
                    ghoul.position.set((pos.0 - 2, pos.1));
                }
                ghoul.health.change_max_value_to(15 + i);
                ghoul.armor_piece.set(Some(SHIRT));
                ghoul.set_weapon(HandType::MainHand, BAD_SWORD);
                if i % 2 == 0 {
                    ghoul.set_shield(BAD_SMALL_SHIELD);
                    ghoul.learn_ability(ENEMY_BRACE);
                }
                characters.push(ghoul);
            }
            for i in 3..=4 {
                // TODO these should have archer behaviour, i.e. run away from melee
                let pos = *enemy_positions[&i].choose().unwrap();
                let archer = Character::new(
                    bot(BotBehaviour::Fighter(Default::default()), 9.0),
                    "Ghoul",
                    PortraitId::Ghoul,
                    SpriteId::Ghoul,
                    Attributes::new(1, 3, 2, 1),
                    pos,
                );
                archer.health.change_max_value_to(9);
                archer.set_weapon(HandType::MainHand, BAD_BOW);
                characters.push(archer);
            }
            for i in 5..=5 {
                let pos = *enemy_positions[&i].choose().unwrap();
                let skeleton = Character::new(
                    bot(BotBehaviour::Fighter(Default::default()), 12.0),
                    "Skeleton",
                    PortraitId::Skeleton,
                    SpriteId::Skeleton,
                    Attributes::new(4, 4, 3, 1),
                    pos,
                );
                skeleton.health.change_max_value_to(35 + i - 5);
                skeleton.armor_piece.set(Some(LEATHER_ARMOR));
                skeleton.set_weapon(HandType::MainHand, BAD_RAPIER);
                skeleton.set_shield(BAD_SMALL_SHIELD);
                skeleton.learn_ability(ENEMY_BRACE);
                skeleton.learn_ability(ENEMY_INSPIRE);
                characters.push(skeleton);
            }
            for i in 6..=7 {
                let pos = *enemy_positions[&i].choose().unwrap();
                let ghoul = Character::new(
                    bot(BotBehaviour::Fighter(Default::default()), 12.0),
                    "Ghoul",
                    PortraitId::Ghoul,
                    SpriteId::Ghoul,
                    Attributes::new(2, 2, 1, 1),
                    pos,
                );
                ghoul.health.change_max_value_to(12 + i - 6);
                ghoul.armor_piece.set(Some(SHIRT));
                ghoul.set_weapon(HandType::MainHand, BAD_SWORD);
                if i % 2 == 0 {
                    ghoul.set_weapon(HandType::MainHand, BAD_DAGGER);
                }
                characters.push(ghoul);
            }
            for i in 8..=8 {
                let pos = *enemy_positions[&i].choose().unwrap();
                let mut ogre = Character::new(
                    bot(BotBehaviour::Fighter(Default::default()), 10.0),
                    "Ogre",
                    PortraitId::Ogre,
                    SpriteId::Ogre,
                    Attributes::new(12, 4, 3, 1),
                    pos,
                );
                ogre.health.change_max_value_to(56);
                ogre.armor_piece.set(Some(GOOD_CHAIN_MAIL));
                ogre.set_weapon(HandType::MainHand, BAD_WAR_HAMMER);
                ogre.learn_ability(ENEMY_TACKLE);
                ogre.known_passive_skills.push(PassiveSkill::BloodRage);
                characters.push(ogre);
            }
        }
    }

    for (x, y) in water_grid.iter().copied() {
        let id = match (
            water_grid.contains(&(x, y - CELLS_PER_ENTITY as i32)),
            water_grid.contains(&(x + CELLS_PER_ENTITY as i32, y)),
            water_grid.contains(&(x, y + CELLS_PER_ENTITY as i32)),
            water_grid.contains(&(x - CELLS_PER_ENTITY as i32, y)),
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

    // TODO: This should be dynamic based on map file
    let grid_dimensions: (u32, u32) = (20 * CELLS_PER_ENTITY, 15 * CELLS_PER_ENTITY);

    let mut background: HashMap<Position, TerrainId> = Default::default();
    let grass_variations = [
        TerrainId::Grass,
        TerrainId::Grass2,
        TerrainId::Grass3,
        TerrainId::Grass4,
    ];
    let uniform_distr = Uniform::new(0, grass_variations.len()).unwrap();
    let mut grass_choices = uniform_distr.sample_iter(&mut rng);
    for x in (1..grid_dimensions.0).step_by(CELLS_PER_ENTITY as usize) {
        for y in (1..grid_dimensions.1).step_by(CELLS_PER_ENTITY as usize) {
            let i = grass_choices.next().unwrap();
            let pos = (x as i32, y as i32);
            //background.insert(pos, grass_variations[i]);
            //TODO
            background.insert(pos, TerrainId::Floor);
        }
    }

    let characters = Characters::new(characters);

    let pathfind_grid = PathfindGrid::new(grid_dimensions);

    for pos in terrain_center_positions {
        pathfind_grid.set_occupied(pos, Some(Occupation::Terrain));
    }
    for ch in characters.iter() {
        pathfind_grid.set_occupied(ch.pos(), Some(Occupation::Character(ch.id())));
    }

    let pathfind_grid = Rc::new(pathfind_grid);

    GameInitState {
        characters,
        active_character_id: 0,
        pathfind_grid,
        background,
        terrain_objects,
    }
}

#[derive(Clone)]
pub struct GameInitState {
    pub characters: Characters,
    pub active_character_id: CharacterId,
    pub pathfind_grid: Rc<PathfindGrid>,
    pub background: HashMap<Position, TerrainId>,
    pub terrain_objects: HashMap<Position, TerrainId>,
}

impl GameInitState {
    pub fn try_add_terrain_object(&mut self, pos: Position, terrain: TerrainId) -> bool {
        if self.pathfind_grid.is_free(None, pos) {
            assert_eq!(self.terrain_objects.get(&pos), None);
            self.terrain_objects.insert(pos, terrain);
            self.pathfind_grid
                .set_occupied(pos, Some(Occupation::Terrain));
            true
        } else {
            //println!("Cannot add terrain. Space occupied");
            false
        }
    }

    pub fn try_add_character(&mut self, pos: Position, character: Character) -> bool {
        todo!("Add character to gameinitstate")
    }

    pub fn try_remove_terrain_object(&mut self, pos: &Position) -> bool {
        if self.pathfind_grid.occupied().get(pos).is_some() {
            self.pathfind_grid.set_occupied(*pos, None);
            self.terrain_objects.remove(pos);
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum FightId {
    EasyPair,
    EasyGuard,
    EasyCluster,
    EasySurrounded,
    EasyRiver,
    EliteOgre,
    EliteMagi,
    Test,
    VerticalSlice,
}
