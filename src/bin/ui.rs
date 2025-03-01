use std::{cell::RefCell, rc::Rc};

use macroquad::{
    color::{
        self, Color, BLACK, BLUE, DARKGRAY, DARKGREEN, GOLD, GRAY, GREEN, LIGHTGRAY, MAGENTA,
        ORANGE, PURPLE, RED, WHITE, YELLOW,
    },
    input::{is_key_pressed, is_mouse_button_pressed, mouse_position, MouseButton},
    shapes::{
        draw_circle, draw_circle_lines, draw_line, draw_rectangle, draw_rectangle_ex,
        draw_rectangle_lines, DrawRectangleParams,
    },
    text::{draw_text, measure_text, Font, TextDimensions},
    time::{self, get_frame_time},
    window::{clear_background, next_frame, screen_height, screen_width, Conf},
};
use rpg::core::{
    Action, BaseAction, Character, CoreGame, GameState, Logger, OnAttackedReaction, OnHitReaction,
    SelfEffectAction, Spell, StateReactToAttack, StateReactToHit,
};

#[macroquad::main(window_conf)]
async fn main() {
    let logbuf = Rc::new(RefCell::new(LogBuf(Default::default())));
    let cloned_logbuf = Rc::clone(&logbuf);

    let game_state = CoreGame::new(cloned_logbuf);

    let game = &game_state.game;
    let mut user_interface = UserInterface::new(&game.player_character());

    let mut character_names = CharacterNames::new(
        &[&game.player_character(), &game.non_player_character()],
        game.active_character_i,
    );

    let game_state = GameState::ChooseAction(game_state);

    let mut state = State::AwaitingPlayerInput(game_state);

    change_state(&state, &mut user_interface);

    let mut timer = 0.0;

    loop {
        user_interface.draw();
        character_names.draw(50.0, 20.0);

        match state {
            State::AwaitingPlayerInput(mut game_state) => {
                let events = user_interface.update();

                if !events.is_empty() {
                    for event in events {
                        match event {
                            Event::ChoseAttackedReaction(reaction) => {
                                let GameState::ReactToAttack(react_to_attack) = game_state else {
                                    panic!();
                                };
                                game_state = react_to_attack.proceed(reaction);
                            }
                            Event::ChoseHitReaction(reaction) => {
                                let GameState::ReactToHit(react_to_hit) = game_state else {
                                    panic!();
                                };
                                game_state = react_to_hit.proceed(reaction);
                            }
                            Event::ChoseSelfEffectAction(se_action) => {
                                let GameState::ChooseAction(choose_action) = game_state else {
                                    panic!();
                                };
                                game_state = choose_action.proceed(Action::SelfEffect(se_action));
                            }
                            Event::ChoseCastSpellAction(spell) => {
                                let GameState::ChooseAction(choose_action) = game_state else {
                                    panic!();
                                };
                                // TODO allow enhancing spell
                                game_state = choose_action.proceed(Action::CastSpell {
                                    spell,
                                    enhanced: false,
                                });
                            }
                        }
                    }

                    user_interface
                        .update_character_resources(&game_state.game().player_character());

                    state = State::CatchingUp(game_state);
                    change_state(&state, &mut user_interface);

                    character_names
                        .set_active_character(state.game_state().game().active_character_i);
                    for (i, character) in state.game_state().game().characters().iter().enumerate()
                    {
                        character_names.set_action_points(i, character.borrow().action_points);
                    }
                } else {
                    state = State::AwaitingPlayerInput(game_state)
                }
            }
            State::CatchingUp(game_state) => {
                let events = user_interface.update();
                assert!(
                    events.is_empty(),
                    "Didn't expect events from UI while we're catching up with game"
                );
                state = State::CatchingUp(game_state)
            }
        }

        state = match state {
            State::AwaitingPlayerInput(..) => state,
            State::CatchingUp(game_state) => {
                timer += get_frame_time();

                if logbuf.borrow_mut().0.is_empty() {
                    let new_state = State::AwaitingPlayerInput(game_state);
                    change_state(&new_state, &mut user_interface);
                    new_state
                } else {
                    let tick = 0.1;
                    while timer > tick {
                        timer -= tick;
                        let line = logbuf.borrow_mut().0.remove(0);
                        user_interface.log.add(line);
                    }

                    State::CatchingUp(game_state)
                }
            }
        };

        next_frame().await
    }
}

enum State {
    AwaitingPlayerInput(GameState),
    CatchingUp(GameState),
}

impl State {
    fn game_state(&self) -> &GameState {
        match self {
            State::AwaitingPlayerInput(game_state) => game_state,
            State::CatchingUp(game_state) => game_state,
        }
    }
}

fn change_state(state: &State, user_interface: &mut UserInterface) {
    match state {
        State::AwaitingPlayerInput(game_state) => match game_state {
            GameState::ChooseAction(..) => {
                user_interface
                    .activity_popup
                    .set_state(ActivityPopupState::ChooseAction, vec![]);
            }
            GameState::ReactToAttack(StateReactToAttack { lines, .. }) => {
                user_interface
                    .activity_popup
                    .set_state(ActivityPopupState::ReactToAttack, lines.clone());
            }
            GameState::ReactToHit(StateReactToHit { lines, .. }) => {
                user_interface
                    .activity_popup
                    .set_state(ActivityPopupState::ReactToHit, lines.clone());
            }
        },
        State::CatchingUp(..) => {
            user_interface
                .activity_popup
                .set_state(ActivityPopupState::Idle, vec![]);
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum ActivityPopupState {
    ChooseAction,
    ChooseAttackEnhancements,
    ReactToAttack,
    ReactToHit,
    Idle,
}

struct ActivityPopup {
    state: ActivityPopupState,
    lines: Vec<String>,
    attacked_reaction_popup_buttons: Vec<ActionButton>,
    hit_reaction_popup_buttons: Vec<ActionButton>,
}

impl ActivityPopup {
    fn draw(&mut self, x: f32, y: f32) {
        match &self.state {
            ActivityPopupState::ChooseAction => {
                draw_text("Choose an action!", x, y, 20.0, WHITE);
            }
            ActivityPopupState::ChooseAttackEnhancements => {
                draw_text("Choose attack enhancements", x, y, 20.0, WHITE);
            }
            ActivityPopupState::ReactToAttack => {
                draw_text("react to attack!", x, y, 20.0, WHITE);
                let mut y0 = y;
                for line in &self.lines {
                    y0 += 20.0;
                    draw_text(line, x, y0, 20.0, WHITE);
                }

                let mut x0 = x;
                y0 += 20.0;
                for btn in &mut self.attacked_reaction_popup_buttons {
                    btn.draw(x0, y0);
                    x0 += btn.size.0 + 10.0;
                }
            }
            ActivityPopupState::ReactToHit => {
                draw_text("react to hit!", x, y, 20.0, WHITE);

                let mut y0 = y;
                for line in &self.lines {
                    y0 += 20.0;
                    draw_text(line, x, y0, 20.0, WHITE);
                }

                let mut x0 = x;
                y0 += 20.0;
                for btn in &mut self.hit_reaction_popup_buttons {
                    btn.draw(x0, y0);
                    x0 += btn.size.0 + 10.0;
                }
            }
            ActivityPopupState::Idle => {
                draw_text(".....", x, y, 20.0, WHITE);
            }
        }
    }

    fn set_state(&mut self, state: ActivityPopupState, lines: Vec<String>) {
        dbg!(state);
        self.state = state;
        self.lines = lines;
    }
}

struct UserInterface {
    log: Log,
    tabs: Tabs,
    resource_bars: Container,
    action_points_label: TextLine,
    action_points_row: ActionPointsRow,
    event_queue: Rc<RefCell<Vec<InternalUiEvent>>>,
    health_bar: Rc<RefCell<LabelledResourceBar>>,
    mana_bar: Rc<RefCell<LabelledResourceBar>>,
    stamina_bar: Rc<RefCell<LabelledResourceBar>>,
    activity_popup: ActivityPopup,
    state: ActivityPopupState,
}

impl UserInterface {
    fn new(character_ref: &Character) -> Self {
        let mut combat_buttons = vec![];
        let mut skill_buttons = vec![];
        let mut spell_buttons = vec![];

        let event_queue = Rc::new(RefCell::new(vec![]));

        for (name, action) in character_ref.known_actions() {
            let btn = action_button(name, ButtonAction::Action(action), &event_queue);
            match action {
                BaseAction::Attack { .. } => combat_buttons.push(btn),
                BaseAction::SelfEffect(..) => skill_buttons.push(btn),
                BaseAction::CastSpell(..) => spell_buttons.push(btn),
            }
        }

        let combat_row = buttons_row(combat_buttons);
        let skill_row = buttons_row(skill_buttons);
        let spell_row = buttons_row(spell_buttons);

        let mut reaction_buttons = vec![];

        let mut attacked_reaction_popup_buttons = vec![];
        for (subtext, reaction) in character_ref.known_on_attacked_reactions() {
            let btn_action = ButtonAction::OnAttackedReaction(Some(reaction));
            reaction_buttons.push(action_button(subtext.clone(), btn_action, &event_queue));
            attacked_reaction_popup_buttons.push(action_button(subtext, btn_action, &event_queue))
        }
        if !attacked_reaction_popup_buttons.is_empty() {
            attacked_reaction_popup_buttons.push(action_button(
                "".to_string(),
                ButtonAction::OnAttackedReaction(None),
                &event_queue,
            ))
        }

        let mut hit_reaction_popup_buttons = vec![];
        for (subtext, reaction) in character_ref.known_on_hit_reactions() {
            let btn_action = ButtonAction::OnHitReaction(Some(reaction));
            let btn = action_button(subtext.clone(), btn_action, &event_queue);
            reaction_buttons.push(btn);
            hit_reaction_popup_buttons.push(action_button(subtext, btn_action, &event_queue));
        }
        if !hit_reaction_popup_buttons.is_empty() {
            hit_reaction_popup_buttons.push(action_button(
                "".to_string(),
                ButtonAction::OnHitReaction(None),
                &event_queue,
            ))
        }

        let reactions_row = buttons_row(reaction_buttons);

        let stats_section = Element::Container(Container {
            layout_dir: LayoutDirection::Vertical,
            elements: vec![
                Element::Container(attribute_row(
                    ("STR", character_ref.base_strength),
                    vec![
                        ("Health", character_ref.health.max as f32),
                        (
                            "Physical resist",
                            character_ref.physical_resistence() as f32,
                        ),
                    ],
                )),
                Element::Container(attribute_row(
                    ("DEX", character_ref.base_dexterity),
                    vec![
                        ("Defense", character_ref.defense() as f32),
                        ("Movement", 99.9),
                    ],
                )),
                Element::Container(attribute_row(
                    ("INT", character_ref.base_intellect),
                    vec![
                        ("Mana", character_ref.mana.max as f32),
                        ("Mental resist", character_ref.mental_resistence() as f32),
                    ],
                )),
            ],
            ..Default::default()
        });

        let actions_section = Element::Container(Container {
            layout_dir: LayoutDirection::Vertical,
            margin: 10.0,
            elements: vec![combat_row, skill_row, spell_row],
            ..Default::default()
        });

        let reactions_section = Element::Container(Container {
            layout_dir: LayoutDirection::Vertical,
            margin: 10.0,
            elements: vec![reactions_row],
            ..Default::default()
        });

        let tabs = Tabs::new(
            0,
            vec![
                ("Actions", actions_section),
                ("Reactions", reactions_section),
                ("Stats", stats_section),
            ],
        );

        let health_bar = Rc::new(RefCell::new(LabelledResourceBar::new(
            character_ref.health.current,
            character_ref.health.max,
            "HP",
            RED,
        )));
        let cloned_health_bar = Rc::clone(&health_bar);

        let mana_bar = Rc::new(RefCell::new(LabelledResourceBar::new(
            character_ref.mana.current,
            character_ref.mana.max,
            "MANA",
            BLUE,
        )));
        let cloned_mana_bar = Rc::clone(&mana_bar);

        let stamina_bar = Rc::new(RefCell::new(LabelledResourceBar::new(
            character_ref.stamina.current,
            character_ref.stamina.max,
            "STA",
            GREEN,
        )));
        let cloned_stamina_bar = Rc::clone(&stamina_bar);

        let resource_bars = Container {
            layout_dir: LayoutDirection::Horizontal,
            margin: 15.0,
            align: Align::End,
            elements: vec![
                Element::Rc(cloned_health_bar),
                Element::Rc(cloned_mana_bar),
                Element::Rc(cloned_stamina_bar),
            ],
            ..Default::default()
        };

        let action_points_label = TextLine::new("Action points", 18);
        let action_points_row = ActionPointsRow::new(character_ref.action_points);

        let state = ActivityPopupState::ChooseAction;

        Self {
            log: Log::new(),
            tabs,
            resource_bars,
            action_points_label,
            action_points_row,
            event_queue,
            health_bar,
            mana_bar,
            stamina_bar,
            activity_popup: ActivityPopup {
                state,
                lines: Default::default(),
                attacked_reaction_popup_buttons,
                hit_reaction_popup_buttons,
            },
            state,
        }
    }

    fn draw(&mut self) {
        clear_background(BLACK);

        let y0 = 600.0;

        self.activity_popup.draw(100.0, y0 - 140.0);

        draw_line(0.0, y0, window_conf().window_width as f32, y0, 1.0, WHITE);
        self.tabs.draw(20.0, y0 + 100.0);
        self.resource_bars.draw(500.0, y0 + 150.0);
        self.action_points_label.draw(20.0, y0 + 10.0);
        self.action_points_row.draw(20.0, y0 + 30.0);
        self.log.draw(800.0, y0);
    }

    fn set_state(&mut self, state: ActivityPopupState, lines: Vec<String>) {
        self.state = state;
        self.activity_popup.set_state(state, lines);
    }

    fn update(&mut self) -> Vec<Event> {
        self.event_queue
            .take()
            .into_iter()
            .filter_map(|event| self.handle_event(event))
            .collect()
    }

    fn handle_event(&mut self, event: InternalUiEvent) -> Option<Event> {
        let can_choose_action = self.state == ActivityPopupState::ChooseAction
            || self.state == ActivityPopupState::ChooseAttackEnhancements;
        let ap = self.action_points_row.current;
        let stamina = self.stamina_bar.borrow().current();
        let mana = self.mana_bar.borrow().current();

        match event {
            InternalUiEvent::Hover(hovered, button_action) => {
                let reserved_ap = if hovered {
                    button_action.action_point_cost()
                } else {
                    0
                };
                self.set_reserved_action_points(reserved_ap);
                None
            }

            InternalUiEvent::Click(btn_action) => match btn_action {
                ButtonAction::Action(BaseAction::Attack {
                    hand: _,
                    action_point_cost,
                }) => {
                    if can_choose_action && ap >= action_point_cost {
                        self.set_state(ActivityPopupState::ChooseAttackEnhancements, vec![]);
                    } else {
                        println!("Cannot choose attack action at this time");
                    }
                    None
                }
                ButtonAction::Action(BaseAction::SelfEffect(se)) => {
                    if can_choose_action && ap >= se.action_point_cost {
                        Some(Event::ChoseSelfEffectAction(se))
                    } else {
                        println!("Cannot use this self effect at this time");
                        None
                    }
                }
                ButtonAction::Action(BaseAction::CastSpell(spell)) => {
                    if can_choose_action
                        && ap >= spell.action_point_cost
                        && mana >= spell.mana_cost
                    {
                        Some(Event::ChoseCastSpellAction(spell))
                    } else {
                        println!("Cannot cast this spell at this time");
                        None
                    }
                }
                ButtonAction::OnAttackedReaction(reaction) => {
                    if self.state == ActivityPopupState::ReactToAttack {
                        let is_valid_choice = reaction
                            .map(|reaction| {
                                ap >= reaction.action_point_cost && stamina >= reaction.stamina_cost
                            })
                            .unwrap_or(true);
                        if is_valid_choice {
                            Some(Event::ChoseAttackedReaction(reaction))
                        } else {
                            println!("Cannot afford this reaction at this time");
                            None
                        }
                    } else {
                        println!("Cannot use this reaction at this time");
                        None
                    }
                }
                ButtonAction::OnHitReaction(reaction) => {
                    if self.state == ActivityPopupState::ReactToHit {
                        let is_valid_choice = reaction
                            .map(|reaction| ap >= reaction.action_point_cost)
                            .unwrap_or(true);
                        if is_valid_choice {
                            Some(Event::ChoseHitReaction(reaction))
                        } else {
                            println!("Cannot afford this reaction at this time");
                            None
                        }
                    } else {
                        println!("Cannot use this reaction at this time");
                        None
                    }
                }
            },
        }
    }

    fn set_reserved_action_points(&mut self, reserved: u32) {
        self.action_points_row.reserved = reserved;
    }

    fn update_character_resources(&mut self, character: &Character) {
        self.health_bar
            .borrow_mut()
            .set_current(character.health.current);
        self.mana_bar
            .borrow_mut()
            .set_current(character.mana.current);
        self.stamina_bar
            .borrow_mut()
            .set_current(character.stamina.current);
        self.action_points_row.current = character.action_points;
    }
}

struct CharacterNames {
    row: Container,
    active_i: usize,
    character_names: Vec<Rc<RefCell<CharacterName>>>,
}

impl CharacterNames {
    fn new(characters: &[&Character], active_i: usize) -> Self {
        let character_names: Vec<Rc<RefCell<CharacterName>>> = characters
            .iter()
            .map(|character| {
                Rc::new(RefCell::new(CharacterName::new(
                    character.name,
                    character.action_points,
                )))
            })
            .collect();

        let mut elements = vec![];
        for char_name in &character_names {
            let cloned = Rc::clone(char_name);
            elements.push(Element::Rc(cloned));
        }

        let row = Container {
            layout_dir: LayoutDirection::Horizontal,
            margin: 10.0,
            elements,
            ..Default::default()
        };

        let mut this = Self {
            row,
            active_i,
            character_names,
        };

        this.set_active_character(active_i);
        this
    }

    fn set_active_character(&mut self, character_i: usize) {
        self.character_names[self.active_i].borrow_mut().active = false;
        self.active_i = character_i;
        self.character_names[self.active_i].borrow_mut().active = true;
    }

    fn set_action_points(&mut self, character_i: usize, action_points: u32) {
        self.character_names[character_i].borrow_mut().action_points = action_points;
    }

    fn draw(&mut self, x: f32, y: f32) {
        self.row.draw(x, y)
    }
}

struct CharacterName {
    text: TextLine,
    active: bool,
    padding: f32,
    action_points: u32,
}

impl CharacterName {
    fn new(name: impl Into<String>, action_points: u32) -> Self {
        Self {
            text: TextLine::new(name, 20),
            active: false,
            padding: 15.0,
            action_points,
        }
    }
}

impl Drawable for CharacterName {
    fn draw(&mut self, x: f32, y: f32) {
        if self.active {
            let (w, h) = self.size();
            draw_rectangle_lines(x, y, w, h, 2.0, GOLD);
        }
        self.text.draw(self.padding + x, self.padding + y);
        draw_text(
            &format!("{} AP", self.action_points),
            self.padding + x,
            y + 60.0,
            20.0,
            WHITE,
        );
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

struct LogBuf(Vec<String>);

impl Logger for LogBuf {
    fn log(&mut self, line: String) {
        self.0.push(line);
    }
}

struct Log {
    lines: Container,
}

impl Log {
    fn new() -> Self {
        let elements = vec![];
        Self {
            lines: Container {
                layout_dir: LayoutDirection::Vertical,
                style: Style {
                    border_color: Some(WHITE),
                    ..Default::default()
                },
                elements,
                padding: 10.0,
                margin: 5.0,
                ..Default::default()
            },
        }
    }

    fn add(&mut self, text: impl Into<String>) {
        if self.lines.elements.len() == 15 {
            self.lines.elements.remove(0);
        }
        self.lines
            .elements
            .push(Element::Text(TextLine::new(text, 18)));
    }

    fn draw(&mut self, x: f32, y: f32) {
        self.lines.draw(x, y);
    }
}

struct ActionPointsRow {
    current: u32,
    reserved: u32,
    max: u32,
    cell_size: (f32, f32),
    padding: f32,
}

impl ActionPointsRow {
    fn new(action_points: u32) -> Self {
        Self {
            current: action_points,
            reserved: 0,
            max: 6,
            cell_size: (20.0, 20.0),
            padding: 3.0,
        }
    }

    fn draw(&mut self, x: f32, y: f32) {
        //assert!(self.reserved <= self.current);
        assert!(self.current <= self.max);

        let mut x0 = x + self.padding;
        let y0 = y + self.padding;
        let r = self.cell_size.1 * 0.3;
        for i in 0..self.max {
            if i < self.current.saturating_sub(self.reserved) {
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
            } else if i < self.reserved {
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

        draw_rectangle_lines(
            x,
            y,
            self.max as f32 * self.cell_size.0 + self.padding * 2.0,
            self.cell_size.1 + self.padding * 2.0,
            1.0,
            WHITE,
        );
    }
}

trait Drawable {
    fn draw(&mut self, x: f32, y: f32);
    fn size(&self) -> (f32, f32);
}

struct ResourceBar {
    current: u32,
    max: u32,
    color: Color,
    cell_size: (f32, f32),
}

impl Drawable for ResourceBar {
    fn draw(&mut self, x: f32, y: f32) {
        let cell_size = self.cell_size;
        let mut y0 = y;
        for i in 0..self.max {
            if i >= self.max - self.current {
                draw_rectangle(x, y0, cell_size.0, cell_size.1, self.color);
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
    fn new(current: u32, max: u32, label: &'static str, color: Color) -> Self {
        let bar = Rc::new(RefCell::new(ResourceBar {
            current,
            max,
            color,
            cell_size: (20.0, 15.0),
        }));
        let cloned_bar = Rc::clone(&bar);

        let value_text = Rc::new(RefCell::new(TextLine::new(
            format!("{}/{}", current, max),
            18,
        )));
        let cloned_value_text = Rc::clone(&value_text);
        let label_text = TextLine::new(label, 18);

        let list = Container {
            layout_dir: LayoutDirection::Vertical,
            align: Align::Center,
            margin: 5.0,
            elements: vec![
                Element::Rc(cloned_bar),
                Element::Rc(cloned_value_text),
                Element::Text(label_text),
            ],
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
        self.bar.borrow_mut().current = value;
        self.value_text
            .borrow_mut()
            .set_string(format!("{}/{}", value, self.max_value));
    }

    fn current(&self) -> u32 {
        self.bar.borrow().current
    }
}

impl Drawable for LabelledResourceBar {
    fn draw(&mut self, x: f32, y: f32) {
        self.list.draw(x, y)
    }

    fn size(&self) -> (f32, f32) {
        self.list.size()
    }
}

fn buttons_row(buttons: Vec<ActionButton>) -> Element {
    let elements = buttons.into_iter().map(Element::Btn).collect();
    Element::Container(Container {
        layout_dir: LayoutDirection::Horizontal,
        margin: 10.0,
        elements,
        ..Default::default()
    })
}

fn action_button(
    mut subtext: String,
    action: ButtonAction,
    event_queue: &Rc<RefCell<Vec<InternalUiEvent>>>,
) -> ActionButton {
    let button_size = (100.0, 50.0);
    let highlight_color = YELLOW;
    let btn_style = Style {
        background_color: Some(DARKGRAY),
        border_color: Some(LIGHTGRAY),
    };

    let text;
    let mut mana_points = 0;
    let mut stamina_points = 0;
    let mut action_points = 0;

    match action {
        ButtonAction::Action(base_action) => match base_action {
            BaseAction::Attack {
                action_point_cost, ..
            } => {
                text = "Attack";
                action_points = action_point_cost;
            }
            BaseAction::SelfEffect(self_effect_action) => {
                text = self_effect_action.name;
                action_points = self_effect_action.action_point_cost;
            }
            BaseAction::CastSpell(spell) => {
                text = spell.name;
                subtext = "(Spell)".to_string();
                action_points = spell.action_point_cost;
                mana_points = spell.mana_cost;
            }
        },
        ButtonAction::OnAttackedReaction(Some(reaction)) => {
            text = reaction.name;
            action_points = reaction.action_point_cost;
            stamina_points = reaction.stamina_cost;
        }
        ButtonAction::OnAttackedReaction(None) => {
            text = "Skip";
        }
        ButtonAction::OnHitReaction(Some(reaction)) => {
            text = reaction.name;
            action_points = reaction.action_point_cost;
        }
        ButtonAction::OnHitReaction(None) => {
            text = "Skip";
        }
    }

    let mut btn = ActionButton::new(
        action,
        button_size,
        btn_style,
        text,
        subtext,
        highlight_color,
        action_points,
        mana_points,
        stamina_points,
    );

    btn.event_sender = Some(EventSender {
        queue: Rc::clone(event_queue),
    });
    btn
}

fn attribute_row(attribute: (&'static str, u32), stats: Vec<(&'static str, f32)>) -> Container {
    let attribute_element = Element::Text(TextLine::new(
        format!("{}: {}", attribute.0, attribute.1),
        22,
    ));

    let stat_rows: Vec<Element> = stats
        .iter()
        .map(|(name, value)| Element::Text(TextLine::new(format!("{} = {}", name, value), 18)))
        .collect();

    let stats_list = Element::Container(Container {
        layout_dir: LayoutDirection::Vertical,
        margin: 4.0,
        elements: stat_rows,
        ..Default::default()
    });
    Container {
        layout_dir: LayoutDirection::Horizontal,
        padding: 5.0,
        margin: 20.0,
        align: Align::Center,
        style: Style {
            border_color: Some(GRAY),
            ..Default::default()
        },
        elements: vec![attribute_element, stats_list],
    }
}

#[derive(Default, Copy, Clone)]
struct Style {
    background_color: Option<Color>,
    border_color: Option<Color>,
}

impl Style {
    fn draw(&self, x: f32, y: f32, size: (f32, f32)) {
        if let Some(color) = self.background_color {
            draw_rectangle(x, y, size.0, size.1, color);
        }
        if let Some(color) = self.border_color {
            draw_rectangle_lines(x, y, size.0, size.1, 1.0, color);
        }
    }
}

enum Element {
    Btn(ActionButton),
    Container(Container),
    Text(TextLine),
    Circle(Circle),
    Rect(Rectangle),
    TabLink(TabLink),
    Box(Box<dyn Drawable>),
    Rc(Rc<RefCell<dyn Drawable>>),
}

impl Element {
    fn size(&self) -> (f32, f32) {
        let size = match self {
            Element::Btn(btn) => btn.size,
            Element::Container(container) => container.size(),
            Element::Text(text) => text.size,
            Element::Circle(circle) => (circle.r * 2.0, circle.r * 2.0),
            Element::Rect(rect) => rect.size,
            Element::TabLink(link) => link.size,
            Element::Box(drawable) => drawable.size(),
            Element::Rc(drawable) => drawable.borrow().size(),
        };

        assert!(size.0.is_finite() && size.1.is_finite());
        size
    }

    fn draw(&mut self, x: f32, y: f32) {
        match self {
            Element::Btn(btn) => btn.draw(x, y),
            Element::Container(container) => container.draw(x, y),
            Element::Text(text) => text.draw(x, y),
            Element::Circle(circle) => circle.draw(x, y),
            Element::Rect(rect) => rect.draw(x, y),
            Element::TabLink(link) => link.draw(x, y),
            Element::Box(drawable) => drawable.draw(x, y),
            Element::Rc(drawable) => drawable.borrow_mut().draw(x, y),
        }
    }

    fn unwrap_tab_link(&mut self) -> &mut TabLink {
        match self {
            Element::TabLink(tab_link) => tab_link,
            _ => panic!("Unexpected variant"),
        }
    }
}

struct Tabs {
    links_row: Container,
    tabs: Vec<Element>,
    active_i: usize,
}

impl Tabs {
    fn new(active_i: usize, links_and_tabs: Vec<(&'static str, Element)>) -> Self {
        let mut links: Vec<TabLink> = links_and_tabs.iter().map(|t| TabLink::new(t.0)).collect();

        links[active_i].active = true;
        let links_row = Container {
            layout_dir: LayoutDirection::Horizontal,
            elements: links.into_iter().map(Element::TabLink).collect(),
            ..Default::default()
        };

        let tabs: Vec<Element> = links_and_tabs.into_iter().map(|t| t.1).collect();
        Self {
            links_row,
            tabs,
            active_i,
        }
    }

    fn draw(&mut self, x: f32, y: f32) {
        // If a link was clicked, update the state of all links
        let mut maybe_clicked_i = None;
        for (i, element) in self.links_row.elements.iter_mut().enumerate() {
            if element.unwrap_tab_link().was_clicked {
                maybe_clicked_i = Some(i);
                self.active_i = i;
                break;
            }
        }
        if let Some(clicked_i) = maybe_clicked_i {
            for (i, element) in self.links_row.elements.iter_mut().enumerate() {
                let tab_link = element.unwrap_tab_link();
                tab_link.was_clicked = false;
                tab_link.active = i == clicked_i;
            }
        }

        self.links_row.draw(x, y);

        self.tabs[self.active_i].draw(x, y + 50.0);
    }
}

struct TabLink {
    text: TextLine,
    active: bool,
    padding: f32,
    size: (f32, f32),
    was_clicked: bool,
}

impl TabLink {
    fn new(text: impl Into<String>) -> Self {
        let text = TextLine::new(text, 20);
        let padding = 5.0;
        let text_size = text.size;
        Self {
            text,
            active: false,
            padding,
            size: (padding * 2.0 + text_size.0, padding * 2.0 + text_size.1),
            was_clicked: false,
        }
    }

    fn draw(&mut self, x: f32, y: f32) {
        if self.active {
            draw_rectangle(x, y, self.size.0, self.size.1, DARKGREEN);
        }

        if is_mouse_button_pressed(MouseButton::Left) {
            let (mouse_x, mouse_y) = mouse_position();
            if (x..=x + self.size.0).contains(&mouse_x) && (y..=y + self.size.1).contains(&mouse_y)
            {
                self.was_clicked = true;
            }
        }

        self.text.draw(x + self.padding, y + self.padding);
    }
}

enum LayoutDirection {
    Horizontal,
    Vertical,
}

impl Default for LayoutDirection {
    fn default() -> Self {
        Self::Horizontal
    }
}

enum Align {
    Start,
    Center,
    End,
}

impl Default for Align {
    fn default() -> Self {
        Self::Start
    }
}

#[derive(Default)]
struct Container {
    layout_dir: LayoutDirection,
    align: Align,
    padding: f32,
    margin: f32,
    style: Style,
    elements: Vec<Element>,
}

impl Container {
    fn size(&self) -> (f32, f32) {
        let mut w = 0.0;
        let mut h = 0.0;
        for element in &self.elements {
            let size = element.size();

            match self.layout_dir {
                LayoutDirection::Horizontal => {
                    w += size.0;
                    if size.1 > h {
                        h = size.1;
                    }
                }
                LayoutDirection::Vertical => {
                    h += size.1;
                    if size.0 > w {
                        w = size.0;
                    }
                }
            }
        }

        w += self.padding * 2.0;
        h += self.padding * 2.0;

        if !self.elements.is_empty() {
            let total_margin = (self.elements.len() - 1) as f32 * self.margin;
            match self.layout_dir {
                LayoutDirection::Horizontal => w += total_margin,
                LayoutDirection::Vertical => h += total_margin,
            }
        }

        (w, h)
    }

    fn draw(&mut self, x: f32, y: f32) {
        let size = self.size();
        self.style.draw(x, y, size);

        let mut x0 = x + self.padding;
        let mut y0 = y + self.padding;
        for element in &mut self.elements {
            let (element_w, element_h) = element.size();

            let offset = match (&self.align, &self.layout_dir) {
                (Align::Start, _) => (0.0, 0.0),
                (Align::Center, LayoutDirection::Horizontal) => {
                    // Place it in the middle, i.e. empty space above and below
                    (0.0, (size.1 - 2.0 * self.padding - element_h) / 2.0)
                }
                (Align::Center, LayoutDirection::Vertical) => {
                    // Place it in the middle, i.e. empty space to the left and right
                    ((size.0 - 2.0 * self.padding - element_w) / 2.0, 0.0)
                }
                (Align::End, LayoutDirection::Horizontal) => {
                    // Push it down so that it touches the bottom
                    (0.0, size.1 - 2.0 * self.padding - element_h)
                }
                (Align::End, LayoutDirection::Vertical) => {
                    // Push it to the right, so that it touches the right side
                    (size.0 - 2.0 * self.padding - element_w, 0.0)
                }
            };

            element.draw(x0 + offset.0, y0 + offset.1);

            match self.layout_dir {
                LayoutDirection::Horizontal => x0 += element_w + self.margin,
                LayoutDirection::Vertical => y0 += element_h + self.margin,
            }
        }

        draw_debug(x, y, size.0, size.1);
    }
}

struct TextLine {
    size: (f32, f32),
    string: String,
    offset_y: f32,
    font_size: u16,
}

impl TextLine {
    fn new(string: impl Into<String>, font_size: u16) -> Self {
        let mut this = Self {
            size: (0.0, 0.0),
            string: "".to_string(),
            offset_y: 0.0,
            font_size,
        };
        this.set_string(string);
        this
    }

    fn set_string(&mut self, string: impl Into<String>) {
        let mut string = string.into();
        if string.is_empty() {
            string.push_str("~~~");
        }
        let text_dimensions = measure_text(&string, None, self.font_size, 1.0);
        self.string = string;
        self.size = (
            text_dimensions.width.max(0.0),
            text_dimensions.height.max(0.0),
        );
        assert!(self.size.0.is_finite() && self.size.1.is_finite());
        self.offset_y = text_dimensions.offset_y;
    }
}

impl Drawable for TextLine {
    fn draw(&mut self, x: f32, y: f32) {
        draw_text(
            &self.string,
            x,
            y + self.offset_y,
            self.font_size as f32,
            WHITE,
        );
        draw_debug(x, y, self.size.0, self.size.1);
    }

    fn size(&self) -> (f32, f32) {
        self.size
    }
}

#[derive(Debug, Copy, Clone)]
enum ButtonAction {
    Action(BaseAction),
    OnAttackedReaction(Option<OnAttackedReaction>),
    OnHitReaction(Option<OnHitReaction>),
}

impl ButtonAction {
    fn action_point_cost(&self) -> u32 {
        match self {
            ButtonAction::Action(base_action) => base_action.action_point_cost(),
            ButtonAction::OnAttackedReaction(reaction) => reaction
                .map(|reaction| reaction.action_point_cost)
                .unwrap_or(0),
            ButtonAction::OnHitReaction(reaction) => reaction
                .map(|reaction| reaction.action_point_cost)
                .unwrap_or(0),
        }
    }
}

struct ActionButton {
    action: ButtonAction,
    size: (f32, f32),
    style: Style,
    content: Box<Element>,
    highlight_border_color: Color,
    points_row: Container,
    point_radius: f32,
    hovered: bool,
    event_sender: Option<EventSender>,
}

impl ActionButton {
    fn new(
        action: ButtonAction,
        size: (f32, f32),
        style: Style,
        text: impl Into<String>,
        subtext: impl Into<String>,
        highlight_border_color: Color,
        action_points: u32,
        mana_points: u32,
        stamina_points: u32,
    ) -> Self {
        let r = 4.0;
        let mut point_icons = vec![];
        for _ in 0..action_points {
            point_icons.push(Element::Rect(Rectangle {
                size: (r * 2.0, r * 2.0),
                style: Style {
                    background_color: Some(color::GOLD),
                    border_color: Some(BLACK),
                },
            }))
        }
        for _ in 0..mana_points {
            point_icons.push(Element::Rect(Rectangle {
                size: (r * 2.0, r * 2.0),
                style: Style {
                    background_color: Some(BLUE),
                    border_color: Some(BLACK),
                },
            }))
        }
        for _ in 0..stamina_points {
            point_icons.push(Element::Rect(Rectangle {
                size: (r * 2.0, r * 2.0),
                style: Style {
                    background_color: Some(GREEN),
                    border_color: Some(BLACK),
                },
            }))
        }
        let points_row = Container {
            elements: point_icons,
            margin: 2.0,
            layout_dir: LayoutDirection::Horizontal,
            ..Default::default()
        };

        let text = Element::Text(TextLine::new(text, 20));
        let subtext = subtext.into();
        let content = if !subtext.is_empty() {
            Box::new(Element::Container(Container {
                layout_dir: LayoutDirection::Vertical,
                margin: 8.0,
                align: Align::Center,
                elements: vec![text, Element::Text(TextLine::new(subtext, 16))],
                ..Default::default()
            }))
        } else {
            Box::new(text)
        };

        Self {
            action,
            size,
            style,
            content,
            highlight_border_color,
            points_row,
            point_radius: r,
            hovered: false,
            event_sender: None,
        }
    }

    fn draw(&mut self, x: f32, y: f32) {
        let (w, h) = self.size;
        self.style.draw(x, y, self.size);

        let (mouse_x, mouse_y) = mouse_position();

        let hovered = (x..=x + w).contains(&mouse_x) && (y..=y + h).contains(&mouse_y);
        if hovered != self.hovered {
            self.hovered = hovered;
            if let Some(event_sender) = &self.event_sender {
                event_sender.send(InternalUiEvent::Hover(hovered, self.action));
            }
        }

        if hovered && is_mouse_button_pressed(MouseButton::Left) {
            if let Some(event_sender) = &self.event_sender {
                event_sender.send(InternalUiEvent::Click(self.action));
            }
        }

        if hovered {
            draw_rectangle_lines(x, y, w, h, 1.0, self.highlight_border_color);
        }

        let margin_x = (w - self.content.size().0) / 2.0;
        let margin_y = (h - self.point_radius * 2.0 - self.content.size().1) / 2.0;
        self.content.draw(x + margin_x, y + margin_y);

        let margin = 4.0;
        let row_size = self.points_row.size();
        self.points_row
            .draw(x + w - row_size.0 - margin, y + h - margin - row_size.1);

        draw_debug(x, y, w, h);
    }
}

enum InternalUiEvent {
    Hover(bool, ButtonAction),
    Click(ButtonAction),
}

enum Event {
    ChoseAttackedReaction(Option<OnAttackedReaction>),
    ChoseHitReaction(Option<OnHitReaction>),
    ChoseSelfEffectAction(SelfEffectAction),
    ChoseCastSpellAction(Spell),
}

struct Circle {
    r: f32,
    color: Color,
}

impl Circle {
    fn draw(&self, x: f32, y: f32) {
        draw_circle(x + self.r, y + self.r, self.r, self.color);
        draw_circle_lines(x + self.r, y + self.r, self.r, 1.0, WHITE);
    }
}

struct Rectangle {
    size: (f32, f32),
    style: Style,
}

impl Rectangle {
    fn draw(&self, x: f32, y: f32) {
        self.style.draw(x, y, self.size);
    }
}

fn draw_debug(x: f32, y: f32, w: f32, h: f32) {
    if false {
        draw_rectangle_lines(x, y, w, h, 1.0, MAGENTA);
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "UI test".to_owned(),
        window_width: 1600,
        window_height: 1200,
        high_dpi: true,
        ..Default::default()
    }
}
