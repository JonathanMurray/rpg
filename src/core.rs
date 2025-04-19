use std::cell::{Cell, RefCell};
use std::cell::{Ref, RefMut};

use std::fmt::Display;
use std::rc::Rc;

use crate::d20::{probability_of_d20_reaching, roll_d20_with_advantage, DiceRollBonus};

use crate::data::{
    BOW, BRACED_DEFENSE_BONUS, BRACED_DESCRIPTION, EFFICIENT, HEAL, KILL, PARRY_EVASION_BONUS,
    RAGE, RAGING_DESCRIPTION,
};
use crate::data::{CHAIN_MAIL, DAGGER, SIDE_STEP};
use crate::data::{FIREBALL, LEATHER_ARMOR, MIND_BLAST, OVERWHELMING, SCREAM, SMALL_SHIELD};

use crate::game_ui_orchestration::GameUserInterface;
use crate::textures::{EquipmentIconId, IconId, SpriteId};

pub const MAX_ACTION_POINTS: u32 = 5;

pub struct CoreGame {
    pub characters: Characters,
    pub active_character_id: CharacterId,
    user_interface: GameUserInterface,
}

impl CoreGame {
    pub fn new(user_interface: GameUserInterface) -> Self {
        let active_character_id = 0;

        let mut bob = Character::new(
            true,
            "Bob",
            SpriteId::Character,
            Attributes::new(5, 2, 7, 7),
            (1, 9),
        );
        bob.main_hand.weapon = Some(BOW);
        bob.off_hand.shield = Some(SMALL_SHIELD);
        bob.armor = Some(CHAIN_MAIL);
        bob.known_attack_enhancements.push(OVERWHELMING);
        bob.known_attacked_reactions.push(SIDE_STEP);
        bob.known_attack_enhancements.push(EFFICIENT);
        bob.known_on_hit_reactions.push(RAGE);
        bob.known_actions.push(BaseAction::CastSpell(SCREAM));
        bob.known_actions.push(BaseAction::CastSpell(MIND_BLAST));
        bob.known_actions.push(BaseAction::CastSpell(FIREBALL));
        bob.known_actions.push(BaseAction::CastSpell(HEAL));
        bob.known_actions.push(BaseAction::CastSpell(KILL));

        let mut enemy1 = Character::new(
            false,
            "Gremlin Nob",
            SpriteId::Character2,
            Attributes::new(5, 5, 3, 3),
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
            Attributes::new(1, 4, 1, 5),
            (2, 3),
        );
        enemy2.main_hand.weapon = Some(BOW);

        let mut david = Character::new(
            true,
            "David",
            SpriteId::Character,
            Attributes::new(2, 10, 10, 5),
            (5, 7),
        );
        david.health.lose(6);
        david.main_hand.weapon = Some(DAGGER);

        let characters = Characters::new(vec![bob, enemy1, enemy2, david]);

        Self {
            characters,
            active_character_id,
            user_interface,
        }
    }

    pub async fn run(mut self) {
        loop {
            let action = self.ui_select_action().await;

            if let Some(action) = action {
                let ap_before_action = self.active_character().action_points.current();
                let attack_hit = self.perform_action(action).await;

                // You recover from 1 stack of Dazed for each AP you spend
                // This must happen before "on attacked and hit" reactions because those might
                // inflict new Dazed stacks, which should not be covered here.
                let spent = ap_before_action - self.active_character().action_points.current();
                self.perform_recover_from_dazed(self.active_character_id, spent)
                    .await;

                if let Some((victim_id, damage)) = attack_hit {
                    // TODO this can remove a Dazed that was just added from the attack, which is bad.

                    // You recover from 1 stack of Dazed each time you're hit by an attack
                    self.perform_recover_from_dazed(victim_id, 1).await;

                    let character = self.active_character();

                    let victim = self.characters.get(victim_id);

                    let is_within_melee =
                        within_meele(character.position.get(), victim.position.get());
                    let can_react = !victim.has_died.get()
                        && !victim.usable_on_hit_reactions(is_within_melee).is_empty();

                    if can_react {
                        if let Some(reaction) = self
                            .ui_choose_hit_reaction(victim_id, damage, is_within_melee)
                            .await
                        {
                            self.perform_on_hit_reaction(victim_id, reaction).await;
                        }
                    }
                }

                {
                    let character = self.characters.get(self.active_character_id);
                    if character.action_points.current() == 0 {
                        self.perform_end_of_turn_character().await;
                        self.active_character_id =
                            self.characters.next_id(self.active_character_id);
                    }
                }
            } else {
                let character = self.characters.get(self.active_character_id);
                let name = character.name;
                self.log(format!("{} ended their turn", name)).await;
                self.perform_end_of_turn_character().await;
                self.active_character_id = self.characters.next_id(self.active_character_id);
            }

            self.remove_dead().await;
        }
    }

    pub fn active_character(&self) -> &Character {
        self.characters.get(self.active_character_id)
    }

    pub fn is_players_turn(&self) -> bool {
        self.active_character().player_controlled
    }

    async fn perform_action(&mut self, action: Action) -> Option<(CharacterId, u32)> {
        match action {
            Action::Attack {
                hand,
                enhancements,
                target,
            } => {
                let attacker = self.active_character();
                let defender = self.characters.get(target);

                assert!(
                    attacker
                        .reaches_with_attack(hand, defender.position.get())
                        .1
                        != ActionReach::No
                );

                attacker
                    .action_points
                    .spend(attacker.weapon(hand).unwrap().action_point_cost);
                for enhancement in &enhancements {
                    attacker.action_points.spend(enhancement.action_point_cost);
                    attacker.stamina.spend(enhancement.stamina_cost);
                }

                let is_within_melee =
                    within_meele(attacker.position.get(), defender.position.get());
                let defender_can_react_to_attack = !defender
                    .usable_on_attacked_reactions(is_within_melee)
                    .is_empty();

                let reaction = if defender_can_react_to_attack {
                    self.ui_choose_attack_reaction(hand, target, is_within_melee)
                        .await
                } else {
                    None
                };

                self.perform_attack(hand, enhancements, target, reaction)
                    .await
            }
            Action::SelfEffect(SelfEffectAction {
                name,
                action_point_cost,
                stamina_cost,
                effect,
                ..
            }) => {
                let character = self.active_character();
                character.action_points.spend(action_point_cost);
                character.stamina.spend(stamina_cost);
                let character_name = character.name;

                self.log(format!("{} uses {}", character_name, name)).await;

                if let ApplyEffect::Condition(condition) = effect {
                    self.ui_handle_event(GameEvent::CharacterReceivedSelfEffect {
                        character: self.active_character_id,
                        condition,
                    })
                    .await;
                }

                let log_line = self.perform_effect_application(effect, &self.active_character());
                self.log(log_line).await;
                None
            }
            Action::CastSpell {
                spell,
                enhancements,
                target,
            } => {
                self.perform_spell(spell, enhancements, target).await;
                None
            }
            Action::Move {
                action_point_cost,
                positions,
                enhancements,
            } => {
                let character = self.active_character();
                character.action_points.spend(action_point_cost);
                for enhancement in &enhancements {
                    character.action_points.spend(enhancement.action_point_cost);
                    character.stamina.spend(enhancement.stamina_cost);
                }

                self.perform_movement(positions).await;
                None
            }
        }
    }

    async fn ui_handle_event(&self, event: GameEvent) {
        self.user_interface.handle_event(self, event).await;
    }

    async fn ui_select_action(&self) -> Option<Action> {
        self.user_interface.select_action(self).await
    }

    async fn ui_choose_attack_reaction(
        &self,
        hand: HandType,
        target: CharacterId,
        is_within_melee: bool,
    ) -> Option<OnAttackedReaction> {
        let attacker_id = self.active_character_id;
        self.user_interface
            .choose_attack_reaction(self, attacker_id, hand, target, is_within_melee)
            .await
    }

    async fn ui_choose_hit_reaction(
        &self,
        victim_id: CharacterId,
        damage: u32,
        is_within_melee: bool,
    ) -> Option<OnHitReaction> {
        let attacker_id = self.active_character_id;
        self.user_interface
            .choose_hit_reaction(self, attacker_id, victim_id, damage, is_within_melee)
            .await
    }

    async fn perform_movement(&self, mut positions: Vec<(u32, u32)>) {
        while !positions.is_empty() {
            let character = self.active_character();
            let new_position = positions.remove(0);
            for ch in self.characters.iter() {
                if new_position == ch.position.get() {
                    panic!(
                        "Character {} tried to move 0 distance from {:?}",
                        ch.id(),
                        ch.position.get()
                    );
                }
            }

            let prev_position = character.position.get();
            let id = character.id();

            self.ui_handle_event(GameEvent::Moved {
                character: id,
                from: prev_position,
                to: new_position,
            })
            .await;

            self.active_character().position.set(new_position);
        }
    }

    fn perform_effect_application(&self, effect: ApplyEffect, receiver: &Character) -> String {
        match effect {
            ApplyEffect::RemoveActionPoints(n) => {
                receiver.action_points.lose(n);
                format!("  {} lost {} AP", receiver.name, n)
            }
            ApplyEffect::Condition(condition) => {
                receiver.receive_condition(condition);
                format!("  {} received {:?}", receiver.name, condition)
            }
        }
    }

    async fn perform_spell(
        &mut self,

        spell: Spell,
        enhancements: Vec<SpellEnhancement>,
        target_id: CharacterId,
    ) {
        let caster_ref_mut = self.active_character();
        let caster_id = caster_ref_mut.id();
        let target_ref = self.characters.get(target_id);

        assert!(caster_ref_mut.can_reach_with_spell(spell, target_ref.position.get()));

        caster_ref_mut.action_points.spend(spell.action_point_cost);
        caster_ref_mut.mana.spend(spell.mana_cost);

        for enhancement in &enhancements {
            caster_ref_mut.mana.spend(enhancement.mana_cost);
        }

        let mut cast_n_times = 1;
        for enhancement in &enhancements {
            if enhancement.effect == Some(SpellEnhancementEffect::CastTwice) {
                cast_n_times = 2;
            }
        }

        for i in 0..cast_n_times {
            let caster_ref = self.characters.get(caster_id);
            let target_ref = self.characters.get(target_id);

            let mut detail_lines = vec![];

            let roll = roll_d20_with_advantage(0);
            let spell_result = roll + caster_ref.spell_modifier();

            let outcome = if let SpellTargetType::SingleEnemy(spell_type) = spell.target_type {
                let (defense_label, defense) = match spell_type {
                    OffensiveSpellType::Mental => ("will", target_ref.will()),
                    OffensiveSpellType::Projectile => ("evasion", target_ref.evasion()),
                };

                detail_lines.push(format!(
                    "Rolled: {} (+{} spell mod) = {}, vs {}={}",
                    roll,
                    caster_ref.spell_modifier(),
                    spell_result,
                    defense_label,
                    defense,
                ));

                if spell_result >= defense {
                    detail_lines.push("The spell was successful!".to_string());
                    let mut dmg_calculation = spell.damage as i32;
                    let mut dmg_str = format!("  Damage: {} ({})", dmg_calculation, spell.name);

                    let degree_of_success = (spell_result - defense) / 5;
                    let (label, bonus_dmg) = match degree_of_success {
                        0 => ("".to_string(), 0),
                        1 => ("Heavy hit".to_string(), 1),
                        n => (format!("Heavy hit ({n})"), n as i32),
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
                        self.perform_losing_health(&target_ref, damage);
                        dmg_str.push_str(&format!(" = {damage}"));
                        detail_lines.push(dmg_str);
                    }

                    if let Some(effect) = spell.on_hit_effect {
                        let log_line = self.perform_effect_application(effect, &target_ref);
                        detail_lines.push(format!("{} ({})", log_line, spell.name));
                    }

                    for enhancement in &enhancements {
                        if let Some(SpellEnhancementEffect::OnHitEffect(effect)) =
                            enhancement.effect
                        {
                            let log_line = self.perform_effect_application(effect, &target_ref);
                            detail_lines.push(format!("{} ({})", log_line, enhancement.name));
                        }
                    }

                    SpellOutcome::HitEnemy(damage)
                } else {
                    match spell_type {
                        OffensiveSpellType::Mental => {
                            detail_lines.push(format!("{} resisted the spell!", target_ref.name))
                        }
                        OffensiveSpellType::Projectile => {
                            detail_lines.push("The spell missed!".to_string())
                        }
                    }
                    SpellOutcome::Resist
                }
            } else {
                detail_lines.push(format!(
                    "Rolled: {} (+{} spell mod) = {}",
                    roll,
                    caster_ref.spell_modifier(),
                    spell_result,
                ));

                let mut health_gained = 0;
                if spell.healing > 0 {
                    let mut healing = spell.healing;

                    let mut line = format!("Healing: {} ({})", spell.healing, spell.name);

                    let effectiveness = spell_result / 10;
                    if effectiveness > 0 {
                        detail_lines.push(format!("Fortune: {}", effectiveness));
                        line.push_str(&format!(" +{} (fortune)", effectiveness));
                        healing += effectiveness;
                    }
                    line.push_str(&format!(" = {}", healing));
                    detail_lines.push(line);

                    health_gained = target_ref.health.gain(healing);
                    detail_lines.push(format!(
                        "{} was healed for {}",
                        target_ref.name, health_gained
                    ));
                };
                SpellOutcome::HealedAlly(health_gained)
            };

            if i < cast_n_times - 1 {
                detail_lines.push(format!("{} cast again!", caster_ref.name))
            }

            let caster_id = caster_ref.id();
            let target_id = target_ref.id();

            self.ui_handle_event(GameEvent::SpellWasCast {
                caster: caster_id,
                target: target_id,
                outcome,
                spell,
                detail_lines,
            })
            .await;
        }
    }

    fn perform_losing_health(&self, character: &Character, amount: u32) {
        character.health.lose(amount);
        //self.log(format!("  {} took {} damage", character.name, amount));

        if character.health.current() == 0 {
            character.has_died.set(true);
        }
    }

    async fn log(&self, line: impl Into<String>) {
        self.ui_handle_event(GameEvent::LogLine(line.into())).await;
    }

    async fn perform_attack(
        &mut self,
        hand_type: HandType,
        attack_enhancements: Vec<AttackEnhancement>,
        defender_id: CharacterId,
        defender_reaction: Option<OnAttackedReaction>,
    ) -> Option<(CharacterId, u32)> {
        let attacker = self.active_character();
        let defender = self.characters.get(defender_id);

        let circumstance_advantage = match attacker
            .reaches_with_attack(hand_type, defender.position.get())
            .1
        {
            ActionReach::Yes => 0,
            ActionReach::YesButDisadvantage(..) => -1,
            ActionReach::No => unreachable!(),
        };

        let attack_bonus = attack_roll_bonus(
            &attacker,
            hand_type,
            &defender,
            circumstance_advantage,
            &attack_enhancements,
        );

        let mut evasion = defender.evasion();

        let mut defender_reacted_with_parry = false;
        let mut defender_reacted_with_sidestep = false;
        let mut skip_attack_exertion = false;

        let mut attack_hit = None;

        let attack_modifier = attacker.attack_modifier(hand_type);

        let mut detail_lines = vec![];

        if let Some(reaction) = defender_reaction {
            defender.action_points.spend(reaction.action_point_cost);
            defender.stamina.spend(reaction.stamina_cost);

            detail_lines.push(format!("{} reacted with {}", defender.name, reaction.name));

            match reaction.effect {
                OnAttackedReactionEffect::Parry => {
                    defender_reacted_with_parry = true;
                    let bonus_evasion = PARRY_EVASION_BONUS;

                    detail_lines.push(format!(
                        "  Evasion: {} +{} (Parry) = {}",
                        evasion,
                        bonus_evasion,
                        evasion + bonus_evasion
                    ));
                    evasion += bonus_evasion;
                    let p_hit =
                        probability_of_d20_reaching(evasion - attack_modifier, attack_bonus);
                    detail_lines.push(format!("  Chance to hit: {:.1}%", p_hit * 100f32));
                }
                OnAttackedReactionEffect::SideStep => {
                    defender_reacted_with_sidestep = true;
                    let bonus_evasion = defender.evasion_from_agility();
                    detail_lines.push(format!(
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
                    detail_lines.push(format!("  Chance to hit: {:.1}%", p_hit * 100f32));
                }
            }
        }

        let roll = roll_d20_with_advantage(attack_bonus.advantage);
        let attack_result = ((roll + attack_modifier) as i32 + attack_bonus.flat_amount) as u32;
        match attack_bonus.advantage.cmp(&0) {
            std::cmp::Ordering::Less => detail_lines.push(format!(
                "Rolled {} dice with disadvantage...",
                attack_bonus.advantage.abs() + 1
            )),
            std::cmp::Ordering::Equal => detail_lines.push("Rolled 1 die...".to_string()),
            std::cmp::Ordering::Greater => detail_lines.push(format!(
                "Rolled {} dice with advantage...",
                attack_bonus.advantage + 1
            )),
        }
        detail_lines.push(format!(
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

                detail_lines.push(format!("  Hit! ({} mitigated)", mitigated));
                dmg_str.push_str(&format!(" -{mitigated} (armor)"));
                dmg_calculation -= mitigated as i32;
            } else {
                on_true_hit_effect = weapon.on_true_hit;

                let degree_of_success = (attack_result - armored_defense) / 5;
                let (label, bonus_dmg) = match degree_of_success {
                    0 => ("True hit".to_string(), 0),
                    1 => ("Heavy hit".to_string(), 1),
                    n => (format!("Heavy hit ({n})"), n as i32),
                };

                detail_lines.push(format!("  {label}!"));
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

            detail_lines.push(dmg_str);

            self.perform_losing_health(&defender, damage);

            attack_hit = Some((defender_id, damage));

            if let Some(effect) = on_true_hit_effect {
                match effect {
                    AttackHitEffect::Apply(effect) => {
                        let log_line = self.perform_effect_application(effect, &defender);
                        detail_lines.push(format!("{} (true hit)", log_line))
                    }
                    AttackHitEffect::SkipExertion => skip_attack_exertion = true,
                }
            }

            for enhancement in &attack_enhancements {
                if let Some(effect) = enhancement.on_hit_effect {
                    let log_line = match effect {
                        AttackEnhancementOnHitEffect::RegainActionPoint => {
                            attacker.action_points.gain(1);
                            format!("{} regained 1 AP", attacker.name)
                        }
                        AttackEnhancementOnHitEffect::Target(apply_effect) => {
                            self.perform_effect_application(apply_effect, &defender)
                        }
                    };

                    detail_lines.push(format!("{} ({})", log_line, enhancement.name))
                }
            }

            AttackOutcome::Hit(damage)
        } else if defender_reacted_with_parry {
            detail_lines.push("  Parried!".to_string());
            AttackOutcome::Parry
        } else if defender_reacted_with_sidestep {
            detail_lines.push("  Side stepped!".to_string());
            AttackOutcome::Dodge
        } else {
            detail_lines.push("  Missed!".to_string());
            AttackOutcome::Miss
        };

        if skip_attack_exertion {
            detail_lines.push("  The attack did not lead to exertion (true hit)".to_string());
        } else {
            let exertion = match hand_type {
                HandType::MainHand => {
                    attacker.receive_condition(Condition::MainHandExertion(1));
                    attacker.hand_exertion(HandType::MainHand)
                }
                HandType::OffHand => {
                    attacker.receive_condition(Condition::OffHandExertion(1));
                    attacker.hand_exertion(HandType::OffHand)
                }
            };
            detail_lines.push(format!("  The attack led to exertion ({})", exertion));
        }

        if defender.lose_braced() {
            detail_lines.push(format!("{} lost Braced", defender.name));
        }

        self.ui_handle_event(GameEvent::Attacked {
            attacker: self.active_character_id,
            target: defender_id,
            outcome,
            detail_lines,
        })
        .await;

        attack_hit
    }

    #[allow(clippy::await_holding_refcell_ref)]
    async fn perform_recover_from_dazed(&mut self, character_id: CharacterId, stacks: u32) {
        let character = self.characters.get(character_id);

        if character.lose_dazed(stacks) {
            let name = character.name;
            self.log(format!("{} recovered from Dazed", name)).await;
        }
    }

    async fn perform_on_hit_reaction(&mut self, reactor_id: CharacterId, reaction: OnHitReaction) {
        let reactor = self.characters.get(reactor_id);
        reactor.action_points.spend(reaction.action_point_cost);
        let reactor_name = reactor.name;

        match reaction.effect {
            OnHitReactionEffect::Rage => {
                let condition = Condition::Raging;

                self.ui_handle_event(GameEvent::CharacterReactedToHit {
                    main_line: format!("{} reacted with Rage", reactor_name),
                    detail_lines: vec![],
                    reactor: reactor_id,
                    outcome: HitReactionOutcome {
                        received_condition: Some(condition),
                        offensive: None,
                    },
                })
                .await;

                let reactor = self.characters.get(reactor_id);
                reactor.receive_condition(condition);
            }
            OnHitReactionEffect::ShieldBash => {
                let mut lines = vec![];

                let offensive = {
                    let attacker = self.characters.get(self.active_character_id);
                    let reactor = self.characters.get(reactor_id);
                    let target = attacker.toughness();
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
                        let degree_of_success = (res - target) / 5;
                        let (label, bonus) = match degree_of_success {
                            0 => ("Hit".to_string(), 0),
                            1 => ("Heavy hit".to_string(), 1),
                            n => (format!("Heavy hit ({n})"), n),
                        };

                        let stacks = 1 + bonus;
                        lines.push(label);

                        Some(Condition::Dazed(stacks))
                    } else {
                        None
                    };

                    if let Some(condition) = condition {
                        let log_line = self.perform_effect_application(
                            ApplyEffect::Condition(condition),
                            &attacker,
                        );
                        lines.push(format!("{} (Shield bash)", log_line));
                    } else {
                        lines.push("  Miss!".to_string());
                    }
                    OffensiveHitReactionOutcome {
                        inflicted_condition: condition,
                    }
                };

                self.ui_handle_event(GameEvent::CharacterReactedToHit {
                    main_line: format!("{} reacted with Shield bash", reactor_name),
                    detail_lines: lines,
                    reactor: reactor_id,
                    outcome: HitReactionOutcome {
                        received_condition: None,
                        offensive: Some(offensive),
                    },
                })
                .await;
            }
        }
    }

    async fn perform_end_of_turn_character(&mut self) {
        let character = self.active_character();
        let name = character.name;

        if character.conditions.borrow().bleeding > 0 {
            self.perform_losing_health(&character, BLEEDING_DAMAGE_AMOUNT);
            character.conditions.borrow_mut().bleeding -= 1;
            if character.conditions.borrow().bleeding == 0 {
                self.log(format!("{} stopped Bleeding", name)).await;
            }
        }
        if character.conditions.borrow().weakened > 0 {
            character.conditions.borrow_mut().weakened = 0;
            self.log(format!("{} recovered from Weakened", name)).await;
        }
        if character.conditions.borrow().raging {
            character.conditions.borrow_mut().raging = false;
            self.log(format!("{} stopped Raging", name)).await;
        }
        character.action_points.set_to_max();
        character.conditions.borrow_mut().mainhand_exertion = 0;
        character.conditions.borrow_mut().offhand_exertion = 0;
        let max_stamina = character.stamina.max;
        character.stamina.gain(max_stamina / 2);

        self.log("End of turn.").await;
    }

    async fn remove_dead(&mut self) {
        for id in self.characters.remove_dead() {
            self.user_interface
                .handle_event(self, GameEvent::CharacterDied { character: id })
                .await;
        }
    }
}

pub trait GameEventHandler {
    fn handle(&self, event: GameEvent);
}

#[derive(Debug, Clone)]
pub enum GameEvent {
    LogLine(String),
    Moved {
        character: CharacterId,
        from: (u32, u32),
        to: (u32, u32),
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
    HitEnemy(u32),
    Resist,
    HealedAlly(u32),
}

pub fn as_percentage(probability: f32) -> String {
    if !(0.01..=0.99).contains(&probability) {
        format!("{:.1}%", probability * 100f32)
    } else {
        format!("{:.0}%", probability * 100f32)
    }
}

pub fn attack_roll_bonus(
    attacker: &Character,
    hand: HandType,
    defender: &Character,
    circumstance_advantage: i32,
    enhancements: &[AttackEnhancement],
) -> DiceRollBonus {
    let mut bonus = attacker.attack_roll_bonus(hand);
    bonus.advantage += defender.incoming_attack_advantage();
    bonus.advantage += circumstance_advantage;
    for enhancement in enhancements {
        bonus.advantage += enhancement.bonus_advantage as i32;
    }
    bonus
}

pub fn prob_attack_hit(
    attacker: &Character,
    hand: HandType,
    defender: &Character,
    circumstance_advantage: i32,
    enhancements: &[AttackEnhancement],
    reaction: Option<OnAttackedReaction>,
) -> f32 {
    let bonus = attack_roll_bonus(
        attacker,
        hand,
        defender,
        circumstance_advantage,
        enhancements,
    );
    let mut evasion = defender.evasion();

    if let Some(reaction) = reaction {
        match reaction.effect {
            OnAttackedReactionEffect::Parry => evasion += PARRY_EVASION_BONUS,
            OnAttackedReactionEffect::SideStep => evasion += defender.evasion_from_agility(),
        }
    }

    let dice_target = evasion
        .saturating_sub(attacker.attack_modifier(hand))
        .max(1);
    probability_of_d20_reaching(dice_target, bonus)
}

pub fn prob_spell_hit(
    caster: &Character,
    spell_type: OffensiveSpellType,
    defender: &Character,
) -> f32 {
    let defender_value = match spell_type {
        OffensiveSpellType::Mental => defender.will(),
        OffensiveSpellType::Projectile => defender.evasion(),
    };
    let target = defender_value
        .saturating_sub(caster.spell_modifier())
        .max(1);
    probability_of_d20_reaching(target, DiceRollBonus::default())
}

pub struct Characters(Vec<(CharacterId, Rc<Character>)>);

impl Characters {
    fn new(characters: Vec<Character>) -> Self {
        Self(
            characters
                .into_iter()
                .enumerate()
                .map(|(i, mut ch)| {
                    let id = i as CharacterId;
                    ch.id = Some(id);
                    (id, Rc::new(ch))
                })
                .collect(),
        )
    }

    fn next_id(&self, current_id: CharacterId) -> CharacterId {
        let mut i = 0;
        let mut passed_current = false;
        loop {
            let (id, ch) = &self.0[i];

            if passed_current && !ch.has_died.get() {
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

    pub fn get(&self, character_id: CharacterId) -> &Character {
        let entry = self.0.iter().find(|(id, _ch)| *id == character_id);

        match entry {
            Some((_id, ch)) => ch,
            None => panic!("No character with id: {character_id}"),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Rc<Character>> {
        self.0.iter().map(|(_id, ch)| ch)
    }

    pub fn iter_with_ids(&self) -> impl Iterator<Item = &(CharacterId, Rc<Character>)> {
        self.0.iter()
    }

    pub fn remove_dead(&mut self) -> Vec<CharacterId> {
        let mut removed = vec![];
        self.0.retain(|(_id, ch)| {
            if ch.has_died.get() {
                removed.push(ch.id());
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
    pub regain_action_points: u32,
    pub stamina_cost: u32,
    pub bonus_damage: u32,
    pub bonus_advantage: u32,
    pub on_hit_effect: Option<AttackEnhancementOnHitEffect>,
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
    pub name: &'static str,
    pub description: &'static str,
    pub icon: IconId,
    pub action_point_cost: u32,
    pub mana_cost: u32,
    pub damage: u32,
    pub healing: u32,
    pub on_hit_effect: Option<ApplyEffect>,
    pub possible_enhancements: [Option<SpellEnhancement>; 2],
    pub range: Range,
    pub target_type: SpellTargetType,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SpellTargetType {
    SingleEnemy(OffensiveSpellType),
    SingleAlly,
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
pub enum AttackEnhancementOnHitEffect {
    RegainActionPoint,
    Target(ApplyEffect),
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum OffensiveSpellType {
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

// TODO Give Character interior mutability?
// Then RefCell<Character> wouldn't be necessary, and CoreGame could freely
// hold onto character references while making calls to the UI.

#[derive(Debug)]
pub struct Character {
    id: Option<CharacterId>,
    pub sprite: SpriteId,
    pub has_died: Cell<bool>,
    pub player_controlled: bool,
    // TODO i32 instead?
    pub position: Cell<(u32, u32)>,
    pub name: &'static str,
    pub base_attributes: Attributes,
    pub health: NumberedResource,
    pub mana: NumberedResource,
    pub move_range: f32,
    pub capacity: u32,
    pub armor: Option<ArmorPiece>,
    main_hand: Hand,
    off_hand: Hand,
    conditions: RefCell<Conditions>,
    pub action_points: NumberedResource,
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
        let max_health = 6 + base_attributes.strength;
        let max_mana = (base_attributes.spirit * 2).saturating_sub(5);

        let move_range = 0.9 + base_attributes.agility as f32 * 0.1;
        let max_stamina = (base_attributes.strength + base_attributes.agility).saturating_sub(5);
        let max_reactive_action_points = base_attributes.intellect / 2;
        let capacity = base_attributes.strength * 2;
        Self {
            id: None,
            sprite: texture,
            has_died: Cell::new(false),
            player_controlled,
            position: Cell::new(position),
            name,
            base_attributes,
            health: NumberedResource::new(max_health),
            mana: NumberedResource::new(max_mana),
            move_range,
            capacity,
            armor: None,
            main_hand: Default::default(),
            off_hand: Default::default(),
            conditions: Default::default(),
            action_points: NumberedResource::new(MAX_ACTION_POINTS),
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

    fn lose_braced(&self) -> bool {
        let mut conditions = self.conditions.borrow_mut();
        if conditions.braced {
            conditions.braced = false;
            true
        } else {
            false
        }
    }

    fn lose_dazed(&self, stacks: u32) -> bool {
        let mut conditions = self.conditions.borrow_mut();
        if conditions.dazed > 0 {
            conditions.dazed = conditions.dazed.saturating_sub(stacks);
            if conditions.dazed == 0 {
                return true;
            }
        }
        false
    }

    pub fn equipment_weight(&self) -> u32 {
        let mut sum = 0;
        if let Some(weapon) = self.weapon(HandType::MainHand) {
            sum += weapon.weight;
        }
        if let Some(weapon) = self.weapon(HandType::OffHand) {
            sum += weapon.weight;
        } else if let Some(shield) = self.shield() {
            sum += shield.weight;
        }
        if let Some(armor) = self.armor {
            sum += armor.weight;
        }
        sum
    }

    pub fn condition_descriptions(&self) -> Vec<(ConditionDescription, Option<u32>)> {
        self.conditions.borrow().descriptions()
    }

    pub fn position_i32(&self) -> (i32, i32) {
        let pos = self.position.get();
        (pos.0 as i32, pos.1 as i32)
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

    pub fn reaches_with_attack(
        &self,
        hand: HandType,
        target_position: (u32, u32),
    ) -> (Range, ActionReach) {
        let weapon = self.weapon(hand).unwrap();
        let weapon_range = weapon.range;

        match weapon_range {
            WeaponRange::Melee => {
                if within_range_squared(
                    weapon_range.squared(),
                    self.position.get(),
                    target_position,
                ) {
                    (weapon_range.into_range(), ActionReach::Yes)
                } else {
                    (weapon_range.into_range(), ActionReach::No)
                }
            }
            WeaponRange::Ranged(..) => {
                if within_range_squared(
                    weapon_range.squared(),
                    self.position.get(),
                    target_position,
                ) {
                    if within_range_squared(
                        Range::Melee.squared(),
                        self.position.get(),
                        target_position,
                    ) {
                        (Range::Melee, ActionReach::YesButDisadvantage("Too close"))
                    } else {
                        (weapon_range.into_range(), ActionReach::Yes)
                    }
                } else {
                    let extended = weapon_range.extended().unwrap();
                    if within_range_squared(
                        extended.powf(2.0),
                        self.position.get(),
                        target_position,
                    ) {
                        (
                            weapon_range.into_range(),
                            ActionReach::YesButDisadvantage("far"),
                        )
                    } else {
                        (Range::Float(extended), ActionReach::No)
                    }
                }
            }
        }
    }

    pub fn can_reach_with_spell(&self, spell: Spell, target_position: (u32, u32)) -> bool {
        within_range_squared(spell.range.squared(), self.position.get(), target_position)
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
        let ap = self.action_points.current();
        match action {
            BaseAction::Attack {
                hand,
                action_point_cost: _,
            } => matches!(self.weapon(hand), Some(weapon) if ap >= weapon.action_point_cost),
            BaseAction::SelfEffect(sea) => {
                ap >= sea.action_point_cost && self.stamina.current() >= sea.stamina_cost
            }
            BaseAction::CastSpell(spell) => {
                ap >= spell.action_point_cost && self.mana.current() >= spell.mana_cost
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
        self.action_points.current() >= weapon.action_point_cost + enhancement.action_point_cost
            && self.stamina.current() >= enhancement.stamina_cost
    }

    pub fn usable_movement_enhancements(&self) -> Vec<(String, MovementEnhancement)> {
        let mut enhancements = vec![
            (
                "1.5x".to_string(),
                MovementEnhancement {
                    name: "Sprint",
                    icon: IconId::X1point5,
                    action_point_cost: 0,
                    stamina_cost: 1,
                    add_percentage: 50,
                },
            ),
            (
                "2x".to_string(),
                MovementEnhancement {
                    name: "Extended",
                    icon: IconId::X2,
                    action_point_cost: 1,
                    stamina_cost: 0,
                    add_percentage: 100,
                },
            ),
            (
                "2.5x".to_string(),
                MovementEnhancement {
                    name: "Extended + sprint",
                    icon: IconId::X3,
                    action_point_cost: 1,
                    stamina_cost: 1,
                    add_percentage: 150,
                },
            ),
            (
                "3x".to_string(),
                MovementEnhancement {
                    name: "Extended + 2 sprint",
                    icon: IconId::X3,
                    action_point_cost: 1,
                    stamina_cost: 2,
                    add_percentage: 200,
                },
            ),
        ];
        enhancements.retain(|(_, enhancement)| {
            self.action_points.current() >= MOVE_ACTION_COST + enhancement.action_point_cost
                && self.stamina.current() >= enhancement.stamina_cost
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
        let ap = self.action_points.current();
        ap >= reaction.action_point_cost
            && (ap - reaction.action_point_cost)
                >= (MAX_ACTION_POINTS - self.max_reactive_action_points)
            && self.stamina.current() >= reaction.stamina_cost
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
            if self.conditions.borrow().raging {
                // Can't use this reaction while already raging
                return false;
            }
        }
        let ap = self.action_points.current();
        ap >= reaction.action_point_cost
            && (ap - reaction.action_point_cost)
                >= (MAX_ACTION_POINTS - self.max_reactive_action_points)
            && (!reaction.must_be_melee || is_within_melee)
    }

    pub fn can_use_spell_enhancement(&self, spell: Spell, enhancement: SpellEnhancement) -> bool {
        //let enhancement = spell.possible_enhancements[enhancement_index].unwrap();
        self.mana.current() >= spell.mana_cost + enhancement.mana_cost
    }

    fn strength(&self) -> u32 {
        (self.base_attributes.strength as i32 - self.conditions.borrow().weakened as i32).max(1)
            as u32
    }

    fn agility(&self) -> u32 {
        (self.base_attributes.agility as i32 - self.conditions.borrow().weakened as i32).max(1)
            as u32
    }

    fn intellect(&self) -> u32 {
        (self.base_attributes.intellect as i32 - self.conditions.borrow().weakened as i32).max(1)
            as u32
    }

    fn spirit(&self) -> u32 {
        (self.base_attributes.spirit as i32 - self.conditions.borrow().weakened as i32).max(1)
            as u32
    }

    pub fn spell_modifier(&self) -> u32 {
        self.intellect() + self.spirit()
    }

    fn is_dazed(&self) -> bool {
        self.conditions.borrow().dazed > 0
    }

    pub fn evasion(&self) -> u32 {
        let from_agi = self.evasion_from_agility();
        let from_int = self.evasion_from_intellect();
        let from_shield = self
            .off_hand
            .shield
            .map(|shield| shield.evasion)
            .unwrap_or(0);
        let from_braced = if self.conditions.borrow().braced {
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
        10 + self.intellect() * 2
    }

    pub fn toughness(&self) -> u32 {
        10 + self.strength() * 2
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
            HandType::MainHand => self.conditions.borrow().mainhand_exertion,
            HandType::OffHand => self.conditions.borrow().offhand_exertion,
        }
    }

    fn attack_roll_bonus(&self, hand_type: HandType) -> DiceRollBonus {
        let flat_amount = -(self.hand_exertion(hand_type) as i32);

        let mut advantage = 0i32;
        if self.is_dazed() {
            advantage -= 1;
        }
        if self.conditions.borrow().raging {
            advantage += 1;
        }

        DiceRollBonus {
            advantage,
            flat_amount,
        }
    }

    pub fn explain_attack_bonus(
        &self,
        hand_type: HandType,
        enhancements: &[AttackEnhancement],
    ) -> Vec<(String, Goodness)> {
        let mut terms = vec![];
        for enhancement in enhancements {
            if enhancement.bonus_advantage > 0 {
                terms.push((enhancement.name.to_string(), Goodness::Good));
            }
        }
        if self.hand_exertion(hand_type) > 0 {
            terms.push(("Exerted".to_string(), Goodness::Bad));
        }
        if self.is_dazed() {
            terms.push(("Dazed".to_string(), Goodness::Bad));
        }
        if self.conditions.borrow().raging {
            terms.push(("Raging".to_string(), Goodness::Good));
        }
        if self.conditions.borrow().weakened > 0 {
            terms.push(("Weakened".to_string(), Goodness::Bad));
        }
        terms
    }

    fn incoming_attack_advantage(&self) -> i32 {
        0
    }

    pub fn explain_incoming_attack_circumstances(
        &self,
        reaction: Option<OnAttackedReaction>,
    ) -> Vec<(String, Goodness)> {
        let mut terms = vec![];
        if self.is_dazed() {
            terms.push(("Dazed".to_string(), Goodness::Good));
        }
        if self.conditions.borrow().weakened > 0 {
            terms.push(("Weakened".to_string(), Goodness::Good));
        }
        if self.conditions.borrow().braced {
            terms.push(("Braced".to_string(), Goodness::Bad));
        }

        if let Some(reaction) = reaction {
            match reaction.effect {
                OnAttackedReactionEffect::Parry => terms.push(("Parry".to_string(), Goodness::Bad)),
                OnAttackedReactionEffect::SideStep => {
                    terms.push(("Sidestep".to_string(), Goodness::Bad))
                }
            }
        }

        terms
    }

    fn receive_condition(&self, condition: Condition) {
        let mut conditions = self.conditions.borrow_mut();
        match condition {
            Condition::Dazed(n) => conditions.dazed += n,
            Condition::Bleeding => conditions.bleeding += 1,
            Condition::Braced => conditions.braced = true,
            Condition::Raging => conditions.raging = true,
            Condition::Weakened(n) => conditions.weakened += n,
            Condition::MainHandExertion(n) => conditions.mainhand_exertion += n,
            Condition::OffHandExertion(n) => conditions.offhand_exertion += n,
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Goodness {
    Good,
    Neutral,
    Bad,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ActionReach {
    Yes,
    YesButDisadvantage(&'static str),
    No,
}

fn within_range_squared(range_squared: f32, source: (u32, u32), destination: (u32, u32)) -> bool {
    let distance_squared = (destination.0 as i32 - source.0 as i32).pow(2)
        + (destination.1 as i32 - source.1 as i32).pow(2);
    distance_squared as f32 <= range_squared
}

fn within_meele(source: (u32, u32), destination: (u32, u32)) -> bool {
    within_range_squared(2.0, source, destination)
}

pub fn distance_between(source: (i32, i32), destination: (i32, i32)) -> f32 {
    (((destination.0 - source.0).pow(2) + (destination.1 - source.1).pow(2)) as f32).sqrt()
}

#[derive(Debug, Clone)]
pub struct NumberedResource {
    current: Cell<u32>,
    pub max: u32,
}

impl NumberedResource {
    fn new(max: u32) -> Self {
        Self {
            current: Cell::new(max),
            max,
        }
    }

    pub fn current(&self) -> u32 {
        self.current.get()
    }

    fn lose(&self, amount: u32) {
        self.current.set(self.current.get().saturating_sub(amount));
    }

    fn spend(&self, amount: u32) {
        self.current.set(self.current.get() - amount);
    }

    fn gain(&self, amount: u32) -> u32 {
        let prev = self.current.get();
        let new = (prev + amount).min(self.max);
        self.current.set(new);
        new - prev
    }

    fn set_to_max(&self) {
        self.current.set(self.max);
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ArmorPiece {
    pub name: &'static str,
    pub protection: u32,
    pub limit_evasion_from_agi: Option<u32>,
    pub icon: EquipmentIconId,
    pub weight: u32,
}

#[derive(Debug, Copy, Clone)]
pub struct Weapon {
    pub name: &'static str,
    pub sprite: Option<SpriteId>,
    pub icon: EquipmentIconId,
    pub range: WeaponRange,
    pub action_point_cost: u32,
    pub damage: u32,
    pub grip: WeaponGrip,
    pub attack_attribute: AttackAttribute,
    pub attack_enhancement: Option<AttackEnhancement>,
    pub on_attacked_reaction: Option<OnAttackedReaction>,
    pub on_true_hit: Option<AttackHitEffect>,
    pub weight: u32,
}

#[derive(Debug, Copy, Clone)]
pub struct Shield {
    pub name: &'static str,
    pub sprite: Option<SpriteId>,
    pub evasion: u32,
    pub on_hit_reaction: Option<OnHitReaction>,
    pub weight: u32,
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
pub enum WeaponRange {
    Melee,
    Ranged(u32),
}

impl Display for WeaponRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Melee => f.write_str("melee"),
            Self::Ranged(range) => f.write_fmt(format_args!("{} ({})", range, range * 2)),
        }
    }
}

impl WeaponRange {
    pub fn squared(&self) -> f32 {
        match self {
            Self::Melee => 2.0,
            Self::Ranged(range) => range.pow(2) as f32,
        }
    }

    pub fn extended(&self) -> Option<f32> {
        match self {
            Self::Ranged(range) => Some((*range as f32) * 1.5),
            _ => None,
        }
    }

    pub fn into_range(self) -> Range {
        match self {
            Self::Melee => Range::Melee,
            Self::Ranged(r) => Range::Ranged(r),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Range {
    Melee,
    Ranged(u32),
    ExtendableRanged(u32),
    Float(f32),
}

impl Display for Range {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Range::Melee => f.write_str("melee"),
            Range::Ranged(range) => f.write_str(&range.to_string()),
            Range::ExtendableRanged(range) => {
                f.write_fmt(format_args!("{} ({})", range, range * 2))
            }
            Range::Float(range) => f.write_fmt(format_args!("{:.2}", range)),
        }
    }
}

impl Range {
    pub fn squared(&self) -> f32 {
        match self {
            Range::Melee => 2.0,
            Range::Ranged(range) => range.pow(2) as f32,
            Range::ExtendableRanged(range) => range.pow(2) as f32,
            Range::Float(range) => range.powf(2.0),
        }
    }
}

impl From<Range> for f32 {
    fn from(range: Range) -> Self {
        match range {
            Range::Melee => 2f32.sqrt(),
            Range::Ranged(r) => r as f32,
            Range::ExtendableRanged(r) => r as f32,
            Range::Float(f) => f,
        }
    }
}
