use std::cell::{Cell, RefCell};

use std::fmt::Display;
use std::rc::{Rc, Weak};

use macroquad::color::Color;

use crate::d20::{probability_of_d20_reaching, roll_d20_with_advantage, DiceRollBonus};

use crate::game_ui_connection::GameUserInterfaceConnection;
use crate::init::GameInitState;
use crate::pathfind::PathfindGrid;
use crate::textures::{EquipmentIconId, IconId, PortraitId, SpriteId};

pub type Position = (i32, i32);

pub const MAX_ACTION_POINTS: u32 = 5;

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

    pub async fn run(mut self) {
        for char in self.characters.iter() {
            let encumbrance = char.equipment_weight() as i32 - char.capacity as i32;
            if encumbrance > 0 {
                char.receive_condition(Condition::Encumbered(encumbrance as u32));
            }
        }

        loop {
            println!(
                "UI SELECT ACTION ... (active char = {})",
                self.active_character().name
            );
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
                }

                {
                    if self.active_character().action_points.current() == 0 {
                        self.perform_end_of_turn_character().await;
                        self.active_character_id =
                            self.characters.next_id(self.active_character_id);
                        self.notify_ui_of_new_turn().await;
                    }
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
            for id in self.characters.remove_dead() {
                let new_active = if active_character_died {
                    Some(self.active_character_id)
                } else {
                    None
                };
                self.ui_handle_event(GameEvent::CharacterDied {
                    character: id,
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

    pub fn is_players_turn(&self) -> bool {
        self.active_character().player_controlled
    }

    pub fn player_positions(&self) -> Vec<Position> {
        let mut positions = vec![];
        for character in self.characters.iter() {
            if character.player_controlled {
                positions.push(character.pos());
            }
        }
        positions
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

                self.perform_attack(
                    self.active_character_id,
                    hand,
                    enhancements,
                    target,
                    reaction,
                )
                .await
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
                stamina_cost,
                positions,
            } => {
                let character = self.active_character();
                character.action_points.spend(action_point_cost);
                character.stamina.spend(stamina_cost);
                self.perform_movement(positions).await;
                None
            }

            Action::ChangeEquipment { from, to } => {
                let character = self.active_character();
                character.action_points.spend(1);
                let from_content = character.equipment(from);
                let to_content = character.equipment(to);
                character.set_equipment(from_content, to);
                character.set_equipment(to_content, from);

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
                if other_char.player_controlled != character.player_controlled
                    && within_meele(character.pos(), other_char.pos())
                    && !within_meele(new_position, other_char.pos())
                    && other_char.can_use_opportunity_attack()
                {
                    let reactor = other_char;

                    let chooses_to_use_opportunity_attack = self
                        .user_interface
                        .choose_opportunity_attack(
                            self,
                            reactor.id(),
                            character.id(),
                            (character.pos(), new_position),
                        )
                        .await;

                    dbg!(chooses_to_use_opportunity_attack);

                    if chooses_to_use_opportunity_attack {
                        self.ui_handle_event(GameEvent::CharacterReactedWithOpportunityAttack {
                            reactor: reactor.id(),
                        })
                        .await;

                        reactor.action_points.spend(1);

                        self.perform_attack(
                            reactor.id(),
                            HandType::MainHand,
                            vec![],
                            character.id(),
                            None,
                        )
                        .await;
                    }
                }
            }

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

    async fn perform_spell(
        &mut self,

        spell: Spell,
        enhancements: Vec<SpellEnhancement>,
        selected_target: ActionTarget,
    ) {
        let caster = self.active_character();
        let caster_id = caster.id();

        caster.action_points.spend(spell.action_point_cost);
        caster.mana.spend(spell.mana_cost);
        caster.stamina.spend(spell.stamina_cost);

        for enhancement in &enhancements {
            caster.action_points.spend(enhancement.action_point_cost);
            caster.mana.spend(enhancement.mana_cost);
            caster.stamina.spend(enhancement.stamina_cost);
        }

        let mut cast_n_times = 1;
        for enhancement in &enhancements {
            if enhancement.effect.cast_twice {
                cast_n_times = 2;
            }
        }

        for i in 0..cast_n_times {
            let caster_ref = self.characters.get(caster_id);

            let mut detail_lines = vec![];

            let mut advantange_level = 0_i32;

            for enhancement in &enhancements {
                let bonus = enhancement.effect.bonus_advantage;
                if bonus > 0 {
                    advantange_level += bonus as i32;
                }
            }

            let roll = roll_d20_with_advantage(advantange_level);

            detail_lines.push(roll_description(advantange_level));

            let mut line = format!("Rolled: {}", roll);
            let mut spell_roll_calculation = roll as i32;
            match spell.modifier {
                SpellModifier::Spell => {
                    spell_roll_calculation += caster_ref.spell_modifier() as i32;
                    line.push_str(&format!(" (+{} spell mod)", caster_ref.spell_modifier()));
                }
                SpellModifier::Attack(bonus) => {
                    spell_roll_calculation +=
                        caster_ref.attack_modifier(HandType::MainHand) as i32 + bonus;
                    let bonus_str = if bonus < 0 {
                        format!(" -{}", -bonus)
                    } else if bonus > 0 {
                        format!(" +{}", bonus)
                    } else {
                        "".to_string()
                    };
                    line.push_str(&format!(
                        " (+{} attack mod{})",
                        caster_ref.attack_modifier(HandType::MainHand),
                        bonus_str,
                    ));
                }
            };

            for enhancement in &enhancements {
                let bonus = enhancement.effect.roll_bonus;
                if bonus > 0 {
                    spell_roll_calculation += bonus as i32;
                    line.push_str(&format!(" +{} ({})", bonus, enhancement.name,));
                }
            }

            let spell_result = spell_roll_calculation as u32;
            line.push_str(&format!(" = {}", spell_result));

            let mut target_outcome = None;
            let mut area_outcomes = None;

            match spell.target {
                SpellTarget::Enemy {
                    effect,
                    impact_area,
                    ..
                } => {
                    let ActionTarget::Character(target_id, movement) = &selected_target else {
                        unreachable!()
                    };

                    // TODO (is it ok to perform movement right inside perform_spell? No weird state interactions?)
                    if let Some(positions) = movement {
                        println!("WILL PERFORM MOVEMENT {positions:?}");
                        self.perform_movement(positions.clone()).await;
                        println!("PERFORMED MOVEMENT {positions:?}");
                    }

                    let target = self.characters.get(*target_id);
                    assert!(caster.reaches_with_spell(spell, &enhancements, target.position.get()));

                    if let Some(contest) = effect.defense_type {
                        match contest {
                            DefenseType::Will => {
                                line.push_str(&format!(", vs will={}", target.will()))
                            }
                            DefenseType::Evasion => {
                                line.push_str(&format!(", vs evasion={}", target.evasion()))
                            }
                        }
                    }

                    detail_lines.push(line);

                    let outcome = self.perform_spell_enemy_effect(
                        spell.name,
                        spell.modifier,
                        &enhancements,
                        effect,
                        spell_result,
                        target,
                        &mut detail_lines,
                        true,
                    );
                    target_outcome = Some((*target_id, outcome));

                    if let Some((radius, area_effect)) = impact_area {
                        detail_lines.push("Area of effect:".to_string());

                        let area_target_outcomes = self.perform_spell_area_enemy_effect(
                            radius,
                            "AoE",
                            spell.modifier,
                            &enhancements,
                            caster,
                            target.position.get(),
                            &mut detail_lines,
                            spell_result,
                            area_effect,
                        );

                        area_outcomes = Some((target.position.get(), area_target_outcomes));
                    }
                }

                SpellTarget::Ally { range: _, effect } => {
                    let ActionTarget::Character(target_id, movement) = &selected_target else {
                        unreachable!()
                    };
                    let target = self.characters.get(*target_id);
                    assert!(caster.reaches_with_spell(spell, &enhancements, target.position.get()));

                    detail_lines.push(line);

                    let degree_of_success = spell_result / 10;
                    if degree_of_success > 0 {
                        detail_lines.push(format!("Fortune: {}", degree_of_success));
                    }
                    let outcome = self.perform_spell_ally_effect(
                        spell.name,
                        &enhancements,
                        effect,
                        target,
                        &mut detail_lines,
                        degree_of_success,
                    );

                    target_outcome = Some((*target_id, outcome));
                }

                SpellTarget::Area {
                    range: _,
                    radius,
                    effect,
                } => {
                    let ActionTarget::Position(target_pos) = selected_target else {
                        unreachable!()
                    };
                    assert!(caster.reaches_with_spell(spell, &enhancements, target_pos));

                    detail_lines.push(line);

                    let outcomes = self.perform_spell_area_effect(
                        spell.name,
                        spell.modifier,
                        &enhancements,
                        caster,
                        target_pos,
                        radius,
                        &mut detail_lines,
                        spell_result,
                        effect,
                    );

                    area_outcomes = Some((target_pos, outcomes));
                }

                SpellTarget::None {
                    self_area,
                    self_effect,
                } => {
                    detail_lines.push(line);

                    if let Some(effect) = self_effect {
                        let degree_of_success = spell_result / 10;
                        if degree_of_success > 0 {
                            detail_lines.push(format!("Fortune: {}", degree_of_success));
                        }
                        let outcome = self.perform_spell_ally_effect(
                            spell.name,
                            &enhancements,
                            effect,
                            caster,
                            &mut detail_lines,
                            degree_of_success,
                        );
                        target_outcome = Some((caster_id, outcome));
                    }

                    if let Some((radius, effect)) = self_area {
                        let outcomes = self.perform_spell_area_effect(
                            spell.name,
                            spell.modifier,
                            &enhancements,
                            caster,
                            caster.position.get(),
                            radius,
                            &mut detail_lines,
                            spell_result,
                            effect,
                        );
                        area_outcomes = Some((caster.position.get(), outcomes));
                    }
                }
            };

            if i < cast_n_times - 1 {
                detail_lines.push(format!("{} cast again!", caster_ref.name))
            }

            let caster_id = caster_ref.id();

            self.ui_handle_event(GameEvent::SpellWasCast {
                caster: caster_id,
                target_outcome,
                area_outcomes,
                spell,
                detail_lines,
            })
            .await;
        }
    }

    fn perform_spell_area_effect(
        &self,
        name: &'static str,
        modifier: SpellModifier,
        enhancements: &[SpellEnhancement],
        caster: &Character,
        area_center: Position,
        radius: Range,
        detail_lines: &mut Vec<String>,
        spell_result: u32,
        effect: SpellEffect,
    ) -> Vec<(u32, SpellTargetOutcome)> {
        match effect {
            SpellEffect::Enemy(enemy_area) => self.perform_spell_area_enemy_effect(
                radius,
                name,
                modifier,
                enhancements,
                caster,
                area_center,
                detail_lines,
                spell_result,
                enemy_area,
            ),

            SpellEffect::Ally(ally_area) => self.perform_spell_area_ally_effect(
                radius,
                name,
                enhancements,
                caster,
                area_center,
                detail_lines,
                spell_result,
                ally_area,
            ),
        }
    }

    fn perform_spell_area_ally_effect(
        &self,
        mut radius: Range,
        name: &'static str,
        enhancements: &[SpellEnhancement],
        caster: &Character,
        area_center: Position,
        detail_lines: &mut Vec<String>,
        spell_result: u32,
        ally_area: SpellAllyEffect,
    ) -> Vec<(u32, SpellTargetOutcome)> {
        let mut target_outcomes = vec![];

        for enhancement in enhancements {
            if enhancement.effect.increased_radius_tenths > 0 {
                radius = radius.plusf(enhancement.effect.increased_radius_tenths as f32 * 0.1);
            }
        }

        let degree_of_success = spell_result / 10;
        if degree_of_success > 0 {
            detail_lines.push(format!("Fortune: {}", degree_of_success));
        }

        for other_char in self.characters.iter() {
            if other_char.player_controlled != caster.player_controlled {
                continue;
            }

            if within_range_squared(radius.squared(), area_center, other_char.position.get()) {
                detail_lines.push(other_char.name.to_string());

                let outcome = self.perform_spell_ally_effect(
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

    fn perform_spell_ally_effect(
        &self,
        name: &'static str,
        enhancements: &[SpellEnhancement],
        ally_effect: SpellAllyEffect,
        target: &Character,
        detail_lines: &mut Vec<String>,
        degree_of_success: u32,
    ) -> SpellTargetOutcome {
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

            health_gained = target.health.gain(healing);
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
            if let Some(effect) = enhancement.effect.on_hit {
                let log_line = self.perform_effect_application(effect, target);
                detail_lines.push(format!("{} ({})", log_line, enhancement.name));
            }
        }

        SpellTargetOutcome::HealedAlly(health_gained)
    }

    fn perform_spell_area_enemy_effect(
        &self,
        mut radius: Range,
        name: &'static str,
        modifier: SpellModifier,
        enhancements: &[SpellEnhancement],
        caster: &Character,
        area_center: Position,
        detail_lines: &mut Vec<String>,
        spell_result: u32,
        enemy_area: SpellEnemyEffect,
    ) -> Vec<(u32, SpellTargetOutcome)> {
        let mut target_outcomes = vec![];

        for enhancement in enhancements {
            if enhancement.effect.increased_radius_tenths > 0 {
                radius = radius.plusf(enhancement.effect.increased_radius_tenths as f32 * 0.1);
            }
        }

        for other_char in self.characters.iter() {
            if other_char.player_controlled == caster.player_controlled {
                continue;
            }

            if within_range_squared(radius.squared(), area_center, other_char.position.get()) {
                let mut line = other_char.name.to_string();
                if let Some(contest) = enemy_area.defense_type {
                    match contest {
                        DefenseType::Will => line.push_str(&format!(" will={}", other_char.will())),
                        DefenseType::Evasion => {
                            line.push_str(&format!(" evasion={}", other_char.evasion()))
                        }
                    }
                }

                detail_lines.push(line);

                let outcome = self.perform_spell_enemy_effect(
                    name,
                    modifier,
                    enhancements,
                    enemy_area,
                    spell_result,
                    other_char,
                    detail_lines,
                    false,
                );

                target_outcomes.push((other_char.id(), outcome));
            }
        }

        target_outcomes
    }

    fn perform_spell_enemy_effect(
        &self,
        spell_name: &'static str,
        modifier: SpellModifier,
        enhancements: &[SpellEnhancement],
        enemy_effect: SpellEnemyEffect,
        spell_result: u32,
        target: &Character,
        detail_lines: &mut Vec<String>,
        is_direct_target: bool,
    ) -> SpellTargetOutcome {
        let success = match enemy_effect.defense_type {
            Some(contest) => {
                let defense = match contest {
                    DefenseType::Will => target.will(),
                    DefenseType::Evasion => target.evasion(),
                };

                if spell_result >= defense {
                    Some((spell_result - defense) / 5)
                } else {
                    None
                }
            }
            None => Some(0),
        };

        if let Some(degree_of_success) = success {
            let heavy_hit_label = match degree_of_success {
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

            let damage = if let Some(spell_damage) = enemy_effect.damage {
                let mut dmg_calculation;
                let mut increased_by_good_roll = true;
                let mut dmg_str = "  Damage: ".to_string();

                match spell_damage {
                    SpellDamage::Static(n) => {
                        dmg_calculation = n as i32;
                        increased_by_good_roll = false;

                        dmg_str.push_str(&format!("{} ({})", dmg_calculation, spell_name));
                    }
                    SpellDamage::AtLeast(n) => {
                        dmg_calculation = n as i32;
                        dmg_str.push_str(&format!("{} ({})", dmg_calculation, spell_name));
                    }
                    SpellDamage::Weapon => {
                        let weapon = self.active_character().weapon(HandType::MainHand).unwrap();
                        dmg_calculation = weapon.damage as i32;
                        dmg_str.push_str(&format!("{} ({})", dmg_calculation, weapon.name));

                        if matches!(weapon.grip, WeaponGrip::Versatile)
                            && self.active_character().off_hand.get().is_empty()
                        {
                            let bonus_dmg = 1;
                            dmg_str.push_str(&format!(" +{} (two-handed)", bonus_dmg));
                            dmg_calculation += bonus_dmg;
                        }
                    }
                };

                if increased_by_good_roll && degree_of_success > 0 {
                    dmg_str.push_str(&format!(" +{degree_of_success} ({heavy_hit_label})"));
                    dmg_calculation += degree_of_success as i32;
                }

                for enhancement in enhancements {
                    let bonus_dmg = if is_direct_target {
                        enhancement.effect.bonus_target_damage
                    } else {
                        enhancement.effect.bonus_area_damage
                    };
                    if bonus_dmg > 0 {
                        dmg_str.push_str(&format!(" +{} ({})", bonus_dmg, enhancement.name));
                        dmg_calculation += bonus_dmg as i32;
                    }
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

            for mut effect in enemy_effect
                .on_hit
                .unwrap_or_default()
                .iter()
                .copied()
                .flatten()
            {
                match effect {
                    ApplyEffect::RemoveActionPoints(ref mut n) => *n += degree_of_success,
                    ApplyEffect::GainStamina(ref mut n) => *n += degree_of_success,
                    ApplyEffect::Condition(ref mut condition) => {
                        if let Some(stacks) = condition.stacks() {
                            *stacks += degree_of_success;
                        }
                    }
                }

                let log_line = self.perform_effect_application(effect, target);
                detail_lines.push(log_line);
            }

            for enhancement in enhancements {
                if let Some(effect) = enhancement.effect.on_hit {
                    let log_line = self.perform_effect_application(effect, target);
                    detail_lines.push(format!("{} ({})", log_line, enhancement.name));
                }
            }

            SpellTargetOutcome::HitEnemy { damage }
        } else {
            let line = match (modifier, enemy_effect.defense_type) {
                (_, None) => unreachable!("uncontested effect cannot fail"),
                (SpellModifier::Spell, Some(DefenseType::Will)) => {
                    format!("  {} resisted the spell", target.name)
                }
                (SpellModifier::Spell, Some(DefenseType::Evasion)) => {
                    format!("  The spell missed {}", target.name)
                }
                (SpellModifier::Attack(_), Some(_)) => {
                    format!("  The ability missed {}", target.name)
                }
            };
            detail_lines.push(line);
            SpellTargetOutcome::Resist
        }
    }

    fn perform_losing_health(&self, character: &Character, amount: u32) -> u32 {
        let amount_lost = character.health.lose(amount);

        if character.health.current() as f32 <= character.health.max as f32 * 0.3 {
            character.receive_condition(Condition::NearDeath);
        } else {
            character.conditions.borrow_mut().near_death = false;
        }

        //self.log(format!("  {} took {} damage", character.name, amount));

        if character.health.current() == 0 {
            character.conditions.borrow_mut().near_death = false;
            character.conditions.borrow_mut().dead = true;
        }

        amount_lost
    }

    async fn log(&self, line: impl Into<String>) {
        self.ui_handle_event(GameEvent::LogLine(line.into())).await;
    }

    async fn perform_attack(
        &self,
        attacker_id: CharacterId,
        hand_type: HandType,
        enhancements: Vec<AttackEnhancement>,
        defender_id: CharacterId,
        defender_reaction: Option<OnAttackedReaction>,
    ) -> Option<(CharacterId, u32)> {
        let attacker = self.characters.get(attacker_id);
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
            attacker,
            hand_type,
            defender,
            circumstance_advantage,
            &enhancements,
            defender_reaction,
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
                    defender_reacted_with_parry = true;
                }
                OnAttackedReactionId::SideStep => {
                    defender_reacted_with_sidestep = true;
                }
            }
        }

        let roll = roll_d20_with_advantage(attack_bonus.advantage);
        let attack_result = ((roll + attack_modifier) as i32 + attack_bonus.flat_amount) as u32;

        detail_lines.push(roll_description(attack_bonus.advantage));

        let mut armor_value = defender.protection_from_armor();
        let mut armor_str = armor_value.to_string();
        for enhancement in &enhancements {
            let armor_pentration = enhancement.effect.armor_penetration;
            if armor_pentration > 0 {
                armor_value = armor_value.saturating_sub(armor_pentration);
                armor_str.push_str(&format!(" -{} ({})", armor_pentration, enhancement.name));
            }
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

        let hit = attack_result >= evasion;

        let outcome = if hit {
            let mut on_true_hit_effect = None;
            let weapon = attacker.weapon(hand_type).unwrap();
            let mut dmg_calculation = weapon.damage as i32;

            let mut dmg_str = format!("  Damage: {} ({})", dmg_calculation, weapon.name);

            if matches!(weapon.grip, WeaponGrip::Versatile) && attacker.off_hand.get().is_empty() {
                let bonus_dmg = 1;
                dmg_str.push_str(&format!(" +{} (two-handed)", bonus_dmg));
                dmg_calculation += bonus_dmg;
            }

            let armored_defense = evasion + armor_value;
            if attack_result < armored_defense {
                let mitigated = armored_defense - attack_result;

                detail_lines.push(format!("  Hit! ({} mitigated)", mitigated));
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
                    detail_lines.push("  True hit".to_string());
                }
            }

            for enhancement in &enhancements {
                let bonus_dmg = enhancement.effect.bonus_damage;
                if bonus_dmg > 0 {
                    dmg_str.push_str(&format!(" +{} ({})", bonus_dmg, enhancement.name));
                    dmg_calculation += bonus_dmg as i32;
                }
            }

            let damage = dmg_calculation.max(0) as u32;

            dmg_str.push_str(&format!(" = {damage}"));

            detail_lines.push(dmg_str);

            self.perform_losing_health(defender, damage);

            attack_hit = Some((defender_id, damage));

            if let Some(effect) = on_true_hit_effect {
                match effect {
                    AttackHitEffect::Apply(effect) => {
                        let log_line = self.perform_effect_application(effect, defender);
                        detail_lines.push(format!("{} (true hit)", log_line))
                    }
                    AttackHitEffect::SkipExertion => skip_attack_exertion = true,
                }
            }

            for enhancement in &enhancements {
                if let Some(effect) = enhancement.effect.on_hit_effect {
                    let log_line = match effect {
                        AttackEnhancementOnHitEffect::RegainActionPoint => {
                            attacker.action_points.gain(1);
                            format!("{} regained 1 AP", attacker.name)
                        }
                        AttackEnhancementOnHitEffect::Target(apply_effect) => {
                            self.perform_effect_application(apply_effect, defender)
                        }
                    };

                    detail_lines.push(format!("{} ({})", log_line, enhancement.name))
                }

                if let Some(mut condition) = enhancement.effect.inflict_condition_per_damage {
                    if damage > 0 {
                        *condition.stacks().unwrap() = damage;
                        let line = self.perform_receive_condition(condition, defender);
                        detail_lines.push(format!("{} ({})", line, enhancement.name))
                    }
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

        if defender.lose_braced() {
            detail_lines.push(format!("{} lost Braced", defender.name));
        }
        if defender.lose_distracted() {
            detail_lines.push(format!("{} lost Distracted", defender.name));
        }

        for enhancement in &enhancements {
            if let Some(effect) = enhancement.effect.on_target {
                let log_line = self.perform_effect_application(effect, defender);
                detail_lines.push(format!("{} ({})", log_line, enhancement.name));
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

        self.ui_handle_event(GameEvent::Attacked {
            attacker: attacker_id,
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

        let mut new_ap = MAX_ACTION_POINTS;
        if conditions.borrow().near_death {
            new_ap = new_ap.saturating_sub(2);
        }
        if conditions.borrow().slowed > 0 {
            new_ap = new_ap.saturating_sub(2);
        }
        character.action_points.current.set(new_ap);

        if conditions.borrow().slowed > 0 {
            conditions.borrow_mut().slowed -= 1;
            if conditions.borrow().slowed == 0 {
                self.log(format!("{} is no longer Slowed", name)).await;
            }
        }

        conditions.borrow_mut().mainhand_exertion = 0;
        conditions.borrow_mut().offhand_exertion = 0;
        let stamina_gain = (character.stamina.max as f32 / 3.0).ceil() as u32;
        character.stamina.gain(stamina_gain);
    }
}

fn roll_description(advantage: i32) -> String {
    match advantage.cmp(&0) {
        std::cmp::Ordering::Less => {
            format!("Rolled {} dice with disadvantage...", advantage.abs() + 1)
        }
        std::cmp::Ordering::Equal => "Rolled 1 die...".to_string(),
        std::cmp::Ordering::Greater => format!("Rolled {} dice with advantage...", advantage + 1),
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
    Attacked {
        attacker: CharacterId,
        target: CharacterId,
        outcome: AttackOutcome,
        detail_lines: Vec<String>,
    },
    SpellWasCast {
        caster: CharacterId,
        target_outcome: Option<(CharacterId, SpellTargetOutcome)>,
        area_outcomes: Option<(Position, Vec<(CharacterId, SpellTargetOutcome)>)>,
        spell: Spell,
        detail_lines: Vec<String>,
    },
    CharacterDied {
        character: CharacterId,
        new_active: Option<CharacterId>,
    },
    NewTurn {
        new_active: CharacterId,
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
pub enum SpellTargetOutcome {
    HitEnemy { damage: Option<u32> },
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

pub fn spell_roll_bonus(
    caster: &Character,
    defender: &Character,
    enhancements: &[SpellEnhancement],
) -> DiceRollBonus {
    let mut bonus = caster.outgoing_spell_roll_bonus(enhancements);
    bonus.advantage += defender.incoming_spell_advantage();
    bonus
}

pub fn attack_roll_bonus(
    attacker: &Character,
    hand: HandType,
    defender: &Character,
    circumstance_advantage: i32,
    enhancements: &[AttackEnhancement],
    reaction: Option<OnAttackedReaction>,
) -> DiceRollBonus {
    let mut bonus = attacker.outgoing_attack_roll_bonus(hand, enhancements, defender.pos());
    bonus.advantage += defender.incoming_attack_advantage(reaction);
    bonus.advantage += circumstance_advantage;
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
        reaction,
    );
    let mut evasion = defender.evasion();

    if let Some(reaction) = reaction {
        evasion += reaction.effect.bonus_evasion;
    }

    let dice_target = evasion
        .saturating_sub(attacker.attack_modifier(hand))
        .max(1);
    probability_of_d20_reaching(dice_target, bonus)
}

pub fn prob_spell_hit(
    caster: &Character,
    defense_type: DefenseType,
    defender: &Character,
    enhancements: &[SpellEnhancement],
) -> f32 {
    let bonus = spell_roll_bonus(caster, defender, enhancements);

    let def = match defense_type {
        DefenseType::Will => defender.will(),
        DefenseType::Evasion => defender.evasion(),
    };
    let target = def.saturating_sub(caster.spell_modifier()).max(1);
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
pub struct AttackEnhancement {
    pub name: &'static str,
    pub description: &'static str,
    pub icon: IconId,
    pub action_point_cost: u32,
    pub stamina_cost: u32,
    pub mana_cost: u32,

    pub effect: AttackEnhancementEffect,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub enum ApplyEffect {
    RemoveActionPoints(u32),
    Condition(Condition),
    GainStamina(u32),
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
        }
    }

    pub const fn description(&self) -> &'static str {
        use Condition::*;
        match self {
            Protected(_) => "Gains +3 armor",
            Dazed(_) => "Gains no evasion from agility and attacks with disadvantage",
            Bleeding(_) => "End of turn: 50% stacks decay, 1 damage for each decayed",
            Braced => "Gain +3 evasion against the next incoming attack",
            Raging => "Gains advantage on melee attacks until end of turn",
            Distracted => "-6 evasion against the next incoming attack",
            Weakened(_) => "-x to all defenses and dice rolls",
            MainHandExertion(_) => "-x on further similar actions",
            OffHandExertion(_) => "-x on further similar actions",
            Encumbered(_) => "-x to Evasion and -x/2 to dice rolls",
            NearDeath => "< 30% HP: Reduced AP, disadvantage on everything",
            Dead => "This character has reached 0 HP and is dead",
            Slowed(_) => "Gains 2 less AP per turn",
            Exposed(_) => "-3 to all defenses",
        }
    }

    pub const fn info(&mut self) -> (ConditionInfo, Option<u32>) {
        (
            ConditionInfo {
                name: self.name(),
                description: self.description(),
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

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct ConditionInfo {
    pub name: &'static str,
    pub description: &'static str,
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
    CastSpell {
        spell: Spell,
        enhancements: Vec<SpellEnhancement>,
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
    CastSpell(Spell),
    Move,
    ChangeEquipment,
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
            BaseAction::CastSpell(spell) => matches!(
                spell.weapon_requirement,
                Some(SpellWeaponRequirement::Melee)
            ),
            _ => false,
        }
    }

    pub fn action_point_cost(&self) -> u32 {
        match self {
            BaseAction::Attack(attack) => attack.action_point_cost,
            BaseAction::CastSpell(spell) => spell.action_point_cost,
            BaseAction::Move => 0,
            BaseAction::ChangeEquipment => 1,
            BaseAction::EndTurn => 0,
        }
    }

    pub fn mana_cost(&self) -> u32 {
        match self {
            BaseAction::Attack { .. } => 0,
            BaseAction::CastSpell(spell) => spell.mana_cost,
            BaseAction::Move => 0,
            BaseAction::ChangeEquipment => 0,
            BaseAction::EndTurn => 0,
        }
    }

    pub fn stamina_cost(&self) -> u32 {
        match self {
            BaseAction::Attack { .. } => 0,
            BaseAction::CastSpell(spell) => spell.stamina_cost,
            BaseAction::Move => 0,
            BaseAction::ChangeEquipment => 0,
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
pub struct Spell {
    pub name: &'static str,
    pub description: &'static str,
    pub icon: IconId,
    pub action_point_cost: u32,
    pub mana_cost: u32,
    pub stamina_cost: u32,
    pub weapon_requirement: Option<SpellWeaponRequirement>,

    pub modifier: SpellModifier,
    pub target: SpellTarget,
    pub possible_enhancements: [Option<SpellEnhancement>; 3],
    pub animation_color: Color,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SpellWeaponRequirement {
    Melee,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SpellModifier {
    Spell,
    Attack(i32),
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SpellEffect {
    Enemy(SpellEnemyEffect),
    Ally(SpellAllyEffect),
}

// enemey effects:

// 1. weapon based (attack contest, optional on_hit)
// 2. spell contest (mental or projectile); damage optionally scaled by degree of success, optional on hit
// 3. no contest, damage not scaled

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct SpellEnemyEffect {
    pub defense_type: Option<DefenseType>,
    pub damage: Option<SpellDamage>,
    pub on_hit: Option<[Option<ApplyEffect>; 2]>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SpellDamage {
    Static(u32),
    AtLeast(u32),
    Weapon,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct SpellAllyEffect {
    pub healing: u32,
    pub apply: Option<ApplyEffect>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SpellTarget {
    Enemy {
        reach: SpellReach,
        effect: SpellEnemyEffect,
        impact_area: Option<(Range, SpellEnemyEffect)>,
    },

    Ally {
        range: Range,
        effect: SpellAllyEffect,
    },

    Area {
        range: Range,
        radius: Range,
        effect: SpellEffect,
    },

    None {
        self_area: Option<(Range, SpellEffect)>,
        self_effect: Option<SpellAllyEffect>,
    },
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SpellReach {
    Range(Range),
    MoveIntoMelee(Range),
}

impl SpellTarget {
    pub fn single_target(&self) -> bool {
        match self {
            SpellTarget::Enemy { .. } => true,
            SpellTarget::Ally { .. } => true,
            SpellTarget::Area { .. } => false,
            SpellTarget::None { .. } => false,
        }
    }

    fn base_range(&self) -> Option<Range> {
        match self {
            SpellTarget::Enemy { reach, .. } => match reach {
                SpellReach::Range(range) => Some(*range),
                SpellReach::MoveIntoMelee(range) => Some(*range),
            },
            SpellTarget::Ally { range, .. } => Some(*range),
            SpellTarget::Area { range, .. } => Some(*range),
            SpellTarget::None { self_area, .. } => {
                // TODO This is actually radius, not range; is this misused somewhere (with enahcenements for example)
                self_area.as_ref().map(|(radius, _effect)| *radius)
            }
        }
    }

    pub fn range(&self, enhancements: &[SpellEnhancement]) -> Option<Range> {
        self.base_range().map(|mut range| {
            for enhancement in enhancements {
                if enhancement.effect.increased_range_tenths > 0 {
                    range = range.plusf(enhancement.effect.increased_range_tenths as f32 * 0.1);
                }
            }
            range
        })
    }
}

// TODO Merge SpellEnhancement and AttackEnhancement? (There may be AttackEnhancements that should also be
// usable for attack abilities (like Lunge attack / Sweeping attack))

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct SpellEnhancement {
    pub name: &'static str,
    pub description: &'static str,
    pub icon: IconId,
    pub action_point_cost: u32,
    pub mana_cost: u32,
    pub stamina_cost: u32,

    pub effect: SpellEnhancementEffect,
}

#[derive(Debug, Copy, Clone, PartialEq, Hash)]
pub struct AttackEnhancementEffect {
    pub action_point_discount: u32,
    pub bonus_damage: u32,
    pub bonus_advantage: u32,
    pub on_hit_effect: Option<AttackEnhancementOnHitEffect>,
    pub roll_modifier: i32,
    pub inflict_condition_per_damage: Option<Condition>,
    pub armor_penetration: u32,
    // TODO Actually handle this
    pub on_self: Option<ApplyEffect>,

    pub on_target: Option<ApplyEffect>,
}

impl AttackEnhancementEffect {
    // the impl from #[derive(Default)] is not const
    pub const fn default() -> Self {
        Self {
            action_point_discount: 0,
            bonus_damage: 0,
            bonus_advantage: 0,
            on_hit_effect: None,
            roll_modifier: 0,
            inflict_condition_per_damage: None,
            armor_penetration: 0,
            on_self: None,
            on_target: None,
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
}

#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct Hand {
    weapon: Option<Weapon>,
    shield: Option<Shield>,
}

impl Hand {
    fn empty() -> Self {
        Self::default()
    }

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

#[derive(Debug, Copy, Clone)]
pub struct Attributes {
    pub strength: u32,
    pub agility: u32,
    pub intellect: u32,
    pub spirit: u32,
}

impl Attributes {
    pub fn new(str: u32, agi: u32, intel: u32, spi: u32) -> Self {
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
    pub name: &'static str,
    pub portrait: PortraitId,

    pub sprite: SpriteId,
    pub player_controlled: bool,
    pub position: Cell<Position>,
    pub base_attributes: Attributes,
    pub health: NumberedResource,
    pub mana: NumberedResource,
    // How many cells you can move per AP
    pub move_speed: f32,
    pub capacity: u32,
    pub inventory: [Cell<Option<EquipmentEntry>>; 6],
    pub armor: Cell<Option<ArmorPiece>>,
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

    changed_equipment_listeners: RefCell<Vec<Weak<Cell<bool>>>>,
}

impl Character {
    pub fn new(
        player_controlled: bool,
        name: &'static str,
        portrait: PortraitId,
        sprite: SpriteId,
        base_attributes: Attributes,
        position: Position,
    ) -> Self {
        let max_health = 6 + base_attributes.strength;
        let max_mana = (base_attributes.spirit * 2).saturating_sub(3);

        let move_speed = 0.9 + base_attributes.agility as f32 * 0.1;
        let max_stamina = (base_attributes.strength + base_attributes.agility).saturating_sub(5);
        let max_reactive_action_points = 1 + base_attributes.intellect / 2;
        let capacity = base_attributes.strength * 2;
        Self {
            id: None,
            portrait,
            sprite,
            player_controlled,
            position: Cell::new(position),
            name,
            base_attributes,
            health: NumberedResource::new(max_health),
            mana: NumberedResource::new(max_mana),
            move_speed,
            capacity,
            inventory: Default::default(),
            armor: Default::default(),
            main_hand: Default::default(),
            off_hand: Default::default(),
            conditions: Default::default(),
            action_points: NumberedResource::new(MAX_ACTION_POINTS),
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
                BaseAction::EndTurn,
            ],
            known_attacked_reactions: Default::default(),
            known_on_hit_reactions: Default::default(),
            changed_equipment_listeners: Default::default(),
        }
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
        if let Some(armor) = self.armor.get() {
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
            EquipmentSlotRole::Armor => self.armor.get().map(EquipmentEntry::Armor),
            EquipmentSlotRole::Inventory(idx) => self.inventory[idx].get(),
        }
    }

    fn on_changed_equipment(&self) {
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
                Some(EquipmentEntry::Armor(armor)) => self.armor.set(Some(armor)),
                None => self.armor.set(None),
                _ => panic!(),
            },
            EquipmentSlotRole::Inventory(i) => self.inventory[i].set(entry),
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

    pub fn reaches_with_spell(
        &self,
        spell: Spell,
        enhancements: &[SpellEnhancement],
        target_position: Position,
    ) -> bool {
        let range = spell.target.range(enhancements).unwrap();
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

    pub fn can_use_action(&self, action: BaseAction) -> bool {
        let ap = self.action_points.current();
        match action {
            BaseAction::Attack(attack) => {
                matches!(self.weapon(attack.hand), Some(weapon) if ap >= weapon.action_point_cost)
            }
            BaseAction::CastSpell(spell) => {
                if matches!(
                    spell.weapon_requirement,
                    Some(SpellWeaponRequirement::Melee)
                ) && !self.has_equipped_melee_weapon()
                {
                    return false;
                }
                ap >= spell.action_point_cost
                    && self.stamina.current() >= spell.stamina_cost
                    && self.mana.current() >= spell.mana_cost
            }
            BaseAction::Move => ap > 0,
            BaseAction::ChangeEquipment => ap > 0,
            BaseAction::EndTurn => true,
        }
    }

    pub fn known_attack_enhancements(
        &self,
        attack_hand: HandType,
    ) -> Vec<(String, AttackEnhancement)> {
        let mut usable = vec![];
        if let Some(weapon) = self.weapon(attack_hand) {
            if let Some(enhancement) = weapon.attack_enhancement {
                usable.push((weapon.name.to_string(), enhancement))
            }
            for enhancement in &self.known_attack_enhancements {
                usable.push(("".to_owned(), *enhancement))
            }
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
        self.action_points.current()
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
            && (!reaction.must_be_melee || is_within_melee)
    }

    pub fn can_use_spell_enhancement(&self, spell: Spell, enhancement: SpellEnhancement) -> bool {
        //let enhancement = spell.possible_enhancements[enhancement_index].unwrap();
        self.action_points.current() >= spell.action_point_cost + enhancement.action_point_cost
            && self.mana.current() >= spell.mana_cost + enhancement.mana_cost
            && self.stamina.current() >= spell.stamina_cost + enhancement.stamina_cost
    }

    fn strength(&self) -> u32 {
        (self.base_attributes.strength as i32).max(1) as u32
    }

    fn agility(&self) -> u32 {
        (self.base_attributes.agility as i32).max(1) as u32
    }

    fn intellect(&self) -> u32 {
        (self.base_attributes.intellect as i32).max(1) as u32
    }

    fn spirit(&self) -> u32 {
        (self.base_attributes.spirit as i32).max(1) as u32
    }

    pub fn spell_modifier(&self) -> u32 {
        let mut res = self.intellect() + self.spirit();

        if let Some(armor) = self.armor.get() {
            res += armor.equip.bonus_spell_modifier;
        }

        res
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
        if let Some(armor) = self.armor.get() {
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
        if let Some(armor) = self.armor.get() {
            protection += armor.protection;
        }

        if self.conditions.borrow().protected > 0 {
            protection += PROTECTED_ARMOR_BONUS;
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

    fn outgoing_spell_roll_bonus(&self, enhancements: &[SpellEnhancement]) -> DiceRollBonus {
        let mut advantage = 0i32;
        let mut flat_amount = 0;
        for (_label, bonus) in self.outgoing_spell_bonuses(enhancements) {
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
        enhancements: &[AttackEnhancement],
        target_pos: Position,
    ) -> DiceRollBonus {
        let mut advantage = 0i32;
        let mut flat_amount = 0;
        for (_label, bonus) in self.outgoing_attack_bonuses(hand_type, enhancements, target_pos) {
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
        enhancements: &[AttackEnhancement],
        target_pos: Position,
    ) -> Vec<(&'static str, RollBonusContributor)> {
        let mut bonuses = vec![];

        let (_range, reach) = self.reaches_with_attack(hand_type, target_pos);

        if let ActionReach::YesButDisadvantage(reason) = reach {
            bonuses.push((reason, RollBonusContributor::Advantage(-1)));
        }

        for enhancement in enhancements {
            dbg!(enhancement);
            if enhancement.effect.bonus_advantage > 0 {
                bonuses.push((
                    enhancement.name,
                    RollBonusContributor::Advantage(enhancement.effect.bonus_advantage as i32),
                ));
            }

            if enhancement.effect.roll_modifier != 0 {
                dbg!(enhancement); // TODO
                bonuses.push((
                    enhancement.name,
                    RollBonusContributor::FlatAmount(enhancement.effect.roll_modifier),
                ));
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

    pub fn outgoing_spell_bonuses(
        &self,
        enhancements: &[SpellEnhancement],
    ) -> Vec<(&'static str, RollBonusContributor)> {
        let mut bonuses = vec![];
        for enhancement in enhancements {
            if enhancement.effect.bonus_advantage > 0 {
                bonuses.push((
                    enhancement.name,
                    RollBonusContributor::Advantage(enhancement.effect.bonus_advantage as i32),
                ));
            }
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
        for (_label, bonus) in self.incoming_attack_bonuses(reaction) {
            match bonus {
                RollBonusContributor::Advantage(n) => advantage += n,
                RollBonusContributor::OtherNegative | RollBonusContributor::OtherPositive => {}
                RollBonusContributor::FlatAmount(_) => unreachable!(),
            }
        }
        advantage
    }

    fn incoming_spell_advantage(&self) -> i32 {
        let mut advantage = 0;
        for (_label, bonus) in self.incoming_spell_bonuses() {
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

    pub fn incoming_spell_bonuses(&self) -> Vec<(&'static str, RollBonusContributor)> {
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
            Slowed(n) => conditions.slowed += n,
            Exposed(n) => conditions.exposed += n,
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

    fn lose(&self, amount: u32) -> u32 {
        let prev = self.current.get();
        let new = self.current.get().saturating_sub(amount);
        self.current.set(new);
        prev - new
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
    pub on_true_hit: Option<AttackHitEffect>,
    pub weight: u32,
}

impl Weapon {
    pub fn is_melee(&self) -> bool {
        matches!(self.range, WeaponRange::Melee)
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
}

impl EquipmentEntry {
    pub fn name(&self) -> &'static str {
        match self {
            EquipmentEntry::Weapon(weapon) => weapon.name,
            EquipmentEntry::Shield(shield) => shield.name,
            EquipmentEntry::Armor(armor) => armor.name,
        }
    }

    pub fn icon(&self) -> EquipmentIconId {
        match self {
            EquipmentEntry::Weapon(weapon) => weapon.icon,
            EquipmentEntry::Shield(_shield) => EquipmentIconId::SmallShield,
            EquipmentEntry::Armor(armor) => armor.icon,
        }
    }

    fn weight(&self) -> u32 {
        match self {
            EquipmentEntry::Weapon(weapon) => weapon.weight,
            EquipmentEntry::Shield(shield) => shield.weight,
            EquipmentEntry::Armor(armor) => armor.weight,
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
