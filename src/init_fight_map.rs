use std::{
    collections::{HashMap, HashSet},
    fs,
    rc::Rc,
};

use indexmap::IndexMap;
use macroquad::rand::ChooseRandom;
use rand::{
    distr::{Distribution, Uniform},
    Rng,
};

use crate::{
    bot::BotBehaviour,
    core::{
        Attributes, BaseAction, Bot, Character, CharacterId, CharacterKind, HandType, Position,
    },
    data::{
        PassiveSkill, BAD_BOW, BAD_DAGGER, BAD_RAPIER, BAD_SMALL_SHIELD, BAD_SWORD, BAD_WAR_HAMMER,
        CHAIN_MAIL, ENEMY_BRACE, ENEMY_INSPIRE, ENEMY_TACKLE, GOOD_CHAIN_MAIL, HULDRA_HEAL,
        HULDRA_INFLICT_WOUNDS, LEATHER_ARMOR, SHIRT,
    },
    map_data::{create_character, CharacterType, MapData},
    pathfind::{Occupation, PathfindGrid, TerrainType, CELLS_PER_ENTITY},
    textures::{PortraitId, SpriteId, TerrainId},
};

pub fn init_fight_map(player_characters: Vec<Rc<Character>>, fight_id: FightId) -> GameInitState {
    let mut player_chars_by_name: HashMap<&'static str, Rc<Character>> = player_characters
        .into_iter()
        .map(|ch| (ch.name, ch))
        .collect();
    let filename = match fight_id {
        FightId::VerticalSliceNew => "ogre_room.json",
        FightId::EasyCluster => "easy_map.json",
        FightId::Medium => "medium_map.json",
        FightId::Test => "test.json",
        FightId::EliteHuldra => "huldra.json",
        unhandled => todo!("Handle map: {:?}", unhandled),
    };
    let map_data = MapData::load_from_file(&format!("maps/{filename}"));
    let mut characters: Vec<Rc<Character>> = Default::default();
    let pathfind_grid = Rc::new(PathfindGrid::new(map_data.grid_dimensions));

    for (i, char_data) in map_data.characters.iter().enumerate() {
        let pos = char_data.pos;
        let char: Option<Rc<Character>> = match char_data.type_ {
            // TODO: Handle this better than string-matching on the name
            CharacterType::Bob => player_chars_by_name.remove("Bob").map(|ch| {
                ch.set_id(i as CharacterId);
                ch.position.set(pos);
                ch
            }),
            CharacterType::Alice => player_chars_by_name.remove("Alice").map(|ch| {
                ch.set_id(i as CharacterId);
                ch.position.set(pos);
                ch
            }),
            CharacterType::Clara => player_chars_by_name.remove("Clara").map(|ch| {
                ch.set_id(i as CharacterId);
                ch.position.set(pos);
                ch
            }),
            _ => Some(create_character(pos, *char_data, None, i as CharacterId)),
        };
        if let Some(char) = char {
            pathfind_grid.set_occupied(pos, Some(Occupation::Character(char.id())));
            characters.push(char);
        }
    }

    assert_eq!(
        player_chars_by_name.len(),
        0,
        "Unassigned player characters: {:?}",
        player_chars_by_name.keys()
    );

    for (pos, terrain_id) in map_data.terrain_objects.iter() {
        pathfind_grid.set_occupied(*pos, Some(Occupation::Terrain(terrain_id.terrain_type())));
    }

    GameInitState {
        characters,
        active_character_id: 0,
        pathfind_grid,
        background: map_data.background,
        terrain_objects: map_data.terrain_objects,
        decorations: map_data.decorations,
    }
}

#[derive(Clone)]
pub struct GameInitState {
    pub characters: Vec<Rc<Character>>,
    pub active_character_id: CharacterId,
    pub pathfind_grid: Rc<PathfindGrid>,
    pub background: IndexMap<Position, TerrainId>,
    pub terrain_objects: IndexMap<Position, TerrainId>,
    pub decorations: IndexMap<Position, TerrainId>,
}

impl GameInitState {
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
    Medium,
    EasyCluster,
    EasySurrounded,
    EasyRiver,
    EliteOgre,
    EliteHuldra,
    Test,
    VerticalSlice,
    VerticalSliceNew,
}
