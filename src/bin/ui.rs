use std::{
    cell::{self, Ref, RefCell},
    rc::Rc,
};

use macroquad::{
    color::{
        self, Color, BLACK, BLUE, DARKBROWN, DARKGRAY, DARKGREEN, GOLD, GRAY, GREEN, LIGHTGRAY,
        MAGENTA, ORANGE, PINK, PURPLE, RED, WHITE, YELLOW,
    },
    input::{
        is_key_pressed, is_mouse_button_down, is_mouse_button_pressed, mouse_position, MouseButton,
    },
    miniquad,
    rand::{self, ChooseRandom},
    shapes::{
        draw_circle, draw_circle_lines, draw_line, draw_rectangle, draw_rectangle_ex,
        draw_rectangle_lines, DrawRectangleParams,
    },
    text::{draw_text, measure_text, Font, TextDimensions},
    time::{self, get_frame_time},
    window::{clear_background, next_frame, screen_height, screen_width, Conf},
};
use rpg::core::{
    Action, AttackEnhancement, BaseAction, Character, CoreGame, GameState, Hand, HandType, Logger,
    OnAttackedReaction, OnHitReaction, SelfEffectAction, Spell, SpellEnhancement,
    StateChooseAction, StateReactToAttack, StateReactToHit,
};

#[macroquad::main(window_conf)]
async fn main() {
    // Seed the random numbers
    rand::srand(miniquad::date::now() as u64);

    let logbuf = Rc::new(RefCell::new(LogBuf(Default::default())));
    let cloned_logbuf = Rc::clone(&logbuf);

    let game = CoreGame::new(cloned_logbuf);

    let player_character = Rc::clone(game.player_character());

    let mut ui_characters = vec![];
    for character in game.characters() {
        ui_characters.push(Rc::clone(character));
    }

    let mut user_interface = UserInterface::new(ui_characters);

    let mut character_names = CharacterPortraits::new(game.characters(), game.active_character_i);

    let mut state = match game.begin() {
        game_state @ GameState::AwaitingBot(..) => State::CatchingUp(game_state),
        game_state @ _ => State::AwaitingPlayerInput(game_state),
    };

    change_state(&state, &mut user_interface);

    let mut timer = 0.0;

    loop {
        clear_background(BLACK);
        character_names.draw(50.0, 20.0);

        user_interface.draw(640.0);

        match state {
            State::AwaitingPlayerInput(mut game_state) => {
                let events = user_interface.update();

                if !events.is_empty() {
                    for event in events {
                        match event {
                            Event::ChoseAttackedReaction(reaction) => {
                                game_state = game_state.unwrap_react_to_attack().proceed(reaction);
                            }
                            Event::ChoseHitReaction(reaction) => {
                                game_state = game_state.unwrap_react_to_hit().proceed(reaction);
                            }
                            Event::ChoseAction(action) => {
                                game_state = game_state.unwrap_choose_action().proceed(action);
                            }
                        }
                    }

                    user_interface.update_character_resources(&player_character.borrow());

                    state = State::CatchingUp(game_state);
                    change_state(&state, &mut user_interface);
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

        character_names.update(state.game_state().game());

        user_interface.update_character_positions();

        state = match state {
            State::AwaitingPlayerInput(..) => state,
            State::CatchingUp(game_state) => {
                timer += get_frame_time();

                if logbuf.borrow_mut().0.is_empty() {
                    if let GameState::AwaitingBot(awaiting_bot) = game_state {
                        let new_game_state = awaiting_bot.proceed();
                        State::CatchingUp(new_game_state)
                    } else {
                        let new_state = State::AwaitingPlayerInput(game_state);
                        change_state(&new_state, &mut user_interface);
                        new_state
                    }
                } else {
                    let tick = 0.2;
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

struct GameGrid {
    characters: Vec<(TextLine, (i32, i32))>,
    movement_preview: Option<(i32, i32)>,
    target_character_i: Option<usize>,
}

impl GameGrid {
    fn new(characters: Vec<(impl Into<String>, (u32, u32))>) -> Self {
        let characters = characters
            .into_iter()
            .map(|(s, pos)| (TextLine::new(s, 25), (pos.0 as i32, pos.1 as i32)))
            .collect();

        Self {
            characters,
            movement_preview: None,
            target_character_i: None,
        }
    }

    fn update_positions(&mut self, characters: &[Rc<RefCell<Character>>]) {
        for (i, character) in characters.iter().enumerate() {
            let pos = character.borrow().position;
            self.characters[i].1 = (pos.0 as i32, pos.1 as i32);
        }
    }

    fn draw(&mut self, x: f32, y: f32) -> Option<(i32, i32)> {
        let cell_w = 25.0;
        let grid_dimensions = (20, 12);

        let grid_x_to_screen = |grid_x: i32| x + grid_x as f32 * cell_w;
        let grid_y_to_screen = |grid_y: i32| y + grid_y as f32 * cell_w;

        let draw_square = |(grid_x, grid_y), color| {
            draw_rectangle_lines(
                grid_x_to_screen(grid_x),
                grid_y_to_screen(grid_y),
                cell_w,
                cell_w,
                1.0,
                color,
            )
        };

        let mut tell_ui_to_enter_move_state = None;

        for col in 0..grid_dimensions.0 + 1 {
            let x0 = grid_x_to_screen(col);
            draw_line(
                x0,
                grid_y_to_screen(0),
                x0,
                grid_y_to_screen(grid_dimensions.1),
                1.0,
                DARKGRAY,
            );
            for row in 0..grid_dimensions.1 + 1 {
                let y0 = grid_y_to_screen(row);
                draw_line(
                    grid_x_to_screen(0),
                    y0,
                    grid_x_to_screen(grid_dimensions.0),
                    y0,
                    1.0,
                    DARKGRAY,
                );
            }
        }

        let text_margin = 5.0;
        for (text, position) in &mut self.characters {
            text.draw(
                grid_x_to_screen(position.0) + text_margin,
                grid_y_to_screen(position.1) + text_margin,
            );
        }

        let (mouse_x, mouse_y) = mouse_position();
        let mouse_local = (mouse_x - x, mouse_y - y);

        // TODO: don't assume that player is the first in the vec
        let (player_x, player_y) = self.characters[0].1;

        let mut character_positions = vec![];
        for (_, pos) in &self.characters {
            character_positions.push(*pos);
        }

        if (0f32..grid_dimensions.0 as f32 * cell_w).contains(&mouse_local.0)
            && (0f32..grid_dimensions.1 as f32 * cell_w).contains(&mouse_local.1)
        {
            let (mouse_grid_x, mouse_grid_y) = (
                (mouse_local.0 / cell_w) as i32,
                (mouse_local.1 / cell_w) as i32,
            );

            let dx = mouse_grid_x - player_x;
            let dy = mouse_grid_y - player_y;

            let collision = character_positions.contains(&(mouse_grid_x, mouse_grid_y));
            let valid_move_destination = dx.abs() <= 1 && dy.abs() <= 1 && !collision;

            let mut hovered_character_i = None;
            for (i, (_name, pos)) in self.characters.iter().enumerate() {
                if *pos == (mouse_grid_x, mouse_grid_y) {
                    hovered_character_i = Some(i);
                }
            }

            if valid_move_destination {
                draw_square((mouse_grid_x, mouse_grid_y), YELLOW);
            } else if self.movement_preview.is_some() {
                draw_square((mouse_grid_x, mouse_grid_y), RED);
            } else if self.target_character_i.is_some() {
                if let Some(character_i) = hovered_character_i {
                    draw_square((mouse_grid_x, mouse_grid_y), MAGENTA);
                    if is_mouse_button_down(MouseButton::Left) {
                        self.target_character_i = Some(character_i);
                    }
                }
            }

            if is_mouse_button_down(MouseButton::Left) && valid_move_destination {
                if self.movement_preview.is_none() {
                    tell_ui_to_enter_move_state = Some((dx, dy));
                }
                self.movement_preview = Some((dx, dy));
            }
        }

        if let Some(movement_preview) = self.movement_preview {
            let arrow_color = ORANGE;
            draw_line(
                grid_x_to_screen(player_x) + cell_w / 2.0,
                grid_y_to_screen(player_y) + cell_w / 2.0,
                grid_x_to_screen(player_x + movement_preview.0) + cell_w / 2.0,
                grid_y_to_screen(player_y + movement_preview.1) + cell_w / 2.0,
                1.0,
                arrow_color,
            );
            draw_line(
                grid_x_to_screen(player_x + movement_preview.0) + cell_w * 0.3,
                grid_y_to_screen(player_y + movement_preview.1) + cell_w * 0.3,
                grid_x_to_screen(player_x + movement_preview.0) + cell_w * 0.7,
                grid_y_to_screen(player_y + movement_preview.1) + cell_w * 0.7,
                3.0,
                arrow_color,
            );
            draw_line(
                grid_x_to_screen(player_x + movement_preview.0) + cell_w * 0.3,
                grid_y_to_screen(player_y + movement_preview.1) + cell_w * 0.7,
                grid_x_to_screen(player_x + movement_preview.0) + cell_w * 0.7,
                grid_y_to_screen(player_y + movement_preview.1) + cell_w * 0.3,
                3.0,
                arrow_color,
            );
        }

        if let Some(character_i) = self.target_character_i {
            let target_pos = self.characters[character_i].1;
            draw_square(target_pos, MAGENTA);
        }

        tell_ui_to_enter_move_state
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
            GameState::AwaitingPlayerAction(..) => {
                user_interface.set_state(ActivityPopupState::ChooseAction, vec![]);
            }
            GameState::AwaitingPlayerAttackReaction(StateReactToAttack { lines, .. }) => {
                user_interface.set_state(ActivityPopupState::ReactToAttack, lines.clone());
            }
            GameState::AwaitingPlayerHitReaction(StateReactToHit { lines, .. }) => {
                user_interface.set_state(ActivityPopupState::ReactToHit, lines.clone());
            }
            GameState::AwaitingBot(..) => {
                unreachable!("The game is awaiting bot, but the UI is awaiting player input?");
            }
        },
        State::CatchingUp(..) => {
            user_interface.set_state(ActivityPopupState::Idle, vec![]);
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum ActivityPopupState {
    ChooseAction,
    CommitAttackAction(HandType),
    CommitSpellAction(Spell),
    CommitSelfEffectAction(SelfEffectAction),
    CommitMove { action_point_cost: u32 },
    ReactToAttack,
    ReactToHit,
    Idle,
}

struct ActivityPopup {
    state: ActivityPopupState,
    lines: Vec<String>,
    buttons: Vec<ActionButton>,
}

impl ActivityPopup {
    fn draw(&mut self, x: f32, y: f32) {
        if matches!(self.state, ActivityPopupState::Idle) {
            return;
        }

        draw_rectangle(x, y, 500.0, 160.0, DARKBROWN);

        let mut x0 = x + 10.0;
        let mut y0 = y + 20.0;

        for line in &self.lines {
            draw_text(line, x0, y0, 20.0, WHITE);
            y0 += 20.0;
        }

        match &self.state {
            ActivityPopupState::ChooseAction => {
                draw_text("Choose an action!", x0, y0, 20.0, WHITE);
            }
            ActivityPopupState::CommitAttackAction(_hand) => {
                draw_text("Add enhancement or proceed", x0, y0, 20.0, WHITE);
            }
            ActivityPopupState::CommitSpellAction(_spell) => {
                draw_text("Add enhancement or proceed", x0, y0, 20.0, WHITE);
            }
            ActivityPopupState::CommitSelfEffectAction(..) => {
                draw_text("Proceed?", x0, y0, 20.0, WHITE);
            }
            ActivityPopupState::CommitMove { .. } => {
                draw_text("Change the direction or proceed", x0, y0, 20.0, WHITE);
            }
            ActivityPopupState::ReactToAttack => {
                draw_text("React to the attack?", x0, y0, 20.0, WHITE);
            }
            ActivityPopupState::ReactToHit => {
                draw_text("React to being hit?", x0, y0, 20.0, WHITE);
            }
            ActivityPopupState::Idle => unreachable!(),
        }

        y0 += 20.0;
        for btn in &mut self.buttons {
            btn.draw(x0, y0);
            x0 += btn.size.0 + 10.0;
        }
    }

    fn set_state(
        &mut self,
        state: ActivityPopupState,
        lines: Vec<String>,
        buttons: Vec<ActionButton>,
    ) {
        if self.state != state {
            for btn in &self.buttons {
                btn.notify_hidden();
            }
        }

        self.state = state;
        self.lines = lines;
        self.buttons = buttons;
    }
}

struct UserInterface {
    characters: Vec<Rc<RefCell<Character>>>,
    reserved_action_points: u32,
    event_queue: Rc<RefCell<Vec<InternalUiEvent>>>,
    state: ActivityPopupState,
    hovered_button: Option<(u32, ButtonAction)>,
    next_available_button_id: u32,

    log: Log,
    tabs: Tabs,
    resource_bars: Container,
    action_points_label: TextLine,
    action_points_row: ActionPointsRow,
    health_bar: Rc<RefCell<LabelledResourceBar>>,
    mana_bar: Rc<RefCell<LabelledResourceBar>>,
    stamina_bar: Rc<RefCell<LabelledResourceBar>>,
    activity_popup: ActivityPopup,
    game_grid: GameGrid,
}

impl UserInterface {
    fn new(characters: Vec<Rc<RefCell<Character>>>) -> Self {
        let mut combat_buttons = vec![];
        let mut skill_buttons = vec![];
        let mut spell_buttons = vec![];

        let event_queue = Rc::new(RefCell::new(vec![]));

        let character_ref = characters[0].borrow();

        let mut next_button_id = 1;

        let mut new_button = |name, btn_action| {
            let btn = action_button(name, btn_action, &event_queue, next_button_id);
            next_button_id += 1;
            btn
        };

        let mut spell_enhancement_buttons = vec![];
        for (name, action) in character_ref.known_actions() {
            let btn = new_button(name, ButtonAction::Action(action));
            match action {
                BaseAction::Attack { .. } => combat_buttons.push(btn),
                BaseAction::SelfEffect(..) => skill_buttons.push(btn),
                BaseAction::CastSpell(spell) => {
                    if let Some(enhancement) = spell.possible_enhancement {
                        let btn_action = ButtonAction::SpellEnhancement(enhancement);
                        let btn = new_button(spell.name.to_string(), btn_action);
                        spell_enhancement_buttons.push(btn);
                    }

                    spell_buttons.push(btn)
                }
                BaseAction::Move { action_point_cost } => {
                    skill_buttons.push(btn);
                }
            }
        }

        let combat_row = buttons_row(combat_buttons);
        let skill_row = buttons_row(skill_buttons);
        let spell_row = buttons_row(spell_buttons);

        let mut reaction_buttons = vec![];
        for (subtext, reaction) in character_ref.known_on_attacked_reactions() {
            let btn_action = ButtonAction::OnAttackedReaction(reaction);
            reaction_buttons.push(new_button(subtext.clone(), btn_action));
        }
        for (subtext, reaction) in character_ref.known_on_hit_reactions() {
            let btn_action = ButtonAction::OnHitReaction(reaction);
            let btn = new_button(subtext.clone(), btn_action);
            reaction_buttons.push(btn);
        }

        let mut attack_enhancement_buttons = vec![];
        for (subtext, enhancement) in character_ref.known_attack_enhancements(HandType::MainHand) {
            let btn_action = ButtonAction::AttackEnhancement(enhancement);
            let btn = new_button(subtext, btn_action);
            attack_enhancement_buttons.push(btn);
        }

        let reactions_row = buttons_row(reaction_buttons);
        let attack_enhancements_row = buttons_row(attack_enhancement_buttons);
        let spell_enhancements_row = buttons_row(spell_enhancement_buttons);

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
            margin: 5.0,
            elements: vec![combat_row, skill_row, spell_row],
            ..Default::default()
        });

        let secondary_actions_section = Element::Container(Container {
            layout_dir: LayoutDirection::Vertical,
            margin: 5.0,
            elements: vec![
                reactions_row,
                attack_enhancements_row,
                spell_enhancements_row,
            ],
            ..Default::default()
        });

        let tabs = Tabs::new(
            0,
            vec![
                ("Actions", actions_section),
                ("Secondary", secondary_actions_section),
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

        drop(character_ref);

        let mut grid_characters = vec![];
        for character in &characters {
            grid_characters.push((&character.borrow().name[0..1], character.borrow().position));
        }
        let game_grid = GameGrid::new(grid_characters);

        Self {
            game_grid,
            characters,
            next_available_button_id: next_button_id,
            hovered_button: None,
            log: Log::new(),
            tabs,
            resource_bars,
            reserved_action_points: 0,
            action_points_label,
            action_points_row,
            event_queue,
            health_bar,
            mana_bar,
            stamina_bar,
            activity_popup: ActivityPopup {
                state,
                lines: Default::default(),
                buttons: vec![],
            },
            state,
        }
    }

    fn new_button(&mut self, subtext: String, btn_action: ButtonAction) -> ActionButton {
        let btn = action_button(
            subtext,
            btn_action,
            &self.event_queue,
            self.next_available_button_id,
        );
        self.next_available_button_id += 1;
        btn
    }

    fn draw(&mut self, y: f32) {
        let maybe_enter_move_state = self.game_grid.draw(100.0, y - 490.0);
        if let Some(direction) = maybe_enter_move_state {
            self.set_state(
                ActivityPopupState::CommitMove {
                    action_point_cost: 1,
                },
                vec![],
            );
        }

        self.activity_popup.draw(100.0, y - 170.0);

        draw_line(0.0, y, window_conf().window_width as f32, y, 2.0, DARKGRAY);
        self.action_points_label.draw(20.0, y + 10.0);
        self.action_points_row.draw(20.0, y + 30.0);
        self.tabs.draw(20.0, y + 70.0);
        self.resource_bars.draw(500.0, y + 80.0);
        self.log.draw(650.0, y);
    }

    fn player_character(&self) -> Ref<Character> {
        self.characters[0].borrow()
    }

    fn set_state(&mut self, state: ActivityPopupState, mut lines: Vec<String>) {
        self.state = state;

        let mut popup_buttons = vec![];
        let mut movement_preview = None;
        let mut is_targetting_enemy = false;
        let mut target_character_i = None;
        match state {
            ActivityPopupState::CommitAttackAction(hand) => {
                let enhancements = self.player_character().usable_attack_enhancements(hand);
                for (subtext, enhancement) in enhancements {
                    let btn_action = ButtonAction::AttackEnhancement(enhancement);
                    let btn = self.new_button(subtext, btn_action);
                    popup_buttons.push(btn);
                }
                popup_buttons.push(self.new_button("".to_string(), ButtonAction::Proceed));
                target_character_i = Some(1);
            }
            ActivityPopupState::CommitSpellAction(spell) => {
                if let Some(enhancement) = spell.possible_enhancement {
                    if self.player_character().can_use_spell_enhancement(spell) {
                        let btn_action = ButtonAction::SpellEnhancement(enhancement);
                        let btn = self.new_button("".to_string(), btn_action);
                        popup_buttons.push(btn);
                    }
                }
                popup_buttons.push(self.new_button("".to_string(), ButtonAction::Proceed));
                target_character_i = Some(1);
            }
            ActivityPopupState::CommitSelfEffectAction(..) => {
                popup_buttons.push(self.new_button("".to_string(), ButtonAction::Proceed));
            }
            ActivityPopupState::CommitMove { .. } => {
                lines.push(format!("Movement"));
                movement_preview = Some((1, 0));
                popup_buttons.push(self.new_button("".to_string(), ButtonAction::Proceed));
            }
            ActivityPopupState::ReactToAttack => {
                let reactions = self.player_character().usable_on_attacked_reactions();
                for (subtext, reaction) in reactions {
                    let btn_action = ButtonAction::OnAttackedReaction(reaction);
                    let btn = self.new_button(subtext, btn_action);
                    popup_buttons.push(btn);
                }
                popup_buttons.push(self.new_button("".to_string(), ButtonAction::Proceed));
            }
            ActivityPopupState::ReactToHit => {
                let reactions = self.player_character().usable_on_hit_reactions();
                for (subtext, reaction) in reactions {
                    let btn_action = ButtonAction::OnHitReaction(reaction);
                    let btn = self.new_button(subtext, btn_action);
                    popup_buttons.push(btn);
                }
                popup_buttons.push(self.new_button("".to_string(), ButtonAction::Proceed));
            }
            ActivityPopupState::ChooseAction => {}
            ActivityPopupState::Idle => {}
        }

        self.activity_popup.set_state(state, lines, popup_buttons);

        self.game_grid.movement_preview = movement_preview;
        self.game_grid.target_character_i = target_character_i
    }

    fn update(&mut self) -> Vec<Event> {
        let public_events = self
            .event_queue
            .take()
            .into_iter()
            .filter_map(|event| self.handle_event(event))
            .collect();

        let ap = if let Some(hovered_btn) = self.hovered_button {
            let is_hovering_valid_followup = match (self.state, hovered_btn.1) {
                (
                    ActivityPopupState::CommitAttackAction(..),
                    ButtonAction::AttackEnhancement(..),
                ) => true,
                (ActivityPopupState::CommitSpellAction(..), ButtonAction::SpellEnhancement(..)) => {
                    true
                }
                (_, ButtonAction::Proceed) => true,
                _ => false,
            };

            if is_hovering_valid_followup {
                self.reserved_action_points + hovered_btn.1.action_point_cost()
            } else {
                hovered_btn.1.action_point_cost()
            }
        } else {
            self.reserved_action_points
        };
        self.action_points_row.reserved_and_hovered = ap;

        public_events
    }

    fn handle_event(&mut self, event: InternalUiEvent) -> Option<Event> {
        match event {
            InternalUiEvent::ButtonHovered(hovered, button_id, button_action) => {
                if hovered {
                    self.hovered_button = Some((button_id, button_action));
                } else {
                    if let Some(previously_hovered_button) = self.hovered_button {
                        if button_id == previously_hovered_button.0 {
                            self.hovered_button = None
                        }
                    }
                }
                None
            }

            InternalUiEvent::ButtonClicked(btn_action) => {
                self.reserved_action_points = 0;

                match btn_action {
                    ButtonAction::Action(base_action) => {
                        let may_choose_action = match self.state {
                            ActivityPopupState::ChooseAction => true,
                            ActivityPopupState::CommitAttackAction(..) => true,
                            ActivityPopupState::CommitSpellAction(..) => true,
                            ActivityPopupState::CommitSelfEffectAction(..) => true,
                            ActivityPopupState::CommitMove { .. } => true,
                            _ => false,
                        };

                        if may_choose_action && self.player_character().can_use_action(base_action)
                        {
                            match base_action {
                                BaseAction::Attack {
                                    hand,
                                    action_point_cost,
                                } => {
                                    self.reserved_action_points = action_point_cost;
                                    let weapon = self.player_character().weapon(hand).unwrap();
                                    let description = format!(
                                        "Attack ({}: {} damage)",
                                        weapon.name, weapon.damage
                                    );

                                    self.set_state(
                                        ActivityPopupState::CommitAttackAction(hand),
                                        vec![description],
                                    );

                                    None
                                }
                                BaseAction::SelfEffect(sea) => {
                                    self.reserved_action_points = sea.action_point_cost;
                                    let description = format!("{}: {}", sea.name, sea.description);
                                    self.set_state(
                                        ActivityPopupState::CommitSelfEffectAction(sea),
                                        vec![description],
                                    );
                                    None
                                }
                                BaseAction::CastSpell(spell) => {
                                    self.reserved_action_points = spell.action_point_cost;
                                    let mut description = format!(
                                        "{}: {}",
                                        spell.name,
                                        spell.description.to_string()
                                    );
                                    if spell.damage > 0 {
                                        description
                                            .push_str(&format!(" ({} damage)", spell.damage));
                                    }
                                    self.set_state(
                                        ActivityPopupState::CommitSpellAction(spell),
                                        vec![description],
                                    );
                                    None
                                }
                                BaseAction::Move { action_point_cost } => {
                                    self.reserved_action_points = action_point_cost;

                                    self.set_state(
                                        ActivityPopupState::CommitMove { action_point_cost },
                                        vec![],
                                    );

                                    None
                                }
                            }
                        } else {
                            println!("Cannot choose this action at this time");
                            None
                        }
                    }
                    ButtonAction::AttackEnhancement(enhancement) => match self.state {
                        ActivityPopupState::CommitAttackAction(hand) => {
                            if self
                                .player_character()
                                .can_use_attack_enhancement(hand, &enhancement)
                            {
                                let target_character_i = self.game_grid.target_character_i.unwrap();
                                Some(Event::ChoseAction(Action::Attack {
                                    hand,
                                    enhancements: vec![enhancement],
                                    target_character_i,
                                }))
                            } else {
                                println!("Cannot use this attack enhancement");
                                None
                            }
                        }
                        _ => {
                            println!("Cannot choose attack enhancement at this time");
                            None
                        }
                    },
                    ButtonAction::SpellEnhancement(_enhancement) => match self.state {
                        ActivityPopupState::CommitSpellAction(spell) => {
                            if self.player_character().can_use_spell_enhancement(spell) {
                                let target_character_i = self.game_grid.target_character_i.unwrap();
                                Some(Event::ChoseAction(Action::CastSpell {
                                    spell,
                                    enhanced: true,
                                    target_character_i,
                                }))
                            } else {
                                println!("Cannot use this spell enhancement");
                                None
                            }
                        }
                        _ => {
                            println!("Cannot choose spell enhancement at this time");
                            None
                        }
                    },
                    ButtonAction::OnAttackedReaction(reaction) => {
                        if self.state == ActivityPopupState::ReactToAttack {
                            if self
                                .player_character()
                                .can_use_on_attacked_reaction(reaction)
                            {
                                Some(Event::ChoseAttackedReaction(Some(reaction)))
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
                            if self.player_character().can_use_on_hit_reaction(reaction) {
                                Some(Event::ChoseHitReaction(Some(reaction)))
                            } else {
                                println!("Cannot afford this reaction at this time");
                                None
                            }
                        } else {
                            println!("Cannot use this reaction at this time");
                            None
                        }
                    }
                    ButtonAction::Proceed => match self.state {
                        ActivityPopupState::CommitAttackAction(hand) => {
                            let target_character_i = self.game_grid.target_character_i.unwrap();
                            Some(Event::ChoseAction(Action::Attack {
                                hand,
                                enhancements: vec![],
                                target_character_i,
                            }))
                        }
                        ActivityPopupState::CommitSpellAction(spell) => {
                            let target_character_i = self.game_grid.target_character_i.unwrap();
                            Some(Event::ChoseAction(Action::CastSpell {
                                spell,
                                enhanced: false,
                                target_character_i,
                            }))
                        }
                        ActivityPopupState::CommitSelfEffectAction(sea) => {
                            Some(Event::ChoseAction(Action::SelfEffect(sea)))
                        }
                        ActivityPopupState::CommitMove {
                            action_point_cost, ..
                        } => {
                            let direction = self.game_grid.movement_preview.take().unwrap();
                            Some(Event::ChoseAction(Action::Move {
                                action_point_cost,
                                direction,
                            }))
                        }
                        ActivityPopupState::ReactToAttack => {
                            Some(Event::ChoseAttackedReaction(None))
                        }
                        ActivityPopupState::ReactToHit => Some(Event::ChoseHitReaction(None)),
                        ActivityPopupState::ChooseAction => {
                            panic!("Invalid state. How to proceed from choosing action?")
                        }
                        ActivityPopupState::Idle => {
                            panic!("Invalid state. How to proceed from idle?")
                        }
                    },
                }
            }
        }
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

    fn update_character_positions(&mut self) {
        self.game_grid.update_positions(&self.characters);
    }
}

struct CharacterPortraits {
    row: Container,
    active_i: usize,
    portraits: Vec<Rc<RefCell<CharacterPortrait>>>,
}

impl CharacterPortraits {
    fn new(characters: &[Rc<RefCell<Character>>], active_i: usize) -> Self {
        let portraits: Vec<Rc<RefCell<CharacterPortrait>>> = characters
            .iter()
            .map(|character| Rc::new(RefCell::new(CharacterPortrait::new(&character.borrow()))))
            .collect();

        let mut elements = vec![];
        for portrait in &portraits {
            let cloned = Rc::clone(portrait);
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
            portraits,
        };

        this.set_active_character(active_i);
        this
    }

    fn set_active_character(&mut self, character_i: usize) {
        self.portraits[self.active_i].borrow_mut().active = false;
        self.active_i = character_i;
        self.portraits[self.active_i].borrow_mut().active = true;
    }

    fn update(&mut self, game: &CoreGame) {
        self.set_active_character(game.active_character_i);
        for (i, character) in game.characters().iter().enumerate() {
            let mut portrait = self.portraits[i].borrow_mut();
            portrait.action_points = character.borrow().action_points;
            portrait.current_health = character.borrow().health.current;
        }
    }

    fn draw(&mut self, x: f32, y: f32) {
        self.row.draw(x, y);
        let y0 = y + self.row.size().1;
        draw_line(
            0.0,
            y0,
            window_conf().window_width as f32,
            y0,
            1.0,
            DARKGRAY,
        );
    }
}

struct CharacterPortrait {
    text: TextLine,
    active: bool,
    padding: f32,
    action_points: u32,
    current_health: u32,
    max_health: u32,
}

impl CharacterPortrait {
    fn new(character: &Character) -> Self {
        Self {
            text: TextLine::new(character.name, 20),
            active: false,
            padding: 15.0,
            action_points: character.action_points,
            current_health: character.health.current,
            max_health: character.health.max,
        }
    }
}

impl Drawable for CharacterPortrait {
    fn draw(&mut self, x: f32, y: f32) {
        if self.active {
            let (w, h) = self.size();
            draw_rectangle_lines(x, y, w, h, 2.0, GOLD);
        }
        self.text.draw(self.padding + x, self.padding + y);
        draw_text(
            &format!("{} AP", self.action_points),
            self.padding + x,
            y + 55.0,
            16.0,
            WHITE,
        );
        draw_text(
            &format!("{}/{}", self.current_health, self.max_health),
            self.padding + x,
            y + 70.0,
            16.0,
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
                elements,
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
        draw_line(x, y, x, y + 350.0, 1.0, DARKGRAY);
        self.lines.draw(x + 10.0, y + 10.0);
    }
}

struct ActionPointsRow {
    current: u32,
    reserved_and_hovered: u32,
    max: u32,
    cell_size: (f32, f32),
    padding: f32,
}

impl ActionPointsRow {
    fn new(action_points: u32) -> Self {
        Self {
            current: action_points,
            reserved_and_hovered: 0,
            max: 6,
            cell_size: (20.0, 20.0),
            padding: 3.0,
        }
    }

    fn draw(&mut self, x: f32, y: f32) {
        assert!(self.current <= self.max);

        let mut x0 = x + self.padding;
        let y0 = y + self.padding;
        let r = self.cell_size.1 * 0.3;
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
        margin: 5.0,
        elements,
        ..Default::default()
    })
}

fn action_button(
    subtext: String,
    action: ButtonAction,
    event_queue: &Rc<RefCell<Vec<InternalUiEvent>>>,
    id: u32,
) -> ActionButton {
    let button_size = (90.0, 50.0);
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
                action_points = spell.action_point_cost;
                mana_points = spell.mana_cost;
            }
            BaseAction::Move { action_point_cost } => {
                action_points = action_point_cost;
                text = "Move";
            }
        },
        ButtonAction::AttackEnhancement(enhancement) => {
            text = enhancement.name;
            action_points = enhancement.action_point_cost;
            stamina_points = enhancement.stamina_cost;
        }
        ButtonAction::Proceed => {
            text = "Proceed";
        }
        ButtonAction::SpellEnhancement(enhancement) => {
            text = enhancement.name;
            mana_points = enhancement.mana_cost;
        }
        ButtonAction::OnAttackedReaction(reaction) => {
            text = reaction.name;
            action_points = reaction.action_point_cost;
            stamina_points = reaction.stamina_cost;
        }
        ButtonAction::OnHitReaction(reaction) => {
            text = reaction.name;
            action_points = reaction.action_point_cost;
        }
    }

    let mut btn = ActionButton::new(
        id,
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

        self.tabs[self.active_i].draw(x, y + 40.0);
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
    OnAttackedReaction(OnAttackedReaction),
    OnHitReaction(OnHitReaction),
    AttackEnhancement(AttackEnhancement),
    SpellEnhancement(SpellEnhancement),
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
        }
    }
}

struct ActionButton {
    id: u32,
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
        id: u32,
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
                elements: vec![text, Element::Text(TextLine::new(subtext, 15))],
                ..Default::default()
            }))
        } else {
            Box::new(text)
        };

        Self {
            id,
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

    fn notify_hidden(&self) {
        if self.hovered {
            if let Some(event_sender) = &self.event_sender {
                event_sender.send(InternalUiEvent::ButtonHovered(false, self.id, self.action));
            }
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
                event_sender.send(InternalUiEvent::ButtonHovered(
                    hovered,
                    self.id,
                    self.action,
                ));
            }
        }

        if hovered && is_mouse_button_pressed(MouseButton::Left) {
            if let Some(event_sender) = &self.event_sender {
                event_sender.send(InternalUiEvent::ButtonClicked(self.action));
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
    ButtonHovered(bool, u32, ButtonAction),
    ButtonClicked(ButtonAction),
}

enum Event {
    ChoseAttackedReaction(Option<OnAttackedReaction>),
    ChoseHitReaction(Option<OnHitReaction>),
    ChoseAction(Action),
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
