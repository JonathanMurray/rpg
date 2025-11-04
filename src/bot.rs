use std::{cell::Cell, rc::Rc};

use rand::Rng;

use crate::{
    core::{
        Action, ActionReach, ActionTarget, BaseAction, Behaviour, Character, CharacterId, CoreGame,
        OnAttackedReaction, OnHitReaction, Spell,
    },
    data::{MAGI_HEAL, MAGI_INFLICT_HORRORS, MAGI_INFLICT_WOUNDS},
    pathfind::Path,
};

#[derive(Debug, Clone, PartialEq)]
pub enum BotBehaviour {
    Normal,
    Magi(MagiBehaviour),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MagiBehaviour {
    current_goal: Cell<Option<(Spell, CharacterId)>>,
}

pub fn bot_choose_action(game: &CoreGame) -> Option<Action> {
    let character = game.active_character();
    assert!(!character.player_controlled());

    let bot_behaviour = match &character.behaviour {
        Behaviour::Player => unreachable!(),
        Behaviour::Bot(bot_behaviour) => bot_behaviour,
    };

    match bot_behaviour {
        BotBehaviour::Normal => run_normal_behaviour(game),
        BotBehaviour::Magi(magi_behaviour) => run_magi_behaviour(game, magi_behaviour),
    }
}

fn run_magi_behaviour(game: &CoreGame, behaviour: &MagiBehaviour) -> Option<Action> {
    let character = game.active_character();

    if let Some((_spell, target_id)) = behaviour.current_goal.get() {
        if !game.characters.contains(target_id) {
            dbg!("MAGI, TARGET HAS DIED?", target_id);
            behaviour.current_goal.set(None);
        }
    }

    if behaviour.current_goal.get().is_none() {
        let mut rng = rand::rng();
        let i = rng.random_range(0..2);

        let is_any_enemy_hurt = game
            .enemies()
            .any(|char| char.health.current() < char.health.max());

        let are_all_players_bleeding = game.player_characters().all(|ch| ch.is_bleeding());

        if i == 0 && is_any_enemy_hurt {
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

    let (spell, target_id) = behaviour.current_goal.get().unwrap();

    if !character.can_use_action(BaseAction::CastSpell(spell)) {
        dbg!(
            "MAGI cannot use spell; not enough AP?",
            &character.action_points
        );
        return None;
    }

    let enhancements = vec![];

    let target = game.characters.get(target_id);

    if !character.reaches_with_spell(spell, &enhancements, target.pos()) {
        let spell_range = spell.target.range(&enhancements).unwrap().into();
        let maybe_path = game.pathfind_grid.find_shortest_path_to_proximity(
            character.pos(),
            target.pos(),
            spell_range,
        );
        if let Some(path) = maybe_path {
            return convert_path_to_move_action(character, path);
        } else {
            return None;
        }
    }

    let action = Action::CastSpell {
        spell,
        enhancements,
        target: ActionTarget::Character(target_id, None),
    };
    behaviour.current_goal.set(None);
    Some(action)
}

fn run_normal_behaviour(game: &CoreGame) -> Option<Action> {
    let character = game.active_character();

    assert!(!character.player_controlled());

    if let Some(attack) = character.usable_attack_action() {
        for (id, other_character) in game.characters.iter_with_ids() {
            if *id == game.active_character_id {
                continue; //Avoid borrowing already borrowed
            }
            if other_character.player_controlled()
                && character
                    .reaches_with_attack(attack.hand, other_character.position.get())
                    .1
                    != ActionReach::No
            {
                return Some(Action::Attack {
                    hand: attack.hand,
                    enhancements: vec![],
                    target: *id,
                });
            }
        }
    }

    let bot_pos = character.position.get();

    let mut shortest_path_to_some_player: Option<Path> = None;

    for player_pos in &game.player_positions() {
        let maybe_path = game
            .pathfind_grid
            .find_shortest_path_to_adjacent(bot_pos, *player_pos);
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
    let mut ap_cost = 0;
    for (dist, pos) in path.positions {
        let cost = dist / character.move_speed();
        if cost <= character.action_points.current() as f32 {
            positions.push(pos);
            ap_cost = cost.ceil() as u32;
        }
    }

    // It's possible that no affordable path was found, if the best path would be diagonal and that costs more than 1 AP
    if ap_cost > 0 {
        Some(Action::Move {
            action_point_cost: ap_cost,
            positions,
            stamina_cost: 0,
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
