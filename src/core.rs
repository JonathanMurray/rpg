use std::cell::RefCell;
use std::cell::RefMut;
use std::rc::Rc;

use macroquad::rand;
use macroquad::rand::ChooseRandom;

use crate::d20::{probability_of_d20_reaching, roll_d20_with_advantage};
use crate::data::BOW;
use crate::data::WAR_HAMMER;
use crate::data::{
    CRUSHING_STRIKE, DAGGER, FIREBALL, LEATHER_ARMOR, MIND_BLAST, RAGE, SCREAM, SIDE_STEP,
    SMALL_SHIELD, SWORD,
};

// You get this many AP per round
const ACTION_POINTS_PER_TURN: u32 = 6;

pub struct CoreGame {
    characters: Characters,
    pub active_character_i: usize,
    player_character_i: usize,
    logger: Rc<RefCell<dyn Logger>>,
}

impl CoreGame {
    pub fn new(logger: Rc<RefCell<dyn Logger>>) -> Self {
        let mut bob = Character::new("Bob", 5, 1, 4, (1, 4));
        bob.main_hand.weapon = Some(WAR_HAMMER);
        bob.off_hand.shield = None;
        bob.known_attack_enhancements.push(CRUSHING_STRIKE);
        bob.known_attacked_reactions.push(SIDE_STEP);
        bob.known_on_hit_reactions.push(RAGE);
        bob.known_actions.push(BaseAction::CastSpell(SCREAM));
        bob.known_actions.push(BaseAction::CastSpell(MIND_BLAST));
        bob.known_actions.push(BaseAction::CastSpell(FIREBALL));

        let mut alice = Character::new("Alice", 2, 2, 3, (2, 4));
        alice.main_hand.weapon = Some(BOW);
        alice.armor = Some(LEATHER_ARMOR);

        bob.action_points = ACTION_POINTS_PER_TURN;
        alice.action_points = ACTION_POINTS_PER_TURN;

        let characters = Characters::new(vec![bob, alice]);
        Self {
            characters,
            player_character_i: 0,
            active_character_i: 0,
            logger,
        }
    }

    pub fn begin(self) -> GameState {
        if self.active_character_i == self.player_character_i {
            GameState::AwaitingPlayerAction(StateChooseAction { game: self })
        } else {
            GameState::AwaitingBot(StateAwaitingBot { game: self })
        }
    }

    pub fn player_character(&self) -> &Rc<RefCell<Character>> {
        self.characters.get(self.player_character_i)
    }

    pub fn non_player_character(&self) -> &Rc<RefCell<Character>> {
        let i = (self.player_character_i + 1) % self.characters.0.len();
        self.characters.get(i)
    }

    pub fn active_character(&self) -> RefMut<Character> {
        self.characters.get(self.active_character_i).borrow_mut()
    }

    pub fn characters(&self) -> &[Rc<RefCell<Character>>] {
        &self.characters.0
    }

    pub fn inactive_character(&self) -> RefMut<Character> {
        let i = (self.active_character_i + 1) % self.characters.0.len();
        self.characters.get(i).borrow_mut()
    }

    fn enter_state_action(self, action: Action) -> GameState {
        let players_turn = self.player_character_i == self.active_character_i;
        let mut character = self.active_character();
        let mut other_character = self.inactive_character();
        let action_points_before_action = character.action_points;

        match action {
            Action::Attack { hand, enhancements } => {
                let mut attacker = character;
                let defender = other_character;

                assert!(attacker.can_reach_with_attack(hand, defender.position));

                attacker.action_points -= attacker.weapon(hand).unwrap().action_point_cost;
                for enhancement in &enhancements {
                    attacker.action_points -= enhancement.action_point_cost;
                    attacker.stamina.lose(enhancement.stamina_cost);
                    if let Some(condition) = enhancement.apply_on_self_before {
                        attacker.receive_condition(condition);
                    }
                }
                let enhancements_str = if !enhancements.is_empty() {
                    let names: Vec<String> = enhancements
                        .iter()
                        .map(|enhancement| enhancement.name.to_string())
                        .collect();
                    format!(" ({})", names.join(", "))
                } else {
                    "".to_string()
                };

                let attacks_str = format!(
                    "{} attacks {} (d20+{} vs {}){}",
                    attacker.name,
                    defender.name,
                    attacker.attack_modifier(hand),
                    defender.defense(),
                    enhancements_str
                );
                self.log(attacks_str.clone());

                let mut lines = vec![];
                lines.push(attacks_str);
                let explanation = format!(
                    "{}{}",
                    attacker.explain_attack_circumstances(hand),
                    defender.explain_incoming_attack_circumstances()
                );
                if !explanation.is_empty() {
                    lines.push(format!("  {explanation}"));
                }
                lines.push(format!(
                    "  Chance to hit: {}",
                    as_percentage(prob_attack_hit(&attacker, hand, &defender))
                ));

                drop(attacker);
                drop(defender);
                return if !players_turn {
                    GameState::AwaitingPlayerAttackReaction(StateReactToAttack {
                        game: self,
                        action_points_before_action,
                        hand,
                        enhancements,
                        lines,
                    })
                } else {
                    self.enter_state_attack(action_points_before_action, hand, enhancements, None)
                };
            }
            Action::SelfEffect(SelfEffectAction {
                name,
                action_point_cost,
                effect,
                ..
            }) => {
                character.action_points -= action_point_cost;

                self.log(format!("{} uses {}", character.name, name));
                self.perform_effect_application(effect, &mut character, "");
            }
            Action::CastSpell { spell, enhanced } => {
                self.perform_spell(&mut character, spell, enhanced, &mut other_character);
            }
            Action::Move { action_point_cost } => {
                character.action_points -= action_point_cost;
                let new_position = (character.position.0 + 1, character.position.1);
                if other_character.position == new_position {
                    self.log(format!(
                        "{} tried moving but position was blocked",
                        character.name
                    ));
                } else {
                    self.log(format!(
                        "{} moved from {:?} to {:?}",
                        character.name, character.position, new_position
                    ));
                    character.position = new_position;
                }
            }
        }

        drop(character);
        drop(other_character);
        return self.enter_state_right_after_action(action_points_before_action, None);
    }

    fn perform_effect_application(
        &self,
        effect: ApplyEffect,
        receiver: &mut Character,
        context: &'static str,
    ) {
        let mut line = String::new();
        match effect {
            ApplyEffect::RemoveActionPoints(n) => {
                receiver.action_points -= n;
                line.push_str(&format!("  {} lost {} AP", receiver.name, n));
            }
            ApplyEffect::Condition(condition) => {
                receiver.receive_condition(condition);
                line.push_str(&format!("  {} received {:?}", receiver.name, condition));
            }
        }
        line.push_str(&format!(" ({})", context));
        self.log(line);
    }

    fn perform_spell(
        &self,
        caster: &mut Character,
        spell: Spell,
        enhanced: bool,
        defender: &mut Character,
    ) {
        assert!(caster.action_points >= spell.action_point_cost);
        caster.action_points -= spell.action_point_cost;
        caster.mana.lose(spell.mana_cost);

        let mut enhancement_str = String::new();
        if enhanced {
            let enhancement = spell.possible_enhancement.unwrap();
            caster.mana.lose(enhancement.mana_cost);

            enhancement_str = format!(" ({})", enhancement.name)
        }

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

        self.log(format!(
            "{} casts {} on {} (d20+{} vs {}){}",
            caster.name,
            spell.name,
            defender.name,
            caster.intellect(),
            target,
            enhancement_str
        ));

        for i in 0..cast_n_times {
            let roll = roll_d20_with_advantage(0);
            let res = roll + caster.intellect();
            self.log(format!(
                "Rolled: {} (+{} int) = {}, vs {}={}",
                roll,
                caster.intellect(),
                res,
                target_label,
                target,
            ));
            if res >= target {
                self.log("  The spell was successful!");
                let damage = spell.damage;
                if damage > 0 {
                    defender.health.lose(damage);
                    self.log(format!("  {} took {} damage", defender.name, damage));
                }

                if let Some(effect) = spell.on_hit_effect {
                    self.perform_effect_application(effect, defender, spell.name);
                }

                match spell.possible_enhancement {
                    Some(SpellEnhancement {
                        effect: SpellEnhancementEffect::OnHitEffect(effect),
                        name,
                        ..
                    }) if enhanced => {
                        self.perform_effect_application(effect, defender, name);
                    }
                    _ => {}
                };
            } else {
                match spell.spell_type {
                    SpellType::Mental => {
                        self.log(format!("  {} resisted the spell!", defender.name))
                    }
                    SpellType::Projectile => self.log("  The spell missed!"),
                }
            }

            if i < cast_n_times - 1 {
                self.log(format!("{} casts again!", caster.name))
            }
        }
    }

    fn log(&self, line: impl Into<String>) {
        self.logger.borrow_mut().log(line.into());
    }

    fn enter_state_attack(
        self,
        action_points_before_action: u32,
        hand_type: HandType,
        attack_enhancements: Vec<AttackEnhancement>,
        defender_reaction: Option<OnAttackedReaction>,
    ) -> GameState {
        let mut attacker = self.active_character();
        let mut defender = self.inactive_character();

        let advantage = attacker.attack_advantage(hand_type) + defender.incoming_attack_advantage();

        let mut defense = defender.defense();

        let mut defender_reacted_with_parry = false;
        let mut defender_reacted_with_sidestep = false;
        let mut skip_attack_exertion = false;

        let mut attack_hit_lines = None;

        let attack_modifier = attacker.attack_modifier(hand_type);

        if let Some(reaction) = defender_reaction {
            defender.action_points -= reaction.action_point_cost;
            defender.stamina.lose(reaction.stamina_cost);

            self.log(format!("{} reacts with {}", defender.name, reaction.name));

            match reaction.effect {
                OnAttackedReactionEffect::Parry => {
                    defender_reacted_with_parry = true;
                    let bonus_def = defender.attack_modifier(HandType::MainHand);

                    self.log(format!(
                        "  Defense: {} +{} (Parry) = {}",
                        defense,
                        bonus_def,
                        defense + bonus_def
                    ));
                    defense += bonus_def;
                    let p_hit = probability_of_d20_reaching(defense - attack_modifier, advantage);
                    self.log(format!("  Chance to hit: {:.1}%", p_hit * 100f32));
                }
                OnAttackedReactionEffect::SideStep => {
                    defender_reacted_with_sidestep = true;
                    let bonus_def = defender.defense_bonus_from_dexterity();
                    self.log(format!(
                        "  Defense: {} +{} (Side step) = {}",
                        defense,
                        bonus_def,
                        defense + bonus_def
                    ));
                    defense += bonus_def;
                    let p_hit = probability_of_d20_reaching(defense - attack_modifier, advantage);
                    self.log(format!("  Chance to hit: {:.1}%", p_hit * 100f32));
                }
            }
        }

        let roll = roll_d20_with_advantage(advantage);
        let res = roll + attack_modifier;
        match advantage.cmp(&0) {
            std::cmp::Ordering::Less => self.log(format!(
                "Rolling {} dice with disadvantage...",
                advantage.abs() + 1
            )),
            std::cmp::Ordering::Equal => self.log(format!("Rolling 1 die...")),
            std::cmp::Ordering::Greater => {
                self.log(format!("Rolling {} dice with advantage...", advantage + 1))
            }
        }
        self.log(format!(
            "Rolled: {} (+{}) = {}, vs def={}, armor={}",
            roll,
            attack_modifier,
            res,
            defense,
            defender.protection_from_armor()
        ));

        if res < defense {
            if defender_reacted_with_parry {
                self.log("  Parried!");
            } else if defender_reacted_with_sidestep {
                self.log("  Side stepped!");
            } else {
                self.log("  Missed!")
            }
        } else {
            let mut on_true_hit_effect = None;
            let weapon = attacker.weapon(hand_type).unwrap();
            let mut damage = weapon.damage;

            let mut dmg_str = format!("  Damage: {} ({})", damage, weapon.name);

            if matches!(weapon.grip, WeaponGrip::Versatile) && attacker.off_hand.is_empty() {
                let bonus_dmg = 1;
                dmg_str.push_str(&format!(" +{} (two-handed)", bonus_dmg));
                damage += bonus_dmg;
            }

            if res < defense + defender.protection_from_armor() {
                self.log("  Hit!".to_string());
            } else {
                on_true_hit_effect = weapon.on_true_hit;
                let (label, bonus_dmg) = if res < defense + defender.protection_from_armor() + 5 {
                    ("True hit", 1)
                } else if res < defense + defender.protection_from_armor() + 10 {
                    ("Heavy hit", 2)
                } else {
                    ("Critical hit", 3)
                };
                self.log(format!("  {label}!"));
                dmg_str.push_str(&format!(" +{bonus_dmg} ({label})"));
                damage += bonus_dmg;
            }

            for enhancement in &attack_enhancements {
                if enhancement.bonus_damage > 0 {
                    dmg_str.push_str(&format!(
                        " +{} ({})",
                        enhancement.bonus_damage, enhancement.name
                    ));
                    damage += enhancement.bonus_damage;
                }
            }
            dmg_str.push_str(&format!(" = {damage}"));

            self.log(dmg_str);

            defender.health.lose(damage);

            self.log(format!(
                "  {} took {} damage and went down to {}/{} health",
                defender.name, damage, defender.health.current, defender.health.max
            ));

            attack_hit_lines = Some(vec![format!(
                "{} took {} damage from an attack by {}",
                defender.name, damage, attacker.name
            )]);

            if let Some(effect) = on_true_hit_effect {
                match effect {
                    AttackHitEffect::Apply(effect) => {
                        self.perform_effect_application(effect, &mut defender, "true hit");
                    }
                    AttackHitEffect::SkipExertion => skip_attack_exertion = true,
                }
            }

            for enhancement in &attack_enhancements {
                if let Some(effect) = enhancement.on_hit_effect {
                    self.perform_effect_application(effect, &mut defender, enhancement.name);
                }
            }
        }

        if skip_attack_exertion {
            self.log("  The attack did not lead to exertion (true hit)".to_string());
        } else {
            let hand = match hand_type {
                HandType::MainHand => &mut attacker.main_hand,
                HandType::OffHand => &mut attacker.off_hand,
            };
            hand.exertion += 1;
            self.log(format!("  The attack led to exertion ({})", hand.exertion));
        }

        if attacker.conditions.careful_aim {
            attacker.conditions.careful_aim = false;
            self.log(format!("{} lost Careful aim", attacker.name));
        }

        if defender.conditions.braced {
            defender.conditions.braced = false;
            self.log(format!("{} lost Braced", defender.name));
        }

        drop(attacker);
        drop(defender);

        return self.enter_state_right_after_action(action_points_before_action, attack_hit_lines);
    }

    fn enter_state_right_after_action(
        self,
        action_points_before_action: u32,
        attack_hit_lines: Option<Vec<String>>,
    ) -> GameState {
        let players_turn = self.player_character_i == self.active_character_i;
        let mut character = self.active_character();
        let mut other_character = self.inactive_character();

        // You recover from 1 stack of Dazed for each AP you spend
        // This must happen before "on attacked and hit" reactions because those might
        // inflict new Dazed stacks, which should not be covered here.
        let spent = action_points_before_action - character.action_points;
        self.perform_recover_from_dazed(&mut character, spent);

        if let Some(lines) = attack_hit_lines {
            // You recover from 1 stack of Dazed each time you're hit by an attack
            self.perform_recover_from_dazed(&mut other_character, 1);

            if !players_turn {
                drop(character);
                drop(other_character);
                return GameState::AwaitingPlayerHitReaction(StateReactToHit { game: self, lines });
            }
        }

        drop(character);
        drop(other_character);
        self.enter_state_longer_after_action()
    }

    fn perform_recover_from_dazed(&self, character: &mut Character, stacks: u32) {
        if character.conditions.dazed > 0 {
            character.conditions.dazed = character.conditions.dazed.saturating_sub(stacks);
            if character.conditions.dazed == 0 {
                self.log(format!("{} recovered from Dazed", character.name));
            }
        }
    }

    fn enter_state_react_after_being_hit(self, reaction: Option<OnHitReaction>) -> GameState {
        {
            let mut character = self.active_character();
            let mut other_character = self.inactive_character();

            if let Some(reaction) = reaction {
                self.perform_on_hit_reaction(&mut character, &mut other_character, reaction);
            }
        }

        return self.enter_state_longer_after_action();
    }

    fn perform_on_hit_reaction(
        &self,
        character: &mut Character,
        other_character: &mut Character,
        reaction: OnHitReaction,
    ) {
        other_character.action_points -= reaction.action_point_cost;
        match reaction.effect {
            OnHitReactionEffect::Rage => {
                self.log(format!("  {} started Raging", other_character.name));
                other_character.receive_condition(Condition::Raging);
            }
            OnHitReactionEffect::ShieldBash => {
                self.log(format!("  {} used Shield bash", other_character.name));

                let target = character.physical_resistence();
                let roll = roll_d20_with_advantage(0);
                let res = roll + other_character.strength();
                self.log(format!(
                    "Rolled: {} (+{} str) = {}, vs physical resist={}",
                    roll,
                    other_character.strength(),
                    res,
                    target,
                ));
                if res >= target {
                    let stacks = if res < target + 5 {
                        self.log("  Hit!");
                        1
                    } else if res < target + 10 {
                        self.log("  Heavy hit!");
                        2
                    } else {
                        self.log("  Critical hit!");
                        3
                    };
                    self.perform_effect_application(
                        ApplyEffect::Condition(Condition::Dazed(stacks)),
                        character,
                        "Shield bash",
                    );
                } else {
                    self.log("  Miss!");
                }
            }
        }
    }

    fn enter_state_longer_after_action(mut self) -> GameState {
        let mut players_turn = self.player_character_i == self.active_character_i;
        let mut character = self.characters.get(self.active_character_i).borrow_mut();

        if character.action_points == 0 {
            self.perform_end_of_turn_character(&mut character);
            self.active_character_i = (self.active_character_i + 1) % self.characters.0.len();
            players_turn = !players_turn;
            character = self.characters.get(self.active_character_i).borrow_mut();
        }

        if players_turn {
            drop(character);
            GameState::AwaitingPlayerAction(StateChooseAction { game: self })
        } else {
            drop(character);
            GameState::AwaitingBot(StateAwaitingBot { game: self })
        }
    }

    fn perform_end_of_turn_character(&self, character: &mut Character) {
        if character.conditions.bleeding > 0 {
            character.health.lose(1);
            self.log(format!(
                "{} took 1 damage from Bleeding and went down to {}/{} health",
                character.name, character.health.current, character.health.max
            ));
            character.conditions.bleeding -= 1;
            if character.conditions.bleeding == 0 {
                self.log(format!("{} stopped Bleeding", character.name));
            }
        }

        if character.conditions.weakened > 0 {
            character.conditions.weakened = 0;
            self.log(format!("{} recovered from Weakened", character.name));
        }

        if character.conditions.raging {
            character.conditions.raging = false;
            self.log(format!("{} stopped Raging", character.name))
        }

        character.action_points =
            (character.action_points + ACTION_POINTS_PER_TURN).min(ACTION_POINTS_PER_TURN);
        character.main_hand.exertion = 0;
        character.off_hand.exertion = 0;
        character.stamina.gain(1);

        self.log("End of turn.");
    }
}

pub enum GameState {
    AwaitingPlayerAction(StateChooseAction),
    AwaitingPlayerAttackReaction(StateReactToAttack),
    AwaitingPlayerHitReaction(StateReactToHit),
    AwaitingBot(StateAwaitingBot),
}

impl GameState {
    pub fn game(&self) -> &CoreGame {
        match self {
            GameState::AwaitingPlayerAction(this) => &this.game,
            GameState::AwaitingPlayerAttackReaction(this) => &this.game,
            GameState::AwaitingPlayerHitReaction(this) => &this.game,
            GameState::AwaitingBot(this) => &this.game,
        }
    }

    pub fn unwrap_choose_action(self) -> StateChooseAction {
        match self {
            GameState::AwaitingPlayerAction(inner) => inner,
            _ => panic!(),
        }
    }

    pub fn unwrap_react_to_attack(self) -> StateReactToAttack {
        match self {
            GameState::AwaitingPlayerAttackReaction(inner) => inner,
            _ => panic!(),
        }
    }

    pub fn unwrap_react_to_hit(self) -> StateReactToHit {
        match self {
            GameState::AwaitingPlayerHitReaction(inner) => inner,
            _ => panic!(),
        }
    }
}

pub trait Logger {
    fn log(&mut self, line: String);
}

pub struct StateChooseAction {
    pub game: CoreGame,
}

impl StateChooseAction {
    pub fn proceed(self, action: Action) -> GameState {
        self.game.enter_state_action(action)
    }
}

pub struct StateReactToAttack {
    pub game: CoreGame,
    action_points_before_action: u32,
    hand: HandType,
    enhancements: Vec<AttackEnhancement>,
    pub lines: Vec<String>,
}

impl StateReactToAttack {
    pub fn proceed(self, reaction: Option<OnAttackedReaction>) -> GameState {
        return self.game.enter_state_attack(
            self.action_points_before_action,
            self.hand,
            self.enhancements,
            reaction,
        );
    }
}

pub struct StateReactToHit {
    pub game: CoreGame,
    pub lines: Vec<String>,
}

impl StateReactToHit {
    pub fn proceed(self, reaction: Option<OnHitReaction>) -> GameState {
        return self.game.enter_state_react_after_being_hit(reaction);
    }
}

pub struct StateAwaitingBot {
    pub game: CoreGame,
}

impl StateAwaitingBot {
    pub fn proceed(self) -> GameState {
        // TODO make sure to only pick an action that the character can actually do (afford, in range, etc)
        let game = self.game;
        let character = game.active_character();
        let other_character = game.inactive_character();

        let mut actions = character.usable_actions();
        let mut chosen_action = None;

        while !actions.is_empty() {
            let i = rand::gen_range(0, actions.len());
            let action = actions.swap_remove(i);

            if character.can_use_action(action) {
                match action {
                    BaseAction::Attack { hand, .. } => {
                        if character.can_reach_with_attack(hand, other_character.position) {
                            chosen_action = Some(Action::Attack {
                                hand,
                                enhancements: vec![],
                            });
                        }
                    }
                    BaseAction::SelfEffect(sea) => chosen_action = Some(Action::SelfEffect(sea)),
                    BaseAction::CastSpell(spell) => {
                        chosen_action = Some(Action::CastSpell {
                            spell,
                            enhanced: false,
                        });
                    }
                    BaseAction::Move { action_point_cost } => {
                        chosen_action = Some(Action::Move { action_point_cost });
                    }
                }
            }
            if chosen_action.is_some() {
                break;
            }
        }

        drop(character);
        drop(other_character);

        game.enter_state_action(chosen_action.unwrap())
    }
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

struct Characters(Vec<Rc<RefCell<Character>>>);

impl Characters {
    fn new(characters: Vec<Character>) -> Self {
        Self(
            characters
                .into_iter()
                .map(|ch| Rc::new(RefCell::new(ch)))
                .collect(),
        )
    }

    fn get(&self, i: usize) -> &Rc<RefCell<Character>> {
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
pub struct OnHitReaction {
    pub name: &'static str,
    pub action_point_cost: u32,
    pub effect: OnHitReactionEffect,
}

#[derive(Debug, Copy, Clone)]
pub enum OnHitReactionEffect {
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
    Move {
        action_point_cost: u32,
    },
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct SelfEffectAction {
    pub name: &'static str,
    pub description: &'static str,
    pub action_point_cost: u32,
    pub effect: ApplyEffect,
}

#[derive(Debug, Copy, Clone)]
pub enum BaseAction {
    Attack {
        hand: HandType,
        action_point_cost: u32,
    },
    SelfEffect(SelfEffectAction),
    CastSpell(Spell),
    Move {
        action_point_cost: u32,
    },
}

impl BaseAction {
    pub fn action_point_cost(&self) -> u32 {
        match self {
            BaseAction::Attack {
                hand: _,
                action_point_cost,
            } => *action_point_cost,
            BaseAction::SelfEffect(self_effect_action) => self_effect_action.action_point_cost,
            BaseAction::CastSpell(spell) => spell.action_point_cost,
            BaseAction::Move { action_point_cost } => *action_point_cost,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum HandType {
    MainHand,
    OffHand,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Spell {
    pub name: &'static str,
    pub description: &'static str,
    pub action_point_cost: u32,
    pub mana_cost: u32,
    pub damage: u32,
    pub on_hit_effect: Option<ApplyEffect>,
    pub spell_type: SpellType,
    pub possible_enhancement: Option<SpellEnhancement>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
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

#[derive(Debug, Copy, Clone, PartialEq)]
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
    pub position: (u32, u32),
    pub name: &'static str,
    pub base_strength: u32,
    pub base_dexterity: u32,
    pub base_intellect: u32,
    pub health: NumberedResource,
    pub mana: NumberedResource,
    pub move_speed: f32,
    armor: Option<ArmorPiece>,
    main_hand: Hand,
    off_hand: Hand,
    conditions: Conditions,
    pub action_points: u32,
    pub stamina: NumberedResource,
    pub known_attack_enhancements: Vec<AttackEnhancement>,
    known_actions: Vec<BaseAction>,
    known_attacked_reactions: Vec<OnAttackedReaction>,
    known_on_hit_reactions: Vec<OnHitReaction>,
}

impl Character {
    fn new(name: &'static str, str: u32, dex: u32, int: u32, position: (u32, u32)) -> Self {
        let mana = if int < 3 { 0 } else { 1 + 2 * (int - 3) };
        Self {
            position,
            name,
            base_strength: str,
            base_dexterity: dex,
            base_intellect: int,
            health: NumberedResource::new(5 + str),
            mana: NumberedResource::new(mana),
            move_speed: 1.0 + dex as f32 * 0.25,
            armor: None,
            main_hand: Default::default(),
            off_hand: Default::default(),
            conditions: Default::default(),
            action_points: 0,
            stamina: NumberedResource::new((str + dex).saturating_sub(5)),
            known_attack_enhancements: Default::default(),
            known_actions: vec![
                BaseAction::Attack {
                    hand: HandType::MainHand,
                    action_point_cost: 0,
                },
                BaseAction::Attack {
                    hand: HandType::OffHand,
                    action_point_cost: 0,
                },
                BaseAction::SelfEffect(SelfEffectAction {
                    name: "Brace",
                    description: "+defense the next time you're attacked",
                    action_point_cost: 1,
                    effect: ApplyEffect::Condition(Condition::Braced),
                }),
                BaseAction::Move {
                    action_point_cost: 1,
                },
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

    pub fn can_reach_with_attack(&self, hand: HandType, target_position: (u32, u32)) -> bool {
        let weapon = self.weapon(hand).unwrap();
        let range_squared = match weapon.range {
            WeaponRange::Melee => 2,
            WeaponRange::Ranged(r) => r * r,
        } as i32;
        let distance_squared = (target_position.0 as i32 - self.position.0 as i32).pow(2)
            + (target_position.1 as i32 - self.position.1 as i32).pow(2);
        distance_squared <= range_squared
    }

    pub fn known_actions(&self) -> Vec<(String, BaseAction)> {
        self.known_actions
            .iter()
            .filter_map(|action: &BaseAction| match action {
                BaseAction::Attack { hand, .. } => self.weapon(*hand).map(|weapon| {
                    (
                        weapon.name.to_string(),
                        BaseAction::Attack {
                            hand: *hand,
                            action_point_cost: weapon.action_point_cost,
                        },
                    )
                }),
                BaseAction::SelfEffect(_self_effect_action) => Some(("".to_string(), *action)),
                BaseAction::CastSpell(_spell) => Some(("".to_string(), *action)),
                BaseAction::Move { .. } => Some(("".to_string(), *action)),
            })
            .collect()
    }

    pub fn usable_actions(&self) -> Vec<BaseAction> {
        self.known_actions()
            .iter()
            .filter_map(|(_, action)| {
                if self.can_use_action(*action) {
                    Some(*action)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn can_use_action(&self, action: BaseAction) -> bool {
        let ap = self.action_points;
        match action {
            BaseAction::Attack {
                hand,
                action_point_cost: _,
            } => matches!(self.weapon(hand), Some(weapon) if ap >= weapon.action_point_cost),
            BaseAction::SelfEffect(self_effect_action) => {
                ap >= self_effect_action.action_point_cost
            }
            BaseAction::CastSpell(spell) => {
                ap >= spell.action_point_cost && self.mana.current >= spell.mana_cost
            }
            BaseAction::Move { action_point_cost } => ap >= action_point_cost,
        }
    }

    pub fn known_attack_enhancements(
        &self,
        attack_hand: HandType,
    ) -> Vec<(String, AttackEnhancement)> {
        let weapon = self.weapon(attack_hand).unwrap();

        let mut usable = vec![];
        if let Some(enhancement) = weapon.attack_enhancement {
            usable.push((weapon.name.to_string(), enhancement))
        }
        for enhancement in &self.known_attack_enhancements {
            usable.push(("".to_owned(), *enhancement))
        }
        usable
    }

    pub fn usable_attack_enhancements(
        &self,
        attack_hand: HandType,
    ) -> Vec<(String, AttackEnhancement)> {
        let mut usable = self.known_attack_enhancements(attack_hand);

        usable.retain(|(_, enhancement)| self.can_use_attack_enhancement(attack_hand, enhancement));
        usable
    }

    pub fn can_use_attack_enhancement(
        &self,
        attack_hand: HandType,
        enhancement: &AttackEnhancement,
    ) -> bool {
        let weapon = self.weapon(attack_hand).unwrap();
        self.action_points >= weapon.action_point_cost + enhancement.action_point_cost
            && self.stamina.current >= enhancement.stamina_cost
    }

    pub fn known_on_attacked_reactions(&self) -> Vec<(String, OnAttackedReaction)> {
        let mut known = vec![];
        for reaction in &self.known_attacked_reactions {
            known.push(("".to_string(), *reaction));
        }
        // TODO: off-hand reactions?
        if let Some(weapon) = &self.main_hand.weapon {
            if let Some(reaction) = weapon.on_attacked_reaction {
                known.push((weapon.name.to_string(), reaction));
            }
        }
        known
    }

    pub fn usable_on_attacked_reactions(&self) -> Vec<(String, OnAttackedReaction)> {
        let mut usable = self.known_on_attacked_reactions();
        usable.retain(|reaction| self.can_use_on_attacked_reaction(reaction.1));
        usable
    }

    pub fn can_use_on_attacked_reaction(&self, reaction: OnAttackedReaction) -> bool {
        self.action_points >= reaction.action_point_cost
            && self.stamina.current >= reaction.stamina_cost
    }

    pub fn known_on_hit_reactions(&self) -> Vec<(String, OnHitReaction)> {
        let mut known = vec![];
        for reaction in &self.known_on_hit_reactions {
            known.push(("".to_string(), *reaction));
        }
        if let Some(shield) = self.off_hand.shield {
            if let Some(reaction) = shield.on_hit_reaction {
                known.push((shield.name.to_string(), reaction));
            }
        }
        known
    }

    pub fn usable_on_hit_reactions(&self) -> Vec<(String, OnHitReaction)> {
        let mut usable = self.known_on_hit_reactions();
        usable.retain(|r| self.can_use_on_hit_reaction(r.1));
        usable
    }

    pub fn can_use_on_hit_reaction(&self, reaction: OnHitReaction) -> bool {
        if let OnHitReactionEffect::Rage = reaction.effect {
            if self.conditions.raging {
                // Can't use this reaction while already raging
                return false;
            }
        }
        self.action_points >= reaction.action_point_cost
    }

    pub fn can_use_spell_enhancement(&self, spell: Spell) -> bool {
        let enhancement = spell.possible_enhancement.unwrap();
        self.mana.current >= spell.mana_cost + enhancement.mana_cost
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

    pub fn defense(&self) -> u32 {
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

    pub fn mental_resistence(&self) -> u32 {
        10 + self.intellect()
    }

    pub fn physical_resistence(&self) -> u32 {
        10 + self.strength()
    }

    pub fn protection_from_armor(&self) -> u32 {
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
    pub range: WeaponRange,
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
    pub on_hit_reaction: Option<OnHitReaction>,
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

#[derive(Debug, Copy, Clone)]
pub enum WeaponRange {
    Melee,
    Ranged(u32),
}
