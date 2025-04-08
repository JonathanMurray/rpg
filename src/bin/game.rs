use std::rc::Rc;

use macroquad::miniquad::window::set_window_position;

use macroquad::text::{load_ttf_font, Font};
use macroquad::texture::FilterMode;
use macroquad::{
    color::BLACK,
    miniquad,
    rand::{self},
    time::get_frame_time,
    window::{clear_background, next_frame, Conf},
};

use rpg::bot::bot_choose_action;
use rpg::bot::{bot_choose_attack_reaction, bot_choose_hit_reaction};
use rpg::core::{CoreGame, GameState, StateChooseReaction};

use rpg::game_ui::{PlayerChose, UiGameEventHandler, UiState, UserInterface};
use rpg::textures::{
    load_all_equipment_icons, load_all_icons, load_all_sprites, load_and_init_texture,
};

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

    let event_handler = Rc::new(UiGameEventHandler::new());
    let game = CoreGame::new(event_handler.clone());

    let sprites = load_all_sprites().await;

    let icons = load_all_icons().await;

    let equipment_icons = load_all_equipment_icons().await;

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

    let grid_font = load_font("manaspace/manaspc.ttf").await;

    let decorative_font = load_font("dpcomic/dpcomic.ttf").await;

    let empty_grass = load_and_init_texture("grass3.png").await;
    let background_textures = vec![
        load_and_init_texture("grass1.png").await,
        load_and_init_texture("grass2.png").await,
        empty_grass.clone(),
        empty_grass.clone(),
        empty_grass.clone(),
    ];

    let mut user_interface = UserInterface::new(
        &game,
        sprites,
        icons,
        equipment_icons,
        font,
        decorative_font,
        grid_font,
        background_textures,
    );

    let mut game_state = game.begin();

    let mut waiting_for_ui_after_game_state_change = true;

    loop {
        let elapsed = get_frame_time();

        let ui_events = user_interface.update(game_state.game(), elapsed);

        clear_background(BLACK);

        user_interface.draw();

        if !ui_events.is_empty() {
            for player_choice in ui_events {
                match player_choice {
                    PlayerChose::AttackedReaction(reaction) => {
                        game_state = game_state.unwrap_react_to_attack().proceed(reaction);
                    }
                    PlayerChose::HitReaction(reaction) => {
                        game_state = game_state.unwrap_react_to_hit().proceed(reaction);
                    }
                    PlayerChose::Action(action) => {
                        dbg!(&action);
                        // TODO: Add option in UI to deliberately end turn
                        game_state = game_state.unwrap_choose_action().proceed(Some(action));
                    }
                }
            }
            waiting_for_ui_after_game_state_change = true;
            user_interface.set_state(UiState::Idle);

            // Handle any game events that might have resulted from the above state change
            for event in event_handler.events.borrow_mut().drain(..) {
                user_interface.handle_game_event(event);
            }
        }

        if user_interface.ready_for_more() && waiting_for_ui_after_game_state_change {
            println!("No longer waiting for UI!");
            waiting_for_ui_after_game_state_change = false;
            let players_turn = game_state.game().is_players_turn();
            game_state = match game_state {
                GameState::AwaitingChooseAction(..) if players_turn => {
                    user_interface.set_state(UiState::ChoosingAction);
                    game_state
                }
                GameState::AwaitingChooseAction(state) => {
                    assert!(!players_turn);
                    let action =
                        bot_choose_action(&state.game, user_interface.game_grid.grid_dimensions);
                    waiting_for_ui_after_game_state_change = true;
                    state.proceed(action)
                }
                GameState::AwaitingChooseReaction(state) if players_turn => {
                    let new_game_state = match state {
                        StateChooseReaction::Attack(choose_reaction) => {
                            let reaction = bot_choose_attack_reaction(
                                &choose_reaction.game,
                                choose_reaction.reactor,
                                choose_reaction.is_within_melee,
                            );
                            choose_reaction.proceed(reaction)
                        }
                        StateChooseReaction::Hit(choose_reaction) => {
                            let reaction = bot_choose_hit_reaction(
                                &choose_reaction.game,
                                choose_reaction.reactor,
                                choose_reaction.is_within_melee,
                            );
                            choose_reaction.proceed(reaction)
                        }
                    };
                    waiting_for_ui_after_game_state_change = true;
                    new_game_state
                }
                GameState::AwaitingChooseReaction(ref state) => {
                    assert!(!players_turn);
                    match state {
                        StateChooseReaction::Attack(inner) => {
                            println!("awaiting player attack reaction");
                            user_interface.set_state(UiState::ReactingToAttack {
                                attacker: inner.attacker,
                                hand: inner.hand,
                                reactor: inner.reactor,
                                is_within_melee: inner.is_within_melee,
                            });
                        }
                        StateChooseReaction::Hit(inner) => {
                            println!("awaiting player hit reaction");
                            user_interface.set_state(UiState::ReactingToHit {
                                attacker: inner.attacker,
                                victim: inner.reactor,
                                damage: inner.damage,
                                is_within_melee: inner.is_within_melee,
                            });
                        }
                    }
                    game_state
                }
                GameState::PerformingMovement(performing_movement) => {
                    waiting_for_ui_after_game_state_change = true;
                    performing_movement.proceed()
                }
            }
        }

        for event in event_handler.events.borrow_mut().drain(..) {
            user_interface.handle_game_event(event);
        }

        next_frame().await
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "UI test".to_owned(),
        window_width: 1600,
        window_height: 1200,
        high_dpi: true,
        ..Default::default()
    }
}
