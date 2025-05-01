use crate::{
    core::{
        Action, ActionReach, ActionTarget, BaseAction, CharacterId, CoreGame, OnAttackedReaction,
        OnHitReaction,
    },
    pathfind::PathfindGrid,
};
use macroquad::rand;

pub fn bot_choose_action(game: &CoreGame, grid_dimensions: (u32, u32)) -> Option<Action> {
    let character = game.active_character();

    assert!(!character.player_controlled);

    let mut actions = character.usable_actions();
    let mut chosen_action = None;

    while !actions.is_empty() {
        let i = rand::gen_range(0, actions.len());
        let action = actions.swap_remove(i);

        if character.can_use_action(action) {
            match action {
                BaseAction::Attack { hand, .. } => {
                    // TODO
                    continue;

                    for (id, other_character) in game.characters.iter_with_ids() {
                        if *id == game.active_character_id {
                            continue; //Avoid borrowing already borrowed
                        }
                        if other_character.player_controlled
                            && character
                                .reaches_with_attack(hand, other_character.position.get())
                                .1
                                != ActionReach::No
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
                BaseAction::CastSpell(spell) => {
                    for (id, other_character) in game.characters.iter_with_ids() {
                        if other_character.player_controlled {
                            chosen_action = Some(Action::CastSpell {
                                spell,
                                enhancements: vec![],
                                target: ActionTarget::Character(*id, None),
                            });
                            break;
                        }
                    }
                }
                BaseAction::Move => {
                    let mut pathfind_grid = PathfindGrid::new(grid_dimensions);
                    for (id, character) in game.characters.iter_with_ids() {
                        if *id == game.active_character_id {
                            continue; // Avoid borrowing already borrowed active character
                        }
                        let pos = character.position.get();
                        pathfind_grid.blocked_positions.insert((pos.0, pos.1));
                    }
                    let pos = character.position.get();
                    pathfind_grid.run((pos.0, pos.1), character.move_speed);

                    for (destination, route) in pathfind_grid.routes {
                        if route.came_from == (pos.0, pos.1) && route.distance_from_start > 0.0 {
                            let destination = (destination.0, destination.1);
                            assert!(destination != pos);
                            chosen_action = Some(Action::Move {
                                action_point_cost: 1,
                                positions: vec![destination],
                                stamina_cost: 0,
                            });
                            break;
                        }
                    }
                }
                BaseAction::ChangeEquipment | BaseAction::EndTurn => {}
            }
        }
        if chosen_action.is_some() {
            break;
        }
    }

    // If a character starts its turn with 0 AP, it can't take any actions, so None is a valid case here
    chosen_action
}

pub fn bot_choose_attack_reaction(
    game: &CoreGame,
    reactor_id: CharacterId,
    is_within_melee: bool,
) -> Option<OnAttackedReaction> {
    let reactions = game
        .characters
        .get(reactor_id)
        .usable_on_attacked_reactions(is_within_melee);
    if let Some((_, reaction)) = reactions.first() {
        Some(*reaction)
    } else {
        None
    }
}

pub fn bot_choose_hit_reaction(
    game: &CoreGame,
    reactor_id: CharacterId,
    is_within_melee: bool,
) -> Option<OnHitReaction> {
    let reactions = game
        .characters
        .get(reactor_id)
        .usable_on_hit_reactions(is_within_melee);
    if let Some((_, reaction)) = reactions.first() {
        Some(*reaction)
    } else {
        None
    }
}
