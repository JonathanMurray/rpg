use std::cell::{self, Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::atomic::Ordering;

use macroquad::color::{Color, BLACK, LIGHTGRAY, MAGENTA, RED, WHITE, YELLOW};
use macroquad::input::{
    is_key_pressed, is_mouse_button_down, is_mouse_button_pressed, mouse_position,
    mouse_position_local, KeyCode, MouseButton,
};
use macroquad::math::Rect;
use macroquad::miniquad::window::set_window_position;
use macroquad::shapes::{draw_rectangle, draw_rectangle_lines};
use macroquad::text::{
    self, draw_text, draw_text_ex, load_ttf_font, measure_text, Font, TextDimensions, TextParams,
};
use macroquad::texture::{draw_texture, draw_texture_ex, DrawTextureParams, FilterMode, Texture2D};
use macroquad::window::{clear_background, Conf};
use macroquad::window::{next_frame, screen_height};

use rpg::base_ui::{
    draw_text_rounded, draw_text_with_font_tags, Align, Checkbox, Container, Drawable, Element,
    LayoutDirection, Style, TextLine,
};
use rpg::core::{
    Ability, Action, ArrowStack, AttackEnhancement, Attributes, BaseAction, Character, CharacterId,
    CharacterKind, Characters, Condition, CoreGame, EquipmentEntry, HandType, OnAttackedReaction,
    OnHitReaction, Party,
};

use rpg::game_ui::{PlayerChose, UiState, UserInterface};
use rpg::game_ui_connection::{QuitEvent, QUIT_WITH_ESCAPE};
use rpg::grid::GameGrid;
use rpg::init_fight_map::{init_fight_map, FightId};
use rpg::map_scene::{MapChoice, MapScene};
use rpg::resources::{init_core_game, GameResources, UiResources};
use rpg::sounds::SoundPlayer;
use rpg::textures::{
    draw_terrain, load_and_init_font_symbols, load_and_init_texture, load_and_init_ui_textures,
    terrain_atlas_area, EquipmentIconId, IconId, PortraitId, SpriteId, StatusId, TerrainId,
};
use rpg::victory_scene::{run_victory_loop, Learning};
use serde::{Deserialize, Serialize};

async fn load_font(path: &str) -> Font {
    let path = format!("fonts/{path}");
    let mut font = load_ttf_font(&path).await.unwrap();
    font.set_filter(FilterMode::Nearest);
    font
}

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

    let bob = Character::new(
        CharacterKind::Player(Rc::clone(&party)),
        "Bob",
        PortraitId::Bob,
        SpriteId::Bob,
        Attributes::new(5, 3, 3, 3),
        (2, 10),
    );

    let mut init_state = init_fight_map(vec![bob], FightId::VerticalSlice);

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

    loop {
        next_frame().await;
        game_grid.draw(true, &mut UiState::Idle, false, None, (0, 0));
        if show_grid.get() {
            game_grid.draw_debug_cells();
        }

        sidebar.draw();
        settings.draw(0.0, screen_height() - settings.size().1);

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
    }
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
