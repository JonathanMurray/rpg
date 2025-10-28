use crate::core::{
    Action, ActionReach, CharacterId, CoreGame, OnAttackedReaction, OnHitReaction, Position,
};

pub fn bot_choose_action(game: &CoreGame) -> Option<Action> {
    let character = game.active_character();

    assert!(!character.player_controlled);

    if let Some(attack) = character.usable_attack_action() {
        for (id, other_character) in game.characters.iter_with_ids() {
            if *id == game.active_character_id {
                continue; //Avoid borrowing already borrowed
            }
            if other_character.player_controlled
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

    let mut shortest_path_to_some_player: Option<(f32, Vec<(f32, Position)>)> = None;

    for player_pos in &game.player_positions() {
        let maybe_path = game
            .pathfind_grid
            .find_path_to_adjacent(bot_pos, *player_pos);
        if let Some(path) = maybe_path {
            dbg!(bot_pos, player_pos, &path);
            if let Some(shortest) = &shortest_path_to_some_player {
                if path.0 < shortest.0 {
                    shortest_path_to_some_player = Some(path);
                }
            } else {
                shortest_path_to_some_player = Some(path);
            }
        }
    }

    if let Some(path) = shortest_path_to_some_player {
        let all_positions = path.1;

        let mut positions = vec![];
        let mut ap_cost = 0;
        for (dist, pos) in all_positions {
            if dist <= character.action_points.current() as f32 {
                positions.push(pos);
                ap_cost = dist.ceil() as u32;
            }
        }

        // It's possible that no affordable path was found, if the best path would be diagonal and that costs more than 1 AP
        if ap_cost > 0 {
            return Some(Action::Move {
                action_point_cost: ap_cost,
                positions,
                stamina_cost: 0,
            });
        }
    }

    // If a character starts its turn with 0 AP, it can't take any actions, so None is a valid case here
    None
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
