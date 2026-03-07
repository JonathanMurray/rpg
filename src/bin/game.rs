use std::collections::HashMap;

use macroquad::color::LIGHTGRAY;
use macroquad::miniquad::window::{self, set_window_position};

use macroquad::text::draw_text;
use macroquad::time::get_time;
use macroquad::window::{clear_background, next_frame, screen_height, screen_width};
use macroquad::{
    color::BLACK,
    miniquad,
    rand::{self},
    window::Conf,
};

use rpg::action_button::ButtonAction;
use rpg::chest_scene::run_chest_loop;
use rpg::core::{BaseAction, Character, Party};

use rpg::data::{CRIPPLING_SHOT, FIREBALL, INSPIRE, PIERCING_SHOT, SHACKLED_MIND, SWEEP_ATTACK};
use rpg::init_fight_map::{init_fight_map, FightId};
use rpg::map_data::{make_high_alice, make_high_bob, make_low_level_party, make_medium_clara};
use rpg::map_scene::{MapChoice, MapScene};
use rpg::resources::{init_core_game, GameResources, UiResources};
use rpg::rest_scene::run_rest_loop;
use rpg::shop_scene::run_shop_loop;
use rpg::sounds::SoundPlayer;
use rpg::textures::{load_and_init_font_symbols, load_and_init_ui_textures};
use rpg::transition_scene::{run_transition_loop, CharacterGrowth};
use rpg::victory_scene::run_victory_loop;

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

    let (party, mut player_characters) = make_low_level_party();

    player_characters = run_fight_loop(
        resources.clone(),
        player_characters,
        FightId::EasyCluster,
        //FightId::Test,
        ui_resources.clone(),
        sound_player.clone(),
    )
    .await;

    player_characters = grow_players(
        player_characters,
        [
            (
                "Bob",
                CharacterGrowth::just_new_skills(vec![ButtonAction::Action(
                    BaseAction::UseAbility(SWEEP_ATTACK),
                )]),
            ),
            (
                "Alice",
                CharacterGrowth::just_new_skills(vec![ButtonAction::AttackEnhancement(
                    CRIPPLING_SHOT,
                )]),
            ),
        ]
        .into(),
        &resources,
        &ui_resources,
        &party,
    )
    .await;

    player_characters = run_fight_loop(
        resources.clone(),
        player_characters,
        FightId::Medium,
        ui_resources.clone(),
        sound_player.clone(),
    )
    .await;

    player_characters.push(make_medium_clara(&party));

    player_characters = grow_players(
        player_characters,
        [
            (
                "Alice",
                CharacterGrowth::just_new_skills(vec![ButtonAction::Action(
                    BaseAction::UseAbility(INSPIRE),
                )]),
            ),
            ("Clara", CharacterGrowth::new_joiner()),
        ]
        .into(),
        &resources,
        &ui_resources,
        &party,
    )
    .await;

    player_characters = run_fight_loop(
        resources.clone(),
        player_characters,
        FightId::EliteHuldra,
        ui_resources.clone(),
        sound_player.clone(),
    )
    .await;

    player_characters = grow_players(
        player_characters,
        [(
            "Alice",
            CharacterGrowth::just_new_skills(vec![ButtonAction::Action(BaseAction::UseAbility(
                PIERCING_SHOT,
            ))]),
        )]
        .into(),
        &resources,
        &ui_resources,
        &party,
    )
    .await;

    player_characters = run_fight_loop(
        resources.clone(),
        player_characters,
        FightId::VerticalSliceNew,
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

fn build_player_growths(
    player_characters: Vec<Character>,
    mut growths: HashMap<&'static str, CharacterGrowth>,
) -> Vec<(Character, CharacterGrowth)> {
    player_characters
        .into_iter()
        .map(|ch| {
            let growth = growths
                .remove(ch.name)
                .unwrap_or(CharacterGrowth::unchanged());
            (ch, growth)
        })
        .collect()
}

async fn grow_players(
    player_characters: Vec<Character>,
    growths: HashMap<&'static str, CharacterGrowth>,
    resources: &GameResources,
    ui_resources: &UiResources,
    party: &Party,
) -> Vec<Character> {
    let player_growths = build_player_growths(player_characters, growths.into());
    run_transition_loop(player_growths, &resources, &ui_resources, &party).await
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
