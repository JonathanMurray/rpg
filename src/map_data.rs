use core::f32;
use std::{cell::Cell, collections::HashMap, fs, iter, rc::Rc};

use indexmap::IndexMap;
use macroquad::rand::ChooseRandom;
use rand::{random_range, Rng};
use serde::{Deserialize, Serialize};

use crate::{
    bot::BotBehaviour,
    core::{
        Ability, AbilityId, AbilityTarget, Action, ActionReach, ActionTarget, Attributes,
        BaseAction, Bot, Character, CharacterId, CharacterKind, Condition, CoreGame, HandType,
        OnAttackedReaction, OnHitReaction, Party, Position, Shield, Weapon,
    },
    data::{
        PassiveSkill, BAD_BOW, BAD_DAGGER, BAD_RAPIER, BAD_SMALL_SHIELD, BAD_SWORD, BAD_WAR_HAMMER,
        ENEMY_BRACE, ENEMY_INSPIRE, ENEMY_TACKLE, GOOD_CHAIN_MAIL, LEATHER_ARMOR, MAGI_HEAL,
        MAGI_INFLICT_HORRORS, MAGI_INFLICT_WOUNDS, SHIRT, SMALL_SHIELD, SWORD,
    },
    grid::GameGrid,
    pathfind::{Occupation, Path, PathfindGrid},
    resources::GameResources,
    sounds::SoundPlayer,
    textures::{PortraitId, SpriteId, TerrainId},
    util::{adjacent_cells, are_entities_within_melee, CustomShuffle},
};

pub fn create_game_grid(
    map_data: &MapData,
    sound_player: SoundPlayer,
    resources: &GameResources,
    party: &Rc<Party>,
) -> GameGrid {
    let mut characters: IndexMap<CharacterId, Rc<Character>> = Default::default();

    let pathfind_grid = Rc::new(PathfindGrid::new(map_data.grid_dimensions));

    for (i, char_data) in map_data.characters.iter().enumerate() {
        let pos = char_data.pos;
        let char = create_character(pos, *char_data, Some(&party), i as CharacterId);
        pathfind_grid.set_occupied(pos, Some(Occupation::Character(char.id())));
        characters.insert(char.id(), char);
    }

    for pos in map_data.terrain_objects.keys().copied() {
        pathfind_grid.set_occupied(pos, Some(Occupation::Terrain));
    }

    let characters_map: HashMap<CharacterId, Rc<Character>> = characters
        .iter()
        .map(|(_id, ch)| (ch.id(), Rc::clone(ch)))
        .collect();
    let mut game_grid = GameGrid::new(
        0,
        characters_map,
        resources.sprites.clone(),
        resources.big_font.clone(),
        resources.simple_font.clone(),
        resources.terrain_atlas.clone(),
        pathfind_grid.clone(),
        map_data.background.clone(),
        map_data.terrain_objects.clone(),
        map_data.decorations.clone(),
        resources.status_textures.clone(),
        resources.effect_textures.clone(),
        sound_player,
    );

    game_grid.auto_tile_terrain_objects();

    game_grid
}

#[derive(Debug)]
pub struct MapData {
    pub grid_dimensions: (u32, u32),
    pub terrain_objects: IndexMap<Position, TerrainId>,
    pub decorations: IndexMap<Position, TerrainId>,
    pub background: IndexMap<Position, TerrainId>,
    pub characters: Vec<CharacterData>,
}

impl MapData {
    pub fn save_to_file(&self, filename: &str) {
        let terrain_objects = keys_pos_to_str(&self.terrain_objects);
        let background = keys_pos_to_str(&self.background);
        let decorations = keys_pos_to_str(&self.decorations);
        let map_data = SerializableMapData {
            grid_dimensions: self.grid_dimensions,
            terrain_objects,
            background,
            decorations,
            characters: self.characters.clone(),
        };
        let json_str = serde_json::to_string_pretty(&map_data).unwrap();
        fs::write(filename, json_str).expect("Writing json to file");
    }

    pub fn load_from_file(filename: &str) -> Self {
        let json = fs::read_to_string(filename).unwrap();
        let map_data = match serde_json::from_str::<SerializableMapData>(&json) {
            Ok(map_data) => map_data,
            Err(e) => {
                println!("File contents: {}", json);
                panic!("Failed to read from file: {:?}", e);
            }
        };
        Self {
            grid_dimensions: map_data.grid_dimensions,
            terrain_objects: keys_str_to_pos(&map_data.terrain_objects),
            background: keys_str_to_pos(&map_data.background),
            decorations: keys_str_to_pos(&map_data.decorations),
            characters: map_data.characters,
        }
    }
}

fn keys_pos_to_str<V: Copy>(map: &IndexMap<Position, V>) -> IndexMap<String, V> {
    map.iter().map(|(k, v)| (serialise_pos(*k), *v)).collect()
}

fn keys_str_to_pos<V: Copy>(map: &IndexMap<String, V>) -> IndexMap<Position, V> {
    map.iter().map(|(k, v)| (deserialise_pos(k), *v)).collect()
}

fn serialise_pos(pos: Position) -> String {
    format!("{pos:?}")
}

fn deserialise_pos(s: &str) -> Position {
    let without_paren = &s[1..s.len() - 1];
    match &without_paren.split(", ").collect::<Vec<_>>()[..] {
        [x, y] => (x.parse::<i32>().unwrap(), y.parse::<i32>().unwrap()),
        _ => panic!(),
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct SerializableMapData {
    pub grid_dimensions: (u32, u32),
    pub terrain_objects: IndexMap<String, TerrainId>,
    pub background: IndexMap<String, TerrainId>,
    pub decorations: IndexMap<String, TerrainId>,
    pub characters: Vec<CharacterData>,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq)]
pub enum CharacterType {
    Bob,
    Alice,
    Clara,
    Skeleton,
    Ogre,
    Ghoul1,
    Ghoul2,
}

impl CharacterType {
    pub fn sprite_id(&self) -> SpriteId {
        match self {
            CharacterType::Bob => SpriteId::Bob,
            CharacterType::Alice => SpriteId::Alice,
            CharacterType::Clara => SpriteId::Clara,
            CharacterType::Skeleton => SpriteId::Skeleton,
            CharacterType::Ogre => SpriteId::Ogre,
            CharacterType::Ghoul1 => SpriteId::Ghoul,
            CharacterType::Ghoul2 => SpriteId::Ghoul,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub struct CharacterData {
    pub type_: CharacterType,
    pub pos: Position,
    pub health: Option<u32>,
    pub main_hand: Option<WeaponId>,
    pub shield: Option<ShieldId>,
}

impl CharacterData {
    pub fn base(type_: CharacterType, pos: Position) -> Self {
        Self {
            type_,
            pos,
            health: None,
            main_hand: None,
            shield: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
enum WeaponId {
    Sword,
    BadSword,
    BadDagger,
    BadBow,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
enum ShieldId {
    BadSmallShield,
}

fn create_weapon(id: WeaponId) -> Weapon {
    match id {
        WeaponId::Sword => SWORD,
        WeaponId::BadSword => BAD_SWORD,
        WeaponId::BadDagger => BAD_DAGGER,
        WeaponId::BadBow => BAD_BOW,
    }
}

fn create_shield(id: ShieldId) -> Shield {
    match id {
        ShieldId::BadSmallShield => BAD_SMALL_SHIELD,
    }
}

pub fn create_character(
    pos: Position,
    char_data: CharacterData,
    party: Option<&Rc<Party>>,
    id: CharacterId,
) -> Rc<Character> {
    let char = match char_data.type_ {
        CharacterType::Bob => {
            let bob = Character::new(
                CharacterKind::Player(Rc::clone(party.unwrap())),
                "Bob",
                PortraitId::Bob,
                char_data.type_.sprite_id(),
                Attributes::new(5, 3, 3, 3),
                pos,
            );
            bob.set_weapon(HandType::MainHand, SWORD);
            bob
        }
        CharacterType::Alice => {
            let bob = Character::new(
                CharacterKind::Player(Rc::clone(party.unwrap())),
                "Alice",
                PortraitId::Alice,
                char_data.type_.sprite_id(),
                Attributes::new(5, 3, 3, 3),
                pos,
            );
            bob.set_weapon(HandType::MainHand, SWORD);
            bob
        }
        CharacterType::Clara => {
            let bob = Character::new(
                CharacterKind::Player(Rc::clone(party.unwrap())),
                "Clara",
                PortraitId::Clara,
                char_data.type_.sprite_id(),
                Attributes::new(5, 3, 3, 3),
                pos,
            );
            bob.set_weapon(HandType::MainHand, SWORD);
            bob
        }
        CharacterType::Skeleton => {
            let skeleton = Character::new(
                bot(BotBehaviour::Fighter(Default::default()), 12.0),
                "Skeleton",
                PortraitId::Skeleton,
                char_data.type_.sprite_id(),
                Attributes::new(4, 4, 4, 1),
                pos,
            );
            skeleton.health.change_max_value_to(35);
            skeleton.armor_piece.set(Some(LEATHER_ARMOR));
            skeleton.set_weapon(HandType::MainHand, BAD_RAPIER);
            skeleton.set_shield(SMALL_SHIELD);
            skeleton.learn_ability(ENEMY_BRACE);
            skeleton.learn_ability(ENEMY_INSPIRE);
            skeleton
        }
        CharacterType::Ghoul1 => {
            // TODO these should have archer behaviour, i.e. run away from melee
            let ghoul = Character::new(
                bot(BotBehaviour::Fighter(Default::default()), 9.0),
                "Ghoul",
                PortraitId::Ghoul,
                char_data.type_.sprite_id(),
                Attributes::new(2, 3, 2, 1),
                pos,
            );
            ghoul.health.change_max_value_to(9);
            ghoul.set_weapon(HandType::MainHand, BAD_BOW);
            ghoul
        }
        CharacterType::Ghoul2 => {
            let ghoul = Character::new(
                bot(BotBehaviour::Fighter(Default::default()), 12.0),
                "Ghoul",
                PortraitId::Ghoul,
                char_data.type_.sprite_id(),
                Attributes::new(3, 2, 1, 1),
                pos,
            );
            ghoul.health.change_max_value_to(12);
            ghoul.armor_piece.set(Some(SHIRT));
            ghoul.set_weapon(HandType::MainHand, BAD_SWORD);
            ghoul
        }
        CharacterType::Ogre => {
            let mut ogre = Character::new(
                bot(BotBehaviour::Fighter(Default::default()), 10.0),
                "Ogre",
                PortraitId::Ogre,
                SpriteId::Ogre,
                Attributes::new(9, 4, 3, 1),
                pos,
            );
            ogre.health.change_max_value_to(45);
            ogre.armor_piece.set(Some(GOOD_CHAIN_MAIL));
            ogre.set_weapon(HandType::MainHand, BAD_WAR_HAMMER);
            ogre.learn_ability(ENEMY_TACKLE);
            ogre.known_passive_skills.push(PassiveSkill::BloodRage);
            ogre
        }
    };

    if let Some(health) = char_data.health {
        char.health.change_max_value_to(health);
    }
    if let Some(weapon_id) = char_data.main_hand {
        char.set_weapon(HandType::MainHand, create_weapon(weapon_id));
    }
    if let Some(shield_id) = char_data.shield {
        char.set_shield(create_shield(shield_id));
        char.learn_ability(ENEMY_BRACE);
    }

    char.set_id(id);
    Rc::new(char)
}

fn bot(behaviour: BotBehaviour, move_speed: f32) -> CharacterKind {
    CharacterKind::Bot(Bot {
        behaviour,
        base_movement: move_speed,
    })
}
