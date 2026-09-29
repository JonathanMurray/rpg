use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use indexmap::IndexMap;
use macroquad::{
    color::{Color, BLACK, DARKGRAY, GOLD, GRAY, LIGHTGRAY, MAGENTA, ORANGE, RED, WHITE, YELLOW},
    input::{is_key_down, KeyCode},
    math::Rect,
    shapes::{draw_line, draw_rectangle},
    text::{draw_text, measure_text, Font, TextParams},
};

use crate::{
    action_button::{
        draw_button_tooltip, ButtonAction, ButtonHovered, ButtonSelected, EventSender,
        InternalUiEvent, REGULAR_ACTION_BUTTON_SIZE,
    },
    base_ui::{draw_text_rounded, draw_text_with_font_tags, measure_text_with_font_tags, Drawable},
    core::{Character, CharacterId, Characters, HandType},
    drawing::{draw_dashed_line, draw_rounded_rectangle_lines},
    game_ui::{draw_rectangle_lines2, ConfiguredAction, UiState, UsabilityProblem},
    pathfind::PathfindGrid,
    sounds::{SoundId, SoundPlayer},
    util::{plus_minus, COL_GOLD, COL_GREEN_0, COL_RED},
};

use crate::action_button::ActionButton;

const BG_COLOR: Color = BLACK;

pub struct ActivityPopup {
    characters: Characters,
    relevant_character_id: CharacterId,

    ui_state: Rc<RefCell<UiState>>,

    big_font: Font,
    font: Font,

    base_lines: Vec<String>,
    pub additional_line: Option<String>,

    next_button_id: Cell<u32>,
    choice_buttons: IndexMap<u32, ActionButton>,
    proceed_button: ActionButton,
    usability_msg: Option<UsabilityMessage>,
    movement_cost_slider: Option<MovementCostSlider>,

    proceed_button_events: Rc<RefCell<Vec<InternalUiEvent>>>,
    choice_button_events: Rc<RefCell<Vec<InternalUiEvent>>>,
    selected_choice_button_ids: Vec<u32>,
    hovered_choice_button_id: Option<u32>,

    pub last_drawn_rectangle: Rect,
    sound_player: SoundPlayer,
    pathfind_grid: Rc<PathfindGrid>,
}

struct UsabilityMessage {
    text: String,
    is_warning: bool,
}

impl ActivityPopup {
    pub fn new(
        big_font: Font,
        font: Font,
        state: Rc<RefCell<UiState>>,
        characters: Characters,
        active_character_id: CharacterId,
        sound_player: SoundPlayer,
        pathfind_grid: Rc<PathfindGrid>,
    ) -> Self {
        let proceed_button_events = Rc::new(RefCell::new(vec![]));

        let mut next_button_id = 0;
        let mut proceed_button = ActionButton::new(
            ButtonAction::Proceed,
            Some(Rc::clone(&proceed_button_events)),
            next_button_id,
            None,
            &font,
        );
        proceed_button.set_parent_bg_color(BG_COLOR);
        next_button_id += 1;

        Self {
            characters,
            relevant_character_id: active_character_id,
            ui_state: state,
            big_font,
            font,
            base_lines: vec![],
            additional_line: None,
            selected_choice_button_ids: Default::default(),
            choice_buttons: Default::default(),
            proceed_button,
            usability_msg: None,
            next_button_id: Cell::new(next_button_id),
            proceed_button_events,
            choice_button_events: Rc::new(RefCell::new(vec![])),
            movement_cost_slider: None,
            hovered_choice_button_id: None,
            last_drawn_rectangle: Default::default(),
            sound_player,
            pathfind_grid,
        }
    }

    pub fn draw(&mut self, x: f32, y: f32) {
        if matches!(
            &*self.ui_state.borrow(),
            UiState::Idle { .. } | UiState::ChoosingAction
        ) {
            self.last_drawn_rectangle = Rect {
                x,
                y,
                w: 0.0,
                h: 0.0,
            };
            return;
        }

        let tooltip_pad = 12.0;
        let margin_under_header = 12.0;

        let detail_font_size = 16;
        let base_text_params = TextParams {
            font: Some(&self.font),
            font_size: detail_font_size,
            color: WHITE,
            ..Default::default()
        };
        let header_font_size = 24;
        let header_params = TextParams {
            font: Some(&self.big_font),
            font_size: header_font_size,
            color: YELLOW,
            ..Default::default()
        };

        let mut measured_lines = vec![];

        let header_dimensions = measure_text_with_font_tags(
            &self.base_lines[0],
            header_params.font,
            header_params.font_size,
            1.0,
        );
        measured_lines.push((&self.base_lines[0], header_dimensions));

        for line in self.base_lines.iter().skip(1) {
            let dimensions = measure_text_with_font_tags(
                line,
                base_text_params.font,
                base_text_params.font_size,
                1.0,
            );
            measured_lines.push((line, dimensions));
        }

        if let Some(line) = &self.additional_line {
            let dimensions = measure_text_with_font_tags(
                line,
                base_text_params.font,
                base_text_params.font_size,
                1.0,
            );
            measured_lines.push((line, dimensions));
        }

        let empty_line_h = 0.0;
        let detail_line_h = detail_font_size as f32;
        let detail_line_margin = 7.0;

        let mut text_content_w = 0.0;
        let mut detail_text_w = 0.0;
        let mut detail_text_h = 0.0;
        for (i, (line, dim)) in measured_lines.iter().enumerate() {
            if dim.width > text_content_w {
                text_content_w = dim.width;
            }
            if i > 0 {
                let this_line_h = if line.is_empty() {
                    empty_line_h
                } else {
                    detail_line_h
                };

                if dim.width > detail_text_w {
                    detail_text_w = dim.width;
                }
                detail_text_h += this_line_h;
                if i > 1 {
                    // spacing above this line to the previous one
                    detail_text_h += detail_line_margin;
                }
            }
        }

        let draw_proceed_button = false;
        /*
        let draw_proceed_button = !matches!(
            &*self.ui_state.borrow(),
            UiState::ConfiguringAction(ConfiguredAction::Move { .. })
        );
         */

        let detail_rect_pad = 10.0;

        let height: f32 = (tooltip_pad * 2.0
            + header_dimensions.offset_y as f32
            + margin_under_header
            + detail_rect_pad * 2.0
            + detail_text_h)
            .max(74.0);

        let buttons_hor_pad = 20.0;
        let button_margin = 10.0;
        let margin_between_choices_and_proceed = 15.0;

        let detail_rect_w = detail_text_w + detail_rect_pad * 2.0;

        let mut width = tooltip_pad + detail_rect_w.max(measured_lines[0].1.width);

        if draw_proceed_button {
            width += self.proceed_button.size.0;
        }

        if !self.choice_buttons.is_empty() {
            width += buttons_hor_pad * 2.0;
            for btn in self.choice_buttons.values() {
                width += btn.size.0;
            }
            width += (self.choice_buttons.len() - 1) as f32 * button_margin;
        } else {
            width += tooltip_pad;
        }
        if !self.choice_buttons.is_empty() && draw_proceed_button {
            width += margin_between_choices_and_proceed;
        }

        // Prevent warning text (drawn in top-right corner) from colliding with header, when no enhancements
        //width = width.max(340.0);

        draw_rectangle(x, y - height, width, height, BG_COLOR);

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

        let x0 = x + tooltip_pad;

        let header_y = y - height + tooltip_pad + header_dimensions.offset_y;
        draw_text_rounded(&measured_lines[0].0, x0, header_y, header_params.clone());

        //dbg!(&measured_lines);

        if measured_lines.len() > 1 {
            let rect_x = x + tooltip_pad;
            let rect_y =
                y - height + tooltip_pad + header_dimensions.offset_y + margin_under_header;

            let rect_h = detail_text_h + detail_rect_pad * 2.0;
            draw_rectangle(
                rect_x,
                rect_y,
                detail_rect_w,
                rect_h,
                Color::new(1.0, 1.0, 1.0, 0.1),
            );
            draw_rounded_rectangle_lines(
                rect_x,
                rect_y,
                detail_rect_w,
                rect_h,
                1.0,
                BG_COLOR, //Color::new(1.0, 1.0, 1.0, 0.15),
                4.0,
                Some((BG_COLOR, 3.0)),
            );
            //draw_rounded_rectangle_lines(x, y, w, h, thickness, color, inner_rounding, outer);
            /*
            draw_line(
                x0,
                rect_y + detail_rect_pad,
                x0 + 100.0,
                rect_y + detail_rect_pad,
                1.0,
                YELLOW,
            );
             */
            let mut text_y = rect_y + detail_rect_pad + measured_lines[1].1.offset_y;
            for (line, _dim) in measured_lines.iter().skip(1) {
                //draw_line(x0, text_y, x0 + 100.0, text_y, 1.0, MAGENTA);
                if line.is_empty() {
                    text_y += empty_line_h + detail_line_margin;
                } else {
                    draw_text_with_font_tags(
                        line,
                        x0 + detail_rect_pad,
                        text_y,
                        base_text_params.clone(),
                        true,
                    );
                    text_y += detail_line_h + detail_line_margin;
                }
            }
            /*
            draw_line(
                x0,
                rect_y + rect_h - detail_rect_pad,
                x0 + 100.0,
                rect_y + rect_h - detail_rect_pad,
                1.0,
                YELLOW,
            );
             */
        }

        //draw_line(vert_line_x, y0+8.0, vert_line_x, y ,1.0, Color::new(0.51, 0.51, 0.51, 0.6));

        let mut btn_x = x + tooltip_pad + detail_rect_w + buttons_hor_pad;

        let btn_y = y - tooltip_pad - 2.0 - REGULAR_ACTION_BUTTON_SIZE.1;
        //let y_btn = y - height / 2.0 - 32.0;

        let first_btn_x = btn_x;

        let configuring_action = matches!(&*self.ui_state.borrow(), UiState::ConfiguringAction(..));

        if !self.choice_buttons.is_empty() && configuring_action {
            let buttons_w = self.choice_buttons.len() as f32 * REGULAR_ACTION_BUTTON_SIZE.0
                + (self.choice_buttons.len() - 1) as f32 * button_margin;

            let label = "Enhancements:";
            let dim = measure_text_with_font_tags(label, Some(&self.font), 16, 1.0);
            let label_x = btn_x + buttons_w / 2.0 - dim.width / 2.0;

            draw_text_with_font_tags(
                label,
                label_x,
                btn_y - 15.0,
                TextParams {
                    font: Some(&self.font),
                    font_size: 16,
                    color: YELLOW.with_alpha(0.7),
                    ..Default::default()
                },
                true,
            );
            let line_y = btn_y - 7.0;
            draw_line(
                btn_x,
                line_y,
                btn_x + buttons_w,
                line_y,
                1.0,
                Color::new(1.0, 1.0, 1.0, 0.3),
            );
        }

        for btn in self.choice_buttons.values() {
            btn.draw(btn_x, btn_y);
            btn_x += btn.size.0 + button_margin;
        }

        if let Some(usability_msg) = &self.usability_msg {
            let font_size = 22;
            let text_dim =
                measure_text_with_font_tags(&usability_msg.text, Some(&self.font), font_size, 1.0);
            let error_x = x + width - text_dim.width - 7.0;
            let error_y = y - height - 12.0;
            let pad = 2.0;
            draw_rectangle(
                error_x - pad,
                error_y - text_dim.offset_y - pad,
                text_dim.width + pad * 2.0,
                text_dim.height + pad * 2.0,
                Color::new(1.0, 1.0, 1.0, 0.7),
            );
            let color = if usability_msg.is_warning {
                COL_RED
            } else {
                BLACK
            };
            draw_text_with_font_tags(
                &usability_msg.text,
                error_x,
                error_y,
                TextParams {
                    font: Some(&self.font),
                    font_size,
                    color,
                    ..Default::default()
                },
                true,
            );
        }

        if draw_proceed_button {
            if !self.choice_buttons.is_empty() {
                btn_x += margin_between_choices_and_proceed;
            }
            if self.usability_msg.is_none() {
                self.proceed_button.draw(btn_x, btn_y + 6.0);
            }
        }

        btn_x = first_btn_x; // step back to render tooltips in the right positions
        for btn in self.choice_buttons.values() {
            if self.hovered_choice_button_id == Some(btn.id) {
                let detailed_tooltip = is_key_down(KeyCode::LeftAlt);
                draw_button_tooltip(&self.font, (btn_x, btn_y), &btn.tooltip(), detailed_tooltip);
            }

            btn_x += btn.size.0 + button_margin;
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
                        if self.hovered_choice_button_id.is_none() {
                            self.sound_player.play(SoundId::HoverButton);
                        }
                        self.hovered_choice_button_id = Some(id);
                    } else if self.hovered_choice_button_id == Some(id) {
                        self.hovered_choice_button_id = None;
                    }
                }

                InternalUiEvent::ButtonClicked { id, .. } => {
                    self.sound_player.play(SoundId::ClickButton);
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
                        UiState::ChoosingAction | UiState::Idle { .. } => unreachable!(),
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
            let max_cost = character
                .stamina
                .current()
                .max(character.action_points.current());
            slider.set_max_allowed(max_cost);

            assert!(cost <= max_cost);

            slider.selected_i = cost;
        }
    }

    pub fn set_movement_cost(&mut self, cost: u32) {
        // TODO: bug: this unwrap panicked, when clicking on an enemy on the grid?
        let slider = self.movement_cost_slider.as_mut().unwrap();
        let character = self.characters.get(self.relevant_character_id);
        let max_cost = character.action_points.current();
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
        let enabled_quick_actions = self
            .characters
            .get(self.relevant_character_id)
            .enabled_quick_actions
            .get();

        if self.movement_cost() > 0 {
            return (self.movement_cost() as i32, 0);
        }

        let borrowed_state = self.ui_state.borrow();
        let base_action = match &*borrowed_state {
            UiState::ConfiguringAction(configured_action) => Some(configured_action),
            _ => None,
        };

        let mut reserved_from_action = base_action
            .as_ref()
            .map(|action| action.base_action_point_cost())
            .unwrap_or(0);

        if !enabled_quick_actions {
            reserved_from_action += base_action
                .as_ref()
                .map(|action| action.base_action().quick_point_cost() as i32)
                .unwrap_or(0);
        }

        let mut reserved_from_choices: i32 = 0;

        for action in self.selected_choices() {
            reserved_from_choices += action.action_point_cost();
            reserved_from_choices -= action.action_point_discount() as i32;

            if !enabled_quick_actions {
                reserved_from_choices += action.quick_point_cost() as i32;
            }
        }
        let mut additional_hovered_from_choices = 0;
        if let Some(id) = self.hovered_choice_button_id {
            if !self.selected_choice_button_ids.contains(&id) {
                let action_point_cost = self.choice_buttons[&id].action.action_point_cost();
                additional_hovered_from_choices += action_point_cost;

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
        let enabled_quick_actions = self
            .characters
            .get(self.relevant_character_id)
            .enabled_quick_actions
            .get();

        let borrowed_state = self.ui_state.borrow();
        let base_action = match &*borrowed_state {
            UiState::ConfiguringAction(configured_action) => Some(configured_action),
            _ => None,
        };

        let mut sta = base_action
            .as_ref()
            .map(|action| action.stamina_cost())
            .unwrap_or(0);

        if enabled_quick_actions {
            sta += base_action
                .as_ref()
                .map(|action| action.base_action().quick_point_cost())
                .unwrap_or(0);
        }

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

    fn new_button(&self, btn_action: ButtonAction) -> ActionButton {
        let mut btn = ActionButton::new(
            btn_action,
            Some(Rc::clone(&self.choice_button_events)),
            self.next_button_id.get(),
            None,
            &self.font,
        );
        btn.set_parent_bg_color(BG_COLOR);
        self.next_button_id.set(self.next_button_id.get() + 1);
        btn
    }

    fn new_button_with_character_dependency(
        &self,
        btn_action: ButtonAction,
        character: Rc<Character>,
        font: &Font,
    ) -> ActionButton {
        let mut btn = ActionButton::new(
            btn_action,
            Some(Rc::clone(&self.choice_button_events)),
            self.next_button_id.get(),
            Some(character),
            font,
        );
        btn.set_parent_bg_color(BG_COLOR);
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

        let mut movement_cost_slider = None;
        self.selected_choice_button_ids.clear();

        println!("on_new_state");
        //dbg!(self.ui_state.borrow());

        match &mut *self.ui_state.borrow_mut() {
            UiState::ConfiguringAction(configured_action) => {
                let tooltip = relevant_action_button.as_ref().unwrap().tooltip();
                lines.push(tooltip.header.to_string());
                if !tooltip.technical_description.is_empty() {
                    //lines.push("".to_string());
                    lines.extend_from_slice(&tooltip.technical_description);
                }

                match configured_action {
                    ConfiguredAction::Attack {
                        attack,
                        selected_enhancements,
                        ..
                    } => {
                        let character = self.characters.get(active_character_id);
                        let known_attack_enhancements =
                            character.known_attack_enhancements(attack.hand);

                        for (_label, enhancement) in known_attack_enhancements {
                            let btn = self.new_button(ButtonAction::AttackEnhancement(enhancement));
                            btn.enabled.set(
                                character
                                    .can_use_attack_enhancement(HandType::MainHand, &enhancement),
                            );
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
                            if character.knows_ability_enhancement(enhancement) {
                                let btn =
                                    self.new_button(ButtonAction::AbilityEnhancement(enhancement));

                                btn.enabled.set(
                                    character.can_use_ability_enhancement(ability, enhancement),
                                );

                                popup_buttons.push(btn);
                            }
                        }
                    }

                    ConfiguredAction::Move { .. } => {
                        let char = self.characters.get(active_character_id);
                        let max_spend = char.stamina.current().max(char.action_points.current());
                        movement_cost_slider =
                            Some(MovementCostSlider::new(max_spend, self.font.clone()));
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
                lines.push("Reaction?".to_string());
                lines.push(format!(
                    "|<name>{}| attacks |<name>{}|!",
                    attacker.name, defender.name
                ));
                lines.push(format!(
                    "|<red_dice>| |<stat>Attack| {} vs |<shield>|<stat>Evasion| {}",
                    plus_minus(attacker.attack_modifier(*hand)),
                    defender.evasion()
                ));

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
                lines.push("Reaction?".to_string());
                lines.push(format!(
                    "|<name>{}| attacked |<name>{}| for {} damage",
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
                lines.push("Reaction?".to_string());
                lines.push(format!(
                    "|<name>{}| has an attack opportunity",
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
                lines.push("Reaction?".to_string());
                lines.push(format!(
                    "|<name>{}| has an attack opportunity",
                    self.characters.get(*reactor).name
                ));

                let btn = self.new_button_with_character_dependency(
                    ButtonAction::OpportunityAttack,
                    self.characters.get_rc(*reactor).clone(),
                    &self.font,
                );
                popup_buttons.push(btn);
            }

            UiState::ChoosingAction | UiState::Idle { .. } => {}
        }

        let mut choice_buttons = IndexMap::new();
        for mut btn in popup_buttons {
            btn.event_sender = Some(EventSender {
                queue: Rc::clone(&self.choice_button_events),
            });
            choice_buttons.insert(btn.id, btn);
        }

        self.movement_cost_slider = movement_cost_slider;

        self.base_lines = lines;
        self.choice_buttons = choice_buttons;

        // We must have assigned choice_buttons before calling this, since we may have selected enhancements that we assume
        // correspond to choice buttons
        self.refresh_enabled_state();
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
            &self.pathfind_grid,
        );

        let mut enabled = false;
        let mut usability_msg = None;
        let mut is_usability_warning = true;

        if !enough_ap {
            usability_msg = Some("Not enough AP".to_string());
        } else if !enough_mana {
            usability_msg = Some("Not enough mana".to_string());
        } else if !enough_stamina {
            usability_msg = Some("Not enough stamina".to_string());
        } else if let Some(e) = usability_problem {
            if !matches!(e, UsabilityProblem::SelectDestination) {
                is_usability_warning = matches!(
                    e,
                    UsabilityProblem::OutOfReach
                        | UsabilityProblem::NoLineOfSight
                        | UsabilityProblem::NotEnoughAp
                );
                usability_msg = Some(e.message().to_string());
            }
        } else {
            enabled = true;
        }

        self.usability_msg = usability_msg.map(|e| {
            let text = if is_usability_warning {
                format!("|<warning>| {e}")
            } else {
                e.to_string()
            };
            UsabilityMessage {
                text,
                is_warning: is_usability_warning,
            }
        });

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

struct MovementCostSlider {
    max: u32,
    max_allowed: u32,
    selected_i: u32,
    //is_sliding: bool,
    cell_w: f32,
    cell_h: f32,
    has_changed: Cell<bool>,
    font: Font,
}

impl MovementCostSlider {
    fn new(max: u32, font: Font) -> Self {
        Self {
            max,
            max_allowed: 0,
            selected_i: 0,
            //is_sliding: false,
            cell_w: 30.0,
            cell_h: 20.0,
            has_changed: Cell::new(false),
            font,
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
        /*
        let (w, h) = (self.cell_w, self.cell_h);

        let pad = 2.0;
        for i in 0..self.selected_i + 1 {
            let x0 = x + w * i as f32;
            let color = if i == 0 { DARKGRAY } else { ORANGE };
            draw_rectangle(x0 + pad, y + pad, w - pad * 2.0, h - pad * 2.0, color);
        }
        for i in 0..self.max_allowed + 1 {
            let x0 = x + w * i as f32;
            draw_rectangle_lines2(x0, y, w, h, 1.0, LIGHTGRAY);
        }
        for i in self.max_allowed + 1..self.max + 1 {
            let x0 = x + w * i as f32;
            draw_rectangle_lines2(x0, y, w, h, 1.0, DARKGRAY);
        }

        let text = format!("{}", self.selected_i);
        let font_size = 16;
        let text_dim = measure_text(&text, Some(&self.font), font_size, 1.0);

        draw_text_rounded(
            &text,
            x + w * self.selected_i as f32 + w / 2.0 - text_dim.width / 2.0,
            y + w / 2.0,
            TextParams {
                font: Some(&self.font),
                font_size,
                color: WHITE,
                ..Default::default()
            },
        );

        let x0 = x + w * self.selected_i as f32;
        let margin = 1.0;
        draw_rectangle_lines2(
            x0 - margin,
            y - margin,
            w + margin * 2.0,
            h + margin * 2.0,
            2.0,
            YELLOW,
        );

        //draw_cross(x, y + h / 2.0 - w / 2.0, w, w, LIGHTGRAY, 2.0, 10.0);
         */
    }
}
