use core::f32;
use std::{cell::Cell, iter, rc::Rc};

use macroquad::rand::ChooseRandom;
use rand::{random_range, Rng};

use crate::{
    core::{
        Ability, AbilityId, AbilityTarget, Action, ActionReach, ActionTarget, BaseAction,
        Character, CharacterId, Condition, CoreGame, HandType, OnAttackedReaction, OnHitReaction,
        Position,
    },
    data::{MAGI_HEAL, MAGI_INFLICT_HORRORS, MAGI_INFLICT_WOUNDS},
    pathfind::Path,
    util::{adjacent_cells, are_entities_within_melee, CustomShuffle},
};

#[derive(Debug, Clone, PartialEq)]
pub enum BotBehaviour {
    Normal,
    Magi(MagiBehaviour),
    Fighter(FighterBehaviour),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MagiBehaviour {
    current_goal: Cell<Option<(Ability, CharacterId)>>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct FighterBehaviour {
    current_target: Cell<Option<CharacterId>>,
    chance_of_switching_target: Cell<f32>,
}

const EXPLORATION_RANGE: f32 = 60.0;

pub fn bot_choose_action(game: &CoreGame) -> Option<Action> {
    let character = game.active_character();
    assert!(!character.player_controlled());

    dbg!("BOT CHOOSING ACTION ...");

    let result = match character.kind.unwrap_bot_behaviour() {
        BotBehaviour::Normal => run_normal_behaviour(game),
        BotBehaviour::Magi(magi) => run_magi_behaviour(game, magi),
        BotBehaviour::Fighter(fighter) => run_fighter_behaviour(game, fighter),
    };
    println!("Bot chose: {:?}", result);

    result
}

fn run_fighter_behaviour(game: &CoreGame, behaviour: &FighterBehaviour) -> Option<Action> {
    println!("--------------------");
    println!("Run fighter behaviour ({})", game.active_character_id);
    println!("--------------------");
    let bot = game.active_character();
    assert!(!bot.player_controlled());

    println!("bot AP: {}", bot.action_points.current());

    let bot_pos = bot.position.get();

    let attack = bot.attack_action().unwrap();
    let weapon_range = bot.weapon(attack.hand).unwrap().range;

    let mut player_chars: Vec<&Rc<Character>> = game.player_characters().collect();

    if random_range(0.0..1.0) < 0.5 {
        println!("Sort player chars by proximity");
        player_chars.sort_by_key(|ch| {
            let distance_to = game
                .pathfind_grid
                .find_shortest_path_to_proximity(
                    bot.id(),
                    bot_pos,
                    ch.pos(),
                    weapon_range.center_to_center_squared(),
                    EXPLORATION_RANGE,
                )
                .map(|p| p.total_distance)
                .unwrap_or(f32::MAX);

            // convert from f32 to make it sortable
            (distance_to * 10.0) as u32
        });
    } else {
        println!("Shuffle player chars");
        CustomShuffle::shuffle(&mut player_chars);
    }

    if let Some(target_id) = behaviour.current_target.get() {
        if !player_chars.iter().any(|ch| ch.id() == target_id) {
            // Player char must have died. Force a target switch.
            behaviour.current_target.set(None);
        }
    }

    if behaviour.current_target.get().is_none() {
        // TODO: this panics if all player chars have died
        behaviour.current_target.set(Some(player_chars[0].id()));
    }

    let mut target_id = behaviour.current_target.get().unwrap();

    let chance_switch_target = behaviour.chance_of_switching_target.get();
    dbg!(chance_switch_target);
    let switch_target = random_range(0.0..1.0) < chance_switch_target;
    dbg!(switch_target);
    if switch_target {
        behaviour.chance_of_switching_target.set(0.0);
    } else if behaviour.chance_of_switching_target.get() == 0.0 {
        // Make it very rare to switch target immediately after acquiring it
        behaviour.chance_of_switching_target.set(0.01);
    } else {
        // After that, the chance increases steadily
        behaviour
            .chance_of_switching_target
            .set(chance_switch_target + 0.1);
    }

    if switch_target {
        println!("bot should try switching target");
        if let Some(new_target) = player_chars.iter().find(|ch| ch.id() != target_id) {
            println!("switching to new target?: {}", new_target.id());

            let maybe_path = game.pathfind_grid.find_shortest_path_to_proximity(
                bot.id(),
                bot_pos,
                new_target.pos(),
                weapon_range.center_to_center_squared(),
                EXPLORATION_RANGE,
            );

            if maybe_path.is_some() {
                println!("Yes, there's a path to it ");

                behaviour.current_target.set(Some(new_target.id()));
                target_id = new_target.id();
            } else {
                println!("no, no path to it");
            }
        }
    } else {
        println!(
            "bot sticks with current target: {:?}",
            behaviour.current_target
        );
    }

    let target = player_chars.iter().find(|ch| ch.id() == target_id).unwrap();

    let mut candidates = vec![];
    if bot.can_attack(attack) {
        candidates.push(BotAction::Attack);
    }
    for a in bot.usable_single_target_abilities() {
        if may_use(bot, a) {
            candidates.push(BotAction::SingleTargetAbility(a));
        }
    }
    for a in bot.usable_abilities() {
        if may_use(bot, a) {
            let candidate = match a.target {
                AbilityTarget::Enemy { .. } => BotAction::SingleTargetAbility(a),
                AbilityTarget::None { .. } => BotAction::NonTargetAbility(a),
                unhandled => todo!("{:?}", unhandled),
            };
            candidates.push(candidate);
        }
    }
    CustomShuffle::shuffle(&mut candidates);

    //dbg!(&candidates);

    if let Some(preferred_action) = candidates.first().copied() {
        //dbg!(("bot preferred action", preferred_action));
        match preferred_action {
            BotAction::Attack => {
                if attack_reaches(bot, target) {
                    println!("bot attacks target");
                    return Some(simple_attack_action(target));
                }
            }
            BotAction::SingleTargetAbility(ability) => {
                if bot.reaches_with_ability(ability, &[], target.pos()) {
                    println!("bot uses ability on target");
                    return Some(simple_targetted_ability_action(ability, target));
                }
            }
            BotAction::NonTargetAbility(ability) => {
                return Some(Action::UseAbility {
                    ability,
                    enhancements: vec![],
                    target: ActionTarget::None,
                });
            }
        }
    }

    let maybe_path = game.pathfind_grid.find_shortest_path_to_proximity(
        bot.id(),
        bot_pos,
        target.pos(),
        weapon_range.center_to_center_squared(),
        EXPLORATION_RANGE,
    );

    if let Some(path) = maybe_path {
        if bot.remaining_movement.get() < path.total_distance {
            println!(
                "Bot will not reach target this turn; look for other things to do before moving"
            );

            for action in candidates {
                match action {
                    BotAction::Attack => {
                        for player_char in &player_chars {
                            if attack_reaches(bot, player_char) {
                                println!("bot attacks someone before moving to target");
                                return Some(simple_attack_action(player_char));
                            }
                        }
                    }
                    BotAction::SingleTargetAbility(ability) => {
                        for player_char in &player_chars {
                            if bot.reaches_with_ability(ability, &[], player_char.pos()) {
                                println!("bot uses ability on someone before moving to target");
                                return Some(simple_targetted_ability_action(ability, player_char));
                            }
                        }
                    }
                    BotAction::NonTargetAbility(ability) => {
                        println!("bot uses nontargeted ability before moving to target");
                        return Some(Action::UseAbility {
                            ability,
                            enhancements: vec![],
                            target: ActionTarget::None,
                        });
                    }
                }
            }
        }

        println!("BOT MOVING PATH: {:?}", path);
        return convert_path_to_move_action(bot, path);
    } else {
        println!("bot finds no path to target");
    }

    println!("No bot action");

    // If a character starts its turn with 0 AP, it can't take any actions, so None is a valid case here
    None
}

#[derive(Copy, Clone, Debug)]
enum BotAction {
    Attack,
    SingleTargetAbility(Ability),
    NonTargetAbility(Ability),
}

fn simple_attack_action(target: &Character) -> Action {
    Action::Attack {
        hand: HandType::MainHand,
        enhancements: vec![],
        target: target.id(),
    }
}

fn simple_targetted_ability_action(ability: Ability, target: &Character) -> Action {
    Action::UseAbility {
        ability,
        enhancements: vec![],
        target: ActionTarget::Character(target.id(), None),
    }
}

fn attack_reaches(bot: &Character, target: &Character) -> bool {
    bot.reaches_with_attack(HandType::MainHand, target.pos(), iter::empty())
        .1
        != ActionReach::No
}

fn run_normal_behaviour(game: &CoreGame) -> Option<Action> {
    let bot = game.active_character();
    assert!(!bot.player_controlled());

    let mut attack_range = None;

    let is_ranged_attacker = bot
        .weapon(HandType::MainHand)
        .map(|weapon| !weapon.is_melee())
        .unwrap_or(false);

    let mut player_chars: Vec<&Rc<Character>> = game.player_characters().collect();

    let bot_pos = bot.position.get();

    // Flee out of melee
    if is_ranged_attacker {
        if let Some(adj_player_char) = player_chars
            .iter()
            .find(|ch| are_entities_within_melee(bot_pos, ch.pos()))
        {
            let safe_adjacent_positions: Vec<Position> = adjacent_cells(bot_pos)
                .into_iter()
                .filter(|pos| {
                    game.pathfind_grid.is_free_for(bot.id(), *pos)
                        && !player_chars
                            .iter()
                            .any(|ch| are_entities_within_melee(ch.pos(), *pos))
                })
                .collect();

            if let Some(safe_pos) = ChooseRandom::choose(&safe_adjacent_positions[..]) {
                if let Some(path) =
                    game.pathfind_grid
                        .find_shortest_path_to(bot.id(), bot_pos, *safe_pos)
                {
                    println!("Bot flees from {}: {:?}", adj_player_char.name, path);
                    return convert_path_to_move_action(bot, path);
                }
            }
        }
    }

    if let Some(attack) = bot.attack_action() {
        attack_range = Some(bot.weapon(attack.hand).unwrap().range);
        CustomShuffle::shuffle(&mut player_chars);
        for player_char in player_chars {
            if bot
                .reaches_with_attack(attack.hand, player_char.position.get(), iter::empty())
                .1
                != ActionReach::No
            {
                if bot.can_attack(attack) {
                    return Some(simple_attack_action(player_char));
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
                bot.id(),
                bot_pos,
                *player_pos,
                range.center_to_center_squared(),
                EXPLORATION_RANGE,
            )
        } else {
            game.pathfind_grid.find_shortest_path_to_adjacent(
                bot.id(),
                bot_pos,
                *player_pos,
                EXPLORATION_RANGE,
            )
        };

        if let Some(path) = maybe_path {
            //dbg!(bot_pos, player_pos, &path);
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
        return convert_path_to_move_action(bot, path);
    }

    // If a character starts its turn with 0 AP, it can't take any actions, so None is a valid case here
    None
}

fn run_magi_behaviour(game: &CoreGame, behaviour: &MagiBehaviour) -> Option<Action> {
    let bot = game.active_character();

    if let Some((_, target_id)) = behaviour.current_goal.get() {
        if !game.characters.contains_alive(target_id) {
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

    if !bot.can_use_action(BaseAction::UseAbility(ability)) {
        dbg!(
            "MAGI cannot use ability; not enough AP?",
            &bot.action_points
        );
        return None;
    }

    let enhancements = vec![];

    let target = game.characters.get(target_id);

    if !bot.reaches_with_ability(ability, &enhancements, target.pos()) {
        let ability_range_sq = ability
            .target
            .range(&enhancements)
            .unwrap()
            .center_to_center_squared();
        let maybe_path = game.pathfind_grid.find_shortest_path_to_proximity(
            bot.id(),
            bot.pos(),
            target.pos(),
            ability_range_sq,
            EXPLORATION_RANGE,
        );
        if let Some(path) = maybe_path {
            return convert_path_to_move_action(bot, path);
        } else {
            return None;
        }
    }

    let action = simple_targetted_ability_action(ability, target);

    behaviour.current_goal.set(None);
    Some(action)
}

pub fn convert_path_to_move_action(character: &Character, path: Path) -> Option<Action> {
    let remaining_free_movement = character.remaining_movement.get();
    dbg!(remaining_free_movement);
    //let max_sprint_usage = character.stamina.current();
    let mut positions = vec![];
    let mut total_distance = 0.0;
    for (dist, pos) in path.positions {
        if dist <= remaining_free_movement {
            positions.push(pos);
            total_distance = dist;
        }
    }

    let extra_cost = 0; // ((total_distance - remaining_free_movement).ceil() as u32).max(0);

    if total_distance > 0.0 {
        Some(Action::Move {
            total_distance,
            positions,
            extra_cost,
        })
    } else {
        None
    }
}

fn may_use(bot: &Character, ability: Ability) -> bool {
    if ability.id == AbilityId::Brace
        && bot.conditions.borrow().get_stacks(&Condition::Protected) > 0
    {
        return false;
    }
    if ability.id == AbilityId::Inspire && bot.conditions.borrow().has(&Condition::Inspired) {
        return false;
    }
    true
}

pub fn bot_choose_attack_reaction(
    game: &CoreGame,
    reactor_id: CharacterId,
    is_within_melee: bool,
) -> Option<OnAttackedReaction> {
    // TODO: it needs to be more intuitive/clear for player how/when/why bot reacts
    return None;
}

pub fn bot_choose_hit_reaction(
    game: &CoreGame,
    reactor_id: CharacterId,
    is_within_melee: bool,
) -> Option<OnHitReaction> {
    // TODO: it needs to be more intuitive/clear for player how/when/why bot reacts
    return None;

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
