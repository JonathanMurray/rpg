use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use macroquad::color::{Color, MAGENTA, WHITE};
use macroquad::input::{get_keys_pressed, mouse_position};
use macroquad::miniquad::window::{self, set_window_position, set_window_size};

use macroquad::shapes::draw_rectangle;
use macroquad::text::{draw_text, load_ttf_font, Font};
use macroquad::texture::{FilterMode, Texture2D};
use macroquad::{
    color::BLACK,
    miniquad,
    rand::{self},
    time::get_frame_time,
    window::{clear_background, next_frame, Conf},
};

use rpg::bot::{bot_choose_attack_reaction, bot_choose_hit_reaction};
use rpg::chest_scene::run_chest_loop;
use rpg::core::{
    Action, Attributes, BaseAction, Behaviour, Character, CharacterId, Condition, CoreGame,
    EquipmentEntry, HandType, OnAttackedReaction, OnHitReaction, PassiveSkill,
};

use rpg::data::{
    BOW, BRACE, CRIPPLING_SHOT, DAGGER, FIREBALL, HEALING_NOVA, HEALING_RAIN, HEALTH_POTION, KILL,
    LEATHER_ARMOR, LUNGE_ATTACK, OVERWHELMING, RAGE, ROBE, SHACKLED_MIND, SHIRT, SIDE_STEP,
    SWEEP_ATTACK, SWORD,
};
use rpg::game_ui::{PlayerChose, UiState, UserInterface};
use rpg::game_ui_connection::GameUserInterfaceConnection;
use rpg::init_fight_map::{init_fight_map, FightId};
use rpg::map_scene::{MapChoice, MapScene};
use rpg::rest_scene::run_rest_loop;
use rpg::shop_scene::run_shop_loop;
use rpg::textures::{
    load_all_equipment_icons, load_all_icons, load_all_portraits, load_all_sprites,
    load_and_init_texture, EquipmentIconId, IconId, PortraitId, SpriteId,
};
use rpg::victory_scene::{run_victory_loop, Learning};

async fn load_font(path: &str) -> Font {
    let path = format!("fonts/{path}");
    let mut font = load_ttf_font(&path).await.unwrap();
    font.set_filter(FilterMode::Nearest);
    font
}

#[macroquad::main(window_conf)]
async fn main() {
    // Seed the random numbers
    rand::srand(miniquad::date::now() as u64);

    // Without this, the window seems to start on a random position on the screen, sometimes with the bottom obscured
    set_window_position(100, 100);

    dbg!(
        window::screen_size(),
        window::dpi_scale(),
        window::high_dpi()
    );

    let font_path = "delicatus/Delicatus.ttf"; // <-- not bad! very thin and readable
    let font = load_font(font_path).await;

    let equipment_icons = load_all_equipment_icons().await;

    let icons = load_all_icons().await;

    let portrait_textures = load_all_portraits().await;

    let mut map_scene = MapScene::new();

    let mut alice = Character::new(
        Behaviour::Player,
        "Alice",
        PortraitId::Alice,
        SpriteId::Alice,
        Attributes::new(2, 3, 3, 2),
        (1, 10),
    );
    alice.try_gain_equipment(EquipmentEntry::Consumable(HEALTH_POTION));
    alice.set_weapon(HandType::MainHand, BOW);
    alice.armor_piece.set(Some(SHIRT));
    alice.inventory[0].set(Some(EquipmentEntry::Weapon(DAGGER)));

    let mut bob = Character::new(
        Behaviour::Player,
        "Bob",
        PortraitId::Bob,
        SpriteId::Bob,
        Attributes::new(3, 2, 2, 3),
        (2, 10),
    );
    bob.set_weapon(HandType::MainHand, SWORD);
    bob.armor_piece.set(Some(LEATHER_ARMOR));
    //bob.health.lose(2);

    let mut player_characters = vec![alice, bob];



    loop {
        let map_choice = map_scene.run_map_loop(font.clone()).await;
        match map_choice {
            MapChoice::Rest => {
                player_characters = run_rest_loop(
                    player_characters,
                    font.clone(),
                    &equipment_icons,
                    icons.clone(),
                    &portrait_textures,
                )
                .await;
            }
            MapChoice::Shop => {
                player_characters = run_shop_loop(
                    player_characters,
                    font.clone(),
                    &equipment_icons,
                    icons.clone(),
                    &portrait_textures,
                )
                .await;
            }
            MapChoice::Fight(fight_id) => {
                player_characters = run_fight_loop(
                    player_characters,
                    fight_id,
                    &equipment_icons,
                    icons.clone(),
                    portrait_textures.clone(),
                )
                .await;

                player_characters = run_victory_loop(
                    player_characters,
                    font.clone(),
                    &equipment_icons,
                    icons.clone(),
                    &portrait_textures,
                )
                .await;
            }
            MapChoice::Chest(reward) => {
                player_characters = run_chest_loop(
                    player_characters,
                    font.clone(),
                    &equipment_icons,
                    icons.clone(),
                    &portrait_textures,
                )
                .await;
            }
        }
    }
}

async fn run_fight_loop(
    player_characters: Vec<Character>,
    fight_id: FightId,
    equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
    icons: HashMap<IconId, Texture2D>,
    portrait_textures: HashMap<PortraitId, Texture2D>,
) -> Vec<Character> {
    let core_game = init_fight_scene(
        player_characters,
        fight_id,
        &equipment_icons,
        icons.clone(),
        portrait_textures.clone(),
    )
    .await;
    core_game.run().await
}

async fn init_fight_scene(
    player_characters: Vec<Character>,
    fight_id: FightId,
    equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
    icons: HashMap<IconId, Texture2D>,
    portrait_textures: HashMap<PortraitId, Texture2D>,
) -> CoreGame {
    let init_state = init_fight_map(player_characters, fight_id);

    let mut game_ui = GameUserInterfaceConnection::uninitialized();

    let core_game = CoreGame::new(game_ui.clone(), &init_state);

    let sprites = load_all_sprites().await;

    //let font_path = "manaspace/manaspc.ttf";
    //let font_path = "yoster-island/yoster.ttf"; // <-- looks like yoshi's island. Not very readable
    //let font_path = "pixy/PIXY.ttf"; // <-- only uppercase, looks a bit too sci-fi?
    //let font_path = "return-of-ganon/retganon.ttf";
    //let font_path = "press-start/prstart.ttf";
    //let font_path = "lunchtime-doubly-so/lunchds.ttf";
    //let font_path = "chonkypixels/ChonkyPixels.ttf";
    let _font_path = "pixelon/Pixelon.ttf";
    let font_path = "delicatus/Delicatus.ttf"; // <-- not bad! very thin and readable
    let font = load_font(font_path).await;

    let grid_big_font = load_font("manaspace/manaspc.ttf").await;

    let decorative_font = load_font("dpcomic/dpcomic.ttf").await;

    /*
    let empty_grass = load_and_init_texture("grass3.png").await;
    let background_textures = vec![
        load_and_init_texture("grass1.png").await,
        load_and_init_texture("grass2.png").await,
        empty_grass.clone(),
        empty_grass.clone(),
        empty_grass.clone(),
    ];
     */

    let terrain_atlas = load_and_init_texture("terrain_atlas.png").await;

    let gfx_user_interface = UserInterface::new(
        &core_game,
        sprites,
        icons,
        equipment_icons,
        portrait_textures,
        terrain_atlas,
        font.clone(),
        decorative_font,
        grid_big_font,
        init_state,
    );

    game_ui.init(gfx_user_interface);

    core_game
}

fn window_conf() -> Conf {
    Conf {
        window_title: "UI test".to_owned(),
        window_width: 1280,
        //window_height: 960,
        window_height: 1060,
        high_dpi: true,
        window_resizable: false,
        ..Default::default()
    }
}
