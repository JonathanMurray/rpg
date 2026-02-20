use std::cell::{self, Cell, RefCell};
use std::collections::HashMap;
use std::fs;
use std::rc::{Rc, Weak};
use std::sync::atomic::Ordering;

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
    OnAttackedReaction, OnHitReaction, Party, Position,
};

use rpg::data::{BAD_SWORD, SWORD};
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

    let party = Rc::new(Party {
        money: Cell::new(8),
        stash: Default::default(),
    });

    let mut map_data = MapData::load_from_file(SAVE_FILE_NAME);

    let mut characters = vec![];

    for (pos, map_char_id) in &map_data.characters {
        match map_char_id {
            MapCharacterId::Player1 => {
                let bob = Character::new(
                    CharacterKind::Player(Rc::clone(&party)),
                    "Bob",
                    PortraitId::Bob,
                    SpriteId::Bob,
                    Attributes::new(5, 3, 3, 3),
                    *pos,
                );
                bob.set_weapon(HandType::MainHand, SWORD);
                characters.push(bob);
            }
            MapCharacterId::Enemy1 => {
                let enemy = Character::new(
                    bot(BotBehaviour::Normal, 10.0),
                    "Enemy 1",
                    PortraitId::Skeleton,
                    SpriteId::Skeleton,
                    Attributes::new(5, 3, 3, 3),
                    *pos,
                );
                enemy.set_weapon(HandType::MainHand, BAD_SWORD);
                characters.push(enemy);
            }
        }
    }

    let characters = Characters::new(characters);

    let pathfind_grid = PathfindGrid::new(map_data.grid_dimensions);

    for pos in map_data.terrain_objects.keys().copied() {
        pathfind_grid.set_occupied(pos, Some(Occupation::Terrain));
    }
    for ch in characters.iter() {
        pathfind_grid.set_occupied(ch.pos(), Some(Occupation::Character(ch.id())));
    }

    let pathfind_grid = Rc::new(pathfind_grid);

    let mut init_state = GameInitState {
        characters,
        active_character_id: 0,
        pathfind_grid,
        background: map_data.background.clone(),
        terrain_objects: map_data.terrain_objects.clone(),
    };

    //let fight_id = FightId::VerticalSlice;
    //let mut init_state = init_fight_map(vec![bob], fight_id);

    let sound_player = SoundPlayer::new().await;

    let mut game_grid = GameGrid::new(
        0,
        init_state.characters.clone(),
        resources.sprites.clone(),
        resources.big_font.clone(),
        resources.simple_font.clone(),
        resources.terrain_atlas.clone(),
        init_state.pathfind_grid.clone(),
        init_state.background.clone(),
        init_state.terrain_objects.clone(),
        resources.status_textures.clone(),
        sound_player.clone(),
    );

    game_grid.auto_tile_terrain_objects();

    let mut sidebar = Sidebar::new(resources.terrain_atlas.clone());
    let show_grid = Rc::new(Cell::new(true));
    let snap_to_grid = Rc::new(Cell::new(true));
    let settings = build_settings(
        &resources.big_font,
        &resources.simple_font,
        sound_player.clone(),
        Rc::clone(&show_grid),
        Rc::clone(&snap_to_grid),
    );

    let file_text = format!("{:?}", SAVE_FILE_NAME);
    let mut has_unsaved_changes = false;

    loop {
        next_frame().await;
        game_grid.draw(true, &mut UiState::Idle, false, None, (0, 0));
        if show_grid.get() {
            game_grid.draw_debug_cells();
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

        if !sidebar.hovered && !settings_hovered {
            if let Some(action) = sidebar.action() {
                let entity_size = game_grid.entity_draw_size();

                let rect = Rect::new(
                    snapped_mouse_screen_pos.0 - game_grid.cell_w,
                    snapped_mouse_screen_pos.1 - game_grid.cell_w,
                    entity_size.0,
                    entity_size.1,
                );

                match action {
                    EditorAction::PlaceTerrain(terrain_id) => {
                        draw_rectangle(
                            rect.x,
                            rect.y,
                            rect.w,
                            rect.h,
                            Color::new(0.0, 0.0, 0.0, 0.3),
                        );
                        game_grid.draw_terrain(
                            terrain_id,
                            snapped_mouse_screen_pos.0,
                            snapped_mouse_screen_pos.1,
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
                }

                if is_mouse_button_down(MouseButton::Left) {
                    match action {
                        EditorAction::PlaceTerrain(terrain_id) => {
                            if init_state.try_add_terrain_object(mouse_grid_pos, terrain_id) {
                                game_grid
                                    .terrain_objects_mut()
                                    .insert(mouse_grid_pos, terrain_id);
                                game_grid.auto_tile_terrain_objects();
                                map_data.terrain_objects.insert(mouse_grid_pos, terrain_id);
                                has_unsaved_changes = true;
                            }
                        }
                        EditorAction::EraseTerrain => {
                            if game_grid
                                .terrain_objects_mut()
                                .contains_key(&mouse_grid_pos)
                            {
                                init_state.try_remove_terrain_object(&mouse_grid_pos);
                                game_grid.terrain_objects_mut().remove(&mouse_grid_pos);
                                game_grid.auto_tile_terrain_objects();
                                map_data.terrain_objects.remove(&mouse_grid_pos);
                                has_unsaved_changes = true;
                            }
                        }
                    }
                }
            }
        }

        if is_key_pressed(KeyCode::Space) {
            println!("Running game ...");
            QUIT_WITH_ESCAPE.store(true, Ordering::SeqCst);
            // If the game grid has auto-tiled the terrain objects, that needs to be applied here as well, to be reflected in-game
            init_state.terrain_objects = game_grid.terrain_objects_mut().clone();
            let core_game = init_core_game(
                resources.clone(),
                ui_resources.clone(),
                sound_player.clone(),
                init_state.clone(),
            );
            match core_game.run().await {
                Ok(_chars) => println!("Game ended naturally"),
                Err(QuitEvent) => println!("User quit from game"),
            }
        }

        if is_key_down(KeyCode::LeftControl) && is_key_pressed(KeyCode::S) {
            map_data.save_to_file(SAVE_FILE_NAME);
            /*
            let json_str = serde_json::to_string_pretty(&map_data).unwrap();

            fs::write("testsavefile.json", json_str).expect("Writing json to file");
            println!("Saved map data to {}", SAVE_FILE_NAME);
             */
            has_unsaved_changes = false;
        }
    }
}

fn bot(behaviour: BotBehaviour, move_speed: f32) -> CharacterKind {
    CharacterKind::Bot(Bot {
        behaviour,
        base_movement: move_speed,
    })
}

struct Sidebar {
    terrain_atlas: Texture2D,
    selected_idx: Option<usize>,
    hovered: bool,
    actions: Vec<EditorAction>,
}

#[derive(Copy, Clone)]
enum EditorAction {
    PlaceTerrain(TerrainId),
    EraseTerrain,
}

impl Sidebar {
    fn new(terrain_atlas: Texture2D) -> Self {
        let terrain_ids = vec![
            TerrainId::Bush,
            TerrainId::Boulder2,
            TerrainId::TreeStump,
            TerrainId::BookShelf,
            TerrainId::Table,
            TerrainId::NewWaterNorthEast,
            TerrainId::StoneWallConvexNorthEast,
        ];
        let mut actions: Vec<EditorAction> = terrain_ids
            .iter()
            .map(|t| EditorAction::PlaceTerrain(*t))
            .collect();
        actions.push(EditorAction::EraseTerrain);

        Self {
            terrain_atlas,
            actions,
            selected_idx: None,
            hovered: false,
        }
    }

    fn draw(&mut self) {
        if let Some(i) = self.selected_idx {
            if is_key_pressed(KeyCode::Left) {
                self.selected_idx =
                    Some(((i as i32 - 1).rem_euclid(self.actions.len() as i32)) as usize);
            } else if is_key_pressed(KeyCode::Right) {
                self.selected_idx =
                    Some(((i as i32 + 1).rem_euclid(self.actions.len() as i32)) as usize);
            }
        }

        let cols = 3;
        let rows = 1 + self.actions.len() / cols;
        let margin = 4.0;
        let pad = 10.0;
        let icon_w = 64.0;
        let w = pad * 2.0 + cols as f32 * icon_w + (cols - 1) as f32 * margin;
        let h = pad * 2.0 + rows as f32 * icon_w + (rows - 1) as f32 * margin;

        let mouse_pos = mouse_position();
        self.hovered = Rect::new(0.0, 0.0, w, h).contains(mouse_pos.into());

        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.5));

        for (i, action) in self.actions.iter().enumerate() {
            let col = i % cols;
            let x = pad + col as f32 * (icon_w + margin);
            let row = i / cols;
            let y = pad + row as f32 * (icon_w + margin);
            if is_mouse_button_pressed(MouseButton::Left)
                && Rect::new(x, y, icon_w, icon_w).contains(mouse_pos.into())
            {
                self.selected_idx = Some(i);
            }
            let bg = if self.selected_idx == Some(i) {
                YELLOW
            } else {
                WHITE
            };
            draw_rectangle(x, y, icon_w, icon_w, bg);
            match action {
                EditorAction::PlaceTerrain(terrain_id) => {
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
                EditorAction::EraseTerrain => {
                    draw_text("Erase", x, y + icon_w / 2.0, 16.0, BLACK);
                }
            }

            if self.selected_idx == Some(i) {
                draw_rectangle_lines(x + 1.0, y + 1.0, icon_w - 2.0, icon_w - 2.0, 3.0, BLACK);
            }
        }
    }

    fn action(&self) -> Option<EditorAction> {
        self.selected_idx.map(|i| self.actions[i])
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

#[derive(Serialize, Deserialize, Debug)]
pub struct MapData {
    pub grid_dimensions: (u32, u32),
    pub terrain_objects: HashMap<Position, TerrainId>,
    pub background: HashMap<Position, TerrainId>,
    pub characters: HashMap<Position, MapCharacterId>,
}

impl MapData {
    fn new(
        grid_dimensions: (u32, u32),
        terrain_objects: HashMap<Position, TerrainId>,
        background: HashMap<Position, TerrainId>,
        characters: HashMap<Position, MapCharacterId>,
    ) -> Self {
        Self {
            grid_dimensions,
            terrain_objects,
            background,
            characters,
        }
    }

    fn save_to_file(&self, filename: &str) {
        let terrain_objects: HashMap<String, TerrainId> = keys_pos_to_str(&self.terrain_objects);
        let background: HashMap<String, TerrainId> = keys_pos_to_str(&self.background);
        let characters = keys_pos_to_str(&self.characters);
        let map_data = SerializableMapData {
            grid_dimensions: self.grid_dimensions,
            terrain_objects,
            background,
            characters,
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
            characters: keys_str_to_pos(&map_data.characters),
        }
    }
}

fn keys_pos_to_str<V: Copy>(map: &HashMap<Position, V>) -> HashMap<String, V> {
    map.iter().map(|(k, v)| (format!("{k:?}"), *v)).collect()
}

fn keys_str_to_pos<V: Copy>(map: &HashMap<String, V>) -> HashMap<Position, V> {
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
pub struct SerializableMapData {
    pub grid_dimensions: (u32, u32),
    pub terrain_objects: HashMap<String, TerrainId>,
    pub background: HashMap<String, TerrainId>,
    pub characters: HashMap<String, MapCharacterId>,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
enum MapCharacterId {
    Player1,
    Enemy1,
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
