use std::cell::RefCell;
use std::rc::Rc;

use macroquad::{
    color::BLACK,
    time::get_frame_time,
    window::{clear_background, next_frame},
};

use crate::core::GameEvent;

use super::bot::bot_choose_action;
use super::bot::{bot_choose_attack_reaction, bot_choose_hit_reaction};
use super::core::{Action, CharacterId, CoreGame, HandType, OnAttackedReaction, OnHitReaction};

use super::game_ui::{GraphicalUserInterface, PlayerChose, UiState};

#[derive(Debug)]
enum UiOutcome {
    ChoseAction(Option<Action>),
    ChoseOnHitReaction(Option<OnHitReaction>),
    ChoseOnAttackedReaction(Option<OnAttackedReaction>),
    None,
}

#[derive(Debug, Clone)]
enum MessageFromGame {
    AwaitingChooseAction,
    AwaitingChooseOnAttackedReaction {
        attacker: CharacterId,
        hand: HandType,
        reactor: CharacterId,
        is_within_melee: bool,
    },
    AwaitingChooseOnHitReaction {
        attacker: CharacterId,
        reactor: CharacterId,
        damage: u32,
        is_within_melee: bool,
    },
    Event(GameEvent),
}

#[derive(Clone)]
pub struct GameUserInterface {
    inner: Rc<RefCell<Option<_GameUserInterface>>>,
}

impl GameUserInterface {
    pub fn uninitialized() -> Self {
        Self {
            inner: Rc::new(RefCell::new(None)),
        }
    }

    pub fn init(&mut self, gfx_user_interface: GraphicalUserInterface) {
        *self.inner.borrow_mut() = Some(_GameUserInterface {
            user_interface: RefCell::new(gfx_user_interface),
        });
    }

    async fn run(&self, game: &CoreGame, message: MessageFromGame) -> UiOutcome {
        let inner_ref = self.inner.borrow_mut();
        inner_ref.as_ref().unwrap().run(game, message).await
    }

    pub async fn select_action(&self, game: &CoreGame) -> Option<Action> {
        match self.run(game, MessageFromGame::AwaitingChooseAction).await {
            UiOutcome::ChoseAction(action) => action,
            unexpected => panic!("Expected action but got: {:?}", unexpected),
        }
    }

    pub async fn choose_attack_reaction(
        &self,
        game: &CoreGame,
        attacker: CharacterId,
        hand: HandType,
        reactor: CharacterId,
        within_melee: bool,
    ) -> Option<OnAttackedReaction> {
        match self
            .run(
                game,
                MessageFromGame::AwaitingChooseOnAttackedReaction {
                    attacker,
                    hand,
                    reactor,
                    is_within_melee: within_melee,
                },
            )
            .await
        {
            UiOutcome::ChoseOnAttackedReaction(reaction) => reaction,
            _ => unreachable!(),
        }
    }

    pub async fn choose_hit_reaction(
        &self,
        game: &CoreGame,
        attacker: CharacterId,
        reactor: CharacterId,
        damage: u32,
        within_melee: bool,
    ) -> Option<OnHitReaction> {
        match self
            .run(
                game,
                MessageFromGame::AwaitingChooseOnHitReaction {
                    attacker,
                    reactor,
                    damage,
                    is_within_melee: within_melee,
                },
            )
            .await
        {
            UiOutcome::ChoseOnHitReaction(reaction) => reaction,
            _ => unreachable!(),
        }
    }

    pub async fn handle_event(&self, game: &CoreGame, event: GameEvent) {
        let msg = MessageFromGame::Event(event);
        match self.run(game, msg).await {
            UiOutcome::None => {}
            _ => unreachable!(),
        }
    }
}

struct _GameUserInterface {
    user_interface: RefCell<GraphicalUserInterface>,
}

impl _GameUserInterface {
    async fn run(&self, game: &CoreGame, msg_from_game: MessageFromGame) -> UiOutcome {
        let mut user_interface = self.user_interface.borrow_mut();

        let players_turn = game.is_players_turn();

        let mut waiting_for_ui_animation_potentially = false;

        match msg_from_game {
            MessageFromGame::AwaitingChooseAction => {
                if players_turn {
                    user_interface.set_state(UiState::ChoosingAction);
                } else {
                    let action = bot_choose_action(game, user_interface.game_grid.grid_dimensions);
                    return UiOutcome::ChoseAction(action);
                }
            }
            MessageFromGame::AwaitingChooseOnAttackedReaction {
                attacker,
                hand,
                reactor,
                is_within_melee,
            } => {
                if players_turn {
                    let reaction = bot_choose_attack_reaction(game, reactor, is_within_melee);
                    return UiOutcome::ChoseOnAttackedReaction(reaction);
                } else {
                    println!("awaiting player attack reaction");
                    user_interface.set_state(UiState::ReactingToAttack {
                        attacker,
                        hand,
                        reactor,
                        is_within_melee,
                    });
                }
            }

            MessageFromGame::AwaitingChooseOnHitReaction {
                reactor,
                is_within_melee,
                attacker,
                damage,
            } => {
                if players_turn {
                    let reaction = bot_choose_hit_reaction(game, reactor, is_within_melee);
                    return UiOutcome::ChoseOnHitReaction(reaction);
                } else {
                    println!("awaiting player hit reaction");
                    user_interface.set_state(UiState::ReactingToHit {
                        attacker,
                        victim: reactor,
                        damage,
                        is_within_melee,
                    });
                }
            }

            MessageFromGame::Event(event) => {
                waiting_for_ui_animation_potentially = true;
                user_interface.handle_game_event(event);
            }
        }

        loop {
            let elapsed = get_frame_time();

            let player_choice = user_interface.update(game, elapsed);

            clear_background(BLACK);
            user_interface.draw();

            if let Some(player_choice) = player_choice {
                user_interface.set_state(UiState::Idle);
                match player_choice {
                    PlayerChose::AttackedReaction(reaction) => {
                        return UiOutcome::ChoseOnAttackedReaction(reaction);
                    }
                    PlayerChose::HitReaction(reaction) => {
                        return UiOutcome::ChoseOnHitReaction(reaction);
                    }
                    PlayerChose::Action(action) => {
                        dbg!(&action);
                        // TODO: Add option in UI to deliberately end turn
                        return UiOutcome::ChoseAction(Some(action));
                    }
                }
            }

            if waiting_for_ui_animation_potentially && !user_interface.has_ongoing_animation() {
                return UiOutcome::None;
            }

            next_frame().await
        }
    }
}
