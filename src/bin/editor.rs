use std::cell::{self, Cell, RefCell};
use std::char::CharTryFromError;
use std::collections::HashMap;
use std::fs;
use std::rc::{Rc, Weak};
use std::sync::atomic::Ordering;

use indexmap::IndexMap;
use macroquad::color::{Color, BLACK, LIGHTGRAY, MAGENTA, RED, WHITE, YELLOW};
use macroquad::input::{
    is_key_down, is_key_pressed, is_mouse_button_down, is_mouse_button_pressed, mouse_position,
    mouse_position_local, KeyCode, MouseButton,
};
use macroquad::math::Rect;
use macroquad::miniquad::window::set_window_position;
use macroquad::shapes::{draw_rectangle, draw_rectangle_lines};
use macroquad::text::{
    self, draw_text, draw_text_ex, load_ttf_font, measure_text, Font, TextDimensions, TextParams,
};
use macroquad::texture::{draw_texture, draw_texture_ex, DrawTextureParams, FilterMode, Texture2D};
use macroquad::window::{clear_background, screen_width, Conf};
use macroquad::window::{next_frame, screen_height};

use rpg::base_ui::{
    draw_text_rounded, draw_text_with_font_tags, Align, Checkbox, Container, Drawable, Element,
    LayoutDirection, Style, TextLine,
};
use rpg::bot::BotBehaviour;
use rpg::core::{
    Ability, Action, ArrowStack, AttackEnhancement, Attributes, BaseAction, Bot, Character,
    CharacterId, CharacterKind, Characters, Condition, CoreGame, EquipmentEntry, HandType,
    OnAttackedReaction, OnHitReaction, Party, Position, Shield, Weapon,
};

use rpg::data::{
    PassiveSkill, BAD_BOW, BAD_DAGGER, BAD_RAPIER, BAD_SMALL_SHIELD, BAD_SWORD, BAD_WAR_HAMMER,
    ENEMY_BRACE, ENEMY_INSPIRE, ENEMY_TACKLE, GOOD_CHAIN_MAIL, LEATHER_ARMOR, SHIRT, SWORD,
};
use rpg::game_ui::{PlayerChose, UiState, UserInterface};
use rpg::game_ui_connection::{QuitEvent, QUIT_WITH_ESCAPE};
use rpg::grid::GameGrid;
use rpg::init_fight_map::{init_fight_map, FightId, GameInitState};
use rpg::map_scene::{MapChoice, MapScene};
use rpg::pathfind::{Occupation, PathfindGrid};
use rpg::resources::{init_core_game, GameResources, UiResources};
use rpg::sounds::SoundPlayer;
use rpg::textures::{
    draw_terrain, load_and_init_font_symbols, load_and_init_texture, load_and_init_ui_textures,
    terrain_atlas_area, EquipmentIconId, IconId, PortraitId, SpriteId, StatusId, TerrainId,
};
use serde::{Deserialize, Serialize};

const SAVE_FILE_NAME: &str = "testsavefile.json";

#[macroquad::main(window_conf)]
async fn main() {
    // Without this, the window seems to start on a random position on the screen, sometimes with the bottom obscured
    set_window_position(100, 100);

    let resources = GameResources::load().await;
    let ui_resources = UiResources::load().await;
    load_and_init_font_symbols().await;
    load_and_init_ui_textures().await;

    let mut map_data = MapData::load_from_file(SAVE_FILE_NAME);

    let sound_player = SoundPlayer::new().await;

    let party = Rc::new(Party {
        money: Cell::new(8),
        stash: Default::default(),
    });

    let mut game_grid = create_game_grid(&map_data, sound_player.clone(), &resources, &party);

    let mut sidebar = Sidebar::new(resources.terrain_atlas.clone(), resources.sprites.clone());
    let show_grid = Rc::new(Cell::new(false));
    let snap_to_grid = Rc::new(Cell::new(false));
    let settings = build_settings(
        &resources.big_font,
        &resources.simple_font,
        sound_player.clone(),
        Rc::clone(&show_grid),
        Rc::clone(&snap_to_grid),
    );

    let file_text = format!("{:?}", SAVE_FILE_NAME);
    let mut has_unsaved_changes = false;

    let mut inspect_target = None;

    let mut character_editor: Option<Container> = None;

    loop {
        next_frame().await;
        let grid_outcome = game_grid.draw(true, &mut UiState::Idle, false, None, (0, 0));
        if show_grid.get() {
            game_grid.draw_debug_cells();
        }

        if let Some(new_inspect_target) = grid_outcome.switched_inspect_target {
            inspect_target = new_inspect_target;
            if let Some(id) = inspect_target {
                let char = game_grid.characters.get(&id).unwrap();
                character_editor = Some(build_character_editor(
                    &resources.big_font,
                    &resources.simple_font,
                    char,
                    &ui_resources.equipment_icons,
                ));
            } else {
                character_editor = None;
            }
        }

        sidebar.draw();
        settings.draw(0.0, screen_height() - settings.size().1);

        let mut t = file_text.clone();
        if has_unsaved_changes {
            t.push_str(" (*)");
        }
        draw_text_ex(
            &t,
            screen_width() / 2.0,
            20.0,
            TextParams {
                font: Some(&resources.simple_font),
                font_size: 32,
                color: WHITE,
                ..Default::default()
            },
        );

        let mut mouse_grid_pos = game_grid.mouse_grid_pos();

        if snap_to_grid.get() {
            // Snap to 'large grid' (3x3 squares)
            mouse_grid_pos = (
                (mouse_grid_pos.0 / 3) * 3 + 1,
                (mouse_grid_pos.1 / 3) * 3 + 1,
            );
        }

        let snapped_mouse_screen_pos = game_grid.grid_pos_to_screen(mouse_grid_pos);

        let settings_hovered = settings
            .last_drawn_rectangle
            .get()
            .contains(mouse_position().into());
        let char_editor_hovered = character_editor
            .as_ref()
            .map(|container| {
                container
                    .last_drawn_rectangle
                    .get()
                    .contains(mouse_position().into())
            })
            .unwrap_or(false);

        if !sidebar.hovered && !settings_hovered && !char_editor_hovered {
            if let Some(action) = sidebar.action() {
                let entity_size = game_grid.entity_draw_size();

                let rect = Rect::new(
                    snapped_mouse_screen_pos.0 - game_grid.cell_w,
                    snapped_mouse_screen_pos.1 - game_grid.cell_w,
                    entity_size.0,
                    entity_size.1,
                );

                match action {
                    EditorAction::PlaceBackground(terrain_id) => {
                        draw_rectangle(
                            rect.x,
                            rect.y,
                            rect.w,
                            rect.h,
                            Color::new(0.0, 0.0, 0.0, 0.3),
                        );
                        game_grid.draw_terrain(
                            *terrain_id,
                            snapped_mouse_screen_pos.0,
                            snapped_mouse_screen_pos.1,
                        );
                    }
                    EditorAction::PlaceTerrain(terrain_id) => {
                        draw_rectangle(
                            rect.x,
                            rect.y,
                            rect.w,
                            rect.h,
                            Color::new(0.0, 0.0, 0.0, 0.3),
                        );
                        game_grid.draw_terrain(
                            *terrain_id,
                            snapped_mouse_screen_pos.0,
                            snapped_mouse_screen_pos.1,
                        );
                    }

                    EditorAction::PlaceDecoration(terrain_id) => {
                        draw_rectangle(
                            rect.x,
                            rect.y,
                            rect.w,
                            rect.h,
                            Color::new(0.0, 0.0, 0.0, 0.3),
                        );
                        game_grid.draw_terrain(
                            *terrain_id,
                            snapped_mouse_screen_pos.0,
                            snapped_mouse_screen_pos.1,
                        );
                    }

                    EditorAction::PlaceCharacter(id) => {
                        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 3.0, YELLOW);
                        draw_text(
                            &format!("{:?}", id),
                            snapped_mouse_screen_pos.0,
                            snapped_mouse_screen_pos.1,
                            16.0,
                            WHITE,
                        );
                    }
                    EditorAction::EraseTerrain => {
                        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 3.0, RED);
                        draw_text(
                            "Erase",
                            snapped_mouse_screen_pos.0,
                            snapped_mouse_screen_pos.1,
                            16.0,
                            WHITE,
                        );
                    }
                    EditorAction::EraseBackground => {
                        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 3.0, RED);
                        draw_text(
                            "Erase",
                            snapped_mouse_screen_pos.0,
                            snapped_mouse_screen_pos.1,
                            16.0,
                            WHITE,
                        );
                    }
                    EditorAction::EraseDecoration => {
                        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 3.0, RED);
                        draw_text(
                            "Erase",
                            snapped_mouse_screen_pos.0,
                            snapped_mouse_screen_pos.1,
                            16.0,
                            WHITE,
                        );
                    }
                    EditorAction::EditCharacter => {
                        draw_text(
                            "Edit",
                            snapped_mouse_screen_pos.0,
                            snapped_mouse_screen_pos.1,
                            16.0,
                            WHITE,
                        );
                    }
                    EditorAction::MoveCharacter(maybe_id) => match maybe_id.get() {
                        Some(_) => {
                            draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 3.0, YELLOW);
                            draw_text(
                                "Move",
                                snapped_mouse_screen_pos.0,
                                snapped_mouse_screen_pos.1,
                                16.0,
                                WHITE,
                            );
                        }
                        None => {
                            draw_text(
                                "Select",
                                snapped_mouse_screen_pos.0,
                                snapped_mouse_screen_pos.1,
                                16.0,
                                WHITE,
                            );
                        }
                    },
                    EditorAction::RemoveCharacter => {
                        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 3.0, RED);
                        draw_text(
                            "Remove",
                            snapped_mouse_screen_pos.0,
                            snapped_mouse_screen_pos.1,
                            16.0,
                            WHITE,
                        );
                    }
                }

                if is_mouse_button_down(MouseButton::Left) {
                    let pos = mouse_grid_pos;
                    match action {
                        EditorAction::PlaceBackground(terrain_id) => {
                            game_grid.background.insert(pos, *terrain_id);
                            map_data.background.insert(pos, *terrain_id);
                            has_unsaved_changes = true;
                        }
                        EditorAction::PlaceTerrain(terrain_id) => {
                            if game_grid.editor_add_terrain(pos, *terrain_id) {
                                assert_eq!(map_data.terrain_objects.get(&pos), None);
                                map_data.terrain_objects.insert(pos, *terrain_id);
                                has_unsaved_changes = true;
                            }
                        }
                        EditorAction::PlaceDecoration(terrain_id) => {
                            if !game_grid.decorations.contains_key(&pos) {
                                game_grid.decorations.insert(pos, *terrain_id);
                                map_data.decorations.insert(pos, *terrain_id);
                                has_unsaved_changes = true;
                            }
                        }
                        EditorAction::PlaceCharacter(char_type) => {
                            if game_grid.pathfind_grid.is_free(None, pos) {
                                let max_id =
                                    game_grid.characters.keys().copied().max().unwrap_or(0);
                                let char_data = CharacterData::base(*char_type, pos);
                                let char = create_character(pos, char_data, &party, max_id + 1);
                                game_grid
                                    .pathfind_grid
                                    .set_occupied(pos, Some(Occupation::Character(char.id())));
                                game_grid.editor_add_character(char.id(), Rc::clone(&char));
                                map_data.characters.push(char_data);
                                has_unsaved_changes = true;
                            }
                        }
                        EditorAction::EraseBackground => {
                            if game_grid.background.contains_key(&mouse_grid_pos) {
                                game_grid.background.swap_remove(&mouse_grid_pos);
                                map_data.background.swap_remove(&pos);
                                has_unsaved_changes = true;
                            }
                        }
                        EditorAction::EraseTerrain => {
                            if game_grid.terrain_objects.contains_key(&mouse_grid_pos) {
                                game_grid.pathfind_grid.set_occupied(pos, None);
                                game_grid.terrain_objects.swap_remove(&mouse_grid_pos);
                                game_grid.auto_tile_terrain_objects();
                                map_data.terrain_objects.swap_remove(&pos);
                                has_unsaved_changes = true;
                            }
                        }
                        EditorAction::EraseDecoration => {
                            if game_grid.decorations.contains_key(&mouse_grid_pos) {
                                game_grid.decorations.swap_remove(&mouse_grid_pos);
                                map_data.decorations.swap_remove(&pos);
                                has_unsaved_changes = true;
                            }
                        }
                        EditorAction::EditCharacter => {
                            // It's taken care of by tracking the game grid's inspect target
                        }
                        EditorAction::MoveCharacter(maybe_id) => {
                            if let Some(char_id) = maybe_id.get() {
                                let char = game_grid.characters.get(&char_id).unwrap();
                                if game_grid.pathfind_grid.is_free(Some(char.id()), pos) {
                                    game_grid.pathfind_grid.set_occupied(char.pos(), None);

                                    let char_data = map_data
                                        .characters
                                        .iter_mut()
                                        .find(|ch| ch.pos == char.pos())
                                        .unwrap();
                                    char_data.pos = pos;

                                    char.position.set(pos);
                                    game_grid
                                        .pathfind_grid
                                        .set_occupied(pos, Some(Occupation::Character(char.id())));
                                    has_unsaved_changes = true;
                                }
                            }

                            maybe_id.set(inspect_target);
                        }
                        EditorAction::RemoveCharacter => {
                            let char_id = game_grid
                                .characters
                                .values()
                                .find(|ch| ch.pos() == pos)
                                .map(|ch| ch.id());
                            if let Some(id) = char_id {
                                game_grid.editor_remove_character(id);

                                map_data.characters.retain(|ch| ch.pos != pos);
                                has_unsaved_changes = true;
                            }
                        }
                    }
                }
            }
        }

        if let Some(character_editor) = &character_editor {
            if sidebar.action() == Some(&EditorAction::EditCharacter) {
                character_editor.draw(
                    screen_width() / 2.0,
                    screen_height() - character_editor.size().1 - 10.0,
                );
            }
        }

        if is_key_pressed(KeyCode::Space) {
            println!("Running game ...");
            QUIT_WITH_ESCAPE.store(true, Ordering::SeqCst);
            let init_state = GameInitState {
                characters: game_grid.characters.values().cloned().collect(),
                active_character_id: 0,
                pathfind_grid: game_grid.pathfind_grid.clone(),
                background: game_grid.background.clone(),
                terrain_objects: game_grid.terrain_objects.clone(),
                decorations: game_grid.decorations.clone(),
            };
            let core_game = init_core_game(
                resources.clone(),
                ui_resources.clone(),
                sound_player.clone(),
                init_state,
            );
            match core_game.run().await {
                Ok(_chars) => println!("Game ended naturally"),
                Err(QuitEvent) => println!("User quit from game"),
            }

            // Restore the original map state, so that we can keep editing
            game_grid = create_game_grid(&map_data, sound_player.clone(), &resources, &party);
        }

        if is_key_down(KeyCode::LeftControl) && is_key_pressed(KeyCode::S) {
            map_data.save_to_file(SAVE_FILE_NAME);
            has_unsaved_changes = false;
        }
    }
}

fn create_game_grid(
    map_data: &MapData,
    sound_player: SoundPlayer,
    resources: &GameResources,
    party: &Rc<Party>,
) -> GameGrid {
    let mut characters: IndexMap<CharacterId, Rc<Character>> = Default::default();

    let pathfind_grid = Rc::new(PathfindGrid::new(map_data.grid_dimensions));

    for (i, char_data) in map_data.characters.iter().enumerate() {
        let pos = char_data.pos;
        let char = create_character(pos, *char_data, &party, i as CharacterId);
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
        sound_player,
    );

    game_grid.auto_tile_terrain_objects();

    game_grid
}

fn create_character(
    pos: Position,
    char_data: CharacterData,
    party: &Rc<Party>,
    id: CharacterId,
) -> Rc<Character> {
    let char = match char_data.type_ {
        CharacterType::Bob => {
            let bob = Character::new(
                CharacterKind::Player(Rc::clone(&party)),
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
                CharacterKind::Player(Rc::clone(&party)),
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
                CharacterKind::Player(Rc::clone(&party)),
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
                Attributes::new(4, 4, 3, 1),
                pos,
            );
            skeleton.health.change_max_value_to(35);
            skeleton.armor_piece.set(Some(LEATHER_ARMOR));
            skeleton.set_weapon(HandType::MainHand, BAD_RAPIER);
            skeleton.set_shield(BAD_SMALL_SHIELD);
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
                Attributes::new(1, 3, 2, 1),
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
                Attributes::new(2, 2, 1, 1),
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
                Attributes::new(12, 4, 3, 1),
                pos,
            );
            ogre.health.change_max_value_to(56);
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
    }

    /*
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

    * */

    char.set_id(id);
    Rc::new(char)
}

fn bot(behaviour: BotBehaviour, move_speed: f32) -> CharacterKind {
    CharacterKind::Bot(Bot {
        behaviour,
        base_movement: move_speed,
    })
}

struct Sidebar {
    terrain_atlas: Texture2D,
    sprites: HashMap<SpriteId, Texture2D>,
    selected_section_idx: usize,
    selected_action_idx: Option<usize>,
    hovered: bool,
    sections: Vec<Vec<EditorAction>>,
}

#[derive(Clone, PartialEq)]
enum EditorAction {
    PlaceBackground(TerrainId),
    PlaceTerrain(TerrainId),
    PlaceDecoration(TerrainId),
    PlaceCharacter(CharacterType),
    EraseBackground,
    EraseTerrain,
    EraseDecoration,
    EditCharacter,
    MoveCharacter(Cell<Option<CharacterId>>),
    RemoveCharacter,
}

impl Sidebar {
    fn new(terrain_atlas: Texture2D, sprites: HashMap<SpriteId, Texture2D>) -> Self {
        let backgrounds = vec![TerrainId::Floor, TerrainId::Floor2, TerrainId::Floor3];

        let mut background_actions: Vec<EditorAction> = backgrounds
            .iter()
            .map(|t| EditorAction::PlaceBackground(*t))
            .collect();
        background_actions.push(EditorAction::EraseBackground);

        let terrain_ids = vec![
            TerrainId::Bush,
            TerrainId::Boulder2,
            TerrainId::TreeStump,
            TerrainId::Table,
            TerrainId::NewWaterNorthEast,
            TerrainId::StoneWallConvexNorthEast,
        ];
        let mut terrain_actions: Vec<EditorAction> = terrain_ids
            .iter()
            .map(|t| EditorAction::PlaceTerrain(*t))
            .collect();
        terrain_actions.push(EditorAction::EraseTerrain);

        let decorations = vec![
            TerrainId::BookShelf,
            TerrainId::WallPainting,
            TerrainId::WallFlag,
            TerrainId::Mat,
        ];
        let mut decoration_actions: Vec<EditorAction> = decorations
            .iter()
            .map(|t| EditorAction::PlaceDecoration(*t))
            .collect();
        decoration_actions.push(EditorAction::EraseDecoration);

        let character_actions = vec![
            EditorAction::PlaceCharacter(CharacterType::Bob),
            EditorAction::PlaceCharacter(CharacterType::Alice),
            EditorAction::PlaceCharacter(CharacterType::Clara),
            EditorAction::PlaceCharacter(CharacterType::Skeleton),
            EditorAction::PlaceCharacter(CharacterType::Ghoul1),
            EditorAction::PlaceCharacter(CharacterType::Ghoul2),
            EditorAction::PlaceCharacter(CharacterType::Ogre),
            EditorAction::EditCharacter,
            EditorAction::MoveCharacter(Cell::new(None)),
            EditorAction::RemoveCharacter,
        ];

        let sections = vec![
            background_actions,
            terrain_actions,
            decoration_actions,
            character_actions,
        ];

        Self {
            terrain_atlas,
            sprites,
            sections,
            selected_section_idx: 0,
            selected_action_idx: None,
            hovered: false,
        }
    }

    fn draw(&mut self) {
        self.hovered = false;
        let mut section_y = 0.0;
        for (section_i, actions) in self.sections.iter().enumerate() {
            if let Some(i) = self.selected_action_idx {
                if is_key_pressed(KeyCode::Left) {
                    self.selected_action_idx =
                        Some(((i as i32 - 1).rem_euclid(actions.len() as i32)) as usize);
                } else if is_key_pressed(KeyCode::Right) {
                    self.selected_action_idx =
                        Some(((i as i32 + 1).rem_euclid(actions.len() as i32)) as usize);
                }
            }

            let cols = 3;
            let rows = 1 + actions.len() / cols;
            let margin = 4.0;
            let pad = 10.0;
            let icon_w = 64.0;
            let section_w = pad * 2.0 + cols as f32 * icon_w + (cols - 1) as f32 * margin;
            let section_h = pad * 2.0 + rows as f32 * icon_w + (rows - 1) as f32 * margin;

            let mouse_pos = mouse_position();
            let section_hovered =
                Rect::new(0.0, section_y, section_w, section_h).contains(mouse_pos.into());
            self.hovered |= section_hovered;

            draw_rectangle(
                0.0,
                section_y,
                section_w,
                section_h,
                Color::new(0.0, 0.0, 0.0, 0.5),
            );

            for (action_i, action) in actions.iter().enumerate() {
                let col = action_i % cols;
                let x = pad + col as f32 * (icon_w + margin);
                let row = action_i / cols;
                let y = section_y + pad + row as f32 * (icon_w + margin);
                if is_mouse_button_pressed(MouseButton::Left)
                    && Rect::new(x, y, icon_w, icon_w).contains(mouse_pos.into())
                {
                    self.selected_section_idx = section_i;
                    self.selected_action_idx = Some(action_i);
                }
                let bg = if self.selected_section_idx == section_i
                    && self.selected_action_idx == Some(action_i)
                {
                    YELLOW
                } else {
                    WHITE
                };
                draw_rectangle(x, y, icon_w, icon_w, bg);
                match action {
                    EditorAction::PlaceBackground(terrain_id) => {
                        self.draw_terrain_icon(icon_w, x, y, terrain_id);
                    }
                    EditorAction::PlaceTerrain(terrain_id) => {
                        self.draw_terrain_icon(icon_w, x, y, terrain_id);
                    }

                    EditorAction::PlaceDecoration(terrain_id) => {
                        self.draw_terrain_icon(icon_w, x, y, terrain_id);
                    }
                    EditorAction::PlaceCharacter(char_type) => {
                        draw_texture_ex(
                            &self.sprites[&char_type.sprite_id()],
                            x,
                            y,
                            WHITE,
                            DrawTextureParams {
                                dest_size: Some((icon_w, icon_w).into()),
                                ..Default::default()
                            },
                        );
                        let text = format!("{:?}", char_type);
                        let font_size = 16;
                        let text_dim = measure_text(&text, None, font_size, 1.0);
                        draw_rectangle(
                            x + icon_w / 2.0 - text_dim.width / 2.0,
                            y + icon_w / 2.0 - text_dim.offset_y,
                            text_dim.width,
                            text_dim.height,
                            WHITE,
                        );
                        draw_text(
                            &text,
                            x + icon_w / 2.0 - text_dim.width / 2.0,
                            y + icon_w / 2.0,
                            font_size as f32,
                            BLACK,
                        );
                    }
                    EditorAction::EraseBackground => {
                        draw_text("ERASE", x + 5.0, y + icon_w / 2.0, 16.0, BLACK);
                    }
                    EditorAction::EraseTerrain => {
                        draw_text("ERASE", x + 5.0, y + icon_w / 2.0, 16.0, BLACK);
                    }
                    EditorAction::EraseDecoration => {
                        draw_text("ERASE", x + 5.0, y + icon_w / 2.0, 16.0, BLACK);
                    }
                    EditorAction::EditCharacter => {
                        draw_text("EDIT", x + 5.0, y + icon_w / 2.0, 16.0, BLACK);
                    }
                    EditorAction::MoveCharacter { .. } => {
                        draw_text("MOVE", x + 5.0, y + icon_w / 2.0, 16.0, BLACK);
                    }
                    EditorAction::RemoveCharacter => {
                        draw_text("REMOVE", x + 5.0, y + icon_w / 2.0, 16.0, BLACK);
                    }
                }

                if self.selected_section_idx == section_i
                    && self.selected_action_idx == Some(action_i)
                {
                    draw_rectangle_lines(x + 1.0, y + 1.0, icon_w - 2.0, icon_w - 2.0, 3.0, BLACK);
                }
            }

            section_y += section_h + 10.0;
        }
    }

    fn draw_terrain_icon(&self, icon_w: f32, x: f32, y: f32, terrain_id: &TerrainId) {
        let (rotation, rect) = terrain_atlas_area(*terrain_id);
        draw_texture_ex(
            &self.terrain_atlas,
            x,
            y,
            WHITE,
            DrawTextureParams {
                dest_size: Some((icon_w, icon_w).into()),
                source: Some(rect),
                rotation,
                ..Default::default()
            },
        );
    }

    fn action(&self) -> Option<&EditorAction> {
        let actions = &self.sections[self.selected_section_idx];
        self.selected_action_idx.map(|i| &actions[i])
    }
}

fn build_character_editor(
    big_font: &Font,
    simple_font: &Font,
    char: &Character,
    equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
) -> Container {
    let weapon = char.weapon(HandType::MainHand);
    let main_hand_element = if let Some(weapon) = weapon {
        Element::Text(TextLine::new(
            weapon.name,
            16,
            WHITE,
            Some(simple_font.clone()),
        ))
    } else {
        Element::Empty(0.0, 0.0)
    };
    let shield = char.shield();
    let off_hand_element = if let Some(shield) = shield {
        Element::Text(TextLine::new(
            shield.name,
            16,
            WHITE,
            Some(simple_font.clone()),
        ))
    } else {
        Element::Empty(0.0, 0.0)
    };
    Container {
        layout_dir: LayoutDirection::Vertical,
        align: Align::Center,
        border_between_children: Some(LIGHTGRAY),
        margin: 40.0,
        style: Style {
            background_color: Some(Color::new(0.0, 0.0, 0.0, 0.7)),
            padding: 15.0,
            ..Default::default()
        },
        children: vec![
            Element::Text(TextLine::new(
                "Edit character",
                12,
                WHITE,
                Some(big_font.clone()),
            )),
            Element::Container(Container {
                layout_dir: LayoutDirection::Vertical,
                align: Align::End,
                margin: 5.0,
                children: vec![
                    Element::Container(Container {
                        layout_dir: LayoutDirection::Horizontal,
                        align: Align::Center,
                        margin: 5.0,
                        children: vec![
                            Element::Text(TextLine::new(
                                "Name:",
                                16,
                                WHITE,
                                Some(simple_font.clone()),
                            )),
                            Element::Text(TextLine::new(
                                char.name,
                                16,
                                WHITE,
                                Some(simple_font.clone()),
                            )),
                        ],
                        ..Default::default()
                    }),
                    Element::Container(Container {
                        layout_dir: LayoutDirection::Horizontal,
                        align: Align::Center,
                        margin: 5.0,
                        children: vec![
                            Element::Text(TextLine::new(
                                "Health:",
                                16,
                                WHITE,
                                Some(simple_font.clone()),
                            )),
                            Element::Text(TextLine::new(
                                format!("{}", char.health.max()),
                                16,
                                WHITE,
                                Some(simple_font.clone()),
                            )),
                        ],
                        ..Default::default()
                    }),
                    Element::Container(Container {
                        layout_dir: LayoutDirection::Horizontal,
                        align: Align::Center,
                        margin: 5.0,
                        children: vec![
                            Element::Text(TextLine::new(
                                "Main-hand:",
                                16,
                                WHITE,
                                Some(simple_font.clone()),
                            )),
                            main_hand_element,
                        ],
                        ..Default::default()
                    }),
                    Element::Container(Container {
                        layout_dir: LayoutDirection::Horizontal,
                        align: Align::Center,
                        margin: 5.0,
                        children: vec![
                            Element::Text(TextLine::new(
                                "Off-hand:",
                                16,
                                WHITE,
                                Some(simple_font.clone()),
                            )),
                            off_hand_element,
                        ],
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            }),
        ],

        ..Default::default()
    }
}

fn build_settings(
    big_font: &Font,
    simple_font: &Font,
    sound_player: SoundPlayer,
    show_grid: Rc<Cell<bool>>,
    snap_to_grid: Rc<Cell<bool>>,
) -> Container {
    Container {
        layout_dir: LayoutDirection::Vertical,
        align: Align::Center,
        border_between_children: Some(LIGHTGRAY),
        margin: 40.0,
        style: Style {
            background_color: Some(Color::new(0.0, 0.0, 0.0, 0.7)),
            padding: 15.0,
            ..Default::default()
        },
        children: vec![
            Element::Container(Container {
                layout_dir: LayoutDirection::Vertical,
                align: Align::End,
                margin: 5.0,
                children: vec![
                    Element::Container(Container {
                        layout_dir: LayoutDirection::Horizontal,
                        align: Align::Center,
                        margin: 5.0,
                        children: vec![
                            Element::Text(TextLine::new(
                                "Snap to grid",
                                16,
                                WHITE,
                                Some(simple_font.clone()),
                            )),
                            Element::Box(Box::new(Checkbox::new(
                                (16.0, 16.0),
                                sound_player.clone(),
                                snap_to_grid,
                            ))),
                        ],
                        ..Default::default()
                    }),
                    Element::Container(Container {
                        layout_dir: LayoutDirection::Horizontal,
                        align: Align::Center,
                        margin: 5.0,
                        children: vec![
                            Element::Text(TextLine::new(
                                "Show grid",
                                16,
                                WHITE,
                                Some(simple_font.clone()),
                            )),
                            Element::Box(Box::new(Checkbox::new(
                                (16.0, 16.0),
                                sound_player.clone(),
                                show_grid,
                            ))),
                        ],
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            }),
            Element::Text(TextLine::new("Settings", 12, WHITE, Some(big_font.clone()))),
        ],

        ..Default::default()
    }
}

#[derive(Debug)]
struct MapData {
    pub grid_dimensions: (u32, u32),
    pub terrain_objects: IndexMap<Position, TerrainId>,
    pub decorations: IndexMap<Position, TerrainId>,
    pub background: IndexMap<Position, TerrainId>,
    pub characters: Vec<CharacterData>,
}

impl MapData {
    fn save_to_file(&self, filename: &str) {
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

    fn load_from_file(filename: &str) -> Self {
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
enum CharacterType {
    Bob,
    Alice,
    Clara,
    Skeleton,
    Ogre,
    Ghoul1,
    Ghoul2,
}

impl CharacterType {
    fn sprite_id(&self) -> SpriteId {
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
struct CharacterData {
    type_: CharacterType,
    pos: Position,
    health: Option<u32>,
    main_hand: Option<WeaponId>,
    shield: Option<ShieldId>,
}

impl CharacterData {
    fn base(type_: CharacterType, pos: Position) -> Self {
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

fn window_conf() -> Conf {
    Conf {
        window_title: "Map Editor".to_owned(),
        window_width: 1600,
        //window_height: 960,
        window_height: 1200,
        high_dpi: true,

        window_resizable: false,
        ..Default::default()
    }
}
