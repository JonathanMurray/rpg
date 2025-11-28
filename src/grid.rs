use std::{cmp::Ordering, collections::HashMap, f32::consts::PI, iter, rc::Rc};

use indexmap::IndexMap;
use macroquad::{
    color::{Color, BLACK, DARKBROWN, GRAY, LIGHTGRAY, MAGENTA, ORANGE},
    input::mouse_wheel,
    math::Vec2,
    shapes::{draw_rectangle_ex, draw_rectangle_lines_ex, DrawRectangleParams},
    text::{draw_text_ex, Font, TextParams},
    window::{screen_height, screen_width},
};
use rand::{random_range, rng, Rng};

use std::cell::Cell;

use macroquad::input::is_key_down;

use macroquad::miniquad::KeyCode;
use macroquad::texture::{draw_texture_ex, DrawTextureParams, Texture2D};
use macroquad::{
    color::{GREEN, RED, WHITE, YELLOW},
    input::{is_mouse_button_down, is_mouse_button_pressed, mouse_position, MouseButton},
    shapes::{draw_circle, draw_circle_lines, draw_line, draw_rectangle, draw_rectangle_lines},
    text::measure_text,
};

use crate::{
    base_ui::{draw_text_rounded, Drawable, Style},
    bot::convert_path_to_move_action,
    core::{
        within_range_squared, AbilityReach, AbilityTarget, ActionReach, ActionTarget, AttackAction,
        BaseAction, Character, Goodness, Position,
    },
    drawing::{
        draw_cornered_rectangle_lines, draw_cross, draw_crosshair, draw_dashed_rectangle_sides,
    },
    game_ui::{ConfiguredAction, UiState},
    game_ui_components::ActionPointsRow,
    game_ui_connection::DEBUG,
    pathfind::{build_path_from_chart, ChartNode, PathfindGrid},
    textures::{character_sprite_height, draw_terrain, SpriteId, TerrainId},
};
use crate::{
    core::{CharacterId, Characters, HandType, Range},
    drawing::{draw_arrow, draw_dashed_line},
};

const BACKGROUND_COLOR: Color = Color::new(0.2, 0.2, 0.2, 1.0);
const GRID_COLOR: Color = Color::new(0.4, 0.4, 0.4, 1.0);

const MOVEMENT_PREVIEW_GRID_COLOR: Color = Color::new(0.9, 0.9, 0.9, 0.08);
const MOVEMENT_PREVIEW_GRID_OUTLINE_COLOR: Color = Color::new(0.9, 0.9, 0.9, 0.15);
const MOVEMENT_ARROW_COLOR: Color = Color::new(1.0, 0.63, 0.0, 1.0);
const HOVER_MOVEMENT_ARROW_COLOR: Color = Color::new(0.7, 0.6, 0.6, 0.8);
const HOVER_VALID_MOVEMENT_COLOR: Color = YELLOW;
const HOVER_INVALID_MOVEMENT_COLOR: Color = RED;

const HOVER_INVALID_TARGET_COLOR: Color = ORANGE;
const HOVER_TERRAIN_NEED_CHAR_TARGET_COLOR: Color = LIGHTGRAY;

const HOVER_ENEMY_COLOR: Color = Color::new(0.8, 0.2, 0.2, 1.0);
const TARGET_ENEMY_COLOR: Color = Color::new(1.0, 0.0, 0.3, 1.0);
const HOVER_ALLY_COLOR: Color = Color::new(0.2, 0.8, 0.2, 1.0);
const INSPECTING_TARGET_COLOR: Color = LIGHTGRAY;

const ACTIVE_CHARACTER_COLOR: Color = Color::new(1.0, 0.8, 0.0, 0.4);
const SELECTED_CHARACTER_COLOR: Color = WHITE;
const MOVE_RANGE_COLOR: Color = Color::new(0.2, 0.8, 0.2, 0.8);

const ACTION_RANGE_INDICATOR_BACKGROUND: Color = Color::new(0.7, 0.7, 0.7, 0.1);
const RANGE_INDICATOR_GOOD_COLOR: Color = GREEN;
const RANGE_INDICATOR_SEMI_BAD_COLOR: Color = ORANGE;
const RANGE_INDICATOR_BAD_COLOR: Color = RED;

const PLAYERS_TARGET_CROSSHAIR_COLOR: Color = WHITE;
const HOVER_PLAYERS_TARGET_CROSSHAIR_COLOR: Color = Color::new(0.7, 0.7, 0.7, 0.8);
const ENEMYS_TARGET_CROSSHAIR_COLOR: Color = MAGENTA;

#[derive(Debug, Copy, Clone)]
struct CharacterAnimation {
    character_id: CharacterId,
    duration: f32,
    remaining_duration: f32,
    kind: AnimationKind,
}

impl CharacterAnimation {
    fn new(character_id: CharacterId, duration: f32, kind: AnimationKind) -> CharacterAnimation {
        Self {
            character_id,
            duration,
            remaining_duration: duration,
            kind,
        }
    }
}

#[derive(Debug, Copy, Clone)]
enum AnimationKind {
    Motion { from: Position, to: Position },
    Shake { random_time_offset: f32 },
    Death,
    Act { random_rotation: f32 },
}

struct MovementRange {
    speed: f32,
    max_range: f32,
}

impl MovementRange {
    fn max(&self) -> f32 {
        self.max_range
    }

    fn set(&mut self, speed: f32, max_range: f32) {
        self.speed = speed;
        self.max_range = max_range;
    }

    fn cost(&self, range: f32, character_remaining_movement: f32) -> u32 {
        let extra_range = range - character_remaining_movement;
        if extra_range <= 0.0 {
            0
        } else {
            // Costs 1 per extra range
            extra_range.ceil() as u32
            //(extra_range / self.speed).ceil() as u32
        }
        //self.options.iter().position(|(_, r)| range <= *r).unwrap()
    }
}

impl Default for MovementRange {
    fn default() -> Self {
        Self {
            speed: 0.0,
            max_range: 0.0,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum RangeIndicator {
    ActionTargetRange,
    TargetAreaEffect,
    CanReachButDisadvantage,
    CannotReach,
}

pub struct GameGrid {
    big_font: Font,
    simple_font: Font,
    cell_w: f32,
    terrain_atlas: Texture2D,
    background: HashMap<Position, TerrainId>,
    terrain_objects: HashMap<Position, TerrainId>,
    sprites: HashMap<SpriteId, Texture2D>,
    pathfind_grid: Rc<PathfindGrid>,
    routes: IndexMap<Position, ChartNode>,
    characters: Characters,

    character_animations: Vec<CharacterAnimation>,
    pub grid_dimensions: (u32, u32),
    pub position_on_screen: (f32, f32),

    zoom_index: usize,
    camera_position: (Cell<f32>, Cell<f32>),
    dragging_camera_from: Option<(f32, f32)>,

    effects: Vec<ConcreteEffect>,
    selected_player_character_id: Option<CharacterId>,
    active_character_id: CharacterId,

    movement_range: MovementRange,

    players_inspect_target: Option<CharacterId>,
    enemys_target: Option<CharacterId>,
}

const ZOOM_LEVELS: [f32; 6] = [40.0, 48.0, 64.0, 96.0, 112.0, 128.0];

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TextEffectStyle {
    Friendly,
    Miss,
    ReactionExclamation,
    HostileGraze,
    HostileHit,
    HostileCrit,
}

impl GameGrid {
    pub fn new(
        selected_character_id: CharacterId,
        characters: Characters,
        sprites: HashMap<SpriteId, Texture2D>,
        big_font: Font,
        simple_font: Font,
        terrain_atlas: Texture2D,
        pathfind_grid: Rc<PathfindGrid>,
        background: HashMap<Position, TerrainId>,
        terrain_objects: HashMap<Position, TerrainId>,
    ) -> Self {
        let zoom_index = 2;
        let cell_w = ZOOM_LEVELS[zoom_index];

        let grid_dimensions = pathfind_grid.dimensions();
        Self {
            sprites,
            pathfind_grid,
            routes: Default::default(),
            dragging_camera_from: None,
            camera_position: (Cell::new(0.0), Cell::new(0.0)),
            characters,
            effects: vec![],
            selected_player_character_id: Some(selected_character_id),
            active_character_id: 0,
            movement_range: MovementRange::default(),
            players_inspect_target: None,
            enemys_target: None,
            zoom_index,
            cell_w,
            grid_dimensions,
            position_on_screen: (0.0, 0.0),
            character_animations: Default::default(),
            /*
            character_motion: None,
            character_dying_animations: vec![],
            character_shake_animations: vec![],
            character_act_animations: vec![],
             */
            big_font,
            simple_font,
            terrain_atlas,
            background,
            terrain_objects,
        }
    }

    pub fn set_character_motion(
        &mut self,
        character_id: CharacterId,
        from: Position,
        to: Position,
        duration: f32,
    ) {
        self.character_animations.push(CharacterAnimation::new(
            character_id,
            duration,
            AnimationKind::Motion { from, to },
        ));
    }

    pub fn animate_death(&mut self, character_id: CharacterId, duration: f32) {
        self.character_animations.push(CharacterAnimation::new(
            character_id,
            duration,
            AnimationKind::Death,
        ));
    }

    pub fn animate_character_acting(&mut self, character_id: CharacterId, duration: f32) {
        let random_rotation = random_range(-0.05..0.05);
        self.character_animations.push(CharacterAnimation::new(
            character_id,
            duration,
            AnimationKind::Act { random_rotation },
        ));
    }

    pub fn animate_character_shaking(&mut self, character_id: CharacterId, duration: f32) {
        let random_time_offset = random_range(0.0..1.0);

        self.character_animations.push(CharacterAnimation::new(
            character_id,
            duration,
            AnimationKind::Shake { random_time_offset },
        ));
    }

    pub fn remove_dead(&mut self) {
        let removed = self.characters.remove_dead();
        for id in removed {
            if self.players_inspect_target == Some(id) {
                self.players_inspect_target = None;
            }

            if self.selected_player_character_id == Some(id) {
                // TODO what about when all PC:s have died?
                self.selected_player_character_id = self
                    .characters
                    .iter()
                    .find(|ch| ch.player_controlled())
                    .map(|ch| ch.id());
            }
        }
    }

    pub fn update(
        &mut self,
        active_character_id: CharacterId,
        selected_player_character_id: Option<CharacterId>,
        elapsed: f32,
    ) {
        self.active_character_id = active_character_id;

        self.selected_player_character_id = selected_player_character_id;

        let pos = self.characters.get(self.active_character_id).pos();
        self.routes = self
            .pathfind_grid
            .explore_outward(pos, self.movement_range.max());

        for effect in &mut self.effects {
            effect.age += elapsed;
        }
        self.effects.retain(|e| e.age <= e.end_time);

        for a in self.character_animations.iter_mut() {
            a.remaining_duration -= elapsed;
        }
        self.character_animations
            .retain(|a| a.remaining_duration > 0.0);

        /*
        if let Some(motion) = &mut self.character_motion {
            motion.remaining_duration -= elapsed;
            if motion.remaining_duration <= 0.0 {
                self.character_motion = None;
            }
        }
        for animation in self.character_dying_animations.iter_mut() {
            animation.remaining_duration -= elapsed;
        }
        self.character_dying_animations
            .retain(|a| a.remaining_duration > 0.0);
        for animation in self.character_shake_animations.iter_mut() {
            animation.remaining_duration -= elapsed;
        }
        self.character_shake_animations
            .retain(|a| a.remaining_duration > 0.0);

             */

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

    fn zoom(&mut self, i: isize) {
        self.zoom_index = ((self.zoom_index as isize) + i)
            .max(0)
            .min(ZOOM_LEVELS.len() as isize - 1) as usize;

        let w = screen_width();
        let h = screen_height();

        let camera_center = (
            self.camera_position.0.get() + w / 2.0,
            self.camera_position.1.get() + h / 2.0,
        );

        let new_cell_w = ZOOM_LEVELS[self.zoom_index];
        let factor = new_cell_w / self.cell_w;
        self.cell_w = new_cell_w;
        let new_camera_center = (camera_center.0 * factor, camera_center.1 * factor);
        self.camera_position.0.set(new_camera_center.0 - w / 2.0);
        self.camera_position.1.set(new_camera_center.1 - h / 2.0);
    }

    pub fn add_text_effect(
        &mut self,
        position: Position,
        start_time: f32,
        duration: f32,
        text: impl Into<String>,
        style: TextEffectStyle,
    ) {
        let mut pos = (position.0 as f32, position.1 as f32);

        let mut rng = rand::rng();
        let dx = rng.random_range(-0.3..=0.3);
        let dy = rng.random_range(-0.3..=0.3);
        pos = (pos.0 + dx, pos.1 + dy);

        let color = match style {
            TextEffectStyle::Friendly => GREEN,
            TextEffectStyle::Miss => WHITE,
            TextEffectStyle::ReactionExclamation => ORANGE,
            TextEffectStyle::HostileGraze => WHITE,
            TextEffectStyle::HostileHit => ORANGE,
            TextEffectStyle::HostileCrit => RED,
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

    pub fn add_effect(&mut self, source: Position, destination: Position, effect: Effect) {
        let source_pos = (source.0 as f32, source.1 as f32);
        let destination_pos = (destination.0 as f32, destination.1 as f32);

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

    pub fn update_move_speed(&mut self, active_char_id: CharacterId, sprint_usage: u32) {
        let active_char = self.characters.get(active_char_id);

        let speed = active_char.move_speed();
        let max_range = active_char.remaining_movement.get() + (sprint_usage as f32);

        self.movement_range.set(speed, max_range);
        let pos = self.characters.get(self.active_character_id).pos();
        self.routes = self
            .pathfind_grid
            .explore_outward(pos, self.movement_range.max());
    }

    fn grid_x_to_screen(&self, grid_x: i32) -> f32 {
        let x =
            self.position_on_screen.0 + grid_x as f32 * self.cell_w - self.camera_position.0.get();

        x.round()
    }

    fn grid_x_f32_to_screen(&self, grid_x: f32) -> f32 {
        let x = self.position_on_screen.0 + grid_x * self.cell_w - self.camera_position.0.get();

        x.round()
    }

    fn grid_y_to_screen(&self, grid_y: i32) -> f32 {
        let y =
            self.position_on_screen.1 + grid_y as f32 * self.cell_w - self.camera_position.1.get();
        y.round()
    }

    fn grid_y_f32_to_screen(&self, grid_y: f32) -> f32 {
        let y = self.position_on_screen.1 + grid_y * self.cell_w - self.camera_position.1.get();
        y.round()
    }

    fn grid_pos_to_screen(&self, pos: Position) -> (f32, f32) {
        (self.grid_x_to_screen(pos.0), self.grid_y_to_screen(pos.1))
    }

    fn character_screen_pos(&self, character: &Character) -> (f32, f32) {
        for animation in &self.character_animations {
            if let AnimationKind::Motion { from, to } = animation.kind {
                if animation.character_id == character.id() {
                    let from = self.grid_pos_to_screen(from);
                    let to = self.grid_pos_to_screen(to);
                    let remaining = animation.remaining_duration / animation.duration;
                    //let remaining = (remaining * 4.0).floor() / 4.0;
                    return (
                        to.0 - (to.0 - from.0) * remaining,
                        to.1 - (to.1 - from.1) * remaining,
                    );
                }
            }
        }
        self.grid_pos_to_screen(character.pos())
    }

    fn draw_cell_outline(
        &self,
        (grid_x, grid_y): Position,
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

    fn fill_cell(&self, (grid_x, grid_y): Position, color: Color, margin: f32) {
        draw_rectangle(
            self.grid_x_to_screen(grid_x) + margin,
            self.grid_y_to_screen(grid_y) + margin,
            self.cell_w - margin * 2.0,
            self.cell_w - margin * 2.0,
            color,
        )
    }

    pub fn set_enemys_target(&mut self, target_character_id: CharacterId) {
        self.enemys_target = Some(target_character_id);
    }

    fn draw_background(&self) {
        for col in 0..self.grid_dimensions.0 as i32 + 1 {
            let x0 = self.grid_x_to_screen(col);

            for row in 0..self.grid_dimensions.1 as i32 + 1 {
                let y0 = self.grid_y_to_screen(row);

                if col < self.grid_dimensions.0 as i32 && row < self.grid_dimensions.1 as i32 {
                    let terrain_id = self.background.get(&(col, row)).unwrap();
                    draw_terrain(&self.terrain_atlas, *terrain_id, self.cell_w, x0, y0);

                    /*
                    let dest_size = Vec2::new(self.cell_w, self.cell_w);
                    let params = DrawTextureParams {
                        dest_size: Some(dest_size),
                        ..Default::default()
                    };

                    let i =
                        self.cell_backgrounds[(row * self.grid_dimensions.0 as i32 + col) as usize];
                    let texture = &self.background_textures[i];
                    draw_texture_ex(texture, x0, y0, WHITE, params);
                     */
                }
            }
        }

        for col in 0..self.grid_dimensions.0 as i32 + 1 {
            let x0 = self.grid_x_to_screen(col);

            for row in 0..self.grid_dimensions.1 as i32 + 1 {
                let y0 = self.grid_y_to_screen(row);

                if col < self.grid_dimensions.0 as i32 && row < self.grid_dimensions.1 as i32 {
                    if let Some(terrain_id) = self.terrain_objects.get(&(col, row)) {
                        draw_terrain(&self.terrain_atlas, *terrain_id, self.cell_w, x0, y0);
                    }
                }
            }
        }

        for pos in self.pathfind_grid.blocked_positions().iter() {
            if false {
                self.draw_cell_outline(*pos, MAGENTA, 0.0, 5.0);
            }
        }
    }

    fn draw_character(&self, character: &Character) {
        let mut params = DrawTextureParams {
            dest_size: Some((self.cell_w, self.cell_w).into()),
            ..Default::default()
        };

        let (mut x, mut y) = self.character_screen_pos(character);

        let mut dying = false;

        for animation in self
            .character_animations
            .iter()
            .filter(|a| a.character_id == character.id())
        {
            let remaining = animation.remaining_duration;
            let duration = animation.duration;
            match animation.kind {
                AnimationKind::Motion { from, to } => {
                    let remaining_part = remaining / duration;
                    let quantized = (remaining_part * 4.0).floor() / 4.0;
                    if quantized < 0.25 {
                        y += 1.0;
                    } else if quantized < 0.5 {
                        params.rotation = -0.05;
                    } else if quantized < 0.75 {
                        y += 1.0;
                    } else {
                        params.rotation = 0.05;
                    }
                }
                AnimationKind::Shake { random_time_offset } => {
                    if (((remaining + random_time_offset) / 0.1).floor()) as i32 % 2 == 0 {
                        x -= 3.0;
                    } else {
                        x += 3.0;
                    }
                }
                AnimationKind::Death => {
                    params.rotation = PI * 0.5;
                    dying = true;
                }
                AnimationKind::Act { random_rotation } => {
                    y -= self.cell_w * 0.07;
                    params.rotation = random_rotation;
                }
            }
        }

        if !dying {
            y -= self.cell_w * 0.2;
        }
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

    pub fn draw(
        &mut self,
        receptive_to_dragging: bool,
        ui_state: &mut UiState,
        obstructed: bool,
        hovered_base_action: Option<BaseAction>,
    ) -> GridOutcome {
        let mut outcome = GridOutcome::default();
        // TODO
        let receptive_to_input = !obstructed;

        let previous_inspect_target = self.players_inspect_target;

        let had_non_empty_movement_path = has_non_empty_movement_path(ui_state);

        let (x, y) = self.position_on_screen;

        let mouse_relative_to_grid = |(x, y): (f32, f32)| {
            (
                ((self.camera_position.0.get() + x) / self.cell_w).floor() as i32,
                ((self.camera_position.1.get() + y) / self.cell_w).floor() as i32,
            )
        };
        let (mouse_x, mouse_y) = mouse_position();
        let mouse_relative = (mouse_x - x, mouse_y - y);
        let mouse_grid_pos = mouse_relative_to_grid(mouse_relative);

        let is_mouse_within_grid = self.is_within_grid(mouse_grid_pos);

        if let UiState::ConfiguringAction(action) = ui_state {
            if is_mouse_within_grid
                && receptive_to_input
                && is_mouse_button_pressed(MouseButton::Right)
            {
                match action {
                    ConfiguredAction::Attack {
                        target: t @ Some(..),
                        ..
                    } => *t = None,

                    ConfiguredAction::UseAbility {
                        target: t @ (ActionTarget::Character(..) | ActionTarget::Position(..)),
                        ..
                    } => *t = ActionTarget::None,

                    ConfiguredAction::Move {
                        selected_movement_path: p,
                        ..
                    } if !p.is_empty() => p.clear(),

                    _ => {
                        *ui_state = UiState::ChoosingAction;
                        outcome.switched_state = Some(NewState::ChoosingAction);
                    }
                }
            }
        }

        if is_mouse_within_grid && receptive_to_dragging {
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

            let (_dx, dy) = mouse_wheel();
            if dy < 0.0 {
                self.zoom(-1);
            } else if dy > 0.0 {
                self.zoom(1);
            }
        }

        draw_rectangle(x, y, screen_width(), screen_height(), BACKGROUND_COLOR);

        self.draw_background();

        let active_char_pos = self.characters.get(self.active_character_id).pos();

        let range_indicator = self.determine_range_indicator(ui_state, hovered_base_action);

        self.draw_active_character_highlight();

        if matches!(ui_state, UiState::ChoosingAction) {
            if let Some(BaseAction::Move) = hovered_base_action {
                self.draw_movement_path_background();
            }
        }

        if let UiState::ConfiguringAction(ConfiguredAction::Move {
            selected_movement_path,
            ..
        }) = ui_state
        {
            self.draw_movement_path_background();
        }

        for character in self.characters.iter() {
            for engager in character.is_engaged_by.borrow().values() {
                let mut engager_pos = self.character_screen_pos(engager);
                let mut target_pos = self.character_screen_pos(character);

                let offset_dir =
                    match (
                        target_pos.0.total_cmp(&engager_pos.0),
                        target_pos.1.total_cmp(&engager_pos.1),
                    ) {
                        (Ordering::Less, Ordering::Less)
                        | (Ordering::Greater, Ordering::Greater) => (1, -1),
                        (Ordering::Less, Ordering::Equal)
                        | (Ordering::Greater, Ordering::Equal) => (0, 1),
                        (Ordering::Less, Ordering::Greater)
                        | (Ordering::Greater, Ordering::Less) => (1, 1),
                        (Ordering::Equal, Ordering::Less)
                        | (Ordering::Equal, Ordering::Greater) => (1, 0),
                        (Ordering::Equal, Ordering::Equal) => unreachable!(),
                    };
                let factor = if engager.id() < character.id() {
                    7.0
                } else {
                    -7.0
                };
                let offset = (offset_dir.0 as f32 * factor, offset_dir.1 as f32 * factor);
                engager_pos = (engager_pos.0 + offset.0, engager_pos.1 + offset.1);
                target_pos = (target_pos.0 + offset.0, target_pos.1 + offset.1);

                let is_player_engaging = engager.player_controlled();

                self.draw_engagement_arrow(engager_pos, target_pos, is_player_engaging);
            }
        }

        for character in self.characters.iter() {
            self.draw_character(character);
            self.draw_character_healthbar(character);
        }

        if let Some((range, indicator)) = range_indicator {
            self.draw_range_indicator(active_char_pos, range, indicator);

            for character in self.characters.iter() {
                if within_range_squared(range.squared(), active_char_pos, character.pos()) {
                    self.draw_character_label(character, false);
                }
            }
        }

        if let UiState::ConfiguringAction(ConfiguredAction::Move {
            selected_movement_path,
            ..
        }) = ui_state
        {
            if !selected_movement_path.is_empty() {
                println!("Draw movement selected");
                self.draw_movement_path(selected_movement_path, false);
            }
        }

        let mut hovered_character_id = None;

        if !(matches!(ui_state, UiState::ReactingToAttack { .. })) {
            self.enemys_target = None;
        }

        let pressed_left_mouse = is_mouse_button_pressed(MouseButton::Left);

        let mouse_state = match ui_state {
            UiState::ChoosingAction => MouseState::None,

            UiState::ConfiguringAction(base_action) => match base_action {
                ConfiguredAction::Attack { .. } => MouseState::RequiresEnemyTarget {
                    area_radius: None,
                    move_into_melee: None,
                },

                ConfiguredAction::UseAbility {
                    ability,
                    selected_enhancements,
                    target: _,
                } => match ability.target {
                    AbilityTarget::Enemy {
                        impact_area, reach, ..
                    } => {
                        let mut area_radius = None;
                        if let Some((mut radius, _acquisition, _effect)) = impact_area {
                            for effect in
                                selected_enhancements.iter().filter_map(|e| e.spell_effect)
                            {
                                if effect.increased_radius_tenths > 0 {
                                    radius =
                                        radius.plusf(effect.increased_radius_tenths as f32 * 0.1);
                                }
                            }
                            area_radius = Some(radius);
                        }
                        let mut move_into_melee = None;
                        if let AbilityReach::MoveIntoMelee(mut range) = reach {
                            for effect in
                                selected_enhancements.iter().filter_map(|e| e.spell_effect)
                            {
                                if effect.increased_range_tenths > 0 {
                                    range = range.plusf(effect.increased_range_tenths as f32 * 0.1);
                                }
                            }
                            move_into_melee = Some(range);
                        }

                        MouseState::RequiresEnemyTarget {
                            area_radius,
                            move_into_melee,
                        }
                    }
                    AbilityTarget::Ally { .. } => MouseState::RequiresAllyTarget,
                    AbilityTarget::Area { area_effect, .. } => {
                        MouseState::RequiresPositionTarget(area_effect.radius)
                    }
                    AbilityTarget::None { .. } => MouseState::ImplicitTarget,
                },

                ConfiguredAction::Move { .. } => MouseState::MayInputMovement,

                ConfiguredAction::ChangeEquipment { .. } => MouseState::None,
                ConfiguredAction::UseConsumable { .. } => MouseState::None,
            },
            _ => MouseState::None,
        };

        match mouse_state {
            MouseState::RequiresEnemyTarget {
                area_radius: Some(radius),
                ..
            } => {
                if let ActionTarget::Character(target_id, _movement) =
                    ui_state.players_action_target()
                {
                    // TODO draw movement?
                    self.draw_range_indicator(
                        self.characters.get(target_id).pos(),
                        radius,
                        RangeIndicator::TargetAreaEffect,
                    );
                } else if is_mouse_within_grid && receptive_to_input {
                    self.draw_range_indicator(
                        mouse_grid_pos,
                        radius,
                        RangeIndicator::TargetAreaEffect,
                    );
                }
            }
            MouseState::RequiresPositionTarget(range) => {
                if let ActionTarget::Position(pos) = ui_state.players_action_target() {
                    self.draw_range_indicator(
                        (pos.0, pos.1),
                        range,
                        RangeIndicator::TargetAreaEffect,
                    );
                } else if is_mouse_within_grid && receptive_to_input {
                    self.draw_range_indicator(
                        mouse_grid_pos,
                        range,
                        RangeIndicator::TargetAreaEffect,
                    );
                }
            }
            _ => {}
        }

        match ui_state {
            UiState::ReactingToMovementAttackOpportunity {
                reactor,
                target,
                movement,
                selected,
            } => {
                let target = self.characters.get(*target);
                let reactor = self.characters.get(*reactor);

                let path = [movement.0, movement.1];
                self.draw_movement_path_arrow(path.iter().copied(), RED, 7.0);
                self.draw_cornered_outline(
                    self.character_screen_pos(reactor),
                    ACTIVE_CHARACTER_COLOR,
                    5.0,
                    2.0,
                );

                if *selected {
                    self.draw_target_crosshair(
                        reactor.pos(),
                        target.pos(),
                        PLAYERS_TARGET_CROSSHAIR_COLOR,
                        4.0,
                    );
                }

                self.draw_character_label(target, false);
                self.draw_character_label(reactor, false);
                self.draw_overhead_question_mark(reactor);
            }

            UiState::ReactingToRangedAttackOpportunity {
                reactor,
                attacker,
                victim,
                selected,
            } => {
                let reactor = self.characters.get(*reactor);
                let attacker = self.characters.get(*attacker);
                let victim = self.characters.get(*victim);

                self.draw_cornered_outline(
                    self.character_screen_pos(reactor),
                    ACTIVE_CHARACTER_COLOR,
                    5.0,
                    2.0,
                );

                self.draw_target_crosshair(attacker.pos(), victim.pos(), RED, 4.0);

                if *selected {
                    self.draw_target_crosshair(
                        reactor.pos(),
                        attacker.pos(),
                        PLAYERS_TARGET_CROSSHAIR_COLOR,
                        4.0,
                    );
                }

                self.draw_character_label(attacker, false);
                self.draw_character_label(reactor, false);
                self.draw_overhead_question_mark(reactor);
            }

            UiState::ReactingToAttack {
                attacker, reactor, ..
            } => {
                let attacker = self.characters.get(*attacker);
                let reactor = self.characters.get(*reactor);

                self.draw_cornered_outline(
                    self.character_screen_pos(reactor),
                    ACTIVE_CHARACTER_COLOR,
                    5.0,
                    2.0,
                );
                self.draw_target_crosshair(attacker.pos(), reactor.pos(), RED, 4.0);
                self.draw_character_label(attacker, false);
                self.draw_character_label(reactor, false);
                self.draw_overhead_question_mark(reactor);
            }

            UiState::ReactingToHit { victim, .. } => {
                let reactor = self.characters.get(*victim);
                self.draw_cornered_outline(
                    self.character_screen_pos(reactor),
                    ACTIVE_CHARACTER_COLOR,
                    5.0,
                    2.0,
                );

                self.draw_character_label(reactor, false);
                self.draw_overhead_question_mark(reactor);
            }

            _ => {}
        }

        if !matches!(
            ui_state,
            UiState::ReactingToMovementAttackOpportunity { .. }
        ) {
            let active_char = self.characters.get(self.active_character_id);
            let draw_action_points = !active_char.player_controlled();
            self.draw_character_label(active_char, draw_action_points);
        }

        if is_mouse_within_grid && receptive_to_input {
            for character in self.characters.iter() {
                if character.pos() == mouse_grid_pos {
                    let id = character.id();
                    outcome.hovered_character_id = Some(id);
                    hovered_character_id = Some(id);
                }
            }

            let pressed_terrain = pressed_left_mouse && hovered_character_id.is_none();

            if pressed_terrain {
                match mouse_state {
                    MouseState::RequiresAllyTarget
                    | MouseState::RequiresEnemyTarget { .. }
                    | MouseState::ImplicitTarget => {
                        *ui_state = UiState::ChoosingAction;
                        outcome.switched_state = Some(NewState::ChoosingAction);
                        outcome.switched_players_action_target = true;
                    }
                    _ => {}
                }

                self.players_inspect_target = None;
            }

            if pressed_left_mouse
                && matches!(mouse_state, MouseState::RequiresPositionTarget { .. })
            {
                if ui_state.players_action_target() == ActionTarget::None {
                    ui_state.set_target(ActionTarget::Position(mouse_grid_pos));
                    outcome.switched_players_action_target = true;
                } else {
                    *ui_state = UiState::ChoosingAction;
                    outcome.switched_state = Some(NewState::ChoosingAction);
                }
            }

            let player_has_action_char_target = matches!(
                ui_state.players_action_target(),
                ActionTarget::Character { .. }
            );

            match mouse_state {
                MouseState::RequiresEnemyTarget { .. } | MouseState::RequiresAllyTarget => {
                    if !player_has_action_char_target && hovered_character_id.is_none() {
                        let mut is_mouse_pos_out_of_range = false;
                        if let Some((range, _indicator)) = range_indicator {
                            is_mouse_pos_out_of_range = (((mouse_grid_pos.0 - active_char_pos.0)
                                .pow(2)
                                + (mouse_grid_pos.1 - active_char_pos.1).pow(2))
                                as f32)
                                > range.squared();
                        }

                        if is_mouse_pos_out_of_range {
                            self.draw_invalid_target_marker(mouse_grid_pos);
                        } else {
                            self.draw_cornered_outline(
                                self.grid_pos_to_screen(mouse_grid_pos),
                                HOVER_TERRAIN_NEED_CHAR_TARGET_COLOR,
                                5.0,
                                2.0,
                            );
                        }
                    }
                }
                MouseState::RequiresPositionTarget { .. } => {
                    if !matches!(
                        ui_state.players_action_target(),
                        ActionTarget::Position { .. }
                    ) {
                        let mut is_mouse_pos_out_of_range = false;
                        if let Some((range, _indicator)) = range_indicator {
                            is_mouse_pos_out_of_range = (((mouse_grid_pos.0 - active_char_pos.0)
                                .pow(2)
                                + (mouse_grid_pos.1 - active_char_pos.1).pow(2))
                                as f32)
                                > range.squared();
                        }

                        if is_mouse_pos_out_of_range {
                            self.draw_invalid_target_marker(mouse_grid_pos);
                        } else {
                            self.draw_cornered_outline(
                                self.grid_pos_to_screen(mouse_grid_pos),
                                HOVER_TERRAIN_NEED_CHAR_TARGET_COLOR,
                                5.0,
                                2.0,
                            );
                            self.draw_target_crosshair(
                                self.characters.get(self.active_character_id).pos(),
                                mouse_grid_pos,
                                HOVER_PLAYERS_TARGET_CROSSHAIR_COLOR,
                                4.0,
                            );
                        }
                    }
                }
                _ => {}
            }

            let hovered_move_route = if matches!(mouse_state, MouseState::MayInputMovement)
                && hovered_character_id.is_none()
            {
                self.routes.get(&mouse_grid_pos).copied()
            } else {
                None
            };

            if let Some(hovered_route) = &hovered_move_route {
                if self.dragging_camera_from.is_none() && !player_has_action_char_target {
                    let path = build_path_from_chart(&self.routes, active_char_pos, mouse_grid_pos);
                    self.draw_movement_path(&path.positions, true);

                    if pressed_left_mouse {
                        let char_remaining_movement = self
                            .characters
                            .get(self.active_character_id)
                            .remaining_movement
                            .get();
                        let cost = self
                            .movement_range
                            .cost(hovered_route.distance_from_start, char_remaining_movement);

                        let commit_movement = matches!(
                            ui_state,
                            UiState::ConfiguringAction(ConfiguredAction::Move { .. })
                        );
                        *ui_state = UiState::ConfiguringAction(ConfiguredAction::Move {
                            cost,
                            selected_movement_path: path.positions,
                        });
                        outcome.switched_state = Some(NewState::Move { commit_movement });
                    }
                }
            } else if let Some(hovered_id) = hovered_character_id {
                let hovers_player = self.characters.get(hovered_id).player_controlled();

                if hovers_player {
                    if matches!(mouse_state, MouseState::RequiresAllyTarget) {
                        self.draw_cornered_outline(
                            self.grid_pos_to_screen(mouse_grid_pos),
                            HOVER_ALLY_COLOR,
                            5.0,
                            4.0,
                        );
                        self.draw_target_crosshair(
                            self.characters.get(self.active_character_id).pos(),
                            mouse_grid_pos,
                            HOVER_PLAYERS_TARGET_CROSSHAIR_COLOR,
                            4.0,
                        );
                    } else if matches!(mouse_state, MouseState::RequiresEnemyTarget { .. }) {
                        self.draw_invalid_target_marker(mouse_grid_pos);
                    }

                    if pressed_left_mouse {
                        if self.active_character_id == hovered_id {
                            if matches!(mouse_state, MouseState::RequiresPositionTarget(..)) {
                                ui_state.set_target(ActionTarget::Position(mouse_grid_pos));
                                outcome.switched_players_action_target = true;
                                self.players_inspect_target = Some(hovered_id);
                            } else if mouse_state == MouseState::RequiresAllyTarget {
                                ui_state.set_target(ActionTarget::Character(hovered_id, None));
                                outcome.switched_players_action_target = true;
                                self.players_inspect_target = Some(hovered_id);
                            } else {
                                // Click self => abort the action
                                *ui_state = UiState::ChoosingAction;
                                outcome.switched_state = Some(NewState::ChoosingAction);
                                outcome.switched_players_action_target = true;
                            }
                        } else {
                            if mouse_state == MouseState::RequiresAllyTarget {
                                ui_state.set_target(ActionTarget::Character(hovered_id, None));
                                outcome.switched_players_action_target = true;
                            }
                            self.players_inspect_target = Some(hovered_id);
                        }
                    }
                } else {
                    if let MouseState::RequiresEnemyTarget {
                        move_into_melee, ..
                    } = mouse_state
                    {
                        if let Some(move_range) = move_into_melee {
                            let positions = if within_range_squared(
                                move_range.squared(),
                                active_char_pos,
                                mouse_grid_pos,
                            ) {
                                self.try_find_path_to_action_target(
                                    mouse_grid_pos,
                                    active_char_pos,
                                    move_range,
                                )
                            } else {
                                vec![]
                            };

                            self.draw_movement_to_target(
                                active_char_pos,
                                mouse_grid_pos,
                                positions,
                            );
                        } else {
                            self.draw_cornered_outline(
                                self.grid_pos_to_screen(mouse_grid_pos),
                                HOVER_ENEMY_COLOR,
                                5.0,
                                3.0,
                            );
                            self.draw_target_crosshair(
                                active_char_pos,
                                mouse_grid_pos,
                                HOVER_PLAYERS_TARGET_CROSSHAIR_COLOR,
                                4.0,
                            );
                        }
                    } else if matches!(mouse_state, MouseState::RequiresAllyTarget) {
                        self.draw_invalid_target_marker(mouse_grid_pos);
                    }

                    if pressed_left_mouse {
                        let mut may_acquire_attack_target = matches!(
                            ui_state,
                            UiState::ChoosingAction
                                | UiState::ConfiguringAction(
                                    ConfiguredAction::Move { .. } | ConfiguredAction::Attack { .. }
                                )
                        );

                        if player_has_action_char_target
                            && matches!(mouse_state, MouseState::RequiresAllyTarget)
                        {
                            may_acquire_attack_target = true; // i.e. change action to attack
                        }

                        if self
                            .characters
                            .get(self.active_character_id)
                            .weapon(HandType::MainHand)
                            .is_none()
                        {
                            may_acquire_attack_target = false;
                        }

                        if may_acquire_attack_target {
                            if let UiState::ConfiguringAction(ConfiguredAction::Attack {
                                ref mut target,
                                ..
                            }) = ui_state
                            {
                                *target = Some(hovered_id);
                            } else {
                                outcome.switched_state = Some(NewState::Attack);

                                let hand = HandType::MainHand;
                                let action_point_cost = self
                                    .characters
                                    .get(self.active_character_id)
                                    .attack_action_point_cost(hand);
                                *ui_state = UiState::ConfiguringAction(ConfiguredAction::Attack {
                                    attack: AttackAction {
                                        hand,
                                        action_point_cost,
                                    },
                                    selected_enhancements: vec![],
                                    target: Some(hovered_id),
                                });
                            }

                            outcome.switched_players_action_target = true;
                            self.players_inspect_target = Some(hovered_id);
                        } else if let MouseState::RequiresEnemyTarget {
                            move_into_melee, ..
                        } = mouse_state
                        {
                            let movement = move_into_melee.map(|move_range| {
                                let mut path = self.try_find_path_to_action_target(
                                    mouse_grid_pos,
                                    active_char_pos,
                                    move_range,
                                );
                                if path.len() == 1 {
                                    // A path consisting of just the start position is not a valid movement to the target
                                    path = vec![];
                                }
                                path
                            });

                            ui_state.set_target(ActionTarget::Character(hovered_id, movement));
                            outcome.switched_players_action_target = true;

                            self.players_inspect_target = Some(hovered_id);
                        } else if !matches!(mouse_state, MouseState::RequiresAllyTarget) {
                            self.players_inspect_target = Some(hovered_id);
                        }
                    }
                }
            } else if had_non_empty_movement_path && self.dragging_camera_from.is_none() {
                self.fill_cell(mouse_grid_pos, Color::new(1.0, 0.0, 0.0, 0.2), 4.0);
                //self.draw_invalid_target_marker(mouse_grid_pos);
                //self.draw_cell_outline(mouse_grid_pos, HOVER_INVALID_MOVEMENT_COLOR, 0.0, 1.0);
            }
        }

        if let Some(id) = self.selected_player_character_id {
            let pos = self.character_screen_pos(self.characters.get(id));
            self.draw_cornered_outline(pos, SELECTED_CHARACTER_COLOR, -1.0, 2.0);
        }

        if let Some(target) = self.players_inspect_target {
            self.draw_cornered_outline(
                self.character_screen_pos(self.characters.get(target)),
                INSPECTING_TARGET_COLOR,
                4.0,
                2.0,
            );
        }

        match ui_state.players_action_target() {
            ActionTarget::Character(target, movement) => {
                let target = self.characters.get(target);
                let target_pos = target.pos();
                if let Some(positions) = movement {
                    self.draw_movement_to_target(active_char_pos, target_pos, positions);
                } else {
                    self.draw_target_crosshair(
                        active_char_pos,
                        target_pos,
                        PLAYERS_TARGET_CROSSHAIR_COLOR,
                        7.0,
                    );
                }

                self.draw_character_label(target, false);
            }
            ActionTarget::Position(target_pos) => {
                self.draw_target_crosshair(
                    active_char_pos,
                    target_pos,
                    PLAYERS_TARGET_CROSSHAIR_COLOR,
                    7.0,
                );
            }
            ActionTarget::None => {}
        }

        if let Some(target) = self.enemys_target {
            let target_pos = self.characters.get(target).pos();
            self.draw_target_crosshair(
                self.characters.get(self.active_character_id).pos(),
                target_pos,
                ENEMYS_TARGET_CROSSHAIR_COLOR,
                7.0,
            );
        }

        if let Some(id) = hovered_character_id {
            if id != self.active_character_id {
                let char = self.characters.get(id);
                self.draw_character_label(char, false);
            }
        }

        self.draw_effects();

        if self.players_inspect_target != previous_inspect_target {
            outcome.switched_inspect_target = Some(self.players_inspect_target);
        }

        if has_non_empty_movement_path(ui_state) != had_non_empty_movement_path {
            outcome.switched_movement_path = true;
        }

        outcome
    }

    fn draw_movement_to_target(
        &self,
        actor_pos: (i32, i32),
        target_pos: (i32, i32),
        movement_to_target: Vec<(i32, i32)>,
    ) {
        if movement_to_target.len() < 2 {
            let invalid_path = [actor_pos, target_pos];
            self.draw_movement_path_arrow(invalid_path.iter().copied(), RED, 7.0);
        } else {
            self.draw_target_crosshair(
                *movement_to_target.last().unwrap(),
                target_pos,
                PLAYERS_TARGET_CROSSHAIR_COLOR,
                7.0,
            );
            self.draw_movement_path_arrow(
                movement_to_target.iter().copied(),
                MOVEMENT_ARROW_COLOR,
                7.0,
            );
        }
    }

    fn try_find_path_to_action_target(
        &mut self,
        target_pos: (i32, i32),
        actor_pos: (i32, i32),
        move_range: Range,
    ) -> Vec<Position> {
        let maybe_path = self.pathfind_grid.find_shortest_path_to_adjacent(
            actor_pos,
            target_pos,
            f32::from(move_range) - 1.0,
        );

        if let Some(path) = maybe_path {
            //path.positions.iter().rev().map(|(_dist, pos)| *pos).collect()
            path.positions.iter().map(|(_dist, pos)| *pos).collect()
        } else {
            vec![]
        }

        /*
        let mut movement = vec![];
        for (dx, dy) in [
            (-1, 0),
            (0, -1),
            (1, 0),
            (0, 1),
            (-1, -1),
            (1, -1),
            (1, 1),
            (-1, 1),
        ] {
            let x = actor_pos.0 + dx;
            let y = actor_pos.1 + dy;
            let blocked = self.pathfind_grid.blocked_positions().contains(&(x, y));

            if !blocked && (x - target_pos.0).abs() <= 1 && (y - target_pos.1).abs() <= 1 {
                movement = vec![(x, y)];
                break;
            }
        }
        movement
         */
    }

    fn determine_range_indicator(
        &self,
        ui_state: &mut UiState,
        hovered_base_action: Option<BaseAction>,
    ) -> Option<(Range, RangeIndicator)> {
        if let UiState::ConfiguringAction(configured_action) = ui_state {
            match configured_action {
                ConfiguredAction::Attack {
                    attack,
                    target,
                    selected_enhancements,
                    ..
                } => match target {
                    Some(target) => {
                        let (range, reach) = self
                            .characters
                            .get(self.active_character_id)
                            .attack_reaches(
                                attack.hand,
                                self.characters.get(*target).position.get(),
                                selected_enhancements.iter().map(|e| e.effect),
                            );

                        let maybe_indicator = match reach {
                            ActionReach::Yes | ActionReach::YesButDisadvantage(..) => {
                                if let ActionReach::YesButDisadvantage(..) = reach {
                                    Some(RangeIndicator::CanReachButDisadvantage)
                                } else {
                                    None
                                }
                            }
                            ActionReach::No => Some(RangeIndicator::CannotReach),
                        };

                        maybe_indicator.map(|indicator| (range, indicator))
                    }

                    None => {
                        let range = self.characters.get(self.active_character_id).attack_range(
                            attack.hand,
                            selected_enhancements.iter().map(|e| e.effect),
                        );
                        Some((range, RangeIndicator::ActionTargetRange))
                    }
                },
                ConfiguredAction::UseAbility {
                    ability,
                    selected_enhancements,
                    target,
                } => match target {
                    ActionTarget::Character(target_char_id, movement) => {
                        let maybe_indicator = if self
                            .characters
                            .get(self.active_character_id)
                            .reaches_with_ability(
                                *ability,
                                selected_enhancements,
                                self.characters.get(*target_char_id).position.get(),
                            ) {
                            None
                        } else {
                            Some(RangeIndicator::CannotReach)
                        };
                        maybe_indicator.map(|indicator| {
                            (
                                ability.target.range(selected_enhancements).unwrap(),
                                indicator,
                            )
                        })
                    }
                    ActionTarget::Position(target_pos) => {
                        let maybe_indicator = if self
                            .characters
                            .get(self.active_character_id)
                            .reaches_with_ability(*ability, selected_enhancements, *target_pos)
                        {
                            None
                        } else {
                            Some(RangeIndicator::CannotReach)
                        };
                        maybe_indicator.map(|indicator| {
                            (
                                ability.target.range(selected_enhancements).unwrap(),
                                indicator,
                            )
                        })
                    }
                    ActionTarget::None => {
                        let radius = ability.target.radius(selected_enhancements);
                        let range = ability.target.range(selected_enhancements);
                        radius
                            .or(range)
                            .map(|range| (range, RangeIndicator::ActionTargetRange))
                    }
                },
                _ => None,
            }
        } else if let UiState::ChoosingAction = ui_state {
            match hovered_base_action {
                Some(BaseAction::Attack(attack)) => {
                    let range = self
                        .characters
                        .get(self.active_character_id)
                        .attack_range(attack.hand, iter::empty());
                    Some((range, RangeIndicator::ActionTargetRange))
                }
                Some(BaseAction::UseAbility(ability)) => {
                    let radius = ability.target.radius(&[]);
                    let range = ability.target.range(&[]);
                    radius
                        .or(range)
                        .map(|range| (range, RangeIndicator::ActionTargetRange))
                }
                _ => None,
            }
        } else {
            None
        }
    }

    fn draw_invalid_target_marker(&self, grid_pos: Position) {
        self.fill_cell(grid_pos, Color::new(1.0, 0.0, 0.0, 0.3), 4.0);
        let (x, y) = self.grid_pos_to_screen(grid_pos);
        draw_cross(x, y, self.cell_w, self.cell_w, RED, 2.0, self.cell_w * 0.15);
    }

    fn draw_character_label(&self, character: &Character, draw_action_points: bool) {
        self._draw_character_label(character, draw_action_points, true, false);
    }

    fn draw_character_healthbar(&self, character: &Character) {
        self._draw_character_label(character, false, false, true);
    }

    fn _draw_character_label(
        &self,
        character: &Character,
        draw_action_points: bool,
        draw_name: bool,
        discrete_healthbar: bool,
    ) {
        let (x, y) = self.character_screen_pos(character);
        let sprite_h = character_sprite_height(character.sprite);
        let texture_h = 32.0;

        let y = y - self.cell_w * 0.3;
        let y = y + (texture_h - sprite_h as f32) / texture_h * self.cell_w;

        let margin = 2.0;
        let health_w = (self.cell_w - 10.0).min(90.0);
        let mut health_h = 10.0;
        let health_x = x + (self.cell_w - health_w) * 0.5;
        let mut health_y = y - health_h;

        if discrete_healthbar {
            health_h -= 6.0;
            health_y += 6.0;
        }

        let font_size = 14;
        let header = character.name;
        let text_dimensions = measure_text(header, Some(&self.big_font), font_size, 1.0);
        let text_pad = 2.0;
        let box_w = text_dimensions.width + text_pad * 2.0;
        let box_h = text_dimensions.height + text_pad * 2.0;

        let box_x = x - (box_w - self.cell_w) / 2.0;
        let box_y = y - health_h - margin - box_h;

        if draw_name {
            draw_rectangle(box_x, box_y, box_w, box_h, Color::new(0.0, 0.0, 0.0, 0.5));
            draw_text_rounded(
                header,
                box_x + text_pad,
                box_y + text_pad + text_dimensions.offset_y,
                TextParams {
                    font: Some(&self.big_font),
                    font_size,
                    color: WHITE,
                    ..Default::default()
                },
            );
        }

        let mut healthbar_bg = BLACK;
        healthbar_bg.a = 0.5;

        draw_rectangle(health_x, health_y, health_w, health_h, healthbar_bg);
        let filled_health_w =
            (health_w) * (character.health.current() as f32 / character.health.max() as f32);
        draw_rectangle(health_x, health_y, filled_health_w, health_h, RED);
        if !discrete_healthbar {
            draw_rectangle_lines(health_x, health_y, health_w, health_h, 1.0, LIGHTGRAY);

            //let health_text = format!("{}/{}", character.health.current(), character.health.max());
            let health_text = format!("{}", character.health.current());
            let health_text_font_size = 16;
            let health_text_font = Some(&self.simple_font);
            let text_dim = measure_text(&health_text, health_text_font, health_text_font_size, 1.0);
            draw_text_rounded(
                &health_text,
                health_x + health_w / 2.0 - text_dim.width / 2.0,
                health_y + health_h / 2.0 - text_dim.height / 2.0 + text_dim.offset_y,
                TextParams {
                    font: health_text_font,
                    font_size: health_text_font_size,
                    color: WHITE,
                    ..Default::default()
                },
            );
        }

        if draw_action_points {
            let mut action_points_row = ActionPointsRow::new(
                (10.0, 10.0),
                0.2,
                Style {
                    background_color: Some(Color::new(0.0, 0.0, 0.0, 0.7)),
                    ..Default::default()
                },
            );
            action_points_row.current_ap = character.action_points.current();
            action_points_row.draw(
                x - (action_points_row.size().0 - self.cell_w) / 2.0,
                box_y - action_points_row.size().1 - 3.0,
            );
        }
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
        let len = self.cell_w * 0.25;
        draw_cornered_rectangle_lines(
            screen_pos.0,
            screen_pos.1,
            self.cell_w,
            self.cell_w,
            thickness,
            color,
            margin,
            len,
        );
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

                    let x = self.grid_x_f32_to_screen(x);
                    let y = self.grid_y_f32_to_screen(y);

                    graphics.draw(x, y, effect, self.cell_w);
                }
                EffectVariant::Line {
                    thickness,
                    end_thickness,
                    color,
                    extend_gradually,
                } => {
                    let from = (
                        self.grid_x_f32_to_screen(effect.source_pos.0 + 0.5),
                        self.grid_y_f32_to_screen(effect.source_pos.1 + 0.5),
                    );
                    let mut to = (
                        self.grid_x_f32_to_screen(effect.destination_pos.0 + 0.5),
                        self.grid_y_f32_to_screen(effect.destination_pos.1 + 0.5),
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

    fn draw_overhead_question_mark(&self, reactor: &Character) {
        let text = "?";
        let font_size = 20;
        let text_dim = measure_text(text, Some(&self.big_font), font_size, 1.0);
        let (x, y) = self.character_screen_pos(reactor);
        let x0 = x + self.cell_w / 2.0 - text_dim.width / 2.0;
        let y0 = y - 25.0;
        draw_text_rounded(
            text,
            x0,
            y0,
            TextParams {
                font: Some(&self.big_font),
                font_size,
                color: YELLOW,
                ..Default::default()
            },
        );
    }

    fn draw_engagement_arrow(
        &self,
        engager: (f32, f32),
        target: (f32, f32),
        is_player_engaging: bool,
    ) {
        let color = if is_player_engaging {
            Color::new(0.1, 0.6, 0.1, 0.4)
        } else {
            Color::new(0.9, 0.1, 0.2, 0.4)
        };
        //let depth = None;
        let depth = Some((Color::new(0.0, 0.0, 0.0, 0.5), 2.0));
        draw_dashed_line(
            (engager.0 + self.cell_w / 2.0, engager.1 + self.cell_w / 2.0),
            (target.0 + self.cell_w / 2.0, target.1 + self.cell_w / 2.0),
            7.0,
            color,
            5.0,
            depth,
        );

        let (mut dx, mut dy) = (0, 0);
        if target.0 < engager.0 {
            dx = -1;
        } else if target.0 > engager.0 {
            dx = 1;
        }
        if target.1 < engager.1 {
            dy = -1;
        } else if target.1 > engager.1 {
            dy = 1;
        }

        //draw_arrow(target, self.cell_w, (dx, dy), color, 3.0);
    }

    fn draw_target_crosshair(
        &self,
        actor_pos: Position,
        target_pos: Position,
        crosshair_color: Color,
        thickness: f32,
    ) {
        let actor_x = self.grid_x_to_screen(actor_pos.0);
        let actor_y = self.grid_y_to_screen(actor_pos.1);
        let target_x = self.grid_x_to_screen(target_pos.0);
        let target_y = self.grid_y_to_screen(target_pos.1);
        let depth = 2.0;

        draw_dashed_line(
            (actor_x + self.cell_w / 2.0, actor_y + self.cell_w / 2.0),
            (target_x + self.cell_w / 2.0, target_y + self.cell_w / 2.0),
            thickness,
            crosshair_color,
            5.0,
            Some((Color::new(0.0, 0.0, 0.0, 0.5), depth)),
        );

        draw_crosshair((target_x + depth, target_y + depth), self.cell_w, BLACK);
        draw_crosshair((target_x, target_y), self.cell_w, crosshair_color);
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
        draw_text_rounded(
            header,
            x + pad,
            y - h + pad + header_dimensions.offset_y,
            params.clone(),
        );
    }

    fn draw_movement_path_background(&self) {
        let active_char_pos = self.characters.get(self.active_character_id).pos();

        for (pos, route) in &self.routes {
            if self.is_within_grid(*pos) && *pos != active_char_pos {
                let color = MOVEMENT_PREVIEW_GRID_COLOR;
                self.fill_cell(*pos, color, self.cell_w / 20.0);
            }
        }
    }

    fn is_within_grid(&self, pos: Position) -> bool {
        (0..self.grid_dimensions.0 as i32).contains(&pos.0)
            && (0..self.grid_dimensions.1 as i32).contains(&pos.1)
    }

    fn draw_movement_path(&self, path: &[(f32, Position)], hover: bool) {
        if hover {
            self.draw_movement_path_arrow(
                path.iter().map(|(_dist, pos)| *pos),
                HOVER_MOVEMENT_ARROW_COLOR,
                3.0,
            );
        } else {
            self.draw_movement_path_arrow(
                path.iter().map(|(_dist, pos)| *pos),
                MOVEMENT_ARROW_COLOR,
                7.0,
            );
        };

        let distance = path.last().unwrap().0;
        let destination = path.last().unwrap().1;
        let (x, y) = (
            self.grid_x_to_screen(destination.0),
            self.grid_y_to_screen(destination.1),
        );

        let text_color = if hover { LIGHTGRAY } else { WHITE };
        let bg_color = if hover {
            Color::new(0.0, 0.0, 0.0, 0.5)
        } else {
            Color::new(0.0, 0.0, 0.0, 0.7)
        };

        let char_remaining_movement = self
            .characters
            .get(self.active_character_id)
            .remaining_movement
            .get();
        let ap = self.movement_range.cost(distance, char_remaining_movement);
        if ap > 0 {
            let text = format!("-{}", ap);
            self.draw_static_text(&text, text_color, bg_color, 4.0, x, y + 14.0);
        }
    }

    fn draw_movement_path_arrow(
        &self,
        mut path: impl ExactSizeIterator<Item = Position>,
        color: Color,
        thickness: f32,
    ) {
        let mut a = path.next().expect("First cell in path");
        let mut b = path.next().expect("Second cell in path");

        loop {
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
                Some((Color::new(0.0, 0.0, 0.0, 0.5), 2.0)),
            );

            if let Some(next) = path.next() {
                a = b;
                b = next;
            } else {
                break;
            }
        }

        let last_direction = (b.0 - a.0, b.1 - a.1);
        assert!(last_direction != (0, 0));

        let end = b;
        draw_arrow(
            (
                self.grid_x_to_screen(end.0) + 2.0,
                self.grid_y_to_screen(end.1) + 2.0,
            ),
            self.cell_w,
            last_direction,
            Color::new(0.0, 0.0, 0.0, 0.5),
            4.0,
        );
        draw_arrow(
            (self.grid_x_to_screen(end.0), self.grid_y_to_screen(end.1)),
            self.cell_w,
            last_direction,
            color,
            4.0,
        );
    }

    /*
    fn draw_move_range_indicator(&self, origin: Position, range: f32) {
        let range_floor = range.floor() as i32;

        let within = |x: i32, y: i32| {
            self.routes
                .get(&(x, y))
                .map(|route| route.distance_from_start <= range)
                .unwrap_or(false)
        };

        for x in (origin.0 - range_floor).max(0)
            ..=(origin.0 + range_floor).min(self.grid_dimensions.0 as i32 - 1)
        {
            for y in (origin.1 - range_floor).max(0)
                ..=(origin.1 + range_floor).min(self.grid_dimensions.1 as i32 - 1)
            {
                let thickness = 2.0;

                if within(x, y) {
                    self.fill_cell((x, y), MOVEMENT_PREVIEW_GRID_COLOR, self.cell_w / 20.0);
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
     */

    fn draw_range_indicator(&self, origin: Position, range: Range, indicator: RangeIndicator) {
        let range_ceil = (f32::from(range)).ceil() as i32;
        let range_squared = range.squared() as i32;
        let draw_background = matches!(indicator, RangeIndicator::ActionTargetRange);
        let color = match indicator {
            RangeIndicator::ActionTargetRange => LIGHTGRAY,
            RangeIndicator::TargetAreaEffect => ORANGE,
            RangeIndicator::CanReachButDisadvantage => RANGE_INDICATOR_SEMI_BAD_COLOR,
            RangeIndicator::CannotReach => RANGE_INDICATOR_BAD_COLOR,
        };
        let is_cell_within =
            |x: i32, y: i32| (x - origin.0).pow(2) + (y - origin.1).pow(2) <= range_squared;
        for x in (origin.0 - range_ceil).max(0)
            ..=(origin.0 + range_ceil).min(self.grid_dimensions.0 as i32 - 1)
        {
            for y in (origin.1 - range_ceil).max(0)
                ..=(origin.1 + range_ceil).min(self.grid_dimensions.1 as i32 - 1)
            {
                if is_cell_within(x, y) {
                    let mut thickness = 2.0;
                    if draw_background {
                        self.fill_cell((x, y), ACTION_RANGE_INDICATOR_BACKGROUND, 0.0);
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

        let (x, y) = self.grid_pos_to_screen((x, y));
        let (w, h) = (self.cell_w, self.cell_w);

        draw_dashed_rectangle_sides(
            x,
            y,
            w,
            h,
            thickness,
            color,
            segment_len,
            left,
            right,
            top,
            bottom,
        );
    }

    fn pan_camera(&self, dx: f32, dy: f32) {
        let new_x = self.camera_position.0.get() + dx;
        let new_y = self.camera_position.1.get() + dy;
        let max_space = 450.0;
        let max_x = self.grid_dimensions.0 as f32 * self.cell_w + max_space - screen_width();
        let max_y = self.grid_dimensions.1 as f32 * self.cell_w + max_space - screen_height();
        self.camera_position
            .0
            .set(new_x.max(-max_space).min(max_x).round());
        self.camera_position
            .1
            .set(new_y.max(-max_space).min(max_y).round());
    }
}

#[derive(Debug, Default)]
pub struct GridOutcome {
    pub switched_state: Option<NewState>,
    pub hovered_character_id: Option<CharacterId>,
    pub switched_inspect_target: Option<Option<CharacterId>>,
    pub switched_players_action_target: bool,
    pub switched_movement_path: bool,
}

#[derive(Debug)]
pub enum NewState {
    Move { commit_movement: bool },
    Attack,
    ChoosingAction,
}

#[derive(Debug)]
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

#[derive(Debug)]
pub enum EffectVariant {
    At(EffectPosition, EffectGraphics),
    Line {
        color: Color,
        thickness: f32,
        end_thickness: Option<f32>,
        extend_gradually: bool,
    },
}
#[derive(Debug)]
pub enum EffectPosition {
    Source,
    Destination,
    Projectile,
}

#[derive(Debug)]
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
                let text_dimensions = measure_text(text, Some(font), font_size, 1.0);

                let x0 = x + cell_w / 2.0 - text_dimensions.width / 2.0;
                let y0 = y - cell_w * 0.3 * t;

                let remaining = effect.end_time - effect.age;
                let fade_duration = 0.4;
                let alpha = if remaining < fade_duration {
                    remaining / fade_duration
                } else {
                    1.0
                };

                let mut text_params = TextParams {
                    font: Some(font),
                    font_size,
                    color: Color::new(0.0, 0.0, 0.0, alpha),
                    ..Default::default()
                };
                // First draw shadow
                draw_text_ex(text, x0 + 2.0, y0 + 2.0, text_params.clone());
                text_params.color = *color;
                text_params.color.a = alpha;

                // Then the regular text
                draw_text_ex(text, x0, y0, text_params);
            }
        }
    }
}

#[derive(PartialEq, Debug)]
enum MouseState {
    RequiresEnemyTarget {
        area_radius: Option<Range>,
        move_into_melee: Option<Range>,
    },
    RequiresAllyTarget,
    RequiresPositionTarget(Range),
    ImplicitTarget,
    MayInputMovement,
    None,
}

fn has_non_empty_movement_path(ui_state: &UiState) -> bool {
    match ui_state {
        UiState::ConfiguringAction(ConfiguredAction::Move {
            selected_movement_path,
            ..
        }) => !selected_movement_path.is_empty(),
        _ => false,
    }
}
