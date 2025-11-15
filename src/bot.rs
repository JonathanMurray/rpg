use std::{cell::Cell, iter, rc::Rc};

use macroquad::rand::ChooseRandom;
use rand::Rng;

use crate::{
    core::{
        Ability, Action, ActionReach, ActionTarget, BaseAction, Character, CharacterId, CoreGame,
        HandType, OnAttackedReaction, OnHitReaction, Position,
    },
    data::{MAGI_HEAL, MAGI_INFLICT_HORRORS, MAGI_INFLICT_WOUNDS},
    pathfind::Path,
    util::{adjacent_positions, are_adjacent},
};

#[derive(Debug, Clone, PartialEq)]
pub enum BotBehaviour {
    Normal,
    Magi(MagiBehaviour),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MagiBehaviour {
    current_goal: Cell<Option<(Ability, CharacterId)>>,
}

const EXPLORATION_RANGE: f32 = 20.0;

pub fn bot_choose_action(game: &CoreGame) -> Option<Action> {
    let character = game.active_character();
    assert!(!character.player_controlled());

    match character.behaviour.unwrap_bot_behaviour() {
        BotBehaviour::Normal => run_normal_behaviour(game),
        BotBehaviour::Magi(magi_behaviour) => run_magi_behaviour(game, magi_behaviour),
    }
}

fn run_magi_behaviour(game: &CoreGame, behaviour: &MagiBehaviour) -> Option<Action> {
    let character = game.active_character();

    if let Some((_, target_id)) = behaviour.current_goal.get() {
        if !game.characters.contains(target_id) {
            dbg!("MAGI, TARGET HAS DIED?", target_id);
            behaviour.current_goal.set(None);
        }
    }

    if behaviour.current_goal.get().is_none() {
        let mut rng = rand::rng();
        let i = rng.random_range(0..2);

        let is_healing_warranted = game
            .enemies()
            .any(|char| char.health.current() < char.health.max() - 2);

        let are_all_players_bleeding = game.player_characters().all(|ch| ch.is_bleeding());

        if i == 0 && is_healing_warranted {
            let target: &Rc<Character> = game
                .enemies()
                .min_by(|a, b| a.health.ratio().total_cmp(&b.health.ratio()))
                .unwrap();

            behaviour.current_goal.set(Some((MAGI_HEAL, target.id())));
            dbg!("NEW MAGI HEAL GOAL: {:?}", target.id());
        } else if !are_all_players_bleeding {
            let non_bleeding_player_chars: Vec<&Rc<Character>> = game
                .player_characters()
                .filter(|ch| !ch.is_bleeding())
                .collect();
            let target =
                non_bleeding_player_chars[rng.random_range(0..non_bleeding_player_chars.len())];

            behaviour
                .current_goal
                .set(Some((MAGI_INFLICT_WOUNDS, target.id())));
            dbg!("NEW MAGI WOUND GOAL: {:?}", target.id());
        } else {
            let player_chars: Vec<&Rc<Character>> = game.player_characters().collect();
            let target = player_chars[rng.random_range(0..player_chars.len())];

            behaviour
                .current_goal
                .set(Some((MAGI_INFLICT_HORRORS, target.id())));
            dbg!("NEW MAGI HORROR GOAL: {:?}", target.id());
        }
    }

    let (ability, target_id) = behaviour.current_goal.get().unwrap();

    if !character.can_use_action(BaseAction::UseAbility(ability)) {
        dbg!(
            "MAGI cannot use ability; not enough AP?",
            &character.action_points
        );
        return None;
    }

    let enhancements = vec![];

    let target = game.characters.get(target_id);

    if !character.reaches_with_ability(ability, &enhancements, target.pos()) {
        let ability_range = ability.target.range(&enhancements).unwrap().into();
        let maybe_path = game.pathfind_grid.find_shortest_path_to_proximity(
            character.pos(),
            target.pos(),
            ability_range,
            EXPLORATION_RANGE,
        );
        if let Some(path) = maybe_path {
            return convert_path_to_move_action(character, path);
        } else {
            return None;
        }
    }

    let action = Action::UseAbility {
        ability,
        enhancements,
        target: ActionTarget::Character(target_id, None),
    };
    behaviour.current_goal.set(None);
    Some(action)
}

fn run_normal_behaviour(game: &CoreGame) -> Option<Action> {
    let character = game.active_character();
    assert!(!character.player_controlled());

    let mut attack_range = None;

    let is_ranged_attacker = character
        .weapon(HandType::MainHand)
        .map(|weapon| !weapon.is_melee())
        .unwrap_or(false);

    let mut player_chars: Vec<&Rc<Character>> = game.player_characters().collect();

    let bot_pos = character.position.get();

    if is_ranged_attacker {
        if let Some(adj_player_char) = player_chars
            .iter()
            .find(|ch| are_adjacent(bot_pos, ch.pos()))
        {
            let safe_adjacent_positions: Vec<Position> = adjacent_positions(bot_pos)
                .into_iter()
                .filter(|pos| {
                    !game.pathfind_grid.blocked_positions().contains(pos)
                        && !player_chars.iter().any(|ch| are_adjacent(ch.pos(), *pos))
                })
                .collect();

            if let Some(safe_pos) = ChooseRandom::choose(&safe_adjacent_positions[..]) {
                if let Some(path) = game.pathfind_grid.find_shortest_path_to(bot_pos, *safe_pos) {
                    println!("Bot flees from {}: {:?}", adj_player_char.name, path);
                    return convert_path_to_move_action(character, path);
                }
            }
        }
    }

    if let Some(attack) = character.attack_action() {
        attack_range = Some(character.weapon(attack.hand).unwrap().range);
        ChooseRandom::shuffle(&mut player_chars[..]);
        for player_char in player_chars {
            if character
                .attack_reaches(attack.hand, player_char.position.get(), iter::empty())
                .1
                != ActionReach::No
            {
                if character.can_attack(attack) {
                    return Some(Action::Attack {
                        hand: attack.hand,
                        enhancements: vec![],
                        target: player_char.id(),
                    });
                } else {
                    println!("bot reaches a player char but doesn't have enough AP to attack. Let it chill.");
                    return None;
                }
            }
        }
    }

    let mut shortest_path_to_some_player: Option<Path> = None;

    for player_pos in &game.player_positions() {
        let maybe_path = if let Some(range) = attack_range {
            game.pathfind_grid.find_shortest_path_to_proximity(
                bot_pos,
                *player_pos,
                range.into_range().into(),
                EXPLORATION_RANGE,
            )
        } else {
            game.pathfind_grid.find_shortest_path_to_adjacent(
                bot_pos,
                *player_pos,
                EXPLORATION_RANGE,
            )
        };

        if let Some(path) = maybe_path {
            dbg!(bot_pos, player_pos, &path);
            if let Some(shortest) = &shortest_path_to_some_player {
                if path.total_distance < shortest.total_distance {
                    shortest_path_to_some_player = Some(path);
                }
            } else {
                shortest_path_to_some_player = Some(path);
            }
        }
    }

    if let Some(path) = shortest_path_to_some_player {
        return convert_path_to_move_action(character, path);
    }

    // If a character starts its turn with 0 AP, it can't take any actions, so None is a valid case here
    None
}

pub fn convert_path_to_move_action(character: &Character, path: Path) -> Option<Action> {
    let mut positions = vec![];
    let mut total_distance = 0.0;
    for (dist, pos) in path.positions {
        if dist <= character.remaining_movement.get() {
            positions.push(pos);
            total_distance = dist;
        }
    }

    if total_distance > 0.0 {
        Some(Action::Move {
            total_distance,
            positions,
            extra_cost: 0,
        })
    } else {
        None
    }
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
