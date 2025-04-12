use std::cell::RefCell;
use std::cell::{Ref, RefMut};

use std::fmt::{Display, Write};
use std::rc::Rc;

use crate::d20::{probability_of_d20_reaching, roll_d20_with_advantage, DiceRollBonus};

use crate::data::{
    SpellId, BOW, BRACED_DEFENSE_BONUS, BRACED_DESCRIPTION, KILL, PARRY_EVASION_BONUS,
    RAGING_DESCRIPTION,
};
use crate::data::{CHAIN_MAIL, DAGGER, SIDE_STEP, SWORD};
use crate::data::{CRUSHING_STRIKE, FIREBALL, LEATHER_ARMOR, MIND_BLAST, SCREAM, SMALL_SHIELD};

use crate::textures::{EquipmentIconId, IconId, SpriteId};

pub const MAX_ACTION_POINTS: u32 = 5;
//pub const ACTION_POINTS_RECOVERY: u32 = 3;

pub struct CoreGame {
    pub characters: Characters,
    pub active_character_id: CharacterId,
    event_handler: Rc<dyn GameEventHandler>,
}

impl CoreGame {
    pub fn new(event_handler: Rc<dyn GameEventHandler>) -> Self {
        let mut bob = Character::new(
            true,
            "Bob",
            SpriteId::Character,
            Attributes::new(5, 4, 2, 5),
            (1, 3),
        );
        bob.main_hand.weapon = Some(SWORD);
        bob.off_hand.shield = Some(SMALL_SHIELD);
        bob.armor = Some(CHAIN_MAIL);
        bob.known_attack_enhancements.push(CRUSHING_STRIKE);
        bob.known_attacked_reactions.push(SIDE_STEP);
        //bob.known_on_hit_reactions.push(RAGE);
        bob.known_actions.push(BaseAction::CastSpell(SCREAM));
        bob.known_actions.push(BaseAction::CastSpell(MIND_BLAST));
        bob.known_actions.push(BaseAction::CastSpell(FIREBALL));
        bob.known_actions.push(BaseAction::CastSpell(KILL));

        let mut enemy1 = Character::new(
            false,
            "Gremlin",
            SpriteId::Character2,
            Attributes::new(5, 5, 5, 5),
            (2, 4),
        );
        enemy1.main_hand.weapon = Some(BOW);
        enemy1.off_hand.shield = Some(SMALL_SHIELD);
        enemy1.armor = Some(LEATHER_ARMOR);
        enemy1.known_attacked_reactions.push(SIDE_STEP);
        enemy1.receive_condition(Condition::Bleeding);
        enemy1.receive_condition(Condition::Dazed(5));

        let mut enemy2 = Character::new(
            false,
            "Gromp",
            SpriteId::Character3,
            Attributes::new(1, 5, 1, 5),
            (2, 3),
        );
        enemy2.main_hand.weapon = Some(BOW);

        let mut david = Character::new(
            true,
            "David",
            SpriteId::Character,
            Attributes::new(10, 10, 10, 5),
            (5, 7),
        );
        david.main_hand.weapon = Some(DAGGER);

        let characters = Characters::new(vec![bob, enemy1, enemy2, david]);

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

                let is_within_melee = within_meele(attacker.position, defender.position);
                let defender_can_react_to_attack = !defender
                    .usable_on_attacked_reactions(is_within_melee)
                    .is_empty();

                drop(attacker);
                drop(defender);
                return if defender_can_react_to_attack {
                    let attacking_character_i = self.active_character_id;

                    transition_to(GameState::AwaitingChooseReaction(
                        StateChooseReaction::Attack(StateChooseAttackReaction {
                            game: self,
                            reactor: target,
                            attacker: attacking_character_i,
                            action_points_before_action,
                            enhancements,
                            hand,
                            is_within_melee,
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
                stamina_cost,
                effect,
                ..
            }) => {
                character.action_points -= action_point_cost;
                character.stamina.spend(stamina_cost);

                self.log(format!("{} uses {}", character.name, name));

                if let ApplyEffect::Condition(condition) = effect {
                    self.event_handler
                        .handle(GameEvent::CharacterReceivedSelfEffect {
                            character: self.active_character_id,
                            condition,
                        });
                }

                let log_line = self.perform_effect_application(effect, &mut character);
                self.log(log_line);
            }
            Action::CastSpell {
                spell,
                enhancements,
                target,
            } => {
                self.perform_spell(&mut character, spell, enhancements, target);
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

        self.event_handler.handle(GameEvent::Moved {
            character: character.id(),
            from: character.position,
            to: new_position,
        });

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

    fn perform_effect_application(&self, effect: ApplyEffect, receiver: &mut Character) -> String {
        match effect {
            ApplyEffect::RemoveActionPoints(n) => {
                receiver.lose_action_points(n);
                format!("  {} lost {} AP", receiver.name, n)
            }
            ApplyEffect::Condition(condition) => {
                receiver.receive_condition(condition);
                format!("  {} received {:?}", receiver.name, condition)
            }
        }
    }

    fn perform_spell(
        &self,
        caster: &mut Character,
        spell: Spell,
        enhancements: Vec<SpellEnhancement>,
        target: CharacterId,
    ) {
        let defender = &mut self.characters.get_mut(target);

        assert!(caster.can_reach_with_spell(spell, defender.position));
        assert!(caster.action_points >= spell.action_point_cost);

        caster.action_points -= spell.action_point_cost;
        caster.mana.spend(spell.mana_cost);

        for enhancement in &enhancements {
            caster.mana.spend(enhancement.mana_cost);
        }

        let (defense_label, defense) = match spell.spell_type {
            SpellType::Mental => ("will", defender.will()),
            SpellType::Projectile => ("evasion", defender.evasion()),
        };

        let mut cast_n_times = 1;
        for enhancement in &enhancements {
            if enhancement.effect == Some(SpellEnhancementEffect::CastTwice) {
                cast_n_times = 2;
            }
        }

        for i in 0..cast_n_times {
            let mut detail_lines = vec![];
            let roll = roll_d20_with_advantage(0);
            let spell_result = roll + caster.spell_modifier();
            detail_lines.push(format!(
                "Rolled: {} (+{} spell mod) = {}, vs {}={}",
                roll,
                caster.spell_modifier(),
                spell_result,
                defense_label,
                defense,
            ));

            let outcome = if spell_result >= defense {
                detail_lines.push("The spell was successful!".to_string());
                let mut dmg_calculation = spell.damage as i32;
                let mut dmg_str = format!("  Damage: {} ({})", dmg_calculation, spell.name);

                let (label, bonus_dmg) = if spell_result < defense + 5 {
                    // No such thing as "True hit" for spells
                    ("", 0)
                } else if spell_result < defense + 10 {
                    ("Heavy hit", 1)
                } else {
                    ("Critical hit", 2)
                };
                if !label.is_empty() {
                    detail_lines.push(format!("  {label}!"));
                }
                if bonus_dmg > 0 {
                    dmg_str.push_str(&format!(" +{bonus_dmg} ({label})"));
                    dmg_calculation += bonus_dmg;
                }

                for enhancement in &enhancements {
                    if enhancement.bonus_damage > 0 {
                        dmg_str.push_str(&format!(
                            " +{} ({})",
                            enhancement.bonus_damage, enhancement.name
                        ));
                        dmg_calculation += enhancement.bonus_damage as i32;
                    }
                }

                let damage = dmg_calculation.max(0) as u32;

                if dmg_calculation > 0 {
                    self.perform_losing_health(defender, damage);
                    dmg_str.push_str(&format!(" = {damage}"));
                    detail_lines.push(dmg_str);
                }

                if let Some(effect) = spell.on_hit_effect {
                    let log_line = self.perform_effect_application(effect, defender);
                    detail_lines.push(format!("{} ({})", log_line, spell.name));
                }

                for enhancement in &enhancements {
                    if let Some(SpellEnhancementEffect::OnHitEffect(effect)) = enhancement.effect {
                        let log_line = self.perform_effect_application(effect, defender);
                        detail_lines.push(format!("{} ({})", log_line, enhancement.name));
                    }
                }

                SpellOutcome::Hit(damage)
            } else {
                match spell.spell_type {
                    SpellType::Mental => {
                        detail_lines.push(format!("{} resisted the spell!", defender.name))
                    }
                    SpellType::Projectile => detail_lines.push("The spell missed!".to_string()),
                }
                SpellOutcome::Resist
            };

            if i < cast_n_times - 1 {
                detail_lines.push(format!("{} cast again!", caster.name))
            }

            self.event_handler.handle(GameEvent::SpellWasCast {
                caster: caster.id(),
                target: defender.id(),
                outcome,
                spell: spell,
                detail_lines,
            });
        }
    }

    fn perform_losing_health(&self, character: &mut Character, amount: u32) {
        character.health.lose(amount);
        //self.log(format!("  {} took {} damage", character.name, amount));
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

        let attack_bonus = attacker.attack_bonus(hand_type);

        let advantage = attack_bonus.advantage + defender.incoming_attack_advantage();

        let mut evasion = defender.evasion();

        let mut defender_reacted_with_parry = false;
        let mut defender_reacted_with_sidestep = false;
        let mut skip_attack_exertion = false;

        let mut attack_hit = None;

        let attack_modifier = attacker.attack_modifier(hand_type);

        let mut log_lines = vec![];

        if let Some(reaction) = defender_reaction {
            defender.action_points -= reaction.action_point_cost;
            defender.stamina.spend(reaction.stamina_cost);

            log_lines.push(format!("{} reacted with {}", defender.name, reaction.name));

            match reaction.effect {
                OnAttackedReactionEffect::Parry => {
                    defender_reacted_with_parry = true;
                    let bonus_evasion = PARRY_EVASION_BONUS;

                    log_lines.push(format!(
                        "  Evasion: {} +{} (Parry) = {}",
                        evasion,
                        bonus_evasion,
                        evasion + bonus_evasion
                    ));
                    evasion += bonus_evasion;
                    let p_hit =
                        probability_of_d20_reaching(evasion - attack_modifier, attack_bonus);
                    log_lines.push(format!("  Chance to hit: {:.1}%", p_hit * 100f32));
                }
                OnAttackedReactionEffect::SideStep => {
                    defender_reacted_with_sidestep = true;
                    let bonus_evasion = defender.evasion_from_agility();
                    log_lines.push(format!(
                        "  Evasion: {} +{} (Side step) = {}",
                        evasion,
                        bonus_evasion,
                        evasion + bonus_evasion
                    ));
                    evasion += bonus_evasion;

                    let p_hit = probability_of_d20_reaching(
                        evasion.saturating_sub(attack_modifier),
                        attack_bonus,
                    );
                    log_lines.push(format!("  Chance to hit: {:.1}%", p_hit * 100f32));
                }
            }
        }

        let roll = roll_d20_with_advantage(attack_bonus.advantage);
        let attack_result = ((roll + attack_modifier) as i32 + attack_bonus.flat_amount) as u32;
        match advantage.cmp(&0) {
            std::cmp::Ordering::Less => log_lines.push(format!(
                "Rolled {} dice with disadvantage...",
                advantage.abs() + 1
            )),
            std::cmp::Ordering::Equal => log_lines.push("Rolled 1 die...".to_string()),
            std::cmp::Ordering::Greater => {
                log_lines.push(format!("Rolled {} dice with advantage...", advantage + 1))
            }
        }
        log_lines.push(format!(
            "Rolled: {} (+{} atk mod) {}= {}, vs evasion={}, armor={}",
            roll,
            attack_modifier,
            if attack_bonus.flat_amount > 0 {
                format!("(+{}) ", attack_bonus.flat_amount)
            } else if attack_bonus.flat_amount < 0 {
                format!("(-{}) ", -attack_bonus.flat_amount)
            } else {
                "".to_string()
            },
            attack_result,
            evasion,
            defender.protection_from_armor()
        ));

        let hit = attack_result >= evasion;

        let outcome = if hit {
            let mut on_true_hit_effect = None;
            let weapon = attacker.weapon(hand_type).unwrap();
            let mut dmg_calculation = weapon.damage as i32;

            let mut dmg_str = format!("  Damage: {} ({})", dmg_calculation, weapon.name);

            if matches!(weapon.grip, WeaponGrip::Versatile) && attacker.off_hand.is_empty() {
                let bonus_dmg = 1;
                dmg_str.push_str(&format!(" +{} (two-handed)", bonus_dmg));
                dmg_calculation += bonus_dmg;
            }

            let armored_defense = evasion + defender.protection_from_armor();
            if attack_result < armored_defense {
                let mitigated = armored_defense - attack_result;

                log_lines.push(format!("  Hit! ({} mitigated)", mitigated));
                dmg_str.push_str(&format!(" -{mitigated} (armor)"));
                dmg_calculation -= mitigated as i32;
            } else {
                on_true_hit_effect = weapon.on_true_hit;
                let (label, bonus_dmg) = if attack_result < armored_defense + 5 {
                    ("True hit", 0)
                } else if attack_result < armored_defense + 10 {
                    ("Heavy hit", 1)
                } else {
                    ("Critical hit", 2)
                };
                log_lines.push(format!("  {label}!"));
                if bonus_dmg > 0 {
                    dmg_str.push_str(&format!(" +{bonus_dmg} ({label})"));
                    dmg_calculation += bonus_dmg;
                }
            }

            for enhancement in &attack_enhancements {
                if enhancement.bonus_damage > 0 {
                    dmg_str.push_str(&format!(
                        " +{} ({})",
                        enhancement.bonus_damage, enhancement.name
                    ));
                    dmg_calculation += enhancement.bonus_damage as i32;
                }
            }

            let damage = dmg_calculation.max(0) as u32;

            dmg_str.push_str(&format!(" = {damage}"));

            log_lines.push(dmg_str);

            self.perform_losing_health(&mut defender, damage);

            attack_hit = Some((defender_id, damage));

            if let Some(effect) = on_true_hit_effect {
                match effect {
                    AttackHitEffect::Apply(effect) => {
                        let log_line = self.perform_effect_application(effect, &mut defender);
                        log_lines.push(format!("{} (true hit)", log_line))
                    }
                    AttackHitEffect::SkipExertion => skip_attack_exertion = true,
                }
            }

            for enhancement in &attack_enhancements {
                if let Some(effect) = enhancement.on_hit_effect {
                    let log_line = self.perform_effect_application(effect, &mut defender);
                    log_lines.push(format!("{} ({})", log_line, enhancement.name))
                }
            }

            AttackOutcome::Hit(damage)
        } else if defender_reacted_with_parry {
            log_lines.push("  Parried!".to_string());
            AttackOutcome::Parry
        } else if defender_reacted_with_sidestep {
            log_lines.push("  Side stepped!".to_string());
            AttackOutcome::Dodge
        } else {
            log_lines.push("  Missed!".to_string());
            AttackOutcome::Miss
        };

        if skip_attack_exertion {
            log_lines.push("  The attack did not lead to exertion (true hit)".to_string());
        } else {
            let exertion = match hand_type {
                HandType::MainHand => {
                    attacker.receive_condition(Condition::MainHandExertion(1));
                    attacker.conditions.mainhand_exertion
                }
                HandType::OffHand => {
                    attacker.receive_condition(Condition::OffHandExertion(1));
                    attacker.conditions.offhand_exertion
                }
            };
            log_lines.push(format!("  The attack led to exertion ({})", exertion));
        }

        if attacker.conditions.careful_aim {
            attacker.conditions.careful_aim = false;
            log_lines.push(format!("{} lost Careful aim", attacker.name));
        }

        if defender.conditions.braced {
            defender.conditions.braced = false;
            log_lines.push(format!("{} lost Braced", defender.name));
        }

        self.event_handler.handle(GameEvent::Attacked {
            attacker: attacker.id(),
            target: defender_id,
            outcome,
            detail_lines: log_lines,
        });

        drop(attacker);
        drop(defender);

        self.enter_state_right_after_action(action_points_before_action, attack_hit)
    }

    fn enter_state_right_after_action(
        self,
        action_points_before_action: u32,
        attack_hit: Option<(CharacterId, u32)>,
    ) -> GameState {
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

            let attacking_id = self.active_character_id;

            let mut is_within_melee = false;
            let can_react = {
                let victim = self.characters.get(attacked_id);

                is_within_melee = within_meele(character.position, victim.position);
                !victim.has_died && !victim.usable_on_hit_reactions(is_within_melee).is_empty()
            };

            drop(character);

            if can_react {
                return transition_to(GameState::AwaitingChooseReaction(StateChooseReaction::Hit(
                    StateChooseHitReaction {
                        game: self,
                        reactor: attacked_id,
                        attacker: attacking_id,
                        damage,
                        is_within_melee,
                    },
                )));
            }
        } else {
            drop(character);
        }

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
        reactor: &mut Character,
        reaction: OnHitReaction,
    ) {
        reactor.action_points -= reaction.action_point_cost;
        match reaction.effect {
            OnHitReactionEffect::Rage => {
                let condition = Condition::Raging;
                self.event_handler.handle(GameEvent::CharacterReactedToHit {
                    main_line: format!("{} reacted with Rage", reactor.name),
                    detail_lines: vec![],
                    reactor: reactor.id(),
                    outcome: HitReactionOutcome {
                        received_condition: Some(condition),
                        offensive: None,
                    },
                });

                reactor.receive_condition(condition);
            }
            OnHitReactionEffect::ShieldBash => {
                let mut lines = vec![];

                let target = character.toughness();
                let roll = roll_d20_with_advantage(0);
                let res = roll + reactor.strength();
                lines.push(format!(
                    "Rolled: {} (+{} str) = {}, vs toughness={}",
                    roll,
                    reactor.strength(),
                    res,
                    target,
                ));
                let condition = if res >= target {
                    let stacks = if res < target + 5 {
                        lines.push("Hit!".to_string());
                        1
                    } else if res < target + 10 {
                        lines.push("Heavy hit!".to_string());
                        2
                    } else {
                        lines.push("Critical hit!".to_string());
                        3
                    };

                    Some(Condition::Dazed(stacks))
                } else {
                    None
                };

                if let Some(condition) = condition {
                    let log_line = self
                        .perform_effect_application(ApplyEffect::Condition(condition), character);
                    lines.push(format!("{} (Shield bash)", log_line));
                } else {
                    lines.push("  Miss!".to_string());
                }

                let offensive = OffensiveHitReactionOutcome {
                    inflicted_condition: condition,
                };

                self.event_handler.handle(GameEvent::CharacterReactedToHit {
                    main_line: format!("{} reacted with Shield bash", reactor.name),
                    detail_lines: lines,
                    reactor: reactor.id(),
                    outcome: HitReactionOutcome {
                        received_condition: None,
                        offensive: Some(offensive),
                    },
                });
            }
        }
    }

    fn enter_state_longer_after_action(mut self) -> GameState {
        {
            let mut character = self.characters.get_mut(self.active_character_id);
            if character.action_points == 0 {
                self.perform_end_of_turn_character(&mut character);
                self.active_character_id = self.characters.next_id(self.active_character_id);
                //self.active_character().recover_action_points();
            }
        }

        transition_to(GameState::AwaitingChooseAction(StateChooseAction {
            game: self,
        }))
    }

    fn enter_state_deliberately_end_turn(mut self) -> GameState {
        {
            let mut character = self.characters.get_mut(self.active_character_id);
            self.log(format!("{} ended their turn", character.name));
            self.perform_end_of_turn_character(&mut character);
            self.active_character_id = self.characters.next_id(self.active_character_id);
        }

        transition_to(GameState::AwaitingChooseAction(StateChooseAction {
            game: self,
        }))
    }

    fn perform_end_of_turn_character(&self, character: &mut Character) {
        if character.conditions.bleeding > 0 {
            self.perform_losing_health(character, BLEEDING_DAMAGE_AMOUNT);
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

        character.recover_action_points();
        character.conditions.mainhand_exertion = 0;
        character.conditions.offhand_exertion = 0;
        character.stamina.gain(character.stamina.max / 2);

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
    Moved {
        character: CharacterId,
        from: (u32, u32),
        to: (u32, u32),
    },
    CharacterTookDamage {
        character: CharacterId,
        amount: u32,
    },
    CharacterReactedToHit {
        main_line: String,
        detail_lines: Vec<String>,
        reactor: CharacterId,
        outcome: HitReactionOutcome,
    },
    Attacked {
        attacker: CharacterId,
        target: CharacterId,
        outcome: AttackOutcome,
        detail_lines: Vec<String>,
    },
    SpellWasCast {
        caster: CharacterId,
        target: CharacterId,
        outcome: SpellOutcome,
        spell: Spell,
        detail_lines: Vec<String>,
    },
    CharacterReceivedSelfEffect {
        character: CharacterId,
        condition: Condition,
    },
    CharacterDied {
        character: CharacterId,
    },
}

#[derive(Debug, Copy, Clone)]
pub enum AttackOutcome {
    Hit(u32),
    Dodge,
    Parry,
    Miss,
}

#[derive(Debug, Copy, Clone)]
pub struct HitReactionOutcome {
    pub received_condition: Option<Condition>,
    pub offensive: Option<OffensiveHitReactionOutcome>,
}

#[derive(Debug, Copy, Clone)]
pub struct OffensiveHitReactionOutcome {
    pub inflicted_condition: Option<Condition>,
}

#[derive(Debug, Copy, Clone)]
pub enum SpellOutcome {
    Hit(u32),
    Resist,
}

pub struct StateChooseAction {
    pub game: CoreGame,
}

impl StateChooseAction {
    pub fn proceed(self, action: Option<Action>) -> GameState {
        if let Some(action) = action {
            self.game.enter_state_action(action)
        } else {
            self.game.enter_state_deliberately_end_turn()
        }
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
    pub is_within_melee: bool,
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
    pub is_within_melee: bool,
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
    let mut bonus = attacker.attack_bonus(hand);
    bonus.advantage += defender.incoming_attack_advantage();
    let target = defender
        .evasion()
        .saturating_sub(attacker.attack_modifier(hand))
        .max(1);
    probability_of_d20_reaching(target, bonus)
}

pub fn prob_spell_hit(caster: &Character, spell_type: SpellType, defender: &Character) -> f32 {
    let defender_value = match spell_type {
        SpellType::Mental => defender.will(),
        SpellType::Projectile => defender.evasion(),
    };
    let target = defender_value
        .saturating_sub(caster.spell_modifier())
        .max(1);
    probability_of_d20_reaching(target, DiceRollBonus::default())
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
        let entry = self.0.iter().find(|(id, _ch)| *id == character_id);

        match entry {
            Some((_id, ch)) => ch.borrow(),
            None => panic!("No character with id: {character_id}"),
        }
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
    pub icon: IconId,
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
    pub icon: IconId,
    pub action_point_cost: u32,
    pub stamina_cost: u32,
    pub effect: OnAttackedReactionEffect,

    pub must_be_melee: bool,
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
    pub icon: IconId,
    pub action_point_cost: u32,
    pub effect: OnHitReactionEffect,
    pub must_be_melee: bool,
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

impl Display for AttackHitEffect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AttackHitEffect::Apply(apply_effect) => match apply_effect {
                ApplyEffect::RemoveActionPoints(n) => {
                    f.write_fmt(format_args!("Target loses {n} AP"))
                }
                ApplyEffect::Condition(condition) => {
                    f.write_fmt(format_args!("Target receives {condition:?}"))
                }
            },
            AttackHitEffect::SkipExertion => f.write_fmt(format_args!("No exertion")),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum Condition {
    Dazed(u32),
    Bleeding,
    Braced,
    Raging,
    CarefulAim,
    Weakened(u32),
    MainHandExertion(u32),
    OffHandExertion(u32),
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct ConditionDescription {
    pub name: &'static str,
    pub description: &'static str,
}

#[derive(Debug, Copy, Clone, Default)]
struct Conditions {
    dazed: u32,
    bleeding: u32,
    braced: bool,
    raging: bool,
    careful_aim: bool,
    weakened: u32,
    mainhand_exertion: u32,
    offhand_exertion: u32,
}

pub const DAZED_DESCRIPTION: ConditionDescription = ConditionDescription {
    name: "Dazed",
    description: "Gains no evasion from agility and attacks with disadvantage",
};

const BLEEDING_DAMAGE_AMOUNT: u32 = 1;
pub const BLEEDING_DESCRIPTION: ConditionDescription = ConditionDescription {
    name: "Bleeding",
    description: "Loses 1 health at end of turn",
};

pub const WEAKENED_DESCRIPTION: ConditionDescription = ConditionDescription {
    name: "Weakened",
    description: "-1 to attributes for each stack",
};

pub const MAINHAND_EXERTION_DESCRIPTION: ConditionDescription = ConditionDescription {
    name: "Exerted (main-hand)",
    description: "-1 per stack on further similar actions",
};
pub const OFFHAND_EXERTION_DESCRIPTION: ConditionDescription = ConditionDescription {
    name: "Exerted (off-hand)",
    description: "-1 per stack on further similar actions",
};

impl Conditions {
    pub fn descriptions(&self) -> Vec<(ConditionDescription, Option<u32>)> {
        let mut result = vec![];
        if self.dazed > 0 {
            result.push((DAZED_DESCRIPTION, Some(self.dazed)));
        }
        if self.bleeding > 0 {
            result.push((BLEEDING_DESCRIPTION, Some(self.bleeding)));
        }
        if self.braced {
            result.push((BRACED_DESCRIPTION, None));
        }
        if self.raging {
            result.push((RAGING_DESCRIPTION, None));
        }
        if self.weakened > 0 {
            result.push((WEAKENED_DESCRIPTION, Some(self.weakened)));
        }
        if self.mainhand_exertion > 0 {
            result.push((MAINHAND_EXERTION_DESCRIPTION, Some(self.mainhand_exertion)));
        }
        if self.offhand_exertion > 0 {
            result.push((OFFHAND_EXERTION_DESCRIPTION, Some(self.offhand_exertion)));
        }

        result
    }
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
        enhancements: Vec<SpellEnhancement>,
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
    pub icon: IconId,
    pub action_point_cost: u32,
    pub stamina_cost: u32,
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

    pub fn mana_cost(&self) -> u32 {
        match self {
            BaseAction::Attack { .. } => 0,
            BaseAction::SelfEffect(..) => 0,
            BaseAction::CastSpell(spell) => spell.mana_cost,
            BaseAction::Move { .. } => 0,
        }
    }

    pub fn stamina_cost(&self) -> u32 {
        match self {
            BaseAction::Attack { .. } => 0,
            BaseAction::SelfEffect(sea) => sea.stamina_cost,
            BaseAction::CastSpell(..) => 0,
            BaseAction::Move { .. } => 0,
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
    pub id: SpellId,
    pub name: &'static str,
    pub description: &'static str,
    pub icon: IconId,
    pub action_point_cost: u32,
    pub mana_cost: u32,
    pub damage: u32,
    pub on_hit_effect: Option<ApplyEffect>,
    pub spell_type: SpellType,
    pub possible_enhancements: [Option<SpellEnhancement>; 2],
    pub range: Range,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct SpellEnhancement {
    pub name: &'static str,
    pub description: &'static str,
    pub icon: IconId,
    pub mana_cost: u32,
    pub bonus_damage: u32,
    pub effect: Option<SpellEnhancementEffect>,
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
    pub icon: IconId,
    pub stamina_cost: u32,
    pub add_percentage: u32,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct Hand {
    weapon: Option<Weapon>,
    shield: Option<Shield>,
}

impl Hand {
    fn is_empty(&self) -> bool {
        self.weapon.is_none() && self.shield.is_none()
    }
}

pub type CharacterId = u32;

#[derive(Debug, Copy, Clone)]
pub struct Attributes {
    pub strength: u32,
    pub agility: u32,
    pub intellect: u32,
    pub spirit: u32,
}

impl Attributes {
    fn new(str: u32, agi: u32, intel: u32, spi: u32) -> Self {
        Self {
            strength: str,
            agility: agi,
            intellect: intel,
            spirit: spi,
        }
    }
}

#[derive(Debug)]
pub struct Character {
    id: Option<CharacterId>,
    pub sprite: SpriteId,
    pub has_died: bool,
    pub player_controlled: bool,
    // TODO i32 instead?
    pub position: (u32, u32),
    pub name: &'static str,
    pub base_attributes: Attributes,
    pub health: NumberedResource,
    pub mana: NumberedResource,
    pub move_range: f32,
    pub armor: Option<ArmorPiece>,
    main_hand: Hand,
    off_hand: Hand,
    conditions: Conditions,
    pub action_points: u32,
    pub max_reactive_action_points: u32,
    pub stamina: NumberedResource,
    pub known_attack_enhancements: Vec<AttackEnhancement>,
    known_actions: Vec<BaseAction>,
    known_attacked_reactions: Vec<OnAttackedReaction>,
    known_on_hit_reactions: Vec<OnHitReaction>,
}

pub const MOVE_ACTION_COST: u32 = 1;

impl Character {
    fn new(
        player_controlled: bool,
        name: &'static str,
        texture: SpriteId,
        base_attributes: Attributes,
        position: (u32, u32),
    ) -> Self {
        let max_health = 7 + base_attributes.strength;
        let max_mana = if base_attributes.spirit < 3 {
            0
        } else {
            1 + 2 * (base_attributes.spirit - 3)
        };
        let move_range = 0.9 + base_attributes.agility as f32 * 0.1;
        let max_stamina = (base_attributes.strength + base_attributes.agility).saturating_sub(5);
        let max_reactive_action_points = base_attributes.intellect / 2;
        Self {
            id: None,
            sprite: texture,
            has_died: false,
            player_controlled,
            position,
            name,
            base_attributes,
            health: NumberedResource::new(max_health),
            mana: NumberedResource::new(max_mana),
            move_range,
            armor: None,
            main_hand: Default::default(),
            off_hand: Default::default(),
            conditions: Default::default(),
            action_points: MAX_ACTION_POINTS,
            max_reactive_action_points,
            stamina: NumberedResource::new(max_stamina),
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
                //BaseAction::SelfEffect(BRACE),
                BaseAction::Move {
                    action_point_cost: MOVE_ACTION_COST,
                    range: move_range,
                },
            ],
            known_attacked_reactions: Default::default(),
            known_on_hit_reactions: Default::default(),
        }
    }

    pub fn recover_action_points(&mut self) {
        self.action_points = MAX_ACTION_POINTS;
    }

    pub fn condition_descriptions(&self) -> Vec<(ConditionDescription, Option<u32>)> {
        self.conditions.descriptions()
    }

    fn lose_action_points(&mut self, amount: u32) {
        self.action_points = self.action_points.saturating_sub(amount);
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

    pub fn known_actions(&self) -> Vec<(&'static str, BaseAction)> {
        self.known_actions
            .iter()
            .filter_map(|action: &BaseAction| match action {
                BaseAction::Attack { hand, .. } => self.weapon(*hand).map(|weapon| {
                    (
                        weapon.name,
                        BaseAction::Attack {
                            hand: *hand,
                            action_point_cost: weapon.action_point_cost,
                        },
                    )
                }),
                BaseAction::SelfEffect(_self_effect_action) => Some(("", *action)),
                BaseAction::CastSpell(_spell) => Some(("", *action)),
                BaseAction::Move { .. } => Some(("", *action)),
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
            BaseAction::SelfEffect(sea) => {
                ap >= sea.action_point_cost && self.stamina.current >= sea.stamina_cost
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
                "1.5x".to_string(),
                MovementEnhancement {
                    name: "Sprint",
                    action_point_cost: 0,
                    icon: IconId::X1point5,
                    stamina_cost: 1,
                    add_percentage: 50,
                },
            ),
            (
                "2x".to_string(),
                MovementEnhancement {
                    name: "Extended",
                    action_point_cost: 1,
                    icon: IconId::X2,
                    stamina_cost: 0,
                    add_percentage: 100,
                },
            ),
            (
                "3x".to_string(),
                MovementEnhancement {
                    name: "Extended sprint",
                    action_point_cost: 1,
                    icon: IconId::X3,
                    stamina_cost: 2,
                    add_percentage: 200,
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

    pub fn usable_on_attacked_reactions(
        &self,
        is_within_melee: bool,
    ) -> Vec<(String, OnAttackedReaction)> {
        let mut usable = self.known_on_attacked_reactions();
        usable.retain(|reaction| self.can_use_on_attacked_reaction(reaction.1, is_within_melee));
        usable
    }

    pub fn can_use_on_attacked_reaction(
        &self,
        reaction: OnAttackedReaction,
        is_within_melee: bool,
    ) -> bool {
        self.action_points >= reaction.action_point_cost
            && (self.action_points - reaction.action_point_cost)
                >= (MAX_ACTION_POINTS - self.max_reactive_action_points)
            && self.stamina.current >= reaction.stamina_cost
            && (!reaction.must_be_melee || is_within_melee)
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

    pub fn usable_on_hit_reactions(&self, is_within_melee: bool) -> Vec<(String, OnHitReaction)> {
        let mut usable = self.known_on_hit_reactions();
        usable.retain(|r| self.can_use_on_hit_reaction(r.1, is_within_melee));
        usable
    }

    pub fn can_use_on_hit_reaction(&self, reaction: OnHitReaction, is_within_melee: bool) -> bool {
        if let OnHitReactionEffect::Rage = reaction.effect {
            if self.conditions.raging {
                // Can't use this reaction while already raging
                return false;
            }
        }
        self.action_points >= reaction.action_point_cost
            && (self.action_points - reaction.action_point_cost)
                >= (MAX_ACTION_POINTS - self.max_reactive_action_points)
            && (!reaction.must_be_melee || is_within_melee)
    }

    pub fn can_use_spell_enhancement(&self, spell: Spell, enhancement_index: usize) -> bool {
        let enhancement = spell.possible_enhancements[enhancement_index].unwrap();
        self.mana.current >= spell.mana_cost + enhancement.mana_cost
    }

    fn strength(&self) -> u32 {
        (self.base_attributes.strength as i32 - self.conditions.weakened as i32).max(1) as u32
    }

    fn agility(&self) -> u32 {
        (self.base_attributes.agility as i32 - self.conditions.weakened as i32).max(1) as u32
    }

    fn intellect(&self) -> u32 {
        (self.base_attributes.intellect as i32 - self.conditions.weakened as i32).max(1) as u32
    }

    fn spirit(&self) -> u32 {
        (self.base_attributes.spirit as i32 - self.conditions.weakened as i32).max(1) as u32
    }

    pub fn spell_modifier(&self) -> u32 {
        self.intellect() + self.spirit()
    }

    fn is_dazed(&self) -> bool {
        self.conditions.dazed > 0
    }

    pub fn evasion(&self) -> u32 {
        let from_agi = self.evasion_from_agility();
        let from_int = self.evasion_from_intellect();
        let from_shield = self
            .off_hand
            .shield
            .map(|shield| shield.evasion)
            .unwrap_or(0);
        let from_braced = if self.conditions.braced {
            BRACED_DEFENSE_BONUS
        } else {
            0
        };
        10 + from_agi + from_int + from_shield + from_braced
    }

    fn evasion_from_agility(&self) -> u32 {
        let mut bonus = if self.is_dazed() { 0 } else { self.agility() };
        if let Some(armor) = self.armor {
            if let Some(limit) = armor.limit_evasion_from_agi {
                bonus = bonus.min(limit);
            }
        }
        bonus
    }

    fn evasion_from_intellect(&self) -> u32 {
        self.intellect() / 2
    }

    pub fn will(&self) -> u32 {
        10 + self.intellect()
    }

    pub fn toughness(&self) -> u32 {
        10 + self.strength()
    }

    pub fn protection_from_armor(&self) -> u32 {
        self.armor.map(|armor| armor.protection).unwrap_or(0)
    }

    pub fn attack_modifier(&self, hand: HandType) -> u32 {
        let str = self.strength();
        let agi = self.agility();
        let weapon = self.weapon(hand).unwrap();

        let use_str = match weapon.attack_attribute {
            AttackAttribute::Strength => true,
            AttackAttribute::Agility => false,
            AttackAttribute::Finesse => str >= agi,
        };

        let physical_attr = if use_str { str } else { agi };
        physical_attr + self.intellect()
    }

    fn hand_exertion(&self, hand_type: HandType) -> u32 {
        match hand_type {
            HandType::MainHand => self.conditions.mainhand_exertion,
            HandType::OffHand => self.conditions.offhand_exertion,
        }
    }

    fn attack_bonus(&self, hand_type: HandType) -> DiceRollBonus {
        let flat_amount = -(self.hand_exertion(hand_type) as i32);

        let mut advantage = 0i32;
        if self.is_dazed() {
            advantage -= 1;
        }
        if self.conditions.raging {
            advantage += 1;
        }
        if self.conditions.careful_aim {
            advantage += 1;
        }

        DiceRollBonus {
            advantage,
            flat_amount,
        }
    }

    pub fn explain_attack_bonus(&self, hand_type: HandType) -> String {
        let mut s = String::new();
        if self.hand_exertion(hand_type) > 0 {
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
            Condition::MainHandExertion(n) => self.conditions.mainhand_exertion += n,
            Condition::OffHandExertion(n) => self.conditions.offhand_exertion += n,
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

pub fn distance_between(source: (i32, i32), destination: (i32, i32)) -> f32 {
    (((destination.0 - source.0).pow(2) + (destination.1 - source.1).pow(2)) as f32).sqrt()
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
    pub limit_evasion_from_agi: Option<u32>,
    pub icon: EquipmentIconId,
}

#[derive(Debug, Copy, Clone)]
pub struct Weapon {
    pub name: &'static str,
    pub sprite: Option<SpriteId>,
    pub icon: EquipmentIconId,
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
    pub sprite: Option<SpriteId>,
    pub evasion: u32,
    pub on_hit_reaction: Option<OnHitReaction>,
}

#[derive(Debug, Copy, Clone)]
pub enum AttackAttribute {
    Strength,
    Agility,
    Finesse,
}

impl Display for AttackAttribute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AttackAttribute::Strength => f.write_str("str"),
            AttackAttribute::Agility => f.write_str("agi"),
            AttackAttribute::Finesse => f.write_str("str | agi"),
        }
    }
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

impl Display for Range {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Range::Melee => f.write_str("melee"),
            Range::Ranged(range) => f.write_str(&range.to_string()),
            Range::Float(range) => f.write_fmt(format_args!("{:.2}", range)),
        }
    }
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
