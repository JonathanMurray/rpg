use std::{cell::RefCell, collections::HashMap, rc::Rc};

use indexmap::IndexMap;
use macroquad::{
    color::{BLACK, DARKGRAY, GRAY, GREEN, LIGHTGRAY, ORANGE, RED, WHITE, YELLOW},
    input::{is_mouse_button_down, is_mouse_button_pressed, mouse_position, MouseButton},
    math::Rect,
    shapes::{draw_line, draw_rectangle, draw_rectangle_lines},
    text::{draw_text_ex, measure_text, Font, TextParams},
    texture::Texture2D,
};

use crate::{
    action_button::{
        draw_button_tooltip, ButtonAction, ButtonSelected, EventSender, InternalUiEvent,
    },
    base_ui::Drawable,
    core::{
        AttackEnhancement, BaseAction, CharacterId, Characters, OnAttackedReaction, OnHitReaction,
        SpellEnhancement,
    },
    drawing::{draw_cross, draw_dashed_line},
    game_ui::UiState,
    textures::IconId,
};

use crate::action_button::ActionButton;

pub struct ActivityPopup {
    characters: Characters,
    active_character_id: CharacterId,
    icons: HashMap<IconId, Texture2D>,
    state: UiState,

    font: Font,

    base_lines: Vec<String>,
    pub additional_line: Option<String>,

    next_button_id: u32,
    choice_buttons: IndexMap<u32, ActionButton>,
    proceed_button: ActionButton,

    movement_stamina_slider: Option<MovementStaminaSlider>,

    proceed_button_events: Rc<RefCell<Vec<InternalUiEvent>>>,
    choice_button_events: Rc<RefCell<Vec<InternalUiEvent>>>,
    base_action: Option<BaseAction>,
    selected_choice_button_ids: Vec<u32>,
    hovered_choice_button_id: Option<u32>,

    movement_base_ap_cost: u32,

    // TODO remove this and just use the slider's value everywhere
    movement_stamina_cost: u32,

    pub last_drawn_rectangle: Rect,
}

impl ActivityPopup {
    pub fn new(
        font: Font,
        state: UiState,
        icons: HashMap<IconId, Texture2D>,
        characters: Characters,
        active_character_id: CharacterId,
    ) -> Self {
        let proceed_button_events = Rc::new(RefCell::new(vec![]));

        let mut next_button_id = 0;
        let proceed_button = ActionButton::new(
            ButtonAction::Proceed,
            &proceed_button_events,
            next_button_id,
            &icons,
            None,
        );
        next_button_id += 1;

        Self {
            characters,
            active_character_id,
            icons,
            state,
            font,
            base_lines: vec![],
            additional_line: None,
            selected_choice_button_ids: Default::default(),
            choice_buttons: Default::default(),
            proceed_button,
            next_button_id,
            proceed_button_events,
            choice_button_events: Rc::new(RefCell::new(vec![])),
            movement_stamina_slider: None,
            base_action: None,
            hovered_choice_button_id: None,
            movement_base_ap_cost: 0,
            movement_stamina_cost: 0,
            last_drawn_rectangle: Default::default(),
        }
    }

    pub fn draw(&mut self, x: f32, y: f32) {
        if matches!(self.state, UiState::Idle | UiState::ChoosingAction) {
            self.last_drawn_rectangle = Rect {
                x,
                y,
                w: 0.0,
                h: 0.0,
            };
            return;
        }

        let bg_color = BLACK;

        let top_pad = 5.0;

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

        let line_margin = 8.0;
        let mut text_content_h = top_pad + header_dimensions.offset_y;
        let mut text_content_w = 0.0;
        for (_line, dim) in &measured_lines {
            //text_content_h += dim.height;
            text_content_h += dim.offset_y;
            if dim.width > text_content_w {
                text_content_w = dim.width;
            }
        }
        text_content_h += (measured_lines.len() - 1) as f32 * line_margin;

        let height = text_content_h.max(74.0);

        let hor_pad = 10.0;
        let margin_between_text_and_buttons = 20.0;
        let button_margin = 10.0;

        let mut width =
            text_content_w + margin_between_text_and_buttons + self.proceed_button.size.0;

        for btn in self.choice_buttons.values() {
            width += button_margin + btn.size.0;
        }

        //let sprint_stamina_text = "Sprint (stamina)";
        let sprint_stamina_text = "Spend stamina:";
        let sprint_stamina_margin = 15.0;
        if let Some(slider) = &self.movement_stamina_slider {
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

        draw_dashed_line((x, y), (x + width, y), 1.0, GRAY, 5.0);

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
                draw_text_ex(line, x0 + 2.0, y0 + 2.0, params.clone());
                params.color = YELLOW;
                draw_text_ex(line, x0, y0, params.clone());
            } else {
                draw_text_ex(line, x0, y0, base_text_params.clone());
            }

            y0 += dim.offset_y + line_margin;
        }

        let mut x_btn = x0 + text_content_w + margin_between_text_and_buttons;

        if let UiState::ConfiguringAction(BaseAction::Move) = self.state {
            let mut text = format!("{} AP", self.movement_base_ap_cost);
            if self.movement_stamina_cost > 0 {
                text.push_str(&format!(" -{}", self.movement_stamina_cost));
            }

            draw_text_ex(&text, x0, y0, base_text_params.clone());
        }

        if let Some(slider) = &mut self.movement_stamina_slider {
            let text_dimensions = draw_text_ex(
                sprint_stamina_text,
                x_btn,
                y - height + 20.0,
                base_text_params.clone(),
            );
            slider.draw(x_btn, y - slider.size().1 - 5.0);
            self.movement_stamina_cost = slider.selected_stamina();
            let movement_config_w = slider.size().0.max(text_dimensions.width);
            x_btn += movement_config_w + sprint_stamina_margin;
        }

        let y_btn = y - height / 2.0 - 32.0;

        for btn in self.choice_buttons.values() {
            btn.draw(x_btn, y_btn);

            if self.hovered_choice_button_id == Some(btn.id) {
                draw_button_tooltip(&self.font, (x_btn, y_btn), &btn.tooltip);
            }

            x_btn += btn.size.0 + button_margin;
        }

        self.proceed_button.draw(x_btn, y_btn + 6.0);
    }

    fn are_choice_buttons_mutually_exclusive(&self) -> bool {
        matches!(self.state, UiState::ReactingToAttack { .. })
            || matches!(self.state, UiState::ReactingToHit { .. })
            || matches!(self.state, UiState::ConfiguringAction(BaseAction::Move))
    }

    pub fn update(&mut self) -> Option<ActivityPopupOutcome> {
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
                    clicked_btn.toggle_selected();

                    // Some choices work like radio boxes
                    if self.are_choice_buttons_mutually_exclusive() {
                        for btn in self.choice_buttons.values() {
                            if btn.id != id {
                                btn.deselect();
                            }
                        }
                    }

                    self.selected_choice_button_ids.clear();
                    for btn in self.choice_buttons.values() {
                        if btn.selected.get() == ButtonSelected::Yes {
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

        if let Some(maybe_reaction) = changed_on_attacked_reaction {
            return Some(ActivityPopupOutcome::ChangedOnAttackedReaction(
                maybe_reaction,
            ));
        }

        None
    }

    fn selected_choices(&self) -> impl Iterator<Item = &ButtonAction> {
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

    pub fn set_movement_ap_cost(&mut self, ap_cost: u32) {
        assert!(matches!(
            self.state,
            UiState::ConfiguringAction(BaseAction::Move)
        ));

        self.movement_base_ap_cost = ap_cost;

        if let Some(slider) = self.movement_stamina_slider.as_mut() {
            // Each AP spent on movement can be accompanied by 1 stamina point
            slider.set_max_allowed(ap_cost / 2);
            self.movement_stamina_cost = slider.selected_stamina();
        }
    }

    pub fn movement_ap_cost(&self) -> u32 {
        self.movement_base_ap_cost - self.movement_stamina_cost
    }

    pub fn movement_stamina_cost(&self) -> u32 {
        self.movement_stamina_cost
    }

    pub fn reserved_and_hovered_action_points(&self) -> (u32, u32) {
        if self.movement_base_ap_cost > 0 {
            return (self.movement_ap_cost(), 0);
        }

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
        if self.movement_stamina_cost > 0 {
            return self.movement_stamina_cost;
        }

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

    pub fn set_enabled(&mut self, mut enabled: bool) {
        if self.movement_ap_cost()
            > self
                .characters
                .get(self.active_character_id)
                .action_points
                .current()
        {
            enabled = false;
        }

        self.proceed_button.enabled.set(enabled);
    }

    fn new_button(&mut self, btn_action: ButtonAction) -> ActionButton {
        let btn = ActionButton::new(
            btn_action,
            &self.choice_button_events,
            self.next_button_id,
            &self.icons,
            None,
        );
        self.next_button_id += 1;
        btn
    }

    pub fn set_state(
        &mut self,
        active_character_id: CharacterId,
        state: UiState,
        relevant_action_button: Option<Rc<ActionButton>>,
    ) {
        self.active_character_id = active_character_id;

        let mut lines = vec![];
        let mut popup_buttons = vec![];

        let mut stamina_slider = None;

        match state {
            UiState::ConfiguringAction(base_action) => {
                let tooltip = &relevant_action_button.unwrap().tooltip;
                lines.push(tooltip.header.to_string());
                lines.extend_from_slice(&tooltip.technical_description);

                match base_action {
                    BaseAction::Attack { hand, .. } => {
                        for (_subtext, enhancement) in self
                            .characters
                            .get(active_character_id)
                            .usable_attack_enhancements(hand)
                        {
                            let btn = self.new_button(ButtonAction::AttackEnhancement(enhancement));
                            popup_buttons.push(btn);
                        }
                    }
                    BaseAction::CastSpell(spell) => {
                        for enhancement in spell.possible_enhancements.iter().flatten().copied() {
                            if self
                                .characters
                                .get(active_character_id)
                                .can_use_spell_enhancement(spell, enhancement)
                            {
                                let btn =
                                    self.new_button(ButtonAction::SpellEnhancement(enhancement));
                                popup_buttons.push(btn);
                            }
                        }
                    }
                    BaseAction::Move => {
                        let active_char = self.characters.get(active_character_id);
                        let speed = active_char.move_speed;
                        lines.push(format!("Speed: {}", speed));
                        let stamina = &active_char.stamina;
                        if stamina.max > 0 {
                            let max_stamina_spend =
                                stamina.current().min(active_char.action_points.current());
                            stamina_slider = Some(MovementStaminaSlider::new(max_stamina_spend));
                        }
                    }
                }
            }

            UiState::ReactingToAttack {
                attacker: attacker_id,
                hand,
                reactor: reactor_id,
                is_within_melee,
            } => {
                let attacker = self.characters.get(attacker_id);
                let defender = self.characters.get(reactor_id);
                lines.push("React (on attacked)".to_string());
                let attacks_str = format!(
                    "{} attacks {} (d20+{} vs {})",
                    attacker.name,
                    defender.name,
                    attacker.attack_modifier(hand),
                    defender.evasion(),
                );
                lines.push(attacks_str);

                let defender = self.characters.get(reactor_id);

                for (_subtext, reaction) in defender.usable_on_attacked_reactions(is_within_melee) {
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
            } => {
                let victim = self.characters.get(victim_id);
                lines.push("React (on hit)".to_string());
                lines.push(format!(
                    "{} attacked {} for {} damage",
                    self.characters.get(attacker_id).name,
                    victim.name,
                    damage,
                ));

                let victim = self.characters.get(victim_id);
                for (_subtext, reaction) in victim.usable_on_hit_reactions(is_within_melee) {
                    let btn_action = ButtonAction::OnHitReaction(reaction);
                    let btn = self.new_button(btn_action);
                    popup_buttons.push(btn);
                }
            }

            UiState::ChoosingAction | UiState::Idle => {}
        }

        if self.state != state {
            // Assume that a change in the layout caused all buttons to no longer be hovered
            for btn in self.choice_buttons.values() {
                btn.notify_hidden();
            }
            self.proceed_button.notify_hidden();
        }

        let mut choice_buttons = IndexMap::new();
        for mut btn in popup_buttons {
            btn.event_sender = Some(EventSender {
                queue: Rc::clone(&self.choice_button_events),
            });
            choice_buttons.insert(btn.id, btn);
        }

        self.movement_stamina_slider = stamina_slider;
        self.movement_base_ap_cost = 0;
        self.movement_stamina_cost = 0;

        self.state = state;
        self.base_lines = lines;
        self.choice_buttons = choice_buttons;
        self.selected_choice_button_ids.clear();

        // TODO remove this and instead just use self.state
        self.base_action = if let UiState::ConfiguringAction(base_action) = state {
            Some(base_action)
        } else {
            None
        };
    }
}

pub enum ActivityPopupOutcome {
    ClickedProceed,
    ChangedOnAttackedReaction(Option<OnAttackedReaction>),
}

struct MovementStaminaSlider {
    max: u32,
    max_allowed: u32,
    selected_i: u32,
    is_sliding: bool,
    cell_w: f32,
    cell_h: f32,
}

impl MovementStaminaSlider {
    fn new(max: u32) -> Self {
        Self {
            max,
            max_allowed: 0,
            selected_i: 0,
            is_sliding: false,
            cell_w: 35.0,
            cell_h: 28.0,
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

    fn selected_stamina(&self) -> u32 {
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
                self.selected_i = (i).min(self.max_allowed);
            }
        }

        draw_cross(x, y + h / 2.0 - w / 2.0, w, w, LIGHTGRAY, 2.0, 10.0);
    }
}
