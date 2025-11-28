use std::cell::{Cell, RefCell};

use std::collections::{HashMap, HashSet};
use std::default;
use std::fmt::Display;
use std::rc::{Rc, Weak};

use indexmap::IndexMap;
use macroquad::color::Color;
use macroquad::rand::ChooseRandom;

use crate::bot::BotBehaviour;
use crate::d20::{probability_of_d20_reaching, roll_d20_with_advantage, DiceRollBonus};

use crate::data::{PassiveSkill, ADRENALIN_POTION};
use crate::game_ui_connection::{ActionOrSwitchTo, GameUserInterfaceConnection};
use crate::init_fight_map::GameInitState;
use crate::pathfind::PathfindGrid;
use crate::textures::{EquipmentIconId, IconId, PortraitId, SpriteId};
use crate::util::are_adjacent;

pub type Position = (i32, i32);

pub const MAX_ACTION_POINTS: u32 = 5;
pub const ACTION_POINTS_PER_TURN: u32 = 3;

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
    round_index: u32,
    round_length: u32,
}

impl CoreGame {
    pub fn new(user_interface: GameUserInterfaceConnection, init_state: &GameInitState) -> Self {
        let characters = init_state.characters.clone();
        let round_length = characters.iter().count() as u32;
        Self {
            characters,
            active_character_id: init_state.active_character_id,
            user_interface,
            pathfind_grid: init_state.pathfind_grid.clone(),
            round_index: 0,
            round_length,
        }
    }

    pub async fn run(mut self) -> Vec<Character> {
        for character in self.characters.iter() {
            character.update_encumbrance();
            character.action_points.current.set(ACTION_POINTS_PER_TURN);
            character.regain_movement();
            character.on_health_changed();
            character.has_taken_a_turn_this_round.set(false);
        }
        self.on_character_positions_changed();

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
                // TODO: extremely iffy. We should instead have a proper 'end of fight' cleanup.
                self.perform_end_of_turn_character().await;

                for character in self.characters.iter() {
                    character.stamina.set_to_max();
                }

                return self.characters.player_characters();
            }

            let action_or_character_change = self.user_interface.select_action(&self).await;

            let action = match action_or_character_change {
                ActionOrSwitchTo::Action(action) => action,
                ActionOrSwitchTo::SwitchTo(character_id) => {
                    println!(
                        "SWITCHING CHAR FROM {} TO {}",
                        self.active_character_id, character_id
                    );
                    assert!(self.active_character_id != character_id);
                    assert!(!self
                        .characters
                        .get(character_id)
                        .has_taken_a_turn_this_round
                        .get());
                    self.active_character_id = character_id;
                    self.notify_ui_of_new_active_char().await;
                    continue;
                }
            };

            let mut ending_turn = false;

            if let Some(action) = action {
                let mut killed_by_action = HashSet::new();
                let action_outcome = self.perform_action(action).await;

                if let ActionOutcome::AttackHit { victim_id, damage } = action_outcome {
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

                /*
                No, don't end turn automatically; what if the player wants to move when having 0 AP?
                if self.active_character().action_points.current() == 0 {
                    ending_turn = true;
                }
                 */
            } else {
                let name = self.active_character().name;
                self.log(format!("{} ended their turn", name)).await;
                ending_turn = true;
            }

            if ending_turn {
                self.perform_end_of_turn_character().await;
                //let prev_index_in_round = self.active_character().index_in_round.unwrap();
                self.active_character_id = self.characters.next_id(self.active_character_id);
                self.notify_ui_of_new_active_char().await;

                let index_in_round = self.active_character().index_in_round.unwrap();

                let new_round = self
                    .characters
                    .iter()
                    .all(|ch| ch.has_taken_a_turn_this_round.get());

                if new_round {
                    println!("NEW ROUND STARTED!");
                    for character in self.characters.iter() {
                        character.has_taken_a_turn_this_round.set(false);
                    }
                    self.round_index += 1;
                }

                let game_time = self.current_time();
                dbg!(
                    self.round_index,
                    self.round_length,
                    index_in_round,
                    game_time
                );

                println!("Expiring character conditions...");
                for character in self.characters.iter() {
                    character.set_current_game_time(game_time);
                }
            }

            // We must make sure to have a valid (alive, existing) active_character_id before handing over control
            // to the UI, as it may ask us about the active character.
            let active_character_died = self.active_character().is_dead();
            if active_character_died {
                println!("ACTIVE CHAR DIED");
                dbg!(self.active_character_id);
                self.active_character()
                    .has_taken_a_turn_this_round
                    .set(true);
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

                self.ui_handle_event(GameEvent::CharacterDying {
                    character: *dead_id,
                })
                .await;

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
                self.notify_ui_of_new_active_char().await;
            }
        }
    }

    async fn notify_ui_of_new_active_char(&self) {
        self.ui_handle_event(GameEvent::NewActiveCharacter {
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

                assert!(
                    attacker
                        .attack_reaches(
                            hand,
                            defender.position.get(),
                            enhancements.iter().map(|e| e.effect)
                        )
                        .1
                        != ActionReach::No
                );

                let mut action_point_cost = attacker.weapon(hand).unwrap().action_point_cost as i32;

                for enhancement in &enhancements {
                    action_point_cost += enhancement.action_point_cost as i32;
                    action_point_cost -= enhancement.effect.action_point_discount as i32;
                    attacker.stamina.spend(enhancement.stamina_cost);
                    attacker.mana.spend(enhancement.mana_cost);
                    attacker.on_mana_changed();
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
                            && other_char.can_use_opportunity_attack(attacker.id())
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

                    let enhancements = enhancements.iter().map(|e| (e.name, e.effect)).collect();

                    self.ui_handle_event(GameEvent::AttackWasInitiated {
                        actor: self.active_character_id,
                        target,
                    })
                    .await;

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
                        AttackOutcome::Hit(dmg, _type) => Some(dmg),
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
                extra_cost,
                positions,
                total_distance,
            } => {
                dbg!("Action::Move", extra_cost, total_distance);
                let character = self.active_character();
                //character.action_points.spend(extra_cost);
                character.stamina.spend(extra_cost);
                // Costs 1 per extra distance
                let paid_distance = extra_cost as f32;
                if total_distance > paid_distance {
                    let free_distance = total_distance - paid_distance;
                    character.spend_movement(free_distance);
                }

                self.perform_movement(positions, true).await;
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
                    character.on_mana_changed();
                }
                if let Some(apply_effect) = consumable.effect {
                    // TODO: log it?
                    self.perform_effect_application(apply_effect, None, character);
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

    async fn perform_movement(
        &self,
        mut positions: Vec<Position>,
        can_trigger_opportunity_attack: bool,
    ) {
        let start_position = positions.remove(0);
        assert!(start_position == self.active_character().pos());
        assert!(
            !positions.is_empty(),
            "movement must consist of more than just the start position"
        );

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
                    // Movement opportunity attack
                    if can_trigger_opportunity_attack
                        && other_char.can_use_opportunity_attack(character.id())
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

        self.on_character_positions_changed();
    }

    fn on_character_positions_changed(&self) {
        let mut positions = vec![];
        for character in self.characters.iter() {
            positions.push((character.pos(), character.player_controlled()));
        }

        for character in self.characters.iter() {
            if character
                .known_passive_skills
                .contains(&PassiveSkill::ThrillOfBattle)
            {
                let mut num_adjacent_enemies = 0;
                for (pos, player_controlled) in &positions {
                    if *player_controlled != character.player_controlled() {
                        if are_adjacent(*pos, character.pos()) {
                            num_adjacent_enemies += 1;
                        }
                    }
                }
                dbg!(num_adjacent_enemies);
                character
                    .conditions
                    .borrow_mut()
                    .add_or_remove(Condition::ThrillOfBattle, num_adjacent_enemies >= 2);
            }
        }
    }

    fn perform_effect_application(
        &self,
        effect: ApplyEffect,
        giver: Option<&Character>,
        receiver: &Character,
    ) -> (String, u32) {
        let mut damage_dealt = 0;
        let line = match effect {
            ApplyEffect::RemoveActionPoints(n) => {
                receiver.action_points.lose(n);
                format!("  {} lost {} AP", receiver.name, n)
            }
            ApplyEffect::GainStamina(n) => {
                let amount_gained = receiver.stamina.gain(n);
                format!("  {} gained {} stamina", receiver.name, amount_gained)
            }
            ApplyEffect::Condition(apply_condition) => {
                self.perform_receive_condition(apply_condition, receiver)
            }
            ApplyEffect::PerBleeding {
                damage,
                caster_healing_percentage,
            } => {
                let stacks = receiver
                    .conditions
                    .borrow()
                    .get(&Condition::Bleeding)
                    .map(|state| state.stacks.unwrap())
                    .unwrap_or(0);
                damage_dealt = self.perform_losing_health(receiver, damage * stacks);
                let healing_amount = damage_dealt * caster_healing_percentage / 100;
                self.perform_gain_health(giver.unwrap(), healing_amount);
                format!(
                    "  {} lost {} health. {} was healed for {}",
                    receiver.name,
                    damage_dealt,
                    giver.unwrap().name,
                    healing_amount
                )
            }
            ApplyEffect::ConsumeCondition { condition } => {
                let stacks_cleared = receiver.clear_condition(condition);
                let mut line = format!("  {} lost {}", receiver.name, condition.name());
                if stacks_cleared > 0 {
                    line.push_str(&format!(" ({})", stacks_cleared));
                }
                line
            }
        };

        (line, damage_dealt)
    }

    fn current_time(&self) -> u32 {
        self.round_index * self.round_length + self.active_character().index_in_round.unwrap()
    }

    fn perform_receive_condition(
        &self,
        apply_condition: ApplyCondition,
        receiver: &Character,
    ) -> String {
        let ends_at = apply_condition
            .duration_rounds
            .map(|rounds| self.current_time() + (rounds * self.round_length));
        receiver.receive_condition(apply_condition.condition, apply_condition.stacks, ends_at);
        let mut line = format!(
            "  {} received {}",
            receiver.name,
            apply_condition.condition.name()
        );

        if let Some(stacks) = apply_condition.stacks {
            line.push_str(&format!(" x {}", stacks));
        }

        if let Some(duration) = apply_condition.duration_rounds {
            line.push_str(&format!(" ({})", duration));
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
        caster.on_mana_changed();
        caster.stamina.spend(ability.stamina_cost);

        let mut enemies_hit = vec![];

        for enhancement in &enhancements {
            caster.action_points.spend(enhancement.action_point_cost);
            caster.mana.spend(enhancement.mana_cost);
            caster.on_mana_changed();
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

            let mut maybe_ability_roll = None;

            if let Some(roll_type) = ability.roll {
                let dice_roll = roll_d20_with_advantage(advantange_level);

                if let Some(description) = roll_description(advantange_level) {
                    detail_lines.push(description);
                }

                let mut dice_roll_line = format!("Rolled: {}", dice_roll);
                let mut roll_calculation = dice_roll as i32;
                match roll_type {
                    AbilityRollType::Spell => {
                        let modifier = caster_ref.spell_modifier() as i32;
                        roll_calculation += modifier;
                        dice_roll_line.push_str(&format!(" (+{} spell mod)", modifier));

                        for enhancement in &enhancements {
                            if let Some(e) = enhancement.spell_effect {
                                let bonus = e.roll_bonus;
                                if bonus > 0 {
                                    roll_calculation += bonus as i32;
                                    dice_roll_line
                                        .push_str(&format!(" +{} ({})", bonus, enhancement.name,));
                                }
                            }
                        }
                        let ability_result = roll_calculation as u32;
                        dice_roll_line.push_str(&format!(" = {}", ability_result));

                        maybe_ability_roll = Some(AbilityRoll::Spell {
                            result: ability_result,
                            line: dice_roll_line,
                        });
                    }
                    AbilityRollType::Attack(bonus) => {
                        maybe_ability_roll = Some(AbilityRoll::Attack { bonus });
                    }
                };
            }

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
                        self.perform_movement(positions.clone(), false).await;
                    }

                    let target = self.characters.get(*target_id);
                    assert!(caster.reaches_with_ability(
                        ability,
                        &enhancements,
                        target.position.get()
                    ));

                    self.ui_handle_event(GameEvent::AbilityWasInitiated {
                        actor: caster_id,
                        ability,
                        target: Some(*target_id),
                    })
                    .await;

                    let mut ability_roll = maybe_ability_roll.unwrap();

                    if let AbilityRoll::Spell { result: _, line } = &mut ability_roll {
                        let spell_enemy_effect = effect.unwrap_spell();
                        if let Some(contest) = spell_enemy_effect.defense_type {
                            match contest {
                                DefenseType::Will => {
                                    line.push_str(&format!(", vs will={}", target.will()))
                                }
                                DefenseType::Evasion => {
                                    line.push_str(&format!(", vs evasion={}", target.evasion()))
                                }
                                DefenseType::Toughness => {
                                    line.push_str(&format!(", vs toughness={}", target.toughness()))
                                }
                            }
                        }
                        detail_lines.push(line.to_string());
                    }

                    let outcome = self.perform_ability_enemy_effect(
                        caster,
                        ability.name,
                        &ability_roll,
                        &enhancements,
                        effect,
                        target,
                        &mut detail_lines,
                        true,
                    );
                    target_outcome = Some((*target_id, outcome));

                    if let Some((radius, acquisition, area_effect)) = impact_area {
                        detail_lines.push("Area of effect:".to_string());

                        let area_target_outcomes = self.perform_ability_area_enemy_effect(
                            radius,
                            "AoE",
                            ability_roll,
                            &enhancements,
                            caster,
                            target.position.get(),
                            &mut detail_lines,
                            area_effect,
                            acquisition,
                        );

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

                    self.ui_handle_event(GameEvent::AbilityWasInitiated {
                        actor: caster_id,
                        ability,
                        target: Some(*target_id),
                    })
                    .await;

                    let ability_roll = maybe_ability_roll.unwrap();
                    let (ability_result, dice_roll_line) = ability_roll.unwrap_spell();
                    detail_lines.push(dice_roll_line.to_string());

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
                    area_effect,
                } => {
                    self.ui_handle_event(GameEvent::AbilityWasInitiated {
                        actor: caster_id,
                        ability,
                        target: None,
                    })
                    .await;

                    let target_pos = selected_target.unwrap_position();
                    assert!(caster.reaches_with_ability(ability, &enhancements, target_pos));

                    let ability_roll = maybe_ability_roll.unwrap();
                    let (_ability_result, dice_roll_line) = ability_roll.unwrap_spell();
                    detail_lines.push(dice_roll_line.to_string());

                    let outcomes = self.perform_ability_area_effect(
                        ability.name,
                        ability_roll,
                        &enhancements,
                        caster,
                        target_pos,
                        area_effect,
                        &mut detail_lines,
                    );

                    area_outcomes = Some((target_pos, outcomes));
                }

                AbilityTarget::None {
                    self_area,
                    self_effect,
                } => {
                    self.ui_handle_event(GameEvent::AbilityWasInitiated {
                        actor: caster_id,
                        ability,
                        target: None,
                    })
                    .await;

                    if let Some(AbilityRoll::Spell { result: _, line }) = &maybe_ability_roll {
                        detail_lines.push(line.clone());
                    }

                    if let Some(effect) = self_effect {
                        let degree_of_success = if let Some(ability_roll) = &maybe_ability_roll {
                            let (ability_result, _dice_roll_line) = ability_roll.unwrap_spell();
                            ability_result / 10
                        } else {
                            0
                        };

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

                    if let Some(area_effect) = self_area {
                        dbg!("SELF AREA ", area_effect.radius);

                        let ability_roll = maybe_ability_roll.unwrap();

                        let outcomes = self.perform_ability_area_effect(
                            ability.name,
                            ability_roll,
                            &enhancements,
                            caster,
                            caster.position.get(),
                            area_effect,
                            &mut detail_lines,
                        );
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
            // TODO also communicate if the caster healed from hitting the target (e.g. necrotic influence)
            self.ui_handle_event(GameEvent::AbilityResolved {
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

    fn perform_ability_area_effect(
        &self,
        name: &'static str,
        ability_roll: AbilityRoll,
        enhancements: &[AbilityEnhancement],
        caster: &Character,
        area_center: Position,
        area_effect: AreaEffect,
        detail_lines: &mut Vec<String>,
    ) -> Vec<(CharacterId, AbilityTargetOutcome)> {
        match area_effect.effect {
            AbilityEffect::Negative(effect) => self.perform_ability_area_enemy_effect(
                area_effect.radius,
                name,
                ability_roll,
                enhancements,
                caster,
                area_center,
                detail_lines,
                effect,
                area_effect.acquisition,
            ),

            AbilityEffect::Positive(effect) => {
                assert!(area_effect.acquisition == AreaTargetAcquisition::Allies);

                self.perform_ability_area_ally_effect(
                    area_effect.radius,
                    name,
                    enhancements,
                    caster,
                    area_center,
                    detail_lines,
                    ability_roll,
                    effect,
                )
            }
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
        ability_roll: AbilityRoll,
        effect: AbilityPositiveEffect,
    ) -> Vec<(CharacterId, AbilityTargetOutcome)> {
        let mut target_outcomes = vec![];

        for enhancement in enhancements {
            let e = enhancement.spell_effect.unwrap();
            if e.increased_radius_tenths > 0 {
                radius = radius.plusf(e.increased_radius_tenths as f32 * 0.1);
            }
        }

        let roll_result = ability_roll.unwrap_spell().0;

        let degree_of_success = roll_result / 10;
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
                    effect,
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
        ally_effect: AbilityPositiveEffect,
        target: &Character,
        detail_lines: &mut Vec<String>,
        degree_of_success: u32,
    ) -> AbilityTargetOutcome {
        let mut maybe_healing = None;

        if ally_effect.healing > 0 {
            let mut healing = ally_effect.healing;

            let mut line = format!("  Healing: {} ({})", ally_effect.healing, name);

            if degree_of_success > 0 {
                line.push_str(&format!(" +{} (fortune)", degree_of_success));
                healing += degree_of_success;
            }
            line.push_str(&format!(" = {}", healing));
            detail_lines.push(line);

            let health_gained = self.perform_gain_health(target, healing);
            detail_lines.push(format!(
                "  {} was healed for {}",
                target.name, health_gained
            ));
            maybe_healing = Some(health_gained);
        };

        for mut effect in ally_effect.apply.iter().flatten().flatten().copied() {
            match effect {
                ApplyEffect::RemoveActionPoints(ref mut n) => *n += degree_of_success,
                ApplyEffect::GainStamina(ref mut n) => *n += degree_of_success,
                ApplyEffect::Condition(ref apply_condition) => {
                    /*
                    if let Some(stacks) = condition.stacks() {
                        *stacks += degree_of_success;
                    }
                     */
                }
                ApplyEffect::PerBleeding { .. } => {}
                ApplyEffect::ConsumeCondition { .. } => {}
            }

            dbg!(effect);

            let (log_line, _damage) = self.perform_effect_application(effect, None, target);
            detail_lines.push(log_line);
        }

        for enhancement in enhancements {
            let effect = enhancement.spell_effect.unwrap();
            for apply_effect in effect.target_on_hit.iter().flatten().flatten() {
                let (log_line, _damage) =
                    self.perform_effect_application(*apply_effect, None, target);
                detail_lines.push(format!("{} ({})", log_line, enhancement.name));
            }
        }

        AbilityTargetOutcome::AffectedAlly {
            healing: maybe_healing,
        }
    }

    fn perform_ability_area_enemy_effect(
        &self,
        mut radius: Range,
        name: &'static str,
        ability_roll: AbilityRoll,
        enhancements: &[AbilityEnhancement],
        caster: &Character,
        area_center: Position,
        detail_lines: &mut Vec<String>,
        effect: AbilityNegativeEffect,
        acquisition: AreaTargetAcquisition,
    ) -> Vec<(CharacterId, AbilityTargetOutcome)> {
        assert!(acquisition != AreaTargetAcquisition::Allies);

        let mut target_outcomes = vec![];

        for enhancement in enhancements {
            if let Some(e) = enhancement.spell_effect {
                if e.increased_radius_tenths > 0 {
                    radius = radius.plusf(e.increased_radius_tenths as f32 * 0.1);
                }
            }
        }

        for other_char in self.characters.iter() {
            let is_ally = other_char.player_controlled() == caster.player_controlled();
            let valid_target = match acquisition {
                AreaTargetAcquisition::Enemies => !is_ally,
                AreaTargetAcquisition::Everyone => true,
                AreaTargetAcquisition::Allies => unreachable!(),
            };
            if !valid_target {
                continue;
            }

            if within_range_squared(radius.squared(), area_center, other_char.position.get()) {
                let mut line = other_char.name.to_string();
                match effect {
                    AbilityNegativeEffect::Spell(spell_enemy_effect) => {
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
                    AbilityNegativeEffect::PerformAttack => {
                        // The relevant details will come from perform_attack, not from here.
                    }
                }

                detail_lines.push(line);

                let outcome = self.perform_ability_enemy_effect(
                    caster,
                    name,
                    &ability_roll,
                    enhancements,
                    effect,
                    other_char,
                    detail_lines,
                    false,
                );

                target_outcomes.push((other_char.id(), outcome));
            }
        }

        target_outcomes
    }

    fn perform_ability_enemy_effect(
        &self,
        caster: &Character,
        ability_name: &'static str,
        ability_roll: &AbilityRoll,
        enhancements: &[AbilityEnhancement],
        enemy_effect: AbilityNegativeEffect,
        target: &Character,
        detail_lines: &mut Vec<String>,
        is_direct_target: bool,
    ) -> AbilityTargetOutcome {
        match enemy_effect {
            AbilityNegativeEffect::Spell(spell_enemy_effect) => self.perform_spell_enemy_effect(
                caster,
                ability_name,
                ability_roll,
                enhancements,
                spell_enemy_effect,
                target,
                detail_lines,
                is_direct_target,
            ),
            AbilityNegativeEffect::PerformAttack => {
                let attack_enhancement_effects = enhancements
                    .iter()
                    .filter_map(|e| e.attack_enhancement_effect())
                    .collect();

                let reaction = None;
                let roll_modifier = ability_roll.unwrap_attack_bonus();
                let event = self.perform_attack(
                    caster.id(),
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
        caster: &Character,
        ability_name: &'static str,
        ability_roll: &AbilityRoll,
        enhancements: &[AbilityEnhancement],
        spell_enemy_effect: SpellNegativeEffect,
        target: &Character,
        detail_lines: &mut Vec<String>,
        is_direct_target: bool,
    ) -> AbilityTargetOutcome {
        let success = match spell_enemy_effect.defense_type {
            Some(contest) => {
                let ability_result = ability_roll.unwrap_spell().0;
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
                    detail_lines.push("  Crit".to_string());
                    "Crit".to_string()
                }
                n => {
                    detail_lines.push(format!("  Crit ({})", n));
                    "Crit".to_string()
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
                    dmg_str.push_str(" -1 (Graze)");
                    // Since there's no armor/protection against spells, rounding up would make the spell too powerful.
                    dmg_calculation -= 1;
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

            let mut damage_from_effects = 0;

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
                    ApplyEffect::Condition(ref mut apply_condition) => {
                        /*
                        if let Some(stacks) = condition.stacks() {
                            apply_degree_of_success(stacks, degree_of_success)
                        }
                         */
                    }
                    ApplyEffect::PerBleeding { .. } => {}
                    ApplyEffect::ConsumeCondition { .. } => {}
                }

                applied_effects.push(effect);
                let (log_line, damage) =
                    self.perform_effect_application(effect, Some(caster), target);
                damage_from_effects += damage;
                detail_lines.push(log_line);
            }

            for enhancement in enhancements {
                // TODO: shouldn't these also be affected by degree of success?
                let e = enhancement.spell_effect.unwrap();
                let effects = if is_direct_target {
                    e.target_on_hit
                } else {
                    e.area_on_hit
                };
                for effect in effects.iter().flatten().flatten() {
                    applied_effects.push(*effect);
                    let (log_line, damage) =
                        self.perform_effect_application(*effect, Some(caster), target);
                    damage_from_effects += damage;
                    detail_lines.push(format!("{} ({})", log_line, enhancement.name));
                }
            }

            let damage = match damage {
                Some(dmg) => Some(dmg + damage_from_effects),
                None if damage_from_effects > 0 => Some(damage_from_effects),
                _ => None,
            };

            AbilityTargetOutcome::HitEnemy {
                damage,
                graze: degree_of_success == -1,
                applied_effects,
            }
        } else {
            let line = match spell_enemy_effect.defense_type {
                Some(DefenseType::Will | DefenseType::Toughness) => {
                    format!("  {} resisted the spell", target.name)
                }
                Some(DefenseType::Evasion) => {
                    format!("  The spell missed {}", target.name)
                }
                None => unreachable!("uncontested effect cannot fail"),
            };
            detail_lines.push(line);
            AbilityTargetOutcome::Resisted
        }
    }

    fn perform_losing_health(&self, character: &Character, amount: u32) -> u32 {
        let amount_lost = character.health.lose(amount);
        character.on_health_changed();
        amount_lost
    }

    fn perform_gain_health(&self, character: &Character, amount: u32) -> u32 {
        let amount_gained = character.health.gain(amount);
        character.on_health_changed();
        amount_gained
    }

    async fn log(&self, line: impl Into<String>) {
        self.ui_handle_event(GameEvent::LogLine(line.into())).await;
    }

    /**
     * Note: lots of duplication with predict_attack
     */
    fn perform_attack(
        &self,
        attacker_id: CharacterId,
        hand_type: HandType,
        enhancements: Vec<(&'static str, AttackEnhancementEffect)>,
        defender_id: CharacterId,
        defender_reaction: Option<OnAttackedReaction>,
        ability_roll_modifier: i32,
    ) -> AttackedEvent {
        let attacker = &self.characters.get_rc(attacker_id);
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

        let mut evasion_from_parry = 0;
        let mut evasion_from_sidestep = 0;
        let mut evasion_from_block = 0;
        let mut skip_attack_exertion = false;

        let attack_modifier = attacker.attack_modifier(hand_type);

        let mut detail_lines = vec![];

        let mut armor_value = defender.protection_from_armor();

        if let Some(reaction) = defender_reaction {
            defender.action_points.spend(reaction.action_point_cost);
            defender.stamina.spend(reaction.stamina_cost);

            detail_lines.push(format!("{} reacted with {}", defender.name, reaction.name));

            let bonus_evasion = reaction.effect.bonus_evasion;
            if bonus_evasion > 0 {
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

            let bonus_armor = reaction.effect.bonus_armor;
            if bonus_armor > 0 {
                detail_lines.push(format!(
                    "  Armor: {} +{} ({}) = {}",
                    armor_value,
                    bonus_armor,
                    reaction.name,
                    armor_value + bonus_armor
                ));
                armor_value += bonus_armor;
            }

            match reaction.id {
                OnAttackedReactionId::Parry => evasion_from_parry = bonus_evasion,
                OnAttackedReactionId::SideStep => evasion_from_sidestep = bonus_evasion,
                OnAttackedReactionId::Block => evasion_from_block = bonus_evasion,
            }
        }

        let unmodified_roll = roll_d20_with_advantage(attack_bonus.advantage);
        let roll_result =
            ((unmodified_roll + attack_modifier) as i32 + attack_bonus.flat_amount) as u32;

        if let Some(description) = roll_description(attack_bonus.advantage) {
            detail_lines.push(description);
        }

        let mut armor_penetrators = vec![];
        let weapon = attacker.weapon(hand_type).unwrap();
        let mut used_arrow = None;

        for (name, effect) in &enhancements {
            let penetration = effect.armor_penetration;
            if penetration > 0 {
                armor_penetrators.push((penetration, *name));
            }
            if effect.consume_equipped_arrow {
                assert!(used_arrow.is_none());
                assert!(!weapon.is_melee());
                let stack = attacker.arrows.get().unwrap();
                used_arrow = Some(stack.arrow);
                attacker.spend_one_arrow();
            }
        }

        if let Some(arrow) = used_arrow {
            let penetration = arrow.bonus_penetration;
            if penetration > 0 {
                armor_penetrators.push((penetration, arrow.name));
            }
        }

        if attacker
            .known_passive_skills
            .contains(&PassiveSkill::WeaponProficiency)
        {
            armor_penetrators.push((1, PassiveSkill::WeaponProficiency.name()));
        }

        let mut armor_str = armor_value.to_string();
        for (penetration, label) in armor_penetrators {
            armor_value = armor_value.saturating_sub(penetration);
            armor_str.push_str(&format!(" -{} ({})", penetration, label));
        }

        detail_lines.push(format!(
            "Rolled: {} (+{} atk mod) {}= {}, vs evasion={}",
            unmodified_roll,
            attack_modifier,
            if attack_bonus.flat_amount > 0 {
                format!("(+{}) ", attack_bonus.flat_amount)
            } else if attack_bonus.flat_amount < 0 {
                format!("(-{}) ", -attack_bonus.flat_amount)
            } else {
                "".to_string()
            },
            roll_result,
            evasion,
        ));

        let weapon = attacker.weapon(hand_type).unwrap();
        let outcome = if roll_result >= evasion.saturating_sub(10) {
            let mut on_true_hit_effect = None;
            let mut dmg_calculation = weapon.damage as i32;

            let mut dmg_str = format!("  Damage: {} ({})", dmg_calculation, weapon.name);
            let mut attack_hit_type = AttackHitType::Regular;

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

            if attacker
                .known_passive_skills
                .contains(&PassiveSkill::Honorless)
            {
                let bonus_dmg = 1;
                if is_target_flanked(attacker.pos(), defender) {
                    dmg_str.push_str(&format!(" +{} (Honorless)", bonus_dmg));
                    dmg_calculation += bonus_dmg as i32;
                }
            }

            if roll_result < evasion {
                attack_hit_type = AttackHitType::Graze;
                dmg_str.push_str(" -25% (graze)");
                dmg_calculation -= (dmg_calculation as f32 * 0.25).ceil() as i32;
                detail_lines.push("  Graze!".to_string());
            } else {
                on_true_hit_effect = weapon.on_true_hit;

                let crit = roll_result >= evasion + 10;

                if crit {
                    attack_hit_type = AttackHitType::Critical;
                    detail_lines.push(format!("  Critical hit"));
                    let bonus_percentage = 50;
                    dmg_str.push_str(&format!(" +{bonus_percentage}% (crit)"));
                    dmg_calculation += (dmg_calculation as f32 * 0.5).ceil() as i32;
                } else {
                    detail_lines.push("  Hit".to_string());
                }
            }

            if armor_value > 0 {
                dmg_str.push_str(&format!(" -{armor_value} (armor)"));
                dmg_calculation -= armor_value as i32;
            }

            let damage = dmg_calculation.max(0) as u32;

            dmg_str.push_str(&format!(" = {damage}"));

            detail_lines.push(dmg_str);

            self.perform_losing_health(defender, damage);

            if let Some(effect) = on_true_hit_effect {
                match effect {
                    AttackHitEffect::Apply(effect) => {
                        let (log_line, _damage) =
                            self.perform_effect_application(effect, Some(attacker), defender);
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
                                let (log_line, _damage) = self.perform_effect_application(
                                    apply_effect,
                                    Some(attacker),
                                    defender,
                                );
                                log_line
                            }
                        };

                        detail_lines.push(format!("{} ({})", log_line, name))
                    }

                    if let Some((x, condition)) = effect.inflict_x_condition_per_damage {
                        //*condition.stacks().unwrap() = damage;
                        let stacks = (damage * x.num) / x.den;
                        let line = self.perform_receive_condition(
                            ApplyCondition {
                                condition,
                                stacks: Some(stacks),
                                duration_rounds: None,
                            },
                            defender,
                        );
                        detail_lines.push(format!("{} ({})", line, name))
                    }
                }

                if let Some(arrow) = used_arrow {
                    if let Some(apply_effect) = arrow.on_damage_apply {
                        let (log_line, _damage) =
                            self.perform_effect_application(apply_effect, Some(attacker), defender);
                        detail_lines.push(format!("{} ({})", log_line, arrow.name))
                    }
                }
            }

            if defender.lose_protected() {
                detail_lines.push(format!("{} lost Protected", defender.name));
            }

            AttackOutcome::Hit(damage, attack_hit_type)
        } else if roll_result
            < evasion
                .saturating_sub(evasion_from_parry + evasion_from_sidestep + evasion_from_block + 5)
        {
            detail_lines.push("  Missed!".to_string());
            AttackOutcome::Miss
        } else if evasion_from_parry > 0 {
            detail_lines.push("  Parried!".to_string());
            AttackOutcome::Parry
        } else if evasion_from_sidestep > 0 {
            detail_lines.push("  Side stepped!".to_string());
            AttackOutcome::Dodge
        } else if evasion_from_block > 0 {
            detail_lines.push("  Blocked!".to_string());
            AttackOutcome::Block
        } else {
            unreachable!("{roll_result}, {evasion}, {evasion_from_parry}, {evasion_from_sidestep}");
        };

        if defender.lose_distracted() {
            detail_lines.push(format!("{} lost Distracted", defender.name));
        }

        for (name, effect) in &enhancements {
            if let Some(effect) = effect.on_target {
                let (log_line, _damage) =
                    self.perform_effect_application(effect, Some(attacker), defender);
                detail_lines.push(format!("{} ({})", log_line, name));
            }
        }

        let mut area_outcomes = None;
        if let Some(arrow) = used_arrow {
            if let Some(area_effect) = arrow.area_effect {
                detail_lines.push("".to_string());
                let area_target_outcomes = self.perform_ability_area_effect(
                    arrow.name,
                    AbilityRoll::Spell {
                        result: roll_result,
                        line: "".to_string(),
                    },
                    &[],
                    &attacker,
                    defender.pos(),
                    area_effect,
                    &mut detail_lines,
                );
                area_outcomes = Some(area_target_outcomes);
            }
        }

        if skip_attack_exertion {
            detail_lines.push("  The attack did not lead to exertion (true hit)".to_string());
        } else {
            let exertion = match hand_type {
                HandType::MainHand => {
                    attacker.receive_condition(Condition::MainHandExertion, Some(1), None);
                    attacker.hand_exertion(HandType::MainHand)
                }
                HandType::OffHand => {
                    attacker.receive_condition(Condition::OffHandExertion, Some(1), None);
                    attacker.hand_exertion(HandType::OffHand)
                }
            };
            detail_lines.push(format!("  The attack led to exertion ({})", exertion));
        }

        if weapon.is_melee() {
            if let Some(previously_engaged) = attacker.engagement_target.take() {
                self.characters
                    .get(previously_engaged)
                    .set_not_engaged_by(attacker.id());
            }
            defender.set_engaged_by(Rc::clone(attacker));
            attacker.engagement_target.set(Some(defender.id()));
        }

        AttackedEvent {
            attacker: attacker_id,
            target: defender_id,
            outcome,
            detail_lines,
            area_outcomes,
        }
    }

    async fn perform_on_hit_reaction(&mut self, reactor_id: CharacterId, reaction: OnHitReaction) {
        let reactor = self.characters.get(reactor_id);
        reactor.action_points.spend(reaction.action_point_cost);
        reactor.stamina.spend(reaction.stamina_cost);
        let reactor_name = reactor.name;

        match reaction.effect {
            OnHitReactionEffect::Rage => {
                let raging = Condition::Raging;

                self.ui_handle_event(GameEvent::CharacterReactedToHit {
                    main_line: format!("{} reacted with Rage", reactor_name),
                    detail_lines: vec![],
                    reactor: reactor_id,
                    outcome: HitReactionOutcome {
                        received_condition: Some(raging),
                        offensive: None,
                    },
                })
                .await;

                let reactor = self.characters.get(reactor_id);
                reactor.receive_condition(raging, None, None);
            }
            OnHitReactionEffect::ShieldBash => {
                let mut lines = vec![];

                let outcome = {
                    let attacker = self.characters.get(self.active_character_id);
                    let reactor = self.characters.get(reactor_id);
                    let toughness = attacker.toughness();
                    let roll = roll_d20_with_advantage(0);
                    let attack_mod = reactor.attack_modifier(HandType::MainHand);
                    let res = roll + attack_mod;
                    lines.push(format!(
                        "Rolled: {} (+{} atk mod) = {}, vs toughness={}",
                        roll, attack_mod, res, toughness,
                    ));
                    let condition = if res >= toughness {
                        let degree_of_success = (res - toughness) / 5;
                        let (label, bonus) = match degree_of_success {
                            0 => ("Hit".to_string(), 0),
                            1 => ("Critical hit".to_string(), 1),
                            n => (format!("Critical hit ({n})"), n),
                        };

                        let duration = 1 + bonus;
                        lines.push(label);

                        Some((Condition::Dazed, Some(duration)))
                    } else {
                        None
                    };

                    if let Some((condition, duration)) = condition {
                        let (log_line, _damage) = self.perform_effect_application(
                            ApplyEffect::Condition(ApplyCondition {
                                condition,
                                stacks: None,
                                duration_rounds: duration,
                            }),
                            Some(reactor),
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
                        offensive: Some(outcome),
                    },
                })
                .await;
            }
        }
    }

    async fn perform_end_of_turn_character(&mut self) {
        let character = self.active_character();
        character.has_taken_a_turn_this_round.set(true);
        let name = character.name;
        let conditions = &character.conditions;

        let bleed_stacks = conditions.borrow().get_stacks(&Condition::Bleeding);
        if bleed_stacks > 0 {
            let decay = (bleed_stacks as f32 / 2.0).ceil() as u32;
            let damage = self.perform_losing_health(character, decay);
            self.ui_handle_event(GameEvent::CharacterTookDamage {
                character: character.id(),
                amount: damage,
                source: "Bleeding",
            })
            .await;
            if conditions
                .borrow_mut()
                .lose_stacks(&Condition::Bleeding, decay)
            {
                self.log(format!("{} stopped Bleeding", name)).await;
            }
        }

        let burn_stacks = conditions.borrow().get_stacks(&Condition::Burning);
        if burn_stacks > 0 {
            let damage = self.perform_losing_health(character, burn_stacks);
            self.ui_handle_event(GameEvent::CharacterTookDamage {
                character: character.id(),
                amount: damage,
                source: "Burning",
            })
            .await;
            conditions.borrow_mut().remove(&Condition::Burning);

            let mut adj_others: Vec<&Rc<Character>> = self
                .characters
                .iter()
                .filter(|other| {
                    other.id != character.id && are_adjacent(other.pos(), character.pos())
                })
                .collect();
            if !adj_others.is_empty() {
                let spread = burn_stacks / 2;
                if spread > 0 {
                    adj_others.shuffle();
                    let per_unit = spread / adj_others.len() as u32;
                    let mut remainder = spread % adj_others.len() as u32;
                    for other in &adj_others {
                        let stacks = if remainder > 0 {
                            // Some unlucky ones get burned more than others...
                            remainder -= 1;
                            per_unit + 1
                        } else {
                            per_unit
                        };
                        let burning = Condition::Burning;
                        self.ui_handle_event(GameEvent::CharacterReceivedCondition {
                            character: other.id(),
                            condition: burning,
                        })
                        .await;
                        other.receive_condition(burning, Some(stacks), None);
                    }
                }
                self.log(format!("The fire spread to {} other(s)", adj_others.len()))
                    .await;
            }
        }

        if conditions.borrow_mut().remove(&Condition::Weakened) {
            self.log(format!("{} is no longer Weakened", name)).await;
        }
        if conditions.borrow_mut().remove(&Condition::Raging) {
            self.log(format!("{} stopped Raging", name)).await;
        }

        //let mut new_ap = MAX_ACTION_POINTS;
        let mut gain_ap = ACTION_POINTS_PER_TURN;
        if conditions.borrow().has(&Condition::Adrenalin) {
            gain_ap += 1;
        }
        if conditions.borrow().has(&Condition::NearDeath) {
            gain_ap = gain_ap.saturating_sub(1);
        }
        if conditions.borrow().has(&Condition::Slowed) {
            gain_ap = gain_ap.saturating_sub(SLOWED_AP_PENALTY);
        }
        character.action_points.gain(gain_ap);
        //character.action_points.current.set(new_ap);

        conditions.borrow_mut().remove(&Condition::MainHandExertion);
        conditions.borrow_mut().remove(&Condition::OffHandExertion);
        conditions.borrow_mut().remove(&Condition::ReaperApCooldown);
        let stamina_gain = (character.stamina.max() as f32 / 4.0).ceil() as u32;
        character.stamina.gain(stamina_gain);
        character.regain_movement();
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

#[derive(Debug, Copy, Clone)]
pub struct AttackPrediction {
    pub percentage_chance_deal_damage: u32,
    pub min_damage: u32,
    pub max_damage: u32,
    pub avg_damage: f32,
}

/**
 * Note: lots of duplication with perform_attack
 */
pub fn predict_attack(
    attacker: &Character,
    hand_type: HandType,
    enhancements: &[(&'static str, AttackEnhancementEffect)],
    defender: &Character,
    defender_reaction: Option<OnAttackedReaction>,
    ability_roll_modifier: i32,
) -> AttackPrediction {
    let mut attack_bonus = attack_roll_bonus(
        attacker,
        hand_type,
        defender,
        enhancements,
        defender_reaction,
    );
    attack_bonus.flat_amount += ability_roll_modifier;

    let mut evasion = defender.evasion();

    let attack_modifier = attacker.attack_modifier(hand_type);

    let mut armor_value = defender.protection_from_armor();

    if let Some(reaction) = defender_reaction {
        let bonus_evasion = reaction.effect.bonus_evasion;
        if bonus_evasion > 0 {
            evasion += bonus_evasion;
        }

        let bonus_armor = reaction.effect.bonus_armor;
        if bonus_armor > 0 {
            armor_value += bonus_armor;
        }
    }

    let mut armor_penetrators = vec![];
    let weapon = attacker.weapon(hand_type).unwrap();
    let mut used_arrow = None;

    for (name, effect) in enhancements {
        let penetration = effect.armor_penetration;
        if penetration > 0 {
            armor_penetrators.push((penetration, *name));
        }
        if effect.consume_equipped_arrow {
            assert!(used_arrow.is_none());
            assert!(!weapon.is_melee());
            let stack = attacker.arrows.get().unwrap();
            used_arrow = Some(stack.arrow);
        }
    }

    if let Some(arrow) = used_arrow {
        let penetration = arrow.bonus_penetration;
        if penetration > 0 {
            armor_penetrators.push((penetration, arrow.name));
        }
    }

    if attacker
        .known_passive_skills
        .contains(&PassiveSkill::WeaponProficiency)
    {
        armor_penetrators.push((1, PassiveSkill::WeaponProficiency.name()));
    }

    for (penetration, _label) in armor_penetrators {
        armor_value = armor_value.saturating_sub(penetration);
    }

    let weapon = attacker.weapon(hand_type).unwrap();

    let mut damage_outcomes = vec![];
    let mut min_dmg = None;
    let mut max_dmg = 0;
    let mut percentage_deal_damage = 0;

    for unmodified_roll in 1..=20 {
        let roll_result =
            ((unmodified_roll + attack_modifier) as i32 + attack_bonus.flat_amount) as u32;
        let damage = if roll_result >= evasion.saturating_sub(10) {
            let mut dmg_calculation = weapon.damage as i32;

            if matches!(weapon.grip, WeaponGrip::Versatile) && attacker.off_hand.get().is_empty() {
                let bonus_dmg = 1;
                dmg_calculation += bonus_dmg;
            }

            for (_name, effect) in enhancements {
                let bonus_dmg = effect.bonus_damage;
                if bonus_dmg > 0 {
                    dmg_calculation += bonus_dmg as i32;
                }
            }

            if attacker
                .known_passive_skills
                .contains(&PassiveSkill::Honorless)
            {
                let bonus_dmg = 1;
                if is_target_flanked(attacker.pos(), defender) {
                    dmg_calculation += bonus_dmg as i32;
                }
            }

            if roll_result < evasion {
                dmg_calculation -= (dmg_calculation as f32 * 0.25).ceil() as i32;
            } else if roll_result >= evasion + 10 {
                dmg_calculation += (dmg_calculation as f32 * 0.5).ceil() as i32;
            }

            if armor_value > 0 {
                dmg_calculation -= armor_value as i32;
            }

            dmg_calculation.max(0) as u32
        } else {
            0
        };

        if min_dmg.is_none() {
            min_dmg = Some(damage);
        }
        max_dmg = damage;
        if damage > 0 && percentage_deal_damage == 0 {
            percentage_deal_damage = (21 - unmodified_roll) * 100 / 20;
        }

        damage_outcomes.push(damage);
    }

    let avg_damage = damage_outcomes.iter().map(|dmg| *dmg as f32).sum::<f32>() / 20.0;

    AttackPrediction {
        percentage_chance_deal_damage: percentage_deal_damage,
        min_damage: min_dmg.unwrap(),
        max_damage: max_dmg,
        avg_damage,
    }
}

#[derive(Debug)]
enum AbilityRoll {
    Spell { result: u32, line: String },
    Attack { bonus: i32 },
}

impl AbilityRoll {
    fn unwrap_spell(&self) -> (u32, &str) {
        match self {
            AbilityRoll::Spell { result, line } => (*result, line),
            _ => panic!(),
        }
    }
    fn unwrap_attack_bonus(&self) -> i32 {
        match self {
            AbilityRoll::Attack { bonus } => *bonus,
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
    AttackWasInitiated {
        actor: CharacterId,
        target: CharacterId,
    },
    Attacked(AttackedEvent),
    AbilityWasInitiated {
        actor: CharacterId,
        ability: Ability,
        target: Option<CharacterId>,
    },
    AbilityResolved {
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
    CharacterDying {
        character: CharacterId,
    },
    CharacterDied {
        character: CharacterId,
        new_active: Option<CharacterId>,
    },
    NewActiveCharacter {
        new_active: CharacterId,
    },
    CharacterTookDamage {
        character: CharacterId,
        amount: u32,
        source: &'static str,
    },
    CharacterReceivedCondition {
        character: CharacterId,
        condition: Condition,
    },
}

#[derive(Debug, Clone)]
pub struct AttackedEvent {
    pub attacker: CharacterId,
    pub target: CharacterId,
    pub outcome: AttackOutcome,
    pub detail_lines: Vec<String>,
    pub area_outcomes: Option<Vec<(CharacterId, AbilityTargetOutcome)>>,
}

#[derive(Debug, Copy, Clone)]
pub enum AttackOutcome {
    Hit(u32, AttackHitType),
    Dodge,
    Block,
    Parry,
    Miss,
}

#[derive(Debug, Copy, Clone)]
pub enum AttackHitType {
    Regular,
    Graze,
    Critical,
}

#[derive(Debug, Copy, Clone)]
pub struct HitReactionOutcome {
    pub received_condition: Option<Condition>,
    pub offensive: Option<OffensiveHitReactionOutcome>,
}

#[derive(Debug, Copy, Clone)]
pub struct OffensiveHitReactionOutcome {
    pub inflicted_condition: Option<(Condition, Option<u32>)>,
}

#[derive(Debug, Clone)]
pub enum AbilityTargetOutcome {
    HitEnemy {
        damage: Option<u32>,
        graze: bool,
        applied_effects: Vec<ApplyEffect>,
    },
    AttackedEnemy(AttackedEvent),
    Resisted,
    AffectedAlly {
        healing: Option<u32>,
    },
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
    modifier: AbilityRollType,
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
    modifier: AbilityRollType,
) -> f32 {
    let bonus = ability_roll_bonus(caster, defender, enhancements, modifier);

    let def = match defense_type {
        DefenseType::Will => defender.will(),
        DefenseType::Evasion => defender.evasion(),
        DefenseType::Toughness => defender.toughness(),
    };

    let modifier_value = match modifier {
        AbilityRollType::Spell => caster.spell_modifier() as i32,
        AbilityRollType::Attack(bonus) => caster.attack_modifier(HandType::MainHand) as i32 + bonus,
    };

    let target = (def as i32 - modifier_value).max(1) as u32;
    probability_of_d20_reaching(target, bonus)
}

#[derive(Clone)]
// TODO: Remove the CharacterId from the tuple? Not needed anymore now that Character has interior mutability
pub struct Characters(Vec<(CharacterId, Rc<Character>)>);

impl Characters {
    pub fn new(characters: Vec<Character>) -> Self {
        let round_length = characters.len() as u32;
        Self(
            characters
                .into_iter()
                .enumerate()
                .map(|(i, mut ch)| {
                    let id = i as CharacterId;
                    ch.id = Some(id);
                    ch.index_in_round = Some(i as u32);
                    ch.round_length = Some(round_length);
                    (id, Rc::new(ch))
                })
                .collect(),
        )
    }

    fn next_id(&self, current_id: CharacterId) -> CharacterId {
        for ch in self.iter() {
            dbg!(ch.name, &ch.has_taken_a_turn_this_round);
            if !ch.has_taken_a_turn_this_round.get() {
                return ch.id();
            }
        }
        return self.0[0].0;

        /*
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
         */
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
    Condition(ApplyCondition),
    GainStamina(u32),
    PerBleeding {
        damage: u32,
        caster_healing_percentage: u32,
    },
    ConsumeCondition {
        condition: Condition,
    },
}

impl Display for ApplyEffect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApplyEffect::RemoveActionPoints(n) => f.write_fmt(format_args!("-{n} AP")),
            ApplyEffect::GainStamina(n) => f.write_fmt(format_args!("+{n} stamina")),
            ApplyEffect::Condition(apply_condition) => {
                f.write_fmt(format_args!("{}", apply_condition.condition.name()))
            }
            ApplyEffect::PerBleeding {
                damage,
                caster_healing_percentage,
            } => {
                f.write_fmt(format_args!("{} damage per Bleeding", damage))?;

                // TODO mention healing? (where is this shown?)
                Ok(())
            }
            ApplyEffect::ConsumeCondition { condition } => {
                f.write_fmt(format_args!("-{}", condition.name()))
            }
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
    pub required_circumstance: Option<AttackCircumstance>,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum OnAttackedReactionId {
    Parry,
    SideStep,
    Block,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct OnAttackedReactionEffect {
    pub bonus_evasion: u32,
    pub bonus_armor: u32,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct OnHitReaction {
    pub name: &'static str,
    pub description: &'static str,
    pub icon: IconId,
    pub action_point_cost: u32,
    pub stamina_cost: u32,
    pub effect: OnHitReactionEffect,
    pub required_circumstance: Option<AttackCircumstance>,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum AttackCircumstance {
    Melee,
    Ranged,
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
            AttackHitEffect::Apply(apply_effect) => {
                f.write_fmt(format_args!("Target: {}", apply_effect))
            }
            AttackHitEffect::SkipExertion => f.write_fmt(format_args!("No exertion")),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Hash, Eq)]
pub enum Condition {
    Protected,
    Dazed,
    Bleeding,
    Burning,
    Blinded,
    Braced,
    Raging,
    Distracted,
    Weakened,
    MainHandExertion,
    OffHandExertion,
    Encumbered,
    NearDeath,
    Dead,
    Slowed,
    Exposed,
    Hindered,
    ReaperApCooldown,
    BloodRage,
    ArcaneSurge,
    ThrillOfBattle,
    Adrenalin,
    ArcaneProwess,
}

impl Condition {
    pub const fn name(&self) -> &'static str {
        use Condition::*;
        match self {
            Protected => "Protected",
            Dazed => "Dazed",
            Bleeding => "Bleeding",
            Burning => "Burning",
            Blinded => "Blinded",
            Braced => "Braced",
            Raging => "Raging",
            Distracted => "Distracted",
            Weakened => "Weakened",
            MainHandExertion => "Exerted (main-hand)",
            OffHandExertion => "Exerted (off-hand)",
            Encumbered => "Encumbered",
            NearDeath => "Near-death",
            Dead => "Dead",
            Slowed => "Slowed",
            Exposed => "Exposed",
            Hindered => "Hindered",
            ReaperApCooldown => "Reaper",
            BloodRage => "Blood rage",
            ArcaneSurge => "Arcane surge",
            ThrillOfBattle => "Thrill of battle",
            Adrenalin => "Adrenalin",
            ArcaneProwess => "Arcane prowess",
        }
    }

    pub const fn description(&self) -> &'static str {
        use Condition::*;
        match self {
            Dazed => "-3 evasion and attacks with disadvantage",
            Blinded => "Disadvantage on dice rolls; always counts as Flanked when being attacked",
            Raging => "Gains advantage on melee attack rolls until end of turn",
            Slowed => "Gains 1 less AP per turn",
            Exposed => "-3 to all defenses",
            Hindered => "Half movement speed",
            Protected => "+x armor against the next attack",
            Bleeding => "End of turn: 50% stacks decay, lose 1 health for each decayed",
            Burning => "End of turn: lose x health; lose all stacks; 50% of them are distributed evenly to adjacent entities",
            Braced => "Gain +3 evasion against the next incoming attack",
            Distracted => "-6 evasion against the next incoming attack",
            Weakened => "-x to all defenses and dice rolls",
            MainHandExertion => "-x on further similar actions",
            OffHandExertion => "-x on further similar actions",
            Encumbered => "-x to Evasion and -x to dice rolls",
            NearDeath => "< 25% HP: Reduced AP, disadvantage on dice rolls, enemies have advantage",
            Dead => "This character has reached 0 HP and is dead",
            ReaperApCooldown => "Can not gain more AP from Reaper this turn",
            BloodRage => "+3 attack modifier (from passive skill)",
            ArcaneSurge => "+3 spell modifier (from passive skill)",
            ThrillOfBattle => "+3 attack/spell modifier (from passive skill)",
            Adrenalin => "Gains 1 more AP per turn",
            ArcaneProwess => "+5 spell modifier"
        }
    }

    pub const fn is_positive(&self) -> bool {
        use Condition::*;
        match self {
            Protected => true,
            Dazed => false,
            Bleeding => false,
            Burning => false,
            Blinded => false,
            Braced => true,
            Raging => true,
            Distracted => false,
            Weakened => false,
            MainHandExertion => false,
            OffHandExertion => false,
            Encumbered => false,
            NearDeath => false,
            Dead => false,
            Slowed => false,
            Exposed => false,
            Hindered => false,
            ReaperApCooldown => false,
            BloodRage => true,
            ArcaneSurge => true,
            ThrillOfBattle => true,
            Adrenalin => true,
            ArcaneProwess => true,
        }
    }
}

const PROTECTED_ARMOR_BONUS: u32 = 1;
const BRACED_DEFENSE_BONUS: u32 = 3;
const DISTRACTED_DEFENSE_PENALTY: u32 = 6;
const DAZED_EVASION_PENALTY: u32 = 3;
const EXPOSED_DEFENSE_PENALTY: u32 = 3;
const SLOWED_AP_PENALTY: u32 = 1;

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct ConditionInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub is_positive: bool,
    pub stacks: Option<u32>,
    pub remaining_rounds: Option<u32>,
}

impl Display for ConditionInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)?;
        if let Some(stacks) = self.stacks {
            write!(f, " x {}", stacks)?;
        }
        if let Some(remaining_rounds) = self.remaining_rounds {
            write!(f, " (remaining: {})", remaining_rounds)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
struct ConditionState {
    stacks: Option<u32>,
    ends_at: Option<u32>,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct ApplyCondition {
    pub condition: Condition,
    pub stacks: Option<u32>,
    pub duration_rounds: Option<u32>,
}

impl ApplyCondition {
    pub const fn new(condition: Condition) -> Self {
        Self {
            condition,
            stacks: None,
            duration_rounds: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct Conditions {
    map: IndexMap<Condition, ConditionState>,
}

impl Conditions {
    pub fn remove(&mut self, condition: &Condition) -> bool {
        self.map.shift_remove(condition).is_some()
    }

    pub fn get(&self, condition: &Condition) -> Option<&ConditionState> {
        self.map.get(condition)
    }

    pub fn has(&self, condition: &Condition) -> bool {
        self.map.contains_key(condition)
    }

    pub fn add(&mut self, condition: Condition) {
        self.map.insert(
            condition,
            ConditionState {
                stacks: None,
                ends_at: None,
            },
        );
    }

    fn maybe_expire(&mut self, game_time: u32) {
        self.map.retain(|condition, state| {
            let keep = state
                .ends_at
                .map(|ends_at| ends_at > game_time)
                .unwrap_or(true);
            println!("{:?}: {:?} {:?}", condition, state, keep);
            keep
        });
    }

    pub fn add_or_remove(&mut self, condition: Condition, add: bool) {
        if add {
            self.add(condition);
        } else {
            self.remove(&condition);
        }
    }

    pub fn get_stacks(&self, condition: &Condition) -> u32 {
        self.map
            .get(condition)
            .map(|state| state.stacks.unwrap())
            .unwrap_or(0)
    }

    pub fn set_stacks(&mut self, condition: Condition, stacks: u32) {
        if stacks == 0 {
            self.map.shift_remove(&condition);
        } else if let Some(state) = self.map.get_mut(&condition) {
            *state.stacks.as_mut().unwrap() = stacks;
        } else {
            self.map.insert(
                condition,
                ConditionState {
                    stacks: Some(stacks),
                    ends_at: None,
                },
            );
        }
    }

    pub fn lose_stacks(&mut self, condition: &Condition, stacks: u32) -> bool {
        let current_stacks = self
            .map
            .get_mut(condition)
            .unwrap()
            .stacks
            .as_mut()
            .unwrap();
        *current_stacks = current_stacks.saturating_sub(stacks);
        if *current_stacks == 0 {
            self.map.shift_remove(condition);
            true
        } else {
            false
        }
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
        total_distance: f32,
        // Including start position, all the way to the destination
        positions: Vec<Position>,
        extra_cost: u32,
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

impl ActionTarget {
    pub fn unwrap_position(&self) -> Position {
        match self {
            Self::Position(pos) => *pos,
            _ => panic!(),
        }
    }
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
    pub fn action_point_cost(&self) -> i32 {
        match self {
            BaseAction::Attack(attack) => attack.action_point_cost as i32,
            BaseAction::UseAbility(ability) => ability.action_point_cost as i32,
            BaseAction::Move => 0,
            BaseAction::ChangeEquipment => 1,
            BaseAction::UseConsumable => 1,
            BaseAction::EndTurn => -(ACTION_POINTS_PER_TURN as i32),
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
    pub requirement: Option<EquipmentRequirement>,

    pub roll: Option<AbilityRollType>,
    pub target: AbilityTarget,
    pub possible_enhancements: [Option<AbilityEnhancement>; 3],
    pub animation_color: Color,
}

impl Ability {
    pub fn requires_melee_weapon(&self) -> bool {
        matches!(
            self.requirement,
            Some(EquipmentRequirement::Weapon(WeaponType::Melee))
        )
    }

    pub fn requires_shield(&self) -> bool {
        matches!(self.requirement, Some(EquipmentRequirement::Shield))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum EquipmentRequirement {
    Weapon(WeaponType),
    Shield,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum AbilityId {
    SweepAttack,
    LungeAttack,
    Brace,
    Scream,
    ShackledMind,
    MindBlast,
    InflictWounds,
    Heal,
    HealingNova,
    SelfHeal,
    HealingRain,
    Fireball,
    SearingLight,
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
pub enum AbilityRollType {
    Spell,
    Attack(i32),
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum AbilityEffect {
    Negative(AbilityNegativeEffect),
    Positive(AbilityPositiveEffect),
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct SpellNegativeEffect {
    pub defense_type: Option<DefenseType>,
    pub damage: Option<AbilityDamage>,
    pub on_hit: Option<[Option<ApplyEffect>; 2]>,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum AbilityNegativeEffect {
    Spell(SpellNegativeEffect),
    PerformAttack,
}

impl AbilityNegativeEffect {
    fn unwrap_spell(&self) -> &SpellNegativeEffect {
        match self {
            AbilityNegativeEffect::Spell(spell_enemy_effect) => spell_enemy_effect,
            AbilityNegativeEffect::PerformAttack => panic!(),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum AbilityDamage {
    Static(u32),
    AtLeast(u32),
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct AbilityPositiveEffect {
    pub healing: u32,
    pub apply: Option<[Option<ApplyEffect>; 2]>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum AbilityTarget {
    Enemy {
        reach: AbilityReach,
        effect: AbilityNegativeEffect,
        impact_area: Option<(Range, AreaTargetAcquisition, AbilityNegativeEffect)>,
    },

    Ally {
        range: Range,
        effect: AbilityPositiveEffect,
    },

    Area {
        range: Range,
        area_effect: AreaEffect,
    },

    None {
        self_area: Option<AreaEffect>,
        self_effect: Option<AbilityPositiveEffect>,
    },
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct AreaEffect {
    pub radius: Range,
    pub acquisition: AreaTargetAcquisition,
    pub effect: AbilityEffect,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum AreaTargetAcquisition {
    Enemies,
    Allies,
    Everyone,
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
            AbilityTarget::None { .. } => None,
        }
    }

    fn base_radius(&self) -> Option<Range> {
        match self {
            AbilityTarget::None { self_area, .. } => {
                self_area.as_ref().map(|area_effect| area_effect.radius)
            }
            _ => None,
        }
    }

    pub fn radius(&self, enhancements: &[AbilityEnhancement]) -> Option<Range> {
        self.base_radius().map(|mut range| {
            for enhancement in enhancements {
                if let Some(e) = enhancement.spell_effect {
                    if e.increased_radius_tenths > 0 {
                        range = range.plusf(e.increased_radius_tenths as f32 * 0.1);
                    }
                }
            }
            range
        })
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
pub struct Fraction {
    pub num: u32,
    pub den: u32,
}

impl Fraction {
    pub const fn new(num: u32, den: u32) -> Self {
        Self { num, den }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct AttackEnhancementEffect {
    pub roll_modifier: i32,
    pub roll_advantage: i32,
    pub bonus_damage: u32,
    pub action_point_discount: u32,
    pub inflict_x_condition_per_damage: Option<(Fraction, Condition)>,
    pub armor_penetration: u32,
    pub range_bonus: u32,

    // TODO Actually handle this
    pub on_self: Option<ApplyEffect>,

    // Gets activated if the attack deals damage
    pub on_damage_effect: Option<AttackEnhancementOnHitEffect>,

    // Gets applied on the target regardless if the attack hits
    pub on_target: Option<ApplyEffect>,

    pub consume_equipped_arrow: bool,
}

impl AttackEnhancementEffect {
    // the impl from #[derive(Default)] is not const
    pub const fn default() -> Self {
        Self {
            action_point_discount: 0,
            bonus_damage: 0,
            roll_advantage: 0,
            on_damage_effect: None,
            roll_modifier: 0,
            inflict_x_condition_per_damage: None,
            armor_penetration: 0,
            range_bonus: 0,
            on_self: None,
            on_target: None,
            consume_equipped_arrow: false,
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
    pub target_on_hit: Option<[Option<ApplyEffect>; 2]>,
    pub area_on_hit: Option<[Option<ApplyEffect>; 2]>,
    pub increased_range_tenths: u32,
    pub increased_radius_tenths: u32,
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
            target_on_hit: None,
            area_on_hit: None,
            increased_range_tenths: 0,
            increased_radius_tenths: 0,
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
        2.0 + self.agility.get() as f32 * 0.2
    }

    fn max_health(&self) -> u32 {
        10 + self.strength.get() * 2
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
pub enum CharacterKind {
    Player(Rc<Party>),
    Bot(Bot),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bot {
    pub behaviour: BotBehaviour,
    pub base_movement: f32,
}

impl CharacterKind {
    pub fn unwrap_bot_behaviour(&self) -> &BotBehaviour {
        match self {
            CharacterKind::Player(..) => panic!(),
            CharacterKind::Bot(bot) => &bot.behaviour,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Party {
    pub money: Cell<u32>,
    pub stash: [Cell<Option<EquipmentEntry>>; 6],
}

impl Party {
    pub fn spend_money(&self, amount: u32) {
        self.money
            .set(self.money.get().checked_sub(amount).unwrap());
    }

    pub fn gain_money(&self, amount: u32) {
        self.money.set(self.money.get() + amount);
    }
}

#[derive(Debug, Clone)]
pub struct Character {
    id: Option<CharacterId>,
    index_in_round: Option<u32>,
    current_game_time: Cell<u32>,
    round_length: Option<u32>,
    pub has_taken_a_turn_this_round: Cell<bool>,

    pub name: &'static str,
    pub portrait: PortraitId,

    pub sprite: SpriteId,
    pub kind: CharacterKind,
    pub position: Cell<Position>,
    pub base_attributes: Attributes,
    pub health: NumberedResource,
    pub mana: NumberedResource,

    // How many cells you can move per AP
    pub base_move_speed: Cell<f32>,
    // How many more cells can you move free of cost, this turn
    pub remaining_movement: Cell<f32>,

    pub capacity: Cell<u32>,
    pub inventory: [Cell<Option<EquipmentEntry>>; 6],
    pub armor_piece: Cell<Option<ArmorPiece>>,
    main_hand: Cell<Hand>,
    off_hand: Cell<Hand>,
    pub arrows: Cell<Option<ArrowStack>>,
    conditions: RefCell<Conditions>,
    pub action_points: NumberedResource,
    pub stamina: NumberedResource,
    pub known_attack_enhancements: Vec<AttackEnhancement>,
    pub known_actions: RefCell<Vec<BaseAction>>,
    pub known_attacked_reactions: Vec<OnAttackedReaction>,
    pub known_on_hit_reactions: Vec<OnHitReaction>,
    pub known_ability_enhancements: Vec<AbilityEnhancement>,
    pub known_passive_skills: Vec<PassiveSkill>,

    pub is_engaged_by: RefCell<HashMap<CharacterId, Rc<Character>>>,
    engagement_target: Cell<Option<CharacterId>>,

    changed_equipment_listeners: RefCell<Vec<Weak<Cell<bool>>>>,
}

impl Character {
    pub fn new(
        kind: CharacterKind,
        name: &'static str,
        portrait: PortraitId,
        sprite: SpriteId,
        base_attributes: Attributes,
        position: Position,
    ) -> Self {
        let max_health = base_attributes.max_health();
        let max_mana = base_attributes.max_mana();

        let move_speed = if let CharacterKind::Bot(bot) = &kind {
            bot.base_movement
        } else {
            base_attributes.move_speed()
        };

        let max_stamina = base_attributes.max_stamina();
        let capacity = base_attributes.capacity();
        let action_points = NumberedResource::new(MAX_ACTION_POINTS);
        action_points.current.set(ACTION_POINTS_PER_TURN);
        Self {
            id: None,
            index_in_round: None,
            round_length: None,
            has_taken_a_turn_this_round: Cell::new(false),
            portrait,
            sprite,
            kind,
            position: Cell::new(position),
            name,
            base_attributes,
            health: NumberedResource::new(max_health),
            mana: NumberedResource::new(max_mana),
            base_move_speed: Cell::new(move_speed),
            remaining_movement: Cell::new(0.0),
            capacity: Cell::new(capacity),
            inventory: Default::default(),
            armor_piece: Default::default(),
            main_hand: Default::default(),
            off_hand: Default::default(),
            arrows: Default::default(),
            conditions: Default::default(),
            current_game_time: Default::default(),
            action_points,
            stamina: NumberedResource::new(max_stamina),
            known_attack_enhancements: Default::default(),
            known_actions: RefCell::new(vec![
                BaseAction::Move,
                BaseAction::Attack(AttackAction {
                    hand: HandType::MainHand,
                    // the action point cost is populated (later) by the equipped weapon
                    action_point_cost: 0,
                }),
                BaseAction::ChangeEquipment,
                BaseAction::UseConsumable,
                BaseAction::EndTurn,
            ]),
            known_attacked_reactions: Default::default(),
            known_on_hit_reactions: Default::default(),
            known_ability_enhancements: Default::default(),
            known_passive_skills: Default::default(),
            is_engaged_by: Default::default(),
            engagement_target: Default::default(),
            changed_equipment_listeners: Default::default(),
        }
    }

    pub fn learn_ability(&self, ability: Ability) {
        self.known_actions
            .borrow_mut()
            .push(BaseAction::UseAbility(ability));
    }

    fn set_current_game_time(&self, game_time: u32) {
        self.current_game_time.set(game_time);
        self.conditions.borrow_mut().maybe_expire(game_time);
    }

    pub fn party_money(&self) -> u32 {
        match &self.kind {
            CharacterKind::Player(party) => party.money.get(),
            CharacterKind::Bot(..) => panic!(),
        }
    }

    pub fn party_stash(&self) -> &[Cell<Option<EquipmentEntry>>; 6] {
        match &self.kind {
            CharacterKind::Player(party) => &party.stash,
            CharacterKind::Bot(..) => panic!(),
        }
    }

    fn on_health_changed(&self) {
        let health_ratio = self.health.ratio();
        let has_blood_rage_passive = self.known_passive_skills.contains(&PassiveSkill::BloodRage);

        if has_blood_rage_passive && health_ratio <= 0.5 {
            self.conditions.borrow_mut().add(Condition::BloodRage);
        } else {
            self.conditions.borrow_mut().remove(&Condition::BloodRage);
        }
        if !has_blood_rage_passive && health_ratio < 0.25 {
            self.conditions.borrow_mut().add(Condition::NearDeath);
        } else {
            self.conditions.borrow_mut().remove(&Condition::NearDeath);
        }

        if self.health.current() == 0 {
            self.conditions.borrow_mut().remove(&Condition::NearDeath);
            self.conditions.borrow_mut().add(Condition::Dead);
        }
    }

    fn on_mana_changed(&self) {
        let add = self
            .known_passive_skills
            .contains(&PassiveSkill::ArcaneSurge)
            && self.mana.ratio() <= 0.5;
        self.conditions
            .borrow_mut()
            .add_or_remove(Condition::ArcaneSurge, add);
    }

    fn regain_movement(&self) {
        dbg!((self.name, self.move_speed()));
        self.remaining_movement.set(self.move_speed());
    }

    fn spend_movement(&self, distance: f32) {
        dbg!((self.name, &self.remaining_movement));
        let remaining = self.remaining_movement.get();
        assert!(distance > 0.0 && distance <= remaining);
        dbg!(("Spend movement", distance, remaining));
        self.remaining_movement.set(remaining - distance);
        dbg!(&self.remaining_movement);
        println!("end of spend_movement()");
    }

    fn maybe_gain_resources_from_reaper(&self, num_killed: u32) -> Option<(u32, u32)> {
        if self.known_passive_skills.contains(&PassiveSkill::Reaper) {
            let sta = self.stamina.gain(num_killed);
            let ap = if self.conditions.borrow().has(&Condition::ReaperApCooldown) {
                0
            } else {
                self.action_points.gain(1)
            };
            self.receive_condition(Condition::ReaperApCooldown, None, None);
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
        if self.conditions.borrow().has(&Condition::Hindered) {
            speed /= 2.0;
        }
        speed
    }

    pub fn player_controlled(&self) -> bool {
        matches!(self.kind, CharacterKind::Player(..))
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
        self.on_health_changed();
        self.stamina.change_max_value_to(attr.max_stamina());
        self.mana.change_max_value_to(attr.max_mana());
        self.on_mana_changed();
        self.capacity.set(attr.capacity());
        self.base_move_speed.set(attr.move_speed());
    }

    pub fn is_dead(&self) -> bool {
        self.conditions.borrow().has(&Condition::Dead)
    }

    pub fn listen_to_changed_equipment(&self) -> Rc<Cell<bool>> {
        let signal = Rc::new(Cell::new(false));
        let weak = Rc::downgrade(&signal);
        self.changed_equipment_listeners.borrow_mut().push(weak);
        signal
    }

    pub fn can_equipment_fit(&self, equipment: EquipmentEntry, role: EquipmentSlotRole) -> bool {
        if matches!(
            role,
            EquipmentSlotRole::Inventory(..) | EquipmentSlotRole::PartyStash(..)
        ) {
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
            EquipmentEntry::Arrows(..) => role == EquipmentSlotRole::Arrows,
            _ => false,
        }
    }

    fn lose_protected(&self) -> bool {
        self.conditions.borrow_mut().remove(&Condition::Protected)
    }

    fn lose_distracted(&self) -> bool {
        self.conditions.borrow_mut().remove(&&Condition::Distracted)
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

    pub fn condition_infos(&self) -> Vec<ConditionInfo> {
        let mut result = vec![];

        for (condition, state) in self.conditions.borrow().map.iter() {
            let remaining_rounds = state.ends_at.map(|ends_at| {
                let remaining = ends_at - self.current_game_time.get();
                (remaining as f32 / self.round_length.unwrap() as f32).ceil() as u32
            });
            let info = ConditionInfo {
                name: condition.name(),
                description: condition.description(),
                is_positive: condition.is_positive(),
                stacks: state.stacks,
                remaining_rounds,
            };
            result.push(info);
        }

        result
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

    pub fn attack_range(
        &self,
        hand: HandType,
        enhancements: impl Iterator<Item = AttackEnhancementEffect>,
    ) -> Range {
        let mut modifier = 0;
        for enhancement in enhancements {
            modifier += enhancement.range_bonus;
        }
        let weapon = self.weapon(hand).unwrap();
        if modifier > 0 {
            weapon.range.into_range().plus(modifier)
        } else {
            // Avoid adding the modifier unless necessary, as it could convert a melee range to a float range
            // which is less trustable for drawing ranges in the grid
            weapon.range.into_range()
        }
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
            EquipmentSlotRole::Arrows => self.arrows.get().map(EquipmentEntry::Arrows),
            EquipmentSlotRole::Inventory(idx) => self.inventory[idx].get(),
            EquipmentSlotRole::PartyStash(idx) => self.party_stash()[idx].get(),
        }
    }

    fn update_encumbrance(&self) {
        let encumbrance = self.equipment_weight().saturating_sub(self.capacity.get());
        self.conditions
            .borrow_mut()
            .set_stacks(Condition::Encumbered, encumbrance as u32);
    }

    fn has_any_consumable_in_inventory(&self) -> bool {
        self.inventory
            .iter()
            .any(|entry| matches!(entry.get(), Some(EquipmentEntry::Consumable(..))))
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

        for action in self.known_actions.borrow_mut().iter_mut() {
            if let BaseAction::Attack(attack) = action {
                attack.action_point_cost = weapon.action_point_cost;
            }
        }

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
            EquipmentSlotRole::Arrows => match entry {
                Some(EquipmentEntry::Arrows(stack)) => self.arrows.set(Some(stack)),
                None => self.arrows.set(None),
                _ => panic!(),
            },
            EquipmentSlotRole::Inventory(i) => self.inventory[i].set(entry),
            EquipmentSlotRole::PartyStash(i) => self.party_stash()[i].set(entry),
        }

        self.on_changed_equipment();
    }

    pub fn swap_equipment_slots(&self, from: EquipmentSlotRole, to: EquipmentSlotRole) {
        let from_content = self.equipment(from);
        let to_content = self.equipment(to);

        match (from_content, to_content) {
            (Some(EquipmentEntry::Arrows(from_arrow)), Some(EquipmentEntry::Arrows(to_arrow))) => {
                if from_arrow.arrow == to_arrow.arrow {
                    // Merge the stacks
                    let quantity = from_arrow.quantity + to_arrow.quantity;
                    self.set_equipment(
                        Some(EquipmentEntry::Arrows(ArrowStack::new(
                            to_arrow.arrow,
                            quantity,
                        ))),
                        to,
                    );
                    self.set_equipment(None, from);
                    return;
                }
            }
            _ => {}
        }
        self.set_equipment(from_content, to);
        self.set_equipment(to_content, from);
    }

    pub fn try_gain_equipment(&self, entry: EquipmentEntry) -> bool {
        for slot in &self.inventory {
            if slot.get().is_none() {
                slot.set(Some(entry));
                self.on_changed_equipment();
                return true;
            }
        }

        false
    }

    pub fn has_space_in_inventory(&self) -> bool {
        self.inventory.iter().any(|slot| slot.get().is_none())
    }

    pub fn spend_one_arrow(&self) {
        let stack = self.arrows.get().unwrap();
        let quantity = stack.quantity;
        assert!(quantity > 0);
        if quantity == 1 {
            self.arrows.set(None);
        } else {
            self.arrows.set(Some(ArrowStack {
                arrow: stack.arrow,
                quantity: quantity - 1,
            }));
        }
        self.on_changed_equipment();
    }

    pub fn attack_action_point_cost(&self, hand: HandType) -> u32 {
        self.hand(hand).get().weapon.unwrap().action_point_cost
    }

    pub fn attack_reaches(
        &self,
        hand: HandType,
        target_position: Position,
        enhancements: impl Iterator<Item = AttackEnhancementEffect>,
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
            WeaponRange::Ranged(range) => {
                let mut modifier = 0;
                for enhancement in enhancements {
                    modifier += enhancement.range_bonus;
                }

                if within_range_squared(
                    (range as f32 + modifier as f32).powf(2.0),
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
                    (weapon_range.into_range(), ActionReach::No)
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
        self.known_actions.borrow().to_vec()
    }

    pub fn usable_attack_action(&self) -> Option<AttackAction> {
        for action in self.known_actions.borrow().iter() {
            if self.can_use_action(*action) {
                if let BaseAction::Attack(attack_action) = action {
                    return Some(*attack_action);
                }
            }
        }
        None
    }

    pub fn attack_action(&self) -> Option<AttackAction> {
        for action in self.known_actions.borrow().iter() {
            if let BaseAction::Attack(attack_action) = action {
                return Some(*attack_action);
            }
        }
        None
    }

    pub fn usable_actions(&self) -> Vec<BaseAction> {
        self.known_actions
            .borrow()
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
        let sta = self.stamina.current();
        let ap = self.action_points.current();
        match action {
            BaseAction::Attack(attack) => {
                matches!(self.weapon(attack.hand), Some(weapon) if ap >= weapon.action_point_cost)
            }
            BaseAction::UseAbility(ability) => {
                if ability.requires_shield() && self.shield().is_none() {
                    return false;
                }
                if ability.requires_melee_weapon() && !self.has_equipped_melee_weapon() {
                    return false;
                }
                ap >= ability.action_point_cost
                    && self.stamina.current() >= ability.stamina_cost
                    && self.mana.current() >= ability.mana_cost
            }
            BaseAction::Move => self.remaining_movement.get() > 1.0 || sta > 0,
            BaseAction::ChangeEquipment => {
                ap as i32 >= BaseAction::ChangeEquipment.action_point_cost()
            }
            BaseAction::UseConsumable => {
                self.has_any_consumable_in_inventory()
                    && ap as i32 >= BaseAction::UseConsumable.action_point_cost()
            }
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
        let mut usable: Vec<AttackEnhancement> = self
            .known_attack_enhancements(attack_hand)
            .iter()
            .filter_map(|(_, e)| {
                if self.can_use_attack_enhancement(attack_hand, e) {
                    Some(*e)
                } else {
                    None
                }
            })
            .collect();

        if let Some(arrows) = self.arrows.get() {
            assert!(arrows.quantity > 0);
            usable.push(AttackEnhancement {
                name: "Use special arrow",
                description: arrows.arrow.name,
                icon: IconId::RangedAttack,
                weapon_requirement: Some(WeaponType::Ranged),
                effect: AttackEnhancementEffect {
                    consume_equipped_arrow: true,
                    ..AttackEnhancementEffect::default()
                },
                ..AttackEnhancement::default()
            })
        }

        usable
    }

    pub fn can_use_opportunity_attack(&self, target: CharacterId) -> bool {
        if !self.known_passive_skills.contains(&PassiveSkill::Vigilant) {
            if let Some(engaged_target) = self.engagement_target.get() {
                if engaged_target != target {
                    // If you're engaging someone else, you're focused on that and miss the opportunity
                    return false;
                }
            }
        }

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
        if let Some(weapon) = &self.weapon(HandType::MainHand) {
            if let Some(reaction) = weapon.on_attacked_reaction {
                known.push((weapon.name.to_string(), reaction));
            }
        }
        if let Some(shield) = &self.shield() {
            if let Some(reaction) = shield.on_attacked_reaction {
                known.push((shield.name.to_string(), reaction));
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
        let allowed = match reaction.required_circumstance {
            None => true,
            Some(AttackCircumstance::Melee) => is_within_melee,
            Some(AttackCircumstance::Ranged) => !is_within_melee,
        };
        let ap = self.action_points.current();
        ap >= reaction.action_point_cost
            && self.stamina.current() >= reaction.stamina_cost
            && allowed
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
            if self.conditions.borrow().has(&Condition::Raging) {
                // Can't use this reaction while already raging
                return false;
            }
        }
        let allowed = match reaction.required_circumstance {
            None => true,
            Some(AttackCircumstance::Melee) => is_within_melee,
            Some(AttackCircumstance::Ranged) => !is_within_melee,
        };
        let ap = self.action_points.current();
        ap >= reaction.action_point_cost
            && self.stamina.current() >= reaction.stamina_cost
            && allowed
    }

    pub fn knows_ability(&self, id: AbilityId) -> bool {
        self.known_actions
            .borrow()
            .iter()
            .any(|action| match action {
                BaseAction::UseAbility(ability) => ability.id == id,
                _ => false,
            })
    }

    pub fn known_abilities(&self) -> Vec<Ability> {
        self.known_actions
            .borrow()
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

        let conditions = self.conditions.borrow();
        if conditions.has(&Condition::ArcaneSurge) {
            res += 3;
        }
        if conditions.has(&Condition::ThrillOfBattle) {
            res += 3;
        }
        if conditions.has(&Condition::ArcaneProwess) {
            res += 5;
        }

        res
    }

    fn is_dazed(&self) -> bool {
        self.conditions.borrow().get(&Condition::Dazed).is_some()
        //self.conditions.borrow().dazed > 0
    }

    pub fn evasion(&self) -> u32 {
        let mut res = 10;
        res += self.evasion_from_agility();
        res += self.evasion_from_intellect();
        res += self.shield().map(|shield| shield.evasion).unwrap_or(0);

        let conditions = self.conditions.borrow();
        if conditions.has(&Condition::Braced) {
            res += BRACED_DEFENSE_BONUS;
        }

        if conditions.has(&Condition::Distracted) {
            res = res.saturating_sub(DISTRACTED_DEFENSE_PENALTY);
        }
        if self.is_dazed() {
            res = res.saturating_sub(DAZED_EVASION_PENALTY);
        }

        res = res.saturating_sub(conditions.get_stacks(&Condition::Encumbered));

        if conditions.has(&Condition::Exposed) {
            res = res.saturating_sub(EXPOSED_DEFENSE_PENALTY);
        }

        let weakened = conditions.get_stacks(&Condition::Weakened);
        if weakened > 0 {
            res = res.saturating_sub(weakened)
        }

        res
    }

    fn evasion_from_agility(&self) -> u32 {
        let mut bonus = self.agility();

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
        if conditions.has(&Condition::Exposed) {
            res = res.saturating_sub(EXPOSED_DEFENSE_PENALTY);
        }
        res = res.saturating_sub(conditions.get_stacks(&Condition::Weakened));
        res
    }

    pub fn toughness(&self) -> u32 {
        let mut res = 10 + self.strength() * 2;
        let conditions = self.conditions.borrow();
        if conditions.has(&Condition::Exposed) {
            res = res.saturating_sub(EXPOSED_DEFENSE_PENALTY);
        }
        res = res.saturating_sub(conditions.get_stacks(&Condition::Weakened));

        res
    }

    pub fn protection_from_armor(&self) -> u32 {
        let mut protection = 0;
        if let Some(armor) = self.armor_piece.get() {
            protection += armor.protection;
        }
        if let Some(shield) = self.shield() {
            protection += shield.armor;
        }

        if let Some(state) = self.conditions.borrow().get(&Condition::Protected) {
            protection += state.stacks.unwrap();
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

        let attack_attribute = self
            .weapon(hand)
            .map(|weapon| weapon.attack_attribute)
            .unwrap_or(AttackAttribute::Strength);
        let physical_attr = match attack_attribute {
            AttackAttribute::Strength => str,
            AttackAttribute::Agility => agi,
            AttackAttribute::Finesse => str.max(agi),
        };

        let mut res = physical_attr + self.intellect();

        let conditions = self.conditions.borrow();
        if conditions.has(&Condition::BloodRage) {
            res += 3;
        }
        if conditions.has(&Condition::ThrillOfBattle) {
            res += 3;
        }

        res
    }

    fn hand_exertion(&self, hand_type: HandType) -> u32 {
        match hand_type {
            HandType::MainHand => self
                .conditions
                .borrow()
                .get_stacks(&Condition::MainHandExertion),
            HandType::OffHand => self
                .conditions
                .borrow()
                .get_stacks(&Condition::OffHandExertion),
        }
    }

    fn outgoing_ability_roll_bonus(
        &self,
        enhancements: &[AbilityEnhancement],
        modifier: AbilityRollType,
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

        if is_target_flanked(self.pos(), target) {
            bonuses.push(("Flanked", RollBonusContributor::FlatAmount(3)));
        }

        let (_range, reach) = self.attack_reaches(
            hand_type,
            target_pos,
            enhancement_effects.iter().map(|(_, e)| *e),
        );

        if let ActionReach::YesButDisadvantage(reason) = reach {
            bonuses.push((reason, RollBonusContributor::Advantage(-1)));
        }

        for (name, effect) in enhancement_effects {
            let adv = effect.roll_advantage;
            if adv != 0 {
                bonuses.push((name, RollBonusContributor::Advantage(adv)));
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
        if conditions.has(&Condition::Raging)
            && self.weapon(hand_type).unwrap().range == WeaponRange::Melee
        {
            bonuses.push(("Raging", RollBonusContributor::Advantage(1)));
        }
        if conditions.has(&Condition::Weakened) {
            // TODO: this seems wrong, shouldn't the penalty be applied here? If not here, then where?
            bonuses.push(("Weakened", RollBonusContributor::OtherNegative));
        }

        let encumbrance_penalty = conditions.get_stacks(&Condition::Encumbered) as i32;
        if encumbrance_penalty > 0 {
            bonuses.push((
                "Encumbered",
                RollBonusContributor::FlatAmount(-(encumbrance_penalty)),
            ));
        }

        if conditions.has(&Condition::NearDeath) {
            bonuses.push(("Near-death", RollBonusContributor::Advantage(-1)));
        }
        if conditions.has(&Condition::Blinded) {
            bonuses.push(("Blinded", RollBonusContributor::Advantage(-1)));
        }

        if conditions.has(&Condition::BloodRage) {
            // applied from attack_modifer()
            bonuses.push(("Blood rage", RollBonusContributor::OtherPositive));
        }
        if conditions.has(&Condition::ThrillOfBattle) {
            // applied from attack_modifer()
            bonuses.push(("Thrill of battle", RollBonusContributor::OtherPositive));
        }

        bonuses
    }

    pub fn outgoing_ability_roll_bonuses(
        &self,
        enhancements: &[AbilityEnhancement],
        modifier: AbilityRollType,
    ) -> Vec<(&'static str, RollBonusContributor)> {
        let is_spell = matches!(modifier, AbilityRollType::Spell);
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

        let conditions = self.conditions.borrow();
        let weakened = conditions.get_stacks(&Condition::Weakened);
        if weakened > 0 {
            bonuses.push((
                "Weakened",
                RollBonusContributor::FlatAmount(-(weakened as i32)),
            ));
        }

        if is_spell && conditions.has(&Condition::ArcaneSurge) {
            // It's applied from spell_modifier()
            bonuses.push(("Arcane surge", RollBonusContributor::OtherPositive));
        }
        if conditions.has(&Condition::ThrillOfBattle) {
            // It's applied from spell_modifier()
            bonuses.push(("Thrill of battle", RollBonusContributor::OtherPositive));
        }

        let encumbrance_penalty = (conditions.get_stacks(&Condition::Encumbered)) as i32;
        if encumbrance_penalty > 0 {
            bonuses.push((
                "Encumbered",
                RollBonusContributor::FlatAmount(-(encumbrance_penalty)),
            ));
        }

        if conditions.has(&Condition::NearDeath) {
            bonuses.push(("Near-death", RollBonusContributor::Advantage(-1)));
        }
        if conditions.has(&Condition::Blinded) {
            bonuses.push(("Blinded", RollBonusContributor::Advantage(-1)));
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
        if conditions.has(&Condition::Weakened) {
            terms.push(("Weakened", RollBonusContributor::OtherPositive));
        }
        if conditions.has(&Condition::Braced) {
            terms.push(("Braced", RollBonusContributor::OtherNegative));
        }
        if conditions.has(&Condition::Distracted) {
            terms.push(("Distracted", RollBonusContributor::OtherPositive));
        }
        if conditions.has(&Condition::NearDeath) {
            terms.push(("Near-death", RollBonusContributor::Advantage(1)))
        }
        if conditions.has(&Condition::Exposed) {
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
        if conditions.has(&Condition::Weakened) {
            terms.push(("Weakened", RollBonusContributor::OtherPositive));
        }
        if conditions.has(&Condition::NearDeath) {
            terms.push(("Near-death", RollBonusContributor::Advantage(1)))
        }
        if conditions.has(&Condition::Exposed) {
            terms.push(("Exposed", RollBonusContributor::OtherPositive));
        }
        terms
    }

    pub fn is_bleeding(&self) -> bool {
        self.conditions.borrow().get(&Condition::Bleeding).is_some()
    }

    pub fn receive_condition(
        &self,
        condition: Condition,
        stacks: Option<u32>,
        ends_at: Option<u32>,
    ) {
        let mut conditions = self.conditions.borrow_mut();

        if let Some(state) = conditions.map.get_mut(&condition) {
            if let Some(ends_at) = ends_at {
                state.ends_at = Some(state.ends_at.unwrap().max(ends_at));
            }
            if let Some(stacks) = stacks {
                *state.stacks.as_mut().unwrap() += stacks;
            }
        } else {
            if condition == Condition::Hindered {
                self.remaining_movement
                    .set(self.remaining_movement.get() * 0.5);
            } else if condition == Condition::Slowed {
                self.action_points.lose(1);
            }

            conditions
                .map
                .insert(condition, ConditionState { stacks, ends_at });
        }
    }

    fn clear_condition(&self, condition: Condition) -> u32 {
        let mut conditions = self.conditions.borrow_mut();

        let prev_stacks = conditions.get_stacks(&condition);

        conditions.remove(&condition);

        prev_stacks
    }
}

fn is_target_flanked(attacker_pos: Position, target: &Character) -> bool {
    let target_is_immune_to_flanking = target
        .known_passive_skills
        .contains(&PassiveSkill::ThrillOfBattle);

    if target_is_immune_to_flanking {
        return false;
    }

    if target.conditions.borrow().has(&Condition::Blinded) {
        return true;
    }

    target
        .is_engaged_by
        .borrow()
        .values()
        .any(|engager| are_flanking_target(attacker_pos, engager.pos(), target.pos()))
}

fn are_flanking_target(attacker: Position, melee_engager: Position, target: Position) -> bool {
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

pub fn within_range_squared(range_squared: f32, source: Position, destination: Position) -> bool {
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

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ArmorPiece {
    pub name: &'static str,
    pub protection: u32,
    pub limit_evasion_from_agi: Option<u32>,
    pub icon: EquipmentIconId,
    pub weight: u32,
    pub equip: EquipEffect,
}

#[derive(Debug, Copy, Clone, PartialEq)]
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
pub struct Arrow {
    pub name: &'static str,
    pub sprite: Option<SpriteId>,
    pub icon: EquipmentIconId,
    pub bonus_penetration: u32,
    pub on_damage_apply: Option<ApplyEffect>,
    pub area_effect: Option<AreaEffect>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Shield {
    pub name: &'static str,
    pub sprite: Option<SpriteId>,
    pub icon: EquipmentIconId,
    pub evasion: u32,
    pub armor: u32,
    pub on_hit_reaction: Option<OnHitReaction>,
    pub on_attacked_reaction: Option<OnAttackedReaction>,
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
            Self::Ranged(range) => f.write_fmt(format_args!("{}", range)),
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

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum EquipmentEntry {
    Weapon(Weapon),
    Shield(Shield),
    Armor(ArmorPiece),
    Arrows(ArrowStack),
    Consumable(Consumable),
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ArrowStack {
    pub arrow: Arrow,
    pub quantity: u32,
}

impl ArrowStack {
    pub fn new(arrow: Arrow, quantity: u32) -> Self {
        Self { arrow, quantity }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Consumable {
    pub name: &'static str,
    pub health_gain: u32,
    pub mana_gain: u32,
    pub effect: Option<ApplyEffect>,
    pub icon: EquipmentIconId,
    pub weight: u32,
}

impl Consumable {
    pub const fn default() -> Self {
        Self {
            name: "",
            health_gain: 0,
            mana_gain: 0,
            effect: None,
            icon: EquipmentIconId::Undefined,
            weight: 0,
        }
    }
}

impl EquipmentEntry {
    pub fn name(&self) -> &'static str {
        match self {
            EquipmentEntry::Weapon(weapon) => weapon.name,
            EquipmentEntry::Shield(shield) => shield.name,
            EquipmentEntry::Armor(armor) => armor.name,
            EquipmentEntry::Consumable(consumable) => consumable.name,
            EquipmentEntry::Arrows(stack) => stack.arrow.name,
        }
    }

    pub fn icon(&self) -> EquipmentIconId {
        match self {
            EquipmentEntry::Weapon(weapon) => weapon.icon,
            EquipmentEntry::Shield(shield) => shield.icon,
            EquipmentEntry::Armor(armor) => armor.icon,
            EquipmentEntry::Consumable(consumable) => consumable.icon,
            EquipmentEntry::Arrows(stack) => stack.arrow.icon,
        }
    }

    fn weight(&self) -> u32 {
        match self {
            EquipmentEntry::Weapon(weapon) => weapon.weight,
            EquipmentEntry::Shield(shield) => shield.weight,
            EquipmentEntry::Armor(armor) => armor.weight,
            EquipmentEntry::Consumable(consumable) => consumable.weight,
            EquipmentEntry::Arrows(..) => 0,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum EquipmentSlotRole {
    MainHand,
    OffHand,
    Armor,
    Arrows,
    Inventory(usize),
    PartyStash(usize),
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
            MainHand | OffHand | Armor | Arrows => true,
            Inventory(..) | PartyStash(..) => false,
        }
    }
}
