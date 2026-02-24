use std::cell::{Cell, RefCell};

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::ops::RemAssign;
use std::rc::{Rc, Weak};
use std::time::SystemTime;

use indexmap::IndexMap;
use macroquad::color::Color;
use macroquad::rand::ChooseRandom;

use crate::bot::BotBehaviour;
use crate::d20::{probability_of_d20_reaching, roll_d20_with_advantage, DiceRollBonus};

use crate::data::PassiveSkill;
use crate::game_ui_connection::{ActionOrSwitchTo, GameUserInterfaceConnection, QuitEvent};
use crate::grid::ParticleShape;
use crate::init_fight_map::GameInitState;
use crate::pathfind::{Occupation, PathfindGrid};
use crate::sounds::SoundId;
use crate::textures::{EquipmentIconId, IconId, PortraitId, SpriteId, StatusId};
use crate::util::{are_entities_within_melee, line_collision, CustomShuffle};

pub type Position = (i32, i32);

pub const MAX_ACTION_POINTS: u32 = 6;
pub const ACTION_POINTS_PER_TURN: u32 = 4;

pub const MOVE_DISTANCE_PER_STAMINA: u32 = 4;

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
        let characters = Characters::new(init_state.characters.clone());

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

    pub async fn run(mut self) -> Result<Vec<Character>, QuitEvent> {
        self.log("The battle begins").await;
        self.log("Round 1").await;

        for character in self.characters.iter() {
            character.on_battle_start();
        }
        for player_char in self.player_characters() {
            player_char.is_part_of_active_group.set(true);
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
                //self.perform_end_of_turn_character().await;

                for character in self.characters.iter() {
                    character.stamina.set_to_max();
                }

                return Ok(self.characters.player_characters());
            }

            let action_or_character_change = self.user_interface.select_action(&self).await?;

            let action = match action_or_character_change {
                ActionOrSwitchTo::Action(action) => action,
                ActionOrSwitchTo::SwitchTo(character_id) => {
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

            let mut turn_ended = false;

            if let Some(action) = action {
                let mut killed_by_action = HashSet::new();
                let action_outcome = self.perform_action(action).await?;

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
                            .await?
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
                            if ap > 0 {
                                self.ui_handle_event(GameEvent::CharacterGainedAP {
                                    character: character.id(),
                                })
                                .await;
                            }
                            let gain_str = match (sta, ap) {
                                (0, _) => format!("{ap} AP"),
                                (_, 0) => format!("{sta} stamina"),
                                _ => format!("{sta} stamina, {ap} AP"),
                            };
                            self.log(format!("{} gained {} (Reaper)", character.name, gain_str))
                                .await;
                        }
                    }
                }
            } else {
                let name = self.active_character().name;
                self.log(format!("|{}| ended their turn", name)).await;

                self.perform_end_of_turn_character().await;
                //let prev_index_in_round = self.active_character().index_in_round.unwrap();
                self.active_character().is_part_of_active_group.set(false);
                self.active_character_id = self.characters.next_id();
                self.active_character().is_part_of_active_group.set(true);
                self.notify_ui_of_new_active_char().await;

                turn_ended = true;
            }

            for ch in self.characters.iter() {
                if let Some((dx, dy)) = ch.is_being_pushed_in_direction.take() {
                    self.perform_character_pushed(ch, dx, dy).await?;
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
                self.active_character_id = self.characters.next_id();
                dbg!(self.active_character_id);
            }

            for ch in self.characters.iter() {
                if ch.is_dead() {
                    println!(
                        "{} is dead. Marking pos {:?} as not occupied",
                        ch.name,
                        ch.pos()
                    );
                    self.pathfind_grid.set_occupied(ch.pos(), None);
                }
            }
            let dead_character_ids = self.characters.remove_dead();

            for dead_id in &dead_character_ids {
                for ch in self.characters.iter() {
                    ch.set_not_engaged_by(*dead_id);
                    ch.set_not_engaging(*dead_id);
                }
            }

            if !dead_character_ids.is_empty() {
                let new_active = if active_character_died {
                    Some(self.active_character_id)
                } else {
                    None
                };
                self.ui_handle_event(GameEvent::CharactersDying {
                    characters: dead_character_ids.clone(),
                })
                .await;
                self.ui_handle_event(GameEvent::CharactersDied {
                    characters: dead_character_ids.clone(),
                    new_active,
                })
                .await;
            }

            // ... but at the same time, we don't want to lie to the UI and claim that the new turn started
            // before the character died.
            if active_character_died {
                self.notify_ui_of_new_active_char().await;
                turn_ended = true;
            }

            if turn_ended {
                let new_round = self
                    .characters
                    .iter()
                    .all(|ch| ch.has_taken_a_turn_this_round.get());

                if new_round {
                    println!("NEW ROUND STARTED!");

                    for character in self.characters.iter() {
                        character.on_new_round();
                    }

                    if self.active_character().player_controlled() {
                        // Player chars can act "simultaneously"
                        for player_char in self.player_characters() {
                            player_char.is_part_of_active_group.set(true);
                        }
                    }

                    self.round_index += 1;

                    self.log(format!("Round {}", self.round_index + 1)).await;
                }

                let game_time = self.current_time();

                for character in self.characters.iter() {
                    character.set_current_game_time(game_time);
                }
            }
        }
    }

    async fn perform_character_pushed(
        &self,
        ch: &Rc<Character>,
        dx: i32,
        dy: i32,
    ) -> Result<(), QuitEvent> {
        assert!((dx, dy) != (0, 0) && (dx == 0 || dy == 0));
        let mut positions = vec![];
        if dx != 0 {
            for i in 0..=dx.abs() {
                positions.push((ch.pos().0 + i * dx.signum(), ch.pos().1));
            }
        }
        if dy != 0 {
            for i in 0..=dy.abs() {
                positions.push((ch.pos().0, ch.pos().1 + i * dy.signum()));
            }
        }

        let original_distance = positions.len();

        let positions: Vec<Position> = positions
            .into_iter()
            .take_while(|pos| self.pathfind_grid.is_free(Some(ch.id()), *pos))
            .collect();

        let reduction_from_collision = original_distance - positions.len();

        if positions.len() > 1 {
            self.ui_handle_event(GameEvent::CharacterReceivedKnockback { character: ch.id() })
                .await;
            self.perform_movement(ch.id(), positions, MovementType::KnockedBack)
                .await?;
        }
        if reduction_from_collision > 0 {
            let collision_damage = reduction_from_collision as u32;
            self.perform_losing_health(ch, collision_damage);
            self.ui_handle_event(GameEvent::CharacterTookDamage {
                character: ch.id(),
                amount: collision_damage,
                source: DamageSource::KnockbackCollision,
            })
            .await;
        }

        Ok(())
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

    async fn perform_action(&mut self, action: Action) -> Result<ActionOutcome, QuitEvent> {
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
                        .reaches_with_attack(
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
                                .await?;

                            dbg!(chooses_to_use_opportunity_attack);

                            if chooses_to_use_opportunity_attack {
                                reactor.set_facing_toward(attacker.pos());

                                self.ui_handle_event(
                                    GameEvent::CharacterReactedWithOpportunityAttack {
                                        reactor: reactor.id(),
                                    },
                                )
                                .await;

                                reactor.action_points.spend(1);

                                let event = Self::perform_attack(
                                    reactor,
                                    HandType::MainHand,
                                    &[],
                                    attacker,
                                    None,
                                    0,
                                    ActionPerformanceMode::Real(self),
                                );

                                self.ui_handle_event(GameEvent::Attacked(event)).await;
                            }
                        }
                    }
                }

                if attacker.is_dead() {
                    Ok(ActionOutcome::Default)
                } else {
                    // TODO: Should not be able to react when flanked?
                    let defender_can_react_to_attack = !defender
                        .usable_on_attacked_reactions(is_within_melee, true)
                        .is_empty();

                    let reaction = if defender_can_react_to_attack {
                        let maybe_self_reaction = self
                            .user_interface
                            .choose_attack_reaction(
                                self,
                                self.active_character_id,
                                target,
                                hand,
                                target,
                                is_within_melee,
                            )
                            .await?;

                        maybe_self_reaction.map(|r| (target, r))
                    } else {
                        let mut maybe_ally_reaction: Option<(u32, OnAttackedReaction)> = None;
                        for ch in self.characters.iter() {
                            let is_ally = ch.player_controlled() == defender.player_controlled();
                            if is_ally && are_entities_within_melee(defender.pos(), ch.pos()) {
                                if !ch
                                    .usable_on_attacked_reactions(is_within_melee, false)
                                    .is_empty()
                                {
                                    let r = self
                                        .user_interface
                                        .choose_attack_reaction(
                                            self,
                                            self.active_character_id,
                                            target,
                                            hand,
                                            ch.id(),
                                            is_within_melee,
                                        )
                                        .await?;
                                    if let Some(r) = r {
                                        maybe_ally_reaction = Some((ch.id(), r));
                                        break;
                                    }
                                }
                            }
                        }

                        maybe_ally_reaction
                    };

                    if let Some((reactor, reaction)) = reaction {
                        self.ui_handle_event(GameEvent::CharacterReactedToAttacked {
                            reactor,
                            with_shield: reaction.used_hand == Some(HandType::OffHand),
                        })
                        .await;
                    }

                    let enhancements: Vec<(&str, AttackEnhancementEffect)> =
                        enhancements.iter().map(|e| (e.name, e.effect)).collect();

                    attacker.set_facing_toward(defender.pos());

                    self.ui_handle_event(GameEvent::AttackWasInitiated {
                        actor: self.active_character_id,
                        target,
                    })
                    .await;

                    let event = Self::perform_attack(
                        attacker,
                        hand,
                        &enhancements,
                        defender,
                        reaction,
                        0,
                        ActionPerformanceMode::Real(self),
                    );
                    self.ui_handle_event(GameEvent::Attacked(event.clone()))
                        .await;

                    let maybe_damage = match event.outcome {
                        AttackOutcome::Hit { damage, .. } => Some(damage),
                        _ => None,
                    };
                    let outcome = if let Some(damage) = maybe_damage {
                        ActionOutcome::AttackHit {
                            victim_id: event.target,
                            damage,
                        }
                    } else {
                        ActionOutcome::Default
                    };
                    Ok(outcome)
                }
            }

            Action::UseAbility {
                ability,
                enhancements,
                target,
            } => {
                let caster = self.characters.get_rc(self.active_character_id);
                let ability_resolved_events = Self::perform_ability(
                    caster,
                    ability,
                    &enhancements,
                    &target,
                    ActionPerformanceMode::Real(self),
                )
                .await;

                let mut enemies_hit = vec![];
                for event in ability_resolved_events {
                    event.enemies_hit(&mut enemies_hit);
                }

                let outcome = if enemies_hit.is_empty() {
                    ActionOutcome::Default
                } else {
                    ActionOutcome::AbilityHitEnemies {
                        victim_ids: enemies_hit,
                    }
                };
                Ok(outcome)
            }

            Action::Move {
                extra_cost,
                positions,
                total_distance,
            } => {
                let character = self.active_character();
                //character.action_points.spend(extra_cost);
                character.stamina.spend(extra_cost);
                let paid_distance = (extra_cost * MOVE_DISTANCE_PER_STAMINA) as f32;
                if total_distance > paid_distance {
                    character.spend_movement(total_distance - paid_distance);
                } else if total_distance < paid_distance {
                    character.gain_movement(paid_distance - total_distance);
                }

                self.perform_movement(self.active_character_id, positions, MovementType::Regular)
                    .await?;
                Ok(ActionOutcome::Default)
            }

            Action::ChangeEquipment { from, to } => {
                let character = self.active_character();
                character.action_points.spend(1);
                character.swap_equipment_slots(from, to);
                Ok(ActionOutcome::Default)
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

                let mut detail_lines = vec![];

                if consumable.health_gain > 0 {
                    let amount = self.perform_gain_health(character, consumable.health_gain);
                    detail_lines.push(format!("gained {} health", amount));
                }
                if consumable.mana_gain > 0 {
                    let amount = character.mana.gain(consumable.mana_gain);
                    character.on_mana_changed();
                    detail_lines.push(format!("gained {} mana", amount));
                }
                if let Some(apply_effect) = consumable.effect {
                    let (applied, line, _damage) =
                        self.perform_effect_application(apply_effect, None, None, character);
                    detail_lines.push(line);
                }

                character.set_equipment(None, slot_role);

                self.ui_handle_event(GameEvent::ConsumableWasUsed {
                    user: character.id(),
                    consumable,
                    detail_lines,
                })
                .await;

                Ok(ActionOutcome::Default)
            }
        }
    }

    async fn ui_handle_event(&self, event: GameEvent) {
        self.user_interface.handle_event(self, event).await
    }

    async fn perform_movement(
        &self,
        character_id: CharacterId,
        mut positions: Vec<Position>,
        movement_type: MovementType,
    ) -> Result<(), QuitEvent> {
        let character = self.characters.get(character_id);
        //dbg!(("perform movement: {:?}", &positions));
        let start_position = positions.remove(0);
        assert!(start_position == character.pos());
        assert!(
            !positions.is_empty(),
            "movement must consist of more than just the start position"
        );

        let mut step_idx = 0;

        while !positions.is_empty() {
            let new_position = positions.remove(0);
            if new_position == character.pos() {
                panic!(
                    "Character {} tried to move 0 distance from {:?}",
                    character.id(),
                    character.pos()
                );
            }

            if !(0..self.pathfind_grid.dimensions().0 as i32).contains(&new_position.0)
                || !(0..self.pathfind_grid.dimensions().1 as i32).contains(&new_position.1)
            {
                panic!(
                    "Character {} tried to move out of the map, from {:?} to {:?}",
                    character.id(),
                    character.pos(),
                    new_position
                )
            }

            for other_char in self.characters.iter() {
                let unfriendly = other_char.player_controlled() != character.player_controlled();
                let leaving_melee = within_meele(character.pos(), other_char.pos())
                    && !within_meele(new_position, other_char.pos());

                if unfriendly && leaving_melee {
                    // Movement opportunity attack
                    if movement_type == MovementType::Regular
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
                            .await?;

                        dbg!(chooses_to_use_opportunity_attack);

                        if chooses_to_use_opportunity_attack {
                            reactor.set_facing_toward(character.pos());

                            self.ui_handle_event(
                                GameEvent::CharacterReactedWithOpportunityAttack {
                                    reactor: reactor.id(),
                                },
                            )
                            .await;

                            reactor.action_points.spend(1);

                            self.ui_handle_event(GameEvent::AttackWasInitiated {
                                actor: reactor.id(),
                                target: character.id(),
                            })
                            .await;

                            let event = Self::perform_attack(
                                reactor,
                                HandType::MainHand,
                                &[],
                                character,
                                None,
                                0,
                                ActionPerformanceMode::Real(self),
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

            if character.is_dead() {
                println!("Breaking out of movement loop as the mover died");
                break;
            }

            let prev_position = character.position.get();
            let id = character.id();

            self.pathfind_grid.set_occupied(prev_position, None);
            self.pathfind_grid
                .set_occupied(new_position, Some(Occupation::Character(id)));

            if movement_type != MovementType::KnockedBack {
                character.set_facing_toward(new_position);
            }

            self.ui_handle_event(GameEvent::Moved {
                character: id,
                from: prev_position,
                to: new_position,
                movement_type,
                step_idx,
            })
            .await;

            character.set_position(new_position);

            step_idx += 1;
        }

        dbg!(character.pos());

        self.on_character_positions_changed();

        Ok(())
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
                    if *player_controlled != character.player_controlled()
                        && are_entities_within_melee(*pos, character.pos())
                    {
                        num_adjacent_enemies += 1;
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
        area_center: Option<Position>,
        receiver: &Character,
    ) -> (Option<ApplyEffect>, String, u32) {
        let mut damage_dealt = 0;
        let mut actual_effect = None;
        let line = match effect {
            ApplyEffect::RemoveActionPoints(n) => {
                let lost = receiver.action_points.lose(n);
                actual_effect = Some(ApplyEffect::RemoveActionPoints(lost));
                format!("  {} lost {} AP", receiver.name, n)
            }
            ApplyEffect::GainStamina(n) => {
                let gained = receiver.stamina.gain(n);
                actual_effect = Some(ApplyEffect::GainStamina(gained));
                format!("  {} gained {} stamina", receiver.name, gained)
            }
            ApplyEffect::GainHealth(n) => {
                let gained = receiver.health.gain(n);
                actual_effect = Some(ApplyEffect::GainHealth(gained));
                format!("  {} gained {} health", receiver.name, gained)
            }
            e @ ApplyEffect::Condition(apply_condition) => {
                actual_effect = Some(e);
                self.perform_receive_condition(apply_condition, receiver)
            }
            e @ ApplyEffect::PerBleeding {
                damage,
                caster_healing_percentage,
            } => {
                actual_effect = Some(e);
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
            e @ ApplyEffect::ConsumeCondition { condition } => {
                let stacks_cleared = receiver.clear_condition(condition);
                let mut line = "".to_string();
                if let Some(stacks) = stacks_cleared {
                    line.push_str(&format!("  {} lost {}", receiver.name, condition.name()));
                    if stacks > 0 {
                        line.push_str(&format!(" ({})", stacks));
                        actual_effect = Some(e);
                    }
                }
                line
            }
            e @ ApplyEffect::Knockback(amount) => {
                let giver = giver.unwrap();
                let mut source_pos = giver.pos();

                if let Some(center) = area_center {
                    // The knockback originates from the area center, unless the area was centered directly on this target
                    if center != receiver.pos() {
                        source_pos = center;
                    }
                }

                let dx = receiver.pos().0 - source_pos.0;
                let dy = receiver.pos().1 - source_pos.1;
                let vector = if dx.abs() >= dy.abs() {
                    (amount as i32 * dx.signum(), 0)
                } else {
                    (0, amount as i32 * dy.signum())
                };
                receiver.is_being_pushed_in_direction.set(Some(vector));
                actual_effect = Some(e);
                format!("  {} was knocked back ({})", receiver.name, amount)
            }
        };

        (actual_effect, line, damage_dealt)
    }

    fn current_time(&self) -> u32 {
        self.round_index * self.round_length + self.active_character().index_in_round.get().unwrap()
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
        caster: &Rc<Character>,
        ability: Ability,
        enhancements: &[AbilityEnhancement],
        selected_target: &ActionTarget,
        mode: ActionPerformanceMode<'_>,
    ) -> Vec<AbilityResolvedEvent> {
        println!(
            "perform_ability {}, real={:?}",
            ability.name,
            matches!(mode, ActionPerformanceMode::Real(..))
        );

        let caster_id = caster.id();

        let real_game: Option<&CoreGame> = mode.real_game();
        let simulated_roll = mode.simulated_roll();

        if real_game.is_some() {
            caster.action_points.spend(ability.action_point_cost);
            caster.spend_mana(ability.mana_cost);
            caster.stamina.spend(ability.stamina_cost);
            for enhancement in enhancements {
                caster.action_points.spend(enhancement.action_point_cost);
                caster.spend_mana(enhancement.mana_cost);
                caster.stamina.spend(enhancement.stamina_cost);
            }
        }

        let mut enemies_hit = vec![];
        let mut resolve_events = vec![];

        let mut cast_n_times = 1;
        for enhancement in enhancements {
            if let Some(e) = enhancement.spell_effect {
                if e.cast_twice {
                    cast_n_times = 2;
                }
            }
        }

        for i in 0..cast_n_times {
            let mut detail_lines = vec![];

            let mut advantange_level = 0_i32;

            for enhancement in enhancements {
                if let Some(e) = enhancement.spell_effect {
                    let bonus = e.bonus_advantage;
                    if bonus > 0 {
                        advantange_level += bonus as i32;
                    }
                }
            }

            let mut maybe_ability_roll = None;

            if let Some(roll_type) = ability.roll {
                let dice_roll = simulated_roll.unwrap_or(roll_d20_with_advantage(advantange_level));

                if let Some(description) = roll_description(advantange_level) {
                    detail_lines.push(description);
                }

                let mut dice_roll_line = format!("Rolled: {}", dice_roll);
                let mut roll_calculation = dice_roll as i32;
                match roll_type {
                    AbilityRollType::Spell => {
                        let modifier = caster.spell_modifier() as i32;
                        roll_calculation += modifier;
                        dice_roll_line.push_str(&format!(" (+{} spell mod)", modifier));

                        for enhancement in enhancements {
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

                        maybe_ability_roll = Some(AbilityRoll::RolledWithSpellModifier {
                            result: ability_result,
                            line: dice_roll_line,
                        });
                    }
                    AbilityRollType::RollAbilityWithAttackModifier => {
                        let modifier = caster.attack_modifier(HandType::MainHand) as i32;
                        roll_calculation += modifier;
                        dice_roll_line.push_str(&format!(" (+{} attack mod)", modifier));

                        for enhancement in enhancements {
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

                        maybe_ability_roll = Some(AbilityRoll::RolledWithAttackModifier {
                            result: ability_result,
                            line: dice_roll_line,
                        });
                    }
                    AbilityRollType::RollDuringAttack(bonus) => {
                        maybe_ability_roll = Some(AbilityRoll::WillRollDuringAttack { bonus });
                    }
                };
            }

            let mut target_outcome = None;
            let mut area_outcome = None;

            match ability.target {
                AbilityTarget::Enemy {
                    effect,
                    impact_circle,
                    ..
                } => {
                    let ActionTarget::Character(target_id, movement) = &selected_target else {
                        unreachable!()
                    };

                    if let Some(game) = real_game {
                        if let Some(positions) = movement {
                            game.perform_movement(
                                caster.id(),
                                positions.clone(),
                                MovementType::AbilityEngage,
                            )
                            .await;
                        }
                    }

                    let target = mode.characters().get(*target_id);

                    if let Some(game) = real_game {
                        assert!(caster.reaches_with_ability(
                            ability,
                            enhancements,
                            target.position.get()
                        ));
                        caster.set_facing_toward(target.pos());
                        game.ui_handle_event(GameEvent::AbilityWasInitiated {
                            actor: caster_id,
                            ability,
                            target: Some(*target_id),
                            area_at: None,
                        })
                        .await;
                    }

                    let mut ability_roll = maybe_ability_roll.unwrap();

                    match &mut ability_roll {
                        AbilityRoll::RolledWithSpellModifier { line, .. } => {
                            let spell_enemy_effect = effect.unwrap_spell();
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
                            detail_lines.push(line.to_string());
                        }
                        AbilityRoll::RolledWithAttackModifier { line, .. } => {
                            let spell_enemy_effect = effect.unwrap_spell();
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
                            detail_lines.push(line.to_string());
                        }
                        AbilityRoll::WillRollDuringAttack { .. } => {}
                    }

                    let before = SystemTime::now();
                    let outcome = Self::perform_ability_enemy_effect(
                        caster,
                        ability.name,
                        &ability_roll,
                        enhancements,
                        effect,
                        target,
                        &mut detail_lines,
                        None,
                        mode,
                    );
                    target_outcome = Some((*target_id, outcome));

                    if let Some((radius, acquisition, area_effect)) = impact_circle {
                        detail_lines.push("Area of effect:".to_string());

                        let area_target_outcomes = Self::perform_ability_area_enemy_effect(
                            AreaShape::Circle(radius),
                            "AoE",
                            ability_roll,
                            enhancements,
                            caster,
                            target.position.get(),
                            &mut detail_lines,
                            area_effect,
                            acquisition,
                            mode,
                        );

                        area_outcome = Some(AbilityAreaOutcome {
                            center: target.position.get(),
                            targets: area_target_outcomes,
                            shape: AreaShape::Circle(radius),
                        });
                    }

                    dbg!(before.elapsed());
                }

                AbilityTarget::Ally { range: _, effect } => {
                    let ActionTarget::Character(target_id, movement) = &selected_target else {
                        unreachable!()
                    };

                    let target = mode.characters().get(*target_id);

                    let ability_roll = maybe_ability_roll.unwrap();
                    let (ability_result, dice_roll_line) = ability_roll.unwrap_actual_roll();
                    detail_lines.push(dice_roll_line.to_string());

                    let degree_of_success = ability_result / 10;
                    if degree_of_success > 0 {
                        detail_lines.push(format!("Fortune: {}", degree_of_success));
                    }
                    if let Some(game) = real_game {
                        assert!(caster.reaches_with_ability(
                            ability,
                            enhancements,
                            target.position.get()
                        ));
                        caster.set_facing_toward(target.pos());
                        game.ui_handle_event(GameEvent::AbilityWasInitiated {
                            actor: caster_id,
                            ability,
                            target: Some(*target_id),
                            area_at: None,
                        })
                        .await;
                    }

                    let outcome = Self::perform_ability_ally_effect(
                        ability.name,
                        enhancements,
                        effect,
                        target,
                        &mut detail_lines,
                        degree_of_success,
                        mode,
                    );

                    target_outcome = Some((*target_id, outcome));
                }

                AbilityTarget::Area {
                    range: _,
                    area_effect,
                } => {
                    let target_pos = selected_target.unwrap_position();

                    if let Some(game) = real_game {
                        assert!(caster.reaches_with_ability(ability, enhancements, target_pos));
                        caster.set_facing_toward(target_pos);
                        game.ui_handle_event(GameEvent::AbilityWasInitiated {
                            actor: caster_id,
                            ability,
                            target: None,
                            area_at: Some((area_effect.shape, target_pos)),
                        })
                        .await;
                    }

                    let ability_roll = maybe_ability_roll.unwrap();

                    if let Some((_ability_result, dice_roll_line)) = ability_roll.actual_roll() {
                        detail_lines.push(dice_roll_line.to_string());
                    }

                    let outcomes = Self::perform_ability_area_effect(
                        ability.name,
                        ability_roll,
                        enhancements,
                        caster,
                        target_pos,
                        area_effect,
                        &mut detail_lines,
                        mode,
                    );

                    area_outcome = Some(AbilityAreaOutcome {
                        center: target_pos,
                        targets: outcomes,
                        shape: area_effect.shape,
                    });
                }

                AbilityTarget::None {
                    self_area,
                    self_effect,
                } => {
                    if let Some(game) = real_game {
                        game.ui_handle_event(GameEvent::AbilityWasInitiated {
                            actor: caster_id,
                            ability,
                            target: None,
                            area_at: None,
                        })
                        .await;
                    }

                    if let Some(AbilityRoll::RolledWithSpellModifier { result: _, line }) =
                        &maybe_ability_roll
                    {
                        detail_lines.push(line.clone());
                    }

                    if let Some(effect) = self_effect {
                        let degree_of_success = if let Some(ability_roll) = &maybe_ability_roll {
                            let (ability_result, _dice_roll_line) =
                                ability_roll.unwrap_actual_roll();
                            ability_result / 10
                        } else {
                            0
                        };

                        if degree_of_success > 0 {
                            detail_lines.push(format!("Fortune: {}", degree_of_success));
                        }

                        let outcome = Self::perform_ability_ally_effect(
                            ability.name,
                            enhancements,
                            effect,
                            caster,
                            &mut detail_lines,
                            degree_of_success,
                            mode,
                        );
                        target_outcome = Some((caster_id, outcome));
                    }

                    if let Some(area_effect) = self_area {
                        dbg!("SELF AREA ", area_effect.shape);

                        let ability_roll = maybe_ability_roll.unwrap();
                        dbg!(&ability_roll);

                        let outcomes = Self::perform_ability_area_effect(
                            ability.name,
                            ability_roll,
                            enhancements,
                            caster,
                            caster.position.get(),
                            area_effect,
                            &mut detail_lines,
                            mode,
                        );
                        area_outcome = Some(AbilityAreaOutcome {
                            center: caster.position.get(),
                            targets: outcomes,
                            shape: area_effect.shape,
                        });
                    }
                }
            };

            if i < cast_n_times - 1 {
                detail_lines.push(format!("{} cast again!", caster.name))
            }

            if let Some((target_id, outcome)) = &target_outcome {
                if matches!(outcome, AbilityTargetOutcome::HitEnemy { .. }) {
                    enemies_hit.push(*target_id);
                }
            }
            if let Some(AbilityAreaOutcome { targets, .. }) = &area_outcome {
                for (target_id, outcome) in targets {
                    if matches!(outcome, AbilityTargetOutcome::HitEnemy { .. }) {
                        enemies_hit.push(*target_id);
                    }
                }
            }

            let caster_id = caster.id();

            let resolve_event = AbilityResolvedEvent {
                actor: caster_id,
                target_outcome,
                area_outcome,
                ability,
                detail_lines,
            };

            resolve_events.push(resolve_event.clone());

            // TODO also communicate if the caster healed from hitting the target (e.g. necrotic influence)
            if let Some(game) = real_game {
                game.ui_handle_event(GameEvent::AbilityResolved(resolve_event))
                    .await;
            }
        }

        resolve_events
    }

    fn perform_ability_area_effect(
        name: &'static str,
        ability_roll: AbilityRoll,
        enhancements: &[AbilityEnhancement],
        caster: &Rc<Character>,
        area_center: Position,
        area_effect: AreaEffect,
        detail_lines: &mut Vec<String>,
        mode: ActionPerformanceMode,
    ) -> Vec<(CharacterId, AbilityTargetOutcome)> {
        match area_effect.effect {
            AbilityEffect::Negative(effect) => Self::perform_ability_area_enemy_effect(
                area_effect.shape,
                name,
                ability_roll,
                enhancements,
                caster,
                area_center,
                detail_lines,
                effect,
                area_effect.acquisition,
                mode,
            ),

            AbilityEffect::Positive(effect) => {
                assert!(area_effect.acquisition == AreaTargetAcquisition::Allies);

                Self::perform_ability_area_ally_effect(
                    area_effect.shape,
                    name,
                    enhancements,
                    caster,
                    area_center,
                    detail_lines,
                    ability_roll,
                    effect,
                    mode,
                )
            }
        }
    }

    fn perform_ability_area_ally_effect(
        mut shape: AreaShape,
        name: &'static str,
        enhancements: &[AbilityEnhancement],
        caster: &Character,
        area_pos: Position,
        detail_lines: &mut Vec<String>,
        ability_roll: AbilityRoll,
        effect: AbilityPositiveEffect,
        mode: ActionPerformanceMode,
    ) -> Vec<(CharacterId, AbilityTargetOutcome)> {
        let mut target_outcomes = vec![];

        for enhancement in enhancements {
            let e = enhancement.spell_effect.unwrap();
            if e.increased_radius_tenths > 0 {
                let AreaShape::Circle(radius) = &mut shape else {
                    panic!()
                };
                *radius = radius.plusf(e.increased_radius_tenths as f32 * 0.1);
            }
        }

        let roll_result = ability_roll.unwrap_actual_roll().0;

        let degree_of_success = roll_result / 10;
        if degree_of_success > 0 {
            detail_lines.push(format!("Fortune: {}", degree_of_success));
        }

        for other_char in mode.characters().iter() {
            if other_char.player_controlled() != caster.player_controlled() {
                continue;
            }

            if is_target_within_shape(caster.pos(), area_pos, shape, other_char) {
                detail_lines.push(other_char.name.to_string());

                let outcome = Self::perform_ability_ally_effect(
                    name,
                    enhancements,
                    effect,
                    other_char,
                    detail_lines,
                    degree_of_success,
                    mode,
                );

                target_outcomes.push((other_char.id(), outcome));
            }
        }

        target_outcomes
    }

    fn perform_ability_ally_effect(
        name: &'static str,
        enhancements: &[AbilityEnhancement],
        ally_effect: AbilityPositiveEffect,
        target: &Character,
        detail_lines: &mut Vec<String>,
        degree_of_success: u32,
        mode: ActionPerformanceMode,
    ) -> AbilityTargetOutcome {
        let mut applied_effects = vec![];

        let real_game = mode.real_game();

        if ally_effect.healing > 0 {
            let mut healing = ally_effect.healing;

            let mut line = format!("  Healing: {} ({})", ally_effect.healing, name);

            if degree_of_success > 0 {
                line.push_str(&format!(" +{} (fortune)", degree_of_success));
                healing += degree_of_success;
            }
            line.push_str(&format!(" = {}", healing));
            detail_lines.push(line);

            if let Some(game) = real_game {
                let health_gained = game.perform_gain_health(target, healing);
                detail_lines.push(format!(
                    "  {} was healed for {}",
                    target.name, health_gained
                ));
                applied_effects.push(ApplyEffect::GainHealth(health_gained));
            } else {
                // This might include over-heal, but hey.
                applied_effects.push(ApplyEffect::GainHealth(healing));
            }
        };

        if let Some(game) = real_game {
            for mut effect in ally_effect.apply.iter().flatten().flatten().copied() {
                match effect {
                    ApplyEffect::RemoveActionPoints(ref mut n) => *n += degree_of_success,
                    ApplyEffect::GainHealth(ref mut n) => *n += degree_of_success,
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
                    ApplyEffect::Knockback { .. } => {}
                }

                let (applied, log_line, _damage) =
                    game.perform_effect_application(effect, None, None, target);
                if let Some(applied) = applied {
                    applied_effects.push(applied);
                }
                detail_lines.push(log_line);
            }

            for enhancement in enhancements {
                let effect = enhancement.spell_effect.unwrap();
                for apply_effect in effect.target_on_hit.iter().flatten().flatten() {
                    let (applied, log_line, _damage) =
                        game.perform_effect_application(*apply_effect, None, None, target);
                    detail_lines.push(format!("{} ({})", log_line, enhancement.name));
                }
            }
        }

        AbilityTargetOutcome::AffectedAlly { applied_effects }
    }

    fn perform_ability_area_enemy_effect(
        mut shape: AreaShape,
        name: &'static str,
        ability_roll: AbilityRoll,
        enhancements: &[AbilityEnhancement],
        caster: &Rc<Character>,
        area_pos: Position,
        detail_lines: &mut Vec<String>,
        effect: AbilityNegativeEffect,
        acquisition: AreaTargetAcquisition,
        mode: ActionPerformanceMode<'_>,
    ) -> Vec<(CharacterId, AbilityTargetOutcome)> {
        assert!(acquisition != AreaTargetAcquisition::Allies);

        let mut target_outcomes = vec![];

        for enhancement in enhancements {
            if let Some(e) = enhancement.spell_effect {
                if e.increased_radius_tenths > 0 {
                    let AreaShape::Circle(radius) = &mut shape else {
                        panic!()
                    };
                    *radius = radius.plusf(e.increased_radius_tenths as f32 * 0.1);
                }
            }
        }

        for other_char in mode.characters().iter() {
            let is_ally = other_char.player_controlled() == caster.player_controlled();
            let valid_target = match acquisition {
                AreaTargetAcquisition::Enemies => !is_ally,
                AreaTargetAcquisition::Everyone => true,
                AreaTargetAcquisition::Allies => unreachable!(),
            };
            if !valid_target {
                continue;
            }

            if is_target_within_shape(caster.pos(), area_pos, shape, other_char) {
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

                let outcome = Self::perform_ability_enemy_effect(
                    caster,
                    name,
                    &ability_roll,
                    enhancements,
                    effect,
                    other_char,
                    detail_lines,
                    Some(area_pos),
                    mode,
                );

                target_outcomes.push((other_char.id(), outcome));
            }
        }

        let num_targets_hit = target_outcomes.len() as u32;

        if let Some(game) = mode.real_game() {
            dbg!(&enhancements);
            for enhancement in enhancements {
                for mut apply_effect in enhancement
                    .apply_on_self_per_area_target_hit
                    .iter()
                    .copied()
                    .flatten()
                    .flatten()
                {
                    if num_targets_hit > 0 {
                        apply_effect.multiply(num_targets_hit);

                        let (applied, _line, _damage) = game.perform_effect_application(
                            apply_effect,
                            Some(caster),
                            None,
                            caster,
                        );

                        let mut applied_effects = vec![];
                        if let Some(applied) = applied {
                            applied_effects.push(applied);
                        }

                        target_outcomes.push((
                            caster.id(),
                            AbilityTargetOutcome::AffectedAlly { applied_effects },
                        ));
                    }
                }
            }
        }
        target_outcomes
    }

    fn perform_ability_enemy_effect(
        caster: &Rc<Character>,
        ability_name: &'static str,
        ability_roll: &AbilityRoll,
        enhancements: &[AbilityEnhancement],
        enemy_effect: AbilityNegativeEffect,
        target: &Character,
        detail_lines: &mut Vec<String>,
        area_center: Option<Position>,
        mode: ActionPerformanceMode,
    ) -> AbilityTargetOutcome {
        match enemy_effect {
            AbilityNegativeEffect::Spell(spell_enemy_effect) => Self::perform_spell_enemy_effect(
                caster,
                ability_name,
                ability_roll,
                enhancements,
                spell_enemy_effect,
                target,
                detail_lines,
                area_center,
                mode,
            ),
            AbilityNegativeEffect::PerformAttack => {
                let attack_enhancement_effects: Vec<(&str, AttackEnhancementEffect)> = enhancements
                    .iter()
                    .filter_map(|e| e.attack_enhancement_effect())
                    .collect();

                let roll_modifier = ability_roll.unwrap_attack_bonus();
                caster.set_facing_toward(target.pos());
                let event: AttackedEvent = Self::perform_attack(
                    caster,
                    HandType::MainHand,
                    &attack_enhancement_effects,
                    target,
                    None,
                    roll_modifier,
                    mode,
                );

                AbilityTargetOutcome::AttackedEnemy(event)
            }
        }
    }

    fn perform_spell_enemy_effect(
        caster: &Character,
        ability_name: &'static str,
        ability_roll: &AbilityRoll,
        enhancements: &[AbilityEnhancement],
        spell_enemy_effect: SpellNegativeEffect,
        target: &Character,
        detail_lines: &mut Vec<String>,
        area_center: Option<Position>,
        mode: ActionPerformanceMode,
    ) -> AbilityTargetOutcome {
        let real_game = mode.real_game();

        let success = match spell_enemy_effect.defense_type {
            Some(contest) => {
                let ability_result = ability_roll.unwrap_actual_roll().0;
                let defense = match contest {
                    DefenseType::Will => target.will(),
                    DefenseType::Evasion => target.evasion(),
                    DefenseType::Toughness => target.toughness(),
                };

                if ability_result >= defense {
                    Some(((ability_result - defense) / 10) as i32)
                } else if ability_result >= defense - 10 {
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
                _ => {
                    detail_lines.push("  Crit".to_string());
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

                for enhancement in enhancements {
                    let e = enhancement.spell_effect.unwrap();
                    let bonus_dmg = if area_center.is_some() {
                        e.bonus_area_damage
                    } else {
                        e.bonus_target_damage
                    };
                    if bonus_dmg > 0 {
                        dmg_str.push_str(&format!(" +{} ({})", bonus_dmg, enhancement.name));
                        dmg_calculation += bonus_dmg as i32;
                    }
                }

                let graze = degree_of_success == -1;

                if graze {
                    dmg_str.push_str(" -50% (Graze)");
                    dmg_calculation -= (dmg_calculation as f32 * 0.5).ceil() as i32;
                } else if increased_by_good_roll && degree_of_success > 0 {
                    dmg_str.push_str(&format!(" +50% ({success_label})"));
                    dmg_calculation += (dmg_calculation as f32 * 0.5).ceil() as i32;
                }

                let damage = dmg_calculation.max(0) as u32;

                if let Some(game) = real_game {
                    if dmg_calculation > 0 {
                        game.perform_losing_health(target, damage);
                        dmg_str.push_str(&format!(" = {damage}"));
                        detail_lines.push(dmg_str);
                    }
                }

                Some(damage)
            } else {
                None
            };

            let mut applied_effects = vec![];

            fn apply_degree_of_success(
                stacks: &mut u32,
                degree_of_success: i32,
                reduced_to_nothing: &mut bool,
            ) {
                if degree_of_success == -1 {
                    // -25% Graze
                    *stacks -= (*stacks as f32 * 0.25).ceil() as u32;
                } else if degree_of_success > 0 {
                    // +50% Crit
                    *stacks += (*stacks as f32 * 0.5).ceil() as u32;
                }

                if *stacks <= 0 {
                    *reduced_to_nothing = true;
                }
            }

            let mut damage_from_effects = 0;

            if let Some(game) = real_game {
                for mut effect in spell_enemy_effect
                    .on_hit
                    .unwrap_or_default()
                    .iter()
                    .copied()
                    .flatten()
                {
                    let mut reduced_to_nothing = false;
                    match effect {
                        ApplyEffect::RemoveActionPoints(ref mut n) => {
                            apply_degree_of_success(n, degree_of_success, &mut reduced_to_nothing);
                        }
                        ApplyEffect::GainStamina(ref mut n) => {
                            apply_degree_of_success(n, degree_of_success, &mut reduced_to_nothing)
                        }
                        ApplyEffect::GainHealth(ref mut n) => {
                            apply_degree_of_success(n, degree_of_success, &mut reduced_to_nothing)
                        }
                        ApplyEffect::Condition(ref mut apply_condition) => {
                            if let Some(stacks) = &mut apply_condition.stacks {
                                apply_degree_of_success(
                                    stacks,
                                    degree_of_success,
                                    &mut reduced_to_nothing,
                                );
                            }
                            if let Some(rounds) = &mut apply_condition.duration_rounds {
                                apply_degree_of_success(
                                    rounds,
                                    degree_of_success,
                                    &mut reduced_to_nothing,
                                );
                            }
                        }
                        ApplyEffect::PerBleeding { .. } => {}
                        ApplyEffect::ConsumeCondition { .. } => {}
                        ApplyEffect::Knockback(ref mut distance) => {
                            apply_degree_of_success(
                                distance,
                                degree_of_success,
                                &mut reduced_to_nothing,
                            );
                        }
                    }

                    if reduced_to_nothing {
                        detail_lines.push(format!("'{}' was reduced to nothing (Graze)", effect));
                    } else {
                        let (applied, log_line, damage) = game.perform_effect_application(
                            effect,
                            Some(caster),
                            area_center,
                            target,
                        );
                        if let Some(applied) = applied {
                            applied_effects.push(applied);
                        }
                        damage_from_effects += damage;
                        detail_lines.push(log_line);
                    }
                }

                for enhancement in enhancements {
                    // TODO: shouldn't these also be affected by degree of success?
                    let e = enhancement.spell_effect.unwrap();
                    let effects = if area_center.is_some() {
                        e.area_on_hit
                    } else {
                        e.target_on_hit
                    };
                    for effect in effects.iter().flatten().flatten() {
                        let (applied, log_line, damage) = game.perform_effect_application(
                            *effect,
                            Some(caster),
                            area_center,
                            target,
                        );
                        if let Some(applied) = applied {
                            applied_effects.push(applied);
                        }
                        damage_from_effects += damage;
                        detail_lines.push(format!("{} ({})", log_line, enhancement.name));
                    }
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

    fn perform_attack(
        attacker: &Rc<Character>,
        hand_type: HandType,
        enhancements: &[(&'static str, AttackEnhancementEffect)],
        defender: &Character,
        maybe_reaction: Option<(CharacterId, OnAttackedReaction)>,
        ability_roll_modifier: i32,
        mode: ActionPerformanceMode,
    ) -> AttackedEvent {
        let game = match mode {
            ActionPerformanceMode::Real(core_game) => Some(core_game),
            ActionPerformanceMode::SimulatedRoll(..) => None,
        };

        let mut attack_bonus = attack_roll_bonus(
            attacker,
            hand_type,
            defender,
            enhancements,
            maybe_reaction.map(|(_reactor, reaction)| reaction),
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

        if let Some((reactor, reaction)) = maybe_reaction {
            if let Some(game) = game {
                let reactor = game.characters.get(reactor);
                reactor.on_use_on_attacked_reaction(reaction);
                detail_lines.push(format!("{} reacted with {}", reactor.name, reaction.name));
            }

            let bonus_evasion = reaction.effect.bonus_evasion;
            if bonus_evasion > 0 {
                evasion += bonus_evasion;

                if game.is_some() {
                    detail_lines.push(format!(
                        "  Evasion: {} +{} ({}) = {}",
                        evasion - bonus_evasion,
                        bonus_evasion,
                        reaction.name,
                        evasion
                    ));
                    let p_hit =
                        probability_of_d20_reaching(evasion - attack_modifier, attack_bonus);
                    detail_lines.push(format!("  Chance to hit: {:.1}%", p_hit * 100f32));
                }
            }

            let bonus_armor = reaction.effect.bonus_armor;
            if bonus_armor > 0 {
                if game.is_some() {
                    detail_lines.push(format!(
                        "  Armor: {} +{} ({}) = {}",
                        armor_value,
                        bonus_armor,
                        reaction.name,
                        armor_value + bonus_armor
                    ));
                }
                armor_value += bonus_armor;
            }

            match reaction.id {
                OnAttackedReactionId::Parry => evasion_from_parry = bonus_evasion,
                OnAttackedReactionId::SideStep => evasion_from_sidestep = bonus_evasion,
                OnAttackedReactionId::Block => evasion_from_block = bonus_evasion,
            }
        }

        let unmodified_roll = mode
            .simulated_roll()
            .unwrap_or(roll_d20_with_advantage(attack_bonus.advantage));
        let roll_result =
            ((unmodified_roll + attack_modifier) as i32 + attack_bonus.flat_amount) as u32;

        if game.is_some() {
            if let Some(description) = roll_description(attack_bonus.advantage) {
                detail_lines.push(description);
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
                if game.is_some() {
                    attacker.spend_one_arrow();
                }
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
        for (penetration, label) in &armor_penetrators {
            armor_value = armor_value.saturating_sub(*penetration);
            armor_str.push_str(&format!(" -{} ({})", penetration, label));
        }

        if game.is_some() {
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
        }

        let weapon = attacker.weapon(hand_type).unwrap();
        let outcome = if roll_result >= evasion.saturating_sub(10) {
            let mut on_true_hit_effect = None;
            let mut dmg_calculation = weapon.damage as i32;

            let mut dmg_str = format!("  Damage: {} ({})", dmg_calculation, weapon.name);
            let mut hit_type = AttackHitType::Regular;

            if matches!(weapon.grip, WeaponGrip::Versatile) && attacker.off_hand.get().is_empty() {
                let bonus_dmg = 1;
                dmg_str.push_str(&format!(" +{} (two-handed)", bonus_dmg));
                dmg_calculation += bonus_dmg;
            }

            let mut graze_improvement = None;

            for (name, effect) in enhancements {
                let bonus_dmg = effect.bonus_damage;
                if bonus_dmg > 0 {
                    dmg_str.push_str(&format!(" +{} ({})", bonus_dmg, name));
                    dmg_calculation += bonus_dmg as i32;
                }
                if effect.improved_graze {
                    graze_improvement = Some(name);
                }
            }

            if attacker
                .known_passive_skills
                .contains(&PassiveSkill::Honorless)
            {
                let bonus_dmg = 1;
                if is_target_flanked(attacker.pos(), defender) {
                    dmg_str.push_str(&format!(" +{} (Honorless)", bonus_dmg));
                    dmg_calculation += bonus_dmg;
                }
            }

            if roll_result < evasion {
                hit_type = AttackHitType::Graze;
            } else if roll_result >= evasion + 10 {
                hit_type = AttackHitType::Critical;
            }

            match hit_type {
                AttackHitType::Graze => {
                    if let Some(source) = graze_improvement {
                        dmg_str.push_str(&format!(" -25% (graze, {})", source));
                        dmg_calculation -= (dmg_calculation as f32 * 0.25).ceil() as i32;
                    } else {
                        dmg_str.push_str(" -50% (graze)");
                        dmg_calculation -= (dmg_calculation as f32 * 0.5).ceil() as i32;
                    }
                    detail_lines.push("  Graze!".to_string());
                }
                AttackHitType::Regular => {
                    on_true_hit_effect = weapon.on_true_hit;
                    detail_lines.push("  Hit".to_string());
                }
                AttackHitType::Critical => {
                    on_true_hit_effect = weapon.on_true_hit;
                    detail_lines.push("  Critical hit".to_string());
                    dmg_str.push_str(" +50% (crit)");
                    dmg_calculation += (dmg_calculation as f32 * 0.5).ceil() as i32;
                }
            }

            if defender.conditions.borrow().has(&Condition::Protected) {
                dmg_str.push_str(" -30% (Protected)");
                dmg_calculation -= (dmg_calculation as f32 * 0.3).ceil() as i32;
            }

            if !armor_penetrators.is_empty() {
                detail_lines.push(format!("  Armor: {} = {}", armor_str, armor_value));
            }
            if armor_value > 0 {
                dmg_str.push_str(&format!(" -{armor_value} (armor)"));
                dmg_calculation -= armor_value as i32;
            }

            let damage = dmg_calculation.max(0) as u32;

            let mut actual_health_lost = 0;

            if let Some(game) = game {
                dmg_str.push_str(&format!(" = {damage}"));
                detail_lines.push(dmg_str);
                actual_health_lost = game.perform_losing_health(defender, damage);
            }

            let mut applied_effects = vec![];

            if let Some(game) = game {
                if let Some(effect) = on_true_hit_effect {
                    match effect {
                        AttackHitEffect::Apply(effect) => {
                            let (applied, log_line, _damage) = game.perform_effect_application(
                                effect,
                                Some(attacker),
                                None,
                                defender,
                            );
                            detail_lines.push(format!("{} (true hit)", log_line))
                        }
                        AttackHitEffect::SkipExertion => skip_attack_exertion = true,
                    }
                }

                if damage > 0 {
                    for (name, effect) in enhancements {
                        if let Some(effect) = effect.on_damage_effect {
                            let log_line = match effect {
                                AttackEnhancementOnHitEffect::RegainActionPoint => {
                                    attacker.action_points.gain(1);
                                    format!("{} regained 1 AP", attacker.name)
                                }
                                AttackEnhancementOnHitEffect::Target(
                                    defense_type,
                                    apply_effect,
                                ) => {
                                    let mut resist = false;
                                    if let Some(defense_type) = defense_type {
                                        let defense = defender.defense(defense_type);
                                        detail_lines.push(format!(
                                            "{} vs {}={}",
                                            roll_result,
                                            defense_type.name(),
                                            defense
                                        ));
                                        if roll_result < defense {
                                            resist = true;
                                        }
                                    }
                                    if resist {
                                        "Resist".to_string()
                                    } else {
                                        let (applied, log_line, _damage) = game
                                            .perform_effect_application(
                                                apply_effect,
                                                Some(attacker),
                                                None,
                                                defender,
                                            );
                                        if let Some(apply_effect) = applied {
                                            applied_effects.push(apply_effect);
                                        }
                                        log_line
                                    }
                                }
                            };

                            detail_lines.push(format!("{} ({})", log_line, name))
                        }

                        if let Some((x, condition)) = effect.inflict_x_condition_per_damage {
                            //*condition.stacks().unwrap() = damage;
                            let stacks = (damage * x.num) / x.den;
                            let line = game.perform_receive_condition(
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
                            let (applied, log_line, _damage) = game.perform_effect_application(
                                apply_effect,
                                Some(attacker),
                                None,
                                defender,
                            );
                            detail_lines.push(format!("{} ({})", log_line, arrow.name))
                        }
                    }
                }

                if defender.lose_protected() {
                    detail_lines.push(format!("{} lost Protected", defender.name));
                }
            }

            AttackOutcome::Hit {
                damage,
                actual_health_lost,
                hit_type,
                applied_effects,
            }
        } else if roll_result
            < evasion.saturating_sub(
                evasion_from_parry + evasion_from_sidestep + evasion_from_block + 10,
            )
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

        let mut area_outcomes = None;
        if let Some(game) = game {
            if defender.lose_distracted() {
                detail_lines.push(format!("{} lost Distracted", defender.name));
            }

            for (name, effect) in enhancements {
                if let Some(effect) = effect.on_target {
                    let (_applied, log_line, _damage) =
                        game.perform_effect_application(effect, Some(attacker), None, defender);
                    detail_lines.push(format!("{} ({})", log_line, name));
                }
            }

            if let Some(arrow) = used_arrow {
                if let Some(area_effect) = arrow.area_effect {
                    detail_lines.push("".to_string());
                    // TODO: This AoE should also be performed (predicted) in attack-prediction mode (so that enemies' healthbar previews can be shown
                    // also for the AoE targets)
                    let area_target_outcomes = Self::perform_ability_area_effect(
                        arrow.name,
                        AbilityRoll::RolledWithSpellModifier {
                            result: roll_result,
                            line: "".to_string(),
                        },
                        &[],
                        attacker,
                        defender.pos(),
                        area_effect,
                        &mut detail_lines,
                        mode,
                    );
                    area_outcomes = Some(area_target_outcomes);
                }
            }

            if weapon.is_melee() {
                if let Some(previously_engaged) = attacker.engagement_target.take() {
                    game.characters
                        .get(previously_engaged)
                        .set_not_engaged_by(attacker.id());
                }
                defender.set_engaged_by(Rc::clone(attacker));
                attacker.engagement_target.set(Some(defender.id()));
            }
        }

        AttackedEvent {
            attacker: attacker.id(),
            target: defender.id(),
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
                        let (_applied, log_line, _damage) = self.perform_effect_application(
                            ApplyEffect::Condition(ApplyCondition {
                                condition,
                                stacks: None,
                                duration_rounds: duration,
                            }),
                            Some(reactor),
                            None,
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
                source: DamageSource::Condition(Condition::Bleeding),
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
                source: DamageSource::Condition(Condition::Burning),
            })
            .await;
            conditions.borrow_mut().remove(&Condition::Burning);

            let mut adj_others: Vec<&Rc<Character>> = self
                .characters
                .iter()
                .filter(|other| {
                    // TODO: if "melee" doesn't encompass diagonally adjacent, the spreading feels
                    // a bit too unlikely
                    other.id != character.id
                        && are_entities_within_melee(other.pos(), character.pos())
                })
                .collect();
            if !adj_others.is_empty() {
                let spread_amount = burn_stacks / 2;
                dbg!(spread_amount);
                if spread_amount > 0 {
                    CustomShuffle::shuffle(&mut adj_others);
                    let per_receiver = spread_amount / adj_others.len() as u32;
                    let mut remainder = spread_amount % adj_others.len() as u32;
                    let mut num_receivers = 0;
                    for other in &adj_others {
                        let stacks = if remainder > 0 {
                            // Some unlucky ones get burned more than others...
                            remainder -= 1;
                            per_receiver + 1
                        } else {
                            per_receiver
                        };
                        if stacks > 0 {
                            num_receivers += 1;
                            let burning = Condition::Burning;
                            self.ui_handle_event(GameEvent::CharacterReceivedCondition {
                                character: other.id(),
                                condition: burning,
                            })
                            .await;
                            other.receive_condition(burning, Some(stacks), None);
                        }
                    }
                    self.log(format!("The fire spread to {} other(s)", num_receivers))
                        .await;
                }
            }
        }

        if conditions.borrow().has(&Condition::HealthPotionRecovering) {
            conditions
                .borrow_mut()
                .lose_stacks(&Condition::HealthPotionRecovering, 1);
            let health_gained = self.perform_gain_health(character, 2);
            // TODO Make this show on grid
            self.log(format!(
                "  {} gained {} health (healing potion)",
                character.name, health_gained
            ))
            .await;
        }

        if conditions.borrow_mut().remove(&Condition::Weakened) {
            self.log(format!("{} is no longer Weakened", name)).await;
        }
        if conditions.borrow_mut().remove(&Condition::Raging) {
            self.log(format!("{} stopped Raging", name)).await;
        }
        if conditions.borrow().has(&Condition::ArcaneSurge)
            && conditions
                .borrow_mut()
                .lose_stacks(&Condition::ArcaneSurge, 1)
        {
            self.log(format!("{} lost Arcane surge", name)).await;
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
        if conditions.borrow().has(&Condition::Hastened) {
            gain_ap += HASTENED_AP_BONUS;
        }
        let gained_ap = character.action_points.gain(gain_ap);
        //character.action_points.current.set(new_ap);

        conditions.borrow_mut().remove(&Condition::MainHandExertion);
        conditions.borrow_mut().remove(&Condition::OffHandExertion);
        conditions.borrow_mut().remove(&Condition::ReaperApCooldown);
        let gain_stamina = (character.stamina.max() as f32 / 4.0).ceil() as u32;
        let gained_stamina = character.stamina.gain(gain_stamina);
        character.regain_full_movement();

        if character.player_controlled() {
            self.ui_handle_event(GameEvent::PlayerCharacterEndedTheirTurn {
                gained_ap,
                gained_stamina,
            })
            .await;
        }
    }
}

fn roll_description(advantage: i32) -> Option<String> {
    match advantage.cmp(&0) {
        Ordering::Less => Some(format!(
            "Rolled {} dice with disadvantage...",
            advantage.abs() + 1
        )),
        Ordering::Equal => None,
        Ordering::Greater => Some(format!("Rolled {} dice with advantage...", advantage + 1)),
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MovementType {
    Regular,
    AbilityEngage,
    KnockedBack,
}

#[derive(Copy, Clone)]
enum ActionPerformanceMode<'a> {
    Real(&'a CoreGame),
    SimulatedRoll(u32, &'a Characters),
}

impl ActionPerformanceMode<'_> {
    fn characters(&self) -> &Characters {
        match self {
            ActionPerformanceMode::Real(core_game) => &core_game.characters,
            ActionPerformanceMode::SimulatedRoll(_, characters) => characters,
        }
    }

    fn simulated_roll(&self) -> Option<u32> {
        match self {
            ActionPerformanceMode::Real(..) => None,
            ActionPerformanceMode::SimulatedRoll(roll, _) => Some(*roll),
        }
    }

    fn real_game(&self) -> Option<&CoreGame> {
        match self {
            ActionPerformanceMode::Real(core_game) => Some(*core_game),
            ActionPerformanceMode::SimulatedRoll(..) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AttackPrediction {
    pub percentage_chance_deal_damage: u32,
    pub min_damage: u32,
    pub max_damage: u32,
    pub avg_damage: f32,
    pub details: Vec<(&'static str, Goodness)>,
}

pub struct AbilityPrediction {
    pub targets: HashMap<CharacterId, TargetPrediction>,
}

#[derive(Debug, Clone)]
pub struct TargetPrediction {
    pub damage: DamageInterval,
    pub is_buff: bool,
    pub details: Vec<(&'static str, Goodness)>,
}

impl From<AttackPrediction> for TargetPrediction {
    fn from(value: AttackPrediction) -> Self {
        Self {
            damage: DamageInterval {
                min: value.min_damage,
                max: value.max_damage,
            },
            is_buff: false,
            details: value.details,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct DamageInterval {
    pub min: u32,
    pub max: u32,
}

pub fn predict_ability(
    characters: &Characters,
    caster: &Rc<Character>,
    ability: Ability,
    enhancements: &[AbilityEnhancement],
    selected_target: &ActionTarget,
) -> AbilityPrediction {
    let mut targets: HashMap<CharacterId, TargetPrediction> = Default::default();

    // To predict the possible range, we first roll a 1 ...
    let resolved_events = pollster::FutureExt::block_on(CoreGame::perform_ability(
        caster,
        ability,
        enhancements,
        selected_target,
        ActionPerformanceMode::SimulatedRoll(1, characters),
    ));
    let event = &resolved_events[0];
    for (target_id, result) in event.affected_targets() {
        let mut details: Vec<(&'static str, Goodness)> = vec![];

        if let Some(roll) = ability.roll {
            for (label, contributor) in caster.outgoing_ability_roll_bonuses(enhancements, roll) {
                details.push((label, contributor.goodness()));
            }
            for (label, contributor) in characters.get(target_id).incoming_ability_bonuses() {
                details.push((label, contributor.goodness()));
            }
        }

        targets.insert(
            target_id,
            TargetPrediction {
                damage: DamageInterval {
                    min: result.damage,
                    max: 0,
                },
                is_buff: result.is_buff,
                details,
            },
        );
    }

    // ... and then roll a 20
    let resolved_events = pollster::FutureExt::block_on(CoreGame::perform_ability(
        caster,
        ability,
        enhancements,
        selected_target,
        ActionPerformanceMode::SimulatedRoll(20, characters),
    ));
    let event = &resolved_events[0];
    for (target_id, result) in event.affected_targets() {
        targets.get_mut(&target_id).unwrap().damage.max = result.damage;
    }

    AbilityPrediction { targets }
}

pub fn predict_attack(
    characters: &Characters,
    attacker: &Rc<Character>,
    hand_type: HandType,
    enhancements: &[(&'static str, AttackEnhancementEffect)],
    defender: &Character,
    reaction: Option<(CharacterId, OnAttackedReaction)>,
    ability_roll_modifier: i32,
) -> AttackPrediction {
    let mut damage_outcomes = vec![];
    let mut min_dmg = None;
    let mut max_dmg = 0;
    let mut percentage_deal_damage = 0;

    let mut details = vec![];
    for (label, contributor) in attacker.outgoing_attack_bonuses(hand_type, enhancements, defender)
    {
        details.push((label, contributor.goodness()));
    }
    for (label, contributor) in defender.incoming_attack_bonuses(reaction.map(|(_id, r)| r)) {
        details.push((label, contributor.goodness()));
    }

    // TODO: The average doesn't account for advantage!
    for unmodified_roll in 1..=20 {
        let event = CoreGame::perform_attack(
            attacker,
            hand_type,
            enhancements,
            defender,
            reaction,
            ability_roll_modifier,
            ActionPerformanceMode::SimulatedRoll(unmodified_roll, characters),
        );

        let damage = match event.outcome {
            AttackOutcome::Hit { damage, .. } => damage,
            _ => 0,
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
        details,
    }
}

#[derive(Debug)]
enum AbilityRoll {
    RolledWithSpellModifier { result: u32, line: String },
    RolledWithAttackModifier { result: u32, line: String },
    WillRollDuringAttack { bonus: i32 },
}

impl AbilityRoll {
    fn actual_roll(&self) -> Option<(u32, &str)> {
        match self {
            AbilityRoll::RolledWithSpellModifier { result, line } => Some((*result, line)),
            AbilityRoll::RolledWithAttackModifier { result, line } => Some((*result, line)),
            AbilityRoll::WillRollDuringAttack { .. } => None,
        }
    }
    fn unwrap_actual_roll(&self) -> (u32, &str) {
        self.actual_roll()
            .unwrap_or_else(|| panic!("haven't rolled"))
    }
    fn unwrap_attack_bonus(&self) -> i32 {
        match self {
            AbilityRoll::WillRollDuringAttack { bonus } => *bonus,
            unexpected => panic!("Not attack roll: {:?}", unexpected),
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
        movement_type: MovementType,
        step_idx: u32,
    },
    CharacterReactedToAttacked {
        reactor: CharacterId,
        with_shield: bool,
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
        area_at: Option<(AreaShape, Position)>,
    },
    AbilityResolved(AbilityResolvedEvent),
    ConsumableWasUsed {
        user: CharacterId,
        consumable: Consumable,
        detail_lines: Vec<String>,
    },
    CharactersDying {
        characters: Vec<CharacterId>,
    },
    CharactersDied {
        characters: Vec<CharacterId>,
        new_active: Option<CharacterId>,
    },
    PlayerCharacterEndedTheirTurn {
        gained_ap: u32,
        gained_stamina: u32,
    },
    NewActiveCharacter {
        new_active: CharacterId,
    },
    CharacterTookDamage {
        character: CharacterId,
        amount: u32,
        source: DamageSource,
    },
    CharacterReceivedCondition {
        character: CharacterId,
        condition: Condition,
    },
    CharacterReceivedKnockback {
        character: CharacterId,
    },
    CharacterGainedAP {
        character: CharacterId,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum DamageSource {
    Condition(Condition),
    KnockbackCollision,
}

impl DamageSource {
    pub fn name(&self) -> &'static str {
        match self {
            DamageSource::Condition(condition) => condition.name(),
            DamageSource::KnockbackCollision => "Collision",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AbilityResolvedEvent {
    pub actor: CharacterId,
    pub target_outcome: Option<(CharacterId, AbilityTargetOutcome)>,
    pub area_outcome: Option<AbilityAreaOutcome>,
    pub ability: Ability,
    pub detail_lines: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AbilityAreaOutcome {
    pub center: Position,
    pub targets: Vec<(CharacterId, AbilityTargetOutcome)>,
    pub shape: AreaShape,
}

impl AbilityResolvedEvent {
    fn enemies_hit(&self, result: &mut Vec<CharacterId>) {
        if let Some((target_id, outcome)) = &self.target_outcome {
            if matches!(outcome, AbilityTargetOutcome::HitEnemy { .. }) {
                result.push(*target_id);
            }
        }
        if let Some(AbilityAreaOutcome { targets, .. }) = &self.area_outcome {
            for (target_id, outcome) in targets {
                if matches!(
                    outcome,
                    AbilityTargetOutcome::HitEnemy { .. } | AbilityTargetOutcome::AttackedEnemy(..)
                ) {
                    result.push(*target_id);
                }
            }
        }
    }

    fn affected_targets(&self) -> HashMap<CharacterId, TargetResult> {
        let mut affected_targets = HashMap::default();
        if let Some((target_id, outcome)) = &self.target_outcome {
            affected_targets.insert(
                *target_id,
                TargetResult {
                    damage: outcome.damage().unwrap_or(0),
                    is_buff: outcome.is_buff(),
                },
            );
        }
        if let Some(AbilityAreaOutcome { targets, .. }) = &self.area_outcome {
            for (target_id, outcome) in targets {
                let entry = affected_targets.entry(*target_id).or_insert(TargetResult {
                    damage: 0,
                    is_buff: false,
                });
                entry.damage += outcome.damage().unwrap_or(0);
                if outcome.is_buff() {
                    entry.is_buff = true;
                }
            }
        }
        affected_targets
    }
}

struct TargetResult {
    damage: u32,
    is_buff: bool,
}

#[derive(Debug, Clone)]
pub struct AttackedEvent {
    pub attacker: CharacterId,
    pub target: CharacterId,
    pub outcome: AttackOutcome,
    pub detail_lines: Vec<String>,
    pub area_outcomes: Option<Vec<(CharacterId, AbilityTargetOutcome)>>,
}

#[derive(Debug, Clone)]
pub enum AttackOutcome {
    Hit {
        damage: u32,
        actual_health_lost: u32,
        hit_type: AttackHitType,
        applied_effects: Vec<ApplyEffect>,
    },
    Dodge,
    Block,
    Parry,
    Miss,
}

impl AttackOutcome {
    fn damage(&self) -> Option<u32> {
        match self {
            AttackOutcome::Hit { damage, .. } => Some(*damage),
            _ => None,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
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
        applied_effects: Vec<ApplyEffect>,
    },
}

impl AbilityTargetOutcome {
    fn damage(&self) -> Option<u32> {
        match self {
            AbilityTargetOutcome::HitEnemy { damage, .. } => *damage,
            AbilityTargetOutcome::AttackedEnemy(attacked_event) => attacked_event.outcome.damage(),
            AbilityTargetOutcome::Resisted => None,
            AbilityTargetOutcome::AffectedAlly { .. } => None,
        }
    }

    fn is_buff(&self) -> bool {
        matches!(self, AbilityTargetOutcome::AffectedAlly { .. })
    }

    /*
    // TODO: This doesn't work. Ability prediction doesn't run the code that applies effects, so applied_effects would always be empty
    fn is_debuff(&self) -> bool {
        match self {
            AbilityTargetOutcome::HitEnemy {
                applied_effects, ..
            } => !applied_effects.is_empty(),
            _ => false,
        }
    }
     */
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

    let def = defender.defense(defense_type);

    let modifier_value = match modifier {
        AbilityRollType::Spell => caster.spell_modifier() as i32,
        AbilityRollType::RollAbilityWithAttackModifier => {
            caster.attack_modifier(HandType::MainHand) as i32
        }
        AbilityRollType::RollDuringAttack(bonus) => {
            caster.attack_modifier(HandType::MainHand) as i32 + bonus
        }
    };

    let target = (def as i32 - modifier_value).max(1) as u32;
    probability_of_d20_reaching(target, bonus)
}

#[derive(Clone)]
pub struct Characters(Vec<Rc<Character>>);

impl Characters {
    pub fn new(mut characters: Vec<Rc<Character>>) -> Self {
        println!(
            "Creating Characters struct from {} entries",
            characters.len()
        );
        let round_length = characters.len() as u32;

        let unique_ch_ids: HashSet<CharacterId> = characters.iter().map(|ch| ch.id()).collect();
        assert_eq!(
            unique_ch_ids.len(),
            characters.len(),
            "Each character must have a unique ID"
        );

        // Player characters act first
        // TODO: it should be sorted by caller
        characters.sort_by(
            |a, b| match (a.player_controlled(), b.player_controlled()) {
                (true, true) => Ordering::Equal,
                (false, false) => Ordering::Equal,
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
            },
        );
        Self(
            characters
                .into_iter()
                .enumerate()
                .map(|(i, ch)| {
                    ch.index_in_round.set(Some(i as u32));
                    ch.round_length.set(Some(round_length));
                    ch
                })
                .collect(),
        )
    }

    fn next_id(&self) -> CharacterId {
        for ch in self.iter() {
            if !ch.has_taken_a_turn_this_round.get() {
                return ch.id();
            }
        }
        self.0[0].id()
    }

    pub fn contains_alive(&self, character_id: CharacterId) -> bool {
        self.0
            .iter()
            .any(|ch| ch.id() == character_id && !ch.is_dead())
    }

    pub fn get(&self, character_id: CharacterId) -> &Character {
        self.get_rc(character_id)
    }

    pub fn safe_get(&self, character_id: CharacterId) -> Option<&Character> {
        let entry = self.0.iter().find(|ch| ch.id() == character_id);
        entry.map(|ch| &**ch)
    }

    pub fn get_rc(&self, character_id: CharacterId) -> &Rc<Character> {
        let entry = self.0.iter().find(|ch| ch.id() == character_id);

        match entry {
            Some(ch) => ch,
            None => panic!("No character with id: {character_id}"),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Rc<Character>> {
        self.0.iter().map(|ch| ch)
    }

    pub fn player_characters(self) -> Vec<Character> {
        self.iter()
            .filter(|ch| ch.player_controlled())
            .map(|ch| Character::clone(ch))
            .collect()
    }

    pub fn remove_dead(&mut self) -> Vec<CharacterId> {
        let mut removed = vec![];
        self.0.retain(|ch| {
            if ch.is_dead() {
                removed.push(ch.id());
                false
            } else {
                true
            }
        });
        removed
    }

    pub fn as_map(&self) -> HashMap<CharacterId, Rc<Character>> {
        self.0.iter().map(|ch| (ch.id(), Rc::clone(ch))).collect()
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum ApplyEffect {
    RemoveActionPoints(u32),
    Condition(ApplyCondition),
    GainHealth(u32),
    GainStamina(u32),
    PerBleeding {
        damage: u32,
        caster_healing_percentage: u32,
    },
    ConsumeCondition {
        condition: Condition,
    },
    Knockback(u32),
}

impl ApplyEffect {
    fn multiply(&mut self, factor: u32) {
        match self {
            ApplyEffect::RemoveActionPoints(n) => *n *= factor,
            ApplyEffect::Condition(apply_condition) => {
                if let Some(rounds) = &mut apply_condition.duration_rounds {
                    *rounds *= factor;
                }
                if let Some(stacks) = &mut apply_condition.stacks {
                    *stacks *= factor;
                }
            }
            ApplyEffect::GainHealth(n) => *n *= factor,
            ApplyEffect::GainStamina(n) => *n *= factor,
            ApplyEffect::PerBleeding {
                damage,
                caster_healing_percentage,
            } => todo!(),
            ApplyEffect::ConsumeCondition { condition } => todo!(),
            ApplyEffect::Knockback(n) => *n *= factor,
        }
    }
}

impl Display for ApplyEffect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApplyEffect::RemoveActionPoints(n) => f.write_fmt(format_args!("-{n} AP")),
            ApplyEffect::GainStamina(n) => f.write_fmt(format_args!("+{n} stamina")),
            ApplyEffect::GainHealth(n) => f.write_fmt(format_args!("{n}")),
            ApplyEffect::Condition(apply_condition) => {
                f.write_fmt(format_args!("{}", apply_condition.condition.name()))
            }
            ApplyEffect::PerBleeding {
                damage,
                caster_healing_percentage,
            } => {
                f.write_fmt(format_args!("{} damage per Bleeding", damage))?;

                Ok(())
            }
            ApplyEffect::ConsumeCondition { condition } => {
                f.write_fmt(format_args!("|<strikethrough>{}|", condition.name()))
            }
            ApplyEffect::Knockback(..) => f.write_str("Knockback"),
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
    pub required_attack_type: Option<AttackType>,
    pub used_hand: Option<HandType>,
    pub target: OnAttackedReactionTarget,
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
pub enum OnAttackedReactionTarget {
    OnlySelf,
    SelfOrAdjacentAlly,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct OnHitReaction {
    pub name: &'static str,
    pub description: &'static str,
    pub icon: IconId,
    pub action_point_cost: u32,
    pub stamina_cost: u32,
    pub effect: OnHitReactionEffect,
    pub required_attack_type: Option<AttackType>,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum AttackType {
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
    Hastened,
    Inspired,
    Exposed,
    Hindered,
    ReaperApCooldown,
    BloodRage,
    CriticalCharge,
    ThrillOfBattle,
    Adrenalin,
    ArcaneSurge,
    HealthPotionRecovering,
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
            Hastened => "Hastened",
            Inspired => "Inspired",
            Exposed => "Exposed",
            Hindered => "Hindered",
            ReaperApCooldown => "Reaper",
            BloodRage => "Blood rage",
            CriticalCharge => "Critical charge",
            ThrillOfBattle => "Thrill of battle",
            Adrenalin => "Adrenalin",
            ArcaneSurge => "Arcane surge",
            HealthPotionRecovering => "Recovering",
        }
    }

    pub const fn description(&self) -> &'static str {
        use Condition::*;
        match self {
            Dazed => "|<value>-5| |<shield>| |<stat>Evasion|, Disadvantage on attacks.",
            Blinded => "Disadvantage, always Flanked when attacked.",
            Raging => "Advantage on melee attacks (until end of turn).",
            Slowed => "|<value>-2| AP per turn, |<value>-25%| movement",
            Hastened => "|<value>+1| AP per turn, |<value>+25%| movement",
            Inspired => "|<value>+3| |<shield>|<stat>Will|, |<value>+3| |<dice>| |<stat>Attack/Spell|",
            Exposed => "|<value>-5| to all |<shield>|, |<value>-50%| armor.",
            Hindered => "|<value>-50%| movement.",
            Protected => "Takes |<value>-30%| damage from the next attack.",
            Bleeding => "Deals |<value>x| damage over time. (50% of remaining at the end of each turn)",
            Burning => "End of turn: deals |<value>x| damage. 50% spreads to adjacent.",
            Braced => "|<value>+3| |<shield>|<stat>Evasion| against the next attack.",
            Distracted => "|<value>-6| |<shield>|<stat>Evasion| against the next attack.",
            Weakened => "|<value>-x| to all |<shield>| and |dice>.",
            MainHandExertion => "-x on further similar actions.",
            OffHandExertion => "-x on further similar actions.",
            Encumbered => "|<value>-x| |<shield>|<stat>Evasion|, |<value>-x| on |<dice>|.",
            NearDeath => "|<value>-1| AP regen, Disadvantage on actions, enemies have Advantage. (Triggers on < 25% health)",
            Dead => "This character is dead.",
            ReaperApCooldown => "Can not gain more AP from Reaper this turn.",
            BloodRage => "|<value>+5| |<dice>| |<stat>Attack| (passive skill).",
            CriticalCharge => "|<value>+5| |<dice>| |<stat>Spell| (passive skill).",
            ThrillOfBattle => "|<value>+5| |<dice>| |<stat>Attack/Spell| (passive skill).",
            Adrenalin => "|<value>+1| AP per turn.",
            ArcaneSurge => "|<value>+x| |<dice>| |<stat>Spell|. Decays 1 at end of turn.",
            HealthPotionRecovering => "End of turn: |<heart>| heal |<value>2|",
        }
    }

    pub const fn is_positive(&self) -> bool {
        use Condition::*;
        match self {
            Protected => true,
            Braced => true,
            Dazed => false,
            Bleeding => false,
            Burning => false,
            Blinded => false,
            Raging => true,
            Distracted => false,
            Weakened => false,
            MainHandExertion => false,
            OffHandExertion => false,
            Encumbered => false,
            NearDeath => false,
            Dead => false,
            Slowed => false,
            Hastened => true,
            Inspired => true,
            Exposed => false,
            Hindered => false,
            ReaperApCooldown => false,
            BloodRage => true,
            CriticalCharge => true,
            ThrillOfBattle => true,
            Adrenalin => true,
            ArcaneSurge => true,
            HealthPotionRecovering => true,
        }
    }

    pub const fn has_cumulative_stacking(&self) -> bool {
        use Condition::*;
        match self {
            Bleeding | Burning | ArcaneSurge => true,
            _ => false,
        }
    }

    pub const fn status_icon(&self) -> StatusId {
        use Condition::*;
        match self {
            Protected => StatusId::Protected,
            Braced => StatusId::Protected,
            Dazed => StatusId::Dazed,
            Bleeding => StatusId::Bleeding,
            Burning => StatusId::Burning,
            HealthPotionRecovering => StatusId::Healing,
            Hindered => StatusId::Hindered,
            Blinded => StatusId::Blinded,
            Exposed => StatusId::Exposed,
            Slowed => StatusId::Slowed,
            Hastened => StatusId::Hastened,
            Inspired => StatusId::Inspired,
            NearDeath => StatusId::NearDeath,
            Dead => StatusId::Dead,
            CriticalCharge => StatusId::CriticalCharge,
            ReaperApCooldown => StatusId::ReaperApCooldown,
            BloodRage => StatusId::Rage,
            Raging => StatusId::Rage,
            _ => {
                if self.is_positive() {
                    StatusId::PlaceholderPositive
                } else {
                    StatusId::PlaceholderNegative
                }
            }
        }
    }
}

const PROTECTED_ARMOR_BONUS: u32 = 1;
const BRACED_DEFENSE_BONUS: u32 = 3;
const DISTRACTED_DEFENSE_PENALTY: u32 = 6;
const DAZED_EVASION_PENALTY: u32 = 5;
const EXPOSED_DEFENSE_PENALTY: u32 = 5;
const INSPIRED_WILL_BONUS: u32 = 3;
const SLOWED_AP_PENALTY: u32 = 2;
const HASTENED_AP_BONUS: u32 = 1;

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct ConditionInfo {
    pub condition: Condition,
    pub name: &'static str,
    pub description: &'static str,
    pub is_positive: bool,
    pub stacks: Option<u32>,
    pub remaining_rounds: Option<u32>,
}

impl ConditionInfo {
    pub fn populated_description(&self) -> String {
        if let Some(stacks) = self.stacks {
            self.condition
                .description()
                .replace("|<value>-x|", &format!("|<value>-{stacks}|"))
                .replace("|<value>+x|", &format!("|<value>+{stacks}|"))
                .replace("|<value>x|", &format!("|<value>{stacks}|"))
        } else {
            self.condition.description().to_string()
        }
    }
}

impl Display for ConditionInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)?;
        if let Some(stacks) = self.stacks {
            write!(f, " x {}", stacks)?;
        }
        if let Some(remaining_rounds) = self.remaining_rounds {
            write!(f, " ({} remaining)", remaining_rounds)?;
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
pub struct Conditions {
    map: IndexMap<Condition, ConditionState>,
}

impl Conditions {
    pub fn remove(&mut self, condition: &Condition) -> bool {
        self.map.shift_remove(condition).is_some()
    }

    fn get(&self, condition: &Condition) -> Option<&ConditionState> {
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
            state
                .ends_at
                .map(|ends_at| ends_at > game_time)
                .unwrap_or(true)
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

    pub fn target_id(&self) -> Option<CharacterId> {
        match self {
            Self::Character(target_id, ..) => Some(*target_id),
            _ => None,
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
        }
    }

    pub fn mana_cost(&self) -> u32 {
        match self {
            BaseAction::Attack { .. } => 0,
            BaseAction::UseAbility(ability) => ability.mana_cost,
            BaseAction::Move => 0,
            BaseAction::ChangeEquipment => 0,
            BaseAction::UseConsumable => 0,
        }
    }

    pub fn stamina_cost(&self) -> u32 {
        match self {
            BaseAction::Attack { .. } => 0,
            BaseAction::UseAbility(ability) => ability.stamina_cost,
            BaseAction::Move => 0,
            BaseAction::ChangeEquipment => 0,
            BaseAction::UseConsumable => 0,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum HandType {
    MainHand,
    OffHand,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct AbilityChargeFx {
    pub(crate) particle_shape: ParticleShape,
    pub(crate) sound: SoundId,
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
    pub initiate_sound: Option<SoundId>,
    pub resolve_sound: Option<SoundId>,
    pub charge_fx: Option<AbilityChargeFx>,
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

    pub fn targets_single_enemy(&self) -> bool {
        matches!(self.target, AbilityTarget::Enemy { .. })
    }

    pub fn targets_single_character(&self) -> bool {
        matches!(
            self.target,
            AbilityTarget::Ally { .. } | AbilityTarget::Enemy { .. }
        )
    }

    pub fn requires_target(&self) -> bool {
        match self.target {
            AbilityTarget::Enemy { .. } => true,
            AbilityTarget::Ally { .. } => true,
            AbilityTarget::Area { .. } => true,
            AbilityTarget::None { .. } => false,
        }
    }

    pub fn has_knockback(&self) -> bool {
        match self.target {
            AbilityTarget::Enemy { effect, .. } => effect.has_knockback(),
            _ => false,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum EquipmentRequirement {
    Weapon(WeaponType),
    Shield,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum AbilityId {
    Tackle,
    ShieldBash,
    SweepAttack,
    LungeAttack,
    Brace,
    Scream,
    ShackledMind,
    MindBlast,
    InflictWounds,
    PiercingShot,
    Heal,
    HealingNova,
    SelfHeal,
    HealingRain,
    Inspire,
    Haste,
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
    RollDuringAttack(i32),
    RollAbilityWithAttackModifier,
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

    pub fn has_knockback(&self) -> bool {
        if let AbilityNegativeEffect::Spell(sne) = self {
            for effect in sne.on_hit.iter().flatten().flatten() {
                if matches!(effect, ApplyEffect::Knockback { .. }) {
                    return true;
                }
            }
        }
        false
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
        impact_circle: Option<(Range, AreaTargetAcquisition, AbilityNegativeEffect)>,
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
    pub shape: AreaShape,
    pub acquisition: AreaTargetAcquisition,
    pub effect: AbilityEffect,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum AreaShape {
    Circle(Range),
    Line,
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
                self_area
                    .as_ref()
                    .and_then(|area_effect| match area_effect.shape {
                        AreaShape::Circle(range) => Some(range),
                        AreaShape::Line => None,
                    })
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

    pub apply_on_self_per_area_target_hit: Option<[Option<ApplyEffect>; 2]>,
}

impl AbilityEnhancement {
    pub fn attack_enhancement_effect(&self) -> Option<(&'static str, AttackEnhancementEffect)> {
        self.attack_effect
            .map(|attack_effect| (self.name, attack_effect))
    }

    pub const fn default() -> Self {
        Self {
            ability_id: AbilityId::Kill,
            name: "<placeholder>",
            description: "",
            icon: IconId::Equip,
            action_point_cost: 0,
            mana_cost: 0,
            stamina_cost: 0,
            spell_effect: None,
            attack_effect: None,
            apply_on_self_per_area_target_hit: None,
        }
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

    pub improved_graze: bool,
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
            improved_graze: false,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum AbilityEnhancementEffect {
    Spell(SpellEnhancementEffect),
    Attack(AttackEnhancementEffect),
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
    Target(Option<DefenseType>, ApplyEffect),
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum DefenseType {
    Will,
    Evasion,
    Toughness,
}

impl DefenseType {
    fn name(&self) -> &'static str {
        match self {
            DefenseType::Will => "will",
            DefenseType::Evasion => "evasion",
            DefenseType::Toughness => "toughness",
        }
    }
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
        6.0 + self.agility.get() as f32 * 0.5
    }

    fn max_health(&self) -> u32 {
        13 + self.strength.get() * 2
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
    id: Cell<Option<CharacterId>>,
    index_in_round: Cell<Option<u32>>,
    current_game_time: Cell<u32>,
    round_length: Cell<Option<u32>>,
    pub is_part_of_active_group: Cell<bool>,
    pub has_taken_a_turn_this_round: Cell<bool>,
    pub has_used_main_hand_reaction_this_round: Cell<bool>,
    pub has_used_off_hand_reaction_this_round: Cell<bool>,

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
    pub conditions: RefCell<Conditions>,
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

    pub is_facing_east: Cell<bool>,
    is_being_pushed_in_direction: Cell<Option<(i32, i32)>>,
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
            id: Cell::new(None),
            index_in_round: Cell::new(None),
            round_length: Cell::new(None),
            has_taken_a_turn_this_round: Cell::new(false),
            has_used_main_hand_reaction_this_round: Cell::new(false),
            has_used_off_hand_reaction_this_round: Cell::new(false),
            is_part_of_active_group: Cell::new(false),
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
            ]),
            known_attacked_reactions: Default::default(),
            known_on_hit_reactions: Default::default(),
            known_ability_enhancements: Default::default(),
            known_passive_skills: Default::default(),
            is_engaged_by: Default::default(),
            engagement_target: Default::default(),
            changed_equipment_listeners: Default::default(),
            is_facing_east: Cell::new(false),
            is_being_pushed_in_direction: Cell::new(None),
        }
    }

    fn set_facing_toward(&self, position: Position) {
        let dx = position.0 - self.position.get().0;
        if dx > 0 {
            self.is_facing_east.set(true);
        } else if dx < 0 {
            self.is_facing_east.set(false);
        }
    }

    fn set_position(&self, new_pos: Position) {
        self.position.set(new_pos);
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

    fn spend_mana(&self, amount: u32) {
        self.mana.spend(amount);
        self.on_mana_changed();
    }

    fn on_mana_changed(&self) {
        let add = self
            .known_passive_skills
            .contains(&PassiveSkill::CriticalCharge)
            && self.mana.ratio() <= 0.5;
        self.conditions
            .borrow_mut()
            .add_or_remove(Condition::CriticalCharge, add);
    }

    fn regain_full_movement(&self) {
        self.remaining_movement.set(self.move_speed());
    }

    fn spend_movement(&self, distance: f32) {
        let remaining = self.remaining_movement.get();
        assert!(
            distance > 0.0 && distance <= remaining,
            "{} {}",
            distance,
            remaining
        );
        self.remaining_movement.set(remaining - distance);
    }

    fn gain_movement(&self, distance: f32) {
        let remaining = self.remaining_movement.get();
        assert!(
            distance > 0.0 && remaining >= 0.0,
            "{} {}",
            distance,
            remaining
        );
        self.remaining_movement.set(remaining + distance);
    }

    fn maybe_gain_resources_from_reaper(&self, num_killed: u32) -> Option<(u32, u32)> {
        if self.known_passive_skills.contains(&PassiveSkill::Reaper) {
            let sta = self.stamina.gain(num_killed);
            let ap = if self.conditions.borrow().has(&Condition::ReaperApCooldown) {
                0
            } else {
                self.action_points.gain(2)
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
        let mut modifier = 1.0;
        if self.conditions.borrow().has(&Condition::Hindered) {
            modifier -= 0.5;
        }
        if self.conditions.borrow().has(&Condition::Slowed) {
            modifier -= 0.25;
        }
        if self.conditions.borrow().has(&Condition::Hastened) {
            modifier += 0.25;
        }
        self.base_move_speed.get() * modifier
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
        self.conditions.borrow_mut().remove(&Condition::Distracted)
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
                (remaining as f32 / self.round_length.get().unwrap() as f32).ceil() as u32
            });
            let info = ConditionInfo {
                condition: *condition,
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

    pub fn occupies_cell(&self, pos: Position) -> bool {
        self.pos() == pos
            || ((pos.0 - self.pos().0).abs() <= 1 && (pos.1 - self.pos().1).abs() <= 1)
    }

    pub fn id(&self) -> CharacterId {
        self.id.get().unwrap()
    }

    pub fn set_id(&self, id: CharacterId) {
        assert!(
            self.id.get().is_none(),
            "set_id() should only be used at initialisation. Tried to set id = {}, but {} already has id = {:?}",
            id, self.name, self.id
        );
        self.id.set(Some(id));
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

    fn on_battle_start(&self) {
        self.update_player_encumbrance();
        self.action_points.current.set(ACTION_POINTS_PER_TURN);
        self.regain_full_movement();
        self.on_health_changed();
        self.has_taken_a_turn_this_round.set(false);
        self.has_used_main_hand_reaction_this_round.set(false);
        self.has_used_off_hand_reaction_this_round.set(false);
    }

    fn on_new_round(&self) {
        self.has_taken_a_turn_this_round.set(false);
        self.has_used_main_hand_reaction_this_round.set(false);
        self.has_used_off_hand_reaction_this_round.set(false);
    }

    fn update_player_encumbrance(&self) {
        if self.player_controlled() {
            let encumbrance = self.equipment_weight().saturating_sub(self.capacity.get());
            self.conditions
                .borrow_mut()
                .set_stacks(Condition::Encumbered, encumbrance);
        }
    }

    fn has_any_consumable_in_inventory(&self) -> bool {
        self.inventory
            .iter()
            .any(|entry| matches!(entry.get(), Some(EquipmentEntry::Consumable(..))))
    }

    fn on_changed_equipment(&self) {
        self.update_player_encumbrance();

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

        if let (Some(EquipmentEntry::Arrows(from_arrow)), Some(EquipmentEntry::Arrows(to_arrow))) =
            (from_content, to_content)
        {
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

    pub fn reaches_with_attack(
        &self,
        hand: HandType,
        target_position: Position,
        enhancements: impl Iterator<Item = AttackEnhancementEffect>,
    ) -> (Range, ActionReach) {
        let weapon = self.weapon(hand).unwrap();
        let weapon_range = weapon.range;

        match weapon_range {
            WeaponRange::Melee => {
                if target_within_range_squared(
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

                if target_within_range_squared(
                    (range + modifier as f32).powf(2.0),
                    self.position.get(),
                    target_position,
                ) {
                    if target_within_range_squared(
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
        target_pos: Position,
    ) -> bool {
        let range = ability.target.range(enhancements).unwrap();
        match ability.target {
            AbilityTarget::Enemy { .. } | AbilityTarget::Ally { .. } => {
                target_within_range_squared(range.squared(), self.position.get(), target_pos)
            }
            AbilityTarget::Area { .. } => {
                within_range_squared(range.squared(), self.position.get(), target_pos)
            }
            AbilityTarget::None { .. } => {
                panic!("Ability that has no target always reaches. Shouldn't be checked")
            }
        }
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
                if self.weapon(attack_action.hand).is_some() {
                    return Some(*attack_action);
                }
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
        }
    }

    pub fn has_enough_ap_for_action(&self, action: BaseAction) -> bool {
        let ap = self.action_points.current();
        match action {
            BaseAction::Attack(attack) => {
                matches!(self.weapon(attack.hand), Some(weapon) if ap >= weapon.action_point_cost)
            }
            BaseAction::UseAbility(ability) => ap >= ability.action_point_cost,
            BaseAction::Move => true,
            BaseAction::ChangeEquipment => {
                ap as i32 >= BaseAction::ChangeEquipment.action_point_cost()
            }
            BaseAction::UseConsumable => ap as i32 >= BaseAction::UseConsumable.action_point_cost(),
        }
    }

    pub fn usable_abilities(&self) -> Vec<Ability> {
        return self
            .known_actions
            .borrow()
            .iter()
            .filter(|a| self.can_use_action(**a))
            .filter_map(|action| match action {
                BaseAction::UseAbility(ability) => Some(*ability),
                _ => None,
            })
            .collect();
    }

    pub fn usable_single_target_abilities(&self) -> Vec<Ability> {
        return self
            .known_actions
            .borrow()
            .iter()
            .filter(|a| self.can_use_action(**a))
            .filter_map(|action| match action {
                BaseAction::UseAbility(ability) if ability.targets_single_enemy() => Some(*ability),
                _ => None,
            })
            .collect();
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

            if let Some(arrows) = self.arrows.get() {
                assert!(arrows.quantity > 0);
                known.push((
                    arrows.arrow.name.to_string(),
                    AttackEnhancement {
                        name: "Use special arrow",
                        description: arrows.arrow.name,
                        icon: IconId::RangedAttack,
                        weapon_requirement: Some(WeaponType::Ranged),
                        effect: AttackEnhancementEffect {
                            consume_equipped_arrow: true,
                            ..AttackEnhancementEffect::default()
                        },
                        ..AttackEnhancement::default()
                    },
                ));
            }
        }

        known
    }

    pub fn usable_attack_enhancements(&self, attack_hand: HandType) -> Vec<AttackEnhancement> {
        let usable: Vec<AttackEnhancement> = self
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

    pub fn known_on_attacked_reactions(&self) -> Vec<OnAttackedReaction> {
        let mut known = vec![];
        for reaction in &self.known_attacked_reactions {
            known.push(*reaction);
        }
        if let Some(weapon) = &self.weapon(HandType::MainHand) {
            if let Some(reaction) = weapon.on_attacked_reaction {
                known.push(reaction);
            }
        }
        if let Some(shield) = &self.shield() {
            if let Some(reaction) = shield.on_attacked_reaction {
                known.push(reaction);
            }
        }
        known
    }

    pub fn usable_on_attacked_reactions(
        &self,
        is_within_melee: bool,
        self_defense: bool,
    ) -> Vec<OnAttackedReaction> {
        let mut usable = self.known_on_attacked_reactions();
        usable.retain(|reaction| {
            self.can_use_on_attacked_reaction(*reaction, is_within_melee, self_defense)
        });
        usable
    }

    fn can_use_on_attacked_reaction(
        &self,
        reaction: OnAttackedReaction,
        is_within_melee: bool,
        self_defense: bool,
    ) -> bool {
        if match reaction.required_attack_type {
            Some(AttackType::Melee) => !is_within_melee,
            Some(AttackType::Ranged) => is_within_melee,
            None => false,
        } {
            return false;
        }

        if match reaction.used_hand {
            Some(HandType::MainHand) => self.has_used_main_hand_reaction_this_round.get(),
            Some(HandType::OffHand) => self.has_used_off_hand_reaction_this_round.get(),
            None => false,
        } {
            return false;
        }

        if !self_defense && reaction.target == OnAttackedReactionTarget::OnlySelf {
            return false;
        }

        let ap = self.action_points.current();
        ap >= reaction.action_point_cost && self.stamina.current() >= reaction.stamina_cost
    }

    fn on_use_on_attacked_reaction(&self, reaction: OnAttackedReaction) {
        self.action_points.spend(reaction.action_point_cost);
        self.stamina.spend(reaction.stamina_cost);
        match reaction.used_hand {
            Some(HandType::MainHand) => self.has_used_main_hand_reaction_this_round.set(true),
            Some(HandType::OffHand) => self.has_used_off_hand_reaction_this_round.set(true),
            None => {}
        }
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
        let allowed = match reaction.required_attack_type {
            None => true,
            Some(AttackType::Melee) => is_within_melee,
            Some(AttackType::Ranged) => !is_within_melee,
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
        if conditions.has(&Condition::Inspired) {
            res += 3;
        }
        if conditions.has(&Condition::CriticalCharge) {
            res += 5;
        }
        if conditions.has(&Condition::ThrillOfBattle) {
            res += 5;
        }
        res += conditions.get_stacks(&Condition::ArcaneSurge);

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

    pub fn defense(&self, defense_type: DefenseType) -> u32 {
        match defense_type {
            DefenseType::Will => self.will(),
            DefenseType::Evasion => self.evasion(),
            DefenseType::Toughness => self.toughness(),
        }
    }

    pub fn will(&self) -> u32 {
        let mut res = 10 + self.intellect() * 2;
        let conditions = self.conditions.borrow();
        if conditions.has(&Condition::Inspired) {
            res += INSPIRED_WILL_BONUS;
        }
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

        /*
        if let Some(state) = self.conditions.borrow().get(&Condition::Protected) {
            protection += state.stacks.unwrap();
        }
         */

        if self
            .known_passive_skills
            .contains(&PassiveSkill::HardenedSkin)
        {
            protection += 1;
        }

        if self.has_condition(&Condition::Exposed) {
            protection /= 2;
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
        if conditions.has(&Condition::Inspired) {
            res += 3;
        }
        if conditions.has(&Condition::BloodRage) {
            res += 5;
        }
        if conditions.has(&Condition::ThrillOfBattle) {
            res += 5;
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
            bonuses.push(("Flanked", RollBonusContributor::FlatAmount(5)));
        }

        let (_range, reach) = self.reaches_with_attack(
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

        if is_spell && conditions.has(&Condition::CriticalCharge) {
            // It's applied from spell_modifier()
            bonuses.push(("Critical charge", RollBonusContributor::OtherPositive));
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
        for (_label, bonus) in self.incoming_attack_bonuses(reaction) {
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
                let current = state.stacks.as_mut().unwrap();
                if condition.has_cumulative_stacking() {
                    *current += stacks;
                } else {
                    *current = (*current).max(stacks);
                }
            }
        } else {
            if condition == Condition::Hindered {
                self.remaining_movement
                    .set(self.remaining_movement.get() - self.base_move_speed.get() * 0.5);
            } else if condition == Condition::Slowed {
                self.action_points.lose(SLOWED_AP_PENALTY);
                self.remaining_movement
                    .set(self.remaining_movement.get() - self.base_move_speed.get() * 0.25);
            } else if condition == Condition::Hastened {
                self.action_points.gain(HASTENED_AP_BONUS);
                self.remaining_movement
                    .set(self.remaining_movement.get() + self.base_move_speed.get() * 0.25);
            }

            conditions
                .map
                .insert(condition, ConditionState { stacks, ends_at });
        }
    }

    fn clear_condition(&self, condition: Condition) -> Option<u32> {
        let mut conditions = self.conditions.borrow_mut();

        if !conditions.has(&condition) {
            return None;
        }

        let prev_stacks = conditions.get_stacks(&condition);

        conditions.remove(&condition);

        Some(prev_stacks)
    }

    fn has_condition(&self, condition: &Condition) -> bool {
        self.conditions.borrow().has(condition)
    }
}

fn is_target_flanked(attacker_pos: Position, target: &Character) -> bool {
    /*
    println!(
        "Check if target {} (pos={:?}) is flanked, from attacker pos {:?} ...",
        target.name,
        target.pos(),
        attacker_pos
    );
     */
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
    let engaged_from = (melee_engager.0 - target.0, melee_engager.1 - target.1);
    let (dx, dy) = (attacker.0 - target.0, attacker.1 - target.1);
    // TODO: this panicked, when using Sweeping attack, after moving to a cell where an enemy had just died (?).
    if (dx, dy) == (0, 0) {
        panic!(
            "Invalid dx,dy: {:?}. Engager={:?}, target={:?}",
            (dx, dy),
            melee_engager,
            target
        );
    }

    match engaged_from {
        // from east
        (3, eng_y) if (-1..=1).contains(&eng_y) => dx < 0 && dy.abs() <= dx.abs(),
        // from west
        (-3, eng_y) if (-1..=1).contains(&eng_y) => dx > 0 && dy.abs() <= dx.abs(),
        // from north
        (eng_x, -3) if (-1..=1).contains(&eng_x) => dy > 0 && dx.abs() <= dy.abs(),
        // from south
        (eng_x, 3) if (-1..=1).contains(&eng_x) => dy < 0 && dx.abs() <= dy.abs(),
        // from some diagonal
        (eng_x, eng_y) => {
            // from northwest
            if eng_x < 0 && eng_y < 0 {
                dx >= 0 && dy >= 0
            }
            // from northeast
            else if eng_x > 0 && eng_y < 0 {
                dx <= 0 && dy >= 0
            }
            // from southwest
            else if eng_x < 0 && eng_y > 0 {
                dx >= 0 && dy <= 0
            }
            // from southeast
            else if eng_x > 0 && eng_y > 0 {
                dx <= 0 && dy <= 0
            }
            // invalid
            else {
                // TODO: Bug / crash:
                // Invalid engagement direction: (0, -2). Engager=(9, 18), target=(9, 20)
                // When using Sweeping attack while standing above 2 targets like so?
                //
                //   XXX
                //   XXX
                //   XXX
                // 000 111
                // 000 111
                // 000 111
                //
                println!("-------------------------");
                println!(
                    "ERROR: Invalid engagement direction: {:?}. Engager={:?}, target={:?}",
                    (eng_x, eng_y),
                    melee_engager,
                    target
                );
                println!("-------------------------");
                false
            }
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

pub fn is_target_within_shape(
    caster_pos: Position,
    area_pos: Position,
    shape: AreaShape,
    target: &Character,
) -> bool {
    match shape {
        AreaShape::Circle(radius) => {
            //let margin = 1.42; // Just large enough to encompass the corner cells occupied by the character
            target_within_range_squared((f32::from(radius)).powf(2.0), area_pos, target.pos())
        }
        AreaShape::Line => {
            let mut is_hit = false;
            line_collision(caster_pos, area_pos, |x, y| {
                if target.occupies_cell((x, y)) {
                    is_hit = true;
                }
            });
            is_hit
        }
    }
}

pub fn within_range_squared(range_squared: f32, source: Position, destination: Position) -> bool {
    let distance_squared = (destination.0 - source.0).pow(2) + (destination.1 - source.1).pow(2);
    distance_squared as f32 <= range_squared
}

pub fn target_within_range_squared(range_squared: f32, source: Position, target: Position) -> bool {
    for x in target.0 - 1..=target.0 + 1 {
        for y in target.1 - 1..=target.1 + 1 {
            let distance_squared = (x - source.0).pow(2) + (y - source.1).pow(2);
            if distance_squared as f32 <= range_squared {
                return true;
            }
        }
    }
    false
}

/*
This is the max distance to be considered melee:

AAA
AAA
AAABBB
BBB
BBB

I.e. two sides must be touching. The distance from A center to B edge
is therefore sqrt(1^2 + 2^2)
*/
pub const TOUCHING_MELEE_RANGE_SQUARED: f32 = 5.0;
pub const CENTER_MELEE_RANGE_SQUARED: f32 = 13.0;

fn within_meele(source: Position, destination: Position) -> bool {
    within_range_squared(CENTER_MELEE_RANGE_SQUARED, source, destination)
}

pub fn sq_distance_between(source: Position, destination: Position) -> f32 {
    ((destination.0 - source.0).pow(2) + (destination.1 - source.1).pow(2)) as f32
}

pub fn distance_between(source: Position, destination: Position) -> f32 {
    sq_distance_between(source, destination).sqrt()
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
    Ranged(f32),
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
            Self::Melee => TOUCHING_MELEE_RANGE_SQUARED as f32,
            Self::Ranged(range) => range.powf(2.0),
        }
    }

    pub fn center_to_center_squared(&self) -> f32 {
        match self {
            // 2^ + 3^2
            WeaponRange::Melee => CENTER_MELEE_RANGE_SQUARED,
            // Add sqrt(2.0) to reach one extra diagonal cell, i.e. to the center cell of the target (?)
            WeaponRange::Ranged(range) => (range + f32::sqrt(2.0)).powf(2.0),
        }
    }

    pub fn into_range(self) -> Range {
        match self {
            Self::Melee => Range::Melee,
            Self::Ranged(r) => Range::Float(r),
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
            Range::Float(range) => f.write_fmt(format_args!("{:.1}", range)),
        }
    }
}

impl Range {
    pub fn squared(&self) -> f32 {
        match self {
            Range::Melee => TOUCHING_MELEE_RANGE_SQUARED,
            Range::Ranged(range) => range.pow(2) as f32,
            Range::ExtendableRanged(range) => range.pow(2) as f32,
            Range::Float(range) => range.powf(2.0),
        }
    }

    pub fn center_to_center_squared(&self) -> f32 {
        let range = match self {
            Range::Melee => return CENTER_MELEE_RANGE_SQUARED,
            Range::Ranged(range) => *range as f32,
            Range::ExtendableRanged(range) => *range as f32,
            Range::Float(range) => *range,
        };

        // Add sqrt(2.0) to reach one extra diagonal cell, i.e. to the center cell of the target (?)
        (range + f32::sqrt(2.0)).powf(2.0)
    }

    pub fn plus(&self, n: u32) -> Range {
        match self {
            Range::Melee => Range::Float(TOUCHING_MELEE_RANGE_SQUARED.sqrt() + n as f32),
            Range::Ranged(range) => Range::Ranged(range + n),
            Range::ExtendableRanged(range) => Range::Ranged(range + n),
            Range::Float(range) => Range::Float(range + n as f32),
        }
    }

    pub fn plusf(&self, n: f32) -> Range {
        match self {
            Range::Melee => Range::Float(TOUCHING_MELEE_RANGE_SQUARED.sqrt() + n),
            Range::Ranged(range) => Range::Float(*range as f32 + n),
            Range::ExtendableRanged(range) => Range::Float(*range as f32 + n),
            Range::Float(range) => Range::Float(range + n),
        }
    }
}

impl From<Range> for f32 {
    fn from(range: Range) -> Self {
        match range {
            Range::Melee => TOUCHING_MELEE_RANGE_SQUARED.sqrt(),
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
