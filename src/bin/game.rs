use std::{collections::HashMap, rc::Rc};

use macroquad::miniquad::window::set_window_position;

use macroquad::texture::{load_texture, FilterMode, Texture2D};
use macroquad::{
    color::BLACK,
    miniquad,
    rand::{self},
    time::get_frame_time,
    window::{clear_background, next_frame, Conf},
};

use rpg::bot::bot_choose_action;
use rpg::bot::{bot_choose_attack_reaction, bot_choose_hit_reaction};
use rpg::core::{CoreGame, GameState, IconId, StateChooseReaction, TextureId};

use rpg::game_ui::{PlayerChose, UiGameEventHandler, UiState, UserInterface};

async fn texture(path: &str) -> Texture2D {
    let texture = load_texture(path).await.unwrap();
    texture.set_filter(FilterMode::Nearest);
    texture
}

async fn load_textures(paths: Vec<(TextureId, &str)>) -> HashMap<TextureId, Texture2D> {
    let mut textures: HashMap<TextureId, Texture2D> = Default::default();
    for (id, path) in paths {
        textures.insert(id, texture(path).await);
    }
    textures
}

async fn load_icons(paths: Vec<(IconId, &str)>) -> HashMap<IconId, Texture2D> {
    let mut textures: HashMap<IconId, Texture2D> = Default::default();
    for (id, path) in paths {
        textures.insert(id, texture(path).await);
    }
    textures
}

#[macroquad::main(window_conf)]
async fn main() {
    // Seed the random numbers
    rand::srand(miniquad::date::now() as u64);

    // Without this, the window seems to start on a random position on the screen, sometimes with the bottom obscured
    set_window_position(100, 100);

    let event_handler = Rc::new(UiGameEventHandler::new());
    let game = CoreGame::new(event_handler.clone());

    let textures = load_textures(vec![
        (TextureId::Character, "character.png"),
        (TextureId::Character2, "character2.png"),
        (TextureId::Warhammer, "warhammer.png"),
        (TextureId::Bow, "bow.png"),
        (TextureId::Sword, "sword.png"),
        (TextureId::Shield, "shield.png"),
    ])
    .await;

    let icons = load_icons(vec![
        (IconId::Fireball, "fireball_icon.png"),
        (IconId::Attack, "attack_icon.png"),
        (IconId::Brace, "brace_icon.png"),
        (IconId::Move, "move_icon.png"),
        (IconId::Scream, "scream_icon.png"),
        (IconId::Mindblast, "mindblast_icon.png"),
        (IconId::Go, "go_icon.png"),
        (IconId::Parry, "parry_icon.png"),
        (IconId::ShieldBash, "shieldbash_icon.png"),
        (IconId::Rage, "rage_icon.png"),
        (IconId::CrushingStrike, "crushing_strike_icon.png"),
        (IconId::Banshee, "banshee_icon.png"),
        (IconId::Dualcast, "dualcast_icon.png"),
        (IconId::AllIn, "all_in_icon.png"),
        (IconId::CarefulAim, "careful_aim_icon.png"),
        (IconId::Plus, "plus_icon.png"),
        (IconId::PlusPlus, "plus_plus_icon.png"),
    ])
    .await;

    let mut user_interface = UserInterface::new(&game, textures, icons);

    let mut game_state = game.begin();

    let mut waiting_for_ui_after_game_state_change = true;

    loop {
        let elapsed = get_frame_time();

        let ui_events = user_interface.update(game_state.game(), elapsed);

        clear_background(BLACK);

        user_interface.draw(640.0);

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
                            );
                            choose_reaction.proceed(reaction)
                        }
                        StateChooseReaction::Hit(choose_reaction) => {
                            let reaction = bot_choose_hit_reaction(
                                &choose_reaction.game,
                                choose_reaction.reactor,
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
                            });
                        }
                        StateChooseReaction::Hit(inner) => {
                            println!("awaiting player hit reaction");
                            user_interface.set_state(UiState::ReactingToHit {
                                attacker: inner.attacker,
                                victim: inner.reactor,
                                damage: inner.damage,
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
