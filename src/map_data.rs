use core::f32;
use std::{cell::Cell, collections::HashMap, fs, rc::Rc};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{
    bot::BotBehaviour,
    core::{
        ArrowStack, Attributes, BaseAction, Bot, Character, CharacterId, CharacterKind, Condition,
        EquipmentEntry, HandType, Party, PlayerId, Position, Shield, Weapon,
    },
    data::{
        BAD_BOW, BAD_DAGGER, BAD_RAPIER, BAD_SMALL_SHIELD, BAD_SWORD, BAD_WAR_HAMMER, CHAIN_MAIL,
        CHEAT_BOW, ENEMY_BRACE, ENEMY_INSPIRE, ENEMY_SLASHING, ENEMY_SLASHING_ATTACK, ENEMY_TACKLE,
        ENSLAVED_RAPIER, ENSLAVED_SWORD, GOOD_CHAIN_MAIL, HULDRA_HEAL, HULDRA_INFLICT_HORRORS,
        HULDRA_INFLICT_WOUNDS, KILL, SMALL_SHIELD,
    },
    grid::GameGrid,
    pathfind::{Occupation, PathfindGrid},
    resources::GameResources,
    sounds::SoundPlayer,
    textures::{PortraitId, SpriteId, TerrainId},
};

use crate::data::{
    PassiveSkill, ARCANE_POTION, BOW, CRIPPLING_SHOT, DAGGER, EXPLODING_ARROWS, FIREBALL,
    FIREBALL_INFERNO, FIREBALL_MASSIVE, FIREBALL_REACH, HEAL, HEALTH_POTION, HEAL_ENERGIZE,
    INFLICT_WOUNDS, INFLICT_WOUNDS_NECROTIC_INFLUENCE, INSPIRE, LEATHER_ARMOR, MANA_POTION,
    MEDIUM_SHIELD, PIERCING_SHOT, SHACKLED_MIND, SHIELD_BASH, SHIELD_BASH_KNOCKBACK, SHIRT, SMITE,
    SWEEP_ATTACK, SWORD,
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
        let char = create_character(pos, *char_data, Some(party), i as CharacterId);
        pathfind_grid.set_occupied(pos, Some(Occupation::Character(char.id())));
        characters.insert(char.id(), char);
    }

    for (pos, terrain_id) in map_data.terrain_objects.iter() {
        pathfind_grid.set_occupied(*pos, Some(Occupation::Terrain(terrain_id.terrain_type())));
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
    pub fn save_to_file(&self, filepath: &str) {
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
        fs::write(filepath, json_str).expect("Writing json to file");
    }

    pub fn load_from_file(filepath: &str) -> Self {
        let json = fs::read_to_string(filepath).unwrap();
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
    SkeletonLeader,
    Ogre,
    Huldra,
    Enslaved,
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
            CharacterType::SkeletonLeader => SpriteId::Skeleton,
            CharacterType::Ogre => SpriteId::Ogre,
            CharacterType::Huldra => SpriteId::Huldra,
            CharacterType::Enslaved => SpriteId::Skeleton2,
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
    BadRapier,
    EnslavedRapier,
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
        WeaponId::BadRapier => BAD_RAPIER,
        WeaponId::EnslavedRapier => ENSLAVED_RAPIER,
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
            let char = make_high_bob(party.unwrap());
            char.position.set(pos);
            char
        }
        CharacterType::Alice => {
            let char = make_high_alice(party.unwrap());
            char.position.set(pos);
            char
        }
        CharacterType::Clara => {
            let char = make_medium_clara(party.unwrap());
            char.position.set(pos);
            char
        }
        CharacterType::Skeleton => {
            let mut skeleton = Character::new(
                bot(BotBehaviour::Fighter(Default::default()), 14.0),
                "Skeleton",
                PortraitId::Skeleton,
                char_data.type_.sprite_id(),
                Attributes::new(4, 4, 4, 1),
                pos,
            );
            skeleton.health.change_max_value_to(35);
            skeleton.armor_piece.set(Some(SHIRT));
            skeleton.set_weapon(HandType::MainHand, BAD_RAPIER);
            skeleton.set_shield(SMALL_SHIELD);

            //skeleton.learn_ability(ENEMY_BRACE);
            //skeleton.learn_ability(ENEMY_SLASHING_ATTACK);
            skeleton.learn_attack_enhancement(ENEMY_SLASHING);

            skeleton
        }
        CharacterType::SkeletonLeader => {
            let skeleton = Character::new(
                bot(BotBehaviour::Fighter(Default::default()), 14.0),
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
                bot(BotBehaviour::Fighter(Default::default()), 11.0),
                "Ghoul",
                PortraitId::Ghoul,
                char_data.type_.sprite_id(),
                Attributes::new(1, 2, 1, 1),
                pos,
            );
            ghoul.health.change_max_value_to(9);
            ghoul.set_weapon(HandType::MainHand, BAD_BOW);
            ghoul
        }
        CharacterType::Ghoul2 => {
            let ghoul = Character::new(
                bot(BotBehaviour::Fighter(Default::default()), 14.0),
                "Ghoul",
                PortraitId::Ghoul,
                char_data.type_.sprite_id(),
                Attributes::new(2, 1, 1, 1),
                pos,
            );
            ghoul.health.change_max_value_to(12);
            ghoul.armor_piece.set(Some(SHIRT));
            ghoul.set_weapon(HandType::MainHand, BAD_SWORD);
            ghoul
        }
        CharacterType::Ogre => {
            let mut ogre = Character::new(
                bot(BotBehaviour::Fighter(Default::default()), 12.0),
                "Ogre",
                PortraitId::Ogre,
                SpriteId::Ogre,
                Attributes::new(9, 4, 3, 1),
                pos,
            );
            ogre.health.change_max_value_to(43);
            ogre.armor_piece.set(Some(CHAIN_MAIL));
            ogre.set_weapon(HandType::MainHand, BAD_WAR_HAMMER);
            ogre.learn_ability(ENEMY_TACKLE);
            ogre.learn_passive(PassiveSkill::BloodRage);
            ogre
        }
        CharacterType::Huldra => {
            let huldra = Character::new(
                //bot(BotBehaviour::Magi(Default::default()), 9.0),
                bot(BotBehaviour::Huldra(Default::default()), 12.0),
                "Huldra",
                PortraitId::Huldra,
                SpriteId::Huldra,
                Attributes::new(2, 5, 9, 5),
                pos,
            );
            huldra.learn_ability(HULDRA_HEAL);
            huldra.learn_ability(HULDRA_INFLICT_WOUNDS);
            huldra.learn_ability(HULDRA_INFLICT_HORRORS);
            huldra.armor_piece.set(Some(SHIRT));
            //huldra.set_weapon(HandType::MainHand, BAD_SWORD);
            huldra.health.change_max_value_to(40);
            huldra
        }
        CharacterType::Enslaved => {
            let enslaved = Character::new(
                bot(BotBehaviour::Fighter(Default::default()), 12.0),
                "Enslaved",
                PortraitId::Ghoul,
                SpriteId::Skeleton2,
                Attributes::new(5, 5, 2, 1),
                pos,
            );
            enslaved.health.change_max_value_to(24);
            enslaved.armor_piece.set(Some(CHAIN_MAIL));
            enslaved.set_weapon(HandType::MainHand, ENSLAVED_SWORD);
            enslaved
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

pub fn make_low_level_party() -> (Rc<Party>, Vec<Character>) {
    let party = Rc::new(Party {
        money: Cell::new(8),
        stash: Default::default(),
    });

    let mut alice = Character::new(
        CharacterKind::Player(Rc::clone(&party), PlayerId::Alice),
        "Alice",
        PortraitId::Alice,
        SpriteId::Alice,
        Attributes::new(3, 5, 3, 3),
        (1, 10),
    );
    alice.set_weapon(HandType::MainHand, BOW);

    // TODO
    /*
    alice.mana.change_max_value_to(20);

    alice.set_weapon(HandType::MainHand, CHEAT_BOW);
    alice.learn_ability(KILL);
     */

    alice.armor_piece.set(Some(SHIRT));
    alice.learn_ability(INSPIRE);

    let bob = Character::new(
        CharacterKind::Player(Rc::clone(&party), PlayerId::Bob),
        "Bob",
        PortraitId::Bob,
        SpriteId::Bob,
        Attributes::new(5, 3, 3, 1),
        (2, 10),
    );
    bob.set_weapon(HandType::MainHand, SWORD);
    bob.set_shield(SMALL_SHIELD);
    bob.armor_piece.set(Some(SHIRT));
    bob.learn_ability(SHIELD_BASH);
    bob.learn_ability_enhancement(SHIELD_BASH_KNOCKBACK);

    //bob.learn_ability(KILL);

    let player_characters = vec![alice, bob];

    (party, player_characters)
}

pub fn make_medium_clara(party: &Rc<Party>) -> Character {
    let mut clara = Character::new(
        CharacterKind::Player(Rc::clone(party), PlayerId::Clara),
        "Clara",
        PortraitId::Clara,
        SpriteId::Clara,
        Attributes::new(2, 2, 3, 7),
        (3, 10),
    );
    clara.set_weapon(HandType::MainHand, DAGGER);
    // TODO:
    clara.armor_piece.set(Some(SHIRT));
    clara.learn_ability(FIREBALL);
    clara.learn_ability_enhancement(FIREBALL_REACH);
    clara.learn_ability(SHACKLED_MIND);
    clara
}

pub fn make_high_bob(party: &Rc<Party>) -> Character {
    let mut char = Character::new(
        CharacterKind::Player(Rc::clone(party), PlayerId::Bob),
        "Bob",
        PortraitId::Bob,
        SpriteId::Bob,
        Attributes::new(5, 3, 3, 3),
        (2, 10),
    );
    char.set_weapon(HandType::MainHand, SWORD);
    char.set_shield(MEDIUM_SHIELD);
    char.armor_piece.set(Some(LEATHER_ARMOR));
    char.learn_passive(PassiveSkill::Reaper);
    char.learn_ability(SWEEP_ATTACK);
    char.learn_ability(SHIELD_BASH);
    char.learn_ability_enhancement(SHIELD_BASH_KNOCKBACK);
    char.learn_ability(INSPIRE);
    char.learn_attack_enhancement(SMITE);
    //bob.learn_attack_enhancement(EMPOWER);
    char.try_gain_equipment(EquipmentEntry::Consumable(HEALTH_POTION));
    char
}

pub fn make_high_alice(party: &Rc<Party>) -> Character {
    let mut char = Character::new(
        CharacterKind::Player(Rc::clone(party), PlayerId::Alice),
        "Alice",
        PortraitId::Alice,
        SpriteId::Alice,
        Attributes::new(3, 5, 3, 3),
        (1, 10),
    );
    char.set_weapon(HandType::MainHand, BOW);
    char.armor_piece.set(Some(SHIRT));
    char.arrows.set(Some(ArrowStack::new(EXPLODING_ARROWS, 3)));
    char.learn_ability(HEAL);
    char.learn_ability_enhancement(HEAL_ENERGIZE);
    char.learn_attack_enhancement(CRIPPLING_SHOT);
    char.learn_passive(PassiveSkill::WeaponProficiency);
    char.learn_ability(PIERCING_SHOT);
    char
}

pub fn make_high_level_party() -> (Rc<Party>, Vec<Character>) {
    let party = Rc::new(Party {
        money: Cell::new(8),
        stash: Default::default(),
    });

    let alice = make_high_alice(&party);

    let bob = make_high_bob(&party);

    let mut clara = Character::new(
        CharacterKind::Player(Rc::clone(&party), PlayerId::Clara),
        "Clara",
        PortraitId::Clara,
        SpriteId::Clara,
        Attributes::new(2, 2, 3, 7),
        (3, 10),
    );
    clara.set_weapon(HandType::MainHand, DAGGER);
    // TODO:
    clara.armor_piece.set(Some(SHIRT));
    clara.learn_passive(PassiveSkill::CriticalCharge);
    clara.learn_ability(FIREBALL);
    clara.learn_ability_enhancement(FIREBALL_INFERNO);
    clara.learn_ability_enhancement(FIREBALL_REACH);
    clara.learn_ability_enhancement(FIREBALL_MASSIVE);
    clara.learn_ability(SHACKLED_MIND);
    clara.learn_ability(INFLICT_WOUNDS);
    clara.learn_ability_enhancement(INFLICT_WOUNDS_NECROTIC_INFLUENCE);
    //clara.learn_ability(MIND_BLAST);

    clara.try_gain_equipment(EquipmentEntry::Consumable(MANA_POTION));
    clara.try_gain_equipment(EquipmentEntry::Consumable(ARCANE_POTION));

    let player_characters = vec![clara, alice, bob];

    (party, player_characters)
}
