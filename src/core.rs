use std::cell::RefCell;
use std::cell::RefMut;

use crate::d20::{probability_of_d20_reaching, roll_d20_with_advantage};
use crate::data::{
    CRUSHING_STRIKE, DAGGER, FIREBALL, LEATHER_ARMOR, MIND_BLAST, RAGE, SCREAM, SIDE_STEP,
    SMALL_SHIELD, SWORD,
};

// You get this many AP per round
const ACTION_POINTS_PER_TURN: u32 = 6;
// You're not allowed to bring your AP below this number, with reactions
const REACTION_AP_THRESHOLD: u32 = 3;

pub struct CoreGame {
    characters: Characters,
    active_character_i: usize,
    player_character_i: usize,
}

impl CoreGame {
    pub fn active_character(&self) -> RefMut<Character> {
        self.characters.get(self.active_character_i).borrow_mut()
    }

    pub fn other_character(&self) -> RefMut<Character> {
        let i = (self.active_character_i + 1) % self.characters.0.len();
        self.characters.get(i).borrow_mut()
    }

    pub fn new() -> WaitingFor {
        let mut bob = Character::new("Bob", 5, 5, 4);
        bob.main_hand.weapon = Some(DAGGER);
        bob.off_hand.shield = Some(SMALL_SHIELD);
        bob.known_attack_enhancements.push(CRUSHING_STRIKE);
        bob.known_attacked_reactions.push(SIDE_STEP);
        bob.known_on_hit_reactions.push(RAGE);
        bob.known_actions.push(BaseAction::CastSpell(SCREAM));
        bob.known_actions.push(BaseAction::CastSpell(MIND_BLAST));
        bob.known_actions.push(BaseAction::CastSpell(FIREBALL));

        let mut alice = Character::new("Alice", 2, 7, 3);
        alice.main_hand.weapon = Some(SWORD);
        alice.armor = Some(LEATHER_ARMOR);

        bob.action_points = ACTION_POINTS_PER_TURN;
        alice.action_points = ACTION_POINTS_PER_TURN;

        let characters = Characters::new(vec![bob, alice]);
        let game = Self {
            characters,
            active_character_i: 0,
            player_character_i: 0,
        };

        let character = game.active_character();

        println!();
        println!(
            "({} AP, {}/{} stamina, {}/{} mana)",
            character.action_points,
            character.stamina.current,
            character.stamina.max,
            character.mana.current,
            character.mana.max
        );

        drop(character);
        WaitingFor::Action(WaitingForAction { game })
    }

    fn enter_state_action(self, action: Action) -> WaitingFor {
        let players_turn = self.player_character_i == self.active_character_i;
        let mut character = self.active_character();
        let mut other_character = self.other_character();
        let action_points_before_action = character.action_points;

        match action {
            Action::Attack { hand, enhancements } => {
                character.action_points -= character.weapon(hand).unwrap().action_point_cost;
                for enhancement in &enhancements {
                    character.action_points -= enhancement.action_point_cost;
                    character.stamina.lose(enhancement.stamina_cost);
                    if let Some(condition) = enhancement.apply_on_self_before {
                        character.receive_condition(condition);
                    }
                }

                print_attack_intro(&character, hand, &other_character);

                drop(character);
                drop(other_character);
                return if !players_turn {
                    WaitingFor::OnAttackedReaction(WaitingForOnAttackedReaction {
                        game: self,
                        action_points_before_action,
                        hand,
                        enhancements,
                    })
                } else {
                    self.enter_state_attack(action_points_before_action, hand, enhancements, None)
                };
            }
            Action::SelfEffect(SelfEffectAction {
                name: _,
                action_point_cost,
                effect,
            }) => {
                character.action_points -= action_point_cost;
                perform_effect_application(effect, &mut character, "");
            }
            Action::CastSpell { spell, enhanced } => {
                character.action_points -= spell.action_point_cost;
                character.mana.lose(spell.mana_cost);

                if enhanced {
                    character
                        .mana
                        .lose(spell.possible_enhancement.unwrap().mana_cost);
                }

                perform_spell(&mut character, spell, enhanced, &mut other_character);
            }
        }

        drop(character);
        drop(other_character);
        return self.enter_state_right_after_action(action_points_before_action, false);
    }

    fn enter_state_attack(
        self,
        action_points_before_action: u32,
        hand: HandType,
        enhancements: Vec<AttackEnhancement>,
        reaction: Option<OnAttackedReaction>,
    ) -> WaitingFor {
        let mut character = self.active_character();
        let mut other_character = self.other_character();

        if let Some(reaction) = reaction {
            other_character.action_points -= reaction.action_point_cost;
        }

        let did_attack_and_hit = perform_attack(
            &mut character,
            enhancements,
            hand,
            &mut other_character,
            reaction,
        );

        drop(character);
        drop(other_character);
        return self
            .enter_state_right_after_action(action_points_before_action, did_attack_and_hit);
    }

    fn enter_state_right_after_action(
        self,
        action_points_before_action: u32,
        did_attack_and_hit: bool,
    ) -> WaitingFor {
        let players_turn = self.player_character_i == self.active_character_i;
        let mut character = self.active_character();
        let mut other_character = self.other_character();

        // You recover from 1 stack of Dazed for each AP you spend
        // This must happen before "on attacked and hit" reactions because those might
        // inflict new Dazed stacks, which should not be covered here.
        let spent = action_points_before_action - character.action_points;
        if character.conditions.dazed > 0 {
            character.conditions.dazed = character.conditions.dazed.saturating_sub(spent);
            if character.conditions.dazed == 0 {
                println!("{} recovered from Dazed", character.name);
            }
        }

        if did_attack_and_hit {
            // You recover from 1 stack of Dazed each time you're hit by an attack
            if other_character.conditions.dazed > 0 {
                other_character.conditions.dazed -= 1;
                if character.conditions.dazed == 0 {
                    println!("{} recovered from Dazed", character.name);
                }
            }

            if !players_turn {
                drop(character);
                drop(other_character);
                return WaitingFor::OnAttackedHitReaction(WaitingForOnAttackedHitReaction {
                    game: self,
                });
            }
        }

        drop(character);
        drop(other_character);
        self.enter_state_longer_after_action()
    }

    fn enter_state_react_after_being_hit(
        self,
        reaction: Option<OnAttackedHitReaction>,
    ) -> WaitingFor {
        {
            let mut character = self.active_character();
            let mut other_character = self.other_character();

            if let Some(reaction) = reaction {
                other_character.action_points -= reaction.action_point_cost;
                match reaction.effect {
                    OnAttackedHitReactionEffect::Rage => {
                        println!("  {} started Raging", other_character.name);
                        other_character.receive_condition(Condition::Raging);
                    }
                    OnAttackedHitReactionEffect::ShieldBash => {
                        println!("  {} used Shield bash", other_character.name);

                        let target = character.physical_resistence();
                        let roll = roll_d20_with_advantage(0);
                        let res = roll + other_character.strength();
                        println!(
                            "Rolled: {} (+{} str) = {}, vs physical resist={}",
                            roll,
                            other_character.strength(),
                            res,
                            target,
                        );
                        if res >= target {
                            let stacks = if res < target + 5 {
                                println!("  Hit!");
                                1
                            } else if res < target + 10 {
                                println!("  Heavy hit!");
                                2
                            } else {
                                println!("  Critical hit!");
                                3
                            };
                            perform_effect_application(
                                ApplyEffect::Condition(Condition::Dazed(stacks)),
                                &mut character,
                                "Shield bash",
                            );
                        } else {
                            println!("  Miss!");
                        }
                    }
                }
            }
        }

        return self.enter_state_longer_after_action();
    }

    fn enter_state_longer_after_action(mut self) -> WaitingFor {
        let mut players_turn = self.player_character_i == self.active_character_i;
        let mut character = self.characters.get(self.active_character_i).borrow_mut();

        if character.action_points == 0 {
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

            if character.conditions.weakened > 0 {
                character.conditions.weakened = 0;
                println!("{} recovered from Weakened", character.name);
            }

            if character.conditions.raging {
                character.conditions.raging = false;
                println!("{} stopped Raging", character.name)
            }

            println!("End of turn.");

            character.action_points =
                (character.action_points + ACTION_POINTS_PER_TURN).min(ACTION_POINTS_PER_TURN);
            character.main_hand.exertion = 0;
            character.off_hand.exertion = 0;
            character.stamina.gain(1);

            self.active_character_i = (self.active_character_i + 1) % self.characters.0.len();
            players_turn = !players_turn;
            character = self.characters.get(self.active_character_i).borrow_mut();
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

        drop(character);

        if players_turn {
            return WaitingFor::Action(WaitingForAction { game: self });
        } else {
            return self.enter_state_action(Action::Attack {
                hand: HandType::MainHand,
                enhancements: Default::default(),
            });
        }
    }
}

pub enum WaitingFor {
    Action(WaitingForAction),
    OnAttackedReaction(WaitingForOnAttackedReaction),
    OnAttackedHitReaction(WaitingForOnAttackedHitReaction),
}

pub struct WaitingForAction {
    pub game: CoreGame,
}

impl WaitingForAction {
    pub fn commit(self, action: Action) -> WaitingFor {
        self.game.enter_state_action(action)
    }
}

pub struct WaitingForOnAttackedReaction {
    pub game: CoreGame,
    action_points_before_action: u32,
    hand: HandType,
    enhancements: Vec<AttackEnhancement>,
}

impl WaitingForOnAttackedReaction {
    pub fn commit(self, reaction: Option<OnAttackedReaction>) -> WaitingFor {
        return self.game.enter_state_attack(
            self.action_points_before_action,
            self.hand,
            self.enhancements,
            reaction,
        );
    }
}

pub struct WaitingForOnAttackedHitReaction {
    pub game: CoreGame,
}

impl WaitingForOnAttackedHitReaction {
    pub fn commit(self, reaction: Option<OnAttackedHitReaction>) -> WaitingFor {
        return self.game.enter_state_react_after_being_hit(reaction);
    }
}

fn print_attack_intro(attacker: &Character, hand: HandType, defender: &Character) {
    println!(
        "{} attacks {} (d20+{} vs {})",
        attacker.name,
        defender.name,
        attacker.attack_modifier(hand),
        defender.defense()
    );
    let explanation = format!(
        "{}{}",
        attacker.explain_attack_circumstances(hand),
        defender.explain_incoming_attack_circumstances()
    );
    if !explanation.is_empty() {
        println!("  {explanation}");
    }
    println!(
        "  Chance to hit: {}",
        as_percentage(prob_attack_hit(attacker, hand, defender))
    );
}

pub fn as_percentage(probability: f32) -> String {
    if !(0.01..=0.99).contains(&probability) {
        format!("{:.1}%", probability * 100f32)
    } else {
        format!("{:.0}%", probability * 100f32)
    }
}

pub fn prob_attack_hit(attacker: &Character, hand: HandType, defender: &Character) -> f32 {
    let advantage_level = attacker.attack_advantage(hand) + defender.incoming_attack_advantage();
    let target = defender
        .defense()
        .saturating_sub(attacker.attack_modifier(hand))
        .max(1);
    probability_of_d20_reaching(target, advantage_level)
}

pub fn prob_spell_hit(caster: &Character, spell_type: SpellType, defender: &Character) -> f32 {
    let defender_value = match spell_type {
        SpellType::Mental => defender.mental_resistence(),
        SpellType::Projectile => defender.defense(),
    };
    let target = defender_value.saturating_sub(caster.intellect()).max(1);
    probability_of_d20_reaching(target, 0)
}

fn perform_spell(caster: &mut Character, spell: Spell, enhanced: bool, defender: &mut Character) {
    let (target_label, target) = match spell.spell_type {
        SpellType::Mental => ("mental resist", defender.mental_resistence()),
        SpellType::Projectile => ("defense", defender.defense()),
    };

    let cast_n_times = if enhanced
        && spell.possible_enhancement.unwrap().effect == SpellEnhancementEffect::CastTwice
    {
        2
    } else {
        1
    };

    println!(
        "{} casts {} on {} (d20+{} vs {})",
        caster.name,
        spell.name,
        defender.name,
        caster.intellect(),
        target
    );

    for i in 0..cast_n_times {
        let roll = roll_d20_with_advantage(0);
        let res = roll + caster.intellect();
        println!(
            "Rolled: {} (+{} int) = {}, vs {}={}",
            roll,
            caster.intellect(),
            res,
            target_label,
            target,
        );
        if res >= target {
            println!("  The spell was successful!");
            let damage = spell.damage;
            if damage > 0 {
                defender.health.lose(damage);
                println!("  {} took {} damage", defender.name, damage);
            }

            if let Some(effect) = spell.on_hit_effect {
                perform_effect_application(effect, defender, spell.name);
            }

            match spell.possible_enhancement {
                Some(SpellEnhancement {
                    effect: SpellEnhancementEffect::OnHitEffect(effect),
                    ..
                }) if enhanced => {
                    perform_effect_application(effect, defender, "Spell enhancement");
                }
                _ => {}
            };
        } else {
            match spell.spell_type {
                SpellType::Mental => println!("  {} resisted the spell!", defender.name),
                SpellType::Projectile => println!("  The spell missed!"),
            }
        }

        if i < cast_n_times - 1 {
            println!("{} casts again!", caster.name)
        }
    }
}

fn perform_attack(
    attacker: &mut Character,
    attack_enhancements: Vec<AttackEnhancement>,
    hand_type: HandType,
    defender: &mut Character,
    defender_reaction: Option<OnAttackedReaction>,
) -> bool {
    let advantage = attacker.attack_advantage(hand_type) + defender.incoming_attack_advantage();

    let mut defense = defender.defense();

    let mut defender_reacted_with_parry = false;
    let mut defender_reacted_with_sidestep = false;
    let mut skip_attack_exertion = false;
    let mut did_hit = false;

    let attack_modifier = attacker.attack_modifier(hand_type);

    if let Some(reaction) = defender_reaction {
        match reaction.effect {
            OnAttackedReactionEffect::Parry => {
                defender_reacted_with_parry = true;
                let bonus_def = defender.attack_modifier(HandType::MainHand);
                println!(
                    "  Defense: {} +{} (Parry) = {}",
                    defense,
                    bonus_def,
                    defense + bonus_def
                );
                defense += bonus_def;
                let p_hit = probability_of_d20_reaching(defense - attack_modifier, advantage);
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
                let p_hit = probability_of_d20_reaching(defense - attack_modifier, advantage);
                println!("  Chance to hit: {:.1}%", p_hit * 100f32);
            }
        }
    }

    let roll = roll_d20_with_advantage(advantage);
    let res = roll + attack_modifier;
    match advantage.cmp(&0) {
        std::cmp::Ordering::Less => {
            println!("Rolling {} dice with disadvantage...", advantage.abs() + 1)
        }
        std::cmp::Ordering::Equal => println!("Rolling 1 die..."),
        std::cmp::Ordering::Greater => println!("Rolling {} dice with advantage...", advantage + 1),
    }
    println!(
        "Rolled: {} (+{}) = {}, vs def={}, armor={}",
        roll,
        attack_modifier,
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
        let weapon = attacker.weapon(hand_type).unwrap();
        let mut damage = weapon.damage;

        let mut damage_prefix = format!("  Damage: {} ({})", damage, weapon.name);

        if matches!(weapon.grip, WeaponGrip::Versatile) && attacker.off_hand.is_empty() {
            let bonus_dmg = 1;
            damage_prefix.push_str(&format!(" +{} (two-handed)", bonus_dmg));
            damage += bonus_dmg;
        }

        if res < defense + defender.protection_from_armor() {
            println!("Hit!");
            print!("{damage_prefix}");
        } else {
            on_true_hit_effect = weapon.on_true_hit;
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
        let hand = match hand_type {
            HandType::MainHand => &mut attacker.main_hand,
            HandType::OffHand => &mut attacker.off_hand,
        };
        hand.exertion += 1;
        println!("The attack led to exertion ({})", hand.exertion);
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

    did_hit
}

fn perform_effect_application(
    effect: ApplyEffect,
    receiver: &mut Character,
    context: &'static str,
) {
    match effect {
        ApplyEffect::RemoveActionPoints(n) => {
            receiver.action_points -= n;
            print!("  {} lost {} AP", receiver.name, n);
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
        Self(characters.into_iter().map(RefCell::new).collect())
    }

    fn get(&self, i: usize) -> &RefCell<Character> {
        self.0.get(i).unwrap()
    }
}

#[derive(Debug, Copy, Clone)]
pub struct AttackEnhancement {
    pub name: &'static str,
    pub action_point_cost: u32,
    pub stamina_cost: u32,
    pub bonus_damage: u32,
    pub apply_on_self_before: Option<Condition>,
    pub on_hit_effect: Option<ApplyEffect>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ApplyEffect {
    RemoveActionPoints(u32),
    Condition(Condition),
}

#[derive(Debug, Copy, Clone)]
pub struct OnAttackedReaction {
    pub name: &'static str,
    pub action_point_cost: u32,
    pub stamina_cost: u32,
    pub effect: OnAttackedReactionEffect,
}

#[derive(Debug, Copy, Clone)]
pub enum OnAttackedReactionEffect {
    Parry,
    SideStep,
}

#[derive(Debug, Copy, Clone)]
pub struct OnAttackedHitReaction {
    pub name: &'static str,
    pub action_point_cost: u32,
    pub effect: OnAttackedHitReactionEffect,
}

#[derive(Debug, Copy, Clone)]
pub enum OnAttackedHitReactionEffect {
    Rage,
    ShieldBash,
}

#[derive(Debug, Copy, Clone)]
pub enum AttackHitEffect {
    Apply(ApplyEffect),
    SkipExertion,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Condition {
    Dazed(u32),
    Bleeding,
    Braced,
    Raging,
    CarefulAim,
    Weakened(u32),
}

#[derive(Debug, Copy, Clone, Default)]
struct Conditions {
    dazed: u32,
    bleeding: u32,
    braced: bool,
    raging: bool,
    careful_aim: bool,
    weakened: u32,
}

#[derive(Debug)]
pub enum Action {
    Attack {
        hand: HandType,
        enhancements: Vec<AttackEnhancement>,
    },
    SelfEffect(SelfEffectAction),
    CastSpell {
        spell: Spell,
        enhanced: bool,
    },
}

#[derive(Debug, Copy, Clone)]
pub struct SelfEffectAction {
    pub name: &'static str,
    pub action_point_cost: u32,
    pub effect: ApplyEffect,
}

#[derive(Debug, Copy, Clone)]
pub enum BaseAction {
    Attack(HandType),
    SelfEffect(SelfEffectAction),
    CastSpell(Spell),
}

#[derive(Debug, Copy, Clone)]
pub enum HandType {
    MainHand,
    OffHand,
}

#[derive(Debug, Copy, Clone)]
pub struct Spell {
    pub name: &'static str,
    pub action_point_cost: u32,
    pub mana_cost: u32,
    pub damage: u32,
    pub on_hit_effect: Option<ApplyEffect>,
    pub spell_type: SpellType,
    pub possible_enhancement: Option<SpellEnhancement>,
}

#[derive(Debug, Copy, Clone)]
pub struct SpellEnhancement {
    pub name: &'static str,
    pub mana_cost: u32,
    pub effect: SpellEnhancementEffect,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SpellEnhancementEffect {
    CastTwice,
    OnHitEffect(ApplyEffect),
}

#[derive(Debug, Copy, Clone)]
pub enum SpellType {
    Mental,
    Projectile,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Hand {
    weapon: Option<Weapon>,
    shield: Option<Shield>,
    exertion: u32,
}

impl Hand {
    fn is_empty(&self) -> bool {
        self.weapon.is_none() && self.shield.is_none()
    }
}

#[derive(Debug)]
pub struct Character {
    name: &'static str,
    base_strength: u32,
    base_dexterity: u32,
    base_intellect: u32,
    health: NumberedResource,
    pub mana: NumberedResource,
    armor: Option<ArmorPiece>,
    main_hand: Hand,
    off_hand: Hand,
    conditions: Conditions,
    pub action_points: u32,
    pub stamina: NumberedResource,
    pub known_attack_enhancements: Vec<AttackEnhancement>,
    known_actions: Vec<BaseAction>,
    known_attacked_reactions: Vec<OnAttackedReaction>,
    known_on_hit_reactions: Vec<OnAttackedHitReaction>,
}

impl Character {
    fn new(name: &'static str, str: u32, dex: u32, int: u32) -> Self {
        let mana = if int < 3 { 0 } else { 1 + 2 * (int - 3) };
        Self {
            name,
            base_strength: str,
            base_dexterity: dex,
            base_intellect: int,
            health: NumberedResource::new(5 + str),
            mana: NumberedResource::new(mana),
            armor: None,
            main_hand: Default::default(),
            off_hand: Default::default(),
            conditions: Default::default(),
            action_points: 0,
            stamina: NumberedResource::new((str + dex).saturating_sub(5)),
            known_attack_enhancements: Default::default(),
            known_actions: vec![
                BaseAction::Attack(HandType::MainHand),
                BaseAction::Attack(HandType::OffHand),
                BaseAction::SelfEffect(SelfEffectAction {
                    name: "Brace",
                    action_point_cost: 1,
                    effect: ApplyEffect::Condition(Condition::Braced),
                }),
            ],
            known_attacked_reactions: Default::default(),
            known_on_hit_reactions: Default::default(),
        }
    }

    fn hand(&self, hand_type: HandType) -> &Hand {
        match hand_type {
            HandType::MainHand => &self.main_hand,
            HandType::OffHand => &self.off_hand,
        }
    }

    pub fn weapon(&self, hand: HandType) -> Option<Weapon> {
        self.hand(hand).weapon
    }

    pub fn usable_actions(&self) -> Vec<BaseAction> {
        let ap = self.action_points;
        self.known_actions
            .iter()
            .filter(|action: &&BaseAction| match action {
                BaseAction::Attack(hand) => {
                    matches!(self.weapon(*hand), Some(weapon) if ap >= weapon.action_point_cost)
                }
                BaseAction::SelfEffect(self_effect_action) => {
                    ap >= self_effect_action.action_point_cost
                }
                BaseAction::CastSpell(spell) => {
                    ap >= spell.action_point_cost && self.mana.current >= spell.mana_cost
                }
            })
            .copied()
            .collect()
    }

    pub fn usable_attack_enhancements(
        &self,
        attack_hand: HandType,
        available_action_points: u32,
    ) -> Vec<(String, AttackEnhancement)> {
        let weapon = self.weapon(attack_hand).unwrap();

        let mut available_attack_enhancements = vec![];
        if let Some(enhancement) = weapon.attack_enhancement {
            available_attack_enhancements.push((format!("{}: ", weapon.name), enhancement))
        }
        for enhancement in &self.known_attack_enhancements {
            available_attack_enhancements.push(("".to_owned(), *enhancement))
        }
        available_attack_enhancements
            .retain(|(_, enhancement)| available_action_points >= enhancement.action_point_cost);
        available_attack_enhancements
    }

    pub fn usable_on_attacked_reactions(&self) -> Vec<(String, OnAttackedReaction)> {
        let mut usable = vec![];
        for reaction in &self.known_attacked_reactions {
            if self.action_points - reaction.action_point_cost >= REACTION_AP_THRESHOLD {
                usable.push(("".to_string(), *reaction));
            }
        }
        // TODO: off-hand reactions?
        if let Some(weapon) = &self.main_hand.weapon {
            if let Some(reaction) = weapon.on_attacked_reaction {
                if self.action_points - reaction.action_point_cost >= REACTION_AP_THRESHOLD {
                    usable.push((format!("{}: ", weapon.name), reaction));
                }
            }
        }
        usable
    }

    pub fn usable_on_hit_reactions(&self) -> Vec<(String, OnAttackedHitReaction)> {
        let mut available_reactions = vec![];

        for reaction in &self.known_on_hit_reactions {
            if let OnAttackedHitReactionEffect::Rage = reaction.effect {
                if self.conditions.raging {
                    // Can't use this reaction while already raging
                    continue;
                }
            }
            if self.action_points - reaction.action_point_cost >= REACTION_AP_THRESHOLD {
                available_reactions.push(("".to_string(), *reaction));
            }
        }

        if let Some(shield) = self.off_hand.shield {
            if let Some(reaction) = shield.on_attacked_hit_reaction {
                if self.action_points - reaction.action_point_cost >= REACTION_AP_THRESHOLD {
                    available_reactions.push((format!("{}: ", shield.name), reaction));
                }
            }
        }

        available_reactions
    }

    fn strength(&self) -> u32 {
        (self.base_strength as i32 - self.conditions.weakened as i32).max(1) as u32
    }

    fn dexterity(&self) -> u32 {
        (self.base_dexterity as i32 - self.conditions.weakened as i32).max(1) as u32
    }

    fn intellect(&self) -> u32 {
        (self.base_intellect as i32 - self.conditions.weakened as i32).max(1) as u32
    }

    fn is_dazed(&self) -> bool {
        self.conditions.dazed > 0
    }

    fn defense(&self) -> u32 {
        let from_dex = self.defense_bonus_from_dexterity();
        let from_shield = self
            .off_hand
            .shield
            .map(|shield| shield.defense)
            .unwrap_or(0);
        let from_braced = if self.conditions.braced { 3 } else { 0 };
        10 + from_dex + from_shield + from_braced
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

    fn physical_resistence(&self) -> u32 {
        10 + self.strength()
    }

    fn protection_from_armor(&self) -> u32 {
        self.armor.map(|armor| armor.protection).unwrap_or(0)
    }

    fn attack_modifier(&self, hand: HandType) -> u32 {
        let str = self.strength();
        let dex = self.dexterity();
        let weapon = self.weapon(hand).unwrap();
        let mut modifier = match weapon.attack_attribute {
            AttackAttribute::Strength => str,
            AttackAttribute::Dexterity => dex,
            AttackAttribute::Finesse => str.max(dex),
        };
        if matches!(hand, HandType::OffHand) {
            modifier -= 3;
        }
        modifier
    }

    fn attack_advantage(&self, hand_type: HandType) -> i32 {
        let mut res = 0i32;

        res -= self.hand(hand_type).exertion as i32;
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

    fn explain_attack_circumstances(&self, hand_type: HandType) -> String {
        let mut s = String::new();
        if self.hand(hand_type).exertion > 0 {
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
        0
    }

    fn explain_incoming_attack_circumstances(&self) -> String {
        let mut s = String::new();
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
            Condition::Bleeding => self.conditions.bleeding += 1,
            Condition::Braced => self.conditions.braced = true,
            Condition::Raging => self.conditions.raging = true,
            Condition::CarefulAim => self.conditions.careful_aim = true,
            Condition::Weakened(n) => self.conditions.weakened += n,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct NumberedResource {
    pub current: u32,
    pub max: u32,
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
pub struct ArmorPiece {
    pub(crate) protection: u32,
    pub(crate) limit_defense_from_dex: Option<u32>,
}

#[derive(Debug, Copy, Clone)]
pub struct Weapon {
    pub name: &'static str,
    pub action_point_cost: u32,
    pub damage: u32,
    pub grip: WeaponGrip,
    pub attack_attribute: AttackAttribute,
    pub attack_enhancement: Option<AttackEnhancement>,
    pub on_attacked_reaction: Option<OnAttackedReaction>,
    pub on_true_hit: Option<AttackHitEffect>,
}

#[derive(Debug, Copy, Clone)]
pub struct Shield {
    pub name: &'static str,
    pub defense: u32,
    pub on_attacked_hit_reaction: Option<OnAttackedHitReaction>,
}

#[derive(Debug, Copy, Clone)]
pub enum AttackAttribute {
    Strength,
    Dexterity,
    Finesse,
}

#[derive(Debug, Copy, Clone)]
pub enum WeaponGrip {
    Light,
    MainHand,
    Versatile,
    TwoHanded,
}
