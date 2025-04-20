use std::collections::HashMap;

use macroquad::{
    color::{Color, BLACK, LIGHTGRAY, MAGENTA, ORANGE},
    math::Vec2,
    shapes::{draw_rectangle_ex, draw_rectangle_lines_ex, DrawRectangleParams},
    text::{draw_text_ex, Font, TextParams},
};

use std::cell::Cell;

use macroquad::input::is_key_down;

use macroquad::miniquad::KeyCode;
use macroquad::texture::{draw_texture_ex, DrawTextureParams, Texture2D};
use macroquad::{
    color::{GRAY, GREEN, RED, WHITE, YELLOW},
    input::{is_mouse_button_down, is_mouse_button_pressed, mouse_position, MouseButton},
    shapes::{draw_circle, draw_circle_lines, draw_line, draw_rectangle, draw_rectangle_lines},
    text::measure_text,
};

use crate::{
    core::{ActionReach, BaseAction, Character, Goodness, MovementEnhancement, SpellTargetType},
    game_ui::UiState,
    pathfind::PathfindGrid,
    textures::SpriteId,
};
use crate::{
    core::{CharacterId, Characters, HandType, Range},
    drawing::{draw_arrow, draw_dashed_line},
};

const BACKGROUND_COLOR: Color = GRAY;
const GRID_COLOR: Color = Color::new(0.4, 0.4, 0.4, 1.0);

const MOVEMENT_PREVIEW_GRID_COLOR: Color = Color::new(0.5, 0.6, 0.5, 0.5);
const MOVEMENT_ARROW_COLOR: Color = Color::new(1.0, 0.63, 0.0, 1.0);
const HOVER_MOVEMENT_ARROW_COLOR: Color = Color::new(0.7, 0.6, 0.6, 0.8);
const HOVER_VALID_MOVEMENT_COLOR: Color = YELLOW;
const HOVER_INVALID_MOVEMENT_COLOR: Color = RED;

const HOVER_INVALID_TARGET_COLOR: Color = ORANGE;
const HOVER_TERRAIN_NEED_TARGET_COLOR: Color = LIGHTGRAY;

const HOVER_ENEMY_COLOR: Color = Color::new(0.8, 0.2, 0.2, 1.0);
const TARGET_ENEMY_COLOR: Color = Color::new(1.0, 0.0, 0.3, 1.0);

const HOVER_ALLY_COLOR: Color = Color::new(0.2, 0.8, 0.2, 1.0);
const INSPECTING_TARGET_COLOR: Color = GRAY;

const ACTIVE_CHARACTER_COLOR: Color = Color::new(1.0, 0.8, 0.0, 0.4);
const SELECTED_CHARACTER_COLOR: Color = WHITE;
const MOVE_RANGE_COLOR: Color = GREEN;

const RANGE_INDICATOR_BACKGROUND: Color = Color::new(0.3, 0.3, 0.3, 0.4);
const RANGE_INDICATOR_GOOD_COLOR: Color = GREEN;
const RANGE_INDICATOR_SEMI_BAD_COLOR: Color = ORANGE;
const RANGE_INDICATOR_BAD_COLOR: Color = RED;

const PLAYERS_TARGET_CROSSHAIR_COLOR: Color = WHITE;
const ENEMYS_TARGET_CROSSHAIR_COLOR: Color = MAGENTA;

#[derive(Debug, Copy, Clone)]
struct CharacterMotion {
    character_id: CharacterId,
    from: (i32, i32),
    to: (i32, i32),
    remaining_duration: f32,
    duration: f32,
}

struct MovementRange {
    options: Vec<(u32, f32)>,
    selected_i: usize,
}

impl MovementRange {
    fn max(&self) -> f32 {
        self.options[self.options.len() - 1].1
    }

    fn set(&mut self, range: f32, enhancements: Vec<MovementEnhancement>) {
        self.options = vec![(0, range)];
        for enhancement in enhancements {
            let enhanced_range = range * (1.0 + enhancement.add_percentage as f32 / 100.0);
            self.options
                .push((enhancement.add_percentage, enhanced_range));
        }

        self.selected_i = self.selected_i.min(self.options.len() - 1);
    }

    fn selected(&self) -> f32 {
        self.options[self.selected_i].1
    }

    fn set_selected_percentage(&mut self, enhancement_added_percentage: u32) {
        self.selected_i = self
            .options
            .iter()
            .position(|(add_percentage, _range)| *add_percentage == enhancement_added_percentage)
            .unwrap();
    }

    fn shortest_encompassing(&self, range: f32) -> usize {
        self.options.iter().position(|(_, r)| range <= *r).unwrap()
    }
}

impl Default for MovementRange {
    fn default() -> Self {
        Self {
            options: vec![(0, 0.0)],
            selected_i: 0,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum RangeIndicator {
    Good,
    GoodWithBackground,
    SemiBad,
    Bad,
}

pub struct GameGrid {
    big_font: Font,
    simple_font: Font,
    cell_w: f32,
    size: (f32, f32),
    background_textures: Vec<Texture2D>,
    cell_backgrounds: Vec<usize>,
    sprites: HashMap<SpriteId, Texture2D>,
    pathfind_grid: PathfindGrid,
    characters: Characters,

    character_motion: Option<CharacterMotion>,

    pub grid_dimensions: (i32, i32),
    pub position_on_screen: (f32, f32),

    camera_position: (Cell<f32>, Cell<f32>),
    dragging_camera_from: Option<(f32, f32)>,

    effects: Vec<ConcreteEffect>,
    selected_player_character_id: CharacterId,
    active_character_id: CharacterId,

    movement_range: MovementRange,
    selected_movement_path: Option<Vec<(f32, (i32, i32))>>,

    pub range_indicator: Option<(Range, RangeIndicator)>,
    players_action_target: Option<CharacterId>,
    players_inspect_target: Option<CharacterId>,
    enemys_target: Option<CharacterId>,
}

impl GameGrid {
    pub fn new(
        selected_character_id: CharacterId,
        characters: &Characters,
        sprites: HashMap<SpriteId, Texture2D>,
        size: (f32, f32),
        big_font: Font,
        simple_font: Font,
        background_textures: Vec<Texture2D>,
        grid_dimensions: (i32, i32),
        cell_backgrounds: Vec<usize>,
    ) -> Self {
        let characters = characters.clone();

        Self {
            sprites,
            pathfind_grid: PathfindGrid::new(grid_dimensions),
            dragging_camera_from: None,
            camera_position: (Cell::new(0.0), Cell::new(0.0)),
            characters,
            effects: vec![],
            selected_player_character_id: selected_character_id,
            active_character_id: 0,
            movement_range: MovementRange::default(),
            selected_movement_path: Default::default(),
            players_action_target: None,
            players_inspect_target: None,
            enemys_target: None,
            range_indicator: None,
            cell_w: 64.0,
            grid_dimensions,
            position_on_screen: (0.0, 0.0), // is set later
            character_motion: None,
            size,
            big_font,
            simple_font,
            background_textures,
            cell_backgrounds,
        }
    }

    pub fn set_character_motion(
        &mut self,
        character_id: CharacterId,
        from: (u32, u32),
        to: (u32, u32),
        duration: f32,
    ) {
        assert!(self.character_motion.is_none());
        self.character_motion = Some(CharacterMotion {
            character_id,
            from: (from.0 as i32, from.1 as i32),
            to: (to.0 as i32, to.1 as i32),
            remaining_duration: duration,
            duration,
        });
    }

    pub fn remove_dead(&mut self) {
        self.characters.remove_dead();
    }

    pub fn update(
        &mut self,
        active_character_id: CharacterId,
        selected_player_character_id: CharacterId,
        characters: &Characters,
        elapsed: f32,
    ) {
        self.pathfind_grid.blocked_positions.clear();

        self.active_character_id = active_character_id;
        self.selected_player_character_id = selected_player_character_id;

        for character in characters.iter() {
            let pos = character.position_i32();

            self.pathfind_grid.blocked_positions.insert(pos);
        }

        let pos = self.characters.get(self.active_character_id).position_i32();
        self.pathfind_grid.run(pos, self.movement_range.max());

        for effect in &mut self.effects {
            effect.age += elapsed;
        }
        self.effects.retain(|e| e.age <= e.end_time);

        if let Some(motion) = &mut self.character_motion {
            motion.remaining_duration -= elapsed;
            if motion.remaining_duration <= 0.0 {
                self.character_motion = None;
            }
        }

        let camera_speed = 5.0;
        if is_key_down(KeyCode::Left) {
            self.pan_camera(-camera_speed, 0.0);
        }
        if is_key_down(KeyCode::Right) {
            self.pan_camera(camera_speed, 0.0);
        }
        if is_key_down(KeyCode::Up) {
            self.pan_camera(0.0, -camera_speed);
        }
        if is_key_down(KeyCode::Down) {
            self.pan_camera(0.0, camera_speed);
        }
    }

    pub fn add_text_effect(
        &mut self,
        position: (i32, i32),
        start_time: f32,
        duration: f32,
        text: impl Into<String>,
        goodness: Goodness,
    ) {
        let pos = (
            self.grid_x_to_screen(position.0),
            self.grid_y_to_screen(position.1),
        );

        let color = match goodness {
            Goodness::Good => GREEN,
            Goodness::Neutral => YELLOW,
            Goodness::Bad => ORANGE,
        };

        let effect = ConcreteEffect {
            age: 0.0,
            start_time,
            end_time: start_time + duration,
            variant: EffectVariant::At(
                EffectPosition::Source,
                EffectGraphics::Text(text.into(), self.big_font.clone(), color),
            ),
            source_pos: pos,
            destination_pos: pos,
        };

        self.effects.push(effect);
    }

    pub fn add_effect(&mut self, source: (i32, i32), destination: (i32, i32), effect: Effect) {
        let source_pos = (
            self.grid_x_to_screen(source.0),
            self.grid_y_to_screen(source.1),
        );
        let destination_pos = (
            self.grid_x_to_screen(destination.0),
            self.grid_y_to_screen(destination.1),
        );

        let concrete_effect = ConcreteEffect {
            age: 0.0,
            start_time: effect.start_time,
            end_time: effect.end_time,
            source_pos,
            destination_pos,
            variant: effect.variant,
        };

        self.effects.push(concrete_effect);
    }

    fn set_an_arbitrary_valid_movement_path(&mut self) {
        let pos = self.characters.get(self.active_character_id).position_i32();
        let mut movement_preview = vec![];
        for (destination, route) in &self.pathfind_grid.routes {
            if route.came_from == pos
                && route.distance_from_start > 0.0
                && route.distance_from_start <= self.movement_range.selected()
            {
                movement_preview.push((route.distance_from_start, *destination));
                movement_preview.push((0.0, pos));
                break;
            }
        }
        assert!(movement_preview.len() > 1);
        self.selected_movement_path = Some(movement_preview);
    }

    pub fn remove_movement_preview(&mut self) {
        self.selected_movement_path = None;
    }

    pub fn has_non_empty_movement_preview(&self) -> bool {
        self.selected_movement_path
            .as_ref()
            .map(|m| !m.is_empty())
            .unwrap_or(false)
    }

    pub fn set_selected_movement_percentage(&mut self, enhancement_added_percentage: u32) {
        self.movement_range
            .set_selected_percentage(enhancement_added_percentage);
        self.ensure_movement_preview_is_within_selected_move_range();
    }

    pub fn set_movement_range_options(
        &mut self,
        range: f32,
        enhancements: Vec<MovementEnhancement>,
    ) {
        self.movement_range.set(range, enhancements);
        self.ensure_movement_preview_is_within_selected_move_range();
        let pos = self.characters.get(self.active_character_id).position_i32();
        self.pathfind_grid.run(pos, self.movement_range.max());
    }

    fn ensure_movement_preview_is_within_selected_move_range(&mut self) {
        if let Some(movement_preview) = &mut self.selected_movement_path {
            while !movement_preview.is_empty()
                && movement_preview[0].0 > self.movement_range.selected()
            {
                movement_preview.remove(0);
            }

            if movement_preview.len() == 1 {
                // A path consisting only of one node is not valid.
                self.set_an_arbitrary_valid_movement_path();
            }
        }
    }

    pub fn take_movement_path(&mut self) -> Vec<(u32, u32)> {
        let mut reversed_path = self.selected_movement_path.take().unwrap();

        // Remove the character's current position; it should not be part of the movement path
        reversed_path.remove(reversed_path.len() - 1);

        reversed_path
            .into_iter()
            .rev()
            .map(|(_dist, (x, y))| (x as u32, y as u32))
            .collect()
    }

    fn grid_x_to_screen(&self, grid_x: i32) -> f32 {
        self.position_on_screen.0 + grid_x as f32 * self.cell_w - self.camera_position.0.get()
    }

    fn grid_y_to_screen(&self, grid_y: i32) -> f32 {
        self.position_on_screen.1 + grid_y as f32 * self.cell_w - self.camera_position.1.get()
    }

    fn grid_pos_to_screen(&self, pos: (i32, i32)) -> (f32, f32) {
        (self.grid_x_to_screen(pos.0), self.grid_y_to_screen(pos.1))
    }

    fn character_screen_pos(&self, character: &Character) -> (f32, f32) {
        if let Some(motion) = self.character_motion {
            if motion.character_id == character.id() {
                let from = (
                    self.grid_x_to_screen(motion.from.0),
                    self.grid_y_to_screen(motion.from.1),
                );
                let to = (
                    self.grid_x_to_screen(motion.to.0),
                    self.grid_y_to_screen(motion.to.1),
                );
                let remaining = motion.remaining_duration / motion.duration;
                return (
                    to.0 - (to.0 - from.0) * remaining,
                    to.1 - (to.1 - from.1) * remaining,
                );
            }
        }
        (
            self.grid_x_to_screen(character.position_i32().0),
            self.grid_y_to_screen(character.position_i32().1),
        )
    }

    fn draw_cell_outline(
        &self,
        (grid_x, grid_y): (i32, i32),
        color: Color,
        margin: f32,
        thickness: f32,
    ) {
        draw_rectangle_lines(
            self.grid_x_to_screen(grid_x) + margin,
            self.grid_y_to_screen(grid_y) + margin,
            self.cell_w - margin * 2.0,
            self.cell_w - margin * 2.0,
            thickness,
            color,
        )
    }

    fn fill_cell(&self, (grid_x, grid_y): (i32, i32), color: Color) {
        let margin = 2.0;
        draw_rectangle(
            self.grid_x_to_screen(grid_x) + margin,
            self.grid_y_to_screen(grid_y) + margin,
            self.cell_w - margin * 2.0,
            self.cell_w - margin * 2.0,
            color,
        )
    }

    pub fn clear_players_action_target(&mut self) {
        self.players_action_target = None;
    }

    pub fn clear_enemys_target(&mut self) {
        self.enemys_target = None;
    }

    pub fn players_action_target(&self) -> Option<CharacterId> {
        self.players_action_target
    }

    pub fn clear_players_target_if_allied(&mut self) {
        self.players_action_target
            .take_if(|id| self.characters.get(*id).player_controlled);
        self.players_inspect_target
            .take_if(|id| self.characters.get(*id).player_controlled);
    }

    pub fn clear_players_action_target_if_enemy(&mut self) {
        self.players_action_target
            .take_if(|id| !self.characters.get(*id).player_controlled);
        self.players_inspect_target
            .take_if(|id| !self.characters.get(*id).player_controlled);
    }

    pub fn set_enemys_target(&mut self, target_character_id: CharacterId) {
        self.enemys_target = Some(target_character_id);
    }

    fn draw_background(&self) {
        for col in 0..self.grid_dimensions.0 + 1 {
            let x0 = self.grid_x_to_screen(col);
            draw_line(
                x0,
                self.grid_y_to_screen(0),
                x0,
                self.grid_y_to_screen(self.grid_dimensions.1),
                1.0,
                GRID_COLOR,
            );
            for row in 0..self.grid_dimensions.1 + 1 {
                let y0 = self.grid_y_to_screen(row);
                draw_line(
                    self.grid_x_to_screen(0),
                    y0,
                    self.grid_x_to_screen(self.grid_dimensions.0),
                    y0,
                    1.0,
                    GRID_COLOR,
                );

                if col < self.grid_dimensions.0 && row < self.grid_dimensions.1 {
                    let params = DrawTextureParams {
                        dest_size: Some(Vec2::new(self.cell_w, self.cell_w)),
                        ..Default::default()
                    };
                    let i = self.cell_backgrounds[(row * self.grid_dimensions.0 + col) as usize];
                    let texture = &self.background_textures[i];
                    draw_texture_ex(texture, x0, y0, WHITE, params);
                }
            }
        }
    }

    fn draw_character(&self, character: &Character) {
        let params = DrawTextureParams {
            dest_size: Some((self.cell_w, self.cell_w).into()),
            ..Default::default()
        };

        let (x, y) = self.character_screen_pos(character);

        draw_texture_ex(
            &self.sprites[&character.sprite],
            x,
            y,
            WHITE,
            params.clone(),
        );

        if let Some(weapon) = character.weapon(HandType::MainHand) {
            if let Some(texture) = weapon.sprite {
                draw_texture_ex(&self.sprites[&texture], x, y, WHITE, params.clone());
            }
        }

        if let Some(shield) = character.shield() {
            if let Some(texture) = shield.sprite {
                draw_texture_ex(&self.sprites[&texture], x, y, WHITE, params);
            }
        }
    }

    pub fn draw(&mut self, receptive_to_input: bool, ui_state: &UiState) -> GridOutcome {
        let previous_inspect_target = self.players_inspect_target;

        let (w, h) = self.size;

        let (x, y) = self.position_on_screen;

        let mouse_relative_to_grid = |(x, y): (f32, f32)| {
            (
                ((self.camera_position.0.get() + x) / self.cell_w).floor() as i32,
                ((self.camera_position.1.get() + y) / self.cell_w).floor() as i32,
            )
        };
        let (mouse_x, mouse_y) = mouse_position();
        let mouse_relative = (mouse_x - x, mouse_y - y);
        let (mouse_grid_x, mouse_grid_y) = mouse_relative_to_grid(mouse_relative);

        let is_mouse_within_grid = (0f32..w).contains(&mouse_relative.0)
            && (0..self.grid_dimensions.0).contains(&mouse_grid_x)
            && (0f32..h).contains(&mouse_relative.1)
            && (0..self.grid_dimensions.1).contains(&mouse_grid_y);

        if is_mouse_within_grid && receptive_to_input {
            if let Some(dragging_from) = self.dragging_camera_from {
                if is_mouse_button_down(MouseButton::Right)
                    || is_mouse_button_down(MouseButton::Middle)
                {
                    let (dx, dy) = (
                        mouse_relative.0 - dragging_from.0,
                        mouse_relative.1 - dragging_from.1,
                    );
                    self.pan_camera(-dx, -dy);
                    self.dragging_camera_from = Some(mouse_relative);
                } else {
                    self.dragging_camera_from = None;
                }
            }

            if is_mouse_button_pressed(MouseButton::Right)
                || is_mouse_button_pressed(MouseButton::Middle)
            {
                self.dragging_camera_from = Some(mouse_relative);
            }
        }

        draw_rectangle(x, y, w, h, BACKGROUND_COLOR);

        self.draw_background();

        let active_char_pos = self.characters.get(self.active_character_id).position_i32();

        if let Some((range, indicator)) = self.range_indicator {
            self.draw_range_indicator(active_char_pos, range, indicator);
        }

        self.draw_active_character_highlight();

        for character in self.characters.iter() {
            self.draw_character(character);
        }

        let mut outcome = GridOutcome::default();

        let mut hovered_character_id = None;
        for character in self.characters.iter() {
            if character.position_i32() == (mouse_grid_x, mouse_grid_y) {
                let id = character.id();
                outcome.hovered_character_id = Some(id);
                hovered_character_id = Some(id);
            }
        }

        if matches!(
            ui_state,
            UiState::ConfiguringAction(BaseAction::Move { .. })
        ) {
            self.draw_movement_path_background();
        }

        let pressed_left_mouse = is_mouse_button_pressed(MouseButton::Left);

        let mouse_state = match ui_state {
            UiState::ChoosingAction => MouseState::MayInputMovement,
            UiState::ConfiguringAction(base_action) => match base_action {
                BaseAction::Attack { .. } => MouseState::RequiresEnemyTarget,
                BaseAction::CastSpell(spell) => match spell.target_type {
                    SpellTargetType::NoTarget { .. } => MouseState::ImplicitTarget,
                    SpellTargetType::SingleEnemy { .. } => MouseState::RequiresEnemyTarget,
                    SpellTargetType::SingleAlly(..) => MouseState::RequiresAllyTarget,
                },
                BaseAction::Move { .. } => MouseState::MayInputMovement,
                BaseAction::SelfEffect(..) => MouseState::ImplicitTarget,
            },
            _ => MouseState::None,
        };

        /*
        let mut players_action_requires_enemy_target = false;
        let mut players_action_requires_ally_target = false;

        let mut may_input_movement = false;

        match ui_state {
            UiState::ChoosingAction => may_input_movement = true,
            UiState::ConfiguringAction(base_action) => match base_action {
                BaseAction::Attack { .. } => players_action_requires_enemy_target = true,
                BaseAction::CastSpell(spell) => match spell.target_type {
                    SpellTargetType::SelfAreaEnemy(..) => {}
                    SpellTargetType::SingleEnemy(..) => players_action_requires_enemy_target = true,
                    SpellTargetType::SingleAlly => players_action_requires_ally_target = true,
                },
                BaseAction::Move { .. } => may_input_movement = true,
                _ => {}
            },
            _ => {}
        }

          let players_action_requires_target =
            players_action_requires_ally_target || players_action_requires_enemy_target;
        */

        if is_mouse_within_grid && receptive_to_input {
            for character in self.characters.iter() {
                if character.position_i32() == (mouse_grid_x, mouse_grid_y) {
                    let id = character.id();
                    outcome.hovered_character_id = Some(id);
                    hovered_character_id = Some(id);
                }
            }

            let pressed_terrain = pressed_left_mouse && hovered_character_id.is_none();

            let player_has_action_target = self.players_action_target.is_some();

            if pressed_terrain {
                if matches!(
                    mouse_state,
                    MouseState::RequiresAllyTarget
                        | MouseState::RequiresEnemyTarget
                        | MouseState::ImplicitTarget
                ) {
                    outcome.switched_action = Some(GridSwitchedTo::Idle);
                }
                self.players_inspect_target = None;
            }

            if matches!(mouse_state, MouseState::RequiresEnemyTarget)
                && !player_has_action_target
                && hovered_character_id.is_none()
            {
                self.draw_cornered_outline(
                    self.grid_pos_to_screen((mouse_grid_x, mouse_grid_y)),
                    HOVER_TERRAIN_NEED_TARGET_COLOR,
                    5.0,
                    2.0,
                );
            }

            let hovered_move_route = self.pathfind_grid.routes.get(&(mouse_grid_x, mouse_grid_y));
            let mut valid_hovered_move_route = None;
            if matches!(mouse_state, MouseState::MayInputMovement) && hovered_character_id.is_none()
            {
                valid_hovered_move_route = hovered_move_route;
            }

            if let Some(hovered_route) = valid_hovered_move_route {
                if self.dragging_camera_from.is_none() && !player_has_action_target {
                    let path =
                        self.build_path_from_route(active_char_pos, (mouse_grid_x, mouse_grid_y));
                    self.draw_movement_path(&path, true);

                    if pressed_left_mouse {
                        self.movement_range.selected_i = self
                            .movement_range
                            .shortest_encompassing(hovered_route.distance_from_start);
                        outcome.switched_action = Some(GridSwitchedTo::Move {
                            selected_option: self.movement_range.selected_i,
                        });

                        self.selected_movement_path = Some(path);
                    }
                }
            } else if let Some(hovered_id) = hovered_character_id {
                let player_controlled = self.characters.get(hovered_id).player_controlled;

                if player_controlled {
                    if matches!(mouse_state, MouseState::RequiresAllyTarget) {
                        self.draw_cornered_outline(
                            self.grid_pos_to_screen((mouse_grid_x, mouse_grid_y)),
                            HOVER_ALLY_COLOR,
                            5.0,
                            4.0,
                        );
                    }

                    if pressed_left_mouse {
                        if self.active_character_id == hovered_id {
                            outcome.switched_action = Some(GridSwitchedTo::Idle);
                        } else {
                            match mouse_state {
                                MouseState::RequiresAllyTarget => {
                                    self.players_action_target = Some(hovered_id);
                                    self.players_inspect_target = Some(hovered_id);
                                    self.selected_movement_path = None;
                                }
                                //MouseState::RequiresEnemyTarget => {}
                                _ => {
                                    self.players_inspect_target = Some(hovered_id);
                                }
                            }
                        }
                    }
                } else {
                    if matches!(mouse_state, MouseState::RequiresEnemyTarget) {
                        self.draw_cornered_outline(
                            self.grid_pos_to_screen((mouse_grid_x, mouse_grid_y)),
                            HOVER_ENEMY_COLOR,
                            5.0,
                            3.0,
                        );
                    }

                    if pressed_left_mouse {
                        let mut may_acquire_attack_target = matches!(
                            ui_state,
                            UiState::ChoosingAction
                                | UiState::ConfiguringAction(
                                    BaseAction::Move { .. } | BaseAction::Attack { .. }
                                )
                        );

                        if player_has_action_target
                            && matches!(mouse_state, MouseState::RequiresAllyTarget)
                        {
                            may_acquire_attack_target = true; // i.e. change action to attack
                        }

                        if may_acquire_attack_target {
                            let is_configuring_attack = matches!(
                                ui_state,
                                UiState::ConfiguringAction(BaseAction::Attack { .. })
                            );
                            if !(is_configuring_attack) {
                                outcome.switched_action = Some(GridSwitchedTo::Attack);
                            }

                            self.players_action_target = Some(hovered_id);
                            self.players_inspect_target = Some(hovered_id);
                            self.selected_movement_path = None;
                        } else if matches!(mouse_state, MouseState::RequiresEnemyTarget) {
                            self.players_action_target = Some(hovered_id);
                            self.players_inspect_target = Some(hovered_id);
                        } else if !matches!(mouse_state, MouseState::RequiresAllyTarget) {
                            self.players_inspect_target = Some(hovered_id);
                        }
                    }
                }
            } else if self.selected_movement_path.is_some() && self.dragging_camera_from.is_none() {
                self.draw_cell_outline(
                    (mouse_grid_x, mouse_grid_y),
                    HOVER_INVALID_MOVEMENT_COLOR,
                    5.0,
                    2.0,
                );
            }
        }

        {
            let pos =
                self.character_screen_pos(self.characters.get(self.selected_player_character_id));
            self.draw_cornered_outline(pos, SELECTED_CHARACTER_COLOR, -1.0, 2.0);
        }

        if let Some(path) = &self.selected_movement_path {
            if !path.is_empty() {
                self.draw_movement_path(path, false);
            }
        }

        if let Some(target) = self.players_inspect_target {
            self.draw_cornered_outline(
                self.character_screen_pos(self.characters.get(target)),
                INSPECTING_TARGET_COLOR,
                4.0,
                2.0,
            );
        }

        if let Some(target) = self.players_action_target {
            self.draw_target_crosshair(target, PLAYERS_TARGET_CROSSHAIR_COLOR);
        }

        if let Some(target) = self.enemys_target {
            self.draw_target_crosshair(target, ENEMYS_TARGET_CROSSHAIR_COLOR);
        }

        self.draw_effects();

        self.draw_character_label(self.characters.get(self.active_character_id));

        if let Some(id) = hovered_character_id {
            if id != self.active_character_id {
                let char = self.characters.get(id);
                self.draw_character_label(char);
            }
        }

        if self.players_inspect_target != previous_inspect_target {
            outcome.switched_inspect_target = Some(self.players_inspect_target);
        }

        outcome
    }

    fn draw_character_label(&self, char: &Character) {
        let (x, y) = self.character_screen_pos(char);
        let y = y - 5.0;

        let font_size = 14;
        let params = TextParams {
            font: Some(&self.big_font),
            font_size,
            color: WHITE,
            ..Default::default()
        };

        let margin = 2.0;
        let healthbar_w = self.cell_w;
        let healthbar_h = 5.0;

        let header = char.name;

        let text_dimensions = measure_text(header, Some(&self.big_font), font_size, 1.0);

        let text_pad = 2.0;
        let box_w = text_dimensions.width + text_pad * 2.0;
        let box_h = text_dimensions.height + text_pad * 2.0;
        let box_x = x - (box_w - self.cell_w) / 2.0;
        let box_y = y - healthbar_h - margin - box_h;

        draw_rectangle(box_x, box_y, box_w, box_h, Color::new(0.0, 0.0, 0.0, 0.5));
        draw_text_ex(
            header,
            box_x + text_pad,
            box_y + text_pad + text_dimensions.offset_y,
            params,
        );

        draw_rectangle(x, y - healthbar_h, healthbar_w, healthbar_h, BLACK);
        draw_rectangle(
            x,
            y - healthbar_h,
            healthbar_w * (char.health.current() as f32 / char.health.max as f32),
            healthbar_h,
            RED,
        );
    }

    fn draw_active_character_highlight(&self) {
        let (x, y) = self.character_screen_pos(self.characters.get(self.active_character_id));
        let margin = 3.0;
        draw_rectangle(
            x + margin,
            y + margin,
            self.cell_w - margin * 2.0,
            self.cell_w - margin * 2.0,
            ACTIVE_CHARACTER_COLOR,
        );
    }

    fn draw_cornered_outline(
        &self,
        screen_pos: (f32, f32),
        color: Color,
        margin: f32,
        thickness: f32,
    ) {
        let (x, y) = screen_pos;

        let left = x + margin;
        let top = y + margin;
        let right = x + self.cell_w - margin;
        let bot = y + self.cell_w - margin;
        let len = 15.0;

        draw_line(left, top, left, top + len, thickness, color);
        draw_line(left, top, left + len, top, thickness, color);
        draw_line(right - len, top, right, top, thickness, color);
        draw_line(right, top, right, top + len, thickness, color);
        draw_line(right, bot - len, right, bot, thickness, color);
        draw_line(right - len, bot, right, bot, thickness, color);
        draw_line(left, bot, left + len, bot, thickness, color);
        draw_line(left, bot, left, bot - len, thickness, color);
    }

    fn draw_effects(&self) {
        for effect in &self.effects {
            if effect.age < effect.start_time {
                continue;
            }
            let t = (effect.age - effect.start_time) / (effect.end_time - effect.start_time);
            match &effect.variant {
                EffectVariant::At(position, graphics) => {
                    let (x, y) = match position {
                        EffectPosition::Source => effect.source_pos,
                        EffectPosition::Destination => effect.destination_pos,
                        EffectPosition::Projectile => {
                            let x = effect.source_pos.0
                                + (effect.destination_pos.0 - effect.source_pos.0) * t;
                            let y = effect.source_pos.1
                                + (effect.destination_pos.1 - effect.source_pos.1) * t;
                            (x, y)
                        }
                    };
                    graphics.draw(x, y, effect, self.cell_w);
                }
                EffectVariant::Line {
                    thickness,
                    end_thickness,
                    color,
                    extend_gradually,
                } => {
                    let from = (
                        effect.source_pos.0 + self.cell_w / 2.0,
                        effect.source_pos.1 + self.cell_w / 2.0,
                    );
                    let mut to = (
                        effect.destination_pos.0 + self.cell_w / 2.0,
                        effect.destination_pos.1 + self.cell_w / 2.0,
                    );

                    if *extend_gradually {
                        to = (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
                    }

                    let thickness = match end_thickness {
                        Some(end_thickness) => thickness + (end_thickness - thickness) * t,
                        None => *thickness,
                    };
                    draw_line(from.0, from.1, to.0, to.1, thickness, *color);
                }
            }
        }
    }

    fn build_path_from_route(
        &self,
        start: (i32, i32),
        destination: (i32, i32),
    ) -> Vec<(f32, (i32, i32))> {
        let route = self.pathfind_grid.routes.get(&destination).unwrap();
        let mut dist = route.distance_from_start;

        let mut movement_preview = vec![(dist, destination)];
        let mut pos = route.came_from;

        loop {
            let route = self.pathfind_grid.routes.get(&pos).unwrap();
            dist = route.distance_from_start;
            movement_preview.push((dist, pos));
            if pos == start {
                break;
            }
            pos = route.came_from;
        }
        assert!(movement_preview.len() > 1);
        movement_preview
    }

    fn draw_target_crosshair(&self, target: CharacterId, crosshair_color: Color) {
        let actor_pos = self.characters.get(self.active_character_id).position_i32();
        let target_pos = self.characters.get(target).position_i32();

        let actor_x = self.grid_x_to_screen(actor_pos.0);
        let actor_y = self.grid_y_to_screen(actor_pos.1);

        let target_x = self.grid_x_to_screen(target_pos.0);
        let target_y = self.grid_y_to_screen(target_pos.1);

        draw_circle_lines(
            target_x + self.cell_w / 2.0,
            target_y + self.cell_w / 2.0,
            self.cell_w * 0.15,
            3.0,
            crosshair_color,
        );
        draw_arrow((target_x, target_y), self.cell_w, (1, 1), crosshair_color);
        draw_arrow((target_x, target_y), self.cell_w, (-1, -1), crosshair_color);

        draw_dashed_line(
            (actor_x + self.cell_w / 2.0, actor_y + self.cell_w / 2.0),
            (target_x + self.cell_w / 2.0, target_y + self.cell_w / 2.0),
            4.0,
            crosshair_color,
            5.0,
        );
    }

    fn draw_static_text(
        &self,
        header: &str,
        text_color: Color,
        bg_color: Color,
        pad: f32,
        mut x: f32,
        y: f32,
    ) {
        let header_font_size = 14;
        let params = TextParams {
            font: Some(&self.big_font),
            font_size: header_font_size,
            color: text_color,
            ..Default::default()
        };

        let header_dimensions = measure_text(header, Some(&self.big_font), header_font_size, 1.0);
        let header_w = header_dimensions.width;
        let mut header_h = 0.0;
        if header_dimensions.height.is_finite() {
            header_h = header_dimensions.height;
        }

        let w = header_w + 2.0 * pad;
        let h = header_h + 2.0 * pad;

        if w > self.cell_w {
            x -= (w - self.cell_w) / 2.0;
        }

        draw_rectangle(x, y - h, w, h, bg_color);
        draw_text_ex(
            header,
            x + pad,
            y - h + pad + header_dimensions.offset_y,
            params.clone(),
        );
    }

    fn draw_movement_path_background(&self) {
        let active_char_pos = self.characters.get(self.active_character_id).position_i32();

        self.draw_move_range_indicator(active_char_pos);

        for pos in self.pathfind_grid.routes.keys() {
            if (0..self.grid_dimensions.0).contains(&pos.0)
                && (0..self.grid_dimensions.1).contains(&pos.1)
                && *pos != active_char_pos
            {
                self.fill_cell(*pos, MOVEMENT_PREVIEW_GRID_COLOR);
            }
        }
    }

    fn draw_movement_path(&self, path: &[(f32, (i32, i32))], hover: bool) {
        if hover {
            self.draw_movement_path_arrow(path, HOVER_MOVEMENT_ARROW_COLOR, 2.0);
        } else {
            self.draw_movement_path_arrow(path, MOVEMENT_ARROW_COLOR, 4.0);
        };

        let distance = path[0].0;
        let destination = path[0].1;
        let (x, y) = (
            self.grid_x_to_screen(destination.0),
            self.grid_y_to_screen(destination.1) + 14.0,
        );

        let text_color = if hover { LIGHTGRAY } else { WHITE };
        let bg_color = if hover {
            Color::new(0.0, 0.0, 0.0, 0.5)
        } else {
            Color::new(0.0, 0.0, 0.0, 0.7)
        };
        self.draw_static_text(
            &format!("{:.4}", distance.to_string()),
            text_color,
            bg_color,
            4.0,
            x,
            y,
        );
    }

    fn draw_movement_path_arrow(&self, path: &[(f32, (i32, i32))], color: Color, thickness: f32) {
        for i in 0..path.len() - 1 {
            let a = path[i].1;
            let b = path[i + 1].1;
            draw_dashed_line(
                (
                    self.grid_x_to_screen(a.0) + self.cell_w / 2.0,
                    self.grid_y_to_screen(a.1) + self.cell_w / 2.0,
                ),
                (
                    self.grid_x_to_screen(b.0) + self.cell_w / 2.0,
                    self.grid_y_to_screen(b.1) + self.cell_w / 2.0,
                ),
                thickness,
                color,
                5.0,
            );
        }

        let end = path[0].1;
        if path.len() < 2 {
            panic!("Expected at least two nodes in path, but got: {:?}", path);
        }
        let last_direction = (end.0 - path[1].1 .0, end.1 - path[1].1 .1);

        draw_arrow(
            (self.grid_x_to_screen(end.0), self.grid_y_to_screen(end.1)),
            self.cell_w,
            last_direction,
            color,
        );
    }

    fn draw_move_range_indicator(&self, origin: (i32, i32)) {
        let range = self.movement_range.selected();
        let range_ceil = range.ceil() as i32;

        let within = |x: i32, y: i32| {
            self.pathfind_grid
                .routes
                .get(&(x, y))
                .map(|route| route.distance_from_start <= range)
                .unwrap_or(false)
        };

        for x in
            (origin.0 - range_ceil).max(0)..=(origin.0 + range_ceil).min(self.grid_dimensions.0 - 1)
        {
            for y in (origin.1 - range_ceil).max(0)
                ..=(origin.1 + range_ceil).min(self.grid_dimensions.1 - 1)
            {
                let thickness = 2.0;

                if within(x, y) {
                    self.draw_dashed_borders(
                        x,
                        y,
                        !within(x - 1, y),
                        !within(x + 1, y),
                        !within(x, y - 1),
                        !within(x, y + 1),
                        thickness,
                        MOVE_RANGE_COLOR,
                    );
                }
            }
        }
    }

    fn draw_range_indicator(&self, origin: (i32, i32), range: Range, indicator: RangeIndicator) {
        let range_ceil = (f32::from(range)).ceil() as i32;
        let range_squared = range.squared() as i32;
        let draw_background = matches!(indicator, RangeIndicator::GoodWithBackground);
        let color = match indicator {
            RangeIndicator::Good => RANGE_INDICATOR_GOOD_COLOR,
            RangeIndicator::GoodWithBackground => LIGHTGRAY,
            RangeIndicator::SemiBad => RANGE_INDICATOR_SEMI_BAD_COLOR,
            RangeIndicator::Bad => RANGE_INDICATOR_BAD_COLOR,
        };
        let is_cell_within =
            |x: i32, y: i32| (x - origin.0).pow(2) + (y - origin.1).pow(2) <= range_squared;
        for x in
            (origin.0 - range_ceil).max(0)..=(origin.0 + range_ceil).min(self.grid_dimensions.0 - 1)
        {
            for y in (origin.1 - range_ceil).max(0)
                ..=(origin.1 + range_ceil).min(self.grid_dimensions.1 - 1)
            {
                if is_cell_within(x, y) {
                    let mut thickness = 2.0;
                    if draw_background {
                        self.fill_cell((x, y), RANGE_INDICATOR_BACKGROUND);
                        thickness = 1.0;
                    }

                    self.draw_dashed_borders(
                        x,
                        y,
                        !is_cell_within(x - 1, y),
                        !is_cell_within(x + 1, y),
                        !is_cell_within(x, y - 1),
                        !is_cell_within(x, y + 1),
                        thickness,
                        color,
                    );
                }
            }
        }
    }

    fn draw_dashed_borders(
        &self,
        x: i32,
        y: i32,
        left: bool,
        right: bool,
        top: bool,
        bottom: bool,
        thickness: f32,
        color: Color,
    ) {
        let segment_len = 8.0;
        if left {
            // Left border
            draw_dashed_line(
                (self.grid_x_to_screen(x), self.grid_y_to_screen(y)),
                (self.grid_x_to_screen(x), self.grid_y_to_screen(y + 1)),
                thickness,
                color,
                segment_len,
            );
        }
        if right {
            // Right border
            draw_dashed_line(
                (self.grid_x_to_screen(x + 1), self.grid_y_to_screen(y)),
                (self.grid_x_to_screen(x + 1), self.grid_y_to_screen(y + 1)),
                thickness,
                color,
                segment_len,
            );
        }
        if top {
            // Top border
            draw_dashed_line(
                (self.grid_x_to_screen(x), self.grid_y_to_screen(y)),
                (self.grid_x_to_screen(x + 1), self.grid_y_to_screen(y)),
                thickness,
                color,
                segment_len,
            );
        }
        if bottom {
            // Bottom border
            draw_dashed_line(
                (self.grid_x_to_screen(x), self.grid_y_to_screen(y + 1)),
                (self.grid_x_to_screen(x + 1), self.grid_y_to_screen(y + 1)),
                thickness,
                color,
                segment_len,
            );
        }
    }

    fn pan_camera(&self, dx: f32, dy: f32) {
        let new_x = self.camera_position.0.get() + dx;
        let new_y = self.camera_position.1.get() + dy;
        let max_space = 250.0;
        let max_x = self.grid_dimensions.0 as f32 * self.cell_w + max_space - self.size.0;
        let max_y = self.grid_dimensions.1 as f32 * self.cell_w + max_space - self.size.1;
        self.camera_position.0.set(new_x.max(-max_space).min(max_x));
        self.camera_position.1.set(new_y.max(-max_space).min(max_y));
    }
}

#[derive(Debug, Default)]
pub struct GridOutcome {
    pub switched_action: Option<GridSwitchedTo>,
    pub hovered_character_id: Option<CharacterId>,
    pub switched_inspect_target: Option<Option<CharacterId>>,
}

#[derive(Debug)]
pub enum GridSwitchedTo {
    Move { selected_option: usize },
    Attack,
    Idle,
}

struct ConcreteEffect {
    age: f32,
    start_time: f32,
    end_time: f32,
    source_pos: (f32, f32),
    destination_pos: (f32, f32),
    variant: EffectVariant,
}

pub struct Effect {
    pub start_time: f32,
    pub end_time: f32,
    pub variant: EffectVariant,
}

pub enum EffectVariant {
    At(EffectPosition, EffectGraphics),
    Line {
        color: Color,
        thickness: f32,
        end_thickness: Option<f32>,
        extend_gradually: bool,
    },
}

pub enum EffectPosition {
    Source,
    Destination,
    Projectile,
}

pub enum EffectGraphics {
    Circle {
        radius: f32,
        end_radius: Option<f32>,
        fill: Option<Color>,
        stroke: Option<(Color, f32)>,
    },
    Rectangle {
        width: f32,
        end_width: Option<f32>,
        start_rotation: f32,
        rotation_per_s: f32,
        fill: Option<Color>,
        stroke: Option<(Color, f32)>,
    },
    Text(String, Font, Color),
}

impl EffectGraphics {
    fn draw(&self, mut x: f32, mut y: f32, effect: &ConcreteEffect, cell_w: f32) {
        if effect.age < effect.start_time {
            return;
        }
        let t = (effect.age - effect.start_time) / (effect.end_time - effect.start_time);
        match self {
            EffectGraphics::Circle {
                radius,
                end_radius,
                fill,
                stroke,
            } => {
                x += cell_w / 2.0;
                y += cell_w / 2.0;
                let r = match end_radius {
                    None => *radius,
                    Some(end_radius) => *radius + (end_radius - radius) * t,
                };
                if let Some(color) = fill {
                    draw_circle(x, y, r, *color);
                }
                if let Some((color, thickness)) = stroke {
                    draw_circle_lines(x, y, r, *thickness, *color);
                }
            }
            EffectGraphics::Rectangle {
                width,
                end_width,
                start_rotation,
                rotation_per_s,
                fill,
                stroke,
            } => {
                x += cell_w / 2.0;
                y += cell_w / 2.0;
                let rotation = *start_rotation + *rotation_per_s * effect.age;

                let width = match end_width {
                    None => *width,
                    Some(end_width) => *width + (end_width - width) * t,
                };

                if let Some(color) = fill {
                    draw_rectangle_ex(
                        x,
                        y,
                        width,
                        width,
                        DrawRectangleParams {
                            offset: Vec2::splat(0.5),
                            rotation,
                            color: *color,
                        },
                    );
                }
                if let Some((color, thickness)) = stroke {
                    draw_rectangle_lines_ex(
                        x,
                        y,
                        width,
                        width,
                        *thickness,
                        DrawRectangleParams {
                            offset: Vec2::splat(0.5),
                            rotation,
                            color: *color,
                        },
                    );
                }
            }
            EffectGraphics::Text(text, font, color) => {
                let font_size = 20;
                let text_dimensions = measure_text(text, None, font_size, 1.0);

                let x0 = x + cell_w / 2.0 - text_dimensions.width / 2.0;
                let y0 = y - cell_w * 0.3 * t;

                let mut text_params = TextParams {
                    font: Some(font),
                    font_size,
                    color: BLACK,
                    ..Default::default()
                };
                draw_text_ex(text, x0 + 2.0, y0 + 2.0, text_params.clone());
                text_params.color = *color;
                draw_text_ex(text, x0, y0, text_params);
            }
        }
    }
}

enum MouseState {
    RequiresEnemyTarget,
    RequiresAllyTarget,
    ImplicitTarget,
    MayInputMovement,
    None,
}
