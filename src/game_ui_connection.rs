use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::{cell::RefCell, sync::atomic::Ordering};

use macroquad::{
    color::BLACK,
    input::{get_keys_pressed, KeyCode},
    time::get_frame_time,
    window::{clear_background, next_frame},
};

use crate::core::{GameEvent, Position};

use super::bot::bot_choose_action;
use super::bot::{bot_choose_attack_reaction, bot_choose_hit_reaction};
use super::core::{Action, CharacterId, CoreGame, HandType, OnAttackedReaction, OnHitReaction};

use super::game_ui::{PlayerChose, UiState, UserInterface};

pub static DEBUG: AtomicBool = AtomicBool::new(true);

#[derive(Debug)]
enum UiOutcome {
    ChoseAction(Option<Action>),
    ChoseOnHitReaction(Option<OnHitReaction>),
    ChoseOnAttackedReaction(Option<OnAttackedReaction>),
    ChoseOpportunityAttack(bool),
    SwitchedTo(CharacterId),
    None,
}

#[derive(Debug)]
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
    AwaitingChooseMovementOpportunityAttack {
        reactor: CharacterId,
        // TODO characterId?
        target: u32,
        movement: ((i32, i32), (i32, i32)),
    },
    AwaitingChooseRangedOpportunityAttack {
        reactor: CharacterId,
        attacker: CharacterId,
        victim: CharacterId,
    },
    Event(GameEvent),
}

pub enum ActionOrSwitchTo {
    Action(Option<Action>),
    SwitchTo(CharacterId),
}

#[derive(Clone)]
pub struct GameUserInterfaceConnection {
    inner: Rc<RefCell<Option<_GameUserInterfaceConnection>>>,
}

impl GameUserInterfaceConnection {
    pub fn uninitialized() -> Self {
        Self {
            inner: Rc::new(RefCell::new(None)),
        }
    }

    pub fn init(&mut self, gfx_user_interface: UserInterface) {
        *self.inner.borrow_mut() = Some(_GameUserInterfaceConnection {
            user_interface: RefCell::new(gfx_user_interface),
        });
    }

    async fn run_ui(&self, game: &CoreGame, message: MessageFromGame) -> UiOutcome {
        println!("run_ui ...");
        let inner_ref = self.inner.borrow_mut();
        println!("have inner ref.");
        inner_ref.as_ref().unwrap().run_ui(game, message).await
    }

    pub async fn select_action(&self, game: &CoreGame) -> ActionOrSwitchTo {
        match self
            .run_ui(game, MessageFromGame::AwaitingChooseAction)
            .await
        {
            UiOutcome::ChoseAction(action) => ActionOrSwitchTo::Action(action),
            UiOutcome::SwitchedTo(character_id) => ActionOrSwitchTo::SwitchTo(character_id),
            unexpected => panic!("Expected action (or char change) but got: {:?}", unexpected),
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
            .run_ui(
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
            .run_ui(
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

    pub async fn choose_movement_opportunity_attack(
        &self,
        game: &CoreGame,
        reactor: CharacterId,
        target: CharacterId,
        movement: (Position, Position),
    ) -> bool {
        match self
            .run_ui(
                game,
                MessageFromGame::AwaitingChooseMovementOpportunityAttack {
                    reactor,
                    target,
                    movement,
                },
            )
            .await
        {
            UiOutcome::ChoseOpportunityAttack(choice) => choice,
            _ => unreachable!(),
        }
    }

    pub async fn choose_ranged_opportunity_attack(
        &self,
        game: &CoreGame,
        reactor: CharacterId,
        attacker: CharacterId,
        victim: CharacterId,
    ) -> bool {
        match self
            .run_ui(
                game,
                MessageFromGame::AwaitingChooseRangedOpportunityAttack {
                    reactor,
                    attacker,
                    victim,
                },
            )
            .await
        {
            UiOutcome::ChoseOpportunityAttack(choice) => choice,
            _ => unreachable!(),
        }
    }

    pub async fn handle_event(&self, game: &CoreGame, event: GameEvent) {
        let msg = MessageFromGame::Event(event);
        match self.run_ui(game, msg).await {
            UiOutcome::None => {}
            _ => unreachable!(),
        }
    }
}

struct _GameUserInterfaceConnection {
    user_interface: RefCell<UserInterface>,
}

impl _GameUserInterfaceConnection {
    async fn run_ui(&self, game: &CoreGame, msg_from_game: MessageFromGame) -> UiOutcome {

        println!("inner run_ui ...");

        let mut user_interface = self.user_interface.borrow_mut();

        let players_turn = game.is_players_turn();

        let mut waiting_for_ui_animation_potentially = false;

        match msg_from_game {
            MessageFromGame::AwaitingChooseAction => {
                if players_turn {
                    user_interface.set_state(UiState::ChoosingAction);
                } else {
                    let action = bot_choose_action(game);
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
                        selected: None,
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
                        selected: None,
                    });
                }
            }

            MessageFromGame::AwaitingChooseMovementOpportunityAttack {
                reactor,
                target,
                movement,
            } => {
                if players_turn {
                    // TODO
                    return UiOutcome::ChoseOpportunityAttack(true);
                } else {
                    user_interface.set_state(UiState::ReactingToMovementAttackOpportunity {
                        reactor,
                        target,
                        movement,
                        selected: false,
                    });
                }
            }

            MessageFromGame::AwaitingChooseRangedOpportunityAttack {
                reactor,
                attacker,
                victim,
            } => {
                if players_turn {
                    // TODO
                    return UiOutcome::ChoseOpportunityAttack(true);
                } else {
                    user_interface.set_state(UiState::ReactingToRangedAttackOpportunity {
                        reactor,
                        attacker,
                        victim,
                        selected: false,
                    });
                }
            }

            MessageFromGame::Event(event) => {
                waiting_for_ui_animation_potentially = true;
                println!("BEFORE CALLING handle_game_event({:?})", event);
                user_interface.handle_game_event(event);
            }
        }

        loop {
            let elapsed = get_frame_time();

            let mut player_choice = user_interface.update(game, elapsed);

            clear_background(BLACK);

            if let Some(choice) = user_interface.draw() {
                assert!(
                    player_choice.is_none(),
                    "Conflicting player choices: {:?} and {:?}",
                    player_choice,
                    choice
                );
                player_choice = Some(choice);
            }

            if get_keys_pressed().contains(&KeyCode::Space) {
                DEBUG.fetch_not(Ordering::SeqCst);
                dbg!(DEBUG.load(Ordering::SeqCst));
            }

            if let Some(player_choice) = player_choice {
                user_interface.set_state(UiState::Idle);
                // Need to call next_frame here, to make sure UI events aren't lingering when
                // PlayerChose::SwitchTo leads us back into selecting the action for the newly
                // selected character (?)
                next_frame().await;
                match player_choice {
                    PlayerChose::AttackedReaction(reaction) => {
                        return UiOutcome::ChoseOnAttackedReaction(reaction);
                    }
                    PlayerChose::HitReaction(reaction) => {
                        return UiOutcome::ChoseOnHitReaction(reaction);
                    }
                    PlayerChose::OpportunityAttack(choice) => {
                        return UiOutcome::ChoseOpportunityAttack(choice)
                    }
                    PlayerChose::Action(action) => {
                        return UiOutcome::ChoseAction(action);
                    }
                    PlayerChose::SwitchTo(character_id) => {
                        return UiOutcome::SwitchedTo(character_id)
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
