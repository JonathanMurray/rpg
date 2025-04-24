use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use macroquad::rand;

use macroquad::{
    color::{BLACK, BLUE, DARKGRAY, GREEN, MAGENTA, ORANGE, RED, WHITE},
    input::mouse_position,
    shapes::{draw_line, draw_rectangle},
    text::Font,
    texture::Texture2D,
    window::{screen_height, screen_width},
};

use crate::{
    action_button::{
        draw_button_tooltip, ActionButton, ButtonAction, ButtonSelected, InternalUiEvent,
    },
    activity_popup::{ActivityPopup, ActivityPopupOutcome},
    base_ui::{Align, Container, Drawable, Element, LayoutDirection, Style, TextLine},
    character_sheet::CharacterSheet,
    conditions_ui::ConditionsList,
    core::{
        as_percentage, distance_between, prob_attack_hit, prob_spell_hit, Action, ActionReach,
        ActionTarget, AttackEnhancement, AttackOutcome, BaseAction, Character, CharacterId,
        Characters, CoreGame, GameEvent, Goodness, HandType, OnAttackedReaction, OnHitReaction,
        SpellTarget, SpellTargetOutcome,
    },
    game_ui_components::{
        ActionPointsRow, CharacterPortraits, CharacterSheetToggle, LabelledResourceBar, Log,
        PlayerPortraits,
    },
    grid::{
        Effect, EffectGraphics, EffectPosition, EffectVariant, GameGrid, GridOutcome,
        GridSwitchedTo, RangeIndicator,
    },
    target_ui::TargetUi,
    textures::{EquipmentIconId, IconId, PortraitId, SpriteId},
};

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum UiState {
    ChoosingAction,
    ConfiguringAction(BaseAction),
    ReactingToAttack {
        hand: HandType,
        attacker: CharacterId,
        reactor: CharacterId,
        is_within_melee: bool,
    },
    ReactingToHit {
        attacker: CharacterId,
        victim: CharacterId,
        damage: u32,
        is_within_melee: bool,
    },
    Idle,
}

#[derive(Debug, Copy, Clone, Default)]
struct StopWatch {
    remaining: Option<f32>,
}

impl StopWatch {
    fn set_to_at_least(&mut self, value: f32) {
        if let Some(remaining) = self.remaining {
            self.remaining = Some(remaining.max(value));
        } else {
            self.remaining = Some(value);
        }
    }

    fn update(&mut self, elapsed: f32) -> bool {
        if let Some(remaining) = &mut self.remaining {
            *remaining = (*remaining - elapsed).max(0.0);
            if *remaining <= 0.0 {
                self.remaining = None;
                return true;
            }
        }
        false
    }
}

struct CharacterUi {
    tracked_action_buttons: HashMap<String, Rc<ActionButton>>,
    action_points_row: ActionPointsRow,
    hoverable_buttons: Vec<Rc<ActionButton>>,
    actions_section: Container,
    character_sheet: CharacterSheet,
    health_bar: Rc<RefCell<LabelledResourceBar>>,
    mana_bar: Rc<RefCell<LabelledResourceBar>>,
    stamina_bar: Rc<RefCell<LabelledResourceBar>>,
    resource_bars: Container,
    conditions_list: ConditionsList,
}

pub struct UserInterface {
    characters: Characters,
    event_queue: Rc<RefCell<Vec<InternalUiEvent>>>,
    state: UiState,
    animation_stopwatch: StopWatch,

    font: Font,

    hovered_button: Option<(u32, ButtonAction, (f32, f32))>,
    active_character_id: CharacterId,

    pub game_grid: GameGrid,
    activity_popup: ActivityPopup,
    character_portraits: CharacterPortraits,
    player_portraits: PlayerPortraits,
    character_sheet_toggle: CharacterSheetToggle,
    character_uis: HashMap<CharacterId, CharacterUi>,
    target_ui: TargetUi,
    log: Log,
}

impl UserInterface {
    pub fn new(
        game: &CoreGame,
        sprites: HashMap<SpriteId, Texture2D>,
        icons: HashMap<IconId, Texture2D>,
        equipment_icons: HashMap<EquipmentIconId, Texture2D>,
        portrait_textures: HashMap<PortraitId, Texture2D>,
        simple_font: Font,
        decorative_font: Font,
        big_font: Font,
        background_textures: Vec<Texture2D>,
    ) -> Self {
        let characters = game.characters.clone();
        let active_character_id = game.active_character_id;

        let event_queue = Rc::new(RefCell::new(vec![]));
        let mut next_button_id = 1;

        let mut new_button = |btn_action, character: Option<&Character>, enabled: bool| {
            let btn =
                ActionButton::new(btn_action, &event_queue, next_button_id, &icons, character);
            btn.enabled.set(enabled);
            next_button_id += 1;
            btn
        };

        let mut character_uis: HashMap<CharacterId, CharacterUi> = Default::default();

        for character in characters.iter() {
            if !character.player_controlled {
                continue;
            }

            let mut tracked_action_buttons = HashMap::new();
            let mut hoverable_buttons = vec![];
            let mut basic_buttons = vec![];
            let mut spell_buttons = vec![];

            let mut attack_button_for_character_sheet = None;
            let mut spell_buttons_for_character_sheet = vec![];
            let mut attack_enhancement_buttons_for_character_sheet = vec![];

            for (_subtext, action) in character.known_actions() {
                let btn_action = ButtonAction::Action(action);
                let btn = Rc::new(new_button(btn_action, Some(character), true));
                tracked_action_buttons.insert(button_action_id(btn_action), Rc::clone(&btn));
                hoverable_buttons.push(Rc::clone(&btn));
                match action {
                    BaseAction::Attack { .. } => {
                        basic_buttons.push(btn);

                        let btn = Rc::new(new_button(btn_action, Some(character), false));
                        attack_button_for_character_sheet = Some(btn.clone());
                        hoverable_buttons.push(btn);
                    }
                    BaseAction::CastSpell(spell) => {
                        spell_buttons.push(btn);

                        let btn = Rc::new(new_button(btn_action, Some(character), false));

                        let enhancement_buttons: Vec<Rc<ActionButton>> = spell
                            .possible_enhancements
                            .iter()
                            .filter_map(|maybe_enhancement| *maybe_enhancement)
                            .map(|enhancement| {
                                let enhancement_btn = Rc::new(new_button(
                                    ButtonAction::SpellEnhancement(enhancement),
                                    None,
                                    false,
                                ));
                                hoverable_buttons.push(enhancement_btn.clone());
                                enhancement_btn
                            })
                            .collect();
                        spell_buttons_for_character_sheet.push((btn.clone(), enhancement_buttons));

                        hoverable_buttons.push(btn);
                    }
                    BaseAction::Move => {
                        basic_buttons.push(btn);
                    }
                }
            }

            let mut reaction_buttons_for_character_sheet = vec![];
            for (_subtext, reaction) in character.known_on_attacked_reactions() {
                let btn_action = ButtonAction::OnAttackedReaction(reaction);
                let btn = Rc::new(new_button(btn_action, None, false));
                hoverable_buttons.push(Rc::clone(&btn));
                reaction_buttons_for_character_sheet.push(btn);
            }
            for (_subtext, reaction) in character.known_on_hit_reactions() {
                let btn_action = ButtonAction::OnHitReaction(reaction);
                let btn = Rc::new(new_button(btn_action, None, false));
                hoverable_buttons.push(Rc::clone(&btn));
                reaction_buttons_for_character_sheet.push(btn);
            }

            for (_subtext, enhancement) in character.known_attack_enhancements(HandType::MainHand) {
                let btn_action = ButtonAction::AttackEnhancement(enhancement);
                let btn = Rc::new(new_button(btn_action, None, false));
                hoverable_buttons.push(Rc::clone(&btn));
                attack_enhancement_buttons_for_character_sheet.push(btn);
            }

            let character_sheet = CharacterSheet::new(
                &simple_font,
                character,
                &equipment_icons,
                attack_button_for_character_sheet,
                reaction_buttons_for_character_sheet,
                attack_enhancement_buttons_for_character_sheet,
                spell_buttons_for_character_sheet,
            );

            let mut upper_buttons = basic_buttons;
            let mut lower_buttons = spell_buttons;
            while lower_buttons.len() > upper_buttons.len() + 1 {
                upper_buttons.push(lower_buttons.pop().unwrap());
            }

            let upper_row = buttons_row(
                upper_buttons
                    .into_iter()
                    .map(|btn| Element::Rc(btn))
                    .collect(),
            );
            let lower_row = buttons_row(
                lower_buttons
                    .into_iter()
                    .map(|btn| Element::Rc(btn))
                    .collect(),
            );

            let actions_section = Container {
                layout_dir: LayoutDirection::Vertical,
                margin: 5.0,
                children: vec![upper_row, lower_row],
                ..Default::default()
            };

            let health_bar = Rc::new(RefCell::new(LabelledResourceBar::new(
                character.health.current(),
                character.health.max,
                "Health",
                RED,
                simple_font.clone(),
            )));
            let cloned_health_bar = Rc::clone(&health_bar);

            let mana_bar = Rc::new(RefCell::new(LabelledResourceBar::new(
                character.mana.current(),
                character.mana.max,
                "Mana",
                BLUE,
                simple_font.clone(),
            )));
            let cloned_mana_bar = Rc::clone(&mana_bar);

            let stamina_bar = Rc::new(RefCell::new(LabelledResourceBar::new(
                character.stamina.current(),
                character.stamina.max,
                "Stamina",
                GREEN,
                simple_font.clone(),
            )));
            let cloned_stamina_bar = Rc::clone(&stamina_bar);

            let resource_bars = Container {
                layout_dir: LayoutDirection::Horizontal,
                margin: 9.0,
                align: Align::End,
                children: vec![
                    Element::RcRefCell(cloned_health_bar),
                    Element::RcRefCell(cloned_mana_bar),
                    Element::RcRefCell(cloned_stamina_bar),
                ],
                style: Style {
                    border_color: Some(DARKGRAY),
                    padding: 5.0,
                    ..Default::default()
                },
                ..Default::default()
            };

            let action_points_row = ActionPointsRow::new(
                character.max_reactive_action_points,
                (20.0, 20.0),
                0.3,
                Style {
                    border_color: Some(WHITE),
                    ..Default::default()
                },
            );

            let character_ui = CharacterUi {
                tracked_action_buttons,
                action_points_row,
                actions_section,
                character_sheet,
                health_bar,
                mana_bar,
                stamina_bar,
                resource_bars,
                conditions_list: ConditionsList::new(simple_font.clone(), vec![]),
                hoverable_buttons,
            };

            character_uis.insert(character.id(), character_ui);
        }

        let ui_state = UiState::Idle;

        let grid_dimensions = (16, 12);
        let mut cell_backgrounds = vec![];
        for _ in 0..(grid_dimensions.0 * grid_dimensions.1) {
            let i = rand::gen_range(0, background_textures.len());
            cell_backgrounds.push(i);
        }

        let first_player_character_id = characters
            .iter_with_ids()
            .find(|(_id, ch)| ch.player_controlled)
            .unwrap()
            .0;

        let game_grid = GameGrid::new(
            first_player_character_id,
            characters.clone(),
            sprites,
            big_font.clone(),
            simple_font.clone(),
            background_textures,
            grid_dimensions,
            cell_backgrounds,
        );

        let player_portraits = PlayerPortraits::new(
            &characters,
            first_player_character_id,
            active_character_id,
            decorative_font.clone(),
            portrait_textures,
        );

        let character_sheet_toggle = CharacterSheetToggle {
            shown: Cell::new(false),
            text_line: TextLine::new("Character sheet", 18, WHITE, Some(simple_font.clone())),
            padding: 10.0,
        };

        let character_portraits = CharacterPortraits::new(
            &game.characters,
            game.active_character_id,
            simple_font.clone(),
            //decorative_font.clone(),
        );

        let target_ui = TargetUi::new(big_font.clone(), simple_font.clone());

        let activity_popup = ActivityPopup::new(
            simple_font.clone(),
            ui_state,
            icons,
            characters.clone(),
            active_character_id,
        );

        Self {
            game_grid,
            characters,
            character_portraits,
            player_portraits,
            character_sheet_toggle,
            active_character_id,
            animation_stopwatch: StopWatch::default(),

            font: simple_font.clone(),

            hovered_button: None,
            log: Log::new(simple_font.clone()),
            character_uis,
            event_queue: Rc::clone(&event_queue),
            activity_popup,
            target_ui,
            state: ui_state,
        }
    }

    pub fn draw(&mut self) {
        let ui_y = screen_height() - 160.0;

        let popup_rectangle = self.activity_popup.last_drawn_rectangle;

        let mouse_pos = mouse_position();
        let is_grid_obstructed = popup_rectangle.contains(mouse_pos.into())
            || self.character_sheet_toggle.shown.get()
            || mouse_pos.1 >= ui_y - 1.0;
        let is_grid_receptive_to_input = !matches!(self.state, UiState::Idle)
            && self.active_character().player_controlled
            && !is_grid_obstructed;
        let is_grid_receptive_to_dragging = !is_grid_obstructed;
        let grid_outcome = self.game_grid.draw(
            is_grid_receptive_to_input,
            is_grid_receptive_to_dragging,
            &self.state,
        );

        self.handle_grid_outcome(grid_outcome);

        draw_rectangle(0.0, ui_y, screen_width(), screen_height() - ui_y, BLACK);
        draw_line(0.0, ui_y, screen_width(), ui_y, 1.0, ORANGE);

        self.activity_popup.draw(20.0, ui_y + 1.0);

        self.player_portraits.draw(570.0, ui_y + 5.0);
        self.character_sheet_toggle.draw(570.0, ui_y + 90.0);

        let character_ui = self
            .character_uis
            .get_mut(&self.player_portraits.selected_i.get())
            .unwrap();

        character_ui.actions_section.draw(20.0, ui_y + 10.0);
        character_ui.action_points_row.draw(430.0, ui_y + 5.0);
        character_ui.resource_bars.draw(400.0, ui_y + 40.0);

        self.log.draw(800.0, ui_y);

        // We draw this late to ensure that any hover popups are shown above other UI elements
        character_ui.conditions_list.draw(620.0, ui_y + 100.0);

        self.character_portraits.draw(10.0, 10.0);

        self.target_ui
            .draw(screen_width() - self.target_ui.size().0 - 10.0, 10.0);

        if self.character_sheet_toggle.shown.get() {
            let clicked_close = character_ui.character_sheet.draw();
            self.character_sheet_toggle.shown.set(!clicked_close);
        }

        if let Some((btn_id, _btn_action, btn_pos)) = self.hovered_button {
            let btn = character_ui
                .hoverable_buttons
                .iter()
                .find(|btn| btn.id == btn_id)
                .unwrap();

            draw_button_tooltip(&self.font, btn_pos, &btn.tooltip);
        }
    }

    fn handle_grid_outcome(&mut self, outcome: GridOutcome) {
        self.character_portraits
            .set_hovered_character_id(outcome.hovered_character_id);

        if let Some(new_inspect_target) = outcome.switched_inspect_target {
            dbg!(new_inspect_target);
            let target = new_inspect_target.map(|id| self.characters.get(id));
            self.target_ui.set_character(target);
        }

        if let Some(grid_switched_to) = outcome.switched_action {
            dbg!(&grid_switched_to);

            match grid_switched_to {
                GridSwitchedTo::Move { ap_cost } => {
                    self.set_state(UiState::ConfiguringAction(BaseAction::Move {}));
                    self.activity_popup.set_movement_ap_cost(ap_cost);
                }
                GridSwitchedTo::Attack => {
                    let hand = HandType::MainHand;
                    let action_point_cost = self.active_character().attack_action_point_cost(hand);
                    self.set_state(UiState::ConfiguringAction(BaseAction::Attack {
                        hand,
                        action_point_cost,
                    }));
                }
                GridSwitchedTo::Idle => {
                    self.set_state(UiState::ChoosingAction);
                }
            }
        }
    }

    fn set_allowed_to_use_action_buttons(&self, allowed: bool) {
        for btn in self.character_uis[&self.player_portraits.selected_i.get()]
            .tracked_action_buttons
            .values()
        {
            if allowed {
                let enabled = match btn.action {
                    ButtonAction::Action(base_action) => {
                        self.active_character().can_use_action(base_action)
                    }
                    _ => unreachable!(),
                };
                btn.enabled.set(enabled);
            } else {
                btn.enabled.set(false);
            }
        }
    }

    fn active_character(&self) -> &Character {
        self.characters.get(self.active_character_id)
    }

    fn update_selected_action_button(&mut self) {
        if let UiState::ConfiguringAction(base_action) = self.state {
            let mut action_is_waiting_for_target_selection = false;

            match base_action {
                BaseAction::Attack { .. } => {
                    action_is_waiting_for_target_selection =
                        matches!(self.game_grid.players_action_target(), ActionTarget::None);
                }
                BaseAction::CastSpell(spell) => {
                    if matches!(
                        spell.target,
                        SpellTarget::Enemy { .. }
                            | SpellTarget::Ally { .. }
                            | SpellTarget::Area { .. }
                    ) {
                        action_is_waiting_for_target_selection =
                            matches!(self.game_grid.players_action_target(), ActionTarget::None);
                    }
                }
                BaseAction::Move => {}
            }

            let fully_selected = !action_is_waiting_for_target_selection;
            self.set_selected_action(Some((ButtonAction::Action(base_action), fully_selected)));
        }
    }

    fn set_selected_action(&self, selected_action: Option<(ButtonAction, bool)>) {
        let mut selected_id = None;
        let mut fully_selected = false;
        if let Some((action, fully)) = selected_action {
            selected_id = Some(button_action_id(action));
            fully_selected = fully;
        }

        if self.active_character().player_controlled {
            if self.player_portraits.selected_i.get() != self.active_character_id {
                self.player_portraits
                    .set_selected_character(self.active_character_id);
            }

            for (btn_action_id, btn) in
                &self.character_uis[&self.active_character_id].tracked_action_buttons
            {
                if selected_id.as_ref() == Some(btn_action_id) {
                    if fully_selected {
                        btn.selected.set(ButtonSelected::Yes);
                    } else {
                        btn.selected.set(ButtonSelected::Partially);
                    }
                } else {
                    btn.selected.set(ButtonSelected::No);
                }
            }
        }
    }

    pub fn set_state(&mut self, state: UiState) {
        if self.state == state {
            return;
        }

        dbg!(&state);

        self.state = state;

        self.activity_popup.additional_line = None;

        let mut movement = false;
        let mut npc_action_has_target = false;
        let mut relevant_action_button = None;

        match state {
            UiState::ConfiguringAction(base_action) => {
                self.set_allowed_to_use_action_buttons(true);

                relevant_action_button = self.character_uis[&self.active_character_id]
                    .tracked_action_buttons
                    .get(&button_action_id(ButtonAction::Action(base_action)))
                    .cloned();

                match base_action {
                    BaseAction::Attack { .. } => {}
                    BaseAction::CastSpell(..) => {}
                    BaseAction::Move => {
                        movement = true;
                    }
                }
            }

            UiState::ReactingToAttack { .. } => {
                self.set_allowed_to_use_action_buttons(false);
                self.on_selected_attacked_reaction(None);
                npc_action_has_target = true;
            }

            UiState::ReactingToHit { .. } => {
                self.set_allowed_to_use_action_buttons(false);
            }

            UiState::ChoosingAction => {
                self.set_allowed_to_use_action_buttons(true);
                self.set_selected_action(None);
            }

            UiState::Idle => {
                self.set_allowed_to_use_action_buttons(false);
                self.game_grid.clear_players_action_target();
            }
        }

        self.activity_popup
            .set_state(self.active_character_id, state, relevant_action_button);

        let active_char = self.active_character();
        let speed = active_char.move_speed;
        let ap = active_char.action_points.current();
        let sta = active_char.stamina.current();
        let max_move_range = ap as f32 * speed + sta.min(ap) as f32 * speed;

        self.game_grid
            .set_move_speed_and_range(speed, max_move_range);

        if !movement {
            self.game_grid.remove_movement_path();
        }

        if !npc_action_has_target {
            self.game_grid.clear_enemys_target();
        }

        self.update_selected_action_button();
    }

    fn on_selected_attacked_reaction(&mut self, reaction: Option<OnAttackedReaction>) {
        let UiState::ReactingToAttack {
            hand,
            attacker,
            reactor,
            is_within_melee: _,
        } = self.state
        else {
            unreachable!()
        };

        let attacker = self.characters.get(attacker);
        let defender = self.characters.get(reactor);

        // TODO
        let attack_enhancements = &[];

        let mut explanation = String::new();

        for (term, _bonus) in attacker.outgoing_attack_bonuses(hand, attack_enhancements) {
            explanation.push_str(term);
            explanation.push(' ');
        }
        for (term, _bonus) in defender.incoming_attack_bonuses(reaction) {
            explanation.push_str(term);
            explanation.push(' ');
        }

        let mut line = format!(
            "Hit chance: {}",
            as_percentage(prob_attack_hit(
                attacker,
                hand,
                defender,
                0,
                attack_enhancements,
                reaction
            ))
        );
        if !explanation.is_empty() {
            line.push_str(&format!("  {explanation}"));
        }
        self.activity_popup.additional_line = Some(line);
        self.game_grid.set_enemys_target(reactor);
    }

    pub fn has_ongoing_animation(&self) -> bool {
        self.animation_stopwatch.remaining.is_some()
    }

    pub fn handle_game_event(&mut self, event: GameEvent) {
        dbg!(&event);
        match event {
            GameEvent::LogLine(line) => {
                self.log.add(line);
            }

            GameEvent::CharacterReactedToHit {
                main_line,
                detail_lines,
                reactor,
                outcome,
            } => {
                self.log.add_with_details(main_line, detail_lines);

                let reactor_pos = self.characters.get(reactor).pos();

                if let Some(condition) = outcome.received_condition {
                    self.game_grid.add_text_effect(
                        reactor_pos,
                        0.0,
                        1.0,
                        format!("{:?}", condition),
                        Goodness::Neutral,
                    );
                }

                let attacker_pos = self.active_character().pos();
                if let Some(offensive) = outcome.offensive {
                    if let Some(condition) = offensive.inflicted_condition {
                        self.game_grid.add_text_effect(
                            attacker_pos,
                            0.0,
                            1.0,
                            format!("{:?}", condition),
                            Goodness::Neutral,
                        );
                    } else {
                        self.game_grid.add_text_effect(
                            attacker_pos,
                            0.0,
                            1.0,
                            "Miss".to_string(),
                            Goodness::Neutral,
                        );
                    }
                }
                self.animation_stopwatch.set_to_at_least(0.5);
            }

            GameEvent::Attacked {
                attacker,
                target,
                outcome,
                detail_lines,
            } => {
                let mut line = format!(
                    "{} attacked {}",
                    self.characters.get(attacker).name,
                    self.characters.get(target).name
                );

                match outcome {
                    AttackOutcome::Hit(dmg) => line.push_str(&format!(" ({} damage)", dmg)),
                    AttackOutcome::Dodge => line.push_str(" (dodge)"),
                    AttackOutcome::Parry => line.push_str(" (parry)"),
                    AttackOutcome::Miss => line.push_str(" (miss)"),
                }

                self.log.add_with_details(line, detail_lines);

                let attacker_pos = self.characters.get(attacker).pos();
                let target_pos = self.characters.get(target).pos();

                let duration = 0.15 * distance_between(attacker_pos, target_pos);

                self.animation_stopwatch.set_to_at_least(duration + 0.4);
                let impact_text = match outcome {
                    AttackOutcome::Hit(damage) => format!("{}", damage),
                    AttackOutcome::Dodge => "Dodge".to_string(),
                    AttackOutcome::Parry => "Parry".to_string(),
                    AttackOutcome::Miss => "Miss".to_string(),
                };
                let goodness = if matches!(outcome, AttackOutcome::Hit(..)) {
                    Goodness::Bad
                } else {
                    Goodness::Neutral
                };

                self.game_grid.add_effect(
                    attacker_pos,
                    target_pos,
                    Effect {
                        start_time: 0.0,
                        end_time: duration,
                        variant: EffectVariant::Line {
                            thickness: 1.0,
                            end_thickness: Some(10.0),
                            color: RED,
                            extend_gradually: true,
                        },
                    },
                );
                self.game_grid.add_effect(
                    attacker_pos,
                    target_pos,
                    Effect {
                        start_time: duration,
                        end_time: duration + 0.2,
                        variant: EffectVariant::At(
                            EffectPosition::Destination,
                            EffectGraphics::Circle {
                                radius: 25.0,
                                end_radius: Some(5.0),
                                fill: None,
                                stroke: Some((MAGENTA, 2.0)),
                            },
                        ),
                    },
                );

                self.game_grid
                    .add_text_effect(target_pos, duration, 0.5, impact_text, goodness);
            }

            GameEvent::SpellWasCast {
                caster,
                target_outcome,
                area_outcomes,
                spell,
                detail_lines,
            } => {
                let mut line = if let Some((target_id, _outcome)) = target_outcome {
                    format!(
                        "{} cast {} on {}",
                        self.characters.get(caster).name,
                        spell.name,
                        self.characters.get(target_id).name
                    )
                } else {
                    format!("{} cast {}", self.characters.get(caster).name, spell.name,)
                };

                if let Some((_target_id, outcome)) = target_outcome {
                    match outcome {
                        SpellTargetOutcome::HitEnemy { damage } => {
                            if let Some(dmg) = damage {
                                line.push_str(&format!(" ({} damage)", dmg))
                            } else {
                                line.push_str(" (hit)");
                            }
                        }
                        SpellTargetOutcome::Resist => line.push_str(" (miss)"),
                        SpellTargetOutcome::HealedAlly(healing) => {
                            line.push_str(&format!(" ({} healing)", healing))
                        }
                    }
                }

                self.log.add_with_details(line, detail_lines);

                let mut duration = 0.0;

                let animation_color = spell.animation_color;
                if let Some((target, outcome)) = target_outcome {
                    let caster_pos = self.characters.get(caster).pos();
                    let target_pos = self.characters.get(target).pos();

                    duration = 0.15 * distance_between(caster_pos, target_pos);

                    self.game_grid.add_effect(
                        caster_pos,
                        target_pos,
                        Effect {
                            start_time: 0.0,
                            end_time: duration,
                            variant: EffectVariant::At(
                                EffectPosition::Projectile,
                                EffectGraphics::Circle {
                                    radius: 10.0,
                                    end_radius: Some(15.0),
                                    fill: Some(animation_color),
                                    stroke: None,
                                },
                            ),
                        },
                    );
                    self.game_grid.add_effect(
                        caster_pos,
                        target_pos,
                        Effect {
                            start_time: 0.025,
                            end_time: duration + 0.025,
                            variant: EffectVariant::At(
                                EffectPosition::Projectile,
                                EffectGraphics::Circle {
                                    radius: 8.0,
                                    end_radius: Some(13.0),
                                    fill: Some(animation_color),
                                    stroke: None,
                                },
                            ),
                        },
                    );
                    self.game_grid.add_effect(
                        caster_pos,
                        target_pos,
                        Effect {
                            start_time: 0.05,
                            end_time: duration + 0.05,
                            variant: EffectVariant::At(
                                EffectPosition::Projectile,
                                EffectGraphics::Circle {
                                    radius: 6.0,
                                    end_radius: Some(11.0),
                                    fill: Some(animation_color),
                                    stroke: None,
                                },
                            ),
                        },
                    );

                    let (target_text, goodness) = match outcome {
                        SpellTargetOutcome::HitEnemy { damage } => {
                            if let Some(dmg) = damage {
                                (format!("{}", dmg), Goodness::Bad)
                            } else {
                                ("Hit".to_string(), Goodness::Bad)
                            }
                        }
                        SpellTargetOutcome::Resist => ("Resist".to_string(), Goodness::Neutral),
                        SpellTargetOutcome::HealedAlly(healing) => {
                            (format!("{}", healing), Goodness::Good)
                        }
                    };

                    self.game_grid.add_text_effect(
                        target_pos,
                        duration,
                        0.5,
                        target_text,
                        goodness,
                    );

                    self.animation_stopwatch.set_to_at_least(duration + 0.3);
                }

                if let Some((area_center_pos, outcomes)) = area_outcomes {
                    let area_duration = 0.2;

                    for (target_id, outcome) in outcomes {
                        let target_pos = self.characters.get(target_id).pos();

                        self.game_grid.add_effect(
                            (area_center_pos.0, area_center_pos.1),
                            target_pos,
                            Effect {
                                start_time: duration,
                                end_time: duration + area_duration,
                                variant: EffectVariant::At(
                                    EffectPosition::Destination,
                                    EffectGraphics::Circle {
                                        radius: 20.0,
                                        stroke: Some((animation_color, 4.0)),
                                        end_radius: Some(25.0),
                                        fill: None,
                                    },
                                ),
                            },
                        );

                        let (target_text, goodness) = match outcome {
                            SpellTargetOutcome::HitEnemy { damage } => {
                                if let Some(dmg) = damage {
                                    (format!("{}", dmg), Goodness::Bad)
                                } else {
                                    ("Hit".to_string(), Goodness::Bad)
                                }
                            }
                            SpellTargetOutcome::Resist => ("Resist".to_string(), Goodness::Neutral),
                            SpellTargetOutcome::HealedAlly(healing) => {
                                (format!("{}", healing), Goodness::Good)
                            }
                        };

                        self.game_grid.add_text_effect(
                            target_pos,
                            duration,
                            0.5,
                            target_text,
                            goodness,
                        );

                        self.animation_stopwatch.set_to_at_least(duration + 0.3);
                    }
                }
            }

            GameEvent::CharacterDied { character } => {
                self.log
                    .add(format!("{} died", self.characters.get(character).name));

                self.characters.remove_dead();
                self.game_grid.remove_dead();
                self.character_portraits.remove_dead();

                // TODO If player was inspecting this target, clear it (or we panic on the next draw)
                //self.target_ui. ...
            }
            GameEvent::Moved {
                character,
                from,
                to,
            } => {
                let mut duration = 0.4;
                if from.0 != to.0 || from.1 != to.1 {
                    // diagonal takes longer
                    duration *= 1.41;
                }

                self.game_grid
                    .set_character_motion(character, from, to, duration);
                self.animation_stopwatch.set_to_at_least(duration);
            }
        }
    }

    pub fn update(&mut self, game: &CoreGame, elapsed: f32) -> Option<PlayerChose> {
        let active_character_id = game.active_character_id;

        if active_character_id != self.active_character_id {
            // When control switches to a new player controlled character, make the UI show that character
            if self.characters.get(active_character_id).player_controlled {
                self.player_portraits
                    .set_selected_character(active_character_id);
            }
        }

        self.active_character_id = active_character_id;

        self.set_allowed_to_use_action_buttons(
            self.player_portraits.selected_i.get() == active_character_id,
        );

        self.game_grid.update(
            active_character_id,
            self.player_portraits.selected_i.get(),
            &self.characters,
            elapsed,
        );

        let popup_outcome = self.activity_popup.update();

        let mut player_choice = None;
        match popup_outcome {
            Some(ActivityPopupOutcome::ClickedProceed) => {
                player_choice = Some(self.handle_popup_proceed());
            }
            Some(ActivityPopupOutcome::ChangedOnAttackedReaction(reaction)) => {
                self.on_selected_attacked_reaction(reaction);
            }
            None => {}
        }

        self.event_queue
            .take()
            .into_iter()
            .for_each(|event| self.handle_internal_ui_event(event));

        let mut popup_enabled = true;

        self.game_grid.range_indicator = None;

        match self.state {
            UiState::ConfiguringAction(base_action @ BaseAction::Attack { hand, .. }) => {
                popup_enabled = false; // until proven otherwise

                match self.game_grid.players_action_target() {
                    ActionTarget::Character(target_id) => {
                        let target_char = self.characters.get(target_id);

                        let enhancements: Vec<AttackEnhancement> =
                            self.activity_popup.selected_attack_enhancements();

                        let (range, reach) = self
                            .active_character()
                            .reaches_with_attack(hand, target_char.position.get());

                        let mut circumstance_advantage = None;

                        let maybe_indicator;
                        match reach {
                            ActionReach::Yes | ActionReach::YesButDisadvantage(..) => {
                                if self.active_character().can_use_action(base_action) {
                                    popup_enabled = true;
                                }
                                if let ActionReach::YesButDisadvantage(reason) = reach {
                                    circumstance_advantage = Some((-1, reason, Goodness::Bad));
                                    maybe_indicator = Some(RangeIndicator::CanReachButDisadvantage);
                                } else {
                                    maybe_indicator = None;
                                }
                            }
                            ActionReach::No => {
                                maybe_indicator = Some(RangeIndicator::CannotReach);
                            }
                        }

                        // We cannot know yet if the defender will react
                        let defender_reaction = None;

                        let chance = as_percentage(prob_attack_hit(
                            self.active_character(),
                            hand,
                            target_char,
                            circumstance_advantage.map(|entry| entry.0).unwrap_or(0),
                            &enhancements,
                            defender_reaction,
                        ));

                        let mut details = vec![];

                        for (term, bonus) in self
                            .active_character()
                            .outgoing_attack_bonuses(hand, &enhancements)
                        {
                            details.push((term.to_string(), bonus.goodness()));
                        }
                        for (term, bonus) in target_char.incoming_attack_bonuses(defender_reaction)
                        {
                            details.push((term.to_string(), bonus.goodness()));
                        }

                        if let Some((_advantage, term, goodness)) = circumstance_advantage {
                            details.push((term.to_string(), goodness));
                        }

                        self.target_ui
                            .set_action(format!("Attack: {}", chance), details, true);

                        self.game_grid.range_indicator =
                            maybe_indicator.map(|indicator| (range, indicator));
                    }

                    ActionTarget::None => {
                        self.target_ui
                            .set_action("Select an enemy".to_string(), vec![], false);

                        let range = self
                            .active_character()
                            .weapon(hand)
                            .unwrap()
                            .range
                            .into_range();
                        self.game_grid.range_indicator =
                            Some((range, RangeIndicator::ActionTargetRange));
                    }

                    ActionTarget::Position(_) => unreachable!(),
                }
            }

            UiState::ConfiguringAction(BaseAction::CastSpell(spell)) => {
                popup_enabled = false; // until proven otherwise

                let enhancements = self.activity_popup.selected_spell_enhancements();

                match self.game_grid.players_action_target() {
                    ActionTarget::Character(target_id) => {
                        let target_char = self.characters.get(target_id);

                        let action_text = match spell.target {
                            SpellTarget::Enemy { effect, .. } => {
                                let prob = effect
                                    .contest_type
                                    .map(|contest| {
                                        prob_spell_hit(
                                            self.active_character(),
                                            contest,
                                            target_char,
                                        )
                                    })
                                    .unwrap_or(1.0);
                                let chance = as_percentage(prob);

                                format!("{}: {}", spell.name, chance)
                            }
                            SpellTarget::Ally { .. } => spell.name.to_string(),
                            SpellTarget::None { .. } | SpellTarget::Area { .. } => {
                                unreachable!()
                            }
                        };

                        self.target_ui.set_action(action_text, vec![], true);

                        let maybe_indicator = if self.active_character().can_reach_with_spell(
                            spell,
                            &enhancements,
                            target_char.position.get(),
                        ) {
                            popup_enabled = true;
                            None
                        } else {
                            Some(RangeIndicator::CannotReach)
                        };
                        self.game_grid.range_indicator = maybe_indicator.map(|indicator| {
                            (spell.target.range(&enhancements).unwrap(), indicator)
                        });
                    }

                    ActionTarget::Position(target_pos) => {
                        assert!(matches!(spell.target, SpellTarget::Area { .. }));

                        self.target_ui
                            .set_action(format!("{} (AoE)", spell.name), vec![], false);

                        let maybe_indicator = if self.active_character().can_reach_with_spell(
                            spell,
                            &enhancements,
                            target_pos,
                        ) {
                            popup_enabled = true;
                            None
                        } else {
                            Some(RangeIndicator::CannotReach)
                        };
                        self.game_grid.range_indicator = maybe_indicator.map(|indicator| {
                            (spell.target.range(&enhancements).unwrap(), indicator)
                        });
                    }

                    ActionTarget::None => {
                        match spell.target {
                            SpellTarget::Enemy { .. } => {
                                self.target_ui.set_action(
                                    "Select an enemy".to_string(),
                                    vec![],
                                    false,
                                );
                            }

                            SpellTarget::Ally { .. } => {
                                self.target_ui.set_action(
                                    "Select an ally".to_string(),
                                    vec![],
                                    false,
                                );
                            }

                            SpellTarget::None { .. } => {
                                let header = spell.name.to_string();
                                self.target_ui.set_action(header, vec![], false);
                                popup_enabled = true;
                            }

                            SpellTarget::Area { .. } => {
                                self.target_ui.set_action(
                                    "Select an area".to_string(),
                                    vec![],
                                    false,
                                );
                                popup_enabled = true;
                            }
                        };

                        if let Some(range) = spell.target.range(&enhancements) {
                            self.game_grid.range_indicator =
                                Some((range, RangeIndicator::ActionTargetRange));
                        } else {
                            self.game_grid.range_indicator = None;
                        }
                    }
                }
            }

            UiState::ConfiguringAction(BaseAction::Move) => {
                if self.game_grid.has_non_empty_selected_movement_path() {
                    popup_enabled = true;
                    self.target_ui.clear_action();
                } else {
                    popup_enabled = false;
                    self.target_ui
                        .set_action("Select a destination".to_string(), vec![], false);
                }
            }

            UiState::ChoosingAction => {
                self.target_ui
                    .set_action("Select an action".to_string(), vec![], false);
            }

            UiState::Idle => {
                self.target_ui.clear_action();
            }

            UiState::ReactingToAttack { .. } | UiState::ReactingToHit { .. } => {
                self.target_ui
                    .set_action("Select a reaction".to_string(), vec![], false);
            }
        }

        self.activity_popup.set_enabled(popup_enabled);

        self.character_portraits.update(game);
        self.player_portraits.update(game);

        self.update_selected_action_button();

        // TODO Update the stats / condition of shown character in target UI (so that it doesn't only refresh when changing inspection target)

        self.update_character_status(&game.characters);

        let character_ui = self
            .character_uis
            .get_mut(&self.player_portraits.selected_i.get())
            .unwrap();
        if let Some(hovered_btn) = self.hovered_button {
            character_ui.action_points_row.reserved_and_hovered_ap = (
                self.activity_popup.reserved_and_hovered_action_points().0,
                hovered_btn.1.action_point_cost(),
            );
            character_ui
                .mana_bar
                .borrow_mut()
                .set_reserved(hovered_btn.1.mana_cost());
            character_ui
                .stamina_bar
                .borrow_mut()
                .set_reserved(hovered_btn.1.stamina_cost());
        } else {
            character_ui.action_points_row.reserved_and_hovered_ap =
                self.activity_popup.reserved_and_hovered_action_points();
            character_ui
                .mana_bar
                .borrow_mut()
                .set_reserved(self.activity_popup.mana_points());
            character_ui
                .stamina_bar
                .borrow_mut()
                .set_reserved(self.activity_popup.stamina_points());
        };

        self.animation_stopwatch.update(elapsed);

        player_choice
    }

    fn handle_popup_proceed(&mut self) -> PlayerChose {
        // Action button is highlighted while the action is being configured in the popup. That should be cleared now.
        self.set_selected_action(None);

        match self.state {
            UiState::ConfiguringAction(base_action) => {
                let target = self.game_grid.players_action_target();
                let action = match base_action {
                    BaseAction::Attack { hand, .. } => {
                        let ActionTarget::Character(target_id) = target else {
                            unreachable!();
                        };
                        Action::Attack {
                            hand,
                            enhancements: self.activity_popup.selected_attack_enhancements(),
                            target: target_id,
                        }
                    }
                    BaseAction::CastSpell(spell) => Action::CastSpell {
                        spell,
                        enhancements: self.activity_popup.selected_spell_enhancements(),
                        target,
                    },
                    BaseAction::Move => Action::Move {
                        action_point_cost: self.activity_popup.movement_ap_cost(),
                        stamina_cost: self.activity_popup.movement_stamina_cost(),
                        positions: self.game_grid.take_movement_path(),
                    },
                };
                PlayerChose::Action(action)
            }

            UiState::ReactingToAttack { .. } => {
                PlayerChose::AttackedReaction(self.activity_popup.selected_on_attacked_reaction())
            }

            UiState::ReactingToHit { .. } => {
                PlayerChose::HitReaction(self.activity_popup.selected_on_hit_reaction())
            }

            UiState::ChoosingAction | UiState::Idle => unreachable!(),
        }
    }

    fn handle_internal_ui_event(&mut self, event: InternalUiEvent) {
        match event {
            InternalUiEvent::ButtonHovered(button_id, button_action, hovered) => {
                if let Some(pos) = hovered {
                    self.hovered_button = Some((button_id, button_action, pos));
                } else if let Some(previously_hovered_button) = self.hovered_button {
                    if button_id == previously_hovered_button.0 {
                        self.hovered_button = None
                    }
                }
            }

            InternalUiEvent::ButtonClicked(_button_id, btn_action) => match btn_action {
                ButtonAction::Action(base_action) => {
                    let may_choose_action = matches!(
                        self.state,
                        UiState::ChoosingAction | UiState::ConfiguringAction(..)
                    );

                    if may_choose_action && self.active_character().can_use_action(base_action) {
                        //self.target_ui.set_character(Option::<&Character>::None);
                        self.game_grid.clear_players_action_target();

                        self.set_state(UiState::ConfiguringAction(base_action));
                    } else {
                        println!("Cannot choose this action at this time");
                    }
                }

                _ => unreachable!(),
            },
        }
    }

    fn update_character_status(&mut self, characters: &Characters) {
        for (id, character) in characters.iter_with_ids() {
            if let Some(ui) = self.character_uis.get_mut(id) {
                ui.health_bar
                    .borrow_mut()
                    .set_current(character.health.current());
                ui.mana_bar
                    .borrow_mut()
                    .set_current(character.mana.current());
                ui.stamina_bar
                    .borrow_mut()
                    .set_current(character.stamina.current());

                ui.conditions_list.descriptions = character.condition_infos();

                ui.action_points_row.current_ap = self
                    .characters
                    .get(self.player_portraits.selected_i.get())
                    .action_points
                    .current();
                ui.action_points_row.is_characters_turn = *id == self.active_character_id;
            }
        }
    }
}

fn button_action_id(btn_action: ButtonAction) -> String {
    match btn_action {
        ButtonAction::Action(base_action) => match base_action {
            BaseAction::Attack { hand, .. } => format!("ATTACK_{:?}", hand),
            BaseAction::CastSpell(spell) => format!("SPELL_{}", spell.name),
            BaseAction::Move => "MOVE".to_string(),
        },
        _ => unreachable!(),
    }
}

fn buttons_row(buttons: Vec<Element>) -> Element {
    Element::Container(Container {
        layout_dir: LayoutDirection::Horizontal,
        margin: 5.0,
        children: buttons,
        ..Default::default()
    })
}

#[derive(Debug)]
pub enum PlayerChose {
    AttackedReaction(Option<OnAttackedReaction>),
    HitReaction(Option<OnHitReaction>),
    Action(Action),
}
