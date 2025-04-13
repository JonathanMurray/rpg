use std::{cell::RefCell, rc::Rc};

use indexmap::IndexMap;
use macroquad::{
    color::{Color, BLACK, LIGHTGRAY, WHITE, YELLOW},
    shapes::{draw_rectangle, draw_rectangle_lines},
    text::{draw_text_ex, Font, TextParams},
};

use crate::{
    action_button::{draw_button_tooltip, ButtonAction, EventSender, InternalUiEvent},
    base_ui::Drawable,
    core::{
        AttackEnhancement, BaseAction, MovementEnhancement, OnAttackedReaction, OnHitReaction,
        SpellEnhancement,
    },
    game_ui::UiState,
};

use crate::action_button::ActionButton;

pub struct ActivityPopup {
    state: UiState,

    font: Font,

    initial_lines: Vec<String>,
    pub target_line: Option<String>,
    pub reaction_probability_line: Option<String>,

    choice_buttons: IndexMap<u32, ActionButton>,
    proceed_button: ActionButton,

    enabled: bool,
    proceed_button_events: Rc<RefCell<Vec<InternalUiEvent>>>,
    choice_button_events: Rc<RefCell<Vec<InternalUiEvent>>>,
    base_action: Option<BaseAction>,
    selected_choice_button_ids: Vec<u32>,
    hovered_choice_button_id: Option<u32>,

    pub last_drawn_size: (f32, f32),
}

impl ActivityPopup {
    pub fn new(font: Font, state: UiState, mut proceed_button: ActionButton) -> Self {
        let proceed_button_events = Rc::new(RefCell::new(vec![]));
        proceed_button.event_sender = Some(EventSender {
            queue: Rc::clone(&proceed_button_events),
        });

        Self {
            state,
            font,
            initial_lines: vec![],
            target_line: None,
            reaction_probability_line: None,
            selected_choice_button_ids: Default::default(),
            choice_buttons: Default::default(),
            proceed_button,
            enabled: true,
            proceed_button_events,
            choice_button_events: Rc::new(RefCell::new(vec![])),
            base_action: None,
            hovered_choice_button_id: None,
            last_drawn_size: (0.0, 0.0),
        }
    }

    pub fn draw(&mut self, x: f32, y: f32) {
        if matches!(self.state, UiState::Idle | UiState::ChoosingAction) {
            self.last_drawn_size = (0.0, 0.0);
            return;
        }

        let x0 = x + 10.0;
        let mut y0 = y + 25.0;

        let bg_color = Color::new(0.2, 0.2, 0.2, 1.0);
        let border_color = LIGHTGRAY;

        let size = (500.0, 85.0);
        draw_rectangle(x, y, size.0, size.1, bg_color);
        draw_rectangle_lines(x, y, size.0, size.1, 2.0, border_color);
        self.last_drawn_size = size;

        let line_height = 18.0;

        let text_params = TextParams {
            font: Some(&self.font),
            font_size: 16,
            color: WHITE,
            ..Default::default()
        };

        for (i, line) in self.initial_lines.iter().enumerate() {
            let mut params = text_params.clone();
            if i == 0 {
                params.font_size = 22;
                params.color = BLACK;
                draw_text_ex(line, x0 + 2.0, y0 + 2.0, params.clone());
                params.color = YELLOW;
                draw_text_ex(line, x0, y0, params.clone());
            } else {
                draw_text_ex(line, x0, y0, params);
            }

            y0 += line_height;
        }

        if let Some(line) = &self.reaction_probability_line {
            draw_text_ex(line, x0, y0, text_params.clone());
            y0 += line_height;
        }

        if let Some(line) = &self.target_line {
            draw_text_ex(line, x0, y0, text_params.clone());
            y0 += line_height;
        }

        let mut choice_description_line = "".to_string();
        for action in self.selected_choices() {
            choice_description_line.push('[');
            let s = match action {
                ButtonAction::AttackEnhancement(enhancement) => enhancement.description,
                ButtonAction::SpellEnhancement(enhancement) => enhancement.name,
                ButtonAction::MovementEnhancement(enhancement) => enhancement.name,
                ButtonAction::OnAttackedReaction(reaction) => reaction.name,
                ButtonAction::OnHitReaction(reaction) => reaction.name,
                ButtonAction::Action(..) | ButtonAction::Proceed => unreachable!(),
            };
            choice_description_line.push_str(s);
            choice_description_line.push(']');
        }
        draw_text_ex(&choice_description_line, x0, y0, text_params.clone());
        y0 += line_height;

        if self.enabled {
            match &self.state {
                UiState::ConfiguringAction(base_action) => {
                    match base_action {
                        BaseAction::Move { range, .. } => {
                            let percentage: u32 = self
                                .selected_choices()
                                .map(|action| action.unwrap_movement_enhancement().add_percentage)
                                .sum();
                            let range = range * (1.0 + percentage as f32 / 100.0);
                            let text = format!("range: {range:.2}");
                            draw_text_ex(&text, x0, y0, text_params.clone());
                        }
                        BaseAction::Attack { .. }
                        | BaseAction::CastSpell(..)
                        | BaseAction::SelfEffect(..) => {}
                    };
                }
                UiState::ReactingToAttack { .. } | UiState::ReactingToHit { .. } => {}
                UiState::Idle | UiState::ChoosingAction => unreachable!(),
            }
        }

        let y_btn = y + 10.0;
        let mut x_btn = x + 425.0;

        self.proceed_button.draw(x_btn, y_btn + 6.0);
        x_btn -= self.proceed_button.size.0 + 10.0;

        for btn in self.choice_buttons.values().rev() {
            btn.draw(x_btn, y_btn);

            if self.hovered_choice_button_id == Some(btn.id) {
                draw_button_tooltip(&self.font, (x_btn, y_btn), &btn.tooltip_lines[..]);
            }

            x_btn -= btn.size.0 + 10.0;
        }
    }

    fn are_choice_buttons_mutually_exclusive(&self) -> bool {
        matches!(self.state, UiState::ReactingToAttack { .. })
            || matches!(self.state, UiState::ReactingToHit { .. })
            || matches!(
                self.state,
                UiState::ConfiguringAction(BaseAction::Move { .. })
            )
    }

    pub fn update(&mut self) -> Option<ActivityPopupOutcome> {
        let mut changed_movement_range = false;
        let mut changed_on_attacked_reaction = None;
        for event in self.choice_button_events.borrow_mut().drain(..) {
            match event {
                InternalUiEvent::ButtonHovered(id, _button_action, hovered_pos) => {
                    if hovered_pos.is_some() {
                        self.hovered_choice_button_id = Some(id);
                    } else if self.hovered_choice_button_id == Some(id) {
                        self.hovered_choice_button_id = None;
                    }
                }

                InternalUiEvent::ButtonClicked(id, _button_action) => {
                    let clicked_btn = &self.choice_buttons[&id];
                    clicked_btn.toggle_highlighted();

                    if let ButtonAction::MovementEnhancement(..) = clicked_btn.action {
                        changed_movement_range = true;
                    }

                    // Some choices work like radio boxes
                    if self.are_choice_buttons_mutually_exclusive() {
                        for btn in self.choice_buttons.values() {
                            if btn.id != id {
                                btn.highlighted.set(false);
                            }
                        }
                    }

                    self.selected_choice_button_ids.clear();
                    for btn in self.choice_buttons.values() {
                        if btn.highlighted.get() {
                            self.selected_choice_button_ids.push(btn.id);
                        }
                    }

                    if let ButtonAction::OnAttackedReaction(..) = clicked_btn.action {
                        let maybe_reaction = self
                            .selected_choices()
                            .map(|action| action.unwrap_on_attacked_reaction())
                            .next();
                        changed_on_attacked_reaction = Some(maybe_reaction)
                    }
                }
            };
        }

        for event in self.proceed_button_events.borrow_mut().drain(..) {
            if matches!(event, InternalUiEvent::ButtonClicked(..)) {
                return Some(ActivityPopupOutcome::ClickedProceed);
            }
        }

        if changed_movement_range {
            let mut added_percentage = 0;
            for action in self.selected_choices() {
                if let ButtonAction::MovementEnhancement(enhancement) = action {
                    added_percentage += enhancement.add_percentage;
                }
            }
            return Some(ActivityPopupOutcome::ChangedMovementRangePercentage(
                added_percentage,
            ));
        }

        if let Some(maybe_reaction) = changed_on_attacked_reaction {
            return Some(ActivityPopupOutcome::ChangedOnAttackedReaction(
                maybe_reaction,
            ));
        }

        None
    }

    pub fn selected_choices(&self) -> impl Iterator<Item = &ButtonAction> {
        self.selected_choice_button_ids
            .iter()
            .map(|id| &self.choice_buttons[id].action)
    }

    pub fn selected_attack_enhancements(&self) -> Vec<AttackEnhancement> {
        self.selected_choices()
            .map(|action| action.unwrap_attack_enhancement())
            .collect()
    }

    pub fn selected_spell_enhancements(&self) -> Vec<SpellEnhancement> {
        self.selected_choices()
            .map(|action| action.unwrap_spell_enhancement())
            .collect()
    }

    pub fn selected_movement_enhancements(&self) -> Vec<MovementEnhancement> {
        self.selected_choices()
            .map(|action| action.unwrap_movement_enhancement())
            .collect()
    }

    pub fn selected_on_attacked_reaction(&self) -> Option<OnAttackedReaction> {
        self.selected_choices()
            .next()
            .map(|action| action.unwrap_on_attacked_reaction())
    }

    pub fn selected_on_hit_reaction(&self) -> Option<OnHitReaction> {
        self.selected_choices()
            .next()
            .map(|action| action.unwrap_on_hit_reaction())
    }

    pub fn select_movement_option(&mut self, selected_enhancement: Option<usize>) {
        assert!(matches!(
            self.state,
            UiState::ConfiguringAction(BaseAction::Move { .. })
        ));

        for (i, (_id, btn)) in self.choice_buttons.iter().enumerate() {
            btn.highlighted.set(selected_enhancement == Some(i));
        }

        self.selected_choice_button_ids.clear();
        for btn in self.choice_buttons.values() {
            if btn.highlighted.get() {
                self.selected_choice_button_ids.push(btn.id);
            }
        }
    }

    pub fn reserved_and_hovered_action_points(&self) -> (u32, u32) {
        let reserved_from_action = self
            .base_action
            .as_ref()
            .map(|action| action.action_point_cost())
            .unwrap_or(0);
        let mut reserved_from_choices = 0;
        for action in self.selected_choices() {
            reserved_from_choices += action.action_point_cost();
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
        let mut mana = self
            .base_action
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
        let mut sta = self
            .base_action
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

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        for btn in &mut self.choice_buttons.values() {
            btn.enabled.set(enabled);
        }
        self.proceed_button.enabled.set(enabled);
    }

    pub fn set_state(&mut self, state: UiState, lines: Vec<String>, buttons: Vec<ActionButton>) {
        if self.state != state {
            // Assume that a change in the layout caused all buttons to no longer be hovered
            for btn in self.choice_buttons.values() {
                btn.notify_hidden();
            }
            self.proceed_button.notify_hidden();
        }

        let mut choice_buttons = IndexMap::new();
        for mut btn in buttons {
            btn.event_sender = Some(EventSender {
                queue: Rc::clone(&self.choice_button_events),
            });
            choice_buttons.insert(btn.id, btn);
        }

        self.state = state;
        self.initial_lines = lines;
        self.choice_buttons = choice_buttons;
        self.selected_choice_button_ids.clear();

        self.base_action = if let UiState::ConfiguringAction(base_action) = state {
            Some(base_action)
        } else {
            None
        };
    }
}

pub enum ActivityPopupOutcome {
    ChangedMovementRangePercentage(u32),
    ClickedProceed,
    ChangedOnAttackedReaction(Option<OnAttackedReaction>),
}
