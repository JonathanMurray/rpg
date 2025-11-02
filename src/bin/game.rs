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
    Action, Attributes, BaseAction, Character, CharacterId, CoreGame, EquipmentEntry, HandType,
    OnAttackedReaction, OnHitReaction,
};

use rpg::data::{
    BOW, FIREBALL, HEALTH_POTION, KILL, LEATHER_ARMOR, LUNGE_ATTACK, OVERWHELMING, RAGE, ROBE,
    SHACKLED_MIND, SIDE_STEP, SWEEP_ATTACK, SWORD,
};
use rpg::game_ui::{PlayerChose, UiState, UserInterface};
use rpg::game_ui_connection::GameUserInterfaceConnection;
use rpg::init::{init, FightId};
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

    let mut player_character = init_player_character();
    player_character.health.lose(10);
    player_character.mana.lose(10);
    player_character.try_gain_equipment(EquipmentEntry::Consumable(HEALTH_POTION));

    loop {
        let map_choice = map_scene.run_map_loop(font.clone()).await;
        match map_choice {
            MapChoice::Rest => {
                player_character
                    .health
                    .gain(player_character.health.max / 2);
                player_character.mana.set_to_max();
                player_character = run_rest_loop(
                    player_character,
                    font.clone(),
                    &equipment_icons,
                    icons.clone(),
                    &portrait_textures,
                )
                .await;
            }
            MapChoice::Shop => {
                player_character = run_shop_loop(
                    player_character,
                    font.clone(),
                    &equipment_icons,
                    icons.clone(),
                    &portrait_textures,
                )
                .await;
            }
            MapChoice::Fight(fight_id) => {
                let core_game = init_fight_scene(
                    player_character,
                    fight_id,
                    &equipment_icons,
                    icons.clone(),
                    portrait_textures.clone(),
                )
                .await;
                player_character = core_game.run().await;

                let reward = run_victory_loop(
                    &player_character,
                    font.clone(),
                    &equipment_icons,
                    icons.clone(),
                )
                .await;
                player_character.gain_money(reward.money);
                learn(&mut player_character, reward.learning);
            }
            MapChoice::Chest(reward) => {
                player_character = run_chest_loop(
                    player_character,
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

fn learn(player_character: &mut Character, learning: Learning) {
    match learning {
        Learning::Spell(spell) => player_character
            .known_actions
            .push(BaseAction::CastSpell(spell)),
        Learning::OnAttackedReaction(reaction) => {
            player_character.known_attacked_reactions.push(reaction)
        }
        Learning::OnHitReaction(reaction) => player_character.known_on_hit_reactions.push(reaction),
        Learning::AttackEnhancement(enhancement) => {
            player_character.known_attack_enhancements.push(enhancement)
        }
        Learning::SpellEnhancement(enhancement) => {
            player_character.known_spell_enhancements.push(enhancement)
        }
    }
}

fn init_player_character() -> Character {
    let mut character = Character::new(
        true,
        "Alice",
        PortraitId::Portrait1,
        SpriteId::Character4,
        Attributes::new(5, 5, 5, 5),
        (1, 10),
    );
    character
        .known_actions
        .push(BaseAction::CastSpell(SHACKLED_MIND));
    /*
    character.known_actions.push(BaseAction::CastSpell(KILL)); //TODO
    character.known_attack_enhancements.push(OVERWHELMING);
    character.known_attacked_reactions.push(SIDE_STEP);
    //alice.known_attack_enhancements.push(QUICK);
    //alice.known_attack_enhancements.push(SMITE);
    character.known_on_hit_reactions.push(RAGE);
    character
        .known_actions
        .push(BaseAction::CastSpell(SWEEP_ATTACK));
    character
        .known_actions
        .push(BaseAction::CastSpell(LUNGE_ATTACK));
     */
    character.armor.set(Some(ROBE));
    character.set_weapon(HandType::MainHand, BOW);
    character.inventory[0].set(Some(EquipmentEntry::Weapon(SWORD)));

    character
}

async fn init_fight_scene(
    player_character: Character,
    fight_id: FightId,
    equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
    icons: HashMap<IconId, Texture2D>,
    portrait_textures: HashMap<PortraitId, Texture2D>,
) -> CoreGame {
    let init_state = init(player_character, fight_id);

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
