use std::collections::HashMap;
use std::rc::Rc;

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
use rpg::core::{ArrowStack, BaseAction, Character, EquipmentEntry, Party, PlayerId};

use rpg::data::{
    PassiveSkill, BARBED_ARROWS, CRIPPLING_SHOT, ENERGY_POTION, EXPLODING_ARROWS, FIREBALL_MASSIVE,
    HEAL, HEALTH_POTION, HEAL_ENERGIZE, LEATHER_ARMOR, PIERCING_SHOT, SWEEP_ATTACK,
};
use rpg::game_over_scene::run_game_over_scene;
use rpg::init_fight_map::{init_fight_map, FightId};
use rpg::map_data::{make_high_alice, make_high_bob, make_low_level_party, make_medium_clara};
use rpg::resources::{init_core_game, GameResources, UiResources};
use rpg::sounds::SoundPlayer;
use rpg::textures::load_and_init_static;
use rpg::transition_scene::{run_transition_loop, CharacterGrowth};

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
    load_and_init_static().await;

    let sound_player = SoundPlayer::new().await;

    run_demo(&resources, &ui_resources, sound_player).await;
}

async fn run_demo(
    resources: &GameResources,
    ui_resources: &UiResources,
    sound_player: SoundPlayer,
) {
    loop {
        let (party, player_characters) = make_low_level_party();
        let mut player_characters: Vec<Rc<Character>> =
            player_characters.into_iter().map(Rc::new).collect();

        //player_characters.push(Rc::new(make_medium_clara(&party)));

        // TODO
        /*
           player_characters = grow_players(
               player_characters,
               vec![
                   (
                       PlayerId::Alice,
                       CharacterGrowth::of(
                           vec![ButtonAction::Action(BaseAction::UseAbility(PIERCING_SHOT))],
                           vec![],
                       ),
                   ),
                   (
                       PlayerId::Bob,
                       CharacterGrowth::of(
                           vec![ButtonAction::Passive(PassiveSkill::Reaper)],
                           vec![EquipmentEntry::Armor(LEATHER_ARMOR)],
                       ),
                   ),
               ]
               .into_iter()
               .collect(),
               resources,
               ui_resources,
               &party,
               sound_player.clone(),
           )
           .await;
        */
        // -------------------------

        let mut demo_sequence = [
            (
                FightId::EasyCluster,
                vec![
                    (
                        PlayerId::Bob,
                        CharacterGrowth::of(
                            vec![ButtonAction::Action(BaseAction::UseAbility(SWEEP_ATTACK))],
                            vec![EquipmentEntry::Consumable(ENERGY_POTION)],
                        ),
                    ),
                    (
                        PlayerId::Alice,
                        CharacterGrowth::of(
                            vec![
                                ButtonAction::AttackEnhancement(CRIPPLING_SHOT),
                                ButtonAction::Action(BaseAction::UseAbility(HEAL)),
                            ],
                            vec![],
                        ),
                    ),
                ],
            ),
            (
                FightId::Medium,
                vec![
                    (
                        PlayerId::Bob,
                        CharacterGrowth::of(vec![], vec![EquipmentEntry::Armor(LEATHER_ARMOR)]),
                    ),
                    (
                        PlayerId::Alice,
                        CharacterGrowth::of(
                            vec![ButtonAction::AbilityEnhancement(HEAL_ENERGIZE)],
                            vec![EquipmentEntry::Arrows(ArrowStack::new(EXPLODING_ARROWS, 3))],
                        ),
                    ),
                    (PlayerId::Clara, CharacterGrowth::new_joiner()),
                ],
            ),
            (
                FightId::EliteHuldra,
                vec![
                    (
                        PlayerId::Alice,
                        CharacterGrowth::of(
                            vec![ButtonAction::Action(BaseAction::UseAbility(PIERCING_SHOT))],
                            vec![EquipmentEntry::Arrows(ArrowStack::new(BARBED_ARROWS, 2))],
                        ),
                    ),
                    (
                        PlayerId::Clara,
                        CharacterGrowth::of(
                            vec![ButtonAction::AbilityEnhancement(FIREBALL_MASSIVE)],
                            vec![],
                        ),
                    ),
                    (
                        PlayerId::Bob,
                        CharacterGrowth::of(
                            vec![ButtonAction::Passive(PassiveSkill::Reaper)],
                            vec![EquipmentEntry::Consumable(HEALTH_POTION)],
                        ),
                    ),
                ],
            ),
            (FightId::VerticalSliceNew, vec![]),
        ]
        .into_iter()
        .peekable();

        while let Some((fight, growths)) = demo_sequence.next() {
            run_fight_loop(
                resources.clone(),
                &player_characters,
                fight,
                ui_resources.clone(),
                sound_player.clone(),
            )
            .await;

            if player_characters.iter().all(|ch| ch.is_dead()) {
                run_game_over_scene(
                    resources,
                    ui_resources,
                    "Your party has perished! Try again, or submit a balancing complaint!",
                )
                .await;
                break;
            } else if demo_sequence.peek().is_none() {
                run_game_over_scene(resources, ui_resources, "You have completed the demo!").await;
                break;
            }

            player_characters = grow_players(
                player_characters,
                growths.into_iter().collect(),
                resources,
                ui_resources,
                &party,
                sound_player.clone(),
            )
            .await;
        }
    }
}

fn build_player_growths(
    player_characters: Vec<Rc<Character>>,
    mut growths: HashMap<PlayerId, CharacterGrowth>,
    party: &Rc<Party>,
) -> Vec<(Rc<Character>, CharacterGrowth)> {
    // Grow existing chars
    let mut result: Vec<(Rc<Character>, CharacterGrowth)> = player_characters
        .into_iter()
        .map(|ch| {
            let growth = growths
                .remove(&ch.player_id())
                .unwrap_or(CharacterGrowth::unchanged());
            (ch, growth)
        })
        .collect();

    // Add new chars
    for (player_id, growth) in growths {
        assert!(growth.is_new_joiner);
        let new_char = match player_id {
            PlayerId::Bob => make_high_bob(party),
            PlayerId::Alice => make_high_alice(party),
            PlayerId::Clara => make_medium_clara(party),
        };
        result.push((Rc::new(new_char), growth));
    }

    result
}

async fn grow_players(
    player_characters: Vec<Rc<Character>>,
    growths: HashMap<PlayerId, CharacterGrowth>,
    resources: &GameResources,
    ui_resources: &UiResources,
    party: &Rc<Party>,
    sound_player: SoundPlayer,
) -> Vec<Rc<Character>> {
    let player_growths = build_player_growths(player_characters, growths, party);
    run_transition_loop(player_growths, resources, ui_resources, party, sound_player).await
}

async fn run_fight_loop(
    resources: GameResources,
    player_characters: &[Rc<Character>],
    fight_id: FightId,
    ui_resources: UiResources,
    sound_player: SoundPlayer,
) {
    let player_characters: Vec<Rc<Character>> = player_characters
        .iter()
        .filter(|ch| !ch.is_dead())
        .map(Rc::clone)
        .collect();
    let init_state = init_fight_map(player_characters, fight_id);
    let core_game = init_core_game(resources, ui_resources, sound_player, init_state);
    // Run one quick frame, so that the core game doesn't think that much time has elapsed on the very first frame
    next_frame().await;
    next_frame().await;
    dbg!(get_time());
    core_game
        .run()
        .await
        .expect("'quit' is only implemented for Editor, as of yet");
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
