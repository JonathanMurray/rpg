use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use indexmap::IndexMap;
use macroquad::{
    color::{BLACK, DARKGRAY, GRAY, GREEN, LIGHTGRAY, ORANGE, RED, WHITE, YELLOW},
    input::{is_key_down, KeyCode},
    math::Rect,
    shapes::{draw_line, draw_rectangle, draw_rectangle_lines},
    text::{measure_text, Font, TextParams},
    texture::Texture2D,
};

use crate::{
    action_button::{
        draw_button_tooltip, ButtonAction, ButtonHovered, ButtonSelected, EventSender,
        InternalUiEvent,
    },
    base_ui::{
        draw_text_rounded, draw_text_with_font_icons, measure_text_with_font_icons, Drawable,
    },
    core::{predict_attack, Character, CharacterId, Characters, MOVE_DISTANCE_PER_STAMINA},
    drawing::{draw_cross, draw_dashed_line},
    game_ui::{ConfiguredAction, UiState},
    textures::IconId,
};

use crate::action_button::ActionButton;

pub struct ActivityPopup {
    characters: Characters,
    relevant_character_id: CharacterId,
    icons: HashMap<IconId, Texture2D>,

    ui_state: Rc<RefCell<UiState>>,

    font: Font,

    base_lines: Vec<String>,
    pub additional_line: Option<String>,

    next_button_id: Cell<u32>,
    choice_buttons: IndexMap<u32, ActionButton>,
    proceed_button: ActionButton,
    proceed_button_error: Option<String>,

    movement_cost_slider: Option<MovementStaminaSlider>,

    proceed_button_events: Rc<RefCell<Vec<InternalUiEvent>>>,
    choice_button_events: Rc<RefCell<Vec<InternalUiEvent>>>,
    selected_choice_button_ids: Vec<u32>,
    hovered_choice_button_id: Option<u32>,

    pub last_drawn_rectangle: Rect,
}

impl ActivityPopup {
    pub fn new(
        font: Font,
        state: Rc<RefCell<UiState>>,
        icons: HashMap<IconId, Texture2D>,
        characters: Characters,
        active_character_id: CharacterId,
    ) -> Self {
        let proceed_button_events = Rc::new(RefCell::new(vec![]));

        let mut next_button_id = 0;
        let proceed_button = ActionButton::new(
            ButtonAction::Proceed,
            Some(Rc::clone(&proceed_button_events)),
            next_button_id,
            &icons,
            None,
            &font,
        );
        next_button_id += 1;

        Self {
            characters,
            relevant_character_id: active_character_id,
            icons,
            ui_state: state,
            font,
            base_lines: vec![],
            additional_line: None,
            selected_choice_button_ids: Default::default(),
            choice_buttons: Default::default(),
            proceed_button,
            proceed_button_error: None,
            next_button_id: Cell::new(next_button_id),
            proceed_button_events,
            choice_button_events: Rc::new(RefCell::new(vec![])),
            movement_cost_slider: None,
            hovered_choice_button_id: None,
            last_drawn_rectangle: Default::default(),
        }
    }

    pub fn draw(&mut self, x: f32, y: f32) {
        if matches!(
            &*self.ui_state.borrow(),
            UiState::Idle | UiState::ChoosingAction
        ) {
            self.last_drawn_rectangle = Rect {
                x,
                y,
                w: 0.0,
                h: 0.0,
            };
            return;
        }

        let bg_color = BLACK;

        let top_pad = 10.0;

        let base_text_params = TextParams {
            font: Some(&self.font),
            font_size: 16,
            color: WHITE,
            ..Default::default()
        };
        let header_params = TextParams {
            font: Some(&self.font),
            font_size: 22,
            color: BLACK,
            ..Default::default()
        };

        let mut measured_lines = vec![];

        let header_dimensions = measure_text(
            &self.base_lines[0],
            header_params.font,
            header_params.font_size,
            1.0,
        );
        measured_lines.push((&self.base_lines[0], header_dimensions));

        for line in self.base_lines.iter().skip(1) {
            let dimensions =
                measure_text(line, base_text_params.font, base_text_params.font_size, 1.0);
            measured_lines.push((line, dimensions));
        }

        if let Some(line) = &self.additional_line {
            let dimensions =
                measure_text(line, base_text_params.font, base_text_params.font_size, 1.0);
            measured_lines.push((line, dimensions));
        }

        let empty_line_h = 12.0;
        let line_h = 22.0;

        let line_margin = 8.0;
        let mut text_content_h = top_pad + header_dimensions.offset_y;
        let mut text_content_w = 0.0;
        for (line, dim) in &measured_lines {
            //text_content_h += dim.height;
            if line.is_empty() {
                text_content_h += empty_line_h;
            } else {
                text_content_h += line_h;
            }
            //text_content_h += dim.offset_y;
            if dim.width > text_content_w {
                text_content_w = dim.width;
            }
        }
        //text_content_h += (measured_lines.len() - 1) as f32 * line_margin;

        let height = (text_content_h + 10.0).max(74.0);

        let draw_proceed_button = !matches!(
            &*self.ui_state.borrow(),
            UiState::ConfiguringAction(ConfiguredAction::Move { .. })
        );

        let hor_pad = 10.0;
        let margin_between_text_and_buttons = 20.0;
        let button_margin = 10.0;
        let margin_between_choices_and_proceed = 15.0;

        let mut width = text_content_w + margin_between_text_and_buttons;

        if draw_proceed_button {
            width += self.proceed_button.size.0;
        }

        for btn in self.choice_buttons.values() {
            width += button_margin + btn.size.0;
        }
        if !self.choice_buttons.is_empty() && draw_proceed_button {
            width += margin_between_choices_and_proceed;
        }

        let sprint_stamina_text = "Stamina cost:";
        let sprint_stamina_margin = 15.0;
        if let Some(slider) = &self.movement_cost_slider {
            let text_dimensions = measure_text(
                sprint_stamina_text,
                base_text_params.font,
                base_text_params.font_size,
                1.0,
            );
            let move_config_w = slider.size().0.max(text_dimensions.width);
            width += move_config_w + sprint_stamina_margin;
        }

        width += hor_pad * 2.0;

        draw_rectangle(x, y - height, width, height, bg_color);

        let upper_border_color = ORANGE;
        draw_line(x, y, x, y - height, 1.0, upper_border_color);
        draw_line(x + width, y, x + width, y - height, 1.0, upper_border_color);
        draw_line(
            x,
            y - height,
            x + width,
            y - height,
            1.0,
            upper_border_color,
        );

        draw_dashed_line((x, y), (x + width, y), 1.0, GRAY, 5.0, None, false);

        self.last_drawn_rectangle = Rect {
            x,
            y: y - height,
            w: width,
            h: height,
        };

        let x0 = x + hor_pad;
        let mut y0 = y - height + top_pad + header_dimensions.offset_y;

        for (i, (line, dim)) in measured_lines.iter().enumerate() {
            if i == 0 {
                let mut params = header_params.clone();
                draw_text_rounded(line, x0 + 2.0, y0 + 2.0, params.clone());
                params.color = YELLOW;
                draw_text_rounded(line, x0, y0, params.clone());
            } else {
                //draw_text_rounded(line, x0, y0, base_text_params.clone());
                draw_text_with_font_icons(line, x0, y0, base_text_params.clone());
            }

            y0 += 22.0;
        }

        let mut x_btn = x0 + text_content_w + margin_between_text_and_buttons;

        if let UiState::ConfiguringAction(ConfiguredAction::Move { .. }) = &*self.ui_state.borrow()
        {
            let cost = self.movement_cost_slider.as_ref().unwrap().selected();
            let character = self.characters.get(self.relevant_character_id);
            let movement =
                character.remaining_movement.get() + (cost * MOVE_DISTANCE_PER_STAMINA) as f32;
            //let dim = draw_text_rounded(&format!("Move:"), x0, y0, base_text_params.clone());
            //y0 += dim.offset_y + line_margin;
            /*
            draw_text_rounded(
                &format!("{:.1}", movement),
                x0 + 3.0,
                y0,
                base_text_params.clone(),
            );
             */
        }

        if let Some(slider) = &mut self.movement_cost_slider {
            let text_dimensions = draw_text_rounded(
                sprint_stamina_text,
                x_btn,
                y - height + 20.0,
                base_text_params.clone(),
            );
            slider.draw(x_btn, y - slider.size().1 - 5.0);
            let movement_config_w = slider.size().0.max(text_dimensions.width);
            x_btn += movement_config_w + sprint_stamina_margin;
        }

        let y_btn = y - height / 2.0 - 32.0;

        let first_btn_x = x_btn;

        for btn in self.choice_buttons.values() {
            btn.draw(x_btn, y_btn);
            x_btn += btn.size.0 + button_margin;
        }

        if draw_proceed_button {
            if !self.choice_buttons.is_empty() {
                x_btn += margin_between_choices_and_proceed;
            }

            if let Some(error) = &self.proceed_button_error {
                let font_size = 22;
                let text_dim = measure_text_with_font_icons(&error, Some(&self.font), font_size);
                draw_text_with_font_icons(
                    &error,
                    x + width - text_dim.width - 10.0,
                    y - height + top_pad + 15.0,
                    TextParams {
                        font: Some(&self.font),
                        font_size: font_size,
                        color: RED,
                        ..Default::default()
                    },
                );
            } else {
                self.proceed_button.draw(x_btn, y_btn + 6.0);
            }
        }

        x_btn = first_btn_x; // step back to render tooltips in the right positions
        for btn in self.choice_buttons.values() {
            if self.hovered_choice_button_id == Some(btn.id) {
                let detailed_tooltip = is_key_down(KeyCode::LeftAlt);
                draw_button_tooltip(&self.font, (x_btn, y_btn), &btn.tooltip(), detailed_tooltip);
            }

            x_btn += btn.size.0 + button_margin;
        }
    }

    fn are_choice_buttons_mutually_exclusive(&self) -> bool {
        let state: &UiState = &self.ui_state.borrow();
        matches!(
            state,
            UiState::ReactingToAttack { .. }
                | UiState::ReactingToHit { .. }
                | UiState::ConfiguringAction(ConfiguredAction::Move { .. })
        )
    }

    pub fn update(&mut self) -> Option<ActivityPopupOutcome> {
        let mut changed_on_attacked_reaction = false;
        let mut changed_ability_enhancements = false;
        let mut changed_attack_enhancements = false;
        for event in self.choice_button_events.borrow_mut().drain(..) {
            match event {
                InternalUiEvent::ButtonHovered(ButtonHovered {
                    id, hovered_pos, ..
                }) => {
                    if hovered_pos.is_some() {
                        self.hovered_choice_button_id = Some(id);
                    } else if self.hovered_choice_button_id == Some(id) {
                        self.hovered_choice_button_id = None;
                    }
                }

                InternalUiEvent::ButtonClicked { id, .. } => {
                    let clicked_btn = &self.choice_buttons[&id];
                    clicked_btn.toggle_selected();

                    // Some choices work like radio boxes
                    if self.are_choice_buttons_mutually_exclusive() {
                        for btn in self.choice_buttons.values() {
                            if btn.id != id {
                                btn.deselect();
                            }
                        }
                    }

                    let selected_button_actions: Vec<ButtonAction> = self
                        .choice_buttons
                        .values()
                        .filter(|btn| btn.selected.get() == ButtonSelected::Yes)
                        .map(|btn| btn.action)
                        .collect();

                    match &mut *self.ui_state.borrow_mut() {
                        UiState::ConfiguringAction(configured_action) => match configured_action {
                            ConfiguredAction::Attack {
                                selected_enhancements,
                                ..
                            } => {
                                *selected_enhancements = selected_button_actions
                                    .iter()
                                    .map(|action| action.unwrap_attack_enhancement())
                                    .collect();
                                changed_attack_enhancements = true;
                            }
                            ConfiguredAction::UseAbility {
                                selected_enhancements,
                                ..
                            } => {
                                *selected_enhancements = selected_button_actions
                                    .iter()
                                    .map(|action| action.unwrap_ability_enhancement())
                                    .collect();

                                changed_ability_enhancements = true;
                            }
                            _ => unreachable!(),
                        },
                        UiState::ReactingToAttack { selected, .. } => {
                            *selected = selected_button_actions
                                .first()
                                .map(|action| action.unwrap_on_attacked_reaction());
                            changed_on_attacked_reaction = true;
                        }
                        UiState::ReactingToHit { selected, .. } => {
                            *selected = selected_button_actions
                                .first()
                                .map(|action| action.unwrap_on_hit_reaction());
                        }
                        UiState::ReactingToMovementAttackOpportunity { selected, .. } => {
                            // It's a binary choice of 'use opportunity attack or not'
                            *selected = !selected_button_actions.is_empty();
                        }
                        UiState::ReactingToRangedAttackOpportunity { selected, .. } => {
                            // It's a binary choice of 'use opportunity attack or not'
                            *selected = !selected_button_actions.is_empty();
                        }
                        UiState::ChoosingAction | UiState::Idle => unreachable!(),
                    }

                    self.selected_choice_button_ids.clear();
                    for btn in self.choice_buttons.values() {
                        if btn.selected.get() == ButtonSelected::Yes {
                            self.selected_choice_button_ids.push(btn.id);
                        }
                    }
                }

                InternalUiEvent::ButtonInvalidClicked { .. } => {}
            };
        }

        self.refresh_enabled_state();

        for event in self.proceed_button_events.borrow_mut().drain(..) {
            if matches!(event, InternalUiEvent::ButtonClicked { .. }) {
                return Some(ActivityPopupOutcome::ClickedProceed);
            }
        }

        if changed_on_attacked_reaction {
            self.refresh_on_attacked_state();
            return Some(ActivityPopupOutcome::ChangedReaction);
        }
        if changed_ability_enhancements {
            return Some(ActivityPopupOutcome::ChangedAbilityEnhancements);
        }
        if changed_attack_enhancements {
            return Some(ActivityPopupOutcome::ChangedAttackEnhancements);
        }
        if let Some(slider) = &self.movement_cost_slider {
            if slider.has_changed.take() {
                return Some(ActivityPopupOutcome::ChangedMovementSprint(
                    slider.selected_i,
                ));
            }
        }

        None
    }

    fn selected_choices(&self) -> impl Iterator<Item = &ButtonAction> {
        self.selected_choice_button_ids
            .iter()
            .map(|id| &self.choice_buttons[id].action)
    }

    // TODO get rid of this?
    pub fn on_new_movement_ap_cost(&mut self) {
        let UiState::ConfiguringAction(ConfiguredAction::Move { cost, .. }) =
            *self.ui_state.borrow()
        else {
            panic!()
        };

        if let Some(slider) = self.movement_cost_slider.as_mut() {
            let character = self.characters.get(self.relevant_character_id);
            let max_cost = character.stamina.current();
            slider.set_max_allowed(max_cost);

            assert!(cost <= max_cost);

            slider.selected_i = cost;
        }
    }

    pub fn set_movement_cost(&mut self, cost: u32) {
        // TODO: bug: this unwrap panicked, when clicking on an enemy on the grid?
        let slider = self.movement_cost_slider.as_mut().unwrap();
        let character = self.characters.get(self.relevant_character_id);
        let max_cost = character.stamina.current();
        slider.set_max_allowed(max_cost);

        assert!(cost <= max_cost);

        slider.selected_i = cost;
    }

    fn movement_cost(&self) -> u32 {
        self.movement_cost_slider
            .as_ref()
            .map(|slider| slider.selected())
            .unwrap_or(0)
    }

    pub fn reserved_and_hovered_action_points(&self) -> (i32, i32) {
        /*
        let movement_cost = self.movement_cost();
        if movement_cost > 0 {
            return (movement_cost as i32, 0);
        }
         */

        let borrowed_state = self.ui_state.borrow();
        let base_action = match &*borrowed_state {
            UiState::ConfiguringAction(configured_action) => Some(configured_action),
            _ => None,
        };

        let reserved_from_action = base_action
            .as_ref()
            .map(|action| action.base_action_point_cost())
            .unwrap_or(0);
        let mut reserved_from_choices: i32 = 0;
        for action in self.selected_choices() {
            reserved_from_choices += action.action_point_cost();
            reserved_from_choices -= action.action_point_discount() as i32;
        }
        let mut additional_hovered_from_choices = 0;
        if let Some(id) = self.hovered_choice_button_id {
            if !self.selected_choice_button_ids.contains(&id) {
                additional_hovered_from_choices +=
                    self.choice_buttons[&id].action.action_point_cost();

                if self.are_choice_buttons_mutually_exclusive() {
                    reserved_from_choices = 0;
                }
            }
        }
        let reserved_ap = reserved_from_action + reserved_from_choices;
        let hovered_ap = if additional_hovered_from_choices > 0 {
            reserved_ap + additional_hovered_from_choices
        } else {
            0
        };
        (reserved_ap, hovered_ap)
    }

    pub fn mana_points(&self) -> u32 {
        let borrowed_state = self.ui_state.borrow();
        let base_action = match &*borrowed_state {
            UiState::ConfiguringAction(configured_action) => Some(configured_action),
            _ => None,
        };

        let mut mana = base_action
            .as_ref()
            .map(|action| action.mana_cost())
            .unwrap_or(0);
        for action in self.selected_choices() {
            mana += action.mana_cost();
        }
        if let Some(id) = self.hovered_choice_button_id {
            if !self.selected_choice_button_ids.contains(&id) {
                mana += self.choice_buttons[&id].action.mana_cost()
            }
        }
        mana
    }

    pub fn stamina_points(&self) -> u32 {
        if self.movement_cost() > 0 {
            return self.movement_cost();
        }

        let borrowed_state = self.ui_state.borrow();
        let base_action = match &*borrowed_state {
            UiState::ConfiguringAction(configured_action) => Some(configured_action),
            _ => None,
        };

        let mut sta = base_action
            .as_ref()
            .map(|action| action.stamina_cost())
            .unwrap_or(0);
        for action in self.selected_choices() {
            sta += action.stamina_cost();
        }
        if let Some(id) = self.hovered_choice_button_id {
            if !self.selected_choice_button_ids.contains(&id) {
                sta += self.choice_buttons[&id].action.stamina_cost();
            }
        }
        sta
    }

    pub fn refresh_on_attacked_state(&mut self) {
        let UiState::ReactingToAttack {
            hand,
            attacker,
            defender,
            reactor,
            is_within_melee: _,
            selected,
        } = &*self.ui_state.borrow()
        else {
            unreachable!()
        };

        let reaction = *selected;

        let attacker = self.characters.get_rc(*attacker);
        let defender = self.characters.get(*defender);
        let reactor = self.characters.get(*reactor);

        let attack_enhancements = &[];

        let mut explanation = String::new();

        for (term, _bonus) in attacker.outgoing_attack_bonuses(*hand, attack_enhancements, defender)
        {
            explanation.push_str(term);
            explanation.push(' ');
        }
        for (term, _bonus) in defender.incoming_attack_bonuses(reaction) {
            explanation.push_str(term);
            explanation.push(' ');
        }

        let prediction = predict_attack(
            &self.characters,
            attacker,
            *hand,
            attack_enhancements,
            defender,
            reaction.map(|r| (reactor.id(), r)),
            0,
        );

        let mut line = format!(
            "Damage chance: {}%, {} - {}",
            prediction.percentage_chance_deal_damage, prediction.min_damage, prediction.max_damage
        );
        if !explanation.is_empty() {
            line.push_str(&format!("  ({explanation})"));
        }
        self.additional_line = Some(line);
    }

    fn new_button(&self, btn_action: ButtonAction) -> ActionButton {
        let btn = ActionButton::new(
            btn_action,
            Some(Rc::clone(&self.choice_button_events)),
            self.next_button_id.get(),
            &self.icons,
            None,
            &self.font,
        );
        self.next_button_id.set(self.next_button_id.get() + 1);
        btn
    }

    fn new_button_with_character_dependency(
        &self,
        btn_action: ButtonAction,
        character: Rc<Character>,
        font: &Font,
    ) -> ActionButton {
        let btn = ActionButton::new(
            btn_action,
            Some(Rc::clone(&self.choice_button_events)),
            self.next_button_id.get(),
            &self.icons,
            Some(character),
            font,
        );
        self.next_button_id.set(self.next_button_id.get() + 1);
        btn
    }

    pub fn on_new_state(
        &mut self,
        active_character_id: CharacterId,
        relevant_action_button: Option<Rc<ActionButton>>,
    ) {
        self.relevant_character_id = active_character_id;

        let mut lines = vec![];
        let mut popup_buttons = vec![];

        let mut stamina_slider = None;
        self.selected_choice_button_ids.clear();

        println!("on_new_state");
        //dbg!(self.ui_state.borrow());

        match &mut *self.ui_state.borrow_mut() {
            UiState::ConfiguringAction(configured_action) => {
                let tooltip = relevant_action_button.as_ref().unwrap().tooltip();
                lines.push(tooltip.header.to_string());
                lines.push("".to_string());
                lines.extend_from_slice(&tooltip.technical_description);

                match configured_action {
                    ConfiguredAction::Attack {
                        attack,
                        selected_enhancements,
                        ..
                    } => {
                        let known_attack_enhancements = self
                            .characters
                            .get(active_character_id)
                            .known_attack_enhancements(attack.hand);

                        for (_label, enhancement) in known_attack_enhancements {
                            let btn = self.new_button(ButtonAction::AttackEnhancement(enhancement));
                            if selected_enhancements.contains(&enhancement) {
                                self.selected_choice_button_ids.push(btn.id);
                                btn.selected.set(ButtonSelected::Yes);
                            }
                            popup_buttons.push(btn);
                        }
                    }

                    ConfiguredAction::UseAbility { ability, .. } => {
                        for enhancement in ability.possible_enhancements.iter().flatten().copied() {
                            let character = self.characters.get(active_character_id);
                            if character.knows_ability_enhancement(enhancement)
                                && character.can_use_ability_enhancement(*ability, enhancement)
                            {
                                let btn =
                                    self.new_button(ButtonAction::AbilityEnhancement(enhancement));
                                popup_buttons.push(btn);
                            }
                        }
                    }

                    ConfiguredAction::Move { .. } => {
                        let active_char = self.characters.get(active_character_id);
                        //let speed = active_char.move_speed();
                        //lines.push(format!("Speed: {:.1}", speed));
                        let stamina = &active_char.stamina;
                        if stamina.max() > 0 {
                            let max_stamina_spend = stamina.current();
                            stamina_slider = Some(MovementStaminaSlider::new(max_stamina_spend));
                        }
                    }

                    ConfiguredAction::ChangeEquipment { .. } => {}
                    ConfiguredAction::UseConsumable { .. } => {}
                }
            }

            UiState::ReactingToAttack {
                hand,
                attacker: attacker_id,
                defender: defender_id,
                reactor: reactor_id,
                is_within_melee,
                ..
            } => {
                self.relevant_character_id = *reactor_id;
                let attacker = self.characters.get_rc(*attacker_id);
                let defender = self.characters.get(*defender_id);
                lines.push("React (on attacked)".to_string());
                let attacks_str = format!(
                    "{} attacks {} (d20+{} vs {})",
                    attacker.name,
                    defender.name,
                    attacker.attack_modifier(*hand),
                    defender.evasion(),
                );
                lines.push(attacks_str);

                let reactor = self.characters.get(*reactor_id);

                for reaction in reactor
                    .usable_on_attacked_reactions(*is_within_melee, defender_id == reactor_id)
                {
                    let btn_action = ButtonAction::OnAttackedReaction(reaction);
                    let btn = self.new_button(btn_action);
                    popup_buttons.push(btn);
                }
            }

            UiState::ReactingToHit {
                attacker: attacker_id,
                damage,
                victim: victim_id,
                is_within_melee,
                ..
            } => {
                self.relevant_character_id = *victim_id;
                let victim = self.characters.get(*victim_id);
                lines.push("React (on hit)".to_string());
                lines.push(format!(
                    "{} attacked {} for {} damage",
                    self.characters.get(*attacker_id).name,
                    victim.name,
                    damage,
                ));

                let victim = self.characters.get(*victim_id);
                for (_subtext, reaction) in victim.usable_on_hit_reactions(*is_within_melee) {
                    let btn_action = ButtonAction::OnHitReaction(reaction);
                    let btn = self.new_button(btn_action);
                    popup_buttons.push(btn);
                }
            }

            UiState::ReactingToMovementAttackOpportunity { reactor, .. } => {
                self.relevant_character_id = *reactor;
                lines.push("React (opportunity attack)".to_string());
                lines.push(format!(
                    "{} has an attack opportunity",
                    self.characters.get(*reactor).name
                ));

                let btn = self.new_button_with_character_dependency(
                    ButtonAction::OpportunityAttack,
                    self.characters.get_rc(*reactor).clone(),
                    &self.font,
                );
                popup_buttons.push(btn);
            }

            UiState::ReactingToRangedAttackOpportunity { reactor, .. } => {
                self.relevant_character_id = *reactor;
                lines.push("React (opportunity attack)".to_string());
                lines.push(format!(
                    "{} has an attack opportunity",
                    self.characters.get(*reactor).name
                ));

                let btn = self.new_button_with_character_dependency(
                    ButtonAction::OpportunityAttack,
                    self.characters.get_rc(*reactor).clone(),
                    &self.font,
                );
                popup_buttons.push(btn);
            }

            UiState::ChoosingAction | UiState::Idle => {}
        }

        let mut choice_buttons = IndexMap::new();
        for mut btn in popup_buttons {
            btn.event_sender = Some(EventSender {
                queue: Rc::clone(&self.choice_button_events),
            });
            choice_buttons.insert(btn.id, btn);
        }

        self.refresh_enabled_state();

        self.movement_cost_slider = stamina_slider;

        self.base_lines = lines;
        self.choice_buttons = choice_buttons;
    }

    fn refresh_enabled_state(&mut self) {
        let char = self.characters.get(self.relevant_character_id);
        let enough_ap =
            char.action_points.current() as i32 >= self.reserved_and_hovered_action_points().0;
        let enough_mana = char.mana.current() >= self.mana_points();
        let enough_stamina = char.stamina.current() >= self.stamina_points();

        let usability_problem = self.ui_state.borrow().action_usability_problem(
            self.characters.get(self.relevant_character_id),
            &self.characters,
        );

        let mut enabled = false;
        let mut error = None;
        if !enough_ap {
            error = Some("Not enough AP".to_string());
        } else if !enough_mana {
            error = Some("Not enough mana".to_string());
        } else if !enough_stamina {
            error = Some("Not enough stamina".to_string());
        } else if let Some(e) = usability_problem {
            error = Some(e.to_string());
        } else {
            enabled = true;
        }

        self.proceed_button_error = error.map(|e| (format!("|<warning>| {e}")));

        self.proceed_button.enabled.set(enabled);
    }
}

pub enum ActivityPopupOutcome {
    ClickedProceed,
    ChangedAbilityEnhancements,
    ChangedAttackEnhancements,
    ChangedMovementSprint(u32),
    ChangedReaction,
}

struct MovementStaminaSlider {
    max: u32,
    max_allowed: u32,
    selected_i: u32,
    //is_sliding: bool,
    cell_w: f32,
    cell_h: f32,
    has_changed: Cell<bool>,
}

impl MovementStaminaSlider {
    fn new(max: u32) -> Self {
        Self {
            max,
            max_allowed: 0,
            selected_i: 0,
            //is_sliding: false,
            cell_w: 35.0,
            cell_h: 28.0,
            has_changed: Cell::new(false),
        }
    }

    fn size(&self) -> (f32, f32) {
        ((self.max + 1) as f32 * self.cell_w, self.cell_w)
    }

    fn set_max_allowed(&mut self, mut max_allowed: u32) {
        max_allowed = max_allowed.min(self.max);
        self.max_allowed = max_allowed;
        self.selected_i = self.selected_i.min(max_allowed);
    }

    fn selected(&self) -> u32 {
        self.selected_i
    }

    fn draw(&mut self, x: f32, y: f32) {
        let (w, h) = (self.cell_w, self.cell_h);

        let pad = 2.0;
        for i in 0..self.selected_i + 1 {
            let x0 = x + w * i as f32;
            let color = if i == 0 { DARKGRAY } else { GREEN };
            draw_rectangle(x0 + pad, y + pad, w - pad * 2.0, h - pad * 2.0, color);
        }
        for i in 0..self.max_allowed + 1 {
            let x0 = x + w * i as f32;
            draw_rectangle_lines(x0, y, w, h, 1.0, LIGHTGRAY);
        }
        for i in self.max_allowed + 1..self.max + 1 {
            let x0 = x + w * i as f32;
            draw_rectangle_lines(x0, y, w, h, 1.0, DARKGRAY);
        }

        let x0 = x + w * self.selected_i as f32;
        let margin = 1.0;
        draw_rectangle_lines(
            x0 - margin,
            y - margin,
            w + margin * 2.0,
            h + margin * 2.0,
            2.0,
            YELLOW,
        );

        /*
        let (mouse_x, mouse_y) = mouse_position();
        if !is_mouse_button_down(MouseButton::Left) {
            self.is_sliding = false;
        }

        if (y..y + h).contains(&mouse_y) && (x..x + w * (self.max + 1) as f32).contains(&mouse_x) {
            let i = ((mouse_x - x) / w) as u32;

            let x0 = x + w * i as f32;

            if i < self.max_allowed + 1 {
                draw_rectangle_lines(x0, y, w, h, 2.0, WHITE);
            } else {
                draw_rectangle_lines(x0, y, w, h, 1.0, RED);
            }

            if is_mouse_button_pressed(MouseButton::Left) {
                self.is_sliding = true;
            }

            if self.is_sliding {
                let new_value = (i).min(self.max_allowed);
                self.has_changed.set(self.selected_i != new_value);
                self.selected_i = new_value;
            }
        }
         */

        draw_cross(x, y + h / 2.0 - w / 2.0, w, w, LIGHTGRAY, 2.0, 10.0);
    }
}
