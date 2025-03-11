use std::cell::RefCell;
use std::cell::{Ref, RefMut};
use std::rc::Rc;

use crate::d20::{probability_of_d20_reaching, roll_d20_with_advantage};
use crate::data::BRACE;
use crate::data::KILL;
use crate::data::WAR_HAMMER;
use crate::data::{BOW, SIDE_STEP};
use crate::data::{
    CRUSHING_STRIKE, FIREBALL, LEATHER_ARMOR, MIND_BLAST, RAGE, SCREAM, SMALL_SHIELD, SWORD,
};

pub const ACTION_POINTS_PER_TURN: u32 = 6;

pub struct CoreGame {
    pub characters: Characters,
    pub active_character_id: CharacterId,
    event_handler: Rc<dyn GameEventHandler>,
}

impl CoreGame {
    pub fn new(event_handler: Rc<dyn GameEventHandler>) -> Self {
        let mut bob = Character::new(true, "Bob", TextureId::Character, 10, 10, 10, (2, 5));
        bob.main_hand.weapon = Some(SWORD);
        bob.off_hand.shield = Some(SMALL_SHIELD);
        bob.armor = Some(LEATHER_ARMOR);
        bob.known_attack_enhancements.push(CRUSHING_STRIKE);
        //bob.known_attacked_reactions.push(SIDE_STEP);
        bob.known_on_hit_reactions.push(RAGE);
        bob.known_actions.push(BaseAction::CastSpell(SCREAM));
        bob.known_actions.push(BaseAction::CastSpell(MIND_BLAST));
        bob.known_actions.push(BaseAction::CastSpell(FIREBALL));
        bob.known_actions.push(BaseAction::CastSpell(KILL));

        let mut alice = Character::new(false, "Gremlin", TextureId::Character2, 5, 5, 5, (2, 4));
        alice.main_hand.weapon = Some(BOW);
        alice.off_hand.shield = Some(SMALL_SHIELD);
        alice.armor = Some(LEATHER_ARMOR);
        alice.known_attacked_reactions.push(SIDE_STEP);

        let mut charlie = Character::new(false, "Gremlin", TextureId::Character2, 1, 2, 1, (3, 4));
        charlie.main_hand.weapon = Some(SWORD);
        charlie.off_hand.shield = Some(SMALL_SHIELD);

        let mut david = Character::new(true, "David", TextureId::Character, 10, 10, 10, (5, 7));
        david.main_hand.weapon = Some(WAR_HAMMER);

        let characters = Characters::new(vec![bob, alice, charlie, david]);

        Self {
            characters,
            active_character_id: 0,
            event_handler,
        }
    }

    pub fn begin(self) -> GameState {
        GameState::AwaitingChooseAction(StateChooseAction { game: self })
    }

    pub fn active_character(&self) -> RefMut<Character> {
        self.characters.get_mut(self.active_character_id)
    }

    pub fn is_players_turn(&self) -> bool {
        self.active_character().player_controlled
    }

    fn enter_state_action(self, action: Action) -> GameState {
        let mut character = self.active_character();
        let action_points_before_action = character.action_points;

        match action {
            Action::Attack {
                hand,
                enhancements,
                target,
            } => {
                let mut attacker = character;
                let defender = self.characters.get_mut(target);

                assert!(attacker.can_reach_with_attack(hand, defender.position));

                attacker.action_points -= attacker.weapon(hand).unwrap().action_point_cost;
                for enhancement in &enhancements {
                    attacker.action_points -= enhancement.action_point_cost;
                    attacker.stamina.spend(enhancement.stamina_cost);
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

                let is_within_melee = within_meele(attacker.position, defender.position);

                drop(attacker);
                drop(defender);
                return if is_within_melee {
                    let attacking_character_i = self.active_character_id;
                    transition_to(GameState::AwaitingChooseReaction(
                        StateChooseReaction::Attack(StateChooseAttackReaction {
                            game: self,
                            reactor: target,
                            attacker: attacking_character_i,
                            action_points_before_action,
                            enhancements,
                            hand,
                        }),
                    ))
                } else {
                    self.enter_state_attack(
                        action_points_before_action,
                        hand,
                        enhancements,
                        target,
                        None,
                    )
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

                if let ApplyEffect::Condition(condition) = effect {
                    self.event_handler
                        .handle(GameEvent::CharacterReceivedSelfEffect {
                            character: self.active_character_id,
                            condition,
                        });
                }

                self.perform_effect_application(effect, &mut character, "");
            }
            Action::CastSpell {
                spell,
                enhanced,
                target,
            } => {
                self.perform_spell(&mut character, spell, enhanced, target);
            }
            Action::Move {
                action_point_cost,
                positions,
                enhancements,
            } => {
                character.action_points -= action_point_cost;
                for enhancement in &enhancements {
                    character.action_points -= enhancement.action_point_cost;
                    character.stamina.spend(enhancement.stamina_cost);
                }

                drop(character);
                return self.perform_movement(positions, action_points_before_action);
            }
        }

        drop(character);
        self.enter_state_right_after_action(action_points_before_action, None)
    }

    fn perform_movement(
        self,
        mut positions: Vec<(u32, u32)>,
        action_points_before_action: u32,
    ) -> GameState {
        let mut character = self.active_character();
        let new_position = positions.remove(0);
        for character in self.characters.iter() {
            if let Ok(ch) = character.try_borrow() {
                assert!(new_position != ch.position)
            }
        }
        self.log(format!(
            "{} moved from {:?} to {:?}",
            character.name, character.position, new_position
        ));
        character.position = new_position;
        drop(character);

        if !positions.is_empty() {
            transition_to(GameState::PerformingMovement(StatePerformingMovement {
                game: self,
                remaining_positions: positions,
                action_points_before_action,
            }))
        } else {
            self.enter_state_right_after_action(action_points_before_action, None)
        }
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
        target: CharacterId,
    ) {
        let defender = &mut self.characters.get_mut(target);

        assert!(caster.can_reach_with_spell(spell, defender.position));
        assert!(caster.action_points >= spell.action_point_cost);

        caster.action_points -= spell.action_point_cost;
        caster.mana.spend(spell.mana_cost);

        let mut enhancement_str = String::new();
        if enhanced {
            let enhancement = spell.possible_enhancement.unwrap();
            caster.mana.spend(enhancement.mana_cost);

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

            let success = res >= target;

            self.event_handler.handle(GameEvent::SpellWasCast {
                caster: caster.id(),
                target: defender.id(),
                success,
                spell_type: spell.spell_type,
            });

            if success {
                self.log("  The spell was successful!");
                let damage = spell.damage;
                if damage > 0 {
                    self.perform_losing_health(defender, damage);
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

    fn perform_losing_health(&self, character: &mut Character, amount: u32) {
        character.health.lose(amount);
        self.log(format!("  {} took {} damage", character.name, amount));
        self.event_handler.handle(GameEvent::CharacterTookDamage {
            character: character.id.unwrap(),
            amount,
        });

        if character.health.current == 0 {
            character.has_died = true;
        }
    }

    fn log(&self, line: impl Into<String>) {
        self.event_handler.handle(GameEvent::LogLine(line.into()));
    }

    fn enter_state_attack(
        self,
        action_points_before_action: u32,
        hand_type: HandType,
        attack_enhancements: Vec<AttackEnhancement>,
        defender_id: CharacterId,
        defender_reaction: Option<OnAttackedReaction>,
    ) -> GameState {
        let mut attacker = self.active_character();
        let mut defender = self.characters.get_mut(defender_id);

        let advantage = attacker.attack_advantage(hand_type) + defender.incoming_attack_advantage();

        let mut defense = defender.defense();

        let mut defender_reacted_with_parry = false;
        let mut defender_reacted_with_sidestep = false;
        let mut skip_attack_exertion = false;

        let mut attack_hit = None;

        let attack_modifier = attacker.attack_modifier(hand_type);

        if let Some(reaction) = defender_reaction {
            defender.action_points -= reaction.action_point_cost;
            defender.stamina.spend(reaction.stamina_cost);

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
            std::cmp::Ordering::Equal => self.log("Rolling 1 die..."),
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

        let hit = res >= defense;

        self.event_handler.handle(GameEvent::Attacked {
            attacker: attacker.id(),
            target: defender_id,
            hit,
        });

        if !hit {
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

            self.perform_losing_health(&mut defender, damage);

            attack_hit = Some((defender_id, damage));

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

        self.enter_state_right_after_action(action_points_before_action, attack_hit)
    }

    fn enter_state_right_after_action(
        self,
        action_points_before_action: u32,
        attack_hit: Option<(CharacterId, u32)>,
    ) -> GameState {
        let players_turn = self.active_character().player_controlled;
        let mut character = self.active_character();

        // You recover from 1 stack of Dazed for each AP you spend
        // This must happen before "on attacked and hit" reactions because those might
        // inflict new Dazed stacks, which should not be covered here.
        let spent = action_points_before_action - character.action_points;
        self.perform_recover_from_dazed(&mut character, spent);

        if let Some((attacked_id, damage)) = attack_hit {
            // TODO this can remove a Dazed that was just added from the attack, which is bad.

            // You recover from 1 stack of Dazed each time you're hit by an attack
            self.perform_recover_from_dazed(&mut self.characters.get_mut(attacked_id), 1);

            drop(character);
            let attacking_id = self.active_character_id;
            return transition_to(GameState::AwaitingChooseReaction(StateChooseReaction::Hit(
                StateChooseHitReaction {
                    game: self,
                    reactor: attacked_id,
                    attacker: attacking_id,
                    damage,
                },
            )));
        }

        drop(character);
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

    fn enter_state_react_after_being_hit(
        self,
        reacting_id: CharacterId,
        reaction: Option<OnHitReaction>,
    ) -> GameState {
        if let Some(reaction) = reaction {
            self.perform_on_hit_reaction(
                &mut self.active_character(),
                &mut self.characters.get_mut(reacting_id),
                reaction,
            );
        }

        self.enter_state_longer_after_action()
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
        {
            let mut character = self.characters.get_mut(self.active_character_id);
            if character.action_points == 0 {
                self.perform_end_of_turn_character(&mut character);
                self.active_character_id = self.characters.next_id(self.active_character_id);
            }
        }

        transition_to(GameState::AwaitingChooseAction(StateChooseAction {
            game: self,
        }))
    }

    fn perform_end_of_turn_character(&self, character: &mut Character) {
        if character.conditions.bleeding > 0 {
            // TODO
            self.perform_losing_health(character, 99);
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

        // TODO also give AP at start of turn
        character.action_points = (character.action_points + 3).min(ACTION_POINTS_PER_TURN);
        character.main_hand.exertion = 0;
        character.off_hand.exertion = 0;
        character.stamina.gain(1);

        self.log("End of turn.");
    }

    fn remove_dead(&mut self) {
        for id in self.characters.remove_dead() {
            self.event_handler
                .handle(GameEvent::CharacterDied { character: id });
        }
    }
}

fn transition_to(mut game_state: GameState) -> GameState {
    game_state.game_mut().remove_dead();
    game_state
}

pub enum GameState {
    AwaitingChooseAction(StateChooseAction),
    AwaitingChooseReaction(StateChooseReaction),
    PerformingMovement(StatePerformingMovement),
}

impl GameState {
    pub fn game(&self) -> &CoreGame {
        match self {
            GameState::AwaitingChooseAction(this) => &this.game,
            GameState::AwaitingChooseReaction(StateChooseReaction::Attack(this)) => &this.game,
            GameState::AwaitingChooseReaction(StateChooseReaction::Hit(this)) => &this.game,
            GameState::PerformingMovement(this) => &this.game,
        }
    }

    pub fn game_mut(&mut self) -> &mut CoreGame {
        match self {
            GameState::AwaitingChooseAction(this) => &mut this.game,
            GameState::AwaitingChooseReaction(StateChooseReaction::Attack(this)) => &mut this.game,
            GameState::AwaitingChooseReaction(StateChooseReaction::Hit(this)) => &mut this.game,
            GameState::PerformingMovement(this) => &mut this.game,
        }
    }

    pub fn unwrap_choose_action(self) -> StateChooseAction {
        match self {
            GameState::AwaitingChooseAction(inner) => inner,
            _ => panic!(),
        }
    }

    pub fn unwrap_react_to_attack(self) -> StateChooseAttackReaction {
        match self {
            GameState::AwaitingChooseReaction(StateChooseReaction::Attack(inner)) => inner,
            _ => panic!(),
        }
    }

    pub fn unwrap_react_to_hit(self) -> StateChooseHitReaction {
        match self {
            GameState::AwaitingChooseReaction(StateChooseReaction::Hit(inner)) => inner,
            _ => panic!(),
        }
    }
}

pub trait GameEventHandler {
    fn handle(&self, event: GameEvent);
}

#[derive(Debug)]
pub enum GameEvent {
    LogLine(String),
    CharacterTookDamage {
        character: CharacterId,
        amount: u32,
    },
    Attacked {
        attacker: CharacterId,
        target: CharacterId,
        hit: bool,
    },
    SpellWasCast {
        caster: CharacterId,
        target: CharacterId,
        success: bool,
        spell_type: SpellType,
    },
    CharacterReceivedSelfEffect {
        character: CharacterId,
        condition: Condition,
    },
    CharacterDied {
        character: CharacterId,
    },
}

pub struct StateChooseAction {
    pub game: CoreGame,
}

impl StateChooseAction {
    pub fn proceed(self, action: Action) -> GameState {
        self.game.enter_state_action(action)
    }
}
pub struct StatePerformingMovement {
    game: CoreGame,
    remaining_positions: Vec<(u32, u32)>,
    action_points_before_action: u32,
}

impl StatePerformingMovement {
    pub fn proceed(self) -> GameState {
        self.game
            .perform_movement(self.remaining_positions, self.action_points_before_action)
    }
}

pub enum StateChooseReaction {
    Attack(StateChooseAttackReaction),
    Hit(StateChooseHitReaction),
}

pub struct StateChooseAttackReaction {
    pub game: CoreGame,
    pub reactor: CharacterId,
    pub attacker: CharacterId,
    action_points_before_action: u32,
    pub enhancements: Vec<AttackEnhancement>,
    pub hand: HandType,
}

impl StateChooseAttackReaction {
    pub fn proceed(self, reaction: Option<OnAttackedReaction>) -> GameState {
        self.game.enter_state_attack(
            self.action_points_before_action,
            self.hand,
            self.enhancements,
            self.reactor,
            reaction,
        )
    }
}

pub struct StateChooseHitReaction {
    pub game: CoreGame,
    pub reactor: CharacterId,
    pub attacker: CharacterId,
    pub damage: u32,
}

impl StateChooseHitReaction {
    pub fn proceed(self, reaction: Option<OnHitReaction>) -> GameState {
        self.game
            .enter_state_react_after_being_hit(self.reactor, reaction)
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

pub struct Characters(Vec<(CharacterId, Rc<RefCell<Character>>)>);

impl Characters {
    fn new(characters: Vec<Character>) -> Self {
        Self(
            characters
                .into_iter()
                .enumerate()
                .map(|(i, mut ch)| {
                    let id = i as CharacterId;
                    ch.id = Some(id);
                    (id, Rc::new(RefCell::new(ch)))
                })
                .collect(),
        )
    }

    fn next_id(&self, current_id: CharacterId) -> CharacterId {
        let mut i = 0;
        let mut passed_current = false;
        loop {
            let (id, ch) = &self.0[i];

            if passed_current && !ch.borrow().has_died {
                return *id;
            }

            if *id == current_id {
                if passed_current {
                    panic!("No alive character found");
                }
                passed_current = true;
            }

            i = (i + 1) % self.0.len();
        }
    }

    pub fn get_mut(&self, character_id: CharacterId) -> RefMut<Character> {
        self.0
            .iter()
            .find(|(id, _ch)| *id == character_id)
            .unwrap()
            .1
            .borrow_mut()
    }

    pub fn get(&self, character_id: CharacterId) -> Ref<Character> {
        self.0
            .iter()
            .find(|(id, _ch)| *id == character_id)
            .unwrap()
            .1
            .borrow()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Rc<RefCell<Character>>> {
        self.0.iter().map(|(_id, ch)| ch)
    }

    pub fn iter_with_ids(&self) -> impl Iterator<Item = &(CharacterId, Rc<RefCell<Character>>)> {
        self.0.iter()
    }

    pub fn remove_dead(&mut self) -> Vec<CharacterId> {
        let mut removed = vec![];
        self.0.retain(|(_id, ch)| {
            if ch.borrow().has_died {
                removed.push(ch.borrow().id());
                false
            } else {
                true
            }
        });
        removed
    }
}

impl Clone for Characters {
    fn clone(&self) -> Self {
        Characters(self.0.clone())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct AttackEnhancement {
    pub name: &'static str,
    pub description: &'static str,
    pub action_point_cost: u32,
    pub stamina_cost: u32,
    pub bonus_damage: u32,
    pub apply_on_self_before: Option<Condition>,
    pub on_hit_effect: Option<ApplyEffect>,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum ApplyEffect {
    RemoveActionPoints(u32),
    Condition(Condition),
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct OnAttackedReaction {
    pub name: &'static str,
    pub description: &'static str,
    pub action_point_cost: u32,
    pub stamina_cost: u32,
    pub effect: OnAttackedReactionEffect,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum OnAttackedReactionEffect {
    Parry,
    SideStep,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct OnHitReaction {
    pub name: &'static str,
    pub description: &'static str,
    pub action_point_cost: u32,
    pub effect: OnHitReactionEffect,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum OnHitReactionEffect {
    Rage,
    ShieldBash,
}

#[derive(Debug, Copy, Clone)]
pub enum AttackHitEffect {
    Apply(ApplyEffect),
    SkipExertion,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
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
        target: CharacterId,
    },
    SelfEffect(SelfEffectAction),
    CastSpell {
        spell: Spell,
        enhanced: bool,
        target: CharacterId,
    },
    Move {
        positions: Vec<(u32, u32)>,
        enhancements: Vec<MovementEnhancement>,
        action_point_cost: u32,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct SelfEffectAction {
    pub name: &'static str,
    pub description: &'static str,
    pub action_point_cost: u32,
    pub effect: ApplyEffect,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum BaseAction {
    Attack {
        hand: HandType,
        action_point_cost: u32,
    },
    SelfEffect(SelfEffectAction),
    CastSpell(Spell),
    Move {
        range: f32,
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
            BaseAction::SelfEffect(sea) => sea.action_point_cost,
            BaseAction::CastSpell(spell) => spell.action_point_cost,
            BaseAction::Move {
                action_point_cost, ..
            } => *action_point_cost,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
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
    pub range: Range,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct SpellEnhancement {
    pub name: &'static str,
    pub mana_cost: u32,
    pub effect: SpellEnhancementEffect,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum SpellEnhancementEffect {
    CastTwice,
    OnHitEffect(ApplyEffect),
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum SpellType {
    Mental,
    Projectile,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct MovementEnhancement {
    pub name: &'static str,
    pub action_point_cost: u32,
    pub stamina_cost: u32,
    pub add_percentage: u32,
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

pub type CharacterId = u32;

#[derive(Debug)]
pub struct Character {
    id: Option<CharacterId>,
    pub texture: TextureId,
    pub has_died: bool,
    pub player_controlled: bool,
    // TODO i32 instead?
    pub position: (u32, u32),
    pub name: &'static str,
    pub base_strength: u32,
    pub base_dexterity: u32,
    pub base_intellect: u32,
    pub health: NumberedResource,
    pub mana: NumberedResource,
    pub move_range: f32,
    pub armor: Option<ArmorPiece>,
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

const MOVE_ACTION_COST: u32 = 1;

impl Character {
    fn new(
        player_controlled: bool,
        name: &'static str,
        texture: TextureId,
        str: u32,
        dex: u32,
        int: u32,
        position: (u32, u32),
    ) -> Self {
        let mana = if int < 3 { 0 } else { 1 + 2 * (int - 3) };
        let move_range = 0.8 + dex as f32 * 0.2;
        Self {
            id: None,
            texture,
            has_died: false,
            player_controlled,
            position,
            name,
            base_strength: str,
            base_dexterity: dex,
            base_intellect: int,
            health: NumberedResource::new(5 + str),
            mana: NumberedResource::new(mana),
            move_range,
            armor: None,
            main_hand: Default::default(),
            off_hand: Default::default(),
            conditions: Default::default(),
            action_points: ACTION_POINTS_PER_TURN,
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
                BaseAction::SelfEffect(BRACE),
                BaseAction::Move {
                    action_point_cost: MOVE_ACTION_COST,
                    range: move_range,
                },
            ],
            known_attacked_reactions: Default::default(),
            known_on_hit_reactions: Default::default(),
        }
    }

    pub fn position_i32(&self) -> (i32, i32) {
        (self.position.0 as i32, self.position.1 as i32)
    }

    pub fn id(&self) -> CharacterId {
        self.id.unwrap()
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

    pub fn shield(&self) -> Option<Shield> {
        self.hand(HandType::OffHand).shield
    }

    pub fn can_reach_with_attack(&self, hand: HandType, target_position: (u32, u32)) -> bool {
        let weapon = self.weapon(hand).unwrap();
        within_range(weapon.range.squared(), self.position, target_position)
    }

    pub fn can_reach_with_spell(&self, spell: Spell, target_position: (u32, u32)) -> bool {
        within_range(spell.range.squared(), self.position, target_position)
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
            BaseAction::Move {
                action_point_cost, ..
            } => ap >= action_point_cost,
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

    pub fn usable_movement_enhancements(&self) -> Vec<(String, MovementEnhancement)> {
        let mut enhancements = vec![
            (
                "2x".to_string(),
                MovementEnhancement {
                    name: "Extend",
                    action_point_cost: 1,
                    stamina_cost: 0,
                    add_percentage: 100,
                },
            ),
            (
                "2.5x".to_string(),
                MovementEnhancement {
                    name: "Sprint",
                    action_point_cost: 1,
                    stamina_cost: 1,
                    add_percentage: 150,
                },
            ),
        ];
        enhancements.retain(|(_, enhancement)| {
            self.action_points >= MOVE_ACTION_COST + enhancement.action_point_cost
                && self.stamina.current >= enhancement.stamina_cost
        });
        enhancements
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

    pub fn attack_modifier(&self, hand: HandType) -> u32 {
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

    pub fn explain_attack_circumstances(&self, hand_type: HandType) -> String {
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

    pub fn explain_incoming_attack_circumstances(&self) -> String {
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

fn within_range(range_squared: f32, source: (u32, u32), destination: (u32, u32)) -> bool {
    let distance_squared = (destination.0 as i32 - source.0 as i32).pow(2)
        + (destination.1 as i32 - source.1 as i32).pow(2);
    distance_squared as f32 <= range_squared
}

fn within_meele(source: (u32, u32), destination: (u32, u32)) -> bool {
    within_range(2.0, source, destination)
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

    fn spend(&mut self, amount: u32) {
        assert!(self.current >= amount);
        self.current -= amount;
    }

    fn gain(&mut self, amount: u32) {
        self.current = (self.current + amount).min(self.max);
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ArmorPiece {
    pub name: &'static str,
    pub protection: u32,
    pub limit_defense_from_dex: Option<u32>,
}

#[derive(Debug, Copy, Clone)]
pub struct Weapon {
    pub name: &'static str,
    pub texture_id: Option<TextureId>,
    pub range: Range,
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
    pub texture_id: Option<TextureId>,
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

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Range {
    Melee,
    Ranged(u32),
    Float(f32),
}

impl Range {
    pub fn squared(&self) -> f32 {
        match self {
            Range::Melee => 2.0,
            Range::Ranged(r) => r.pow(2) as f32,
            Range::Float(f) => f.powf(2.0),
        }
    }
}

impl From<Range> for f32 {
    fn from(range: Range) -> Self {
        match range {
            Range::Melee => 2f32.sqrt(),
            Range::Ranged(r) => r as f32,
            Range::Float(f) => f,
        }
    }
}

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug)]
pub enum TextureId {
    Character,
    Character2,
    Warhammer,
    Bow,
    Sword,
    Shield,
}
