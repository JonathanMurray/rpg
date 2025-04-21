use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use macroquad::{color::SKYBLUE, rand};

use indexmap::IndexMap;
use macroquad::{
    color::{
        Color, BLACK, BLUE, DARKGRAY, GOLD, GRAY, GREEN, LIGHTGRAY, MAGENTA, ORANGE, RED, WHITE,
    },
    input::{is_mouse_button_pressed, mouse_position, MouseButton},
    shapes::{draw_circle, draw_circle_lines, draw_line, draw_rectangle, draw_rectangle_lines},
    text::Font,
    texture::Texture2D,
    window::{screen_height, screen_width},
};

use crate::{
    action_button::{draw_button_tooltip, ActionButton, ButtonAction, InternalUiEvent},
    activity_popup::{ActivityPopup, ActivityPopupOutcome},
    base_ui::{
        Align, Container, ContainerScroll, Drawable, Element, LayoutDirection, Style, TextLine,
    },
    character_sheet::CharacterSheet,
    conditions_ui::ConditionsList,
    core::{
        as_percentage, distance_between, prob_attack_hit, prob_spell_hit, Action, ActionReach,
        ActionTarget, AttackEnhancement, AttackOutcome, BaseAction, Character, CharacterId,
        Characters, CoreGame, GameEvent, GameEventHandler, Goodness, HandType, MovementEnhancement,
        OnAttackedReaction, OnHitReaction, SpellTargetOutcome, SpellTargetType, MAX_ACTION_POINTS,
        MOVE_ACTION_COST,
    },
    grid::{
        Effect, EffectGraphics, EffectPosition, EffectVariant, GameGrid, GridSwitchedTo,
        RangeIndicator,
    },
    target_ui::TargetUi,
    textures::{EquipmentIconId, IconId, SpriteId},
};

const Y_USER_INTERFACE: f32 = 800.0;

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

    icons: HashMap<IconId, Texture2D>,

    hovered_button: Option<(u32, ButtonAction, (f32, f32))>,
    next_available_button_id: u32,
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

        for character in game.characters.iter() {
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
                    BaseAction::SelfEffect(..) => basic_buttons.push(btn),
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
                    BaseAction::Move { .. } => {
                        basic_buttons.push(btn);
                    }
                }
            }

            let basic_row = buttons_row(
                basic_buttons
                    .into_iter()
                    .map(|btn| Element::Rc(btn))
                    .collect(),
            );
            let spell_row = buttons_row(
                spell_buttons
                    .into_iter()
                    .map(|btn| Element::Rc(btn))
                    .collect(),
            );

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

            let actions_section = Container {
                layout_dir: LayoutDirection::Vertical,
                margin: 5.0,
                children: vec![basic_row, spell_row],
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

        let state = UiState::Idle;

        let grid_dimensions = (16, 12);
        let mut cell_backgrounds = vec![];
        for _ in 0..(grid_dimensions.0 * grid_dimensions.1) {
            let i = rand::gen_range(0, background_textures.len());
            cell_backgrounds.push(i);
        }

        let first_player_character_id = game
            .characters
            .iter_with_ids()
            .find(|(_id, ch)| ch.player_controlled)
            .unwrap()
            .0;

        let game_grid = GameGrid::new(
            first_player_character_id,
            &game.characters,
            sprites,
            (screen_width(), Y_USER_INTERFACE),
            big_font.clone(),
            simple_font.clone(),
            background_textures,
            grid_dimensions,
            cell_backgrounds,
        );

        let popup_proceed_btn = new_button(ButtonAction::Proceed, None, true);

        let player_portraits = PlayerPortraits::new(
            &game.characters,
            first_player_character_id,
            active_character_id,
            decorative_font.clone(),
        );

        let character_sheet_toggle = CharacterSheetToggle {
            shown: Cell::new(false),
            text_line: TextLine::new("Character sheet", 18, WHITE, Some(simple_font.clone())),
            padding: 10.0,
        };

        let character_portraits = CharacterPortraits::new(
            &game.characters,
            game.active_character_id,
            decorative_font.clone(),
        );

        let target_ui = TargetUi::new(big_font.clone(), simple_font.clone());

        Self {
            game_grid,
            characters,
            character_portraits,
            player_portraits,
            character_sheet_toggle,
            active_character_id,
            animation_stopwatch: StopWatch::default(),

            icons,
            font: simple_font.clone(),

            next_available_button_id: next_button_id,
            hovered_button: None,
            log: Log::new(simple_font.clone()),
            character_uis,
            event_queue: Rc::clone(&event_queue),
            activity_popup: ActivityPopup::new(simple_font, state, popup_proceed_btn),
            target_ui,
            state,
        }
    }

    fn new_button(&mut self, btn_action: ButtonAction) -> ActionButton {
        let btn = ActionButton::new(
            btn_action,
            &self.event_queue,
            self.next_available_button_id,
            &self.icons,
            None,
        );
        self.next_available_button_id += 1;
        btn
    }

    pub fn draw(&mut self) {
        let mut y = Y_USER_INTERFACE;

        let popup_rectangle = self.activity_popup.last_drawn_rectangle;

        self.game_grid.position_on_screen = (0.0, 0.0);

        let (mouse_x, mouse_y) = mouse_position();
        let grid_receptive_to_input = !matches!(self.state, UiState::Idle)
            && !self.character_sheet_toggle.shown.get()
            && self.active_character().player_controlled
            && !popup_rectangle.contains((mouse_x, mouse_y).into());

        let grid_outcome = self.game_grid.draw(grid_receptive_to_input, &self.state);

        draw_rectangle(0.0, y, screen_width(), screen_height() - y, BLACK);
        draw_line(0.0, y, screen_width(), y, 1.0, ORANGE);

        self.activity_popup.draw(20.0, y + 1.0);

        y -= 60.0;

        self.player_portraits.draw(620.0, y + 70.0);
        self.character_sheet_toggle.draw(620.0, y + 120.0);

        let character_ui = self
            .character_uis
            .get_mut(&self.player_portraits.selected_i.get())
            .unwrap();

        character_ui.actions_section.draw(20.0, y + 70.0);

        character_ui.action_points_row.draw(430.0, y + 70.0);
        character_ui.resource_bars.draw(400.0, y + 100.0);

        self.log.draw(800.0, y + 60.0);

        // We draw this late to ensure that any hover popups are shown above other UI elements
        self.draw_player_conditions(620.0, y + 160.0);

        self.character_portraits
            .set_hovered_character_id(grid_outcome.hovered_character_id);

        self.character_portraits.draw(10.0, 10.0);

        if let Some(new_inspect_target) = grid_outcome.switched_inspect_target {
            dbg!(new_inspect_target);
            self.update_shown_target(new_inspect_target);
        }

        self.target_ui
            .draw(1280.0 - self.target_ui.size().0 - 10.0, 10.0);

        let character_ui = self
            .character_uis
            .get_mut(&self.player_portraits.selected_i.get())
            .unwrap();
        if self.character_sheet_toggle.shown.get() {
            let clicked_close = character_ui.character_sheet.draw();
            self.character_sheet_toggle.shown.set(!clicked_close);
        }

        if let Some((btn_id, _btn_action, btn_pos)) = self.hovered_button {
            let btn = character_ui
                .hoverable_buttons
                .iter()
                .find(|btn| btn.id == btn_id)
                .expect("hovered button");

            draw_button_tooltip(&self.font, btn_pos, &btn.tooltip);
        }

        if let Some(grid_switched_to) = grid_outcome.switched_action {
            match grid_switched_to {
                GridSwitchedTo::Move { selected_option } => {
                    let move_range = self.active_character().move_range;
                    self.set_state(UiState::ConfiguringAction(BaseAction::Move {
                        action_point_cost: MOVE_ACTION_COST,
                        range: move_range,
                    }));

                    let selected_enhancement = if selected_option == 0 {
                        None
                    } else {
                        Some(selected_option - 1)
                    };
                    self.activity_popup
                        .select_movement_option(selected_enhancement);
                }
                GridSwitchedTo::Attack => {
                    let hand = HandType::MainHand;
                    let action_point_cost = self
                        .active_character()
                        .weapon(hand)
                        .unwrap()
                        .action_point_cost;
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

    fn update_shown_target(&mut self, shown_target_id: Option<CharacterId>) {
        let target = shown_target_id.map(|id| self.characters.get(id));
        self.target_ui.set_character(target);
    }

    fn draw_player_conditions(&self, x: f32, y: f32) {
        self.character_uis[&self.player_portraits.selected_i.get()]
            .conditions_list
            .draw(x, y);
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
                    _ => todo!(),
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

    pub fn set_state(&mut self, state: UiState) {
        if self.state == state {
            return;
        }

        dbg!(&state);

        self.state = state;

        self.activity_popup.additional_line = None;

        let mut popup_initial_lines = vec![];
        let mut popup_buttons = vec![];
        let mut movement = false;

        let mut player_wants_enemy_target = false;
        let mut player_wants_ally_target = false;
        let mut npc_action_has_target = false;

        match state {
            UiState::ConfiguringAction(base_action) => {
                self.set_allowed_to_use_action_buttons(true);

                let tooltip = &self.character_uis[&self.active_character_id].tracked_action_buttons
                    [&button_action_id(ButtonAction::Action(base_action))]
                    .tooltip;

                popup_initial_lines.push(tooltip.header.to_string());
                popup_initial_lines.extend_from_slice(&tooltip.technical_description);

                self.set_highlighted_action(Some(ButtonAction::Action(base_action)));

                match base_action {
                    BaseAction::Attack {
                        hand,
                        action_point_cost: _,
                    } => {
                        let enhancements = self.active_character().usable_attack_enhancements(hand);
                        for (_subtext, enhancement) in enhancements {
                            let btn = self.new_button(ButtonAction::AttackEnhancement(enhancement));
                            popup_buttons.push(btn);
                        }
                        player_wants_enemy_target = true;
                    }
                    BaseAction::SelfEffect(..) => {}
                    BaseAction::CastSpell(spell) => {
                        for enhancement in spell.possible_enhancements.iter().flatten().copied() {
                            if self
                                .active_character()
                                .can_use_spell_enhancement(spell, enhancement)
                            {
                                let btn_action = ButtonAction::SpellEnhancement(enhancement);
                                let btn = self.new_button(btn_action);
                                popup_buttons.push(btn);
                            }
                        }

                        match spell.target_type {
                            SpellTargetType::TargetEnemy { .. } => player_wants_enemy_target = true,
                            SpellTargetType::TargetAlly { .. } => player_wants_ally_target = true,
                            SpellTargetType::TargetArea { .. } => {}
                            SpellTargetType::NoTarget { .. } => {}
                        }
                    }
                    BaseAction::Move { .. } => {
                        let enhancements = self.active_character().usable_movement_enhancements();
                        for (_subtext, enhancement) in enhancements {
                            let btn =
                                self.new_button(ButtonAction::MovementEnhancement(enhancement));
                            popup_buttons.push(btn);
                        }
                        movement = true;
                    }
                }
            }

            UiState::ReactingToAttack {
                attacker: attacker_id,
                hand,
                reactor: reactor_id,
                is_within_melee,
            } => {
                self.set_allowed_to_use_action_buttons(false);

                let attacker = self.characters.get(attacker_id);
                let defender = self.characters.get(reactor_id);

                popup_initial_lines.push("React (on attacked)".to_string());
                let attacks_str = format!(
                    "{} attacks {} (d20+{} vs {})",
                    attacker.name,
                    defender.name,
                    attacker.attack_modifier(hand),
                    defender.evasion(),
                );
                popup_initial_lines.push(attacks_str);

                let reactions = defender.usable_on_attacked_reactions(is_within_melee);
                for (_subtext, reaction) in reactions {
                    let btn_action = ButtonAction::OnAttackedReaction(reaction);
                    let btn = self.new_button(btn_action);
                    popup_buttons.push(btn);
                }

                self.on_selected_attacked_reaction(None);
                npc_action_has_target = true;
            }

            UiState::ReactingToHit {
                attacker: attacker_id,
                damage,
                victim: victim_id,
                is_within_melee,
            } => {
                self.set_allowed_to_use_action_buttons(false);

                let victim = self.characters.get(victim_id);
                popup_initial_lines.push("React (on hit)".to_string());
                popup_initial_lines.push(format!(
                    "{} attacked {} for {} damage",
                    self.characters.get(attacker_id).name,
                    victim.name,
                    damage,
                ));
                let reactions = victim.usable_on_hit_reactions(is_within_melee);
                for (_subtext, reaction) in reactions {
                    let btn_action = ButtonAction::OnHitReaction(reaction);
                    let btn = self.new_button(btn_action);
                    popup_buttons.push(btn);
                }
            }

            UiState::ChoosingAction => {
                self.set_allowed_to_use_action_buttons(true);
                self.set_highlighted_action(None);
            }

            UiState::Idle => {
                self.set_allowed_to_use_action_buttons(false);
            }
        }

        self.activity_popup
            .set_state(state, popup_initial_lines, popup_buttons);

        let move_range = self.active_character().move_range;

        let move_enhancements: Vec<MovementEnhancement> = self
            .active_character()
            .usable_movement_enhancements()
            .into_iter()
            .map(|(_, enhancement)| enhancement)
            .collect();

        self.game_grid
            .set_movement_range_options(move_range, move_enhancements);

        if movement {
            //self.game_grid.ensure_has_some_movement_preview();
        } else {
            self.game_grid.remove_movement_preview();
        }

        if player_wants_enemy_target {
            self.game_grid.clear_players_target_if_allied();
            //self.game_grid.ensure_player_has_enemy_target();
        } else if player_wants_ally_target {
            self.game_grid.clear_players_action_target_if_enemy();
            //self.game_grid.ensure_player_has_ally_target();
        } else {
            self.game_grid.clear_players_action_target();
        }

        if !npc_action_has_target {
            self.game_grid.clear_enemys_target();
        }

        //self.update_shown_target();
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

        for (term, _goodness) in attacker.explain_attack_bonus(hand, attack_enhancements) {
            explanation.push_str(&term);
            explanation.push(' ');
        }
        for (term, _goodness) in defender.explain_incoming_attack_circumstances(reaction) {
            explanation.push_str(&term);
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
            GameEvent::CharacterReceivedSelfEffect {
                character,
                condition,
            } => {
                let pos = self.characters.get(character).position.get();
                let duration = 1.0;
                self.game_grid.add_text_effect(
                    (pos.0, pos.1),
                    0.0,
                    duration,
                    format!("{:?}", condition),
                    Goodness::Neutral,
                );
                self.animation_stopwatch.set_to_at_least(duration);
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
            Some(ActivityPopupOutcome::ChangedMovementRangePercentage(added_percentage)) => {
                self.game_grid
                    .set_selected_movement_percentage(added_percentage);
            }
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

                        let enhancements: Vec<AttackEnhancement> = self
                            .activity_popup
                            .selected_choices()
                            .map(|action| action.unwrap_attack_enhancement())
                            .collect();

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

                        for (term, goodness) in self
                            .active_character()
                            .explain_attack_bonus(hand, &enhancements)
                        {
                            details.push((term, goodness));
                        }
                        for (term, goodness) in
                            target_char.explain_incoming_attack_circumstances(defender_reaction)
                        {
                            details.push((term, goodness));
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
                            .set_action("Select a target".to_string(), vec![], false);

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

                match self.game_grid.players_action_target() {
                    ActionTarget::Character(target_id) => {
                        let target_char = self.characters.get(target_id);

                        let action_text = match spell.target_type {
                            SpellTargetType::TargetEnemy { effect, .. } => {
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
                            SpellTargetType::TargetAlly { .. } => spell.name.to_string(),
                            SpellTargetType::NoTarget { .. }
                            | SpellTargetType::TargetArea { .. } => {
                                unreachable!()
                            }
                        };

                        self.target_ui.set_action(action_text, vec![], true);

                        let maybe_indicator = if self
                            .active_character()
                            .can_reach_with_spell(spell, target_char.position.get())
                        {
                            popup_enabled = true;
                            None
                        } else {
                            Some(RangeIndicator::CannotReach)
                        };
                        self.game_grid.range_indicator = maybe_indicator
                            .map(|indicator| (spell.target_type.range().unwrap(), indicator));
                    }

                    ActionTarget::Position(target_pos) => {
                        assert!(matches!(
                            spell.target_type,
                            SpellTargetType::TargetArea { .. }
                        ));

                        self.target_ui
                            .set_action(format!("{} (AoE)", spell.name), vec![], false);

                        let maybe_indicator = if self
                            .active_character()
                            .can_reach_with_spell(spell, target_pos)
                        {
                            popup_enabled = true;
                            None
                        } else {
                            Some(RangeIndicator::CannotReach)
                        };
                        self.game_grid.range_indicator = maybe_indicator
                            .map(|indicator| (spell.target_type.range().unwrap(), indicator));
                    }

                    ActionTarget::None => {
                        match spell.target_type {
                            SpellTargetType::TargetEnemy { .. }
                            | SpellTargetType::TargetAlly { .. } => {
                                self.target_ui.set_action(
                                    "Select a target".to_string(),
                                    vec![],
                                    false,
                                );
                            }

                            SpellTargetType::NoTarget { .. } => {
                                let header = spell.name.to_string();
                                self.target_ui.set_action(header, vec![], false);
                                popup_enabled = true;
                            }

                            SpellTargetType::TargetArea { .. } => {
                                self.target_ui.set_action(
                                    "Select an area".to_string(),
                                    vec![],
                                    false,
                                );
                                popup_enabled = true;
                            }
                        };

                        if let Some(range) = spell.target_type.range() {
                            self.game_grid.range_indicator =
                                Some((range, RangeIndicator::ActionTargetRange));
                        } else {
                            self.game_grid.range_indicator = None;
                        }
                    }
                }
            }
            UiState::ConfiguringAction(BaseAction::Move { .. }) => {
                if self.game_grid.has_non_empty_movement_preview() {
                    popup_enabled = true;
                    self.target_ui.clear_action();
                } else {
                    popup_enabled = false;
                    self.target_ui
                        .set_action("Select a destination".to_string(), vec![], false);
                }
            }
            UiState::ConfiguringAction(BaseAction::SelfEffect(..)) => {
                popup_enabled = true;
                self.target_ui.clear_action();
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

        // TODO Update the stats / condition of shown character in target UI (so that it doesn't only refresh when changing inspection target)

        self.update_character_status(&game.characters);

        if let Some(hovered_btn) = self.hovered_button {
            self.character_uis
                .get_mut(&self.player_portraits.selected_i.get())
                .unwrap()
                .action_points_row
                .reserved_and_hovered_ap = (
                self.activity_popup.reserved_and_hovered_action_points().0,
                hovered_btn.1.action_point_cost(),
            );
            self.character_uis[&self.player_portraits.selected_i.get()]
                .mana_bar
                .borrow_mut()
                .set_reserved(hovered_btn.1.mana_cost());
            self.character_uis[&self.player_portraits.selected_i.get()]
                .stamina_bar
                .borrow_mut()
                .set_reserved(hovered_btn.1.stamina_cost());
        } else {
            self.character_uis
                .get_mut(&self.player_portraits.selected_i.get())
                .unwrap()
                .action_points_row
                .reserved_and_hovered_ap = self.activity_popup.reserved_and_hovered_action_points();
            self.character_uis[&self.player_portraits.selected_i.get()]
                .mana_bar
                .borrow_mut()
                .set_reserved(self.activity_popup.mana_points());
            self.character_uis[&self.player_portraits.selected_i.get()]
                .stamina_bar
                .borrow_mut()
                .set_reserved(self.activity_popup.stamina_points());
        };

        if self.animation_stopwatch.update(elapsed) {
            println!("UI is now ready...");
        }

        player_choice
    }

    fn handle_popup_proceed(&mut self) -> PlayerChose {
        // Action button is highlighted while the action is being configured in the popup. That should be cleared now.
        self.set_highlighted_action(None);

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
                    BaseAction::SelfEffect(sea) => Action::SelfEffect(sea),
                    BaseAction::CastSpell(spell) => Action::CastSpell {
                        spell,
                        enhancements: self.activity_popup.selected_spell_enhancements(),
                        target,
                    },
                    BaseAction::Move {
                        action_point_cost,
                        range: _,
                    } => Action::Move {
                        action_point_cost,
                        enhancements: self.activity_popup.selected_movement_enhancements(),
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

    fn set_highlighted_action(&self, highlighted_action: Option<ButtonAction>) {
        let highlighted_id = highlighted_action.map(button_action_id);

        if self.active_character().player_controlled {
            if self.player_portraits.selected_i.get() != self.active_character_id {
                self.player_portraits
                    .set_selected_character(self.active_character_id);
            }

            for (btn_action_id, btn) in
                &self.character_uis[&self.active_character_id].tracked_action_buttons
            {
                btn.highlighted
                    .set(highlighted_id.as_ref() == Some(btn_action_id));
            }
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
            BaseAction::SelfEffect(sea) => format!("SELF_EFFECT_{}", sea.name),
            BaseAction::CastSpell(spell) => format!("SPELL_{}", spell.name),
            BaseAction::Move { .. } => "MOVE".to_string(),
        },
        _ => unreachable!(),
    }
}

struct CharacterPortraits {
    row: Container,
    active_id: CharacterId,
    hovered_id: Option<CharacterId>,
    portraits: HashMap<CharacterId, Rc<RefCell<TopCharacterPortrait>>>,
}

impl CharacterPortraits {
    fn new(characters: &Characters, active_id: CharacterId, font: Font) -> Self {
        let mut portraits: HashMap<CharacterId, Rc<RefCell<TopCharacterPortrait>>> =
            Default::default();

        let mut elements = vec![];

        for (id, character) in characters.iter_with_ids() {
            let portrait = Rc::new(RefCell::new(TopCharacterPortrait::new(
                character,
                font.clone(),
            )));
            let cloned = Rc::downgrade(&portrait);
            portraits.insert(*id, portrait);
            elements.push(Element::WeakRefCell(cloned));
        }

        let row = Container {
            layout_dir: LayoutDirection::Horizontal,
            margin: 4.0,
            children: elements,
            style: Style {
                ..Default::default()
            },
            ..Default::default()
        };

        let mut this = Self {
            row,
            active_id,
            hovered_id: None,
            portraits,
        };

        this.set_active_character(active_id);
        this
    }

    fn set_active_character(&mut self, id: CharacterId) {
        if let Some(portrait) = self.portraits.get(&self.active_id) {
            // The entry may have been removed if the active character died during its turn
            portrait.borrow_mut().strong_highlight = false;
        }
        self.active_id = id;
        self.portraits[&self.active_id]
            .borrow_mut()
            .strong_highlight = true;
    }

    fn update(&mut self, game: &CoreGame) {
        self.set_active_character(game.active_character_id);
        for (id, character) in game.characters.iter_with_ids() {
            let portrait = self.portraits[id].borrow_mut();
            portrait.action_points_row.borrow_mut().current_ap = character.action_points.current();
            portrait.action_points_row.borrow_mut().is_characters_turn =
                *id == game.active_character_id;
            portrait.health_bar.borrow_mut().current = character.health.current();
        }
    }

    fn set_hovered_character_id(&mut self, id: Option<CharacterId>) {
        if let Some(previous_id) = self.hovered_id {
            if let Some(portrait) = self.portraits.get(&previous_id) {
                // The entry may have been removed if the character died recently
                portrait.borrow_mut().weak_highlight = false;
            }
        }
        self.hovered_id = id;
        if let Some(id) = self.hovered_id {
            self.portraits[&id].borrow_mut().weak_highlight = true;
        }
    }

    fn draw(&self, x: f32, y: f32) {
        self.row.draw(x, y);
    }

    fn remove_dead(&mut self) {
        self.portraits
            .retain(|_id, portrait| !portrait.borrow().character.has_died.get());
        self.row.remove_dropped_children();
    }
}

struct TopCharacterPortrait {
    strong_highlight: bool,
    weak_highlight: bool,
    action_points_row: Rc<RefCell<ActionPointsRow>>,
    health_bar: Rc<RefCell<ResourceBar>>,
    padding: f32,
    container: Container,
    character: Rc<Character>,
}

impl TopCharacterPortrait {
    fn new(character: &Rc<Character>, font: Font) -> Self {
        let action_points_row = Rc::new(RefCell::new(ActionPointsRow::new(
            character.max_reactive_action_points,
            (10.0, 10.0),
            0.25,
            Style::default(),
        )));
        let cloned_ap_row = Rc::clone(&action_points_row);

        let name_color = if character.player_controlled {
            WHITE
        } else {
            Color::new(1.0, 0.7, 0.7, 1.0)
        };

        let mut name_text_line = TextLine::new(character.name, 16, name_color, Some(font.clone()));
        name_text_line.set_min_height(13.0);
        let mut container = Container {
            layout_dir: LayoutDirection::Vertical,
            align: Align::Center,
            children: vec![
                Element::Text(name_text_line),
                Element::RcRefCell(cloned_ap_row),
            ],
            margin: 3.0,
            ..Default::default()
        };

        let health_bar = Rc::new(RefCell::new(ResourceBar::horizontal(
            character.health.max,
            RED,
            (container.content_size().0, 6.0),
        )));

        container
            .children
            .push(Element::RcRefCell(health_bar.clone()));

        Self {
            strong_highlight: false,
            weak_highlight: false,
            action_points_row,
            health_bar,
            padding: 5.0,
            container,
            character: character.clone(),
        }
    }
}

impl Drawable for TopCharacterPortrait {
    fn draw(&self, x: f32, y: f32) {
        let (w, h) = self.size();
        draw_rectangle(x, y, w, h, BLACK);
        draw_rectangle_lines(x + 1.0, y + 1.0, w - 2.0, h - 2.0, 3.0, DARKGRAY);
        if self.strong_highlight {
            draw_rectangle_lines(x, y, w, h, 5.0, GOLD);
        }
        if self.weak_highlight {
            draw_rectangle_lines(x + 1.0, y + 1.0, w - 2.0, h - 2.0, 2.0, LIGHTGRAY);
        }
        self.container.draw(x + self.padding, y + self.padding);
    }

    fn size(&self) -> (f32, f32) {
        let (w, h) = self.container.size();
        (w + self.padding * 2.0, h + self.padding * 2.0)
    }
}

struct CharacterSheetToggle {
    shown: Cell<bool>,
    text_line: TextLine,
    padding: f32,
}

impl Drawable for CharacterSheetToggle {
    fn draw(&self, x: f32, y: f32) {
        self.text_line.draw(x + self.padding, y + self.padding);

        let size = self.size();

        let (mouse_x, mouse_y) = mouse_position();
        let hovered = (x..x + size.0).contains(&mouse_x) && (y..y + size.1).contains(&mouse_y);
        if hovered && is_mouse_button_pressed(MouseButton::Left) {
            self.shown.set(!self.shown.get());
        }

        if self.shown.get() {
            draw_rectangle_lines(x, y, size.0, size.1, 2.0, GOLD);
        } else {
            draw_rectangle_lines(x, y, size.0, size.1, 1.0, LIGHTGRAY);
        }

        if hovered {
            draw_rectangle_lines(x + 2.0, y + 2.0, size.0 - 4.0, size.1 - 4.0, 1.0, WHITE);
        }
    }

    fn size(&self) -> (f32, f32) {
        let (w, h) = self.text_line.size();
        (w + self.padding * 2.0, h + self.padding * 2.0)
    }
}

struct PlayerPortraits {
    row: Container,
    selected_i: Cell<CharacterId>,
    active_i: Cell<CharacterId>,
    portraits: IndexMap<CharacterId, Rc<RefCell<PlayerCharacterPortrait>>>,
}

impl PlayerPortraits {
    fn new(
        characters: &Characters,
        selected_id: CharacterId,
        active_id: CharacterId,
        font: Font,
    ) -> Self {
        let mut portraits: IndexMap<CharacterId, Rc<RefCell<PlayerCharacterPortrait>>> =
            Default::default();

        for (id, character) in characters.iter_with_ids() {
            if character.player_controlled {
                portraits.insert(
                    *id,
                    Rc::new(RefCell::new(PlayerCharacterPortrait::new(
                        character,
                        font.clone(),
                    ))),
                );
            }
        }

        let mut elements = vec![];
        for portrait in portraits.values() {
            let cloned = Rc::clone(portrait);
            elements.push(Element::RcRefCell(cloned));
        }

        let row = Container {
            layout_dir: LayoutDirection::Horizontal,
            margin: 10.0,
            children: elements,
            ..Default::default()
        };

        let this = Self {
            row,
            selected_i: Cell::new(selected_id),
            active_i: Cell::new(active_id),
            portraits,
        };

        this.set_selected_character(selected_id);
        this
    }

    fn set_selected_character(&self, character_id: CharacterId) {
        self.portraits[&self.selected_i.get()]
            .borrow()
            .shown_character
            .set(false);
        self.selected_i.set(character_id);
        self.portraits[&self.selected_i.get()]
            .borrow()
            .shown_character
            .set(true);
    }

    fn set_active_character(&self, character_id: CharacterId) {
        if let Some(portrait) = self.portraits.get(&self.active_i.get()) {
            portrait.borrow().active_character.set(false);
        }
        self.active_i.set(character_id);
        if let Some(portrait) = self.portraits.get(&character_id) {
            portrait.borrow().active_character.set(true);
        }
    }

    fn update(&self, game: &CoreGame) {
        self.set_active_character(game.active_character_id);
    }

    fn draw(&self, x: f32, y: f32) {
        self.row.draw(x, y);

        for (i, portrait) in &self.portraits {
            if portrait.borrow().has_been_clicked.get() {
                portrait.borrow().has_been_clicked.set(false);
                self.set_selected_character(*i);
                break;
            }
        }
    }
}

struct PlayerCharacterPortrait {
    text: TextLine,
    shown_character: Cell<bool>,
    active_character: Cell<bool>,
    padding: f32,
    has_been_clicked: Cell<bool>,
}

impl PlayerCharacterPortrait {
    fn new(character: &Character, font: Font) -> Self {
        let mut text = TextLine::new(character.name, 20, WHITE, Some(font));
        text.set_depth(BLACK, 2.0);
        Self {
            text,
            shown_character: Cell::new(false),
            active_character: Cell::new(false),
            padding: 15.0,
            has_been_clicked: Cell::new(false),
        }
    }
}

impl Drawable for PlayerCharacterPortrait {
    fn draw(&self, x: f32, y: f32) {
        let (w, h) = self.size();
        draw_rectangle(x, y, w, h, DARKGRAY);
        if self.shown_character.get() {
            draw_rectangle_lines(x, y, w, h, 3.0, WHITE);
        } else {
            draw_rectangle_lines(x, y, w, h, 1.0, GRAY);
        }

        self.text.draw(self.padding + x, self.padding + y);

        if self.active_character.get() {
            let y_line = y + h - 10.0;
            let line_margin = 5.0;
            draw_line(
                x + self.padding - line_margin,
                y_line,
                x + w - self.padding + line_margin,
                y_line,
                2.0,
                GOLD,
            );
        }

        let (mouse_x, mouse_y) = mouse_position();
        let hovered = (x..x + w).contains(&mouse_x) && (y..y + h).contains(&mouse_y);

        if hovered {
            draw_rectangle_lines(x + 1.0, y + 1.0, w - 2.0, h - 2.0, 1.0, LIGHTGRAY);
            if is_mouse_button_pressed(MouseButton::Left) {
                self.has_been_clicked.set(true);
            }
        }
    }

    fn size(&self) -> (f32, f32) {
        let text_size = self.text.size();
        (
            text_size.0 + self.padding * 2.0,
            text_size.1 + self.padding * 2.0,
        )
    }
}

#[derive(Debug)]
pub struct UiGameEventHandler {
    pub events: RefCell<Vec<GameEvent>>,
}

impl Default for UiGameEventHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl UiGameEventHandler {
    pub fn new() -> Self {
        Self {
            events: RefCell::new(vec![]),
        }
    }
}

impl GameEventHandler for UiGameEventHandler {
    fn handle(&self, event: GameEvent) {
        self.events.borrow_mut().push(event);
    }
}

struct Log {
    container: Container,
    text_lines: Vec<Rc<TextLine>>,
    line_details: Vec<Option<Container>>,
    font: Font,
    padding: f32,
}

impl Log {
    fn new(font: Font) -> Self {
        let h = 150.0;
        Self {
            container: Container {
                layout_dir: LayoutDirection::Vertical,
                children: vec![],
                margin: 4.0,
                align: Align::End,
                scroll: Some(ContainerScroll::default()),
                min_width: Some(450.0),
                min_height: Some(h),
                max_height: Some(h),
                style: Style {
                    border_color: Some(GRAY),
                    padding: 5.0,
                    ..Default::default()
                },
                ..Default::default()
            },
            text_lines: vec![],
            line_details: vec![],
            font,
            padding: 10.0,
        }
    }

    fn add(&mut self, text: impl Into<String>) {
        self.add_with_details(text, vec![]);
    }

    fn add_with_details(&mut self, text: impl Into<String>, details: Vec<String>) {
        const MAX_LINES: usize = 50;
        if self.container.children.len() == MAX_LINES {
            self.container.children.remove(0);
            self.text_lines.remove(0);
            self.line_details.remove(0);
        }
        let mut text_line = TextLine::new(text, 18, WHITE, Some(self.font.clone()));
        text_line.set_padding(3.0, 3.0);
        let text_line = Rc::new(text_line);
        self.text_lines.push(text_line.clone());
        self.container.children.push(Element::Rc(text_line));

        if !details.is_empty() {
            let details_container = Container {
                layout_dir: LayoutDirection::Vertical,
                margin: 5.0,
                style: Style {
                    background_color: Some(BLACK),
                    padding: 5.0,
                    border_color: Some(GOLD),
                    ..Default::default()
                },
                children: details
                    .iter()
                    .map(|s| Element::Text(TextLine::new(s, 18, WHITE, Some(self.font.clone()))))
                    .collect(),
                ..Default::default()
            };
            self.line_details.push(Some(details_container));
        } else {
            self.line_details.push(None);
        }
    }

    fn draw(&self, x: f32, y: f32) {
        draw_line(x, y, x, y + 350.0, 1.0, DARKGRAY);
        self.container.draw(x + self.padding, y + self.padding);

        let size = self.size();
        for (i, text_line) in self.text_lines.iter().enumerate() {
            if let Some(line_pos) = text_line.has_been_hovered.take() {
                if let Some(details) = &self.line_details[i] {
                    let popup_size = details.size();
                    let details_x = x + size.0 - details.size().0 - 10.0;
                    let mut details_y = line_pos.1 + text_line.size().1 + 5.0;

                    if details_y + popup_size.1 > y + size.1 {
                        details_y = line_pos.1 - popup_size.1 - 5.0;
                    }

                    details.draw(details_x, details_y);
                }
            }
        }
    }

    fn size(&self) -> (f32, f32) {
        let container_size = self.container.size();
        (
            container_size.0 + self.padding,
            container_size.1 + self.padding,
        )
    }
}

#[derive(Default)]
pub struct ActionPointsRow {
    pub is_characters_turn: bool,
    max_reactive_ap: u32,
    pub current_ap: u32,
    reserved_and_hovered_ap: (u32, u32),
    max_ap: u32,
    cell_size: (f32, f32),
    padding: f32,
    style: Style,
    radius_factor: f32,
}

impl ActionPointsRow {
    pub fn new(
        max_reactive_ap: u32,
        cell_size: (f32, f32),
        radius_factor: f32,
        style: Style,
    ) -> Self {
        Self {
            is_characters_turn: false,
            max_reactive_ap,
            current_ap: 0,
            reserved_and_hovered_ap: (0, 0),
            max_ap: MAX_ACTION_POINTS,
            cell_size,
            radius_factor,
            padding: 3.0,
            style,
        }
    }
}

impl Drawable for ActionPointsRow {
    fn draw(&self, x: f32, y: f32) {
        assert!(self.current_ap <= self.max_ap);

        let size = self.size();
        draw_rectangle(x, y, size.0, size.1, BLACK);

        let mut x0 = x + self.padding;
        let y0 = y + self.padding;
        let r = self.cell_size.1 * self.radius_factor;
        let (reserved_ap, hovered_ap) = self.reserved_and_hovered_ap;

        for i in 0..self.max_ap {
            let is_point_hovered =
                (self.current_ap.saturating_sub(hovered_ap)..self.current_ap).contains(&i);

            let blocked_by_lack_of_reactive_ap = !self.is_characters_turn
                && i < self.max_ap.saturating_sub(self.max_reactive_ap)
                && i < self.current_ap;

            let mut overcomitted = false;

            if i < self.current_ap.saturating_sub(reserved_ap) {
                // Unreserved point

                if blocked_by_lack_of_reactive_ap {
                    draw_circle(
                        x0 + self.cell_size.0 / 2.0,
                        y0 + self.cell_size.1 / 2.0,
                        r,
                        GOLD,
                    );
                    draw_rectangle(x0, y0, self.cell_size.0, self.cell_size.1 * 0.5, BLACK);
                } else {
                    draw_circle(
                        x0 + self.cell_size.0 / 2.0,
                        y0 + self.cell_size.1 / 2.0,
                        r,
                        GOLD,
                    );
                }
            } else if i < self.current_ap {
                // Reserved
                draw_circle(
                    x0 + self.cell_size.0 / 2.0,
                    y0 + self.cell_size.1 / 2.0,
                    r,
                    WHITE,
                );
            } else if i < reserved_ap.max(hovered_ap) {
                // Overcomitted
                overcomitted = true;
                draw_circle(
                    x0 + self.cell_size.0 / 2.0,
                    y0 + self.cell_size.1 / 2.0,
                    r,
                    GRAY,
                );
            } else {
                // Spent / missing
                draw_circle(
                    x0 + self.cell_size.0 / 2.0,
                    y0 + self.cell_size.1 / 2.0,
                    r,
                    GRAY,
                );
            }

            if overcomitted {
                draw_circle_lines(
                    x0 + self.cell_size.0 / 2.0,
                    y0 + self.cell_size.1 / 2.0,
                    r,
                    2.0,
                    RED,
                );
            } else if is_point_hovered {
                draw_circle_lines(
                    x0 + self.cell_size.0 / 2.0,
                    y0 + self.cell_size.1 / 2.0,
                    r,
                    2.0,
                    SKYBLUE,
                );
            } else {
                draw_circle_lines(
                    x0 + self.cell_size.0 / 2.0,
                    y0 + self.cell_size.1 / 2.0,
                    r,
                    1.0,
                    GRAY,
                );
            }

            x0 += self.cell_size.0;
        }

        self.style.draw(x, y, self.size());
    }

    fn size(&self) -> (f32, f32) {
        (
            self.max_ap as f32 * self.cell_size.0 + self.padding * 2.0,
            self.cell_size.1 + self.padding * 2.0,
        )
    }
}

pub struct ResourceBar {
    pub current: u32,
    pub reserved: u32,
    pub max: u32,
    pub color: Color,
    pub cell_size: (f32, f32),
    pub layout: LayoutDirection,
}

impl ResourceBar {
    pub fn horizontal(max: u32, color: Color, size: (f32, f32)) -> Self {
        Self {
            current: max,
            reserved: 0,
            max,
            color,
            cell_size: (size.0 / max as f32, size.1),
            layout: LayoutDirection::Horizontal,
        }
    }
}

impl Drawable for ResourceBar {
    fn draw(&self, x: f32, y: f32) {
        assert!(self.current <= self.max);

        let cell_size = self.cell_size;
        let mut x0 = x;
        let mut y0 = y;

        match self.layout {
            LayoutDirection::Horizontal => {
                for i in 0..self.max {
                    if i < self.current + self.reserved {
                        if i >= self.current {
                            draw_rectangle(x0, y0, cell_size.0, cell_size.1, WHITE);
                        } else {
                            draw_rectangle(x0, y0, cell_size.0, cell_size.1, self.color);
                        }
                    }

                    if i > 0 {
                        let space = 4.0;
                        draw_line(x0, y0 + space, x0, y0 + cell_size.1 - space, 1.0, DARKGRAY);
                    }
                    x0 += cell_size.0;
                }

                draw_rectangle_lines(x, y, self.max as f32 * cell_size.0, cell_size.1, 1.0, WHITE);
            }
            LayoutDirection::Vertical => {
                for i in 0..self.max {
                    if i >= self.max - self.current {
                        if i < self.max - self.current + self.reserved {
                            draw_rectangle(x0, y0, cell_size.0, cell_size.1, WHITE);
                        } else {
                            draw_rectangle(x0, y0, cell_size.0, cell_size.1, self.color);
                        }
                    }

                    if i > 0 {
                        let space = 4.0;
                        draw_line(x0 + space, y0, x0 + cell_size.0 - space, y0, 1.0, DARKGRAY);
                    }
                    y0 += cell_size.1;
                }

                draw_rectangle_lines(x, y, cell_size.0, self.max as f32 * cell_size.1, 1.0, WHITE);
            }
        }
    }

    fn size(&self) -> (f32, f32) {
        match self.layout {
            LayoutDirection::Horizontal => (self.cell_size.0 * self.max as f32, self.cell_size.1),
            LayoutDirection::Vertical => (self.cell_size.0, self.cell_size.1 * self.max as f32),
        }
    }
}

struct LabelledResourceBar {
    list: Container,
    bar: Rc<RefCell<ResourceBar>>,
    value_text: Rc<RefCell<TextLine>>,
    max_value: u32,
}

impl LabelledResourceBar {
    fn new(current: u32, max: u32, label: &'static str, color: Color, font: Font) -> Self {
        assert!(current <= max);

        let cell_w = 15.0;
        let max_h = 70.0;
        let cell_h = if max <= 7 {
            max_h / 7.0
        } else {
            max_h / max as f32
        };
        let bar = Rc::new(RefCell::new(ResourceBar {
            current,
            reserved: 0,
            max,
            color,
            cell_size: (cell_w, cell_h),
            layout: LayoutDirection::Vertical,
        }));
        let cloned_bar = Rc::clone(&bar);

        let value_text = Rc::new(RefCell::new(TextLine::new(
            format!("{}/{}", current, max),
            17,
            WHITE,
            Some(font.clone()),
        )));
        let cloned_value_text = Rc::clone(&value_text);
        let label_text = TextLine::new(label, 16, WHITE, Some(font.clone()));

        let list = Container {
            layout_dir: LayoutDirection::Vertical,
            align: Align::Center,
            margin: 5.0,
            children: vec![
                Element::RcRefCell(cloned_bar),
                Element::RcRefCell(cloned_value_text),
                Element::Text(label_text),
            ],
            min_width: Some(40.0),
            ..Default::default()
        };

        Self {
            list,
            bar,
            value_text,
            max_value: max,
        }
    }

    fn set_current(&mut self, value: u32) {
        assert!(value <= self.bar.borrow().max);
        self.bar.borrow_mut().current = value;
        self.value_text
            .borrow_mut()
            .set_string(format!("{}/{}", value, self.max_value));
    }

    fn set_reserved(&mut self, value: u32) {
        self.bar.borrow_mut().reserved = value;
    }
}

impl Drawable for LabelledResourceBar {
    fn draw(&self, x: f32, y: f32) {
        self.list.draw(x, y);
    }

    fn size(&self) -> (f32, f32) {
        self.list.size()
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
