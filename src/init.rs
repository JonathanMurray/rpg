use std::{
    collections::{HashMap, HashSet},
    fs,
    rc::Rc,
};

use rand::distr::{Distribution, Uniform};

use crate::{
    core::{
        Attributes, BaseAction, Character, CharacterId, Characters, EquipmentEntry, HandType,
        Position,
    },
    data::{
        BOW, CHAIN_MAIL, DAGGER, FIREBALL, HEALING_NOVA, HEALING_RAIN, KILL, LEATHER_ARMOR, LUNGE_ATTACK, MIND_BLAST, OVERWHELMING, RAGE, RAPIER, ROBE, SHIRT, SIDE_STEP, SMALL_SHIELD, SWEEP_ATTACK, SWORD
    },
    pathfind::PathfindGrid,
    textures::{PortraitId, SpriteId, TerrainId},
};

pub fn init() -> GameInitState {
    let active_character_id = 0;

    let mut bob = Character::new(
        true,
        "Bob",
        PortraitId::Portrait2,
        SpriteId::Character5,
        Attributes::new(3, 3, 5, 5),
        (1, 8),
    );
    bob.set_weapon(HandType::MainHand, SWORD);
    bob.set_weapon(HandType::MainHand, BOW);
    bob.armor.set(Some(SHIRT));

    bob.known_actions.push(BaseAction::CastSpell(MIND_BLAST));
    bob.known_actions.push(BaseAction::CastSpell(FIREBALL));
    bob.known_actions.push(BaseAction::CastSpell(HEALING_RAIN));
    bob.known_actions.push(BaseAction::CastSpell(HEALING_NOVA));
    //bob.known_actions.push(BaseAction::CastSpell(HEAL));
    //bob.known_actions.push(BaseAction::CastSpell(SHACKLED_MIND));

    let mut alice = Character::new(
        true,
        "Alice",
        PortraitId::Portrait1,
        SpriteId::Character4,
        Attributes::new(5, 5, 5, 1),
        (1, 10),
    );
    alice.known_actions.push(BaseAction::CastSpell(KILL)); //TODO
    alice.known_attack_enhancements.push(OVERWHELMING);
    alice.known_attacked_reactions.push(SIDE_STEP);
    //alice.known_attack_enhancements.push(QUICK);
    //alice.known_attack_enhancements.push(SMITE);
    alice.known_on_hit_reactions.push(RAGE);
    alice
        .known_actions
        .push(BaseAction::CastSpell(SWEEP_ATTACK));
    alice
        .known_actions
        .push(BaseAction::CastSpell(LUNGE_ATTACK));
    alice.armor.set(Some(ROBE));
    alice.set_weapon(HandType::MainHand, BOW);
    alice.inventory[0].set(Some(EquipmentEntry::Weapon(SWORD)));
    //alice.set_shield(SMALL_SHIELD);

    let skeleton1 = Character::new(
        true,
        "Skeleton",
        PortraitId::Portrait3,
        SpriteId::Character3,
        Attributes::new(4, 2, 1, 1),
        (5, 7),
    );
    skeleton1.armor.set(Some(CHAIN_MAIL));
    skeleton1.set_weapon(HandType::MainHand, BOW);

    let skeleton2 = Character::new(
        false,
        "Skeleton",
        PortraitId::Skeleton,
        SpriteId::Character2,
        Attributes::new(2, 2, 1, 1),
        (7, 7),
    );
    skeleton2.set_weapon(HandType::MainHand, DAGGER);
    skeleton2.set_shield(SMALL_SHIELD);

    let characters = vec![alice, bob, skeleton1, skeleton2];

    let map_str = fs::read_to_string("map.txt").unwrap();

    dbg!(&map_str);

    let mut terrain_objects: HashMap<Position, TerrainId> = Default::default();

    let mut water_grid: HashSet<Position> = Default::default();

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
                ' ' => {}
                _ => panic!("Unhandled map object: {}", ch),
            }
        }
        row += 1;
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
