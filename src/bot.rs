use crate::{
    core::{Action, BaseAction, CoreGame},
    pathfind::PathfindGrid,
};
use macroquad::rand;

pub fn bot_choose_action(game: &CoreGame) -> Action {
    // TODO make sure to only pick an action that the character can actually do (afford, in range, etc)
    let character = game.active_character();

    let mut actions = character.usable_actions();
    let mut chosen_action = None;

    while !actions.is_empty() {
        let i = rand::gen_range(0, actions.len());
        let action = actions.swap_remove(i);

        if character.can_use_action(action) {
            match action {
                BaseAction::Attack { hand, .. } => {
                    for (id, other_character) in game.characters.iter_with_ids() {
                        if *id == game.active_character_id {
                            continue; //Avoid borrowing already borrowed
                        }
                        if other_character.borrow().player_controlled
                            && character
                                .can_reach_with_attack(hand, other_character.borrow().position)
                        {
                            chosen_action = Some(Action::Attack {
                                hand,
                                enhancements: vec![],
                                target: *id,
                            });
                            break;
                        }
                    }
                }
                BaseAction::SelfEffect(sea) => chosen_action = Some(Action::SelfEffect(sea)),
                BaseAction::CastSpell(spell) => {
                    for (id, other_character) in game.characters.iter_with_ids() {
                        if *id == game.active_character_id {
                            continue; //Avoid borrowing already borrowed
                        }
                        if other_character.borrow().player_controlled {
                            chosen_action = Some(Action::CastSpell {
                                spell,
                                enhanced: false,
                                target: *id,
                            });
                            break;
                        }
                    }
                }
                BaseAction::Move {
                    action_point_cost,
                    range: _,
                } => {
                    let mut pathfind_grid = PathfindGrid::new();
                    for (id, character) in game.characters.iter_with_ids() {
                        if *id == game.active_character_id {
                            continue; // Avoid borrowing already borrowed active character
                        }
                        let pos = character.borrow().position;
                        pathfind_grid
                            .blocked_positions
                            .insert((pos.0 as i32, pos.1 as i32));
                    }
                    let pos = character.position;
                    pathfind_grid.run((pos.0 as i32, pos.1 as i32), character.move_range);

                    for (destination, (_distance, enter_from)) in pathfind_grid.distances {
                        if enter_from == (pos.0 as i32, pos.1 as i32) {
                            chosen_action = Some(Action::Move {
                                action_point_cost,
                                positions: vec![(destination.0 as u32, destination.1 as u32)],
                                enhancements: vec![],
                            });
                            break;
                        }
                    }
                }
            }
        }
        if chosen_action.is_some() {
            break;
        }
    }

    chosen_action.unwrap()
}
