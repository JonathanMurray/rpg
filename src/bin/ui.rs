use std::collections::HashSet;
use std::{
    cell::{self, Cell, Ref, RefCell},
    collections::HashMap,
    rc::Rc,
};

use macroquad::{
    color::{
        self, Color, BLACK, BLUE, BROWN, DARKBROWN, DARKGRAY, DARKGREEN, GOLD, GRAY, GREEN,
        LIGHTGRAY, MAGENTA, MAROON, ORANGE, PINK, PURPLE, RED, WHITE, YELLOW,
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
use rpg::bot::bot_choose_action;
use rpg::core::{
    as_percentage, prob_attack_hit, prob_spell_hit, Action, AttackEnhancement, BaseAction,
    Character, CoreGame, GameState, Hand, HandType, Logger, MovementEnhancement,
    OnAttackedReaction, OnHitReaction, Range, SelfEffectAction, Spell, SpellEnhancement,
    StateChooseAction, StateReactToAttack, StateReactToHit,
};
use rpg::pathfind::PathfindGrid;

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

    let mut character_portraits =
        CharacterPortraits::new(game.characters(), game.active_character_i);

    let mut game_state = game.begin();
    let mut catching_up = true;
    let mut timer = 0.0;

    change_state(&game_state, catching_up, &mut user_interface);

    loop {
        clear_background(BLACK);
        character_portraits.draw(50.0, 20.0);

        user_interface.draw(640.0);

        let active_character_i = game_state.game().active_character_i;
        let events = user_interface.update(active_character_i);

        user_interface.update_character_resources(&player_character.borrow());

        character_portraits.update(game_state.game());

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

            catching_up = true;
            timer = 0.0;
            change_state(&game_state, catching_up, &mut user_interface);
        }

        for line in logbuf.borrow_mut().0.drain(..) {
            user_interface.log.add(line);
        }

        if catching_up {
            timer += get_frame_time();
            if timer > 1.0 {
                catching_up = false;

                match game_state {
                    GameState::AwaitingBotChooseAction(awaiting_bot) => {
                        let action = bot_choose_action(&awaiting_bot.game);
                        game_state = awaiting_bot.proceed(action);
                        catching_up = true;
                        timer = 0.0;
                        change_state(&game_state, catching_up, &mut user_interface);
                    }
                    GameState::PerformingMovement(performing_movement) => {
                        game_state = performing_movement.proceed();
                        catching_up = true;
                        timer = 0.0;
                        change_state(&game_state, catching_up, &mut user_interface);
                    }
                    _ => {
                        catching_up = false;
                        change_state(&game_state, catching_up, &mut user_interface);
                    }
                }
            }
        }

        match game_state {
            GameState::AwaitingPlayerAttackReaction(..)
            | GameState::AwaitingPlayerHitReaction(..)
                if catching_up =>
            {
                // The reaction popup should show up immediately
                catching_up = false;
                change_state(&game_state, catching_up, &mut user_interface);
            }
            _ => {}
        }

        next_frame().await
    }
}

// TODO should be part of UI struct?
fn change_state(game_state: &GameState, catching_up: bool, user_interface: &mut UserInterface) {
    if catching_up {
        println!("catching up");
        user_interface.set_state(UiState::Idle);
    } else {
        match game_state {
            GameState::AwaitingPlayerAction(..) => {
                println!("awaiting player action");
                user_interface.set_state(UiState::ChooseAction);
            }
            GameState::AwaitingPlayerAttackReaction(StateReactToAttack {
                attacking_character_i,
                hand,
                ..
            }) => {
                println!("awaiting player attack reaction");
                user_interface.set_state(UiState::ReactToAttack {
                    attacking_character_i: *attacking_character_i,
                    hand: *hand,
                });
            }
            GameState::AwaitingPlayerHitReaction(StateReactToHit {
                attacking_character_i,
                damage,
                ..
            }) => {
                println!("awaiting player hit reaction");
                user_interface.set_state(UiState::ReactToHit {
                    attacking_character_i: *attacking_character_i,
                    damage: *damage,
                });
            }
            GameState::AwaitingBotChooseAction(..) | GameState::PerformingMovement(..) => {
                println!("awaiting bot, or performing movement");
                user_interface.set_state(UiState::Idle);
            }
        }
    }
}

struct GameGrid {
    pathfind_grid: PathfindGrid,
    characters: Vec<(TextLine, (i32, i32))>,
    active_character_i: usize,
    movement_range: f32,
    movement_preview: Option<Vec<(f32, (i32, i32))>>,
    target_character_i: Option<usize>,
    event_sender: EventSender,
    receptive_to_input: bool,
    range_indicator: Option<Range>,
}

impl GameGrid {
    fn new(characters: Vec<(impl Into<String>, (u32, u32))>, event_sender: EventSender) -> Self {
        let characters = characters
            .into_iter()
            .map(|(s, pos)| (TextLine::new(s, 25), (pos.0 as i32, pos.1 as i32)))
            .collect();

        Self {
            pathfind_grid: PathfindGrid::new(),
            characters,
            active_character_i: 0,
            movement_range: 0.0,
            movement_preview: Default::default(),
            target_character_i: None,
            event_sender,
            receptive_to_input: false,
            range_indicator: None,
        }
    }

    fn update(&mut self, active_character_i: usize, characters: &[Rc<RefCell<Character>>]) {
        // TODO don't assume that player is the first in the vec
        self.receptive_to_input = active_character_i == 0;

        self.pathfind_grid.blocked_positions.clear();

        self.active_character_i = active_character_i;
        for (i, character) in characters.iter().enumerate() {
            let pos = character.borrow().position;
            self.characters[i].1 = (pos.0 as i32, pos.1 as i32);

            self.pathfind_grid
                .blocked_positions
                .insert((pos.0 as i32, pos.1 as i32));
        }

        let pos = self.characters[self.active_character_i].1;
        self.pathfind_grid.run(pos, self.movement_range);
    }

    fn set_movement_range(&mut self, range: f32) {
        self.movement_range = range;
        if let Some(movement_preview) = &mut self.movement_preview {
            while !movement_preview.is_empty() && movement_preview[0].0 > range {
                movement_preview.remove(0);
            }
        }

        let pos = self.characters[self.active_character_i].1;
        self.pathfind_grid.run(pos, self.movement_range);
    }

    fn take_movement_path(&mut self) -> Vec<(u32, u32)> {
        self.movement_preview
            .take()
            .unwrap()
            .into_iter()
            .rev()
            .map(|(_dist, (x, y))| (x as u32, y as u32))
            .collect()
    }

    fn draw(&mut self, x: f32, y: f32) {
        let cell_w = 25.0;
        let grid_dimensions = (20, 12);

        let grid_x_to_screen = |grid_x: i32| x + grid_x as f32 * cell_w;
        let grid_y_to_screen = |grid_y: i32| y + grid_y as f32 * cell_w;

        let active_character_pos = self.characters[self.active_character_i].1;

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

        // TODO: don't assume that player is the first in the vec
        let (player_x, player_y) = self.characters[0].1;

        draw_rectangle(
            grid_x_to_screen(active_character_pos.0),
            grid_y_to_screen(active_character_pos.1),
            cell_w,
            cell_w,
            BROWN,
        );

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

        if self.movement_preview.is_some() {
            for (pos, _) in &self.pathfind_grid.distances {
                draw_square(*pos, GREEN);
            }
        }

        if let Some(range) = self.range_indicator {
            let range_ceil = (f32::from(range)).ceil() as i32;
            let range_squared = range.squared() as i32;
            let within =
                |x: i32, y: i32| (x - player_x).pow(2) + (y - player_y).pow(2) <= range_squared;
            for x in
                (player_x - range_ceil).max(0)..=(player_x + range_ceil).min(grid_dimensions.0 - 1)
            {
                for y in (player_y - range_ceil).max(0)
                    ..=(player_y + range_ceil).min(grid_dimensions.1 - 1)
                {
                    if within(x, y) {
                        let color = YELLOW;
                        if !within(x - 1, y) {
                            draw_line(
                                grid_x_to_screen(x),
                                grid_y_to_screen(y),
                                grid_x_to_screen(x),
                                grid_y_to_screen(y + 1),
                                1.0,
                                color,
                            );
                        }
                        if !within(x + 1, y) {
                            draw_line(
                                grid_x_to_screen(x + 1),
                                grid_y_to_screen(y),
                                grid_x_to_screen(x + 1),
                                grid_y_to_screen(y + 1),
                                1.0,
                                color,
                            );
                        }
                        if !within(x, y - 1) {
                            draw_line(
                                grid_x_to_screen(x),
                                grid_y_to_screen(y),
                                grid_x_to_screen(x + 1),
                                grid_y_to_screen(y),
                                1.0,
                                color,
                            );
                        }
                        if !within(x, y + 1) {
                            draw_line(
                                grid_x_to_screen(x),
                                grid_y_to_screen(y + 1),
                                grid_x_to_screen(x + 1),
                                grid_y_to_screen(y + 1),
                                1.0,
                                color,
                            );
                        }
                    }
                }
            }
        }

        let text_margin = 5.0;
        for (text, position) in &self.characters {
            text.draw(
                grid_x_to_screen(position.0) + text_margin,
                grid_y_to_screen(position.1) + text_margin,
            );
        }

        let (mouse_x, mouse_y) = mouse_position();
        let mouse_local = (mouse_x - x, mouse_y - y);

        let mut character_positions = vec![];
        for (_, pos) in &self.characters {
            character_positions.push(*pos);
        }

        if (0f32..grid_dimensions.0 as f32 * cell_w).contains(&mouse_local.0)
            && (0f32..grid_dimensions.1 as f32 * cell_w).contains(&mouse_local.1)
            && self.receptive_to_input
        {
            let (mouse_grid_x, mouse_grid_y) = (
                (mouse_local.0 / cell_w) as i32,
                (mouse_local.1 / cell_w) as i32,
            );

            //let dx = mouse_grid_x - player_x;
            //let dy = mouse_grid_y - player_y;

            let collision = character_positions.contains(&(mouse_grid_x, mouse_grid_y));
            //let valid_move_destination = dx.abs() <= 1 && dy.abs() <= 1 && !collision;

            let valid_move_destination = match self
                .pathfind_grid
                .distances
                .get(&(mouse_grid_x, mouse_grid_y))
            {
                Some((dist, _enter_from)) => *dist <= self.movement_range,
                _ => false,
            } && !collision;

            let mut hovered_npc_i = None;
            for (i, (_name, pos)) in self.characters.iter().enumerate() {
                if *pos == (mouse_grid_x, mouse_grid_y) && *pos != (player_x, player_y) {
                    hovered_npc_i = Some(i);
                }
            }

            if valid_move_destination {
                let destination = (mouse_grid_x, mouse_grid_y);
                draw_square(destination, YELLOW);
                if is_mouse_button_down(MouseButton::Left) {
                    if self.movement_preview.is_none() {
                        self.event_sender
                            .send(InternalUiEvent::SwitchedToMoveInGrid);
                    }

                    let dist_enter_from = self.pathfind_grid.distances.get(&destination).unwrap();
                    let mut dist = dist_enter_from.0;
                    let mut movement_preview = vec![(dist, destination)];
                    let mut pos = dist_enter_from.1;

                    loop {
                        let dist_enter_from = self.pathfind_grid.distances.get(&pos).unwrap();
                        dist = dist_enter_from.0;
                        movement_preview.push((dist, pos));
                        if pos == (player_x, player_y) {
                            break;
                        }
                        pos = dist_enter_from.1;
                    }
                    self.movement_preview = Some(movement_preview);
                }
            } else if let Some(i) = hovered_npc_i {
                draw_square((mouse_grid_x, mouse_grid_y), MAGENTA);
                if is_mouse_button_down(MouseButton::Left) {
                    if self.target_character_i.is_none() {
                        self.event_sender
                            .send(InternalUiEvent::SwitchedToAttackInGrid);
                    }
                    self.target_character_i = Some(i);
                    self.movement_preview = None;
                }
            } else if !self.movement_preview.is_some() {
                draw_square((mouse_grid_x, mouse_grid_y), RED);
            }
        }

        if let Some(movement_preview) = &self.movement_preview {
            if !movement_preview.is_empty() {
                let arrow_color = ORANGE;
                for i in 0..movement_preview.len() - 1 {
                    let a = movement_preview[i].1;
                    let b = movement_preview[i + 1].1;
                    draw_line(
                        grid_x_to_screen(a.0) + cell_w / 2.0,
                        grid_y_to_screen(a.1) + cell_w / 2.0,
                        grid_x_to_screen(b.0) + cell_w / 2.0,
                        grid_y_to_screen(b.1) + cell_w / 2.0,
                        1.0,
                        arrow_color,
                    );
                }

                let end = movement_preview.first().unwrap().1;
                draw_line(
                    grid_x_to_screen(end.0) + cell_w * 0.3,
                    grid_y_to_screen(end.1) + cell_w * 0.3,
                    grid_x_to_screen(end.0) + cell_w * 0.7,
                    grid_y_to_screen(end.1) + cell_w * 0.7,
                    3.0,
                    arrow_color,
                );
                draw_line(
                    grid_x_to_screen(end.0) + cell_w * 0.3,
                    grid_y_to_screen(end.1) + cell_w * 0.7,
                    grid_x_to_screen(end.0) + cell_w * 0.7,
                    grid_y_to_screen(end.1) + cell_w * 0.3,
                    3.0,
                    arrow_color,
                );
            }
        }

        if let Some(character_i) = self.target_character_i {
            let target_pos = self.characters[character_i].1;
            draw_square(target_pos, MAGENTA);
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum UiState {
    ChooseAction,
    CommitAction(BaseAction),
    ReactToAttack {
        hand: HandType,
        attacking_character_i: usize,
    },
    ReactToHit {
        attacking_character_i: usize,
        damage: u32,
    },
    Idle,
}

struct ActivityPopup {
    state: UiState,
    initial_lines: Vec<String>,
    target_line: Option<String>,
    choice_buttons: HashMap<u32, ActionButton>,
    proceed_button: ActionButton,

    enabled: bool,
    choice_button_events: Rc<RefCell<Vec<InternalUiEvent>>>,
    base_action_points: u32,
    selected_button_ids: Vec<u32>,
    hovered_button_id: Option<u32>,
}

impl ActivityPopup {
    fn new(state: UiState, proceed_button: ActionButton) -> Self {
        Self {
            state,
            initial_lines: vec![],
            target_line: None,
            selected_button_ids: Default::default(),
            choice_buttons: Default::default(),
            proceed_button,
            enabled: true,
            choice_button_events: Rc::new(RefCell::new(vec![])),
            base_action_points: 0,
            hovered_button_id: None,
        }
    }

    fn draw(&self, x: f32, y: f32) {
        if matches!(self.state, UiState::Idle) {
            return;
        }

        let mut x0 = x + 10.0;
        let mut y0 = y + 20.0;

        if matches!(self.state, UiState::ChooseAction) {
            draw_rectangle(x, y, 500.0, 30.0, DARKBROWN);
            draw_text("Choose an action!", x0, y0, 20.0, WHITE);
            return;
        }

        draw_rectangle(x, y, 500.0, 160.0, DARKBROWN);

        for line in &self.initial_lines {
            draw_text(line, x0, y0, 20.0, WHITE);
            y0 += 20.0;
        }

        if let Some(line) = &self.target_line {
            draw_text(&line, x0, y0, 20.0, WHITE);
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
        draw_text(&choice_description_line, x0, y0, 20.0, WHITE);
        y0 += 20.0;

        if self.enabled {
            match &self.state {
                UiState::CommitAction(base_action) => {
                    match base_action {
                        BaseAction::SelfEffect(..) => {
                            draw_text("Proceed?", x0, y0, 20.0, WHITE);
                            y0 += 20.0;
                        }
                        BaseAction::Move { range, .. } => {
                            let percentage: u32 = self
                                .selected_actions()
                                .map(|action| action.unwrap_movement_enhancement().add_percentage)
                                .sum();
                            let range = range * (1.0 + percentage as f32 / 100.0);
                            let text = format!("range: {range}");
                            draw_text(&text, x0, y0, 20.0, WHITE);
                            y0 += 20.0;
                        }
                        BaseAction::Attack { .. } => {}
                        BaseAction::CastSpell(..) => {}
                    };
                }
                UiState::ReactToAttack { .. } => {
                    draw_text("Reaction:", x0, y0, 20.0, WHITE);
                    y0 += 20.0;
                }
                UiState::ReactToHit { .. } => {
                    draw_text("Reaction:", x0, y0, 20.0, WHITE);
                    y0 += 20.0;
                }
                UiState::Idle => unreachable!(),
                UiState::ChooseAction => unreachable!(),
            }
        }

        for btn in self.choice_buttons.values() {
            btn.draw(x0, y0);
            x0 += btn.size.0 + 10.0;
        }

        self.proceed_button.draw(x0, y0);
    }

    fn update(&mut self) -> Option<u32> {
        let mut changed_movement_range = false;
        for event in self.choice_button_events.borrow_mut().drain(..) {
            match event {
                InternalUiEvent::ButtonHovered(id, _button_action, hovered) => {
                    if hovered {
                        self.hovered_button_id = Some(id);
                    } else {
                        if self.hovered_button_id == Some(id) {
                            self.hovered_button_id = None;
                        }
                    }
                }

                InternalUiEvent::ButtonClicked(id, _button_action) => {
                    let clicked_btn = &self.choice_buttons[&id];
                    clicked_btn.toggle_highlighted();

                    if let ButtonAction::MovementEnhancement(..) = clicked_btn.action {
                        changed_movement_range = true;
                    }

                    // Some choices work like radio boxes
                    if matches!(self.state, UiState::ReactToAttack { .. })
                        || matches!(self.state, UiState::ReactToHit { .. })
                        || matches!(clicked_btn.action, ButtonAction::MovementEnhancement { .. })
                    {
                        for btn in self.choice_buttons.values() {
                            if btn.id != id {
                                btn.highlighted.set(false);
                            }
                        }
                    }

                    self.selected_button_ids.clear();
                    for btn in self.choice_buttons.values() {
                        if btn.highlighted.get() {
                            self.selected_button_ids.push(btn.id);
                        }
                    }
                }
                _ => unreachable!(),
            };
        }

        if changed_movement_range {
            let mut added_percentage = 0;
            for action in self.selected_actions() {
                if let ButtonAction::MovementEnhancement(enhancement) = action {
                    added_percentage += enhancement.add_percentage;
                }
            }
            Some(added_percentage)
        } else {
            None
        }
    }

    fn take_selected_actions(&mut self) -> Vec<ButtonAction> {
        self.selected_button_ids
            .drain(..)
            .map(|id| self.choice_buttons[&id].action)
            .collect()
    }

    fn selected_actions(&self) -> impl Iterator<Item = &ButtonAction> {
        self.selected_button_ids
            .iter()
            .map(|id| &self.choice_buttons[&id].action)
    }

    fn action_points(&self) -> u32 {
        let mut ap = self.base_action_points;
        for action in self.selected_actions() {
            ap += self.action_point_cost(*action);
        }
        if let Some(id) = self.hovered_button_id {
            if !self.selected_button_ids.contains(&id) {
                ap += self.action_point_cost(self.choice_buttons[&id].action);
            }
        }
        ap
    }

    fn action_point_cost(&self, button_action: ButtonAction) -> u32 {
        match button_action {
            ButtonAction::AttackEnhancement(enhancement) => enhancement.action_point_cost,
            ButtonAction::SpellEnhancement(_enhancement) => 0,
            ButtonAction::OnAttackedReaction(reaction) => reaction.action_point_cost,
            ButtonAction::OnHitReaction(reaction) => reaction.action_point_cost,
            ButtonAction::MovementEnhancement(enhancement) => enhancement.action_point_cost,
            ButtonAction::Action(..) | ButtonAction::Proceed => unreachable!(),
        }
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
            for btn in self.choice_buttons.values() {
                btn.notify_hidden();
            }
            self.proceed_button.notify_hidden();
        }

        let mut buttons_map = HashMap::new();
        for mut btn in buttons {
            btn.event_sender = Some(EventSender {
                queue: Rc::clone(&self.choice_button_events),
            });
            buttons_map.insert(btn.id, btn);
        }

        self.state = state;
        self.initial_lines = lines;
        self.choice_buttons = buttons_map;
        self.selected_button_ids.clear();

        self.base_action_points = if let UiState::CommitAction(base_action) = state {
            match base_action {
                BaseAction::Attack {
                    action_point_cost, ..
                } => action_point_cost,
                BaseAction::SelfEffect(sea) => sea.action_point_cost,
                BaseAction::CastSpell(spell) => spell.action_point_cost,
                BaseAction::Move {
                    action_point_cost, ..
                } => action_point_cost,
            }
        } else {
            0
        };
    }
}

struct UserInterface {
    characters: Vec<Rc<RefCell<Character>>>,
    event_queue: Rc<RefCell<Vec<InternalUiEvent>>>,
    state: UiState,
    hovered_button: Option<(u32, ButtonAction)>,
    next_available_button_id: u32,
    tracked_buttons: HashMap<String, Rc<ActionButton>>,

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

        let mut new_button = |subtext, btn_action| {
            let btn = ActionButton::new(subtext, btn_action, &event_queue, next_button_id);
            next_button_id += 1;
            btn
        };

        let mut tracked_buttons = HashMap::new();

        let mut spell_enhancement_buttons = vec![];
        for (name, action) in character_ref.known_actions() {
            let btn = Rc::new(new_button(name, ButtonAction::Action(action)));
            let cloned_btn = Rc::clone(&btn);
            match action {
                BaseAction::Attack { .. } => combat_buttons.push(btn),
                BaseAction::SelfEffect(..) => skill_buttons.push(btn),
                BaseAction::CastSpell(spell) => {
                    if let Some(enhancement) = spell.possible_enhancement {
                        let btn_action = ButtonAction::SpellEnhancement(enhancement);
                        let btn = new_button(spell.name.to_string(), btn_action);
                        btn.enabled.set(false);
                        spell_enhancement_buttons.push(btn);
                    }
                    spell_buttons.push(btn);
                }
                BaseAction::Move { .. } => {
                    skill_buttons.push(btn);
                }
            }
            tracked_buttons.insert(base_action_id(action), cloned_btn);
        }

        let combat_row = buttons_row(
            combat_buttons
                .into_iter()
                .map(|btn| Element::Rc(btn))
                .collect(),
        );
        let skill_row = buttons_row(
            skill_buttons
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
            let btn = new_button(subtext.clone(), btn_action);
            btn.enabled.set(false);
            reaction_buttons.push(btn);
        }
        for (subtext, reaction) in character_ref.known_on_hit_reactions() {
            let btn_action = ButtonAction::OnHitReaction(reaction);
            let btn = new_button(subtext.clone(), btn_action);
            btn.enabled.set(false);
            reaction_buttons.push(btn);
        }

        let mut attack_enhancement_buttons = vec![];
        for (subtext, enhancement) in character_ref.known_attack_enhancements(HandType::MainHand) {
            let btn_action = ButtonAction::AttackEnhancement(enhancement);
            let btn = new_button(subtext.clone(), btn_action);
            btn.enabled.set(false);
            attack_enhancement_buttons.push(btn);
        }

        let reactions_row = buttons_row(reaction_buttons.into_iter().map(Element::Btn).collect());
        let attack_enhancements_row = buttons_row(
            attack_enhancement_buttons
                .into_iter()
                .map(Element::Btn)
                .collect(),
        );
        let spell_enhancements_row = buttons_row(
            spell_enhancement_buttons
                .into_iter()
                .map(Element::Btn)
                .collect(),
        );

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
                Element::RcRefCell(cloned_health_bar),
                Element::RcRefCell(cloned_mana_bar),
                Element::RcRefCell(cloned_stamina_bar),
            ],
            ..Default::default()
        };

        let action_points_label = TextLine::new("Action points", 18);
        let action_points_row = ActionPointsRow::new(character_ref.action_points);

        let state = UiState::ChooseAction;

        drop(character_ref);

        let mut grid_characters = vec![];
        for character in &characters {
            grid_characters.push((&character.borrow().name[0..1], character.borrow().position));
        }
        let game_grid = GameGrid::new(
            grid_characters,
            EventSender {
                queue: Rc::clone(&event_queue),
            },
        );

        let popup_proceed_btn = new_button("".to_string(), ButtonAction::Proceed);

        Self {
            game_grid,
            characters,
            next_available_button_id: next_button_id,
            hovered_button: None,
            log: Log::new(),
            tabs,
            resource_bars,
            action_points_label,
            action_points_row,
            event_queue: Rc::clone(&event_queue),
            health_bar,
            mana_bar,
            stamina_bar,
            activity_popup: ActivityPopup::new(state, popup_proceed_btn),
            state,
            tracked_buttons,
        }
    }

    fn new_button(&mut self, subtext: String, btn_action: ButtonAction) -> ActionButton {
        let btn = ActionButton::new(
            subtext,
            btn_action,
            &self.event_queue,
            self.next_available_button_id,
        );
        self.next_available_button_id += 1;
        btn
    }

    fn draw(&mut self, y: f32) {
        self.game_grid.draw(100.0, y - 490.0);

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

    fn set_action_buttons_enabled(&self, enabled: bool) {
        for (_, btn) in &self.tracked_buttons {
            btn.enabled.set(enabled);
        }
    }

    fn set_state(&mut self, state: UiState) {
        self.state = state;

        let mut popup_lines = vec![];
        let mut popup_buttons = vec![];
        let mut movement = false;
        let mut has_target = false;

        match state {
            UiState::CommitAction(base_action) => {
                self.set_action_buttons_enabled(true);

                match base_action {
                    BaseAction::Attack {
                        hand,
                        action_point_cost,
                    } => {
                        self.set_highlighted_button(Some(base_action_id(base_action)));

                        let weapon = self.player_character().weapon(hand).unwrap();
                        popup_lines.push(format!(
                            "{} attack ({} AP)",
                            weapon.name, weapon.action_point_cost
                        ));
                        popup_lines.push(format!("{} damage", weapon.damage));
                        let enhancements = self.player_character().usable_attack_enhancements(hand);
                        for (subtext, enhancement) in enhancements {
                            let btn = self
                                .new_button(subtext, ButtonAction::AttackEnhancement(enhancement));
                            popup_buttons.push(btn);
                        }
                        has_target = true;
                    }
                    BaseAction::SelfEffect(sea) => {
                        self.set_highlighted_button(Some(base_action_id(base_action)));

                        popup_lines.push(format!("{} ({} AP)", sea.name, sea.action_point_cost));
                        popup_lines.push(sea.description.to_string());
                    }
                    BaseAction::CastSpell(spell) => {
                        self.set_highlighted_button(Some(base_action_id(base_action)));

                        popup_lines.push(format!(
                            "{} ({} AP, {} mana)",
                            spell.name, spell.action_point_cost, spell.mana_cost
                        ));
                        let mut description = spell.description.to_string();
                        if spell.damage > 0 {
                            description.push_str(&format!(" ({} damage)", spell.damage));
                        }
                        popup_lines.push(description);
                        if let Some(enhancement) = spell.possible_enhancement {
                            if self.player_character().can_use_spell_enhancement(spell) {
                                let btn_action = ButtonAction::SpellEnhancement(enhancement);
                                let btn = self.new_button("".to_string(), btn_action);
                                popup_buttons.push(btn);
                            }
                        }
                        has_target = true;
                    }
                    BaseAction::Move {
                        action_point_cost, ..
                    } => {
                        self.set_highlighted_button(Some(base_action_id(base_action)));
                        popup_lines.push(format!("Movement ({} AP)", action_point_cost));

                        let enhancements = self.player_character().usable_movement_enhancements();
                        for enhancement in enhancements {
                            let btn = self.new_button(
                                "".to_string(),
                                ButtonAction::MovementEnhancement(enhancement),
                            );
                            popup_buttons.push(btn);
                        }
                        movement = true;
                    }
                }
            }

            UiState::ReactToAttack {
                attacking_character_i,
                hand,
            } => {
                self.set_action_buttons_enabled(false);

                let attacker = self.characters[attacking_character_i].borrow();
                let defender = self.player_character();
                let attacks_str = format!(
                    "{} attacks {} (d20+{} vs {})",
                    attacker.name,
                    defender.name,
                    attacker.attack_modifier(hand),
                    defender.defense(),
                );
                popup_lines.push(attacks_str);
                let explanation = format!(
                    "{}{}",
                    attacker.explain_attack_circumstances(hand),
                    defender.explain_incoming_attack_circumstances()
                );
                if !explanation.is_empty() {
                    popup_lines.push(format!("  {explanation}"));
                }
                popup_lines.push(format!(
                    "  Chance to hit: {}",
                    as_percentage(prob_attack_hit(&attacker, hand, &defender))
                ));
                drop(attacker);
                drop(defender);

                let reactions = self.player_character().usable_on_attacked_reactions();
                for (subtext, reaction) in reactions {
                    let btn_action = ButtonAction::OnAttackedReaction(reaction);
                    let btn = self.new_button(subtext, btn_action);
                    popup_buttons.push(btn);
                }
            }

            UiState::ReactToHit {
                attacking_character_i,
                damage,
            } => {
                self.set_action_buttons_enabled(false);

                popup_lines.push(format!(
                    "{} took {} damage from an attack by {}",
                    self.player_character().name,
                    damage,
                    self.characters[attacking_character_i].borrow().name
                ));
                let reactions = self.player_character().usable_on_hit_reactions();
                for (subtext, reaction) in reactions {
                    let btn_action = ButtonAction::OnHitReaction(reaction);
                    let btn = self.new_button(subtext, btn_action);
                    popup_buttons.push(btn);
                }
            }

            UiState::ChooseAction => {
                self.set_action_buttons_enabled(true);
            }

            UiState::Idle => {
                self.set_action_buttons_enabled(false);
            }
        }

        self.activity_popup
            .set_state(state, popup_lines, popup_buttons);

        let move_range = self.player_character().move_range;
        self.game_grid.set_movement_range(move_range);

        if movement {
            if self.game_grid.movement_preview.is_none() {
                self.game_grid.movement_preview = Some(vec![]);
            }
        } else {
            self.game_grid.movement_preview = None;
        }

        if has_target {
            if self.game_grid.target_character_i.is_none() {
                // We pick an arbitrary enemy if none is picked already
                self.game_grid.target_character_i = Some(1);
            }
        } else {
            self.game_grid.target_character_i = None;
        }
    }

    fn update(&mut self, active_character_i: usize) -> Vec<Event> {
        self.game_grid.update(active_character_i, &self.characters);

        let maybe_updated_movement_range = self.activity_popup.update();

        if let Some(added_percentage) = maybe_updated_movement_range {
            let move_range =
                self.player_character().move_range * (1.0 + added_percentage as f32 / 100.0);
            self.game_grid.set_movement_range(move_range);
        }

        let public_events = self
            .event_queue
            .take()
            .into_iter()
            .filter_map(|event| self.handle_event(event))
            .collect();

        // TODO: Also show reserved mana and stamina
        self.action_points_row.reserved_and_hovered = if let Some(hovered_btn) = self.hovered_button
        {
            if matches!(hovered_btn.1, ButtonAction::Proceed) {
                self.activity_popup.action_points()
            } else {
                hovered_btn.1.action_point_cost()
            }
        } else {
            self.activity_popup.action_points()
        };

        self.activity_popup.set_enabled(true);
        self.activity_popup.target_line = None;
        self.game_grid.range_indicator = None;

        // TODO disable activity popup Proceed button if no movement has been chosen

        if let Some(i) = self.game_grid.target_character_i {
            let target_char = self.characters[i].borrow();
            match self.state {
                UiState::CommitAction(base_action @ BaseAction::Attack { hand, .. }) => {
                    if self
                        .player_character()
                        .can_reach_with_attack(hand, target_char.position)
                    {
                        if self.player_character().can_use_action(base_action) {
                            let chance = as_percentage(prob_attack_hit(
                                &self.player_character(),
                                hand,
                                &target_char,
                            ));
                            let mut explanation =
                                self.player_character().explain_attack_circumstances(hand);
                            explanation
                                .push_str(&target_char.explain_incoming_attack_circumstances());
                            self.activity_popup.target_line = Some(format!(
                                "target: {}, hit chance: {} {}",
                                target_char.name, chance, explanation
                            ));
                        } else {
                            self.activity_popup.target_line = Some("CAN NOT ATTACK".to_string());
                            self.activity_popup.set_enabled(false);
                        }
                    } else {
                        let range = self.player_character().weapon(hand).unwrap().range;
                        self.game_grid.range_indicator = Some(range);

                        self.activity_popup.target_line =
                            Some(format!("target: {}, OUT OF RANGE", target_char.name));
                        self.activity_popup.set_enabled(false);
                    }
                }
                UiState::CommitAction(BaseAction::CastSpell(spell)) => {
                    if self
                        .player_character()
                        .can_reach_with_spell(spell, target_char.position)
                    {
                        let chance = as_percentage(prob_spell_hit(
                            &self.player_character(),
                            spell.spell_type,
                            &target_char,
                        ));
                        self.activity_popup.target_line = Some(format!(
                            "target: {}, success chance: {}",
                            target_char.name, chance
                        ));
                    } else {
                        self.activity_popup.target_line =
                            Some(format!("target: {}, OUT OF RANGE", target_char.name));
                        self.activity_popup.set_enabled(false);
                    }
                }
                _ => unreachable!(),
            }
        }

        public_events
    }

    fn handle_event(&mut self, event: InternalUiEvent) -> Option<Event> {
        match event {
            InternalUiEvent::ButtonHovered(button_id, button_action, hovered) => {
                if hovered {
                    self.hovered_button = Some((button_id, button_action));
                } else {
                    if let Some(previously_hovered_button) = self.hovered_button {
                        if button_id == previously_hovered_button.0 {
                            self.hovered_button = None
                        }
                    }
                }
            }

            InternalUiEvent::ButtonClicked(_button_id, btn_action) => {
                match btn_action {
                    ButtonAction::Action(base_action) => {
                        let may_choose_action = match self.state {
                            UiState::ChooseAction => true,
                            UiState::CommitAction(..) => true,
                            _ => false,
                        };

                        if may_choose_action && self.player_character().can_use_action(base_action)
                        {
                            self.set_state(UiState::CommitAction(base_action));
                        } else {
                            println!("Cannot choose this action at this time");
                        }
                    }

                    ButtonAction::Proceed => {
                        self.set_highlighted_button(None);

                        let event = match self.state {
                            UiState::CommitAction(base_action) => {
                                let target_char_i = self.game_grid.target_character_i;
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
                                            target_character_i: target_char_i.unwrap(),
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
                                            target_character_i: target_char_i.unwrap(),
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
                                Event::ChoseAction(action)
                            }
                            UiState::ReactToAttack { .. } => {
                                let reaction =
                                    self.activity_popup.take_selected_actions().first().map(
                                        |action| match action {
                                            ButtonAction::OnAttackedReaction(reaction) => *reaction,
                                            _ => unreachable!(),
                                        },
                                    );
                                Event::ChoseAttackedReaction(reaction)
                            }
                            UiState::ReactToHit { .. } => {
                                let reaction =
                                    self.activity_popup.take_selected_actions().first().map(
                                        |action| match action {
                                            ButtonAction::OnHitReaction(reaction) => *reaction,
                                            _ => unreachable!(),
                                        },
                                    );

                                Event::ChoseHitReaction(reaction)
                            }
                            UiState::ChooseAction => unreachable!(),
                            UiState::Idle => unreachable!(),
                        };
                        return Some(event);
                    }

                    _ => unreachable!(),
                }
            }

            InternalUiEvent::SwitchedToMoveInGrid => {
                let move_range = self.player_character().move_range;
                self.set_state(UiState::CommitAction(BaseAction::Move {
                    action_point_cost: 1,
                    range: move_range,
                }));
            }

            InternalUiEvent::SwitchedToAttackInGrid => {
                let hand = HandType::MainHand;
                let action_point_cost = self
                    .player_character()
                    .weapon(hand)
                    .unwrap()
                    .action_point_cost;
                self.set_state(UiState::CommitAction(BaseAction::Attack {
                    hand,
                    action_point_cost,
                }));
            }
        }

        None
    }

    fn set_highlighted_button(&self, highlighted_button_action: Option<String>) {
        for (base_action_id, btn) in &self.tracked_buttons {
            btn.highlighted
                .set(highlighted_button_action.as_ref() == Some(base_action_id));
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
}

fn base_action_id(base_action: BaseAction) -> String {
    match base_action {
        BaseAction::Attack { hand, .. } => format!("ATTACK_{:?}", hand),
        BaseAction::SelfEffect(sea) => format!("SELF_EFFECT_{}", sea.name),
        BaseAction::CastSpell(spell) => format!("SPELL_{}", spell.name),
        BaseAction::Move { .. } => format!("MOVE"),
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
            elements.push(Element::RcRefCell(cloned));
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

    fn draw(&self, x: f32, y: f32) {
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
    fn draw(&self, x: f32, y: f32) {
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

    fn draw(&self, x: f32, y: f32) {
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

    fn draw(&self, x: f32, y: f32) {
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
    fn draw(&self, x: f32, y: f32);
    fn size(&self) -> (f32, f32);
}

struct ResourceBar {
    current: u32,
    max: u32,
    color: Color,
    cell_size: (f32, f32),
}

impl Drawable for ResourceBar {
    fn draw(&self, x: f32, y: f32) {
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
                Element::RcRefCell(cloned_bar),
                Element::RcRefCell(cloned_value_text),
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
        elements: buttons,
        ..Default::default()
    })
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
    RcRefCell(Rc<RefCell<dyn Drawable>>),
    Rc(Rc<dyn Drawable>),
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
            Element::RcRefCell(drawable) => drawable.borrow().size(),
            Element::Rc(drawable) => drawable.size(),
        };

        assert!(size.0.is_finite() && size.1.is_finite());
        size
    }

    fn draw(&self, x: f32, y: f32) {
        match self {
            Element::Btn(btn) => btn.draw(x, y),
            Element::Container(container) => container.draw(x, y),
            Element::Text(text) => text.draw(x, y),
            Element::Circle(circle) => circle.draw(x, y),
            Element::Rect(rect) => rect.draw(x, y),
            Element::TabLink(link) => link.draw(x, y),
            Element::Box(drawable) => drawable.draw(x, y),
            Element::RcRefCell(drawable) => drawable.borrow_mut().draw(x, y),
            Element::Rc(drawable) => drawable.draw(x, y),
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
    links: Container,
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
            links: links_row,
            tabs,
            active_i,
        }
    }

    fn draw(&mut self, x: f32, y: f32) {
        // If a link was clicked, update the state of all links
        let mut maybe_clicked_i = None;
        for (i, link) in self.links.elements.iter_mut().enumerate() {
            if link.unwrap_tab_link().was_clicked.get() {
                maybe_clicked_i = Some(i);
                self.active_i = i;
                break;
            }
        }
        if let Some(clicked_i) = maybe_clicked_i {
            for (i, element) in self.links.elements.iter_mut().enumerate() {
                let tab_link = element.unwrap_tab_link();
                tab_link.was_clicked.set(false);
                tab_link.active = i == clicked_i;
            }
        }

        self.links.draw(x, y);

        self.tabs[self.active_i].draw(x, y + 40.0);
    }
}

struct TabLink {
    text: TextLine,
    active: bool,
    padding: f32,
    size: (f32, f32),
    was_clicked: Cell<bool>,
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
            was_clicked: Cell::new(false),
        }
    }

    fn draw(&self, x: f32, y: f32) {
        if self.active {
            draw_rectangle(x, y, self.size.0, self.size.1, DARKGREEN);
        }

        if is_mouse_button_pressed(MouseButton::Left) {
            let (mouse_x, mouse_y) = mouse_position();
            if (x..=x + self.size.0).contains(&mouse_x) && (y..=y + self.size.1).contains(&mouse_y)
            {
                self.was_clicked.set(true);
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

    fn draw(&self, x: f32, y: f32) {
        let size = self.size();
        self.style.draw(x, y, size);

        let mut x0 = x + self.padding;
        let mut y0 = y + self.padding;
        for element in &self.elements {
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
    fn draw(&self, x: f32, y: f32) {
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

    fn unwrap_movement_enhancement(&self) -> MovementEnhancement {
        match self {
            ButtonAction::MovementEnhancement(enhancement) => *enhancement,
            _ => panic!(),
        }
    }
}

struct ActionButton {
    id: u32,
    action: ButtonAction,
    size: (f32, f32),
    style: Style,
    content: Box<Element>,
    hover_border_color: Color,
    points_row: Container,
    point_radius: f32,
    hovered: Cell<bool>,
    enabled: Cell<bool>,
    highlighted: Cell<bool>,
    event_sender: Option<EventSender>,
}

impl ActionButton {
    fn new(
        subtext: String,
        action: ButtonAction,
        event_queue: &Rc<RefCell<Vec<InternalUiEvent>>>,
        id: u32,
    ) -> Self {
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
                BaseAction::Move {
                    action_point_cost,
                    range,
                } => {
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
            ButtonAction::MovementEnhancement(enhancement) => {
                text = enhancement.name;
                action_points = enhancement.action_point_cost;
                stamina_points = enhancement.stamina_cost;
            }
        }

        let size = (90.0, 50.0);
        let style = Style {
            background_color: Some(DARKGRAY),
            border_color: Some(LIGHTGRAY),
        };
        let hover_border_color = YELLOW;

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
            hover_border_color,
            points_row,
            point_radius: r,
            hovered: Cell::new(false),
            enabled: Cell::new(true),
            highlighted: Cell::new(false),
            event_sender: Some(EventSender {
                queue: Rc::clone(&event_queue),
            }),
        }
    }

    fn toggle_highlighted(&self) {
        self.highlighted.set(!self.highlighted.get());
    }

    fn notify_hidden(&self) {
        if self.hovered.get() {
            if let Some(event_sender) = &self.event_sender {
                // Since this button has become hidden, it's no longer hovered
                event_sender.send(InternalUiEvent::ButtonHovered(self.id, self.action, false));
            }
        }
    }
}

impl Drawable for ActionButton {
    fn draw(&self, x: f32, y: f32) {
        let (w, h) = self.size;
        self.style.draw(x, y, self.size);

        let (mouse_x, mouse_y) = mouse_position();

        if self.enabled.get() {
            let hovered = (x..=x + w).contains(&mouse_x) && (y..=y + h).contains(&mouse_y);
            if hovered != self.hovered.get() {
                self.hovered.set(hovered);
                if let Some(event_sender) = &self.event_sender {
                    event_sender.send(InternalUiEvent::ButtonHovered(
                        self.id,
                        self.action,
                        hovered,
                    ));
                }
            }
            if hovered {
                if is_mouse_button_pressed(MouseButton::Left) {
                    if let Some(event_sender) = &self.event_sender {
                        event_sender.send(InternalUiEvent::ButtonClicked(self.id, self.action));
                    }
                }
                draw_rectangle_lines(x, y, w, h, 1.0, self.hover_border_color);
            }
        } else {
            draw_rectangle_lines(x, y, w, h, 1.0, GRAY);
        }

        if self.highlighted.get() {
            draw_rectangle_lines(x, y, w, h, 2.0, GREEN);
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

    fn size(&self) -> (f32, f32) {
        self.size
    }
}

enum InternalUiEvent {
    ButtonHovered(u32, ButtonAction, bool),
    ButtonClicked(u32, ButtonAction),
    SwitchedToMoveInGrid,
    SwitchedToAttackInGrid,
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
