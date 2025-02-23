mod d20;

use std::{cell::RefCell, collections::HashSet, io, rc::Rc};

use d20::{probability_of_d20_reaching, roll_d20_with_advantage};

fn main() {
    let leather_armor = ArmorPiece {
        protection: 3,
        limit_defense_from_dex: None,
    };
    let chain_mail = ArmorPiece {
        protection: 5,
        limit_defense_from_dex: Some(4),
    };
    let dagger = Weapon {
        name: "Dagger",
        action_point_cost: 1,
        damage: 1,
        finesse: true,
        attack_action_enhancement: Default::default(),
        on_attacked_reaction: Default::default(),
        on_true_hit: Some(AttackHitEffect::Apply(ApplyEffect::Condition(
            Condition::Weakened(1),
        ))),
    };
    let parry = OnAttackedReaction {
        name: "Parry",
        action_point_cost: 1,
        stamina_cost: 0,
        effect: OnAttackedReactionEffect::Parry,
    };
    let sword = Weapon {
        name: "Sword",
        action_point_cost: 2,
        damage: 1,
        finesse: true,
        attack_action_enhancement: Default::default(),
        on_attacked_reaction: Some(parry),
        on_true_hit: Some(AttackHitEffect::Apply(ApplyEffect::Condition(
            Condition::Bleeding,
        ))),
    };
    let rapier = Weapon {
        name: "Rapier",
        action_point_cost: 2,
        damage: 1,
        finesse: true,
        attack_action_enhancement: Default::default(),
        on_attacked_reaction: Some(parry),
        on_true_hit: Some(AttackHitEffect::SkipExertion),
    };
    let war_hammer = Weapon {
        name: "War hammer",
        action_point_cost: 2,
        damage: 2,
        finesse: false,
        attack_action_enhancement: Some(AttackEnhancement {
            name: "All-in attack",
            action_point_cost: 1,
            stamina_cost: 0,
            bonus_damage: 1,
            apply_on_self_before: None,
            on_hit_effect: None,
        }),
        on_attacked_reaction: Some(parry),
        on_true_hit: Some(AttackHitEffect::Apply(ApplyEffect::Condition(
            Condition::Dazed(1),
        ))),
    };
    let bow = Weapon {
        name: "Bow",
        action_point_cost: 2,
        damage: 2,
        finesse: true, //TODO
        attack_action_enhancement: Some(AttackEnhancement {
            name: "Careful aim",
            action_point_cost: 1,
            stamina_cost: 0,
            bonus_damage: 0,
            apply_on_self_before: Some(Condition::CarefulAim),
            on_hit_effect: None,
        }),
        on_attacked_reaction: None,
        on_true_hit: Some(AttackHitEffect::Apply(ApplyEffect::Condition(
            Condition::Weakened(1),
        ))),
    };

    let crushing_strike = AttackEnhancement {
        name: "Crushing strike",
        action_point_cost: 0,
        stamina_cost: 1,
        bonus_damage: 0,
        apply_on_self_before: None,
        on_hit_effect: Some(ApplyEffect::Stunned(1)),
    };

    let spells = vec![
        Spell {
            name: "Scream",
            action_point_cost: 2,
            mana_cost: 1,
            damage: 0,
            on_hit_effect: Some(ApplyEffect::Condition(Condition::Dazed(1))),
        },
        Spell {
            name: "Mind blast",
            action_point_cost: 2,
            mana_cost: 1,
            damage: 1,
            on_hit_effect: Some(ApplyEffect::Stunned(1)),
        },
    ];

    let mut bob = Character::new("Bob", 10, 10, 4, bow);
    bob.armor = Some(chain_mail);
    bob.known_attack_enhancements.push(crushing_strike);
    bob.known_attacked_reactions.push(OnAttackedReaction {
        name: "Side step",
        action_point_cost: 1,
        stamina_cost: 1,
        effect: OnAttackedReactionEffect::SideStep,
    });
    for spell in spells {
        bob.known_actions.push(Action::CastSpell(spell));
    }

    let mut alice = Character::new("Alice", 4, 2, 3, dagger);
    alice.armor = Some(leather_armor);

    alice.receive_condition(Condition::Stunned);

    let action_points_per_turn = 5;

    bob.action_points = action_points_per_turn;
    alice.action_points = action_points_per_turn;

    dbg!(&bob);
    dbg!(&alice);

    let characters = Characters::new(vec![bob, alice]);

    loop {
        for i in 0..2 {
            let players_turn = i == 0;
            let mut character = characters.get(i).borrow_mut();
            let mut other_character = characters.get((i + 1) % 2).borrow_mut();

            println!("---\nNew turn: {}\n---", character.name);

            if character.conditions.braced {
                character.conditions.braced = false;
                println!("{} lost Braced", character.name);
            }

            while character.action_points > 0 {
                if character.conditions.stunned {
                    character.conditions.stunned = false;
                    println!("{} recovered from Stunned", character.name);
                }
                println!();
                println!(
                    "({} AP, {}/{} stamina, {}/{} mana)",
                    character.action_points,
                    character.stamina.current,
                    character.stamina.max,
                    character.mana.current,
                    character.mana.max
                );
                let action = if players_turn {
                    player_choose_action(&character, &other_character)
                } else {
                    Action::Attack(Default::default())
                };

                match action {
                    Action::Attack(attack_enhancements) => {
                        character.action_points -= character.weapon.action_point_cost as i32;
                        for enhancement in &attack_enhancements {
                            character.action_points -= enhancement.action_point_cost as i32;
                            character.stamina.lose(enhancement.stamina_cost);
                            if let Some(condition) = enhancement.apply_on_self_before {
                                character.receive_condition(condition);
                            }
                        }

                        print_attack_intro(&character, &other_character);

                        let defender_reaction = if players_turn {
                            None
                        } else {
                            player_choose_on_attacked_reaction(&other_character)
                        };

                        if let Some(reaction) = defender_reaction {
                            other_character.action_points -= reaction.action_point_cost as i32;
                        }

                        let did_attack_hit = perform_attack(
                            &mut character,
                            attack_enhancements,
                            &mut other_character,
                            defender_reaction,
                        );

                        if did_attack_hit {
                            if !players_turn {
                                if let Some(reaction) =
                                    player_choose_on_attacked_hit_reaction(&other_character)
                                {
                                    match reaction.effect {
                                        OnAttackedHitReactionEffect::Rage => {
                                            other_character.receive_condition(Condition::Raging);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Action::ApplyOnSelf {
                        name: _,
                        action_point_cost,
                        effect,
                    } => {
                        character.action_points -= action_point_cost as i32;
                        perform_effect_application(effect, &mut character, "");
                    }
                    Action::CastSpell(spell) => {
                        character.action_points -= spell.action_point_cost as i32;
                        character.mana.lose(spell.mana_cost);

                        let target = other_character.mental_resistence();

                        let roll = roll_d20_with_advantage(0);
                        let res = roll + character.intellect();
                        println!(
                            "Rolled: {} (+{} int) = {}, vs mental resist={}",
                            roll,
                            character.intellect(),
                            res,
                            target,
                        );

                        if res >= target {
                            println!("  It's a spell hit");
                            let damage = spell.damage;
                            other_character.health.lose(damage);
                            println!("  {} took {} damage", other_character.name, damage);
                            if let Some(effect) = spell.on_hit_effect {
                                perform_effect_application(
                                    effect,
                                    &mut other_character,
                                    spell.name,
                                );
                            }
                        } else {
                            println!("  It's a spell miss");
                        }
                    }
                }
            }

            if character.conditions.bleeding > 0 {
                character.health.lose(1);
                println!(
                    "{} took 1 damage from Bleeding and went down to {}/{} health",
                    character.name, character.health.current, character.health.max
                );
                character.conditions.bleeding -= 1;
                if character.conditions.bleeding == 0 {
                    println!("{} stopped Bleeding", character.name);
                }
            }

            if character.conditions.dazed > 0 {
                character.conditions.dazed = 0;
                println!("{} recovered from Dazed", character.name);
            }

            if character.conditions.weakened > 0 {
                character.conditions.weakened = 0;
                println!("{} recovered from Weakened", character.name);
            }

            if character.conditions.raging {
                character.conditions.raging = false;
                println!("{} stopped Raging", character.name)
            }

            println!("End of turn.");

            character.action_points += action_points_per_turn;
            character.attack_exertion = 0;
            character.stamina.gain(1);
        }
    }
}

fn player_choose_action(character: &Character, other_character: &Character) -> Action {
    let mut available_actions = vec![];

    for action in &character.known_actions {
        match action {
            Action::Attack(_) => {
                if character.action_points >= character.weapon.action_point_cost as i32 {
                    available_actions.push(action);
                }
            }
            Action::ApplyOnSelf {
                name: _,
                action_point_cost,
                effect: _,
            } => {
                if character.action_points >= *action_point_cost as i32 {
                    available_actions.push(action)
                }
            }
            Action::CastSpell(spell) => {
                if character.action_points >= spell.action_point_cost as i32
                    && character.mana.current >= spell.mana_cost
                {
                    available_actions.push(action);
                }
            }
        }
    }

    println!("Choose an action:");
    for (i, action) in available_actions.iter().enumerate() {
        let label = match action {
            Action::Attack(_) => format!(
                "[{}: Attack] ({} AP, hit={})",
                character.weapon.name,
                character.weapon.action_point_cost,
                as_percentage(prob_attack_hit(&character, &other_character))
            ),
            Action::ApplyOnSelf {
                name,
                action_point_cost,
                effect: _,
            } => {
                format!("[{}] ({} AP)", name, action_point_cost)
            }
            Action::CastSpell(spell) => format!(
                "[{}] ({} AP, {} mana, hit={})",
                spell.name,
                spell.action_point_cost,
                spell.mana_cost,
                as_percentage(prob_spell_hit(&character, &other_character))
            ),
        };
        println!("  {}. {}", i + 1, label);
    }
    let action_choice = read_user_choice(available_actions.len() as u32);

    match &available_actions[action_choice as usize - 1] {
        Action::Attack(_) => {
            let reserved_action_points = character.weapon.action_point_cost as i32;

            let mut available_attack_enhancements = vec![];
            let mut picked_attack_enhancements = vec![];

            if let Some(enhancement) = character.weapon.attack_action_enhancement {
                let prefix = format!("{}: ", character.weapon.name);
                available_attack_enhancements.push((prefix, enhancement))
            }
            for &enhancement in &character.known_attack_enhancements {
                available_attack_enhancements.push(("".to_owned(), enhancement))
            }
            available_attack_enhancements = available_attack_enhancements
                .into_iter()
                .filter(|(_, enhancement)| {
                    character.action_points - reserved_action_points
                        >= enhancement.action_point_cost as i32
                })
                .collect();

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
                    let line = input.trim_end_matches("\r\n").trim_end_matches("\n");
                    if line.is_empty() {
                        // player picked no enhancements
                        break;
                    }
                    let picked_numbers = line
                        .split_whitespace()
                        .map(|w| w.parse::<u32>())
                        .filter(|res| res.is_ok())
                        .map(|res| res.unwrap())
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

                    if character.action_points - reserved_action_points >= total_cost as i32 {
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

            Action::Attack(picked_attack_enhancements)
        }
        Action::ApplyOnSelf {
            name,
            action_point_cost,
            effect,
        } => Action::ApplyOnSelf {
            name,
            action_point_cost: *action_point_cost,
            effect: *effect,
        },
        Action::CastSpell(spell) => Action::CastSpell(*spell),
    }
}

fn player_choose_on_attacked_reaction(defender: &Character) -> Option<OnAttackedReaction> {
    let mut available_reactions = vec![];

    for reaction in &defender.known_attacked_reactions {
        if defender.action_points >= reaction.action_point_cost as i32 {
            available_reactions.push(("".to_string(), *reaction));
        }
    }
    if let Some(reaction) = &defender.weapon.on_attacked_reaction {
        if defender.action_points >= reaction.action_point_cost as i32 {
            available_reactions.push((format!("{}: ", defender.weapon.name), *reaction));
        }
    }

    if !available_reactions.is_empty() {
        println!("({} AP remaining) React to attack:", defender.action_points);
        for (i, (prefix, reaction)) in available_reactions.iter().enumerate() {
            let cost = match (reaction.action_point_cost, reaction.stamina_cost) {
                (ap, 0) => format!("{} AP", ap),
                (0, sta) => format!("{} sta", sta),
                (ap, sta) => format!("{} AP, {} sta", ap, sta),
            };
            println!("  {}. [{}{}] ({})", i + 1, prefix, reaction.name, cost);
        }

        println!("  {}. Skip", available_reactions.len() + 1);
        let chosen_index = read_user_choice(available_reactions.len() as u32 + 1) as usize - 1;
        if chosen_index < available_reactions.len() {
            return Some(available_reactions[chosen_index].1);
        }
    }

    None
}

fn player_choose_on_attacked_hit_reaction(defender: &Character) -> Option<OnAttackedHitReaction> {
    let mut available_reactions = vec![];
    let rage = OnAttackedHitReaction {
        name: "Rage",
        action_point_cost: 1,
        effect: OnAttackedHitReactionEffect::Rage,
    };
    if !defender.conditions.raging && defender.action_points >= rage.action_point_cost as i32 {
        available_reactions.push(rage);
    }

    if !available_reactions.is_empty() {
        println!(
            "({} AP remaining) React to being hit:",
            defender.action_points
        );
        for (i, reaction) in available_reactions.iter().enumerate() {
            println!(
                "  {}. [{}] ({} AP)",
                i + 1,
                reaction.name,
                reaction.action_point_cost
            );
        }

        println!("  {}. Skip", available_reactions.len() + 1);
        let chosen_index = read_user_choice(available_reactions.len() as u32 + 1) as usize - 1;
        if chosen_index < available_reactions.len() {
            return Some(available_reactions[chosen_index]);
        }
    }

    None
}

fn read_user_choice(max_allowed: u32) -> u32 {
    let stdin = io::stdin();
    let input = &mut String::new();
    loop {
        input.clear();
        stdin.read_line(input).unwrap();
        let line = input.trim_end_matches("\r\n").trim_end_matches("\n");
        if let Ok(i) = line.parse::<u32>() {
            if 1 <= i && i <= max_allowed {
                return i;
            }
        }
    }
}

fn print_attack_intro(attacker: &Character, defender: &Character) {
    println!(
        "{} attacks {} (d20+{} vs {})",
        attacker.name,
        defender.name,
        attacker.attack_modifier(),
        defender.defense()
    );
    println!(
        "  {}{}",
        attacker.explain_attack_circumstances(),
        defender.explain_incoming_attack_circumstances()
    );
    println!(
        "  Chance to hit: {}",
        as_percentage(prob_attack_hit(attacker, defender))
    );
}

fn as_percentage(probability: f32) -> String {
    if probability < 0.01 || 0.99 < probability {
        format!("{:.1}%", probability * 100f32)
    } else {
        format!("{:.0}%", probability * 100f32)
    }
}

fn prob_attack_hit(attacker: &Character, defender: &Character) -> f32 {
    let advantage_level = attacker.attack_advantage() + defender.incoming_attack_advantage();
    let target = defender
        .defense()
        .saturating_sub(attacker.attack_modifier())
        .max(1);
    probability_of_d20_reaching(target, advantage_level)
}

fn prob_spell_hit(caster: &Character, defender: &Character) -> f32 {
    let target = defender
        .mental_resistence()
        .saturating_sub(caster.intellect())
        .max(1);
    probability_of_d20_reaching(target, 0)
}

fn perform_attack(
    attacker: &mut Character,
    attack_enhancements: Vec<AttackEnhancement>,
    defender: &mut Character,
    defender_reaction: Option<OnAttackedReaction>,
) -> bool {
    let mut advantage = attacker.attack_advantage() + defender.incoming_attack_advantage();

    let mut defense = defender.defense();

    let mut defender_reacted_with_parry = false;
    let mut defender_reacted_with_sidestep = false;
    let mut skip_attack_exertion = false;
    let mut did_hit = false;

    if let Some(reaction) = defender_reaction {
        match reaction.effect {
            OnAttackedReactionEffect::Parry => {
                defender_reacted_with_parry = true;
                let bonus_def = defender.strength();
                println!(
                    "  Defense: {} +{} (Parry) = {}",
                    defense,
                    bonus_def,
                    defense + bonus_def
                );
                defense += bonus_def;
                let p_hit =
                    probability_of_d20_reaching(defense - attacker.attack_modifier(), advantage);
                println!("  Chance to hit: {:.1}%", p_hit * 100f32);
            }
            OnAttackedReactionEffect::SideStep => {
                defender_reacted_with_sidestep = true;
                let bonus_def = defender.defense_bonus_from_dexterity();
                println!(
                    "  Defense: {} +{} (Side step) = {}",
                    defense,
                    bonus_def,
                    defense + bonus_def
                );
                defense += bonus_def;
                let p_hit =
                    probability_of_d20_reaching(defense - attacker.attack_modifier(), advantage);
                println!("  Chance to hit: {:.1}%", p_hit * 100f32);
            }
        }
    }

    let roll = roll_d20_with_advantage(advantage);
    let res = roll + attacker.attack_modifier();
    println!(
        "Rolled: {} (+{}) = {}, vs def={}, armor={}",
        roll,
        attacker.attack_modifier(),
        res,
        defense,
        defender.protection_from_armor()
    );

    print!("  ");
    if res < defense {
        if defender_reacted_with_parry {
            println!("Parried!");
        } else if defender_reacted_with_sidestep {
            println!("Side stepped!");
        } else {
            println!("Missed!")
        }
    } else {
        did_hit = true;

        let mut on_true_hit_effect = None;
        let mut damage = attacker.weapon.damage;

        let damage_prefix = format!("  Damage: {} (weapon)", attacker.weapon.damage);
        if res < defense + defender.protection_from_armor() {
            println!("Hit!");
            print!("{damage_prefix}");
        } else {
            on_true_hit_effect = attacker.weapon.on_true_hit;
            let (label, bonus_dmg) = if res < defense + defender.protection_from_armor() + 5 {
                ("True hit", 1)
            } else if res < defense + defender.protection_from_armor() + 10 {
                ("Heavy hit", 2)
            } else {
                ("Critical hit", 3)
            };
            println!("{label}!");
            print!("{damage_prefix} +{bonus_dmg} ({label})");
            damage += bonus_dmg;
        }

        for enhancement in &attack_enhancements {
            if enhancement.bonus_damage > 0 {
                print!(" +{} ({})", enhancement.bonus_damage, enhancement.name);
                damage += enhancement.bonus_damage;
            }
        }

        println!(" = {damage}");

        defender.health.lose(damage);

        println!(
            "  {} took {} damage and went down to {}/{} health",
            defender.name, damage, defender.health.current, defender.health.max
        );

        if let Some(effect) = on_true_hit_effect {
            match effect {
                AttackHitEffect::Apply(effect) => {
                    perform_effect_application(effect, defender, "true hit");
                }
                AttackHitEffect::SkipExertion => skip_attack_exertion = true,
            }
        }

        for enhancement in &attack_enhancements {
            if let Some(effect) = enhancement.on_hit_effect {
                perform_effect_application(effect, defender, enhancement.name);
            }
        }
    }

    if skip_attack_exertion {
        println!("The attack did not lead to exertion (true hit)");
    } else {
        attacker.attack_exertion += 1;
        println!("The attack led to exertion ({})", attacker.attack_exertion);
    }

    if attacker.conditions.dazed > 0 {
        attacker.conditions.dazed -= 1;
        println!(
            "{} lost 1 Dazed (down to {})",
            attacker.name, attacker.conditions.dazed
        );
    }

    if attacker.conditions.careful_aim {
        attacker.conditions.careful_aim = false;
        println!("{} lost Careful aim", attacker.name);
    }

    if defender.conditions.braced {
        defender.conditions.braced = false;
        println!("{} lost Braced", defender.name);
    }

    return did_hit;
}

fn perform_effect_application(
    effect: ApplyEffect,
    receiver: &mut Character,
    context: &'static str,
) {
    match effect {
        ApplyEffect::Stunned(n) => {
            receiver.action_points -= n as i32;
            receiver.receive_condition(Condition::Stunned);
            print!("  {} lost {} AP and became Stunned", receiver.name, n);
        }
        ApplyEffect::Condition(condition) => {
            receiver.receive_condition(condition);
            print!("  {} received {:?}", receiver.name, condition);
        }
    }
    println!(" ({})", context);
}

struct Characters(Vec<RefCell<Character>>);

impl Characters {
    fn new(characters: Vec<Character>) -> Self {
        Self(characters.into_iter().map(|ch| RefCell::new(ch)).collect())
    }

    fn get(&self, i: usize) -> &RefCell<Character> {
        self.0.get(i).unwrap()
    }
}

#[derive(Debug, Copy, Clone)]
struct AttackEnhancement {
    name: &'static str,
    action_point_cost: u32,
    stamina_cost: u32,
    bonus_damage: u32,
    apply_on_self_before: Option<Condition>,
    on_hit_effect: Option<ApplyEffect>,
}

#[derive(Debug, Copy, Clone)]
enum ApplyEffect {
    Stunned(u32),
    Condition(Condition),
}

#[derive(Debug, Copy, Clone)]
struct OnAttackedReaction {
    name: &'static str,
    action_point_cost: u32,
    stamina_cost: u32,
    effect: OnAttackedReactionEffect,
}

#[derive(Debug, Copy, Clone)]
enum OnAttackedReactionEffect {
    Parry,
    SideStep,
}

#[derive(Debug, Copy, Clone)]
struct OnAttackedHitReaction {
    name: &'static str,
    action_point_cost: u32,
    effect: OnAttackedHitReactionEffect,
}

#[derive(Debug, Copy, Clone)]
enum OnAttackedHitReactionEffect {
    Rage,
}

#[derive(Debug, Copy, Clone)]
enum AttackHitEffect {
    Apply(ApplyEffect),
    SkipExertion,
}

#[derive(Debug, Copy, Clone)]
enum Condition {
    Dazed(u32),
    Stunned,
    Bleeding,
    Braced,
    Raging,
    CarefulAim,
    Weakened(u32),
}

#[derive(Debug, Copy, Clone, Default)]
struct Conditions {
    dazed: u32,
    stunned: bool,
    bleeding: u32,
    braced: bool,
    raging: bool,
    careful_aim: bool,
    weakened: u32,
}

#[derive(Debug)]
enum Action {
    Attack(Vec<AttackEnhancement>),
    ApplyOnSelf {
        name: &'static str,
        action_point_cost: u32,
        effect: ApplyEffect,
    },
    CastSpell(Spell),
}

#[derive(Debug, Copy, Clone)]
struct Spell {
    name: &'static str,
    action_point_cost: u32,
    mana_cost: u32,
    damage: u32,
    on_hit_effect: Option<ApplyEffect>,
}

#[derive(Debug)]
struct Character {
    name: &'static str,
    base_strength: u32,
    base_dexterity: u32,
    base_intellect: u32,
    health: NumberedResource,
    mana: NumberedResource,
    armor: Option<ArmorPiece>,
    weapon: Weapon,
    conditions: Conditions,
    action_points: i32,
    stamina: NumberedResource,
    attack_exertion: u32,
    known_attack_enhancements: Vec<AttackEnhancement>,
    known_actions: Vec<Action>,
    known_attacked_reactions: Vec<OnAttackedReaction>,
}

impl Character {
    fn new(name: &'static str, str: u32, dex: u32, int: u32, weapon: Weapon) -> Self {
        let mana = if int < 3 { 0 } else { 1 + 2 * (int - 3) };
        Self {
            name,
            base_strength: str,
            base_dexterity: dex,
            base_intellect: int,
            health: NumberedResource::new(5 + str),
            mana: NumberedResource::new(mana),
            armor: None,
            weapon,
            conditions: Default::default(),
            action_points: 0,
            stamina: NumberedResource::new((str + dex).saturating_sub(5)),
            attack_exertion: 0,
            known_attack_enhancements: Default::default(),
            known_actions: vec![
                Action::Attack(Default::default()),
                Action::ApplyOnSelf {
                    name: "Brace",
                    action_point_cost: 1,
                    effect: ApplyEffect::Condition(Condition::Braced),
                },
            ],
            known_attacked_reactions: Default::default(),
        }
    }

    fn strength(&self) -> u32 {
        self.base_strength - self.conditions.weakened
    }

    fn dexterity(&self) -> u32 {
        self.base_dexterity - self.conditions.weakened
    }

    fn intellect(&self) -> u32 {
        self.base_intellect - self.conditions.weakened
    }

    fn is_dazed(&self) -> bool {
        self.conditions.dazed > 0
    }

    fn defense(&self) -> u32 {
        let from_dex = self.defense_bonus_from_dexterity();
        let from_braced = if self.conditions.braced { 3 } else { 0 };
        10 + from_dex + from_braced
    }

    fn defense_bonus_from_dexterity(&self) -> u32 {
        let mut bonus = if self.is_dazed() { 0 } else { self.dexterity() };
        if let Some(armor) = self.armor {
            if let Some(limit) = armor.limit_defense_from_dex {
                bonus = bonus.min(limit);
            }
        }
        bonus
    }

    fn mental_resistence(&self) -> u32 {
        10 + self.intellect()
    }

    fn protection_from_armor(&self) -> u32 {
        self.armor.map(|armor| armor.protection).unwrap_or(0)
    }

    fn attack_modifier(&self) -> u32 {
        let str = self.strength();
        let dex = self.dexterity();
        if self.weapon.finesse {
            str.max(dex)
        } else {
            str
        }
    }

    fn attack_advantage(&self) -> i32 {
        let mut res = 0i32;
        res -= self.attack_exertion as i32;
        if self.is_dazed() {
            res -= 1;
        }
        if self.conditions.raging {
            res += 1;
        }
        if self.conditions.careful_aim {
            res += 1;
        }
        res
    }

    fn explain_attack_circumstances(&self) -> String {
        let mut s = String::new();
        if self.attack_exertion > 0 {
            s.push_str("[exerted -]");
        }
        if self.is_dazed() {
            s.push_str("[dazed -]");
        }
        if self.conditions.raging {
            s.push_str("[raging +]");
        }
        if self.conditions.careful_aim {
            s.push_str("[careful aim +]");
        }
        if self.conditions.weakened > 0 {
            s.push_str("[weakened -]");
        }
        s
    }

    fn incoming_attack_advantage(&self) -> i32 {
        let mut res = 0;
        if self.conditions.stunned {
            // attacker has advantage
            res += 1;
        }
        res
    }

    fn explain_incoming_attack_circumstances(&self) -> String {
        let mut s = String::new();
        if self.conditions.stunned {
            s.push_str("[stunned +]")
        }
        if self.is_dazed() {
            s.push_str("[dazed +]")
        }
        if self.conditions.weakened > 0 {
            s.push_str("[weakened +]");
        }
        s
    }

    fn receive_condition(&mut self, condition: Condition) {
        match condition {
            Condition::Dazed(n) => self.conditions.dazed += n,
            Condition::Stunned => self.conditions.stunned = true,
            Condition::Bleeding => self.conditions.bleeding += 1,
            Condition::Braced => self.conditions.braced = true,
            Condition::Raging => self.conditions.raging = true,
            Condition::CarefulAim => self.conditions.careful_aim = true,
            Condition::Weakened(n) => self.conditions.weakened += n,
        }
    }
}

#[derive(Debug, Copy, Clone)]
struct NumberedResource {
    current: u32,
    max: u32,
}

impl NumberedResource {
    fn new(max: u32) -> Self {
        Self { current: max, max }
    }

    fn lose(&mut self, amount: u32) {
        self.current = self.current.saturating_sub(amount); // cannot go below 0
    }

    fn gain(&mut self, amount: u32) {
        self.current = (self.current + amount).min(self.max);
    }
}

#[derive(Debug, Copy, Clone)]
struct ArmorPiece {
    protection: u32,
    limit_defense_from_dex: Option<u32>,
}

#[derive(Debug)]
struct Weapon {
    name: &'static str,
    action_point_cost: u32,
    damage: u32,
    // By default, meele weapons use STR, but Finesse weapons use DEX if it's higher than STR
    finesse: bool,
    attack_action_enhancement: Option<AttackEnhancement>,
    on_attacked_reaction: Option<OnAttackedReaction>,
    on_true_hit: Option<AttackHitEffect>,
}
