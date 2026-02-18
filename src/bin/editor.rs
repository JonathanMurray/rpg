use std::cell::{self, Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::atomic::Ordering;

use macroquad::color::{MAGENTA, WHITE};
use macroquad::input::{
    is_key_pressed, is_mouse_button_pressed, mouse_position, KeyCode, MouseButton,
};
use macroquad::shapes::draw_rectangle;
use macroquad::text::{
    draw_text, draw_text_ex, load_ttf_font, measure_text, Font, TextDimensions, TextParams,
};
use macroquad::texture::{draw_texture, draw_texture_ex, DrawTextureParams, FilterMode, Texture2D};
use macroquad::window::next_frame;
use macroquad::window::{clear_background, Conf};

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
    EquipmentIconId, IconId, PortraitId, SpriteId, StatusId, TerrainId,
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

    let mut terrain_idx = 0;
    let terrain_ids = vec![
        TerrainId::Bush,
        TerrainId::Boulder2,
        TerrainId::TreeStump,
        TerrainId::Table,
        TerrainId::NewWaterEast,
        TerrainId::NewWaterNorthEast,
    ];

    loop {
        next_frame().await;
        game_grid.draw(true, &mut UiState::Idle, false, None, (0, 0));
        game_grid.draw_debug_cells();

        let mouse_grid_pos = game_grid.mouse_grid_pos();
        let snapped_mouse_screen_pos = game_grid.grid_pos_to_screen(mouse_grid_pos);

        if is_key_pressed(KeyCode::Left) {
            terrain_idx = ((terrain_idx as i32 - 1).rem_euclid(terrain_ids.len() as i32)) as usize;
        } else if is_key_pressed(KeyCode::Right) {
            terrain_idx = ((terrain_idx as i32 + 1).rem_euclid(terrain_ids.len() as i32)) as usize;
        }

        let entity_size = game_grid.entity_draw_size();
        draw_rectangle(
            snapped_mouse_screen_pos.0 - game_grid.cell_w,
            snapped_mouse_screen_pos.1 - game_grid.cell_w,
            entity_size.0,
            entity_size.1,
            MAGENTA,
        );

        game_grid.draw_terrain(
            terrain_ids[terrain_idx],
            snapped_mouse_screen_pos.0,
            snapped_mouse_screen_pos.1,
        );

        if is_mouse_button_pressed(MouseButton::Left) {
            if init_state.try_add_terrain_object(mouse_grid_pos, terrain_ids[terrain_idx]) {
                game_grid
                    .terrain_objects_mut()
                    .insert(mouse_grid_pos, terrain_ids[terrain_idx]);
            }
        } else if is_mouse_button_pressed(MouseButton::Right) {
            if game_grid
                .terrain_objects_mut()
                .contains_key(&mouse_grid_pos)
            {
                init_state.try_remove_terrain_object(&mouse_grid_pos);
                game_grid.terrain_objects_mut().remove(&mouse_grid_pos);
            }
        }

        if is_key_pressed(KeyCode::Space) {
            println!("Running game ...");
            QUIT_WITH_ESCAPE.store(true, Ordering::SeqCst);
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
