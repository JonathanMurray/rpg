use std::{cell::RefCell, collections::HashSet, io, rc::Rc};

use rpg::core::{
    as_percentage, prob_ability_hit, prob_attack_hit, Action, BaseAction, Character, CoreGame,
    GameState, HandType, Logger, OnAttackedReaction, OnHitReaction, SelfEffectAction,
};

fn main() {
    let waiting_for = CoreGame::new(Rc::new(RefCell::new(StdoutLogger)));
    let mut waiting_for = GameState::AwaitingChooseAction(waiting_for);

    loop {
        match waiting_for {
            GameState::AwaitingChooseAction(state) => {
                let active_character = state.game.active_character();
                let other_character = state.game.inactive_character();
                let action = player_choose_action(&active_character, &other_character);
                drop(active_character);
                drop(other_character);
                waiting_for = state.proceed(action);
            }
            GameState::AwaitingChooseAttackReaction(state) => {
                let defender = state.game.inactive_character();
                let reaction = player_choose_on_attacked_reaction(&defender);
                drop(defender);
                waiting_for = state.proceed(reaction);
            }
            GameState::AwaitingChooseHitReaction(state) => {
                let defender = state.game.inactive_character();
                let reaction = player_choose_on_hit_reaction(&defender);
                drop(defender);
                waiting_for = state.proceed(reaction);
            }
        }
    }
}

struct StdoutLogger;

impl Logger for StdoutLogger {
    fn log(&mut self, line: String) {
        println!("{line}")
    }
}

pub fn player_choose_action(character: &Character, other_character: &Character) -> Action {
    let available_actions = character.usable_actions();

    println!("Choose an action:");
    for (i, action) in available_actions.iter().enumerate() {
        let label = match action {
            BaseAction::Attack {
                hand,
                action_point_cost,
            } => {
                let weapon = character.weapon(*hand).unwrap();
                let label = match hand {
                    HandType::MainHand => "Attack",
                    HandType::OffHand => "Off-hand attack",
                };
                format!(
                    "[{}: {}] ({} AP, hit={})",
                    weapon.name,
                    label,
                    weapon.action_point_cost,
                    as_percentage(prob_attack_hit(character, *hand, other_character))
                )
            }

            BaseAction::SelfEffect(SelfEffectAction {
                name,
                action_point_cost,
                effect: _,
            }) => {
                format!("[{}] ({} AP)", name, action_point_cost)
            }
            BaseAction::UseAbility(spell) => format!(
                "[{}] ({} AP, {} mana, hit={})",
                spell.name,
                spell.action_point_cost,
                spell.mana_cost,
                as_percentage(prob_ability_hit(
                    character,
                    spell.spell_type,
                    other_character
                ))
            ),
        };
        println!("  {}. {}", i + 1, label);
    }
    let action_choice = player_make_choice(available_actions.len() as u32);

    match &available_actions[action_choice as usize - 1] {
        BaseAction::Attack {
            hand,
            action_point_cost: _,
        } => {
            let weapon = character.weapon(*hand).unwrap();
            let reserved_action_points = weapon.action_point_cost;

            let available_attack_enhancements = character.usable_attack_enhancements(*hand);
            let mut picked_attack_enhancements = vec![];

            if !available_attack_enhancements.is_empty() {
                println!(
                    "({} AP, {}/{} stamina)",
                    character.action_points - reserved_action_points,
                    character.stamina.current,
                    character.stamina.max
                );
                println!(
                    "Add attack enhancements (whitespace-separated numbers; empty line to skip)"
                );

                for (i, (prefix, enhancement)) in available_attack_enhancements.iter().enumerate() {
                    let cost = match (enhancement.action_point_cost, enhancement.stamina_cost) {
                        (ap, 0) if ap > 0 => format!("{} AP", ap),
                        (0, sta) => format!("{} sta", sta),
                        (ap, sta) => format!("{} AP, {} sta", ap, sta),
                    };
                    println!("  {}. [{}{}] ({})", i + 1, prefix, enhancement.name, cost);
                }

                let stdin = io::stdin();
                let input = &mut String::new();
                loop {
                    input.clear();
                    stdin.read_line(input).unwrap();
                    let line = input.trim_end_matches("\r\n").trim_end_matches('\n');
                    if line.is_empty() {
                        // player picked no enhancements
                        break;
                    }
                    let picked_numbers = line
                        .split_whitespace()
                        .map(|w| w.parse::<u32>())
                        .filter_map(|res| res.ok())
                        .collect::<HashSet<_>>();
                    if picked_numbers.is_empty() {
                        println!("Invalid input. Provide valid numbers, or an empty line.");
                        continue;
                    }

                    let total_cost: u32 = picked_numbers
                        .iter()
                        .map(|&i| {
                            available_attack_enhancements[i as usize - 1]
                                .1
                                .action_point_cost
                        })
                        .sum();

                    if character.action_points - reserved_action_points >= total_cost {
                        for i in picked_numbers {
                            picked_attack_enhancements
                                .push(available_attack_enhancements[i as usize - 1].1);
                        }
                        break;
                    } else {
                        println!("Invalid input. Not enough action points.");
                        continue;
                    }
                }
            }

            Action::Attack {
                hand: *hand,
                enhancements: picked_attack_enhancements,
            }
        }
        BaseAction::SelfEffect(self_effect) => Action::SelfEffect(*self_effect),
        BaseAction::UseAbility(spell) => {
            let mut enhanced = false;

            if let Some(enhancement) = spell.possible_enhancement {
                if character.mana.current - spell.mana_cost >= enhancement.mana_cost {
                    println!(
                        "({}/{} mana)",
                        character.mana.current - spell.mana_cost,
                        character.mana.max
                    );
                    println!("Add spell enhancement");
                    println!(
                        "  1. [{}] ({} mana)",
                        enhancement.name, enhancement.mana_cost
                    );
                    println!("  2. Skip");

                    let choice = player_make_choice(2);
                    enhanced = choice == 1;
                }
            }

            Action::UseAbility {
                ability: *spell,
                enhanced,
            }
        }
    }
}

pub fn player_choose_on_attacked_reaction(defender: &Character) -> Option<OnAttackedReaction> {
    let reactions = defender.usable_on_attacked_reactions();

    if !reactions.is_empty() {
        println!("({} AP remaining) React to attack:", defender.action_points);
        for (i, (prefix, reaction)) in reactions.iter().enumerate() {
            let cost = match (reaction.action_point_cost, reaction.stamina_cost) {
                (ap, 0) => format!("{} AP", ap),
                (0, sta) => format!("{} sta", sta),
                (ap, sta) => format!("{} AP, {} sta", ap, sta),
            };
            let label = if prefix.len() > 0 {
                format!("{}: {}", prefix, reaction.name)
            } else {
                reaction.name.to_string()
            };
            println!("  {}. [{}] ({})", i + 1, label, cost);
        }

        println!("  {}. Skip", reactions.len() + 1);
        let chosen_index = player_make_choice(reactions.len() as u32 + 1) as usize - 1;
        if chosen_index < reactions.len() {
            return Some(reactions[chosen_index].1);
        }
    }

    None
}

pub fn player_choose_on_hit_reaction(defender: &Character) -> Option<OnHitReaction> {
    let reactions = defender.usable_on_hit_reactions();

    if !reactions.is_empty() {
        println!(
            "({} AP remaining) React to being hit:",
            defender.action_points
        );
        for (i, (prefix, reaction)) in reactions.iter().enumerate() {
            println!(
                "  {}. [{}{}] ({} AP)",
                i + 1,
                prefix,
                reaction.name,
                reaction.action_point_cost
            );
        }

        println!("  {}. Skip", reactions.len() + 1);
        let chosen_index = player_make_choice(reactions.len() as u32 + 1) as usize - 1;
        if chosen_index < reactions.len() {
            return Some(reactions[chosen_index].1);
        }
    }

    None
}

pub fn player_make_choice(max_allowed: u32) -> u32 {
    let stdin = io::stdin();
    let input = &mut String::new();
    loop {
        input.clear();
        stdin.read_line(input).unwrap();
        let line = input.trim_end_matches("\r\n").trim_end_matches('\n');
        if let Ok(i) = line.parse::<u32>() {
            if 1 <= i && i <= max_allowed {
                return i;
            }
        }
    }
}
