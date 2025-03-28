use std::{
    cell::{Cell, Ref, RefCell},
    char::MAX,
    collections::HashMap,
    rc::Rc,
};

use indexmap::IndexMap;
use macroquad::{
    color::{
        Color, BLACK, BLUE, DARKBROWN, DARKGRAY, GOLD, GRAY, GREEN, LIGHTGRAY, MAGENTA, ORANGE,
        RED, WHITE, YELLOW,
    },
    input::{is_mouse_button_pressed, mouse_position, MouseButton},
    math::Rect,
    shapes::{draw_circle, draw_circle_lines, draw_line, draw_rectangle, draw_rectangle_lines},
    text::{draw_text, draw_text_ex, measure_text, Font, TextParams},
    texture::{draw_texture_ex, DrawTextureParams, Texture2D},
    window::{screen_height, screen_width},
};

use crate::{
    base_ui::{
        draw_debug, table, Align, Container, ContainerScroll, Drawable, Element, LayoutDirection,
        Rectangle, Style, Tabs, TextLine,
    },
    core::{
        as_percentage, distance_between, prob_attack_hit, prob_spell_hit, Action,
        AttackEnhancement, AttackOutcome, BaseAction, Character, CharacterId, Characters, CoreGame,
        GameEvent, GameEventHandler, HandType, IconId, MovementEnhancement, OnAttackedReaction,
        OnHitReaction, SpellEnhancement, SpellType, TextureId, ACTION_POINTS_PER_TURN,
        MOVE_ACTION_COST,
    },
    grid::{Effect, EffectGraphics, EffectPosition, EffectVariant, GameGrid},
};

const Y_USER_INTERFACE: f32 = 700.0;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum UiState {
    ChoosingAction,
    ConfiguringAction(BaseAction),
    ReactingToAttack {
        hand: HandType,
        attacker: CharacterId,
        reactor: CharacterId,
    },
    ReactingToHit {
        attacker: CharacterId,
        victim: CharacterId,
        damage: u32,
    },
    Idle,
}

struct ActivityPopup {
    state: UiState,

    font: Font,

    initial_lines: Vec<String>,
    target_line: Option<String>,

    choice_buttons: IndexMap<u32, ActionButton>,
    proceed_button: ActionButton,

    enabled: bool,
    proceed_button_events: Rc<RefCell<Vec<InternalUiEvent>>>,
    choice_button_events: Rc<RefCell<Vec<InternalUiEvent>>>,
    base_action: Option<BaseAction>,
    selected_choice_button_ids: Vec<u32>,
    hovered_choice_button_id: Option<u32>,

    last_drawn_size: (f32, f32),
}

impl ActivityPopup {
    fn new(font: Font, state: UiState, mut proceed_button: ActionButton) -> Self {
        let proceed_button_events = Rc::new(RefCell::new(vec![]));
        proceed_button.event_sender = Some(EventSender {
            queue: Rc::clone(&proceed_button_events),
        });

        Self {
            state,
            font,
            initial_lines: vec![],
            target_line: None,
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

    fn draw(&mut self, x: f32, y: f32) {
        if matches!(self.state, UiState::Idle | UiState::ChoosingAction) {
            self.last_drawn_size = (0.0, 0.0);
            return;
        }

        let x0 = x + 10.0;
        let mut y0 = y + 20.0;

        let bg_color = DARKGRAY;
        let border_color = LIGHTGRAY;

        let size = (500.0, 85.0);
        draw_rectangle(x, y, size.0, size.1, bg_color);
        draw_rectangle_lines(x, y, size.0, size.1, 1.0, border_color);
        self.last_drawn_size = size;

        let text_params = TextParams {
            font: Some(&self.font),
            font_size: 16,
            color: WHITE,
            ..Default::default()
        };

        for line in &self.initial_lines {
            draw_text_ex(line, x0, y0, text_params.clone());
            y0 += 20.0;
        }

        if let Some(line) = &self.target_line {
            draw_text_ex(line, x0, y0, text_params.clone());
            y0 += 20.0;
        }

        let mut choice_description_line = "".to_string();
        for action in self.selected_actions() {
            choice_description_line.push('[');
            let s = match action {
                ButtonAction::AttackEnhancement(enhancement) => enhancement.description,
                ButtonAction::SpellEnhancement(enhancement) => enhancement.name,
                ButtonAction::MovementEnhancement(enhancement) => enhancement.name,
                ButtonAction::OnAttackedReaction(reaction) => reaction.description,
                ButtonAction::OnHitReaction(reaction) => reaction.description,
                ButtonAction::Action(..) | ButtonAction::Proceed => unreachable!(),
            };
            choice_description_line.push_str(s);
            choice_description_line.push(']');
        }
        draw_text_ex(&choice_description_line, x0, y0, text_params.clone());
        y0 += 20.0;

        if self.enabled {
            match &self.state {
                UiState::ConfiguringAction(base_action) => {
                    match base_action {
                        BaseAction::SelfEffect(..) => {}
                        BaseAction::Move { range, .. } => {
                            let percentage: u32 = self
                                .selected_actions()
                                .map(|action| action.unwrap_movement_enhancement().add_percentage)
                                .sum();
                            let range = range * (1.0 + percentage as f32 / 100.0);
                            let text = format!("range: {range:.2}");
                            draw_text_ex(&text, x0, y0, text_params.clone());
                            y0 += 20.0;
                        }
                        BaseAction::Attack { .. } | BaseAction::CastSpell(..) => {}
                    };
                }
                UiState::ReactingToAttack { .. } => {
                    draw_text_ex("Reaction:", x0, y0, text_params.clone());
                    y0 += 20.0;
                }
                UiState::ReactingToHit { .. } => {
                    draw_text_ex("Reaction:", x0, y0, text_params.clone());
                    y0 += 20.0;
                }
                UiState::Idle | UiState::ChoosingAction => unreachable!(),
            }
        }

        let y_btn = y + 10.0;
        let mut x_btn = x + 280.0;

        for btn in self.choice_buttons.values() {
            btn.draw(x_btn, y_btn);

            if self.hovered_choice_button_id == Some(btn.id) {
                draw_button_tooltip(&self.font, (x_btn, y_btn), &btn.tooltip_lines[..]);
            }

            x_btn += btn.size.0 + 10.0;
        }

        self.proceed_button.draw(x_btn, y_btn);
    }

    fn update(&mut self) -> Option<ActivityPopupOutcome> {
        let mut changed_movement_range = false;
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
                    if matches!(self.state, UiState::ReactingToAttack { .. })
                        || matches!(self.state, UiState::ReactingToHit { .. })
                        || matches!(clicked_btn.action, ButtonAction::MovementEnhancement { .. })
                    {
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
            for action in self.selected_actions() {
                if let ButtonAction::MovementEnhancement(enhancement) = action {
                    added_percentage += enhancement.add_percentage;
                }
            }
            return Some(ActivityPopupOutcome::ChangedMovementRangePercentage(
                added_percentage,
            ));
        }

        None
    }

    fn take_selected_actions(&mut self) -> Vec<ButtonAction> {
        self.selected_choice_button_ids
            .drain(..)
            .map(|id| self.choice_buttons[&id].action)
            .collect()
    }

    fn selected_actions(&self) -> impl Iterator<Item = &ButtonAction> {
        self.selected_choice_button_ids
            .iter()
            .map(|id| &self.choice_buttons[id].action)
    }

    fn select_movement_option(&mut self, selected_enhancement: Option<usize>) {
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

    fn action_points(&self) -> u32 {
        let mut ap = self
            .base_action
            .map(|action| action.action_point_cost())
            .unwrap_or(0);
        for action in self.selected_actions() {
            ap += action.action_point_cost();
        }
        if let Some(id) = self.hovered_choice_button_id {
            if !self.selected_choice_button_ids.contains(&id) {
                ap += self.choice_buttons[&id].action.action_point_cost();
            }
        }
        ap
    }

    fn mana_points(&self) -> u32 {
        let mut mana = self
            .base_action
            .map(|action| action.mana_cost())
            .unwrap_or(0);
        for action in self.selected_actions() {
            mana += action.mana_cost();
        }
        if let Some(id) = self.hovered_choice_button_id {
            if !self.selected_choice_button_ids.contains(&id) {
                mana += self.choice_buttons[&id].action.mana_cost()
            }
        }
        mana
    }

    fn stamina_points(&self) -> u32 {
        let mut sta = 0;
        for action in self.selected_actions() {
            sta += action.stamina_cost();
        }
        if let Some(id) = self.hovered_choice_button_id {
            if !self.selected_choice_button_ids.contains(&id) {
                sta += self.choice_buttons[&id].action.stamina_cost();
            }
        }
        sta
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        for btn in &mut self.choice_buttons.values() {
            btn.enabled.set(enabled);
        }
        self.proceed_button.enabled.set(enabled);
    }

    fn set_state(&mut self, state: UiState, lines: Vec<String>, buttons: Vec<ActionButton>) {
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

enum ActivityPopupOutcome {
    ChangedMovementRangePercentage(u32),
    ClickedProceed,
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
    buttons: Vec<Rc<ActionButton>>,
    tabs: Tabs,
    health_bar: Rc<RefCell<LabelledResourceBar>>,
    mana_bar: Rc<RefCell<LabelledResourceBar>>,
    stamina_bar: Rc<RefCell<LabelledResourceBar>>,
    resource_bars: Container,
    conditions: Vec<String>,
}

pub struct UserInterface {
    characters: Characters,
    event_queue: Rc<RefCell<Vec<InternalUiEvent>>>,
    state: UiState,
    stopwatch: StopWatch,

    font: Font,

    icons: HashMap<IconId, Texture2D>,

    hovered_button: Option<(u32, ButtonAction, (f32, f32))>,
    next_available_button_id: u32,
    active_character_id: CharacterId,

    pub game_grid: GameGrid,
    activity_popup: ActivityPopup,
    character_portraits: CharacterPortraits,
    player_portraits: PlayerPortraits,
    action_points_label: TextLine,
    action_points_row: ActionPointsRow,
    character_uis: HashMap<CharacterId, CharacterUi>,
    log: Log,
}

impl UserInterface {
    pub fn new(
        game: &CoreGame,
        textures: HashMap<TextureId, Texture2D>,
        icons: HashMap<IconId, Texture2D>,
        simple_font: Font,
        decorative_font: Font,
    ) -> Self {
        let characters = game.characters.clone();
        let active_character_id = game.active_character_id;

        let event_queue = Rc::new(RefCell::new(vec![]));
        let mut next_button_id = 1;

        let mut new_button = |subtext, btn_action, character: Option<&Character>| {
            let btn =
                ActionButton::new(btn_action, &event_queue, next_button_id, &icons, character);
            next_button_id += 1;
            btn
        };

        let mut character_uis: HashMap<CharacterId, CharacterUi> = Default::default();

        for (_i, character) in game.characters.iter().enumerate() {
            let character_ref = character.borrow();
            if !character_ref.player_controlled {
                continue;
            }

            let mut tracked_action_buttons = HashMap::new();
            let mut buttons = vec![];
            let mut basic_buttons = vec![];
            let mut spell_buttons = vec![];

            let mut enhancement_buttons = vec![];
            for (name, action) in character_ref.known_actions() {
                let btn_action = ButtonAction::Action(action);
                let btn = Rc::new(new_button(name, btn_action, Some(&character_ref)));
                tracked_action_buttons.insert(button_action_id(btn_action), Rc::clone(&btn));
                buttons.push(Rc::clone(&btn));
                match action {
                    BaseAction::Attack { .. } => basic_buttons.push(btn),
                    BaseAction::SelfEffect(..) => basic_buttons.push(btn),
                    BaseAction::CastSpell(spell) => {
                        if let Some(enhancement) = spell.possible_enhancement {
                            let btn_action = ButtonAction::SpellEnhancement(enhancement);
                            let btn = Rc::new(new_button(spell.name.to_string(), btn_action, None));
                            buttons.push(Rc::clone(&btn));
                            btn.enabled.set(false);
                            enhancement_buttons.push(btn);
                        }
                        spell_buttons.push(btn);
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

            let mut reaction_buttons = vec![];
            for (subtext, reaction) in character_ref.known_on_attacked_reactions() {
                let btn_action = ButtonAction::OnAttackedReaction(reaction);
                let btn = Rc::new(new_button(subtext.clone(), btn_action, None));
                buttons.push(Rc::clone(&btn));
                btn.enabled.set(false);
                reaction_buttons.push(btn);
            }
            for (subtext, reaction) in character_ref.known_on_hit_reactions() {
                let btn_action = ButtonAction::OnHitReaction(reaction);
                let btn = Rc::new(new_button(subtext.clone(), btn_action, None));
                buttons.push(Rc::clone(&btn));
                btn.enabled.set(false);
                reaction_buttons.push(btn);
            }
            let reactions_row = buttons_row(
                reaction_buttons
                    .into_iter()
                    .map(|btn| Element::Rc(btn))
                    .collect(),
            );

            for (subtext, enhancement) in
                character_ref.known_attack_enhancements(HandType::MainHand)
            {
                let btn_action = ButtonAction::AttackEnhancement(enhancement);
                let btn = Rc::new(new_button(subtext.clone(), btn_action, None));
                buttons.push(Rc::clone(&btn));
                btn.enabled.set(false);
                enhancement_buttons.push(btn);
            }
            let enhancements_row = buttons_row(
                enhancement_buttons
                    .into_iter()
                    .map(|btn| Element::Rc(btn))
                    .collect(),
            );

            let stats_table = Element::Container(Container {
                layout_dir: LayoutDirection::Vertical,
                children: vec![
                    Element::Container(attribute_row(
                        ("STR", character_ref.base_strength),
                        vec![
                            ("Health", character_ref.health.max as f32),
                            (
                                "Physical resist",
                                character_ref.physical_resistence() as f32,
                            ),
                        ],
                        simple_font.clone(),
                    )),
                    Element::Container(attribute_row(
                        ("DEX", character_ref.base_dexterity),
                        vec![
                            ("Defense", character_ref.defense() as f32),
                            ("Movement", character_ref.move_range),
                        ],
                        simple_font.clone(),
                    )),
                    Element::Container(attribute_row(
                        ("INT", character_ref.base_intellect),
                        vec![
                            ("Mana", character_ref.mana.max as f32),
                            ("Mental resist", character_ref.mental_resistence() as f32),
                        ],
                        simple_font.clone(),
                    )),
                ],
                border_between_children: Some(GRAY),
                style: Style {
                    border_color: Some(GRAY),
                    ..Default::default()
                },
                ..Default::default()
            });

            let mut equipment_cells = vec![];
            for hand in [HandType::MainHand, HandType::OffHand] {
                if let Some(weapon) = character_ref.weapon(hand) {
                    equipment_cells.push(format!("{}:", weapon.name));
                    equipment_cells.push(format!("{} dmg", weapon.damage));
                }
            }
            if let Some(shield) = character_ref.shield() {
                equipment_cells.push(format!("{}:", shield.name));
                equipment_cells.push(format!("{} def", shield.defense));
            }
            if let Some(armor) = character_ref.armor {
                equipment_cells.push(format!("{}:", armor.name));
                equipment_cells.push(format!("{} armor", armor.protection));
            }
            let equipment_table = table(
                equipment_cells,
                vec![Align::End, Align::Start],
                simple_font.clone(),
            );
            let stats_section = Element::Container(Container {
                layout_dir: LayoutDirection::Horizontal,
                children: vec![stats_table, equipment_table],
                margin: 10.0,
                ..Default::default()
            });

            let actions_section = Element::Container(Container {
                layout_dir: LayoutDirection::Vertical,
                margin: 5.0,
                children: vec![basic_row, spell_row],
                ..Default::default()
            });

            let secondary_actions_section = Element::Container(Container {
                layout_dir: LayoutDirection::Vertical,
                margin: 5.0,
                children: vec![reactions_row, enhancements_row],
                ..Default::default()
            });

            let tabs = Tabs::new(
                0,
                vec![
                    ("Actions", actions_section),
                    ("Secondary", secondary_actions_section),
                    ("Stats", stats_section),
                ],
                simple_font.clone(),
            );

            let health_bar = Rc::new(RefCell::new(LabelledResourceBar::new(
                character_ref.health.current,
                character_ref.health.max,
                "Health",
                RED,
                simple_font.clone(),
            )));
            let cloned_health_bar = Rc::clone(&health_bar);

            let mana_bar = Rc::new(RefCell::new(LabelledResourceBar::new(
                character_ref.mana.current,
                character_ref.mana.max,
                "Mana",
                BLUE,
                simple_font.clone(),
            )));
            let cloned_mana_bar = Rc::clone(&mana_bar);

            let stamina_bar = Rc::new(RefCell::new(LabelledResourceBar::new(
                character_ref.stamina.current,
                character_ref.stamina.max,
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
                    border_color: Some(GRAY),
                    padding: 10.0,
                    ..Default::default()
                },
                ..Default::default()
            };

            let character_ui = CharacterUi {
                tracked_action_buttons,
                tabs,
                health_bar,
                mana_bar,
                stamina_bar,
                resource_bars,
                conditions: vec![],
                buttons,
            };

            character_uis.insert(character.borrow().id(), character_ui);
        }

        let action_points_label =
            TextLine::new("Action points", 18, WHITE, Some(simple_font.clone()));
        let action_points_row = ActionPointsRow::new(
            (20.0, 20.0),
            0.3,
            Style {
                border_color: Some(WHITE),
                ..Default::default()
            },
        );

        let state = UiState::Idle;

        let game_grid = GameGrid::new(
            &game.characters,
            textures,
            (screen_width(), Y_USER_INTERFACE),
            simple_font.clone(),
        );

        let popup_proceed_btn = new_button("".to_string(), ButtonAction::Proceed, None);

        let first_player_character_id = game
            .characters
            .iter_with_ids()
            .find(|(_id, ch)| ch.borrow().player_controlled)
            .unwrap()
            .0;

        let player_portraits = PlayerPortraits::new(
            &game.characters,
            first_player_character_id,
            simple_font.clone(),
        );

        let character_portraits = CharacterPortraits::new(
            &game.characters,
            game.active_character_id,
            decorative_font.clone(),
        );

        Self {
            game_grid,
            characters,
            character_portraits,
            player_portraits,
            active_character_id,
            stopwatch: StopWatch::default(),

            icons,
            font: simple_font.clone(),

            next_available_button_id: next_button_id,
            hovered_button: None,
            log: Log::new(simple_font.clone()),
            character_uis,
            action_points_label,
            action_points_row,
            event_queue: Rc::clone(&event_queue),
            activity_popup: ActivityPopup::new(simple_font, state, popup_proceed_btn),
            state,
        }
    }

    fn new_button(&mut self, subtext: String, btn_action: ButtonAction) -> ActionButton {
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
        let y = Y_USER_INTERFACE;

        let popup_rectangle = Rect {
            x: 100.0,
            y: y - 90.0,
            w: self.activity_popup.last_drawn_size.0,
            h: self.activity_popup.last_drawn_size.1,
        };

        self.game_grid.position_on_screen = (0.0, 0.0);
        let grid_outcome = self.game_grid.draw(popup_rectangle);

        self.activity_popup
            .draw(popup_rectangle.x, popup_rectangle.y);

        draw_rectangle(0.0, y, screen_width(), screen_height() - y, BLACK);
        draw_line(0.0, y, screen_width(), y, 1.0, ORANGE);

        self.player_portraits.draw(270.0, y + 10.0);
        self.action_points_label.draw(20.0, y + 10.0);
        self.action_points_row.draw(20.0, y + 30.0);

        self.character_uis
            .get_mut(&self.player_portraits.selected_i.get())
            .unwrap()
            .tabs
            .draw(20.0, y + 70.0);

        let text_params = TextParams {
            font: Some(&self.font),
            font_size: 18,
            color: WHITE,
            ..Default::default()
        };
        for (i, s) in self.character_uis[&self.player_portraits.selected_i.get()]
            .conditions
            .iter()
            .enumerate()
        {
            draw_text_ex(s, 630.0, y + 30.0 + 20.0 * i as f32, text_params.clone());
        }

        self.character_uis[&self.player_portraits.selected_i.get()]
            .resource_bars
            .draw(620.0, y + 100.0);

        if let Some((btn_id, _btn_action, btn_pos)) = self.hovered_button {
            let btn = &self.character_uis[&self.player_portraits.selected_i.get()]
                .buttons
                .iter()
                .find(|btn| btn.id == btn_id)
                .unwrap();

            draw_button_tooltip(&self.font, btn_pos, &btn.tooltip_lines[..]);
        }

        self.log.draw(800.0, y);

        self.character_portraits
            .set_hovered_character_id(grid_outcome.hovered_character_id);

        self.character_portraits.draw(10.0, 10.0);

        if let Some(selected_move_option) = grid_outcome.switched_to_move_i {
            let move_range = self.active_character().move_range;
            self.set_state(UiState::ConfiguringAction(BaseAction::Move {
                action_point_cost: MOVE_ACTION_COST,
                range: move_range,
            }));

            let selected_enhancement = if selected_move_option == 0 {
                None
            } else {
                Some(selected_move_option - 1)
            };
            self.activity_popup
                .select_movement_option(selected_enhancement);
        }

        if grid_outcome.switched_to_idle {
            self.set_state(UiState::ChoosingAction);
        }

        if grid_outcome.switched_to_attack {
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
    }

    fn set_allowed_to_use_action_buttons(&self, allowed: bool) {
        for btn in self.character_uis[&self.player_portraits.selected_i.get()]
            .tracked_action_buttons
            .values()
        {
            let able = match btn.action {
                ButtonAction::Action(base_action) => {
                    self.active_character().can_use_action(base_action)
                }
                _ => todo!(),
            };

            btn.enabled.set(allowed && able);
        }
    }

    fn active_character(&self) -> Ref<Character> {
        self.characters.get(self.active_character_id)
    }

    pub fn set_state(&mut self, state: UiState) {
        if self.state == state {
            return;
        }

        self.state = state;

        let mut popup_initial_lines = vec![];
        let mut popup_buttons = vec![];
        let mut movement = false;
        let mut wants_target = false;

        match state {
            UiState::ConfiguringAction(base_action) => {
                self.game_grid.receptive_to_input = true;
                self.set_allowed_to_use_action_buttons(true);

                popup_initial_lines = self.character_uis[&self.active_character_id]
                    .tracked_action_buttons[&button_action_id(ButtonAction::Action(base_action))]
                    .tooltip_lines
                    .iter()
                    .map(|s| s.to_string())
                    .collect();

                self.set_highlighted_action(Some(ButtonAction::Action(base_action)));

                match base_action {
                    BaseAction::Attack {
                        hand,
                        action_point_cost: _,
                    } => {
                        let enhancements = self.active_character().usable_attack_enhancements(hand);
                        for (subtext, enhancement) in enhancements {
                            let btn = self
                                .new_button(subtext, ButtonAction::AttackEnhancement(enhancement));
                            popup_buttons.push(btn);
                        }
                        wants_target = true;
                    }
                    BaseAction::SelfEffect(..) => {}
                    BaseAction::CastSpell(spell) => {
                        if let Some(enhancement) = spell.possible_enhancement {
                            if self.active_character().can_use_spell_enhancement(spell) {
                                let btn_action = ButtonAction::SpellEnhancement(enhancement);
                                let btn = self.new_button("".to_string(), btn_action);
                                popup_buttons.push(btn);
                            }
                        }
                        wants_target = true;
                    }
                    BaseAction::Move { .. } => {
                        let enhancements = self.active_character().usable_movement_enhancements();
                        for (subtext, enhancement) in enhancements {
                            let btn = self.new_button(
                                subtext,
                                ButtonAction::MovementEnhancement(enhancement),
                            );
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
            } => {
                self.set_allowed_to_use_action_buttons(false);

                let attacker = self.characters.get(attacker_id);
                let defender = self.characters.get(reactor_id);

                let attacks_str = format!(
                    "{} attacks {} (d20+{} vs {})",
                    attacker.name,
                    defender.name,
                    attacker.attack_modifier(hand),
                    defender.defense(),
                );
                popup_initial_lines.push(attacks_str);
                let explanation = format!(
                    "{}{}",
                    attacker.explain_attack_circumstances(hand),
                    defender.explain_incoming_attack_circumstances()
                );
                if !explanation.is_empty() {
                    popup_initial_lines.push(format!("  {explanation}"));
                }
                popup_initial_lines.push(format!(
                    "  Chance to hit: {}",
                    as_percentage(prob_attack_hit(&attacker, hand, &defender))
                ));
                let reactions = defender.usable_on_attacked_reactions();
                drop(attacker);
                drop(defender);
                for (subtext, reaction) in reactions {
                    let btn_action = ButtonAction::OnAttackedReaction(reaction);
                    let btn = self.new_button(subtext, btn_action);
                    popup_buttons.push(btn);
                }
            }

            UiState::ReactingToHit {
                attacker: attacker_id,
                damage,
                victim: victim_id,
            } => {
                self.set_allowed_to_use_action_buttons(false);

                let victim = self.characters.get(victim_id);
                popup_initial_lines.push(format!(
                    "{} took {} damage from an attack by {}",
                    victim.name,
                    damage,
                    self.characters.get(attacker_id).name
                ));
                let reactions = victim.usable_on_hit_reactions();
                drop(victim);
                for (subtext, reaction) in reactions {
                    let btn_action = ButtonAction::OnHitReaction(reaction);
                    let btn = self.new_button(subtext, btn_action);
                    popup_buttons.push(btn);
                }
            }

            UiState::ChoosingAction => {
                self.game_grid.receptive_to_input = true;
                self.set_allowed_to_use_action_buttons(true);
            }

            UiState::Idle => {
                self.set_allowed_to_use_action_buttons(false);
                self.game_grid.receptive_to_input = false;
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
            self.game_grid.ensure_has_some_movement_preview();
        } else {
            self.game_grid.remove_movement_preview();
        }

        if wants_target {
            // We pick an arbitrary enemy if none is picked already
            self.game_grid.ensure_has_npc_target();
        } else {
            self.game_grid.remove_target();
        }
    }

    pub fn ready_for_more(&self) -> bool {
        self.stopwatch.remaining.is_none()
    }

    pub fn handle_game_event(&mut self, event: GameEvent) {
        dbg!(&event);
        match event {
            GameEvent::LogLine(line) => {
                self.log.add(line);
            }
            GameEvent::CharacterTookDamage { character, amount } => {
                // TODO: We show this as part of Attacked, SpellWasCast, etc, instead
                /*
                let pos = self.characters.get(character).position_i32();
                self.game_grid
                    .add_text_effect(pos, 0.0, 2.0, format!("{}", amount));
                 */
            }

            GameEvent::CharacterReactedToHit {
                main_line,
                detail_lines,
            } => {
                self.log.add_with_details(main_line, detail_lines);
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

                let attacker_pos = self.characters.get(attacker).position_i32();
                let target_pos = self.characters.get(target).position_i32();

                let dist = distance_between(attacker_pos, target_pos);
                let duration = 0.15 * dist;

                self.stopwatch.set_to_at_least(duration + 0.4);
                let impact_text = match outcome {
                    AttackOutcome::Hit(damage) => format!("{}", damage),
                    AttackOutcome::Dodge => "Dodge".to_string(),
                    AttackOutcome::Parry => "Parry".to_string(),
                    AttackOutcome::Miss => "Miss".to_string(),
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
                    .add_text_effect(target_pos, duration, 0.5, impact_text);
            }
            GameEvent::SpellWasCast {
                caster,
                target,
                outcome,
                spell,
                detail_lines,
            } => {
                let mut line = format!(
                    "{} cast {} on {}",
                    self.characters.get(caster).name,
                    spell.name,
                    self.characters.get(target).name
                );

                match outcome {
                    crate::core::SpellOutcome::Hit(damage) => {
                        line.push_str(&format!(" ({} damage)", damage))
                    }
                    crate::core::SpellOutcome::Resist => line.push_str("  (miss)"),
                }

                self.log.add_with_details(line, detail_lines);

                let caster_pos = self.characters.get(caster).position_i32();
                let target_pos = self.characters.get(target).position_i32();
                let color = match spell.spell_type {
                    SpellType::Mental => BLUE,
                    SpellType::Projectile => RED,
                };

                let dist = distance_between(caster_pos, target_pos);
                let duration = 0.15 * dist;

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
                                fill: Some(color),
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
                                fill: Some(color),
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
                                fill: Some(color),
                                stroke: None,
                            },
                        ),
                    },
                );

                /*
                self.game_grid.add_effect(
                    caster_pos,
                    target_pos,
                    Effect {
                        start_time: 0.0,
                        end_time: duration,
                        variant: EffectVariant::At(
                            EffectPosition::Projectile,
                            EffectGraphics::Rectangle { width: 15.0, end_width: Some(30.0), start_rotation: 2.0, rotation_per_s: 6.0, fill: None, stroke: Some((color, 4.0)) }
                        ),
                    },
                );
                self.game_grid.add_effect(
                    caster_pos,
                    target_pos,
                    Effect {
                        start_time: 0.0,
                        end_time: duration,
                        variant: EffectVariant::At(
                            EffectPosition::Projectile,
                            EffectGraphics::Rectangle { width: 10.0, end_width: Some(20.0), start_rotation: 0.0, rotation_per_s: 6.0, fill: None, stroke: Some((MAGENTA, 4.0)) }
                        ),
                    },
                );
                self.game_grid.add_effect(
                    caster_pos,
                    target_pos,
                    Effect {
                        start_time: 0.0,
                        end_time: 0.1,
                        variant: EffectVariant::Line { color:BLACK, thickness: 5.0, end_thickness: Some(0.0), extend_gradually: false },
                    },
                );
                 */

                let impact_text = match outcome {
                    crate::core::SpellOutcome::Hit(damage) => format!("{}", damage),
                    crate::core::SpellOutcome::Resist => "Resist".to_string(),
                };

                self.game_grid
                    .add_text_effect(target_pos, duration, 0.5, impact_text);

                self.stopwatch.set_to_at_least(duration + 0.3);
            }
            GameEvent::CharacterReceivedSelfEffect {
                character,
                condition,
            } => {
                let pos = self.characters.get(character).position;
                let duration = 1.0;
                self.game_grid.add_text_effect(
                    (pos.0 as i32, pos.1 as i32),
                    0.0,
                    duration,
                    format!("{:?}", condition),
                );
                self.stopwatch.set_to_at_least(duration);
            }
            GameEvent::CharacterDied { character } => {
                self.log
                    .add(format!("{} died", self.characters.get(character).name));

                self.characters.remove_dead();
                self.game_grid.remove_dead();
                self.character_portraits.remove_dead();
            }
            GameEvent::Moved {
                character,
                from,
                to,
            } => {
                /*
                self.log.add(format!(
                    "{} moved from {:?} to {:?}",
                    self.characters.get(character).name,
                    from,
                    to
                ));
                 */

                let duration = 0.6;
                self.game_grid
                    .set_character_motion(character, from, to, duration);
                self.stopwatch.set_to_at_least(duration);
            }
        }
    }

    pub fn update(&mut self, game: &CoreGame, elapsed: f32) -> Vec<PlayerChose> {
        let active_character_id = game.active_character_id;

        if active_character_id != self.active_character_id {
            // When control switches to a new player controlled character, make the UI show that character
            if self.characters.get(active_character_id).player_controlled {
                self.player_portraits
                    .set_selected_character(active_character_id);
            }
        }

        self.set_allowed_to_use_action_buttons(
            self.player_portraits.selected_i.get() == active_character_id,
        );

        self.active_character_id = active_character_id;

        self.game_grid
            .update(active_character_id, &self.characters, elapsed);

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
            None => {}
        }

        self.event_queue
            .take()
            .into_iter()
            .for_each(|event| self.handle_internal_ui_event(event));

        let mut popup_enabled = true;

        self.activity_popup.target_line = None;
        self.game_grid.static_text = None;
        self.game_grid.range_indicator = None;

        match self.state {
            UiState::ConfiguringAction(base_action @ BaseAction::Attack { hand, .. }) => {
                popup_enabled = false; // until proven otherwise
                if let Some(i) = self.game_grid.target() {
                    let target_char = self.characters.get(i);

                    let chance = as_percentage(prob_attack_hit(
                        &self.active_character(),
                        hand,
                        &target_char,
                    ));
                    let mut explanation =
                        self.active_character().explain_attack_circumstances(hand);
                    explanation.push_str(&target_char.explain_incoming_attack_circumstances());

                    self.game_grid.static_text = Some((
                        target_char.position_i32(),
                        vec![format!("Attack: {}", chance), explanation],
                    ));

                    if self
                        .active_character()
                        .can_reach_with_attack(hand, target_char.position)
                    {
                        if self.active_character().can_use_action(base_action) {
                            popup_enabled = true;
                        } else {
                            println!("Can not attack!");
                        }
                    } else {
                        let range = self.active_character().weapon(hand).unwrap().range;
                        self.game_grid.range_indicator = Some(range);

                        self.activity_popup.target_line =
                            Some(format!("[{}] Out of range!", target_char.name));
                    }
                } else {
                    self.activity_popup.target_line = Some("Select a target".to_string());
                }
            }
            UiState::ConfiguringAction(BaseAction::CastSpell(spell)) => {
                popup_enabled = false; // until proven otherwise
                if let Some(i) = self.game_grid.target() {
                    let target_char = self.characters.get(i);
                    let chance = as_percentage(prob_spell_hit(
                        &self.active_character(),
                        spell.spell_type,
                        &target_char,
                    ));

                    self.game_grid.static_text = Some((
                        target_char.position_i32(),
                        vec![format!("{}: {}", spell.name, chance)],
                    ));

                    if self
                        .active_character()
                        .can_reach_with_spell(spell, target_char.position)
                    {
                        popup_enabled = true;
                    } else {
                        let range = spell.range;
                        self.game_grid.range_indicator = Some(range);
                        self.activity_popup.target_line =
                            Some(format!("[{}] Out of range!", target_char.name));
                    }
                } else {
                    self.activity_popup.target_line = Some("Select a target".to_string());
                }
            }
            UiState::ConfiguringAction(BaseAction::Move { .. }) => {
                popup_enabled = self.game_grid.has_non_empty_movement_preview();
            }
            _ => {}
        }

        self.activity_popup.set_enabled(popup_enabled);

        self.character_portraits.update(game);

        self.update_character_status(&game.characters);

        if let Some(hovered_btn) = self.hovered_button {
            self.action_points_row.reserved_and_hovered = hovered_btn.1.action_point_cost();
            self.character_uis[&self.player_portraits.selected_i.get()]
                .mana_bar
                .borrow_mut()
                .set_reserved(hovered_btn.1.mana_cost());
            self.character_uis[&self.player_portraits.selected_i.get()]
                .stamina_bar
                .borrow_mut()
                .set_reserved(hovered_btn.1.stamina_cost());
        } else {
            self.action_points_row.reserved_and_hovered = self.activity_popup.action_points();
            self.character_uis[&self.player_portraits.selected_i.get()]
                .mana_bar
                .borrow_mut()
                .set_reserved(self.activity_popup.mana_points());
            self.character_uis[&self.player_portraits.selected_i.get()]
                .stamina_bar
                .borrow_mut()
                .set_reserved(self.activity_popup.stamina_points());
        };

        if self.stopwatch.update(elapsed) {
            println!("UI is now ready...");
        }

        if let Some(choice) = player_choice {
            vec![choice]
        } else {
            vec![]
        }
    }

    fn handle_popup_proceed(&mut self) -> PlayerChose {
        // Action button is highlighted while the action is being configured in the popup. That should be cleared now.
        self.set_highlighted_action(None);

        match self.state {
            UiState::ConfiguringAction(base_action) => {
                let target = self.game_grid.target();
                let action = match base_action {
                    BaseAction::Attack { hand, .. } => {
                        let enhancements = self
                            .activity_popup
                            .take_selected_actions()
                            .into_iter()
                            .map(|action| match action {
                                ButtonAction::AttackEnhancement(e) => e,
                                _ => unreachable!(),
                            })
                            .collect();

                        Action::Attack {
                            hand,
                            enhancements,
                            target: target.unwrap(),
                        }
                    }
                    BaseAction::SelfEffect(sea) => Action::SelfEffect(sea),
                    BaseAction::CastSpell(spell) => {
                        // TODO multiple spell enhancements
                        let enhanced: bool = self
                            .activity_popup
                            .take_selected_actions()
                            .into_iter()
                            .map(|action| match action {
                                ButtonAction::SpellEnhancement(e) => e,
                                _ => unreachable!(),
                            })
                            .count()
                            > 0;

                        Action::CastSpell {
                            spell,
                            enhanced,
                            target: target.unwrap(),
                        }
                    }
                    BaseAction::Move {
                        action_point_cost,
                        range: _,
                    } => {
                        let enhancements = self
                            .activity_popup
                            .take_selected_actions()
                            .into_iter()
                            .map(|action| match action {
                                ButtonAction::MovementEnhancement(e) => e,
                                _ => unreachable!(),
                            })
                            .collect();

                        Action::Move {
                            action_point_cost,
                            enhancements,
                            positions: self.game_grid.take_movement_path(),
                        }
                    }
                };
                PlayerChose::Action(action)
            }
            UiState::ReactingToAttack { .. } => {
                let reaction =
                    self.activity_popup.take_selected_actions().first().map(
                        |action| match action {
                            ButtonAction::OnAttackedReaction(reaction) => *reaction,
                            _ => unreachable!(),
                        },
                    );
                PlayerChose::AttackedReaction(reaction)
            }
            UiState::ReactingToHit { .. } => {
                let reaction =
                    self.activity_popup.take_selected_actions().first().map(
                        |action| match action {
                            ButtonAction::OnHitReaction(reaction) => *reaction,
                            _ => unreachable!(),
                        },
                    );

                PlayerChose::HitReaction(reaction)
            }
            UiState::ChoosingAction => unreachable!(),
            UiState::Idle => unreachable!(),
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
            let character = character.borrow();
            if let Some(ui) = self.character_uis.get_mut(id) {
                ui.health_bar
                    .borrow_mut()
                    .set_current(character.health.current);
                ui.mana_bar.borrow_mut().set_current(character.mana.current);
                ui.stamina_bar
                    .borrow_mut()
                    .set_current(character.stamina.current);

                ui.conditions = character.condition_strings();
            }
        }

        // TODO: Don't crash on player death
        self.action_points_row.current = self
            .characters
            .get(self.player_portraits.selected_i.get())
            .action_points;
    }
}

fn draw_button_tooltip(font: &Font, button_position: (f32, f32), lines: &[String]) {
    let font_size = 18;
    let mut max_line_w = 0.0;
    let text_margin = 8.0;
    for line in lines {
        let dimensions = measure_text(line, None, font_size, 1.0);
        if dimensions.width > max_line_w {
            max_line_w = dimensions.width;
        }
    }

    let tooltip_w = max_line_w + text_margin * 2.0;

    let line_h = 22.0;
    let tooltip_h = lines.len() as f32 * line_h + text_margin * 2.0;

    let tooltip_rect = (
        button_position.0,
        button_position.1 - 3.0 - tooltip_h,
        tooltip_w,
        tooltip_h,
    );
    draw_rectangle(
        tooltip_rect.0,
        tooltip_rect.1,
        tooltip_rect.2,
        tooltip_rect.3,
        Color::new(0.0, 0.0, 0.0, 0.9),
    );
    draw_rectangle_lines(
        tooltip_rect.0,
        tooltip_rect.1,
        tooltip_rect.2,
        tooltip_rect.3,
        1.0,
        GRAY,
    );

    let text_params = TextParams {
        font: Some(font),
        font_size,
        color: WHITE,
        ..Default::default()
    };

    let mut line_y = button_position.1 - lines.len() as f32 * line_h + text_margin - 5.0;
    for line in lines {
        draw_text_ex(
            line,
            button_position.0 + text_margin,
            line_y,
            text_params.clone(),
        );
        line_y += line_h;
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
        ButtonAction::OnAttackedReaction(_on_attacked_reaction) => todo!(),
        ButtonAction::OnHitReaction(_on_hit_reaction) => todo!(),
        ButtonAction::AttackEnhancement(_attack_enhancement) => todo!(),
        ButtonAction::SpellEnhancement(_spell_enhancement) => todo!(),
        ButtonAction::MovementEnhancement(_movement_enhancement) => todo!(),
        ButtonAction::Proceed => todo!(),
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
            margin: 10.0,
            children: elements,
            style: Style {
                background_color: Some(BLACK),
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
            let character = character.borrow();
            portrait.action_points_row.borrow_mut().current = character.action_points;
            portrait.hp_text.borrow_mut().set_string(format!(
                "{}/{}",
                character.health.current, character.health.max
            ));
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
            .retain(|_id, portrait| !portrait.borrow().character.borrow().has_died);
        self.row.remove_dropped_children();
    }
}

struct TopCharacterPortrait {
    strong_highlight: bool,
    weak_highlight: bool,
    hp_text: Rc<RefCell<TextLine>>,
    action_points_row: Rc<RefCell<ActionPointsRow>>,
    padding: f32,
    container: Container,
    character: Rc<RefCell<Character>>,
}

impl TopCharacterPortrait {
    fn new(character: &Rc<RefCell<Character>>, font: Font) -> Self {
        let action_points_row = Rc::new(RefCell::new(ActionPointsRow::new(
            (10.0, 10.0),
            0.15,
            Style::default(),
        )));
        let cloned_row = Rc::clone(&action_points_row);

        let hp_text = Rc::new(RefCell::new(TextLine::new(
            "0/0",
            16,
            WHITE,
            Some(font.clone()),
        )));
        let cloned_text = Rc::clone(&hp_text);

        let name_color = if character.borrow().player_controlled {
            WHITE
        } else {
            Color::new(1.0, 0.7, 0.7, 1.0)
        };

        let container = Container {
            layout_dir: LayoutDirection::Vertical,
            align: Align::Center,
            children: vec![
                Element::Text(TextLine::new(
                    character.borrow().name,
                    16,
                    name_color,
                    Some(font.clone()),
                )),
                Element::RcRefCell(cloned_row),
                Element::RcRefCell(cloned_text),
            ],
            margin: 5.0,
            ..Default::default()
        };

        Self {
            strong_highlight: false,
            weak_highlight: false,
            action_points_row,
            hp_text,
            padding: 5.0,
            container,
            character: character.clone(),
        }
    }
}

impl Drawable for TopCharacterPortrait {
    fn draw(&self, x: f32, y: f32) {
        if self.strong_highlight {
            let (w, h) = self.size();
            draw_rectangle_lines(x + 1.0, y + 1.0, w - 2.0, h - 2.0, 3.0, GOLD);
        }
        if self.weak_highlight {
            let (w, h) = self.size();
            draw_rectangle_lines(x + 1.0, y + 1.0, w - 2.0, h - 2.0, 1.0, WHITE);
        }
        self.container.draw(x + self.padding, y + self.padding);
    }

    fn size(&self) -> (f32, f32) {
        let (w, h) = self.container.size();
        (w + self.padding * 2.0, h + self.padding * 2.0)
    }
}

struct PlayerPortraits {
    row: Container,
    selected_i: Cell<CharacterId>,
    portraits: IndexMap<CharacterId, Rc<RefCell<PlayerCharacterPortrait>>>,
}

impl PlayerPortraits {
    fn new(characters: &Characters, selected_id: CharacterId, font: Font) -> Self {
        let mut portraits: IndexMap<CharacterId, Rc<RefCell<PlayerCharacterPortrait>>> =
            Default::default();

        for (id, character) in characters.iter_with_ids() {
            if character.borrow().player_controlled {
                portraits.insert(
                    *id,
                    Rc::new(RefCell::new(PlayerCharacterPortrait::new(
                        &character.borrow(),
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
            portraits,
        };

        this.set_selected_character(selected_id);
        this
    }

    fn set_selected_character(&self, character_id: CharacterId) {
        self.portraits[&self.selected_i.get()]
            .borrow()
            .selected
            .set(false);
        self.selected_i.set(character_id);
        self.portraits[&self.selected_i.get()]
            .borrow()
            .selected
            .set(true);
    }

    fn draw(&self, x: f32, y: f32) {
        self.row.draw(x, y);

        for (i, portrait) in &self.portraits {
            if portrait.borrow().clicked.get() {
                portrait.borrow().clicked.set(false);
                self.set_selected_character(*i);
                break;
            }
        }
    }
}

struct PlayerCharacterPortrait {
    text: TextLine,
    selected: Cell<bool>,
    padding: f32,
    clicked: Cell<bool>,
}

impl PlayerCharacterPortrait {
    fn new(character: &Character, font: Font) -> Self {
        Self {
            text: TextLine::new(character.name, 20, WHITE, Some(font)),
            selected: Cell::new(false),
            padding: 15.0,
            clicked: Cell::new(false),
        }
    }
}

impl Drawable for PlayerCharacterPortrait {
    fn draw(&self, x: f32, y: f32) {
        let (w, h) = self.size();
        draw_rectangle(x, y, w, h, DARKGRAY);
        if self.selected.get() {
            draw_rectangle_lines(x, y, w, h, 2.0, GOLD);
        }
        self.text.draw(self.padding + x, self.padding + y);

        let (mouse_x, mouse_y) = mouse_position();
        if (x..x + w).contains(&mouse_x)
            && (y..y + h).contains(&mouse_y)
            && is_mouse_button_pressed(MouseButton::Left)
        {
            self.clicked.set(true);
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

struct EventSender {
    queue: Rc<RefCell<Vec<InternalUiEvent>>>,
}

impl EventSender {
    fn send(&self, value: InternalUiEvent) {
        self.queue.borrow_mut().push(value);
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
}

impl Log {
    fn new(font: Font) -> Self {
        let this = Self {
            container: Container {
                layout_dir: LayoutDirection::Vertical,
                children: vec![],
                margin: 4.0,
                align: Align::End,
                scroll: Some(ContainerScroll::default()),
                min_width: Some(450.0),
                min_height: Some(250.0),
                max_height: Some(250.0),
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
        };

        this
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
        text_line.set_padding(3.0);
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
        self.container.draw(x + 10.0, y + 10.0);

        for (i, text_line) in self.text_lines.iter().enumerate() {
            if let Some(line_pos) = text_line.has_been_hovered.take() {
                if let Some(details) = &self.line_details[i] {
                    let details_x = x + self.container.size().0 - details.size().0;
                    details.draw(details_x, line_pos.1 + text_line.size().1 + 5.0);
                }
            }
        }
    }
}

struct ActionPointsRow {
    current: u32,
    reserved_and_hovered: u32,
    max: u32,
    cell_size: (f32, f32),
    padding: f32,
    style: Style,
    radius_factor: f32,
}

impl ActionPointsRow {
    fn new(cell_size: (f32, f32), radius_factor: f32, style: Style) -> Self {
        Self {
            current: 0,
            reserved_and_hovered: 0,
            max: ACTION_POINTS_PER_TURN,
            cell_size,
            radius_factor,
            padding: 3.0,
            style,
        }
    }
}

impl Drawable for ActionPointsRow {
    fn draw(&self, x: f32, y: f32) {
        assert!(self.current <= self.max);

        let mut x0 = x + self.padding;
        let y0 = y + self.padding;
        let r = self.cell_size.1 * self.radius_factor;
        for i in 0..self.max {
            if i < self.current.saturating_sub(self.reserved_and_hovered) {
                draw_circle(
                    x0 + self.cell_size.0 / 2.0,
                    y0 + self.cell_size.1 / 2.0,
                    r,
                    GOLD,
                );
            } else if i < self.current {
                draw_circle(
                    x0 + self.cell_size.0 / 2.0,
                    y0 + self.cell_size.1 / 2.0,
                    r,
                    WHITE,
                );
            } else if i < self.reserved_and_hovered {
                draw_circle(
                    x0 + self.cell_size.0 / 2.0,
                    y0 + self.cell_size.1 / 2.0,
                    r,
                    RED,
                );
            } else {
                draw_circle(
                    x0 + self.cell_size.0 / 2.0,
                    y0 + self.cell_size.1 / 2.0,
                    r,
                    GRAY,
                );
            }
            draw_circle_lines(
                x0 + self.cell_size.0 / 2.0,
                y0 + self.cell_size.1 / 2.0,
                r,
                1.0,
                DARKGRAY,
            );
            x0 += self.cell_size.0;
        }

        self.style.draw(x, y, self.size());
    }

    fn size(&self) -> (f32, f32) {
        (
            self.max as f32 * self.cell_size.0 + self.padding * 2.0,
            self.cell_size.1 + self.padding * 2.0,
        )
    }
}

struct ResourceBar {
    current: u32,
    reserved: u32,
    max: u32,
    color: Color,
    cell_size: (f32, f32),
}

impl Drawable for ResourceBar {
    fn draw(&self, x: f32, y: f32) {
        assert!(self.current <= self.max);

        let cell_size = self.cell_size;
        let mut y0 = y;
        for i in 0..self.max {
            if i >= self.max - self.current {
                if i < self.max - self.current + self.reserved {
                    draw_rectangle(x, y0, cell_size.0, cell_size.1, WHITE);
                } else {
                    draw_rectangle(x, y0, cell_size.0, cell_size.1, self.color);
                }
            }

            if i > 0 {
                let space = 4.0;
                draw_line(x + space, y0, x + cell_size.0 - space, y0, 1.0, DARKGRAY);
            }

            y0 += cell_size.1;
        }

        draw_rectangle_lines(x, y, cell_size.0, self.max as f32 * cell_size.1, 1.0, WHITE);
    }

    fn size(&self) -> (f32, f32) {
        (self.cell_size.0, self.cell_size.1 * self.max as f32)
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
        let max_h = 100.0;
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
        self.list.draw(x, y)
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

fn attribute_row(
    attribute: (&'static str, u32),
    stats: Vec<(&'static str, f32)>,
    font: Font,
) -> Container {
    let attribute_element = Element::Text(TextLine::new(
        format!("{}: {}", attribute.0, attribute.1),
        22,
        WHITE,
        Some(font.clone()),
    ));

    let stat_rows: Vec<Element> = stats
        .iter()
        .map(|(name, value)| {
            Element::Text(TextLine::new(
                format!("{} = {}", name, value),
                18,
                WHITE,
                Some(font.clone()),
            ))
        })
        .collect();

    let stats_list = Element::Container(Container {
        layout_dir: LayoutDirection::Vertical,
        margin: 4.0,
        children: stat_rows,
        ..Default::default()
    });
    Container {
        layout_dir: LayoutDirection::Horizontal,
        margin: 20.0,
        align: Align::Center,
        children: vec![attribute_element, stats_list],
        style: Style {
            padding: 5.0,
            ..Default::default()
        },
        ..Default::default()
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum ButtonAction {
    Action(BaseAction),
    OnAttackedReaction(OnAttackedReaction),
    OnHitReaction(OnHitReaction),
    AttackEnhancement(AttackEnhancement),
    SpellEnhancement(SpellEnhancement),
    MovementEnhancement(MovementEnhancement),
    Proceed,
}

impl ButtonAction {
    fn action_point_cost(&self) -> u32 {
        match self {
            ButtonAction::Action(base_action) => base_action.action_point_cost(),
            ButtonAction::OnAttackedReaction(reaction) => reaction.action_point_cost,
            ButtonAction::OnHitReaction(reaction) => reaction.action_point_cost,
            ButtonAction::AttackEnhancement(enhancement) => enhancement.action_point_cost,
            ButtonAction::SpellEnhancement(..) => 0,
            ButtonAction::Proceed => 0,
            ButtonAction::MovementEnhancement(enhancement) => enhancement.action_point_cost,
        }
    }

    fn mana_cost(&self) -> u32 {
        match self {
            ButtonAction::Action(base_action) => base_action.mana_cost(),
            ButtonAction::OnAttackedReaction(..) => 0,
            ButtonAction::OnHitReaction(..) => 0,
            ButtonAction::AttackEnhancement(..) => 0,
            ButtonAction::SpellEnhancement(enhancement) => enhancement.mana_cost,
            ButtonAction::Proceed => 0,
            ButtonAction::MovementEnhancement(..) => 0,
        }
    }

    fn stamina_cost(&self) -> u32 {
        match self {
            ButtonAction::Action(base_action) => 0,
            ButtonAction::OnAttackedReaction(reaction) => reaction.stamina_cost,
            ButtonAction::OnHitReaction(reaction) => 0,
            ButtonAction::AttackEnhancement(enhancement) => enhancement.stamina_cost,
            ButtonAction::SpellEnhancement(enhancement) => 0,
            ButtonAction::Proceed => 0,
            ButtonAction::MovementEnhancement(enhancement) => enhancement.stamina_cost,
        }
    }

    fn unwrap_movement_enhancement(&self) -> MovementEnhancement {
        match self {
            ButtonAction::MovementEnhancement(enhancement) => *enhancement,
            _ => panic!(),
        }
    }
}

struct ActionButton {
    id: u32,
    tooltip_lines: Vec<String>,
    action: ButtonAction,
    size: (f32, f32),
    style: Style,
    hover_border_color: Color,
    points_row: Container,
    hovered: Cell<bool>,
    enabled: Cell<bool>,
    highlighted: Cell<bool>,
    event_sender: Option<EventSender>,
    icon: Texture2D,
}

impl ActionButton {
    fn new(
        action: ButtonAction,
        event_queue: &Rc<RefCell<Vec<InternalUiEvent>>>,
        id: u32,
        icons: &HashMap<IconId, Texture2D>,
        character: Option<&Character>,
    ) -> Self {
        let mut mana_points = 0;
        let mut stamina_points = 0;
        let mut action_points = 0;

        let icon: IconId;
        let mut tooltip_lines = vec![];

        match action {
            ButtonAction::Action(base_action) => match base_action {
                BaseAction::Attack {
                    action_point_cost,
                    hand,
                } => {
                    action_points = action_point_cost;
                    icon = IconId::Attack;

                    let char = character.unwrap();
                    let weapon = char.weapon(hand).unwrap();
                    tooltip_lines.push(format!(
                        "{} attack ({} AP)",
                        weapon.name, weapon.action_point_cost
                    ));
                    tooltip_lines.push(format!("{} damage", weapon.damage));
                }
                BaseAction::SelfEffect(sea) => {
                    action_points = sea.action_point_cost;
                    icon = sea.icon;

                    tooltip_lines.push(format!("{} ({} AP)", sea.name, sea.action_point_cost));
                    tooltip_lines.push(sea.description.to_string());
                }
                BaseAction::CastSpell(spell) => {
                    action_points = spell.action_point_cost;
                    mana_points = spell.mana_cost;
                    icon = spell.icon;

                    tooltip_lines.push(format!(
                        "{} ({} AP, {} mana)",
                        spell.name, spell.action_point_cost, spell.mana_cost
                    ));
                    let mut description = spell.description.to_string();
                    if spell.damage > 0 {
                        description.push_str(&format!(" ({} damage)", spell.damage));
                    }
                    tooltip_lines.push(description);
                }
                BaseAction::Move {
                    action_point_cost,
                    range: _,
                } => {
                    action_points = action_point_cost;
                    icon = IconId::Move;

                    tooltip_lines.push(format!("Movement ({} AP)", action_point_cost));
                }
            },
            ButtonAction::AttackEnhancement(enhancement) => {
                action_points = enhancement.action_point_cost;
                stamina_points = enhancement.stamina_cost;
                icon = enhancement.icon;

                tooltip_lines.push(format!(
                    "{} ({} AP, {} stamina)",
                    enhancement.name, enhancement.action_point_cost, enhancement.stamina_cost
                ));
                tooltip_lines.push(enhancement.description.to_string());
            }

            ButtonAction::SpellEnhancement(enhancement) => {
                mana_points = enhancement.mana_cost;
                icon = enhancement.icon;

                tooltip_lines.push(format!(
                    "{} ({} mana)",
                    enhancement.name, enhancement.mana_cost
                ));
                tooltip_lines.push(enhancement.description.to_string());
            }
            ButtonAction::OnAttackedReaction(reaction) => {
                action_points = reaction.action_point_cost;
                stamina_points = reaction.stamina_cost;
                icon = reaction.icon;

                tooltip_lines.push(format!(
                    "{} ({} AP, {} stamina)",
                    reaction.name, reaction.action_point_cost, reaction.stamina_cost
                ));
                tooltip_lines.push(reaction.description.to_string());
            }
            ButtonAction::OnHitReaction(reaction) => {
                action_points = reaction.action_point_cost;
                icon = reaction.icon;

                tooltip_lines.push(format!(
                    "{} ({} AP)",
                    reaction.name, reaction.action_point_cost
                ));
                tooltip_lines.push(reaction.description.to_string());
            }
            ButtonAction::MovementEnhancement(enhancement) => {
                action_points = enhancement.action_point_cost;
                stamina_points = enhancement.stamina_cost;
                icon = enhancement.icon;

                tooltip_lines.push(format!(
                    "{} ({} AP, {} stamina)",
                    enhancement.name, enhancement.action_point_cost, enhancement.stamina_cost
                ));
                tooltip_lines.push(format!("+{}% range", enhancement.add_percentage));
            }
            ButtonAction::Proceed => {
                icon = IconId::Go;
                tooltip_lines.push("Proceed".to_string());
            }
        }

        let size = (64.0, 64.0);
        let style = Style {
            background_color: Some(DARKGRAY),
            border_color: Some(LIGHTGRAY),
            ..Default::default()
        };
        let hover_border_color = YELLOW;

        let r = 4.0;
        let mut point_icons = vec![];
        let border_width = Some(3.0);
        for _ in 0..action_points {
            point_icons.push(Element::Rect(Rectangle {
                size: (r * 2.0, r * 2.0),
                style: Style {
                    background_color: Some(GOLD),
                    border_color: Some(BLACK),
                    border_width,
                    ..Default::default()
                },
            }))
        }
        for _ in 0..mana_points {
            point_icons.push(Element::Rect(Rectangle {
                size: (r * 2.0, r * 2.0),
                style: Style {
                    background_color: Some(BLUE),
                    border_color: Some(BLACK),
                    border_width,
                    ..Default::default()
                },
            }))
        }
        for _ in 0..stamina_points {
            point_icons.push(Element::Rect(Rectangle {
                size: (r * 2.0, r * 2.0),
                style: Style {
                    background_color: Some(GREEN),
                    border_color: Some(BLACK),
                    border_width,
                    ..Default::default()
                },
            }))
        }
        let points_row = Container {
            children: point_icons,
            margin: 1.0,
            layout_dir: LayoutDirection::Horizontal,
            ..Default::default()
        };

        assert!(!tooltip_lines.is_empty());

        let icon = icons[&icon].clone();
        Self {
            id,
            action,
            size,
            style,
            hover_border_color,
            points_row,
            hovered: Cell::new(false),
            enabled: Cell::new(true),
            highlighted: Cell::new(false),
            event_sender: Some(EventSender {
                queue: Rc::clone(event_queue),
            }),
            icon,
            tooltip_lines,
        }
    }

    fn toggle_highlighted(&self) {
        self.highlighted.set(!self.highlighted.get());
    }

    fn notify_hidden(&self) {
        if self.hovered.get() {
            if let Some(event_sender) = &self.event_sender {
                // Since this button has become hidden, it's no longer hovered
                event_sender.send(InternalUiEvent::ButtonHovered(self.id, self.action, None));
            }
        }
    }
}

impl Drawable for ActionButton {
    fn draw(&self, x: f32, y: f32) {
        let (w, h) = self.size;
        self.style.draw(x, y, self.size);

        let (mouse_x, mouse_y) = mouse_position();

        let hovered = (x..=x + w).contains(&mouse_x) && (y..=y + h).contains(&mouse_y);
        if hovered != self.hovered.get() {
            self.hovered.set(hovered);
            if let Some(event_sender) = &self.event_sender {
                event_sender.send(InternalUiEvent::ButtonHovered(
                    self.id,
                    self.action,
                    if hovered { Some((x, y)) } else { None },
                ));
            }
        }

        if self.enabled.get() {
            if hovered {
                if is_mouse_button_pressed(MouseButton::Left) {
                    if let Some(event_sender) = &self.event_sender {
                        event_sender.send(InternalUiEvent::ButtonClicked(self.id, self.action));
                    }
                }
                draw_rectangle_lines(x, y, w, h, 1.0, self.hover_border_color);
            }
        } else {
            draw_rectangle(x, y, w, h, Color::new(0.2, 0.0, 0.0, 0.3));
            //draw_rectangle_lines(x, y, w, h, 1.0, RED);
        }

        let params = DrawTextureParams {
            dest_size: Some((60.0, 48.0).into()),
            ..Default::default()
        };
        draw_texture_ex(&self.icon, x + 2.0, y + 2.0, WHITE, params);

        if self.highlighted.get() {
            draw_rectangle_lines(x, y, w, h, 2.0, GREEN);
        }

        let margin = 4.0;
        let row_size = self.points_row.size();
        self.points_row
            .draw(x + w - row_size.0 - margin, y + h - margin - row_size.1);

        draw_debug(x, y, w, h);
    }

    fn size(&self) -> (f32, f32) {
        self.size
    }
}

enum InternalUiEvent {
    ButtonHovered(u32, ButtonAction, Option<(f32, f32)>),
    ButtonClicked(u32, ButtonAction),
}

#[derive(Debug)]
pub enum PlayerChose {
    AttackedReaction(Option<OnAttackedReaction>),
    HitReaction(Option<OnHitReaction>),
    Action(Action),
}
