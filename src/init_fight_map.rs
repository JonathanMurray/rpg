use std::{collections::HashMap, rc::Rc, str::FromStr};

use clap::ValueEnum;

use indexmap::IndexMap;

use crate::{
    core::{Character, CharacterId, PlayerId, Position},
    grid::ControlPoint,
    map_data::{create_character, CharacterType, MapData},
    pathfind::{Elevation, Occupation, PathfindGrid, Surface},
    resources::TerrainId,
};

pub fn init_fight_map(player_characters: Vec<Rc<Character>>, fight_id: FightId) -> GameInitState {
    let mut player_chars_by_id: HashMap<PlayerId, Rc<Character>> = player_characters
        .into_iter()
        .map(|ch| (ch.player_id(), ch))
        .collect();
    let filename = match fight_id {
        FightId::Easy => "easy_map.json",
        FightId::Medium => "medium_map.json",
        FightId::Huldra => "huldra.json",
        FightId::Draug => "draug2.json",
        FightId::Ogre => "ogre_room.json",
        FightId::Test => "test.json",
        FightId::Graveyard => "graveyard.json",
    };
    let map_data = MapData::load_from_file(&format!("maps/{filename}"));
    let mut characters: Vec<Rc<Character>> = Default::default();
    let pathfind_grid = Rc::new(PathfindGrid::new(map_data.grid_dimensions));

    let mut active_char_id = None;

    for (i, char_data) in map_data.characters.iter().enumerate() {
        let pos = char_data.pos;
        let char: Option<Rc<Character>> = match char_data.type_ {
            CharacterType::Bob => player_chars_by_id.remove(&PlayerId::Bob).inspect(|ch| {
                ch.set_id(i as CharacterId);
                ch.position.set(pos);
            }),
            CharacterType::Alice => player_chars_by_id.remove(&PlayerId::Alice).inspect(|ch| {
                ch.set_id(i as CharacterId);
                ch.position.set(pos);
            }),
            CharacterType::Clara => player_chars_by_id.remove(&PlayerId::Clara).inspect(|ch| {
                ch.set_id(i as CharacterId);
                ch.position.set(pos);
            }),
            _ => Some(create_character(pos, &char_data, None, i as CharacterId)),
        };

        if let Some(char) = char {
            if active_char_id.is_none() && !char.is_dead() && char.player_controlled() {
                active_char_id = Some(char.id());
            }
            pathfind_grid.set_occupied(pos, Occupation::Character(char.id()));
            characters.push(char);
        }
    }

    assert_eq!(
        player_chars_by_id.len(),
        0,
        "Unassigned player characters: {:?}",
        player_chars_by_id.keys()
    );

    for (pos, terrain_id) in &map_data.surfaces {
        if let Some((surface, size)) = terrain_id.surface() {
            pathfind_grid.set_surface(*pos, Some(surface), size);
        }
    }

    for (pos, terrain_id) in map_data.terrain_objects.iter() {
        pathfind_grid.set_occupied(*pos, Occupation::Terrain(*terrain_id));
    }

    for (pos, control_point) in &map_data.control_points {
        pathfind_grid
            .control_points
            .borrow_mut()
            .insert(*pos, *control_point);
    }

    for (pos, elevation) in &map_data.elevation {
        pathfind_grid.set_elevation_center(*pos, *elevation);
    }

    GameInitState {
        characters,
        active_character_id: active_char_id.unwrap(),
        pathfind_grid,
        background: map_data.background,
        terrain_objects: map_data.terrain_objects,
        decorations: map_data.decorations,
        surfaces: map_data.surfaces,
        control_points: map_data.control_points,
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
    pub surfaces: IndexMap<Position, TerrainId>,
    pub control_points: IndexMap<Position, ControlPoint>,
}

#[derive(Debug, Copy, Clone, PartialEq, ValueEnum)]
pub enum FightId {
    Medium,
    Easy,
    Huldra,
    Draug,
    Test,
    Ogre,
    Graveyard,
}
