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
        on_heavy_hit: Default::default(),
    };
    let parry = OnAttackedReaction {
        name: "Parry",
        action_point_cost: 1,
        effect: OnAttackedReactionEffect::Parry,
    };
    let rapier = Weapon {
        name: "Rapier",
        action_point_cost: 2,
        damage: 1,
        finesse: true,
        attack_action_enhancement: Default::default(),
        on_attacked_reaction: Some(parry),
        on_heavy_hit: Some(OnHeavyHitEffect::SkipExertion),
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
            effect: AttackEnhancementEffect::AllInAttack,
        }),
        on_attacked_reaction: Some(parry),
        on_heavy_hit: Some(OnHeavyHitEffect::ApplyDazed),
    };

    let crushing_strike = AttackEnhancement {
        name: "Crushing strike",
        action_point_cost: 0,
        stamina_cost: 1,
        effect: AttackEnhancementEffect::CrushingStrike,
    };

    let mut bob = Character::new("Bob", 10, 3, 4, war_hammer);
    bob.armor = Some(chain_mail);
    bob.attack_action_enhancements.push(crushing_strike);
    bob.on_attacked_reactions.push(OnAttackedReaction {
        name: "Side step",
        action_point_cost: 1,
        effect: OnAttackedReactionEffect::SideStep,
    });

    let mut alice = Character::new("Alice", 4, 8, 3, dagger);
    alice.armor = Some(leather_armor);

    bob.receive_condition(Condition::Godlike);
    alice.receive_condition(Condition::Stunned);

    let action_points_per_turn = 6;

    bob.action_points = action_points_per_turn;
    alice.action_points = action_points_per_turn;

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
                println!();
                println!(
                    "({} AP, {}/{} stamina)",
                    character.action_points, character.stamina.current, character.stamina.max
                );
                let action = if players_turn {
                    let mut available_actions = vec![];
                    if character.action_points >= character.weapon.action_point_cost as i32 {
                        available_actions.push(Action::Attack(Default::default()));
                    }
                    available_actions.push(Action::Idle);
                    available_actions.push(Action::Brace);

                    println!("Choose an action from:");
                    for (i, action) in available_actions.iter().enumerate() {
                        let label = match action {
                            Action::Attack(_) => format!(
                                "[Attack] ({}: {} AP, {} dmg, hit={})",
                                character.weapon.name,
                                character.weapon.action_point_cost,
                                character.weapon.damage,
                                as_percentage(prob_attack_hit(&character, &other_character))
                            ),
                            Action::Idle => "[Idle]".to_string(),
                            Action::Brace => "[Brace]".to_string(),
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
                            for &enhancement in &character.attack_action_enhancements {
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
                                println!("Add attack enhancements (whitespace-separated numbers; empty line to skip)");

                                for (i, (prefix, enhancement)) in
                                    available_attack_enhancements.iter().enumerate()
                                {
                                    let cost = match (
                                        enhancement.action_point_cost,
                                        enhancement.stamina_cost,
                                    ) {
                                        (ap, 0) if ap > 0 => format!("{} AP", ap),
                                        (0, sta) => format!("{} sta", sta),
                                        (ap, sta) => format!("{} AP, {} sta", ap, sta),
                                    };
                                    println!(
                                        "  {}. [{}{}] ({})",
                                        i + 1,
                                        prefix,
                                        enhancement.name,
                                        cost
                                    );
                                }

                                let stdin = io::stdin();
                                let input = &mut String::new();
                                loop {
                                    input.clear();
                                    stdin.read_line(input).unwrap();
                                    let line =
                                        input.trim_end_matches("\r\n").trim_end_matches("\n");
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

                                    if character.action_points - reserved_action_points
                                        >= total_cost as i32
                                    {
                                        for i in picked_numbers {
                                            picked_attack_enhancements.push(
                                                available_attack_enhancements[i as usize - 1].1,
                                            );
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
                        Action::Idle => Action::Idle,
                        Action::Brace => Action::Brace,
                    }
                } else {
                    Action::Attack(Default::default())
                };

                match action {
                    Action::Attack(attack_enhancements) => {
                        character.action_points -= character.weapon.action_point_cost as i32;

                        print_attack_intro(&character, &other_character);

                        let defender_reaction = if players_turn {
                            None
                        } else {
                            let mut defender_reaction = None;
                            // TODO handle character's non-weapon reactions (like side-step)
                            if let Some(reaction) = other_character.weapon.on_attacked_reaction {
                                if other_character.action_points
                                    >= reaction.action_point_cost as i32
                                {
                                    println!(
                                        "({} AP remaining) React to attack:",
                                        other_character.action_points
                                    );
                                    println!(
                                        "  1. [{}: {}]",
                                        other_character.weapon.name, reaction.name
                                    );
                                    println!("  2. Skip");
                                    if read_user_choice(2) == 1 {
                                        defender_reaction = Some(reaction);
                                    }
                                }
                            }
                            defender_reaction
                        };

                        for enhancement in &attack_enhancements {
                            character.action_points -= enhancement.action_point_cost as i32;
                            character.stamina.lose(enhancement.stamina_cost);
                        }

                        if let Some(reaction) = defender_reaction {
                            other_character.action_points -= reaction.action_point_cost as i32;
                        }

                        perform_attack(
                            &mut character,
                            attack_enhancements,
                            &mut other_character,
                            defender_reaction,
                        );
                    }
                    Action::Idle => {
                        character.action_points -= 1;
                    }
                    Action::Brace => {
                        character.action_points -= 1;
                        character.receive_condition(Condition::Braced);
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
                    println!("{} stopped bleeding", character.name);
                }
            }

            if character.conditions.dazed > 0 {
                character.conditions.dazed = 0;
                println!("{} recovered from Dazed", character.name);
            }

            println!("End of turn.");

            character.action_points += action_points_per_turn;
            character.attack_exertion = 0;
            character.stamina.gain(1);
        }
    }
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
    format!("{:.1}%", probability * 100f32)
}

fn prob_attack_hit(attacker: &Character, defender: &Character) -> f32 {
    let advantage_level = attacker.attack_advantage() + defender.incoming_attack_advantage();
    let target = defender
        .defense()
        .saturating_sub(attacker.attack_modifier())
        .max(1);
    probability_of_d20_reaching(target, advantage_level)
}

fn perform_attack(
    attacker: &mut Character,
    attack_enhancements: Vec<AttackEnhancement>,
    defender: &mut Character,
    defender_reaction: Option<OnAttackedReaction>,
) {
    let advantage = attacker.attack_advantage() + defender.incoming_attack_advantage();
    let mut defense = defender.defense();

    let mut defender_is_parrying = false;
    let mut skip_attack_exertion = false;

    if let Some(reaction) = defender_reaction {
        match reaction.effect {
            OnAttackedReactionEffect::Parry => {
                defender_is_parrying = true;
                let bonus_def = defender.str;
                println!(
                    "  Defense: {} +{} (parry) = {}",
                    defense,
                    bonus_def,
                    defense + bonus_def
                );
                defense += bonus_def;
                let p_hit =
                    probability_of_d20_reaching(defense - attacker.attack_modifier(), advantage);
                println!("  Chance to hit: {:.1}%", p_hit * 100f32);
            }
            OnAttackedReactionEffect::SideStep => todo!(),
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
        if defender_is_parrying {
            println!("It's a parry!");
        } else {
            println!("It's a miss!")
        }
    } else {
        let mut on_heavy_hit_effect = None;
        let mut apply_crushing_strike = false;
        let mut damage = attacker.weapon.damage;
        println!("It's a hit!");
        let damage_prefix = format!("  Damage: {} (weapon)", attacker.weapon.damage);
        print!("{damage_prefix}");
        if res >= defense + defender.protection_from_armor() {
            on_heavy_hit_effect = attacker.weapon.on_heavy_hit;

            let (label, bonus_dmg) = if res >= defense + defender.protection_from_armor() + 5 {
                ("brutal hit", 2)
            } else {
                ("heavy hit", 1)
            };
            print!(" +{bonus_dmg} ({label})");
            damage += bonus_dmg;
        }

        for enhancement in attack_enhancements {
            match enhancement.effect {
                AttackEnhancementEffect::AllInAttack => {
                    let damage_bonus = 1;
                    print!(" +{} ({})", damage_bonus, enhancement.name);
                    damage += damage_bonus;
                }
                AttackEnhancementEffect::CrushingStrike => {
                    apply_crushing_strike = true;
                }
            }
        }

        println!(" = {damage}");

        defender.health.lose(damage);

        println!(
            "  {} took {} damage and went down to {}/{} health",
            defender.name, damage, defender.health.current, defender.health.max
        );

        if let Some(effect) = on_heavy_hit_effect {
            match effect {
                OnHeavyHitEffect::ApplyBleed => {
                    defender.receive_condition(Condition::Bleeding);
                    println!("  {} received Bleeding (heavy hit)", defender.name);
                }
                OnHeavyHitEffect::SkipExertion => skip_attack_exertion = true,
                OnHeavyHitEffect::ApplyDazed => {
                    defender.receive_condition(Condition::Dazed(1));
                    println!("  {} received Dazed (heavy hit)", defender.name);
                }
            }
        }

        if apply_crushing_strike {
            defender.action_points -= 1;
            defender.receive_condition(Condition::Stunned);
            println!(
                "  {} lost 1 AP and became Stunned (Crushing strike)",
                defender.name
            );
        }
    }

    if skip_attack_exertion {
        println!("The attack did not lead to exertion (heavy hit)");
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

    if defender.conditions.braced {
        defender.conditions.braced = false;
        println!("{} lost Braced", defender.name);
    }
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
    effect: AttackEnhancementEffect,
}

#[derive(Debug, Copy, Clone)]
enum AttackEnhancementEffect {
    AllInAttack,
    CrushingStrike,
}

#[derive(Debug, Copy, Clone)]
struct OnAttackedReaction {
    name: &'static str,
    action_point_cost: u32,
    effect: OnAttackedReactionEffect,
}

#[derive(Debug, Copy, Clone)]
enum OnAttackedReactionEffect {
    Parry,
    SideStep,
}

#[derive(Debug, Copy, Clone)]
enum OnHeavyHitEffect {
    ApplyBleed,
    ApplyDazed,
    SkipExertion,
}

#[derive(Debug, Copy, Clone)]
enum Condition {
    Dazed(u32),
    Godlike,
    Stunned,
    Bleeding,
    Braced,
}

#[derive(Debug, Copy, Clone, Default)]
struct Conditions {
    dazed: u32,
    godlike: bool,
    stunned: bool,
    bleeding: u32,
    braced: bool,
}

#[derive(Debug)]
enum Action {
    Attack(Vec<AttackEnhancement>),
    Brace,
    Idle,
}

#[derive(Debug)]
struct Character {
    name: &'static str,
    // Strength
    str: u32,
    // Dexterity
    dex: u32,
    // Intellect
    int: u32,
    health: NumberedResource,
    armor: Option<ArmorPiece>,
    weapon: Weapon,
    conditions: Conditions,
    action_points: i32,
    stamina: NumberedResource,
    attack_exertion: u32,
    attack_action_enhancements: Vec<AttackEnhancement>,
    on_attacked_reactions: Vec<OnAttackedReaction>,
}

impl Character {
    fn new(name: &'static str, str: u32, dex: u32, int: u32, weapon: Weapon) -> Self {
        Self {
            name,
            str,
            dex,
            int,
            health: NumberedResource::new(5 + str),
            armor: None,
            weapon,
            conditions: Default::default(),
            action_points: 0,
            stamina: NumberedResource::new((str + dex).saturating_sub(5)),
            attack_exertion: 0,
            attack_action_enhancements: Default::default(),
            on_attacked_reactions: Default::default(),
        }
    }

    fn is_dazed(&self) -> bool {
        self.conditions.dazed > 0
    }

    fn defense(&self) -> u32 {
        let mut from_dex = if self.is_dazed() { 0 } else { self.dex };
        match self.armor {
            Some(armor) => {
                if let Some(limit) = armor.limit_defense_from_dex {
                    from_dex = from_dex.min(limit);
                }
            }
            None => {}
        };
        let from_braced = if self.conditions.braced { 3 } else { 0 };
        8 + from_dex + from_braced
    }

    fn protection_from_armor(&self) -> u32 {
        self.armor.map(|armor| armor.protection).unwrap_or(0)
    }

    fn attack_modifier(&self) -> u32 {
        if self.weapon.finesse {
            self.str.max(self.dex)
        } else {
            self.str
        }
    }

    fn attack_advantage(&self) -> i32 {
        let mut res = 0i32;
        if self.is_dazed() {
            res -= 1;
        }
        if self.conditions.godlike {
            res += 2;
        }
        res -= self.attack_exertion as i32;
        res
    }

    fn explain_attack_circumstances(&self) -> String {
        let mut s = String::new();
        if self.attack_exertion > 0 {
            s.push_str("[exerted -]");
        }
        if self.is_dazed() {
            s.push_str("[dazed -]")
        }
        if self.conditions.godlike {
            s.push_str("[godline +]")
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
        s
    }

    fn receive_condition(&mut self, condition: Condition) {
        match condition {
            Condition::Dazed(n) => self.conditions.dazed += n,
            Condition::Godlike => self.conditions.godlike = true,
            Condition::Stunned => self.conditions.stunned = true,
            Condition::Bleeding => self.conditions.bleeding += 1,
            Condition::Braced => self.conditions.braced = true,
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
    on_heavy_hit: Option<OnHeavyHitEffect>,
}
