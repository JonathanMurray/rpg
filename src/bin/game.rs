use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

use macroquad::audio::load_sound;
use macroquad::color::{Color, LIGHTGRAY, MAGENTA, WHITE};
use macroquad::input::{get_keys_pressed, mouse_position};
use macroquad::math::{vec2, Rect};
use macroquad::miniquad::window::{self, set_window_position, set_window_size};
use macroquad::miniquad::KeyCode;

use macroquad::shapes::draw_rectangle;
use macroquad::text::{draw_text, load_ttf_font, Font};
use macroquad::texture::{draw_texture, draw_texture_ex, DrawTextureParams, FilterMode, Texture2D};
use macroquad::time::get_time;
use macroquad::window::{clear_background, next_frame, screen_height, screen_width};
use macroquad::{
    color::BLACK,
    miniquad,
    rand::{self},
    time::get_frame_time,
    window::Conf,
};

use rpg::bot::{bot_choose_attack_reaction, bot_choose_hit_reaction};
use rpg::chest_scene::run_chest_loop;
use rpg::core::{
    Action, ArrowStack, Attributes, BaseAction, Character, CharacterId, CharacterKind, Condition,
    CoreGame, EquipmentEntry, HandType, OnAttackedReaction, OnHitReaction, Party,
};

use rpg::data::{
    PassiveSkill, ADRENALIN_POTION, ARCANE_POTION, BARBED_ARROWS, BONE_CRUSHER, BOW, BRACE,
    CHAIN_MAIL, COLD_ARROWS, CRIPPLING_SHOT, DAGGER, EMPOWER, ENERGY_POTION, EXPLODING_ARROWS,
    FIREBALL, FIREBALL_INFERNO, FIREBALL_MASSIVE, FIREBALL_REACH, HASTE, HEAL, HEALING_NOVA,
    HEALING_RAIN, HEALTH_POTION, HEAL_ENERGIZE, INFLICT_WOUNDS, INFLICT_WOUNDS_NECROTIC_INFLUENCE,
    INSPIRE, KILL, LEATHER_ARMOR, LONGER_REACH, LUNGE_ATTACK, LUNGE_ATTACK_HEAVY_IMPACT,
    LUNGE_ATTACK_REACH, MANA_POTION, MEDIUM_SHIELD, MIND_BLAST, OVERWHELMING, PENETRATING_ARROWS,
    PIERCING_SHOT, RAGE, ROBE, SCREAM, SCREAM_SHRIEK, SEARING_LIGHT, SEARING_LIGHT_BURN,
    SHACKLED_MIND, SHIELD_BASH, SHIELD_BASH_KNOCKBACK, SHIRT, SIDE_STEP, SMALL_SHIELD, SMITE,
    SWEEP_ATTACK, SWEEP_ATTACK_PRECISE, SWORD,
};
use rpg::game_ui::{PlayerChose, UiState, UserInterface};
use rpg::game_ui_connection::GameUserInterfaceConnection;
use rpg::init_fight_map::{init_fight_map, FightId, GameInitState};
use rpg::map_scene::{MapChoice, MapScene};
use rpg::resources::{init_core_game, GameResources, UiResources};
use rpg::rest_scene::run_rest_loop;
use rpg::shop_scene::{generate_shop_contents, run_shop_loop};
use rpg::skill_tree::run_skill_tree_scene;
use rpg::sounds::SoundPlayer;
use rpg::textures::{
    load_all_equipment_icons, load_all_icons, load_all_portraits, load_all_sprites,
    load_all_status_textures, load_and_init_font_symbols, load_and_init_texture,
    load_and_init_ui_textures, EquipmentIconId, IconId, PortraitId, SpriteId, DICE_SYMBOL,
};
use rpg::victory_scene::{run_victory_loop, Learning};

#[macroquad::main(window_conf)]
async fn main() {
    // Seed the random numbers
    rand::srand(miniquad::date::now() as u64);

    // Without this, the window seems to start on a random position on the screen, sometimes with the bottom obscured
    set_window_position(100, 100);

    dbg!(get_time());
    dbg!(
        window::screen_size(),
        window::dpi_scale(),
        window::high_dpi()
    );

    clear_background(BLACK);
    draw_text(
        "Loading...",
        screen_width() / 2.0,
        screen_height() / 2.0,
        32.0,
        LIGHTGRAY,
    );
    next_frame().await;
    dbg!(get_time());

    let resources = GameResources::load().await;
    let ui_resources = UiResources::load().await;
    load_and_init_font_symbols().await;
    load_and_init_ui_textures().await;

    let mut map_scene = MapScene::new(ui_resources.portrait_textures.clone()).await;

    let sound_player = SoundPlayer::new().await;

    let party = Rc::new(Party {
        money: Cell::new(8),
        stash: Default::default(),
    });

    let mut alice = Character::new(
        CharacterKind::Player(Rc::clone(&party)),
        "Alice",
        PortraitId::Alice,
        SpriteId::Alice,
        Attributes::new(3, 5, 3, 3),
        (1, 10),
    );
    alice.set_weapon(HandType::MainHand, BOW);
    alice.armor_piece.set(Some(SHIRT));
    alice.arrows.set(Some(ArrowStack::new(EXPLODING_ARROWS, 3)));
    alice.learn_ability(HEAL);
    alice.known_ability_enhancements.push(HEAL_ENERGIZE);
    alice.known_attack_enhancements.push(CRIPPLING_SHOT);
    alice
        .known_passive_skills
        .push(PassiveSkill::WeaponProficiency);
    alice.learn_ability(PIERCING_SHOT);

    let mut bob = Character::new(
        CharacterKind::Player(Rc::clone(&party)),
        "Bob",
        PortraitId::Bob,
        SpriteId::Bob,
        Attributes::new(5, 3, 3, 3),
        (2, 10),
    );
    bob.set_weapon(HandType::MainHand, SWORD);
    bob.set_shield(MEDIUM_SHIELD);
    bob.armor_piece.set(Some(LEATHER_ARMOR));
    bob.known_passive_skills.push(PassiveSkill::Reaper);
    bob.learn_ability(SWEEP_ATTACK);
    bob.learn_ability(SHIELD_BASH);
    bob.known_ability_enhancements.push(SHIELD_BASH_KNOCKBACK);
    bob.learn_ability(INSPIRE);
    bob.known_attack_enhancements.push(SMITE);
    //bob.known_attack_enhancements.push(EMPOWER);
    bob.try_gain_equipment(EquipmentEntry::Consumable(HEALTH_POTION));

    let mut clara = Character::new(
        CharacterKind::Player(Rc::clone(&party)),
        "Clara",
        PortraitId::Clara,
        SpriteId::Clara,
        Attributes::new(2, 2, 3, 7),
        (3, 10),
    );
    clara.set_weapon(HandType::MainHand, DAGGER);
    // TODO:
    clara.armor_piece.set(Some(SHIRT));
    clara
        .known_passive_skills
        .push(PassiveSkill::CriticalCharge);
    clara.learn_ability(FIREBALL);
    clara.known_ability_enhancements.push(FIREBALL_INFERNO);
    clara.known_ability_enhancements.push(FIREBALL_REACH);
    clara.known_ability_enhancements.push(FIREBALL_MASSIVE);
    clara.learn_ability(SHACKLED_MIND);
    clara.learn_ability(INFLICT_WOUNDS);
    clara
        .known_ability_enhancements
        .push(INFLICT_WOUNDS_NECROTIC_INFLUENCE);
    //clara.learn_ability(MIND_BLAST);

    clara.try_gain_equipment(EquipmentEntry::Consumable(MANA_POTION));
    clara.try_gain_equipment(EquipmentEntry::Consumable(ARCANE_POTION));

    let mut player_characters = vec![clara, alice, bob];

    dbg!(get_time());

    player_characters = run_fight_loop(
        resources.clone(),
        player_characters,
        //FightId::VerticalSliceNew,
        FightId::Test,
        ui_resources.clone(),
        sound_player.clone(),
    )
    .await;

    loop {
        let map_choice = map_scene
            .run_map_loop(resources.simple_font.clone(), &player_characters[..])
            .await;
        match map_choice {
            MapChoice::Rest => {
                player_characters = run_rest_loop(
                    player_characters,
                    resources.simple_font.clone(),
                    &ui_resources.equipment_icons,
                    ui_resources.icons.clone(),
                    &ui_resources.portrait_textures,
                )
                .await;
            }
            MapChoice::Shop(entries) => {
                player_characters = run_shop_loop(
                    player_characters,
                    resources.simple_font.clone(),
                    &ui_resources.equipment_icons,
                    ui_resources.icons.clone(),
                    &ui_resources.portrait_textures,
                    &party,
                    entries,
                )
                .await;
            }
            MapChoice::Fight(fight_id) => {
                player_characters = run_fight_loop(
                    resources.clone(),
                    player_characters,
                    *fight_id,
                    ui_resources.clone(),
                    sound_player.clone(),
                )
                .await;

                player_characters = run_victory_loop(
                    player_characters,
                    resources.simple_font.clone(),
                    &ui_resources.equipment_icons,
                    ui_resources.icons.clone(),
                    &ui_resources.portrait_textures,
                    &party,
                )
                .await;
            }
            MapChoice::Chest(entries) => {
                player_characters = run_chest_loop(
                    player_characters,
                    resources.simple_font.clone(),
                    &ui_resources.equipment_icons,
                    ui_resources.icons.clone(),
                    &ui_resources.portrait_textures,
                    entries,
                )
                .await;
            }
        }
    }
}

async fn run_fight_loop(
    resources: GameResources,
    player_characters: Vec<Character>,
    fight_id: FightId,
    ui_resources: UiResources,
    sound_player: SoundPlayer,
) -> Vec<Character> {
    let init_state = init_fight_map(player_characters, fight_id);
    let core_game = init_core_game(resources, ui_resources, sound_player, init_state);
    // Run one quick frame, so that the core game doesn't think that much time has elapsed on the very first frame
    next_frame().await;
    next_frame().await;
    dbg!(get_time());
    core_game
        .run()
        .await
        .expect("'quit' is only implemented for Editor, as of yet")
}

fn window_conf() -> Conf {
    Conf {
        window_title: "RPG".to_owned(),
        window_width: 1600,
        //window_height: 960,
        window_height: 1200,
        high_dpi: true,

        window_resizable: false,
        ..Default::default()
    }
}
