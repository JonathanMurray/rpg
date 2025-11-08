use std::cell::{Cell, RefCell};

use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::rc::{Rc, Weak};

use macroquad::color::Color;

use crate::bot::BotBehaviour;
use crate::d20::{probability_of_d20_reaching, roll_d20_with_advantage, DiceRollBonus};

use crate::game_ui_connection::GameUserInterfaceConnection;
use crate::init_fight_map::GameInitState;
use crate::pathfind::PathfindGrid;
use crate::textures::{EquipmentIconId, IconId, PortraitId, SpriteId};

pub type Position = (i32, i32);

pub const MAX_ACTION_POINTS: u32 = 5;
pub const ACTION_POINTS_PER_TURN: u32 = 4;

#[derive(Debug)]
enum ActionOutcome {
    AttackHit { victim_id: CharacterId, damage: u32 },
    AbilityHitEnemies { victim_ids: Vec<CharacterId> },
    Default,
}

pub struct CoreGame {
    pub characters: Characters,
    pub active_character_id: CharacterId,
    user_interface: GameUserInterfaceConnection,
    pub pathfind_grid: Rc<PathfindGrid>,
}

impl CoreGame {
    pub fn new(user_interface: GameUserInterfaceConnection, init_state: &GameInitState) -> Self {
        Self {
            characters: init_state.characters.clone(),
            active_character_id: init_state.active_character_id,
            user_interface,
            pathfind_grid: init_state.pathfind_grid.clone(),
        }
    }

    pub async fn run(mut self) -> Vec<Character> {
        for character in self.characters.iter() {
            character.update_encumbrance();
            character.action_points.current.set(ACTION_POINTS_PER_TURN);
        }

        loop {
            println!(
                "UI SELECT ACTION ... (active char = {})",
                self.active_character().name
            );

            let enemies_remaining = self
                .characters
                .iter()
                .any(|character| !character.player_controlled());

            if !enemies_remaining {
                println!("No enemies remaining. Exiting game loop");

                // If the active character is player controlled, it's important that it runs end-of-turn
                // to let debuffs decay.
                self.perform_end_of_turn_character().await;

                for character in self.characters.iter() {
                    character.stamina.set_to_max();
                }

                return self.characters.player_characters();
            }

            let action = self.ui_select_action().await;

            if let Some(action) = action {
                let mut killed_by_action = HashSet::new();
                let ap_before_action = self.active_character().action_points.current();
                let action_outcome = self.perform_action(action).await;

                // You recover from 1 stack of Dazed for each AP you spend
                // This must happen before "on attacked and hit" reactions because those might
                // inflict new Dazed stacks, which should not be covered here.
                let spent = ap_before_action - self.active_character().action_points.current();
                self.perform_recover_from_dazed(self.active_character_id, spent)
                    .await;

                if let ActionOutcome::AttackHit { victim_id, damage } = action_outcome {
                    // TODO this can remove a Dazed that was just added from the attack, which is bad.

                    // You recover from 1 stack of Dazed each time you're hit by an attack
                    self.perform_recover_from_dazed(victim_id, 1).await;

                    let victim = self.characters.get(victim_id);
                    if victim.is_dead() {
                        killed_by_action.insert(victim.id());
                    }

                    let character = self.active_character();
                    let is_within_melee =
                        within_meele(character.position.get(), victim.position.get());
                    let can_react = !victim.is_dead()
                        && !victim.usable_on_hit_reactions(is_within_melee).is_empty();
                    if can_react {
                        if let Some(reaction) = self
                            .user_interface
                            .choose_hit_reaction(
                                &self,
                                self.active_character_id,
                                victim_id,
                                damage,
                                is_within_melee,
                            )
                            .await
                        {
                            self.perform_on_hit_reaction(victim_id, reaction).await;
                        }
                    }
                } else if let ActionOutcome::AbilityHitEnemies { victim_ids } = action_outcome {
                    for victim_id in victim_ids {
                        let victim = self.characters.get(victim_id);
                        if victim.is_dead() {
                            killed_by_action.insert(victim.id());
                        }
                    }
                }

                if !killed_by_action.is_empty() {
                    let character = self.active_character();
                    if let Some((sta, ap)) =
                        character.maybe_gain_resources_from_reaper(killed_by_action.len() as u32)
                    {
                        if sta + ap > 0 {
                            let gain = match (sta, ap) {
                                (0, _) => format!("{ap} AP"),
                                (_, 0) => format!("{sta} stamina"),
                                _ => format!("{sta} stamina, {ap} AP"),
                            };
                            self.log(format!("{} gained {} (Reaper)", character.name, gain))
                                .await;
                        }
                    }
                }

                if self.active_character().action_points.current() == 0 {
                    self.perform_end_of_turn_character().await;
                    self.active_character_id = self.characters.next_id(self.active_character_id);
                    self.notify_ui_of_new_turn().await;
                }
            } else {
                let name = self.active_character().name;
                self.log(format!("{} ended their turn", name)).await;
                self.perform_end_of_turn_character().await;
                self.active_character_id = self.characters.next_id(self.active_character_id);
                self.notify_ui_of_new_turn().await;
            }

            // We must make sure to have a valid (alive, existing) active_character_id before handing over control
            // to the UI, as it may ask us about the active character.
            let active_character_died = self.active_character().is_dead();
            if active_character_died {
                println!("ACTIVE CHAR DIED");
                dbg!(self.active_character_id);
                self.active_character_id = self.characters.next_id(self.active_character_id);
                dbg!(self.active_character_id);
            }

            for ch in self.characters.iter() {
                if ch.is_dead() {
                    self.pathfind_grid.set_blocked(ch.pos(), false);
                }
            }
            let dead_character_ids = self.characters.remove_dead();
            for dead_id in &dead_character_ids {
                for ch in self.characters.iter() {
                    ch.set_not_engaged_by(*dead_id);
                    ch.set_not_engaging(*dead_id);
                }

                let new_active = if active_character_died {
                    Some(self.active_character_id)
                } else {
                    None
                };
                self.ui_handle_event(GameEvent::CharacterDied {
                    character: *dead_id,
                    new_active,
                })
                .await;
            }

            // ... but at the same time, we don't want to lie to the UI and claim that the new turn started
            // before the character died.
            if active_character_died {
                self.notify_ui_of_new_turn().await;
            }
        }
    }

    async fn notify_ui_of_new_turn(&self) {
        self.ui_handle_event(GameEvent::NewTurn {
            new_active: self.active_character_id,
        })
        .await;
    }

    pub fn active_character(&self) -> &Character {
        self.characters.get(self.active_character_id)
    }

    pub fn enemies(&self) -> impl Iterator<Item = &Rc<Character>> {
        self.characters.iter().filter(|ch| !ch.player_controlled())
    }

    pub fn player_characters(&self) -> impl Iterator<Item = &Rc<Character>> {
        self.characters.iter().filter(|ch| ch.player_controlled())
    }

    pub fn is_players_turn(&self) -> bool {
        self.active_character().player_controlled()
    }

    pub fn player_positions(&self) -> Vec<Position> {
        let mut positions = vec![];
        for character in self.characters.iter() {
            if character.player_controlled() {
                positions.push(character.pos());
            }
        }
        positions
    }

    async fn perform_action(&mut self, action: Action) -> ActionOutcome {
        match action {
            Action::Attack {
                hand,
                enhancements,
                target,
            } => {
                //let attacker = self.active_character();
                let attacker = self.characters.get_rc(self.active_character_id);
                let defender = self.characters.get(target);

                assert!(attacker.attack_reach(hand, defender.position.get()).1 != ActionReach::No);

                let mut action_point_cost = attacker.weapon(hand).unwrap().action_point_cost as i32;

                for enhancement in &enhancements {
                    action_point_cost += enhancement.action_point_cost as i32;
                    action_point_cost -= enhancement.effect.action_point_discount as i32;
                    attacker.stamina.spend(enhancement.stamina_cost);
                    attacker.mana.spend(enhancement.mana_cost);
                }

                attacker.action_points.spend(action_point_cost as u32);

                let is_within_melee =
                    within_meele(attacker.position.get(), defender.position.get());

                // Opportunity attack vs ranged attacker
                if !is_within_melee {
                    for other_char in self.characters.iter() {
                        let unfriendly =
                            other_char.player_controlled() != attacker.player_controlled();
                        if unfriendly
                            && within_meele(attacker.pos(), other_char.pos())
                            && target != other_char.id()
                            && other_char.can_use_opportunity_attack()
                            && other_char.is_engaging(attacker.id())
                        {
                            let reactor = other_char;
                            let chooses_to_use_opportunity_attack = self
                                .user_interface
                                .choose_ranged_opportunity_attack(
                                    self,
                                    reactor.id(),
                                    attacker.id(),
                                    target,
                                )
                                .await;

                            dbg!(chooses_to_use_opportunity_attack);

                            if chooses_to_use_opportunity_attack {
                                self.ui_handle_event(
                                    GameEvent::CharacterReactedWithOpportunityAttack {
                                        reactor: reactor.id(),
                                    },
                                )
                                .await;

                                reactor.action_points.spend(1);

                                let event = self.perform_attack(
                                    reactor.id(),
                                    HandType::MainHand,
                                    vec![],
                                    attacker.id(),
                                    None,
                                    0,
                                );
                                self.ui_handle_event(GameEvent::Attacked(event)).await;
                            }
                        }
                    }
                }

                if attacker.is_dead() {
                    ActionOutcome::Default
                } else {
                    // TODO: Should not be able to react when flanked?
                    let defender_can_react_to_attack = !defender
                        .usable_on_attacked_reactions(is_within_melee)
                        .is_empty();

                    let reaction = if defender_can_react_to_attack {
                        self.user_interface
                            .choose_attack_reaction(
                                self,
                                self.active_character_id,
                                hand,
                                target,
                                is_within_melee,
                            )
                            .await
                    } else {
                        None
                    };

                    if reaction.is_some() {
                        self.ui_handle_event(GameEvent::CharacterReactedToAttacked {
                            reactor: defender.id(),
                        })
                        .await;
                    }

                    if attacker.weapon(hand).unwrap().is_melee() {
                        if let Some(previously_engaged) = attacker.engagement_target.take() {
                            self.characters
                                .get(previously_engaged)
                                .set_not_engaged_by(attacker.id());
                        }
                        defender.set_engaged_by(Rc::clone(attacker));
                        attacker.engagement_target.set(Some(defender.id()));
                    }

                    let enhancements = enhancements.iter().map(|e| (e.name, e.effect)).collect();

                    let event = self.perform_attack(
                        self.active_character_id,
                        hand,
                        enhancements,
                        target,
                        reaction,
                        0,
                    );
                    self.ui_handle_event(GameEvent::Attacked(event.clone()))
                        .await;

                    let maybe_damage = match event.outcome {
                        AttackOutcome::Hit(dmg) => Some(dmg),
                        AttackOutcome::Graze(dmg) => Some(dmg),
                        _ => None,
                    };
                    if let Some(damage) = maybe_damage {
                        ActionOutcome::AttackHit {
                            victim_id: event.target,
                            damage,
                        }
                    } else {
                        ActionOutcome::Default
                    }
                }
            }

            Action::UseAbility {
                ability,
                enhancements,
                target,
            } => {
                let enemies_hit = self.perform_ability(ability, enhancements, target).await;
                if enemies_hit.is_empty() {
                    ActionOutcome::Default
                } else {
                    ActionOutcome::AbilityHitEnemies {
                        victim_ids: enemies_hit,
                    }
                }
            }

            Action::Move {
                action_point_cost,
                stamina_cost,
                positions,
            } => {
                let character = self.active_character();
                character.action_points.spend(action_point_cost);
                character.stamina.spend(stamina_cost);
                self.perform_movement(positions).await;
                ActionOutcome::Default
            }

            Action::ChangeEquipment { from, to } => {
                let character = self.active_character();
                character.action_points.spend(1);
                character.swap_equipment_slots(from, to);
                ActionOutcome::Default
            }

            Action::UseConsumable {
                inventory_equipment_index,
            } => {
                let character = self.active_character();
                character.action_points.spend(1);
                let slot_role = EquipmentSlotRole::Inventory(inventory_equipment_index);
                let consumable = match character.equipment(slot_role).unwrap() {
                    EquipmentEntry::Consumable(consumable) => consumable,
                    unexpected => unreachable!("Not consumable: {:?}", unexpected),
                };

                if consumable.health_gain > 0 {
                    self.perform_gain_health(character, consumable.health_gain);
                }
                if consumable.mana_gain > 0 {
                    character.mana.gain(consumable.mana_gain);
                }

                character.set_equipment(None, slot_role);

                self.ui_handle_event(GameEvent::ConsumableWasUsed {
                    user: character.id(),
                    consumable,
                })
                .await;

                ActionOutcome::Default
            }
        }
    }

    async fn ui_handle_event(&self, event: GameEvent) {
        self.user_interface.handle_event(self, event).await;
    }

    async fn ui_select_action(&self) -> Option<Action> {
        self.user_interface.select_action(self).await
    }

    async fn perform_movement(&self, mut positions: Vec<Position>) {
        while !positions.is_empty() {
            let character = self.active_character();
            let new_position = positions.remove(0);
            if new_position == character.pos() {
                panic!(
                    "Character {} tried to move 0 distance from {:?}",
                    character.id(),
                    character.pos()
                );
            }

            for other_char in self.characters.iter() {
                let unfriendly = other_char.player_controlled() != character.player_controlled();
                let leaving_melee = within_meele(character.pos(), other_char.pos())
                    && !within_meele(new_position, other_char.pos());

                if unfriendly && leaving_melee {
                    if other_char.can_use_opportunity_attack()
                        && other_char.is_engaging(character.id())
                    {
                        let reactor = other_char;

                        let chooses_to_use_opportunity_attack = self
                            .user_interface
                            .choose_movement_opportunity_attack(
                                self,
                                reactor.id(),
                                character.id(),
                                (character.pos(), new_position),
                            )
                            .await;

                        dbg!(chooses_to_use_opportunity_attack);

                        if chooses_to_use_opportunity_attack {
                            self.ui_handle_event(
                                GameEvent::CharacterReactedWithOpportunityAttack {
                                    reactor: reactor.id(),
                                },
                            )
                            .await;

                            reactor.action_points.spend(1);

                            let event = self.perform_attack(
                                reactor.id(),
                                HandType::MainHand,
                                vec![],
                                character.id(),
                                None,
                                0,
                            );
                            self.ui_handle_event(GameEvent::Attacked(event)).await;
                        }
                    }

                    character.set_not_engaging(other_char.id());
                    character.set_not_engaged_by(other_char.id());
                    other_char.set_not_engaging(character.id());
                    other_char.set_not_engaged_by(character.id());
                }
            }

            // TODO don't perform movement if the actor died from opportunity attack!

            let prev_position = character.position.get();
            let id = character.id();

            self.pathfind_grid.set_blocked(prev_position, false);
            self.pathfind_grid.set_blocked(new_position, true);

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
            ApplyEffect::GainStamina(n) => {
                let amount_gained = receiver.stamina.gain(n);
                format!("  {} gained {} stamina", receiver.name, amount_gained)
            }
            ApplyEffect::Condition(condition) => {
                self.perform_receive_condition(condition, receiver)
            }
        }
    }

    fn perform_receive_condition(&self, mut condition: Condition, receiver: &Character) -> String {
        receiver.receive_condition(condition);
        let mut line = format!("  {} received {}", receiver.name, condition.name());
        if let Some(stacks) = condition.stacks() {
            line.push_str(&format!(" ({})", stacks));
        }
        line
    }

    async fn perform_ability(
        &mut self,
        ability: Ability,
        enhancements: Vec<AbilityEnhancement>,
        selected_target: ActionTarget,
    ) -> Vec<CharacterId> {
        let caster = self.active_character();
        let caster_id = caster.id();

        caster.action_points.spend(ability.action_point_cost);
        caster.mana.spend(ability.mana_cost);
        caster.stamina.spend(ability.stamina_cost);

        let mut enemies_hit = vec![];

        for enhancement in &enhancements {
            caster.action_points.spend(enhancement.action_point_cost);
            caster.mana.spend(enhancement.mana_cost);
            caster.stamina.spend(enhancement.stamina_cost);
        }

        let mut cast_n_times = 1;
        for enhancement in &enhancements {
            if let Some(e) = enhancement.spell_effect {
                if e.cast_twice {
                    cast_n_times = 2;
                }
            }
        }

        for i in 0..cast_n_times {
            let caster_ref = self.characters.get(caster_id);

            let mut detail_lines = vec![];

            let mut advantange_level = 0_i32;

            for enhancement in &enhancements {
                if let Some(e) = enhancement.spell_effect {
                    let bonus = e.bonus_advantage;
                    if bonus > 0 {
                        advantange_level += bonus as i32;
                    }
                }
            }

            let roll = roll_d20_with_advantage(advantange_level);

            if let Some(description) = roll_description(advantange_level) {
                detail_lines.push(description);
            }

            let mut line = format!("Rolled: {}", roll);
            let mut roll_calculation = roll as i32;
            match ability.modifier {
                AbilityModifier::Spell => {
                    let modifier = caster_ref.spell_modifier() as i32;
                    roll_calculation += modifier;
                    line.push_str(&format!(" (+{} spell mod)", modifier));
                }
                AbilityModifier::Attack(bonus) => {
                    let modifier = caster_ref.attack_modifier(HandType::MainHand) as i32;
                    roll_calculation += modifier + bonus;
                    let bonus_str = if bonus < 0 {
                        format!(" -{}", -bonus)
                    } else if bonus > 0 {
                        format!(" +{}", bonus)
                    } else {
                        "".to_string()
                    };
                    line.push_str(&format!(" (+{} attack mod{})", modifier, bonus_str,));
                }
            };

            for enhancement in &enhancements {
                if let Some(e) = enhancement.spell_effect {
                    let bonus = e.roll_bonus;
                    if bonus > 0 {
                        roll_calculation += bonus as i32;
                        line.push_str(&format!(" +{} ({})", bonus, enhancement.name,));
                    }
                }
            }

            let ability_result = roll_calculation as u32;
            line.push_str(&format!(" = {}", ability_result));

            let mut target_outcome = None;
            let mut area_outcomes = None;

            match ability.target {
                AbilityTarget::Enemy {
                    effect,
                    impact_area,
                    ..
                } => {
                    let ActionTarget::Character(target_id, movement) = &selected_target else {
                        unreachable!()
                    };

                    if let Some(positions) = movement {
                        self.perform_movement(positions.clone()).await;
                    }

                    let target = self.characters.get(*target_id);
                    assert!(caster.reaches_with_ability(
                        ability,
                        &enhancements,
                        target.position.get()
                    ));

                    match effect {
                        AbilityEnemyEffect::Spell(spell_enemy_effect) => {
                            if let Some(contest) = spell_enemy_effect.defense_type {
                                match contest {
                                    DefenseType::Will => {
                                        line.push_str(&format!(", vs will={}", target.will()))
                                    }
                                    DefenseType::Evasion => {
                                        line.push_str(&format!(", vs evasion={}", target.evasion()))
                                    }
                                    DefenseType::Toughness => line.push_str(&format!(
                                        ", vs toughness={}",
                                        target.toughness()
                                    )),
                                }
                            }
                        }
                        AbilityEnemyEffect::Attack => {
                            // The relevant details will come from perform_attack, not from here.
                        }
                    }

                    detail_lines.push(line);

                    let outcome = self
                        .perform_ability_enemy_effect(
                            caster_id,
                            ability.name,
                            ability.modifier,
                            &enhancements,
                            effect,
                            ability_result,
                            target,
                            &mut detail_lines,
                            true,
                        )
                        .await;
                    target_outcome = Some((*target_id, outcome));

                    if let Some((radius, area_effect)) = impact_area {
                        detail_lines.push("Area of effect:".to_string());

                        let area_target_outcomes = self
                            .perform_ability_area_enemy_effect(
                                radius,
                                "AoE",
                                ability.modifier,
                                &enhancements,
                                caster,
                                target.position.get(),
                                &mut detail_lines,
                                ability_result,
                                area_effect,
                            )
                            .await;

                        area_outcomes = Some((target.position.get(), area_target_outcomes));
                    }
                }

                AbilityTarget::Ally { range: _, effect } => {
                    let ActionTarget::Character(target_id, movement) = &selected_target else {
                        unreachable!()
                    };
                    let target = self.characters.get(*target_id);
                    assert!(caster.reaches_with_ability(
                        ability,
                        &enhancements,
                        target.position.get()
                    ));

                    detail_lines.push(line);

                    let degree_of_success = ability_result / 10;
                    if degree_of_success > 0 {
                        detail_lines.push(format!("Fortune: {}", degree_of_success));
                    }
                    let outcome = self.perform_ability_ally_effect(
                        ability.name,
                        &enhancements,
                        effect,
                        target,
                        &mut detail_lines,
                        degree_of_success,
                    );

                    target_outcome = Some((*target_id, outcome));
                }

                AbilityTarget::Area {
                    range: _,
                    radius,
                    effect,
                } => {
                    let ActionTarget::Position(target_pos) = selected_target else {
                        unreachable!()
                    };
                    assert!(caster.reaches_with_ability(ability, &enhancements, target_pos));

                    detail_lines.push(line);

                    let outcomes = self
                        .perform_ability_area_effect(
                            ability.name,
                            ability.modifier,
                            &enhancements,
                            caster,
                            target_pos,
                            radius,
                            &mut detail_lines,
                            ability_result,
                            effect,
                        )
                        .await;

                    area_outcomes = Some((target_pos, outcomes));
                }

                AbilityTarget::None {
                    self_area,
                    self_effect,
                } => {
                    detail_lines.push(line);

                    if let Some(effect) = self_effect {
                        let degree_of_success = ability_result / 10;
                        if degree_of_success > 0 {
                            detail_lines.push(format!("Fortune: {}", degree_of_success));
                        }
                        let outcome = self.perform_ability_ally_effect(
                            ability.name,
                            &enhancements,
                            effect,
                            caster,
                            &mut detail_lines,
                            degree_of_success,
                        );
                        target_outcome = Some((caster_id, outcome));
                    }

                    if let Some((radius, effect)) = self_area {
                        dbg!("SELF AREA ", radius);

                        let outcomes = self
                            .perform_ability_area_effect(
                                ability.name,
                                ability.modifier,
                                &enhancements,
                                caster,
                                caster.position.get(),
                                radius,
                                &mut detail_lines,
                                ability_result,
                                effect,
                            )
                            .await;
                        area_outcomes = Some((caster.position.get(), outcomes));
                    }
                }
            };

            if i < cast_n_times - 1 {
                detail_lines.push(format!("{} cast again!", caster_ref.name))
            }

            if let Some((target_id, outcome)) = &target_outcome {
                if matches!(outcome, AbilityTargetOutcome::HitEnemy { .. }) {
                    enemies_hit.push(*target_id);
                }
            }
            if let Some((_, outcomes)) = &area_outcomes {
                for (target_id, outcome) in outcomes {
                    if matches!(outcome, AbilityTargetOutcome::HitEnemy { .. }) {
                        enemies_hit.push(*target_id);
                    }
                }
            }

            let caster_id = caster_ref.id();
            self.ui_handle_event(GameEvent::AbilityWasUsed {
                actor: caster_id,
                target_outcome,
                area_outcomes,
                ability,
                detail_lines,
            })
            .await;
        }

        enemies_hit
    }

    async fn perform_ability_area_effect(
        &self,
        name: &'static str,
        modifier: AbilityModifier,
        enhancements: &[AbilityEnhancement],
        caster: &Character,
        area_center: Position,
        radius: Range,
        detail_lines: &mut Vec<String>,
        ability_result: u32,
        effect: AbilityEffect,
    ) -> Vec<(u32, AbilityTargetOutcome)> {
        match effect {
            AbilityEffect::Enemy(enemy_area) => {
                self.perform_ability_area_enemy_effect(
                    radius,
                    name,
                    modifier,
                    enhancements,
                    caster,
                    area_center,
                    detail_lines,
                    ability_result,
                    enemy_area,
                )
                .await
            }

            AbilityEffect::Ally(ally_area) => self.perform_ability_area_ally_effect(
                radius,
                name,
                enhancements,
                caster,
                area_center,
                detail_lines,
                ability_result,
                ally_area,
            ),
        }
    }

    fn perform_ability_area_ally_effect(
        &self,
        mut radius: Range,
        name: &'static str,
        enhancements: &[AbilityEnhancement],
        caster: &Character,
        area_center: Position,
        detail_lines: &mut Vec<String>,
        ability_result: u32,
        ally_area: AbilityAllyEffect,
    ) -> Vec<(u32, AbilityTargetOutcome)> {
        let mut target_outcomes = vec![];

        for enhancement in enhancements {
            let e = enhancement.spell_effect.unwrap();
            if e.increased_radius_tenths > 0 {
                radius = radius.plusf(e.increased_radius_tenths as f32 * 0.1);
            }
        }

        let degree_of_success = ability_result / 10;
        if degree_of_success > 0 {
            detail_lines.push(format!("Fortune: {}", degree_of_success));
        }

        for other_char in self.characters.iter() {
            if other_char.player_controlled() != caster.player_controlled() {
                continue;
            }

            if within_range_squared(radius.squared(), area_center, other_char.position.get()) {
                detail_lines.push(other_char.name.to_string());

                let outcome = self.perform_ability_ally_effect(
                    name,
                    enhancements,
                    ally_area,
                    other_char,
                    detail_lines,
                    degree_of_success,
                );

                target_outcomes.push((other_char.id(), outcome));
            }
        }

        target_outcomes
    }

    fn perform_ability_ally_effect(
        &self,
        name: &'static str,
        enhancements: &[AbilityEnhancement],
        ally_effect: AbilityAllyEffect,
        target: &Character,
        detail_lines: &mut Vec<String>,
        degree_of_success: u32,
    ) -> AbilityTargetOutcome {
        let mut health_gained = 0;

        if ally_effect.healing > 0 {
            let mut healing = ally_effect.healing;

            let mut line = format!("  Healing: {} ({})", ally_effect.healing, name);

            if degree_of_success > 0 {
                line.push_str(&format!(" +{} (fortune)", degree_of_success));
                healing += degree_of_success;
            }
            line.push_str(&format!(" = {}", healing));
            detail_lines.push(line);

            health_gained = self.perform_gain_health(target, healing);
            detail_lines.push(format!(
                "  {} was healed for {}",
                target.name, health_gained
            ));
        };

        if let Some(mut effect) = ally_effect.apply {
            match effect {
                ApplyEffect::RemoveActionPoints(ref mut n) => *n += degree_of_success,
                ApplyEffect::GainStamina(ref mut n) => *n += degree_of_success,
                ApplyEffect::Condition(ref mut condition) => {
                    if let Some(stacks) = condition.stacks() {
                        *stacks += degree_of_success;
                    }
                }
            }

            dbg!(effect);

            let log_line = self.perform_effect_application(effect, target);
            detail_lines.push(log_line);
        }

        for enhancement in enhancements {
            let effect = enhancement.spell_effect.unwrap();
            if let Some(apply_effect) = effect.on_hit {
                let log_line = self.perform_effect_application(apply_effect, target);
                detail_lines.push(format!("{} ({})", log_line, enhancement.name));
            }
        }

        AbilityTargetOutcome::HealedAlly(health_gained)
    }

    async fn perform_ability_area_enemy_effect(
        &self,
        mut radius: Range,
        name: &'static str,
        modifier: AbilityModifier,
        enhancements: &[AbilityEnhancement],
        caster: &Character,
        area_center: Position,
        detail_lines: &mut Vec<String>,
        ability_result: u32,
        enemy_area: AbilityEnemyEffect,
    ) -> Vec<(u32, AbilityTargetOutcome)> {
        let mut target_outcomes = vec![];

        for enhancement in enhancements {
            if let Some(e) = enhancement.spell_effect {
                if e.increased_radius_tenths > 0 {
                    radius = radius.plusf(e.increased_radius_tenths as f32 * 0.1);
                }
            }
        }

        for other_char in self.characters.iter() {
            dbg!(other_char.name);
            if other_char.player_controlled() == caster.player_controlled() {
                continue;
            }

            if within_range_squared(radius.squared(), area_center, other_char.position.get()) {
                let mut line = other_char.name.to_string();
                match enemy_area {
                    AbilityEnemyEffect::Spell(spell_enemy_effect) => {
                        if let Some(contest) = spell_enemy_effect.defense_type {
                            match contest {
                                DefenseType::Will => {
                                    line.push_str(&format!(" will={}", other_char.will()))
                                }
                                DefenseType::Evasion => {
                                    line.push_str(&format!(" evasion={}", other_char.evasion()))
                                }
                                DefenseType::Toughness => {
                                    line.push_str(&format!(" toughness={}", other_char.toughness()))
                                }
                            }
                        }
                    }
                    AbilityEnemyEffect::Attack => {
                        // The relevant details will come from perform_attack, not from here.
                    }
                }

                detail_lines.push(line);

                let outcome = self
                    .perform_ability_enemy_effect(
                        caster.id(),
                        name,
                        modifier,
                        enhancements,
                        enemy_area,
                        ability_result,
                        other_char,
                        detail_lines,
                        false,
                    )
                    .await;

                target_outcomes.push((other_char.id(), outcome));
            }
        }

        target_outcomes
    }

    async fn perform_ability_enemy_effect(
        &self,
        caster_id: CharacterId,
        ability_name: &'static str,
        modifier: AbilityModifier,
        enhancements: &[AbilityEnhancement],
        enemy_effect: AbilityEnemyEffect,
        ability_result: u32,
        target: &Character,
        detail_lines: &mut Vec<String>,
        is_direct_target: bool,
    ) -> AbilityTargetOutcome {
        match enemy_effect {
            AbilityEnemyEffect::Spell(spell_enemy_effect) => self.perform_spell_enemy_effect(
                ability_name,
                modifier,
                enhancements,
                spell_enemy_effect,
                ability_result,
                target,
                detail_lines,
                is_direct_target,
            ),
            AbilityEnemyEffect::Attack => {
                let attack_enhancement_effects = enhancements
                    .iter()
                    .filter_map(|e| e.attack_enhancement_effect())
                    .collect();

                let reaction = None;
                let AbilityModifier::Attack(roll_modifier) = modifier else {
                    unreachable!()
                };
                let event = self.perform_attack(
                    caster_id,
                    HandType::MainHand,
                    attack_enhancement_effects,
                    target.id(),
                    reaction,
                    roll_modifier,
                );

                AbilityTargetOutcome::AttackedEnemy(event)
            }
        }
    }

    fn perform_spell_enemy_effect(
        &self,
        ability_name: &'static str,
        modifier: AbilityModifier,
        enhancements: &[AbilityEnhancement],
        spell_enemy_effect: SpellEnemyEffect,
        ability_result: u32,
        target: &Character,
        detail_lines: &mut Vec<String>,
        is_direct_target: bool,
    ) -> AbilityTargetOutcome {
        let success = match spell_enemy_effect.defense_type {
            Some(contest) => {
                let defense = match contest {
                    DefenseType::Will => target.will(),
                    DefenseType::Evasion => target.evasion(),
                    DefenseType::Toughness => target.toughness(),
                };

                if ability_result >= defense {
                    Some(((ability_result - defense) / 5) as i32)
                } else if ability_result >= defense - 5 {
                    // graze
                    Some(-1)
                } else {
                    None
                }
            }
            None => Some(0),
        };

        if let Some(degree_of_success) = success {
            let success_label = match degree_of_success {
                -1 => {
                    detail_lines.push("  Graze".to_string());
                    "Graze".to_string()
                }
                0 => "".to_string(),
                1 => {
                    detail_lines.push("  Heavy hit".to_string());
                    "Heavy hit".to_string()
                }
                n => {
                    detail_lines.push(format!("  Heavy hit ({})", n));
                    "Heavy hit".to_string()
                }
            };

            let damage = if let Some(ability_damage) = spell_enemy_effect.damage {
                let mut dmg_calculation;
                let mut increased_by_good_roll = true;
                let mut dmg_str = "  Damage: ".to_string();

                match ability_damage {
                    AbilityDamage::Static(n) => {
                        dmg_calculation = n as i32;
                        increased_by_good_roll = false;

                        dmg_str.push_str(&format!("{} ({})", dmg_calculation, ability_name));
                    }
                    AbilityDamage::AtLeast(n) => {
                        dmg_calculation = n as i32;
                        dmg_str.push_str(&format!("{} ({})", dmg_calculation, ability_name));
                    }
                };

                if increased_by_good_roll && degree_of_success > 0 {
                    dmg_str.push_str(&format!(" +{degree_of_success} ({success_label})"));
                    dmg_calculation += degree_of_success;
                }

                for enhancement in enhancements {
                    let e = enhancement.spell_effect.unwrap();
                    let bonus_dmg = if is_direct_target {
                        e.bonus_target_damage
                    } else {
                        e.bonus_area_damage
                    };
                    if bonus_dmg > 0 {
                        dmg_str.push_str(&format!(" +{} ({})", bonus_dmg, enhancement.name));
                        dmg_calculation += bonus_dmg as i32;
                    }
                }

                let graze = degree_of_success == -1;

                if graze {
                    dmg_str.push_str(" -50% (Graze)");
                    // Since there's no armor/protection against spells, rounding up would make the spell too powerful.
                    dmg_calculation /= 2;
                }

                let damage = dmg_calculation.max(0) as u32;

                if dmg_calculation > 0 {
                    self.perform_losing_health(target, damage);
                    dmg_str.push_str(&format!(" = {damage}"));
                    detail_lines.push(dmg_str);
                }
                Some(damage)
            } else {
                None
            };

            let mut applied_effects = vec![];

            fn apply_degree_of_success(stacks: &mut u32, degree_of_success: i32) {
                if degree_of_success == -1 {
                    *stacks /= 2;
                } else {
                    assert!(degree_of_success >= 0);
                    *stacks += degree_of_success as u32;
                }
            }

            for mut effect in spell_enemy_effect
                .on_hit
                .unwrap_or_default()
                .iter()
                .copied()
                .flatten()
            {
                match effect {
                    ApplyEffect::RemoveActionPoints(ref mut n) => {
                        apply_degree_of_success(n, degree_of_success)
                    }
                    ApplyEffect::GainStamina(ref mut n) => {
                        apply_degree_of_success(n, degree_of_success)
                    }
                    ApplyEffect::Condition(ref mut condition) => {
                        if let Some(stacks) = condition.stacks() {
                            apply_degree_of_success(stacks, degree_of_success)
                        }
                    }
                }

                applied_effects.push(effect);
                let log_line = self.perform_effect_application(effect, target);
                detail_lines.push(log_line);
            }

            for enhancement in enhancements {
                // TODO: shouldn't these also be affected by degree of success?
                let e = enhancement.spell_effect.unwrap();
                if let Some(effect) = e.on_hit {
                    applied_effects.push(effect);
                    let log_line = self.perform_effect_application(effect, target);
                    detail_lines.push(format!("{} ({})", log_line, enhancement.name));
                }
            }

            assert!(applied_effects.len() <= 2);
            let applied_effects = [
                applied_effects.first().copied(),
                applied_effects.get(1).copied(),
            ];

            AbilityTargetOutcome::HitEnemy {
                damage,
                graze: degree_of_success == -1,
                applied_effects,
            }
        } else {
            let line = match (modifier, spell_enemy_effect.defense_type) {
                (_, None) => unreachable!("uncontested effect cannot fail"),
                (AbilityModifier::Spell, Some(DefenseType::Will | DefenseType::Toughness)) => {
                    format!("  {} resisted the spell", target.name)
                }
                (AbilityModifier::Spell, Some(DefenseType::Evasion)) => {
                    format!("  The spell missed {}", target.name)
                }
                (AbilityModifier::Attack(_), Some(_)) => {
                    format!("  The ability missed {}", target.name)
                }
            };
            detail_lines.push(line);
            AbilityTargetOutcome::Resist
        }
    }

    fn perform_losing_health(&self, character: &Character, amount: u32) -> u32 {
        let amount_lost = character.health.lose(amount);
        self.on_character_health_changed(character);
        amount_lost
    }

    fn perform_gain_health(&self, character: &Character, amount: u32) -> u32 {
        let amount_gained = character.health.gain(amount);
        self.on_character_health_changed(character);
        amount_gained
    }

    fn on_character_health_changed(&self, character: &Character) {
        character.conditions.borrow_mut().near_death =
            (character.health.current() as f32) < character.health.max() as f32 / 4.0;

        if character.health.current() == 0 {
            character.conditions.borrow_mut().near_death = false;
            character.conditions.borrow_mut().dead = true;
        }
    }

    async fn log(&self, line: impl Into<String>) {
        self.ui_handle_event(GameEvent::LogLine(line.into())).await;
    }

    fn perform_attack(
        &self,
        attacker_id: CharacterId,
        hand_type: HandType,
        enhancements: Vec<(&'static str, AttackEnhancementEffect)>,
        defender_id: CharacterId,
        defender_reaction: Option<OnAttackedReaction>,
        ability_roll_modifier: i32,
    ) -> AttackedEvent {
        let attacker = self.characters.get(attacker_id);
        let defender = self.characters.get(defender_id);

        let mut attack_bonus = attack_roll_bonus(
            attacker,
            hand_type,
            defender,
            &enhancements,
            defender_reaction,
        );
        attack_bonus.flat_amount += ability_roll_modifier;

        let mut evasion = defender.evasion();

        let mut evasion_added_by_parry = 0;
        let mut evasion_added_by_sidestep = 0;
        let mut skip_attack_exertion = false;

        let attack_modifier = attacker.attack_modifier(hand_type);

        let mut detail_lines = vec![];

        if let Some(reaction) = defender_reaction {
            defender.action_points.spend(reaction.action_point_cost);
            defender.stamina.spend(reaction.stamina_cost);

            detail_lines.push(format!("{} reacted with {}", defender.name, reaction.name));

            if reaction.effect.bonus_evasion > 0 {
                let bonus_evasion = reaction.effect.bonus_evasion;

                detail_lines.push(format!(
                    "  Evasion: {} +{} ({}) = {}",
                    evasion,
                    bonus_evasion,
                    reaction.name,
                    evasion + bonus_evasion
                ));
                evasion += bonus_evasion;
                let p_hit = probability_of_d20_reaching(evasion - attack_modifier, attack_bonus);
                detail_lines.push(format!("  Chance to hit: {:.1}%", p_hit * 100f32));
            }

            match reaction.id {
                OnAttackedReactionId::Parry => {
                    evasion_added_by_parry = reaction.effect.bonus_evasion;
                }
                OnAttackedReactionId::SideStep => {
                    evasion_added_by_sidestep = reaction.effect.bonus_evasion;
                }
            }
        }

        let roll = roll_d20_with_advantage(attack_bonus.advantage);
        let attack_result = ((roll + attack_modifier) as i32 + attack_bonus.flat_amount) as u32;

        if let Some(description) = roll_description(attack_bonus.advantage) {
            detail_lines.push(description);
        }

        let mut armor_penetrators = vec![];
        for (name, effect) in &enhancements {
            let penetration = effect.armor_penetration;
            if penetration > 0 {
                armor_penetrators.push((penetration, *name));
            }
        }
        if attacker
            .known_passive_skills
            .contains(&PassiveSkill::WeaponProficiency)
        {
            armor_penetrators.push((1, PassiveSkill::WeaponProficiency.name()));
        }
        let mut armor_value = defender.protection_from_armor();
        let mut armor_str = armor_value.to_string();
        for (penetration, label) in armor_penetrators {
            armor_value = armor_value.saturating_sub(penetration);
            armor_str.push_str(&format!(" -{} ({})", penetration, label));
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
            armor_str
        ));

        let outcome = if attack_result >= evasion.saturating_sub(5) {
            let mut on_true_hit_effect = None;
            let weapon = attacker.weapon(hand_type).unwrap();
            let mut dmg_calculation = weapon.damage as i32;

            let mut dmg_str = format!("  Damage: {} ({})", dmg_calculation, weapon.name);

            if matches!(weapon.grip, WeaponGrip::Versatile) && attacker.off_hand.get().is_empty() {
                let bonus_dmg = 1;
                dmg_str.push_str(&format!(" +{} (two-handed)", bonus_dmg));
                dmg_calculation += bonus_dmg;
            }

            for (name, effect) in &enhancements {
                let bonus_dmg = effect.bonus_damage;
                if bonus_dmg > 0 {
                    dmg_str.push_str(&format!(" +{} ({})", bonus_dmg, name));
                    dmg_calculation += bonus_dmg as i32;
                }
            }

            let armored_defense = evasion + armor_value;

            if attack_result < evasion {
                //let mut line = "  Graze! (50% damage".to_string();
                let line = "  Graze!".to_string();
                dmg_str.push_str(" -50% (graze)");
                dmg_calculation = (dmg_calculation as f32 * 0.5).ceil() as i32;

                if armor_value > 0 {
                    //line.push_str(&format!(", {} mitigated", armor_value));
                    dmg_str.push_str(&format!(" -{armor_value} (armor)"));
                    dmg_calculation -= armor_value as i32;
                }

                //line.push_str(")");
                detail_lines.push(line);
            } else if attack_result < armored_defense {
                let mitigated = armored_defense - attack_result;

                detail_lines.push(format!("  Mitigated hit ({} armor)", mitigated));
                dmg_str.push_str(&format!(" -{mitigated} (armor)"));
                dmg_calculation -= mitigated as i32;
            } else {
                on_true_hit_effect = weapon.on_true_hit;

                let degree_of_success = (attack_result - armored_defense) / 5;

                if degree_of_success > 1 {
                    detail_lines.push(format!("  Heavy hit ({})", degree_of_success));
                    dmg_str.push_str(&format!(" +{degree_of_success} (Heavy hit)"));
                    dmg_calculation += degree_of_success as i32;
                } else if degree_of_success == 1 {
                    detail_lines.push("  Heavy hit".to_string());
                    dmg_str.push_str(" +1 (Heavy hit)");
                    dmg_calculation += 1;
                } else {
                    if armor_value > 0 {
                        detail_lines.push("  Penetrating hit".to_string());
                    } else {
                        detail_lines.push("  Neutral hit".to_string());
                    }
                }
            }

            let damage = dmg_calculation.max(0) as u32;

            dmg_str.push_str(&format!(" = {damage}"));

            detail_lines.push(dmg_str);

            self.perform_losing_health(defender, damage);

            if let Some(effect) = on_true_hit_effect {
                match effect {
                    AttackHitEffect::Apply(effect) => {
                        let log_line = self.perform_effect_application(effect, defender);
                        detail_lines.push(format!("{} (true hit)", log_line))
                    }
                    AttackHitEffect::SkipExertion => skip_attack_exertion = true,
                }
            }

            if damage > 0 {
                for (name, effect) in &enhancements {
                    if let Some(effect) = effect.on_damage_effect {
                        let log_line = match effect {
                            AttackEnhancementOnHitEffect::RegainActionPoint => {
                                attacker.action_points.gain(1);
                                format!("{} regained 1 AP", attacker.name)
                            }
                            AttackEnhancementOnHitEffect::Target(apply_effect) => {
                                self.perform_effect_application(apply_effect, defender)
                            }
                        };

                        detail_lines.push(format!("{} ({})", log_line, name))
                    }

                    if let Some(mut condition) = effect.inflict_condition_per_damage {
                        *condition.stacks().unwrap() = damage;
                        let line = self.perform_receive_condition(condition, defender);
                        detail_lines.push(format!("{} ({})", line, name))
                    }
                }
            }

            if attack_result < evasion {
                AttackOutcome::Graze(damage)
            } else {
                AttackOutcome::Hit(damage)
            }
        } else if attack_result
            < evasion.saturating_sub(evasion_added_by_parry + evasion_added_by_sidestep + 5)
        {
            detail_lines.push("  Missed!".to_string());
            AttackOutcome::Miss
        } else if evasion_added_by_parry > 0 {
            detail_lines.push("  Parried!".to_string());
            AttackOutcome::Parry
        } else if evasion_added_by_sidestep > 0 {
            detail_lines.push("  Side stepped!".to_string());
            AttackOutcome::Dodge
        } else {
            unreachable!(
                "{attack_result}, {evasion}, {evasion_added_by_parry}, {evasion_added_by_sidestep}"
            );
        };

        if defender.lose_braced() {
            detail_lines.push(format!("{} lost Braced", defender.name));
        }
        if defender.lose_distracted() {
            detail_lines.push(format!("{} lost Distracted", defender.name));
        }

        for (name, effect) in &enhancements {
            if let Some(effect) = effect.on_target {
                let log_line = self.perform_effect_application(effect, defender);
                detail_lines.push(format!("{} ({})", log_line, name));
            }
        }

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

        AttackedEvent {
            attacker: attacker_id,
            target: defender_id,
            outcome,
            detail_lines,
        }
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
        reactor.stamina.spend(reaction.stamina_cost);
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
                            attacker,
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
        let conditions = &character.conditions;

        let bleed_stacks = conditions.borrow().bleeding;
        if bleed_stacks > 0 {
            let decay = (bleed_stacks / 2).max(1);
            let damage = self.perform_losing_health(character, decay);
            self.log(format!("{} took {} damage from Bleeding", name, damage))
                .await;
            conditions.borrow_mut().bleeding -= decay;
            if conditions.borrow().bleeding == 0 {
                self.log(format!("{} stopped Bleeding", name)).await;
            }
        }
        if conditions.borrow().weakened > 0 {
            conditions.borrow_mut().weakened = 0;
            self.log(format!("{} is no longer Weakened", name)).await;
        }
        if conditions.borrow().raging {
            conditions.borrow_mut().raging = false;
            self.log(format!("{} stopped Raging", name)).await;
        }
        if conditions.borrow().exposed > 0 {
            conditions.borrow_mut().exposed -= 1;
            if conditions.borrow().exposed == 0 {
                self.log(format!("{} is no longer Exposed", name)).await;
            }
        }
        if conditions.borrow().hindered > 0 {
            conditions.borrow_mut().hindered -= 1;
            if conditions.borrow().hindered == 0 {
                self.log(format!("{} is no longer Hindered", name)).await;
            }
        }

        if conditions.borrow().slowed > 0 {
            conditions.borrow_mut().slowed -= 1;
            if conditions.borrow().slowed == 0 {
                self.log(format!("{} is no longer Slowed", name)).await;
            }
        }

        //let mut new_ap = MAX_ACTION_POINTS;
        let mut gain_ap = ACTION_POINTS_PER_TURN;
        if conditions.borrow().near_death {
            gain_ap = gain_ap.saturating_sub(1);
        }
        if conditions.borrow().slowed > 0 {
            gain_ap = gain_ap.saturating_sub(SLOWED_AP_PENALTY);
        }
        character.action_points.gain(gain_ap);
        //character.action_points.current.set(new_ap);

        conditions.borrow_mut().mainhand_exertion = 0;
        conditions.borrow_mut().offhand_exertion = 0;
        conditions.borrow_mut().reaper_ap_cooldown = false;
        let stamina_gain = (character.stamina.max() as f32 / 3.0).ceil() as u32;
        character.stamina.gain(stamina_gain);
    }
}

fn roll_description(advantage: i32) -> Option<String> {
    match advantage.cmp(&0) {
        std::cmp::Ordering::Less => Some(format!(
            "Rolled {} dice with disadvantage...",
            advantage.abs() + 1
        )),
        std::cmp::Ordering::Equal => None,
        std::cmp::Ordering::Greater => {
            Some(format!("Rolled {} dice with advantage...", advantage + 1))
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
        from: Position,
        to: Position,
    },
    CharacterReactedToAttacked {
        reactor: CharacterId,
    },
    CharacterReactedToHit {
        main_line: String,
        detail_lines: Vec<String>,
        reactor: CharacterId,
        outcome: HitReactionOutcome,
    },
    CharacterReactedWithOpportunityAttack {
        reactor: CharacterId,
    },
    Attacked(AttackedEvent),
    AbilityWasUsed {
        actor: CharacterId,
        target_outcome: Option<(CharacterId, AbilityTargetOutcome)>,
        area_outcomes: Option<(Position, Vec<(CharacterId, AbilityTargetOutcome)>)>,
        ability: Ability,
        detail_lines: Vec<String>,
    },
    ConsumableWasUsed {
        user: CharacterId,
        consumable: Consumable,
    },
    CharacterDied {
        character: CharacterId,
        new_active: Option<CharacterId>,
    },
    NewTurn {
        new_active: CharacterId,
    },
}

#[derive(Debug, Clone)]
pub struct AttackedEvent {
    pub attacker: CharacterId,
    pub target: CharacterId,
    pub outcome: AttackOutcome,
    pub detail_lines: Vec<String>,
}

#[derive(Debug, Copy, Clone)]
pub enum AttackOutcome {
    Hit(u32),
    Graze(u32),
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

#[derive(Debug, Clone)]
pub enum AbilityTargetOutcome {
    HitEnemy {
        damage: Option<u32>,
        graze: bool,
        applied_effects: [Option<ApplyEffect>; 2],
    },
    AttackedEnemy(AttackedEvent),
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

fn ability_roll_bonus(
    caster: &Character,
    defender: &Character,
    enhancements: &[AbilityEnhancement],
    modifier: AbilityModifier,
) -> DiceRollBonus {
    let mut bonus = caster.outgoing_ability_roll_bonus(enhancements, modifier);
    bonus.advantage += defender.incoming_ability_advantage();
    bonus
}

pub fn attack_roll_bonus(
    attacker: &Character,
    hand: HandType,
    defender: &Character,
    //circumstance_advantage: i32,
    enhancements: &[(&'static str, AttackEnhancementEffect)],
    reaction: Option<OnAttackedReaction>,
) -> DiceRollBonus {
    let mut bonus = attacker.outgoing_attack_roll_bonus(hand, enhancements, defender);
    bonus.advantage += defender.incoming_attack_advantage(reaction);
    bonus
}

pub fn prob_attack_hit(
    attacker: &Character,
    hand: HandType,
    defender: &Character,
    enhancements: &[(&'static str, AttackEnhancementEffect)],
    reaction: Option<OnAttackedReaction>,
) -> f32 {
    let bonus = attack_roll_bonus(attacker, hand, defender, enhancements, reaction);
    let mut evasion = defender.evasion();

    if let Some(reaction) = reaction {
        evasion += reaction.effect.bonus_evasion;
    }

    let dice_target = evasion
        .saturating_sub(attacker.attack_modifier(hand))
        .max(1);
    probability_of_d20_reaching(dice_target, bonus)
}

pub fn prob_attack_penetrating_hit(
    attacker: &Character,
    hand: HandType,
    defender: &Character,
    enhancements: &[(&'static str, AttackEnhancementEffect)],
    reaction: Option<OnAttackedReaction>,
) -> f32 {
    let bonus = attack_roll_bonus(attacker, hand, defender, enhancements, reaction);
    let mut evasion = defender.evasion();

    if let Some(reaction) = reaction {
        evasion += reaction.effect.bonus_evasion;
    }

    let mut armor = defender.protection_from_armor();
    for (_name, effect) in enhancements {
        armor = armor.saturating_sub(effect.armor_penetration);
    }

    let armored_defense = evasion + armor;

    let dice_target = armored_defense
        .saturating_sub(attacker.attack_modifier(hand))
        .max(1);
    probability_of_d20_reaching(dice_target, bonus)
}

pub fn prob_ability_hit(
    caster: &Character,
    defense_type: DefenseType,
    defender: &Character,
    enhancements: &[AbilityEnhancement],
    modifier: AbilityModifier,
) -> f32 {
    let bonus = ability_roll_bonus(caster, defender, enhancements, modifier);

    let def = match defense_type {
        DefenseType::Will => defender.will(),
        DefenseType::Evasion => defender.evasion(),
        DefenseType::Toughness => defender.toughness(),
    };

    let modifier_value = match modifier {
        AbilityModifier::Spell => caster.spell_modifier() as i32,
        AbilityModifier::Attack(bonus) => caster.attack_modifier(HandType::MainHand) as i32 + bonus,
    };

    let target = (def as i32 - modifier_value).max(1) as u32;
    probability_of_d20_reaching(target, bonus)
}

#[derive(Clone)]
pub struct Characters(Vec<(CharacterId, Rc<Character>)>);

impl Characters {
    pub fn new(characters: Vec<Character>) -> Self {
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

            if passed_current && !ch.is_dead() {
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

    pub fn contains(&self, character_id: CharacterId) -> bool {
        self.0.iter().any(|(id, _ch)| *id == character_id)
    }

    pub fn get(&self, character_id: CharacterId) -> &Character {
        self.get_rc(character_id)
    }

    pub fn get_rc(&self, character_id: CharacterId) -> &Rc<Character> {
        let entry = self.0.iter().find(|(id, _ch)| *id == character_id);

        match entry {
            Some((_id, ch)) => ch,
            None => panic!("No character with id: {character_id}"),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Rc<Character>> {
        self.0.iter().map(|(_id, ch)| ch)
    }

    pub fn player_characters(self) -> Vec<Character> {
        self.iter()
            .filter(|ch| ch.player_controlled())
            .map(|ch| Character::clone(ch))
            .collect()
    }

    pub fn iter_with_ids(&self) -> impl Iterator<Item = &(CharacterId, Rc<Character>)> {
        self.0.iter()
    }

    pub fn remove_dead(&mut self) -> Vec<CharacterId> {
        let mut removed = vec![];
        self.0.retain(|(_id, ch)| {
            if ch.is_dead() {
                removed.push(ch.id());
                false
            } else {
                true
            }
        });
        removed
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum ApplyEffect {
    RemoveActionPoints(u32),
    Condition(Condition),
    GainStamina(u32),
}

impl Display for ApplyEffect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApplyEffect::RemoveActionPoints(n) => f.write_fmt(format_args!("-{n} AP")),
            ApplyEffect::GainStamina(n) => f.write_fmt(format_args!("+{n} stamina")),
            ApplyEffect::Condition(condition) => f.write_fmt(format_args!("{}", condition.name())),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct OnAttackedReaction {
    pub id: OnAttackedReactionId,
    pub name: &'static str,
    pub description: &'static str,
    pub icon: IconId,
    pub action_point_cost: u32,
    pub stamina_cost: u32,
    pub effect: OnAttackedReactionEffect,
    pub must_be_melee: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum OnAttackedReactionId {
    Parry,
    SideStep,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct OnAttackedReactionEffect {
    pub bonus_evasion: u32,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct OnHitReaction {
    pub name: &'static str,
    pub description: &'static str,
    pub icon: IconId,
    pub action_point_cost: u32,
    pub stamina_cost: u32,
    pub effect: OnHitReactionEffect,
    pub must_be_melee: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum OnHitReactionEffect {
    Rage,
    ShieldBash,
}

#[derive(Debug, Copy, Clone, PartialEq)]
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
                ApplyEffect::GainStamina(n) => {
                    f.write_fmt(format_args!("Target gains {n} stamina"))
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
    Protected(u32),
    Dazed(u32),
    Bleeding(u32),
    Braced,
    Raging,
    Distracted,
    Weakened(u32),
    MainHandExertion(u32),
    OffHandExertion(u32),
    Encumbered(u32),
    NearDeath,
    Dead,
    Slowed(u32),
    Exposed(u32),
    Hindered(u32),
    ReaperApCooldown,
}

impl Condition {
    pub const fn stacks(&mut self) -> Option<&mut u32> {
        use Condition::*;
        match self {
            Protected(n) => Some(n),
            Dazed(n) => Some(n),
            Bleeding(n) => Some(n),
            Braced => None,
            Raging => None,
            Distracted => None,
            Weakened(n) => Some(n),
            MainHandExertion(n) => Some(n),
            OffHandExertion(n) => Some(n),
            Encumbered(n) => Some(n),
            NearDeath => None,
            Dead => None,
            Slowed(n) => Some(n),
            Exposed(n) => Some(n),
            Hindered(n) => Some(n),
            ReaperApCooldown => None,
        }
    }

    pub const fn name(&self) -> &'static str {
        use Condition::*;
        match self {
            Protected(_) => "Protected",
            Dazed(_) => "Dazed",
            Bleeding(_) => "Bleeding",
            Braced => "Braced",
            Raging => "Raging",
            Distracted => "Distracted",
            Weakened(_) => "Weakened",
            MainHandExertion(_) => "Exerted (main-hand)",
            OffHandExertion(_) => "Exerted (off-hand)",
            Encumbered(_) => "Encumbered",
            NearDeath => "Near-death",
            Dead => "Dead",
            Slowed(..) => "Slowed",
            Exposed(..) => "Exposed",
            Hindered(..) => "Hindered",
            ReaperApCooldown => "Reaper",
        }
    }

    pub const fn description(&self) -> &'static str {
        use Condition::*;
        match self {
            Protected(_) => "Gains +3 armor",
            Dazed(_) => "Gains no evasion from agility and attacks with disadvantage",
            Bleeding(_) => "End of turn: 50% stacks decay, 1 damage for each decayed",
            Braced => "Gain +3 evasion against the next incoming attack",
            Raging => "Gains advantage on melee attack rolls until end of turn",
            Distracted => "-6 evasion against the next incoming attack",
            Weakened(_) => "-x to all defenses and dice rolls",
            MainHandExertion(_) => "-x on further similar actions",
            OffHandExertion(_) => "-x on further similar actions",
            Encumbered(_) => "-x to Evasion and -x/2 to dice rolls",
            NearDeath => "< 25% HP: Reduced AP, disadvantage on everything",
            Dead => "This character has reached 0 HP and is dead",
            Slowed(_) => "Gains 2 less AP per turn",
            Exposed(_) => "-3 to all defenses",
            Hindered(..) => "Half movement speed",
            ReaperApCooldown => "Can not gain more AP from Reaper this turn",
        }
    }

    pub const fn is_positive(&self) -> bool {
        use Condition::*;
        match self {
            Protected(_) => true,
            Dazed(_) => false,
            Bleeding(_) => false,
            Braced => true,
            Raging => true,
            Distracted => false,
            Weakened(_) => false,
            MainHandExertion(_) => false,
            OffHandExertion(_) => false,
            Encumbered(_) => false,
            NearDeath => false,
            Dead => false,
            Slowed(_) => false,
            Exposed(_) => false,
            Hindered(..) => false,
            ReaperApCooldown => false,
        }
    }

    pub const fn info(&mut self) -> (ConditionInfo, Option<u32>) {
        (
            ConditionInfo {
                name: self.name(),
                description: self.description(),
                is_positive: self.is_positive(),
            },
            self.stacks().copied(),
        )
    }
}

const BLEEDING_DAMAGE_AMOUNT: u32 = 1;
const PROTECTED_ARMOR_BONUS: u32 = 3;
const BRACED_DEFENSE_BONUS: u32 = 3;
const DISTRACTED_DEFENSE_PENALTY: u32 = 6;
const EXPOSED_DEFENSE_PENALTY: u32 = 3;
const SLOWED_AP_PENALTY: u32 = 2;

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct ConditionInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub is_positive: bool,
}

#[derive(Debug, Copy, Clone, Default)]
struct Conditions {
    protected: u32,
    dazed: u32,
    bleeding: u32,
    braced: bool,
    raging: bool,
    distracted: bool,
    weakened: u32,
    mainhand_exertion: u32,
    offhand_exertion: u32,
    encumbered: u32,
    near_death: bool,
    dead: bool,
    slowed: u32,
    exposed: u32,
    hindered: u32,
    reaper_ap_cooldown: bool,
}

impl Conditions {
    pub fn infos(&mut self) -> Vec<(ConditionInfo, Option<u32>)> {
        let mut result = vec![];
        if self.protected > 0 {
            result.push(Condition::Protected(self.protected).info());
        }
        if self.dazed > 0 {
            result.push(Condition::Dazed(self.dazed).info());
        }
        if self.bleeding > 0 {
            result.push(Condition::Bleeding(self.bleeding).info());
        }
        if self.braced {
            result.push(Condition::Braced.info());
        }
        if self.raging {
            result.push(Condition::Raging.info());
        }
        if self.distracted {
            result.push(Condition::Distracted.info());
        }
        if self.weakened > 0 {
            result.push(Condition::Weakened(self.weakened).info());
        }
        if self.mainhand_exertion > 0 {
            result.push(Condition::MainHandExertion(self.mainhand_exertion).info());
        }
        if self.offhand_exertion > 0 {
            result.push(Condition::OffHandExertion(self.offhand_exertion).info());
        }
        if self.encumbered > 0 {
            result.push(Condition::Encumbered(self.encumbered).info());
        }
        if self.near_death {
            result.push(Condition::NearDeath.info());
        }
        if self.dead {
            result.push(Condition::Dead.info());
        }
        if self.slowed > 0 {
            result.push(Condition::Slowed(self.slowed).info())
        }
        if self.exposed > 0 {
            result.push(Condition::Exposed(self.exposed).info())
        }
        if self.hindered > 0 {
            result.push(Condition::Hindered(self.hindered).info())
        }
        if self.reaper_ap_cooldown {
            result.push(Condition::ReaperApCooldown.info())
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
    UseAbility {
        ability: Ability,
        enhancements: Vec<AbilityEnhancement>,
        target: ActionTarget,
    },
    Move {
        positions: Vec<Position>,
        action_point_cost: u32,
        stamina_cost: u32,
    },
    ChangeEquipment {
        from: EquipmentSlotRole,
        to: EquipmentSlotRole,
    },
    UseConsumable {
        inventory_equipment_index: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub enum ActionTarget {
    Character(CharacterId, Option<Vec<Position>>),
    Position(Position),
    None,
}

#[derive(Debug, Copy, Clone, PartialEq)]

pub enum BaseAction {
    Attack(AttackAction),
    UseAbility(Ability),
    Move,
    ChangeEquipment,
    UseConsumable,
    EndTurn,
    // TODO add "DelayTurn" action that lets you put yourself one step later in the
    // turn order
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct AttackAction {
    pub hand: HandType,
    pub action_point_cost: u32,
}

impl BaseAction {
    pub fn requires_equipped_melee_weapon(&self) -> bool {
        match self {
            BaseAction::UseAbility(ability) => {
                matches!(ability.weapon_requirement, Some(WeaponType::Melee))
            }
            _ => false,
        }
    }

    pub fn action_point_cost(&self) -> u32 {
        match self {
            BaseAction::Attack(attack) => attack.action_point_cost,
            BaseAction::UseAbility(ability) => ability.action_point_cost,
            BaseAction::Move => 0,
            BaseAction::ChangeEquipment => 1,
            BaseAction::UseConsumable => 1,
            BaseAction::EndTurn => 0,
        }
    }

    pub fn mana_cost(&self) -> u32 {
        match self {
            BaseAction::Attack { .. } => 0,
            BaseAction::UseAbility(ability) => ability.mana_cost,
            BaseAction::Move => 0,
            BaseAction::ChangeEquipment => 0,
            BaseAction::UseConsumable => 0,
            BaseAction::EndTurn => 0,
        }
    }

    pub fn stamina_cost(&self) -> u32 {
        match self {
            BaseAction::Attack { .. } => 0,
            BaseAction::UseAbility(ability) => ability.stamina_cost,
            BaseAction::Move => 0,
            BaseAction::ChangeEquipment => 0,
            BaseAction::UseConsumable => 0,
            BaseAction::EndTurn => 0,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum HandType {
    MainHand,
    OffHand,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Ability {
    pub id: AbilityId,
    pub name: &'static str,
    pub description: &'static str,
    pub icon: IconId,
    pub action_point_cost: u32,
    pub mana_cost: u32,
    pub stamina_cost: u32,
    pub weapon_requirement: Option<WeaponType>,

    pub modifier: AbilityModifier,
    pub target: AbilityTarget,
    pub possible_enhancements: [Option<AbilityEnhancement>; 3],
    pub animation_color: Color,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum AbilityId {
    SweepAttack,
    LungeAttack,
    Brace,
    Scream,
    ShackledMind,
    MindBlast,
    Heal,
    HealingNova,
    SelfHeal,
    HealingRain,
    Fireballl,
    Kill,

    MagiHeal,
    MagiInflictWounds,
    MagiInflictHorrors,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum WeaponType {
    Melee,
    Ranged,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum AbilityModifier {
    Spell,
    Attack(i32),
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum AbilityEffect {
    Enemy(AbilityEnemyEffect),
    Ally(AbilityAllyEffect),
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct SpellEnemyEffect {
    pub defense_type: Option<DefenseType>,
    pub damage: Option<AbilityDamage>,
    pub on_hit: Option<[Option<ApplyEffect>; 2]>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum AbilityEnemyEffect {
    Spell(SpellEnemyEffect),
    Attack,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum AbilityDamage {
    Static(u32),
    AtLeast(u32),
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct AbilityAllyEffect {
    pub healing: u32,
    pub apply: Option<ApplyEffect>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum AbilityTarget {
    Enemy {
        reach: AbilityReach,
        effect: AbilityEnemyEffect,
        impact_area: Option<(Range, AbilityEnemyEffect)>,
    },

    Ally {
        range: Range,
        effect: AbilityAllyEffect,
    },

    Area {
        range: Range,
        radius: Range,
        effect: AbilityEffect,
    },

    None {
        self_area: Option<(Range, AbilityEffect)>,
        self_effect: Option<AbilityAllyEffect>,
    },
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum AbilityReach {
    Range(Range),
    MoveIntoMelee(Range),
}

impl AbilityTarget {
    pub fn single_target(&self) -> bool {
        match self {
            AbilityTarget::Enemy { .. } => true,
            AbilityTarget::Ally { .. } => true,
            AbilityTarget::Area { .. } => false,
            AbilityTarget::None { .. } => false,
        }
    }

    fn base_range(&self) -> Option<Range> {
        match self {
            AbilityTarget::Enemy { reach, .. } => match reach {
                AbilityReach::Range(range) => Some(*range),
                AbilityReach::MoveIntoMelee(range) => Some(*range),
            },
            AbilityTarget::Ally { range, .. } => Some(*range),
            AbilityTarget::Area { range, .. } => Some(*range),
            AbilityTarget::None { self_area, .. } => {
                // TODO This is actually radius, not range; is this misused somewhere (with enahcenements for example)
                self_area.as_ref().map(|(radius, _effect)| *radius)
            }
        }
    }

    pub fn range(&self, enhancements: &[AbilityEnhancement]) -> Option<Range> {
        self.base_range().map(|mut range| {
            for enhancement in enhancements {
                if let Some(e) = enhancement.spell_effect {
                    if e.increased_range_tenths > 0 {
                        range = range.plusf(e.increased_range_tenths as f32 * 0.1);
                    }
                }
            }
            range
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PassiveSkill {
    HardenedSkin,
    WeaponProficiency,
    ArcaneSurge,
    Reaper,
}

impl PassiveSkill {
    pub fn name(&self) -> &'static str {
        match self {
            Self::HardenedSkin => "Hardened skin",
            Self::WeaponProficiency => "Weapon proficiency",
            Self::ArcaneSurge => "Arcane surge",
            Self::Reaper => "Reaper",
        }
    }

    pub fn icon(&self) -> IconId {
        match self {
            Self::HardenedSkin => IconId::HardenedSkin,
            Self::WeaponProficiency => IconId::WeaponProficiency,
            Self::ArcaneSurge => IconId::ArcaneSurge,
            Self::Reaper => IconId::Reaper,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::HardenedSkin => "+1 armor",
            Self::WeaponProficiency => "Attacks gain +1 armor penetration",
            Self::ArcaneSurge => "+3 spell modifier while at/below 50% mana",
            Self::Reaper => "On kill: gain 1 stamina, 1 AP (max 1 AP per turn)",
        }
    }
}

// TODO Merge SpellEnhancement and AttackEnhancement? (There may be AttackEnhancements that should also be
// usable for attack abilities (like Lunge attack / Sweeping attack))

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct AttackEnhancement {
    pub name: &'static str,
    pub description: &'static str,
    pub icon: IconId,
    pub action_point_cost: u32,
    pub mana_cost: u32,
    pub stamina_cost: u32,
    pub weapon_requirement: Option<WeaponType>,
    pub effect: AttackEnhancementEffect,
}

impl AttackEnhancement {
    // the version from the Default trait is not const
    pub const fn default() -> Self {
        Self {
            name: "<placeholder>",
            description: "",
            icon: IconId::Equip,
            action_point_cost: 0,
            stamina_cost: 0,
            mana_cost: 0,
            weapon_requirement: None,
            effect: AttackEnhancementEffect::default(),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct AbilityEnhancement {
    pub ability_id: AbilityId,
    pub name: &'static str,
    pub description: &'static str,
    pub icon: IconId,
    pub action_point_cost: u32,
    pub mana_cost: u32,
    pub stamina_cost: u32,

    pub spell_effect: Option<SpellEnhancementEffect>,
    pub attack_effect: Option<AttackEnhancementEffect>,
}

impl AbilityEnhancement {
    pub fn attack_enhancement_effect(&self) -> Option<(&'static str, AttackEnhancementEffect)> {
        self.attack_effect
            .map(|attack_effect| (self.name, attack_effect))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct AttackEnhancementEffect {
    pub roll_modifier: i32,
    pub bonus_advantage: u32,
    pub bonus_damage: u32,
    pub action_point_discount: u32,
    pub inflict_condition_per_damage: Option<Condition>,
    pub armor_penetration: u32,
    // TODO Actually handle this
    pub on_self: Option<ApplyEffect>,

    // Gets activated if the attack deals damage
    pub on_damage_effect: Option<AttackEnhancementOnHitEffect>,

    // Gets applied on the target regardless if the attack hits
    pub on_target: Option<ApplyEffect>,
}

impl AttackEnhancementEffect {
    // the impl from #[derive(Default)] is not const
    pub const fn default() -> Self {
        Self {
            action_point_discount: 0,
            bonus_damage: 0,
            bonus_advantage: 0,
            on_damage_effect: None,
            roll_modifier: 0,
            inflict_condition_per_damage: None,
            armor_penetration: 0,
            on_self: None,
            on_target: None,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum AbilityEnhancementEffect {
    Spell(SpellEnhancementEffect),
    Attack(AttackEnhancementEffect),
}

impl AbilityEnhancementEffect {
    fn unwrap_spell_enhancement_effect(&self) -> &SpellEnhancementEffect {
        match self {
            AbilityEnhancementEffect::Spell(e) => e,
            AbilityEnhancementEffect::Attack(..) => panic!("{:?}", self),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct SpellEnhancementEffect {
    pub roll_bonus: u32,
    pub bonus_advantage: u32,
    pub bonus_target_damage: u32,
    pub bonus_area_damage: u32,
    pub cast_twice: bool,
    pub on_hit: Option<ApplyEffect>,
    pub increased_range_tenths: u32,
    pub increased_radius_tenths: u32,

    // Used by abilities that perform attacks
    //TODO remove this, use enum variant instead
    pub attack_enhancement_effect: Option<AttackEnhancementEffect>,
}

impl SpellEnhancementEffect {
    // the impl from #[derive(Default)] is not const
    pub const fn default() -> Self {
        Self {
            roll_bonus: 0,
            bonus_advantage: 0,
            bonus_target_damage: 0,
            bonus_area_damage: 0,
            cast_twice: false,
            on_hit: None,
            increased_range_tenths: 0,
            increased_radius_tenths: 0,
            attack_enhancement_effect: None,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum AttackEnhancementOnHitEffect {
    RegainActionPoint,
    Target(ApplyEffect),
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum DefenseType {
    Will,
    Evasion,
    Toughness,
}

#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct Hand {
    weapon: Option<Weapon>,
    shield: Option<Shield>,
}

impl Hand {
    fn is_empty(&self) -> bool {
        self.weapon.is_none() && self.shield.is_none()
    }

    fn with_weapon(weapon: Weapon) -> Self {
        Self {
            weapon: Some(weapon),
            shield: None,
        }
    }

    fn with_shield(shield: Shield) -> Self {
        Self {
            weapon: None,
            shield: Some(shield),
        }
    }
}

pub type CharacterId = u32;

#[derive(Debug, Clone)]
pub struct Attributes {
    pub strength: Cell<u32>,
    pub agility: Cell<u32>,
    pub intellect: Cell<u32>,
    pub spirit: Cell<u32>,
}

impl Attributes {
    pub fn new(str: u32, agi: u32, intel: u32, spi: u32) -> Self {
        Self {
            strength: Cell::new(str),
            agility: Cell::new(agi),
            intellect: Cell::new(intel),
            spirit: Cell::new(spi),
        }
    }

    fn move_speed(&self) -> f32 {
        0.9 + self.agility.get() as f32 * 0.1
    }

    fn max_health(&self) -> u32 {
        6 + self.strength.get()
    }

    fn max_mana(&self) -> u32 {
        (self.spirit.get() * 2).saturating_sub(3)
    }

    fn max_stamina(&self) -> u32 {
        (self.strength.get() + self.agility.get()).saturating_sub(2)
    }

    fn capacity(&self) -> u32 {
        self.strength.get() * 2
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Behaviour {
    Player,
    Bot(BotBehaviour),
}

impl Behaviour {
    pub fn unwrap_bot_behaviour(&self) -> &BotBehaviour {
        match self {
            Behaviour::Player => panic!(),
            Behaviour::Bot(bot_behaviour) => bot_behaviour,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Character {
    id: Option<CharacterId>,
    pub name: &'static str,
    pub portrait: PortraitId,

    pub sprite: SpriteId,
    pub behaviour: Behaviour,
    pub position: Cell<Position>,
    pub base_attributes: Attributes,
    pub health: NumberedResource,
    pub mana: NumberedResource,
    // How many cells you can move per AP
    pub base_move_speed: Cell<f32>,
    pub capacity: Cell<u32>,
    pub inventory: [Cell<Option<EquipmentEntry>>; 6],
    pub armor_piece: Cell<Option<ArmorPiece>>,
    main_hand: Cell<Hand>,
    off_hand: Cell<Hand>,
    conditions: RefCell<Conditions>,
    pub action_points: NumberedResource,
    pub max_reactive_action_points: u32,
    pub stamina: NumberedResource,
    pub known_attack_enhancements: Vec<AttackEnhancement>,
    pub known_actions: Vec<BaseAction>,
    pub known_attacked_reactions: Vec<OnAttackedReaction>,
    pub known_on_hit_reactions: Vec<OnHitReaction>,
    pub known_ability_enhancements: Vec<AbilityEnhancement>,
    pub known_passive_skills: Vec<PassiveSkill>,

    pub is_engaged_by: RefCell<HashMap<CharacterId, Rc<Character>>>,
    engagement_target: Cell<Option<CharacterId>>,

    changed_equipment_listeners: RefCell<Vec<Weak<Cell<bool>>>>,

    pub money: Cell<u32>,
}

impl Character {
    pub fn new(
        behaviour: Behaviour,
        name: &'static str,
        portrait: PortraitId,
        sprite: SpriteId,
        base_attributes: Attributes,
        position: Position,
    ) -> Self {
        let max_health = base_attributes.max_health();
        let max_mana = base_attributes.max_mana();

        let move_speed = base_attributes.move_speed();
        let max_stamina = base_attributes.max_stamina();
        let max_reactive_action_points = 1 + base_attributes.intellect.get() / 2;
        let capacity = base_attributes.capacity();
        let action_points = NumberedResource::new(MAX_ACTION_POINTS);
        action_points.current.set(ACTION_POINTS_PER_TURN);
        Self {
            id: None,
            portrait,
            sprite,
            behaviour,
            position: Cell::new(position),
            name,
            base_attributes,
            health: NumberedResource::new(max_health),
            mana: NumberedResource::new(max_mana),
            base_move_speed: Cell::new(move_speed),
            capacity: Cell::new(capacity),
            inventory: Default::default(),
            armor_piece: Default::default(),
            main_hand: Default::default(),
            off_hand: Default::default(),
            conditions: Default::default(),
            action_points,
            max_reactive_action_points,
            stamina: NumberedResource::new(max_stamina),
            known_attack_enhancements: Default::default(),
            known_actions: vec![
                BaseAction::Attack(AttackAction {
                    hand: HandType::MainHand,
                    action_point_cost: 2,
                }),
                //BaseAction::SelfEffect(BRACE),
                BaseAction::Move,
                BaseAction::ChangeEquipment,
                BaseAction::UseConsumable,
                BaseAction::EndTurn,
            ],
            known_attacked_reactions: Default::default(),
            known_on_hit_reactions: Default::default(),
            known_ability_enhancements: Default::default(),
            known_passive_skills: Default::default(),
            is_engaged_by: Default::default(),
            engagement_target: Default::default(),
            changed_equipment_listeners: Default::default(),
            money: Cell::new(5),
        }
    }

    fn maybe_gain_resources_from_reaper(&self, num_killed: u32) -> Option<(u32, u32)> {
        if self.known_passive_skills.contains(&PassiveSkill::Reaper) {
            let sta = self.stamina.gain(num_killed);
            let ap = if self.conditions.borrow().reaper_ap_cooldown {
                0
            } else {
                self.action_points.gain(1)
            };
            self.receive_condition(Condition::ReaperApCooldown);
            Some((sta, ap))
        } else {
            None
        }
    }

    fn set_engaged_by(&self, engager: Rc<Character>) {
        self.is_engaged_by
            .borrow_mut()
            .insert(engager.id(), engager);
    }

    fn set_not_engaged_by(&self, not_engager: CharacterId) {
        self.is_engaged_by.borrow_mut().remove(&not_engager);
    }

    fn set_not_engaging(&self, target: CharacterId) {
        if self.engagement_target.get() == Some(target) {
            self.engagement_target.set(None);
        }
    }

    fn is_engaging(&self, target: CharacterId) -> bool {
        self.engagement_target.get() == Some(target)
    }

    pub fn move_speed(&self) -> f32 {
        let mut speed = self.base_move_speed.get();
        if self.conditions.borrow().hindered > 0 {
            speed /= 2.0;
        }
        speed
    }

    pub fn player_controlled(&self) -> bool {
        matches!(self.behaviour, Behaviour::Player)
    }

    pub fn add_to_strength(&self, amount: i32) {
        let current = self.base_attributes.strength.get();
        let new = current as i32 + amount;
        assert!(new >= 0);
        self.base_attributes.strength.set(new as u32);

        self.on_attributes_changed();
    }

    pub fn add_to_agility(&self, amount: i32) {
        let current = self.base_attributes.agility.get();
        let new = current as i32 + amount;
        assert!(new >= 0);
        self.base_attributes.agility.set(new as u32);

        self.on_attributes_changed();
    }

    pub fn add_to_intellect(&self, amount: i32) {
        let current = self.base_attributes.intellect.get();
        let new = current as i32 + amount;
        assert!(new >= 0);
        self.base_attributes.intellect.set(new as u32);

        self.on_attributes_changed();
    }

    pub fn add_to_spirit(&self, amount: i32) {
        let current = self.base_attributes.spirit.get();
        let new = current as i32 + amount;
        assert!(new >= 0);
        self.base_attributes.spirit.set(new as u32);

        self.on_attributes_changed();
    }

    fn on_attributes_changed(&self) {
        let attr = &self.base_attributes;
        self.health.change_max_value_to(attr.max_health());
        self.stamina.change_max_value_to(attr.max_stamina());
        self.mana.change_max_value_to(attr.max_mana());
        self.capacity.set(attr.capacity());
        self.base_move_speed.set(attr.move_speed());
    }

    pub fn is_dead(&self) -> bool {
        self.conditions.borrow().dead
    }

    pub fn listen_to_changed_equipment(&self) -> Rc<Cell<bool>> {
        let signal = Rc::new(Cell::new(false));
        let weak = Rc::downgrade(&signal);
        self.changed_equipment_listeners.borrow_mut().push(weak);
        signal
    }

    pub fn can_equipment_fit(&self, equipment: EquipmentEntry, role: EquipmentSlotRole) -> bool {
        if matches!(role, EquipmentSlotRole::Inventory(..)) {
            return true;
        }
        match equipment {
            EquipmentEntry::Weapon(weapon) if role == EquipmentSlotRole::MainHand => {
                weapon.grip != WeaponGrip::TwoHanded || self.off_hand.get().is_empty()
            }
            EquipmentEntry::Shield(..) if role == EquipmentSlotRole::OffHand => {
                if let Some(weapon) = self.weapon(HandType::MainHand) {
                    weapon.grip != WeaponGrip::TwoHanded
                } else {
                    true
                }
            }
            EquipmentEntry::Armor(..) => role == EquipmentSlotRole::Armor,
            _ => false,
        }
    }

    pub fn gain_money(&self, amount: u32) {
        self.money.set(self.money.get() + amount);
    }

    pub fn spend_money(&self, amount: u32) {
        assert!(self.money.get() >= amount);
        self.money.set(self.money.get() - amount);
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

    fn lose_distracted(&self) -> bool {
        let mut conditions = self.conditions.borrow_mut();
        if conditions.distracted {
            conditions.distracted = false;
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
        if let Some(armor) = self.armor_piece.get() {
            sum += armor.weight;
        }
        for entry in &self.inventory {
            if let Some(entry) = entry.get() {
                sum += entry.weight()
            }
        }
        sum
    }

    pub fn condition_infos(&self) -> Vec<(ConditionInfo, Option<u32>)> {
        self.conditions.borrow_mut().infos()
    }

    pub fn pos(&self) -> Position {
        self.position.get()
    }

    pub fn id(&self) -> CharacterId {
        self.id.unwrap()
    }

    fn hand(&self, hand_type: HandType) -> &Cell<Hand> {
        match hand_type {
            HandType::MainHand => &self.main_hand,
            HandType::OffHand => &self.off_hand,
        }
    }

    pub fn weapon(&self, hand: HandType) -> Option<Weapon> {
        self.hand(hand).get().weapon
    }

    pub fn has_equipped_ranged_weapon(&self) -> bool {
        if let Some(weapon) = self.weapon(HandType::MainHand) {
            !weapon.is_melee()
        } else {
            false
        }
    }

    pub fn has_equipped_melee_weapon(&self) -> bool {
        if let Some(weapon) = self.weapon(HandType::MainHand) {
            weapon.is_melee()
        } else {
            false
        }
    }

    pub fn shield(&self) -> Option<Shield> {
        self.hand(HandType::OffHand).get().shield
    }

    pub fn equipment(&self, slot_role: EquipmentSlotRole) -> Option<EquipmentEntry> {
        match slot_role {
            EquipmentSlotRole::MainHand => {
                self.weapon(HandType::MainHand).map(EquipmentEntry::Weapon)
            }
            EquipmentSlotRole::OffHand => self.shield().map(EquipmentEntry::Shield),
            EquipmentSlotRole::Armor => self.armor_piece.get().map(EquipmentEntry::Armor),
            EquipmentSlotRole::Inventory(idx) => self.inventory[idx].get(),
        }
    }

    fn update_encumbrance(&self) {
        let encumbrance = self.equipment_weight() as i32 - self.capacity.get() as i32;
        dbg!(encumbrance);
        if encumbrance > 0 {
            self.conditions.borrow_mut().encumbered = encumbrance as u32;
        } else {
            self.conditions.borrow_mut().encumbered = 0;
        }
    }

    fn on_changed_equipment(&self) {
        self.update_encumbrance();

        self.changed_equipment_listeners
            .borrow_mut()
            .retain(|weak| match weak.upgrade() {
                Some(signal) => {
                    signal.set(true);
                    true
                }
                // No one is listening on the other end => remove the listener reference
                None => false,
            });
    }

    pub fn set_weapon(&self, hand_type: HandType, weapon: Weapon) {
        assert!(self.can_equipment_fit(
            EquipmentEntry::Weapon(weapon),
            EquipmentSlotRole::from_hand_type(hand_type)
        ));
        self.hand(hand_type).set(Hand::with_weapon(weapon));
        self.on_changed_equipment();
    }

    pub fn set_shield(&self, shield: Shield) {
        assert!(self.can_equipment_fit(EquipmentEntry::Shield(shield), EquipmentSlotRole::OffHand));
        self.off_hand.set(Hand::with_shield(shield));
        self.on_changed_equipment();
    }

    pub fn set_equipment(&self, entry: Option<EquipmentEntry>, slot_role: EquipmentSlotRole) {
        match slot_role {
            EquipmentSlotRole::MainHand => match entry {
                Some(EquipmentEntry::Weapon(weapon)) => {
                    self.set_weapon(HandType::MainHand, weapon);
                }
                None => self.main_hand.set(Hand::default()),
                _ => panic!(),
            },
            EquipmentSlotRole::OffHand => match entry {
                Some(EquipmentEntry::Shield(shield)) => {
                    self.set_shield(shield);
                }
                None => self.off_hand.set(Hand::default()),
                _ => panic!(),
            },
            EquipmentSlotRole::Armor => match entry {
                Some(EquipmentEntry::Armor(armor)) => self.armor_piece.set(Some(armor)),
                None => self.armor_piece.set(None),
                _ => panic!(),
            },
            EquipmentSlotRole::Inventory(i) => self.inventory[i].set(entry),
        }

        self.on_changed_equipment();
    }

    pub fn swap_equipment_slots(&self, from: EquipmentSlotRole, to: EquipmentSlotRole) {
        let from_content = self.equipment(from);
        let to_content = self.equipment(to);
        self.set_equipment(from_content, to);
        self.set_equipment(to_content, from);
    }

    pub fn try_gain_equipment(&self, entry: EquipmentEntry) -> bool {
        dbg!(self.name, entry);
        for slot in &self.inventory {
            if slot.get().is_none() {
                slot.set(Some(entry));
                self.on_changed_equipment();
                return true;
            }
        }

        false
    }

    pub fn attack_action_point_cost(&self, hand: HandType) -> u32 {
        self.hand(hand).get().weapon.unwrap().action_point_cost
    }

    pub fn attack_reach(&self, hand: HandType, target_position: Position) -> (Range, ActionReach) {
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
                            ActionReach::YesButDisadvantage("Far"),
                        )
                    } else {
                        (Range::Float(extended), ActionReach::No)
                    }
                }
            }
        }
    }

    pub fn reaches_with_ability(
        &self,
        ability: Ability,
        enhancements: &[AbilityEnhancement],
        target_position: Position,
    ) -> bool {
        let range = ability.target.range(enhancements).unwrap();
        within_range_squared(range.squared(), self.position.get(), target_position)
    }

    pub fn known_actions(&self) -> Vec<BaseAction> {
        self.known_actions.to_vec()
    }

    pub fn usable_attack_action(&self) -> Option<AttackAction> {
        for action in &self.known_actions {
            if self.can_use_action(*action) {
                if let BaseAction::Attack(attack_action) = action {
                    return Some(*attack_action);
                }
            }
        }
        None
    }

    pub fn attack_action(&self) -> Option<AttackAction> {
        for action in &self.known_actions {
            if let BaseAction::Attack(attack_action) = action {
                return Some(*attack_action);
            }
        }
        None
    }

    pub fn usable_actions(&self) -> Vec<BaseAction> {
        self.known_actions
            .iter()
            .filter_map(|action| {
                if self.can_use_action(*action) {
                    Some(*action)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn can_attack(&self, attack: AttackAction) -> bool {
        let ap = self.action_points.current();
        matches!(self.weapon(attack.hand), Some(weapon) if ap >= weapon.action_point_cost)
    }

    pub fn can_use_action(&self, action: BaseAction) -> bool {
        let ap = self.action_points.current();
        match action {
            BaseAction::Attack(attack) => {
                matches!(self.weapon(attack.hand), Some(weapon) if ap >= weapon.action_point_cost)
            }
            BaseAction::UseAbility(ability) => {
                if matches!(ability.weapon_requirement, Some(WeaponType::Melee))
                    && !self.has_equipped_melee_weapon()
                {
                    return false;
                }
                ap >= ability.action_point_cost
                    && self.stamina.current() >= ability.stamina_cost
                    && self.mana.current() >= ability.mana_cost
            }
            BaseAction::Move => ap > 0,
            BaseAction::ChangeEquipment => ap >= BaseAction::ChangeEquipment.action_point_cost(),
            BaseAction::UseConsumable => ap >= BaseAction::UseConsumable.action_point_cost(),
            BaseAction::EndTurn => true,
        }
    }

    pub fn known_attack_enhancements(
        &self,
        attack_hand: HandType,
    ) -> Vec<(String, AttackEnhancement)> {
        let mut known = vec![];
        if let Some(weapon) = self.weapon(attack_hand) {
            if let Some(enhancement) = weapon.attack_enhancement {
                known.push((weapon.name.to_string(), enhancement))
            }
            for enhancement in &self.known_attack_enhancements {
                known.push(("".to_owned(), *enhancement))
            }
        }
        known
    }

    pub fn usable_attack_enhancements(&self, attack_hand: HandType) -> Vec<AttackEnhancement> {
        self.known_attack_enhancements(attack_hand)
            .iter()
            .filter_map(|(_, e)| {
                if self.can_use_attack_enhancement(attack_hand, e) {
                    Some(*e)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn can_use_opportunity_attack(&self) -> bool {
        if let Some(weapon) = self.weapon(HandType::MainHand) {
            weapon.is_melee() && self.action_points.current() >= 1
        } else {
            false
        }
    }

    pub fn can_use_attack_enhancement(
        &self,
        attack_hand: HandType,
        enhancement: &AttackEnhancement,
    ) -> bool {
        let weapon = self.weapon(attack_hand).unwrap();
        let is_weapon_compatible = enhancement
            .weapon_requirement
            .map(|required_type| weapon.weapon_type() == required_type)
            .unwrap_or(true);

        is_weapon_compatible
            && self.action_points.current()
                >= weapon.action_point_cost + enhancement.action_point_cost
                    - enhancement.effect.action_point_discount
            && self.stamina.current() >= enhancement.stamina_cost
            && self.mana.current() >= enhancement.mana_cost
    }

    pub fn known_on_attacked_reactions(&self) -> Vec<(String, OnAttackedReaction)> {
        let mut known = vec![];
        for reaction in &self.known_attacked_reactions {
            known.push(("".to_string(), *reaction));
        }
        // TODO: off-hand reactions?
        if let Some(weapon) = &self.weapon(HandType::MainHand) {
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
        if let Some(shield) = self.shield() {
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
            && self.stamina.current() >= reaction.stamina_cost
            && (!reaction.must_be_melee || is_within_melee)
    }

    pub fn knows_ability(&self, id: AbilityId) -> bool {
        self.known_actions.iter().any(|action| match action {
            BaseAction::UseAbility(ability) => ability.id == id,
            _ => false,
        })
    }

    pub fn known_abilities(&self) -> Vec<Ability> {
        self.known_actions
            .iter()
            .filter_map(|action| match action {
                BaseAction::UseAbility(ability) => Some(*ability),
                _ => None,
            })
            .collect()
    }

    pub fn knows_ability_enhancement(&self, enhancement: AbilityEnhancement) -> bool {
        self.known_ability_enhancements.contains(&enhancement)
    }

    pub fn can_use_ability_enhancement(
        &self,
        ability: Ability,
        enhancement: AbilityEnhancement,
    ) -> bool {
        self.action_points.current() >= ability.action_point_cost + enhancement.action_point_cost
            && self.mana.current() >= ability.mana_cost + enhancement.mana_cost
            && self.stamina.current() >= ability.stamina_cost + enhancement.stamina_cost
    }

    fn strength(&self) -> u32 {
        (self.base_attributes.strength.get() as i32).max(1) as u32
    }

    fn agility(&self) -> u32 {
        (self.base_attributes.agility.get() as i32).max(1) as u32
    }

    fn intellect(&self) -> u32 {
        (self.base_attributes.intellect.get() as i32).max(1) as u32
    }

    fn spirit(&self) -> u32 {
        (self.base_attributes.spirit.get() as i32).max(1) as u32
    }

    pub fn spell_modifier(&self) -> u32 {
        let mut res = self.intellect() + self.spirit();

        if let Some(armor) = self.armor_piece.get() {
            res += armor.equip.bonus_spell_modifier;
        }

        if self.has_active_arcane_surge() {
            res += 3;
        }

        res
    }

    fn has_active_arcane_surge(&self) -> bool {
        self.known_passive_skills
            .contains(&PassiveSkill::ArcaneSurge)
            && self.mana.ratio() <= 0.5
    }

    fn is_dazed(&self) -> bool {
        self.conditions.borrow().dazed > 0
    }

    pub fn evasion(&self) -> u32 {
        let mut res = 10;
        res += self.evasion_from_agility();
        res += self.evasion_from_intellect();
        res += self.shield().map(|shield| shield.evasion).unwrap_or(0);

        let conditions = self.conditions.borrow();
        if conditions.braced {
            res += BRACED_DEFENSE_BONUS;
        }
        if conditions.distracted {
            res -= DISTRACTED_DEFENSE_PENALTY;
        }
        res = res.saturating_sub(conditions.encumbered);

        if conditions.exposed > 0 {
            res = res.saturating_sub(EXPOSED_DEFENSE_PENALTY);
        }

        if conditions.weakened > 0 {
            res = res.saturating_sub(conditions.weakened)
        }

        res
    }

    fn evasion_from_agility(&self) -> u32 {
        let mut bonus = if self.is_dazed() { 0 } else { self.agility() };
        if let Some(armor) = self.armor_piece.get() {
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
        let mut res = 10 + self.intellect() * 2;
        let conditions = self.conditions.borrow();
        if conditions.exposed > 0 {
            res = res.saturating_sub(EXPOSED_DEFENSE_PENALTY);
        }
        if conditions.weakened > 0 {
            res = res.saturating_sub(conditions.weakened)
        }
        res
    }

    pub fn toughness(&self) -> u32 {
        let mut res = 10 + self.strength() * 2;
        let conditions = self.conditions.borrow();
        if conditions.exposed > 0 {
            res = res.saturating_sub(EXPOSED_DEFENSE_PENALTY);
        }
        if conditions.weakened > 0 {
            res = res.saturating_sub(conditions.weakened)
        }

        res
    }

    pub fn protection_from_armor(&self) -> u32 {
        let mut protection = 0;
        if let Some(armor) = self.armor_piece.get() {
            protection += armor.protection;
        }

        if self.conditions.borrow().protected > 0 {
            protection += PROTECTED_ARMOR_BONUS;
        }

        if self
            .known_passive_skills
            .contains(&PassiveSkill::HardenedSkin)
        {
            protection += 1;
        }

        protection
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

    fn outgoing_ability_roll_bonus(
        &self,
        enhancements: &[AbilityEnhancement],
        modifier: AbilityModifier,
    ) -> DiceRollBonus {
        let mut advantage = 0i32;
        let mut flat_amount = 0;
        for (_label, bonus) in self.outgoing_ability_roll_bonuses(enhancements, modifier) {
            match bonus {
                RollBonusContributor::Advantage(n) => advantage += n,
                RollBonusContributor::FlatAmount(n) => flat_amount += n,
                RollBonusContributor::OtherNegative | RollBonusContributor::OtherPositive => {}
            }
        }

        DiceRollBonus {
            advantage,
            flat_amount,
        }
    }

    fn outgoing_attack_roll_bonus(
        &self,
        hand_type: HandType,
        enhancements: &[(&'static str, AttackEnhancementEffect)],
        target: &Character,
    ) -> DiceRollBonus {
        let mut advantage = 0i32;
        let mut flat_amount = 0;
        for (label, bonus) in self.outgoing_attack_bonuses(hand_type, enhancements, target) {
            match bonus {
                RollBonusContributor::Advantage(n) => advantage += n,
                RollBonusContributor::FlatAmount(n) => flat_amount += n,
                RollBonusContributor::OtherNegative | RollBonusContributor::OtherPositive => {}
            }
        }

        DiceRollBonus {
            advantage,
            flat_amount,
        }
    }

    pub fn outgoing_attack_bonuses(
        &self,
        hand_type: HandType,
        enhancement_effects: &[(&'static str, AttackEnhancementEffect)],
        target: &Character,
    ) -> Vec<(&'static str, RollBonusContributor)> {
        let target_pos = target.pos();
        let mut bonuses = vec![];

        let flanking = target
            .is_engaged_by
            .borrow()
            .values()
            .any(|engager| are_flanking_target(self.pos(), engager.pos(), target_pos));

        if flanking {
            bonuses.push(("Flanked", RollBonusContributor::FlatAmount(3)));
        }

        let (_range, reach) = self.attack_reach(hand_type, target_pos);

        if let ActionReach::YesButDisadvantage(reason) = reach {
            bonuses.push((reason, RollBonusContributor::Advantage(-1)));
        }

        for (name, effect) in enhancement_effects {
            if effect.bonus_advantage > 0 {
                bonuses.push((
                    name,
                    RollBonusContributor::Advantage(effect.bonus_advantage as i32),
                ));
            }

            if effect.roll_modifier != 0 {
                bonuses.push((name, RollBonusContributor::FlatAmount(effect.roll_modifier)));
            }
        }
        let exertion_penalty = self.hand_exertion(hand_type) as i32;
        if exertion_penalty > 0 {
            bonuses.push((
                "Exerted",
                RollBonusContributor::FlatAmount(-exertion_penalty),
            ));
        }
        if self.is_dazed() {
            bonuses.push(("Dazed", RollBonusContributor::Advantage(-1)));
        }
        let conditions = self.conditions.borrow();
        if conditions.raging && self.weapon(hand_type).unwrap().range == WeaponRange::Melee {
            bonuses.push(("Raging", RollBonusContributor::Advantage(1)));
        }
        if conditions.weakened > 0 {
            // TODO: this seems wrong, shouldn't the penalty be applied here? If not here, then where?
            bonuses.push(("Weakened", RollBonusContributor::OtherNegative));
        }

        let encumbrance_penalty = (conditions.encumbered / 2) as i32;
        if encumbrance_penalty > 0 {
            bonuses.push((
                "Encumbered",
                RollBonusContributor::FlatAmount(-(encumbrance_penalty)),
            ));
        }

        if conditions.near_death {
            bonuses.push(("Near-death", RollBonusContributor::Advantage(-1)));
        }

        bonuses
    }

    pub fn outgoing_ability_roll_bonuses(
        &self,
        enhancements: &[AbilityEnhancement],
        modifier: AbilityModifier,
    ) -> Vec<(&'static str, RollBonusContributor)> {
        let is_spell = matches!(modifier, AbilityModifier::Spell);
        let mut bonuses = vec![];
        for enhancement in enhancements {
            if let Some(e) = enhancement.spell_effect {
                if e.bonus_advantage > 0 {
                    bonuses.push((
                        enhancement.name,
                        RollBonusContributor::Advantage(e.bonus_advantage as i32),
                    ));
                }
            }
        }

        if is_spell && self.has_active_arcane_surge() {
            bonuses.push(("Arcane surge", RollBonusContributor::OtherPositive));
        }

        let conditions = self.conditions.borrow();
        if conditions.weakened > 0 {
            bonuses.push((
                "Weakened",
                RollBonusContributor::FlatAmount(-(conditions.weakened as i32)),
            ));
        }

        let encumbrance_penalty = (conditions.encumbered / 2) as i32;
        if encumbrance_penalty > 0 {
            bonuses.push((
                "Encumbered",
                RollBonusContributor::FlatAmount(-(encumbrance_penalty)),
            ));
        }

        if conditions.near_death {
            bonuses.push(("Near-death", RollBonusContributor::Advantage(-1)));
        }

        bonuses
    }

    fn incoming_attack_advantage(&self, reaction: Option<OnAttackedReaction>) -> i32 {
        let mut advantage = 0;
        for (label, bonus) in self.incoming_attack_bonuses(reaction) {
            dbg!(label, bonus);
            match bonus {
                RollBonusContributor::Advantage(n) => advantage += n,
                RollBonusContributor::OtherNegative | RollBonusContributor::OtherPositive => {}
                RollBonusContributor::FlatAmount(_) => unreachable!(),
            }
        }
        advantage
    }

    fn incoming_ability_advantage(&self) -> i32 {
        let mut advantage = 0;
        for (_label, bonus) in self.incoming_ability_bonuses() {
            match bonus {
                RollBonusContributor::Advantage(n) => advantage += n,
                RollBonusContributor::OtherNegative | RollBonusContributor::OtherPositive => {}
                RollBonusContributor::FlatAmount(_) => unreachable!(),
            }
        }
        advantage
    }

    pub fn incoming_attack_bonuses(
        &self,
        reaction: Option<OnAttackedReaction>,
    ) -> Vec<(&'static str, RollBonusContributor)> {
        let mut terms = vec![];
        if self.is_dazed() {
            terms.push(("Dazed", RollBonusContributor::OtherPositive));
        }
        let conditions = self.conditions.borrow();
        if conditions.weakened > 0 {
            terms.push(("Weakened", RollBonusContributor::OtherPositive));
        }
        if conditions.braced {
            terms.push(("Braced", RollBonusContributor::OtherNegative));
        }
        if conditions.distracted {
            terms.push(("Distracted", RollBonusContributor::OtherPositive));
        }
        if conditions.near_death {
            terms.push(("Near-death", RollBonusContributor::Advantage(1)))
        }
        if conditions.exposed > 0 {
            terms.push(("Exposed", RollBonusContributor::OtherPositive));
        }

        if let Some(reaction) = reaction {
            if reaction.effect.bonus_evasion > 0 {
                terms.push((reaction.name, RollBonusContributor::OtherNegative));
            }
        }

        terms
    }

    pub fn incoming_ability_bonuses(&self) -> Vec<(&'static str, RollBonusContributor)> {
        let mut terms = vec![];
        let conditions = self.conditions.borrow();
        if conditions.weakened > 0 {
            terms.push(("Weakened", RollBonusContributor::OtherPositive));
        }
        if conditions.near_death {
            terms.push(("Near-death", RollBonusContributor::Advantage(1)))
        }
        if conditions.exposed > 0 {
            terms.push(("Exposed", RollBonusContributor::OtherPositive));
        }
        terms
    }

    pub fn is_bleeding(&self) -> bool {
        self.conditions.borrow().bleeding > 0
    }

    pub fn receive_condition(&self, condition: Condition) {
        let mut conditions = self.conditions.borrow_mut();
        use Condition::*;
        match condition {
            Protected(n) => conditions.protected += n,
            Dazed(n) => conditions.dazed += n,
            Bleeding(n) => conditions.bleeding += n,
            Braced => conditions.braced = true,
            Raging => conditions.raging = true,
            Distracted => conditions.distracted = true,
            Weakened(n) => conditions.weakened += n,
            MainHandExertion(n) => conditions.mainhand_exertion += n,
            OffHandExertion(n) => conditions.offhand_exertion += n,
            Encumbered(n) => conditions.encumbered += n,
            NearDeath => conditions.near_death = true,
            Dead => conditions.dead = true,
            Slowed(n) => {
                if conditions.slowed == 0 && n > 0 {
                    // Since a character receives max AP at end-of-turn, if they are then slowed by an enemy
                    // that debuff must reasonably affect the character's next turn.
                    self.action_points.lose(SLOWED_AP_PENALTY);
                }
                conditions.slowed += n;
            }
            Exposed(n) => conditions.exposed += n,
            Hindered(n) => conditions.hindered += n,
            ReaperApCooldown => conditions.reaper_ap_cooldown = true,
        }
    }
}

fn are_flanking_target(
    attacker: (i32, i32),
    melee_engager: (i32, i32),
    target: (i32, i32),
) -> bool {
    // Note: engagement is always melee, which is why this vector is also a direction
    let engage_dir = (melee_engager.0 - target.0, melee_engager.1 - target.1);
    let (dx, dy) = (attacker.0 - target.0, attacker.1 - target.1);
    assert!((dx, dy) != (0, 0));

    match engage_dir {
        (1, 0) => dx < 0 && dy.abs() <= dx.abs(),
        (1, -1) => dx <= 0 && dy >= 0,
        (0, -1) => dy > 0 && dx.abs() <= dy.abs(),
        (-1, -1) => dx >= 0 && dy >= 0,
        (-1, 0) => dx > 0 && dy.abs() <= dx.abs(),
        (-1, 1) => dx >= 0 && dy <= 0,
        (0, 1) => dy < 0 && dx.abs() <= dy.abs(),
        (1, 1) => dx <= 0 && dy <= 0,
        _ => unreachable!("Engagement not melee: {engage_dir:?}"),
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

fn within_range_squared(range_squared: f32, source: Position, destination: Position) -> bool {
    let distance_squared = (destination.0 - source.0).pow(2) + (destination.1 - source.1).pow(2);
    distance_squared as f32 <= range_squared
}

fn within_meele(source: Position, destination: Position) -> bool {
    within_range_squared(2.0, source, destination)
}

pub fn distance_between(source: Position, destination: Position) -> f32 {
    (((destination.0 - source.0).pow(2) + (destination.1 - source.1).pow(2)) as f32).sqrt()
}

#[derive(Debug, Clone)]
pub struct NumberedResource {
    current: Cell<u32>,
    max: Cell<u32>,
}

impl NumberedResource {
    fn new(max: u32) -> Self {
        Self {
            current: Cell::new(max),
            max: Cell::new(max),
        }
    }

    pub fn is_at_max(&self) -> bool {
        self.current() == self.max()
    }

    pub fn current(&self) -> u32 {
        self.current.get()
    }

    pub fn ratio(&self) -> f32 {
        self.current() as f32 / self.max() as f32
    }

    pub fn max(&self) -> u32 {
        self.max.get()
    }

    pub fn lose(&self, amount: u32) -> u32 {
        let prev = self.current.get();
        let new = self.current.get().saturating_sub(amount);
        self.current.set(new);
        prev - new
    }

    fn spend(&self, amount: u32) {
        // The caller must have checked that we have the required amount
        self.current.set(self.current.get() - amount);
    }

    pub fn gain(&self, amount: u32) -> u32 {
        let prev = self.current.get();
        let new = (prev + amount).min(self.max.get());
        self.current.set(new);
        new - prev
    }

    pub fn set_to_max(&self) {
        self.current.set(self.max.get());
    }

    pub fn change_max_value_to(&self, new_max: u32) {
        let diff = new_max as i32 - self.max() as i32;
        self.max.set(new_max);
        let new_value = self.current() as i32 + diff;
        assert!(
            new_value >= 0 && new_value <= new_max as i32,
            "{new_value}, {new_max}"
        );
        self.current.set(new_value as u32);
        if self.current.get() > new_max {
            self.current.set(new_max);
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ArmorPiece {
    pub name: &'static str,
    pub protection: u32,
    pub limit_evasion_from_agi: Option<u32>,
    pub icon: EquipmentIconId,
    pub weight: u32,
    pub equip: EquipEffect,
}

#[derive(Debug, Copy, Clone)]
pub struct EquipEffect {
    pub bonus_spell_modifier: u32,
}

impl EquipEffect {
    pub const fn default() -> Self {
        Self {
            bonus_spell_modifier: 0,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
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
    // TODO: Not used?
    pub on_true_hit: Option<AttackHitEffect>,
    pub weight: u32,
}

impl Weapon {
    pub fn is_melee(&self) -> bool {
        matches!(self.range, WeaponRange::Melee)
    }

    pub fn weapon_type(&self) -> WeaponType {
        match self.range {
            WeaponRange::Melee => WeaponType::Melee,
            WeaponRange::Ranged(_) => WeaponType::Ranged,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Shield {
    pub name: &'static str,
    pub sprite: Option<SpriteId>,
    pub icon: EquipmentIconId,
    pub evasion: u32,
    pub on_hit_reaction: Option<OnHitReaction>,
    pub weight: u32,
}

#[derive(Debug, Copy, Clone, PartialEq)]
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

#[derive(Debug, Copy, Clone, PartialEq)]
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
            Self::Ranged(range) => {
                f.write_fmt(format_args!("{} ({})", range, self.extended().unwrap()))
            }
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

    pub fn plus(&self, n: u32) -> Range {
        match self {
            Range::Melee => Range::Float(2f32.sqrt() + n as f32),
            Range::Ranged(range) => Range::Ranged(range + n),
            Range::ExtendableRanged(range) => Range::Ranged(range + n),
            Range::Float(range) => Range::Float(range + n as f32),
        }
    }

    pub fn plusf(&self, n: f32) -> Range {
        match self {
            Range::Melee => Range::Float(2f32.sqrt() + n),
            Range::Ranged(range) => Range::Float(*range as f32 + n),
            Range::ExtendableRanged(range) => Range::Float(*range as f32 + n),
            Range::Float(range) => Range::Float(range + n),
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

#[derive(Debug, Copy, Clone)]
pub enum RollBonusContributor {
    Advantage(i32),
    FlatAmount(i32),
    OtherNegative,
    OtherPositive,
}

impl RollBonusContributor {
    pub fn goodness(&self) -> Goodness {
        match self {
            RollBonusContributor::Advantage(n) => {
                if *n > 0 {
                    Goodness::Good
                } else if *n < 0 {
                    Goodness::Bad
                } else {
                    unreachable!()
                }
            }
            RollBonusContributor::FlatAmount(n) => {
                if *n > 0 {
                    Goodness::Good
                } else if *n < 0 {
                    Goodness::Bad
                } else {
                    unreachable!()
                }
            }
            RollBonusContributor::OtherNegative => Goodness::Bad,
            RollBonusContributor::OtherPositive => Goodness::Good,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum EquipmentEntry {
    Weapon(Weapon),
    Shield(Shield),
    Armor(ArmorPiece),
    Consumable(Consumable),
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Consumable {
    pub name: &'static str,
    pub health_gain: u32,
    pub mana_gain: u32,
    pub icon: EquipmentIconId,
    pub weight: u32,
}

impl EquipmentEntry {
    pub fn name(&self) -> &'static str {
        match self {
            EquipmentEntry::Weapon(weapon) => weapon.name,
            EquipmentEntry::Shield(shield) => shield.name,
            EquipmentEntry::Armor(armor) => armor.name,
            EquipmentEntry::Consumable(consumable) => consumable.name,
        }
    }

    pub fn icon(&self) -> EquipmentIconId {
        match self {
            EquipmentEntry::Weapon(weapon) => weapon.icon,
            EquipmentEntry::Shield(_shield) => EquipmentIconId::SmallShield,
            EquipmentEntry::Armor(armor) => armor.icon,
            EquipmentEntry::Consumable(consumable) => consumable.icon,
        }
    }

    fn weight(&self) -> u32 {
        match self {
            EquipmentEntry::Weapon(weapon) => weapon.weight,
            EquipmentEntry::Shield(shield) => shield.weight,
            EquipmentEntry::Armor(armor) => armor.weight,
            EquipmentEntry::Consumable(consumable) => consumable.weight,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum EquipmentSlotRole {
    MainHand,
    OffHand,
    Armor,
    Inventory(usize),
}

impl EquipmentSlotRole {
    fn from_hand_type(hand: HandType) -> Self {
        match hand {
            HandType::MainHand => Self::MainHand,
            HandType::OffHand => Self::OffHand,
        }
    }

    pub fn is_equipped(&self) -> bool {
        use EquipmentSlotRole::*;
        match self {
            MainHand | OffHand | Armor => true,
            Inventory(_) => false,
        }
    }
}
