use std::{collections::HashMap, usize};

use macroquad::{
    color::{Color, BLACK, ORANGE},
    math::Vec2,
    shapes::{draw_rectangle_ex, draw_rectangle_lines_ex, DrawRectangleParams},
    text::{draw_text_ex, Font, TextParams},
};

use std::cell::Cell;

use macroquad::input::is_key_down;
use macroquad::math::Rect;

use macroquad::miniquad::KeyCode;
use macroquad::texture::{draw_texture_ex, DrawTextureParams, Texture2D};
use macroquad::{
    color::{GRAY, GREEN, RED, WHITE, YELLOW},
    input::{is_mouse_button_down, is_mouse_button_pressed, mouse_position, MouseButton},
    shapes::{draw_circle, draw_circle_lines, draw_line, draw_rectangle, draw_rectangle_lines},
    text::measure_text,
};

use crate::{
    core::{AttackReach, Goodness, MovementEnhancement},
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
const HOVER_MOVEMENT_ARROW_COLOR: Color = Color::new(0.7, 0.6, 0.6, 1.0);
const HOVER_VALID_MOVEMENT_COLOR: Color = YELLOW;
const HOVER_INVALID_MOVEMENT_COLOR: Color = RED;
const HOVER_NPC_COLOR: Color = Color::new(0.8, 0.2, 0.8, 1.0);
const TARGET_NPC_COLOR: Color = Color::new(1.0, 0.0, 1.0, 1.0);
const ACTIVE_CHARACTER_COLOR: Color = Color::new(1.0, 0.8, 0.0, 0.4);
const SELECTED_CHARACTER_COLOR: Color = WHITE;
const MOVE_RANGE_COLOR: Color = GREEN;
const ACTION_WITHIN_RANGE_COLOR: Color = GREEN;
const ACTION_WITHIN_RANGE_BUT_DISADV_COLOR: Color = ORANGE;
const ACTION_OUT_OF_RANGE_COLOR: Color = RED;
const TARGET_CROSSHAIR_COLOR: Color = WHITE;

#[derive(Debug, Copy, Clone)]
struct CharacterMotion {
    character_id: CharacterId,
    from: (i32, i32),
    to: (i32, i32),
    remaining_duration: f32,
    duration: f32,
}

#[derive(Copy, Clone, Debug)]
enum Target {
    Some(CharacterId),
    Memorized(CharacterId),
    None,
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

pub struct StaticText {
    position: (i32, i32),
    header: String,
    details: Vec<(String, Goodness)>,
}

pub struct GameGrid {
    sprites: HashMap<SpriteId, Texture2D>,
    pathfind_grid: PathfindGrid,
    characters: Characters,
    camera_position: (Cell<f32>, Cell<f32>),
    dragging_camera_from: Option<(f32, f32)>,

    effects: Vec<ConcreteEffect>,
    static_text: Option<StaticText>,

    selected_character_id: CharacterId,
    active_character_id: CharacterId,
    pub action_range_indicator: Option<(Range, AttackReach)>,

    movement_range: MovementRange,
    movement_preview: Option<Vec<(f32, (i32, i32))>>,
    target: Target,

    pub grid_dimensions: (i32, i32),
    pub position_on_screen: (f32, f32),

    character_motion: Option<CharacterMotion>,

    big_font: Font,
    simple_font: Font,

    cell_w: f32,
    size: (f32, f32),
    background_textures: Vec<Texture2D>,
    cell_backgrounds: Vec<usize>,
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
            static_text: None,
            selected_character_id,
            active_character_id: 0,
            movement_range: MovementRange::default(),
            movement_preview: Default::default(),
            target: Target::None,
            action_range_indicator: None,
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

    pub fn set_static_text(
        &mut self,
        position: (i32, i32),
        header: String,
        details: Vec<(String, Goodness)>,
    ) {
        self.static_text = Some(StaticText {
            position,
            header,
            details,
        });
    }

    pub fn clear_static_text(&mut self) {
        self.static_text = None;
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
        selected_character_id: CharacterId,
        characters: &Characters,
        elapsed: f32,
    ) {
        self.pathfind_grid.blocked_positions.clear();

        self.active_character_id = active_character_id;
        self.selected_character_id = selected_character_id;

        for character in characters.iter() {
            let character = character.borrow();
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
    ) {
        let pos = (
            self.grid_x_to_screen(position.0),
            self.grid_y_to_screen(position.1),
        );

        let effect = ConcreteEffect {
            age: 0.0,
            start_time,
            end_time: start_time + duration,
            variant: EffectVariant::At(
                EffectPosition::Source,
                EffectGraphics::Text(text.into(), self.big_font.clone()),
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

    pub fn ensure_has_some_movement_preview(&mut self) {
        if self.movement_preview.is_none() {
            self.set_an_arbitrary_valid_movement_preview();
        }
    }

    fn set_an_arbitrary_valid_movement_preview(&mut self) {
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
        self.movement_preview = Some(movement_preview);
    }

    pub fn remove_movement_preview(&mut self) {
        self.movement_preview = None;
    }

    pub fn has_non_empty_movement_preview(&self) -> bool {
        self.movement_preview
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
        if let Some(movement_preview) = &mut self.movement_preview {
            while !movement_preview.is_empty()
                && movement_preview[0].0 > self.movement_range.selected()
            {
                movement_preview.remove(0);
            }

            if movement_preview.len() == 1 {
                // A path consisting only of one node is not valid.
                self.set_an_arbitrary_valid_movement_preview();
            }
        }
    }

    pub fn take_movement_path(&mut self) -> Vec<(u32, u32)> {
        let mut reversed_path = self.movement_preview.take().unwrap();

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

    fn character_screen_pos(
        &self,
        character_id: CharacterId,
        character_pos: (i32, i32),
    ) -> (f32, f32) {
        if let Some(motion) = self.character_motion {
            if motion.character_id == character_id {
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
            self.grid_x_to_screen(character_pos.0),
            self.grid_y_to_screen(character_pos.1),
        )
    }

    fn draw_cell_outline(&self, (grid_x, grid_y): (i32, i32), color: Color, margin: f32) {
        draw_rectangle_lines(
            self.grid_x_to_screen(grid_x) + margin,
            self.grid_y_to_screen(grid_y) + margin,
            self.cell_w - margin * 2.0,
            self.cell_w - margin * 2.0,
            2.0,
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

    pub fn clear_target(&mut self) {
        if let Target::Some(id) = self.target {
            self.target = Target::Memorized(id);
        }
    }

    pub fn target(&self) -> Option<CharacterId> {
        match self.target {
            Target::Some(id) => Some(id),
            _ => None,
        }
    }

    pub fn ensure_has_npc_target(&mut self) {
        match self.target {
            Target::Some(_) => {}
            Target::Memorized(id) => self.target = Target::Some(id),
            Target::None => {
                // pick an arbitrary enemy
                for (id, character) in self.characters.iter_with_ids() {
                    if *id != self.active_character_id && !character.borrow().player_controlled {
                        self.target = Target::Some(*id);
                        break;
                    }
                }
            }
        }
    }

    pub fn set_target(&mut self, target_character_id: CharacterId) {
        self.target = Target::Some(target_character_id);
    }

    pub fn draw(&mut self, blocked_screen_area: Rect, receptive_to_input: bool) -> GridOutcome {
        let (w, h) = self.size;

        let (x, y) = self.position_on_screen;

        draw_rectangle(x, y, w, h, BACKGROUND_COLOR);

        let mouse_relative_to_grid = |(x, y): (f32, f32)| {
            (
                ((self.camera_position.0.get() + x) / self.cell_w).floor() as i32,
                ((self.camera_position.1.get() + y) / self.cell_w).floor() as i32,
            )
        };

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
                        dest_size: Some(Vec2::new(64.0, 64.0)),
                        ..Default::default()
                    };
                    let i = self.cell_backgrounds[(row * self.grid_dimensions.0 + col) as usize];
                    let texture = &self.background_textures[i];
                    draw_texture_ex(texture, x0, y0, WHITE, params);
                }
            }
        }

        let active_char_pos = self.characters.get(self.active_character_id).position_i32();

        if let Some((range, reach)) = self.action_range_indicator {
            self.draw_out_of_range_indicator(active_char_pos, range, reach);
        }

        self.draw_active_character_outline();

        for ch in self.characters.iter() {
            let ch = ch.borrow();

            let position = ch.position_i32();

            let params = DrawTextureParams {
                dest_size: Some((self.cell_w, self.cell_w).into()),
                ..Default::default()
            };

            let (x, y) = self.character_screen_pos(ch.id(), position);

            draw_texture_ex(&self.sprites[&ch.sprite], x, y, WHITE, params.clone());

            if let Some(weapon) = ch.weapon(HandType::MainHand) {
                if let Some(texture) = weapon.sprite {
                    draw_texture_ex(&self.sprites[&texture], x, y, WHITE, params.clone());
                }
            }

            if let Some(shield) = ch.shield() {
                if let Some(texture) = shield.sprite {
                    draw_texture_ex(&self.sprites[&texture], x, y, WHITE, params);
                }
            }
        }

        let (mouse_x, mouse_y) = mouse_position();
        let mouse_relative = (mouse_x - x, mouse_y - y);

        let mut character_positions = vec![];
        for character in self.characters.iter() {
            character_positions.push(character.borrow().position_i32());
        }

        let (mouse_grid_x, mouse_grid_y) = mouse_relative_to_grid(mouse_relative);

        let is_mouse_within_grid = (0f32..w).contains(&mouse_relative.0)
            && (0..self.grid_dimensions.0).contains(&mouse_grid_x)
            && (0f32..h).contains(&mouse_relative.1)
            && (0..self.grid_dimensions.1).contains(&mouse_grid_y);
        let is_mouse_blocked = blocked_screen_area.contains((mouse_x, mouse_y).into());

        let receptive_to_input = receptive_to_input
            && self
                .characters
                .get(self.active_character_id)
                .player_controlled;

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
                || is_mouse_button_down(MouseButton::Middle)
            {
                self.dragging_camera_from = Some(mouse_relative);
            }
        }

        let mut outcome = GridOutcome::default();

        self.draw_movement_preview_background();

        if is_mouse_within_grid && !is_mouse_blocked && receptive_to_input {
            let collision = character_positions.contains(&(mouse_grid_x, mouse_grid_y));

            let valid_move_route =
                match self.pathfind_grid.routes.get(&(mouse_grid_x, mouse_grid_y)) {
                    Some(route)
                        if route.distance_from_start <= self.movement_range.max() && !collision =>
                    {
                        Some(route)
                    }
                    _ => None,
                };

            let mut hovered_npc_id = None;
            let mut hovering_active_player_controlled = false;
            for character in self.characters.iter() {
                if character.borrow().position_i32() == (mouse_grid_x, mouse_grid_y) {
                    let id = character.borrow().id();
                    outcome.hovered_character_id = Some(id);
                    if !character.borrow().player_controlled {
                        hovered_npc_id = Some(id);
                    } else if id == self.active_character_id {
                        hovering_active_player_controlled = true
                    }
                }
            }

            if let Some(move_route) = valid_move_route {
                if self.dragging_camera_from.is_none() {
                    let destination = (mouse_grid_x, mouse_grid_y);

                    let path = self.build_path_from_route(active_char_pos, destination);

                    self.draw_movement_path_arrow(&path, HOVER_MOVEMENT_ARROW_COLOR);
                    self.draw_cell_outline(destination, HOVER_VALID_MOVEMENT_COLOR, 5.0);

                    if is_mouse_button_pressed(MouseButton::Left) {
                        self.movement_range.selected_i = self
                            .movement_range
                            .shortest_encompassing(move_route.distance_from_start);
                        outcome.switched_to = Some(GridSwitchedTo::Move {
                            selected_option: self.movement_range.selected_i,
                        });

                        self.movement_preview = Some(path);
                    }
                }
            } else if let Some(id) = hovered_npc_id {
                self.draw_cell_outline((mouse_grid_x, mouse_grid_y), HOVER_NPC_COLOR, 1.0);
                if is_mouse_button_pressed(MouseButton::Left) {
                    match self.target {
                        Target::Some(_) => {}
                        Target::Memorized(_) | Target::None => {
                            outcome.switched_to = Some(GridSwitchedTo::Attack)
                        }
                    }
                    outcome.switched_target = true;
                    self.target = Target::Some(id);
                    self.movement_preview = None;
                }
            } else if hovering_active_player_controlled {
                if is_mouse_button_pressed(MouseButton::Left) {
                    outcome.switched_to = Some(GridSwitchedTo::Idle);
                }
            } else if self.movement_preview.is_some() && self.dragging_camera_from.is_none() {
                self.draw_cell_outline(
                    (mouse_grid_x, mouse_grid_y),
                    HOVER_INVALID_MOVEMENT_COLOR,
                    5.0,
                );
            }
        }

        self.draw_selected_character_outline();

        if let Some(movement_preview) = &self.movement_preview {
            if !movement_preview.is_empty() {
                let destination = movement_preview[0].1;
                let distance = movement_preview[0].0;

                self.draw_movement_path_arrow(movement_preview, MOVEMENT_ARROW_COLOR);

                self.draw_static_text_lines(
                    destination,
                    &format!("Move {:.3}", distance.to_string()),
                    &[],
                    4.0,
                    14.0,
                );
            }
        }

        self.draw_target_crosshair();

        self.draw_effects();

        self.draw_static_text();

        outcome
    }

    fn draw_active_character_outline(&self) {
        let active_char_pos = self.characters.get(self.active_character_id).position_i32();
        let (x, y) = self.character_screen_pos(self.active_character_id, active_char_pos);
        let margin = 3.0;
        draw_rectangle(
            x + margin,
            y + margin,
            self.cell_w - margin * 2.0,
            self.cell_w - margin * 2.0,
            ACTIVE_CHARACTER_COLOR,
        );
    }

    fn draw_selected_character_outline(&self) {
        let selected_char_pos = self
            .characters
            .get(self.selected_character_id)
            .position_i32();
        let (x, y) = self.character_screen_pos(self.selected_character_id, selected_char_pos);
        let margin = 1.0;

        let left = x - margin;
        let top = y - margin;
        let right = x + self.cell_w + margin;
        let bot = y + self.cell_w + margin;
        let len = 15.0;
        let thickness = 2.0;
        let color = SELECTED_CHARACTER_COLOR;
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

    fn draw_target_crosshair(&self) {
        if let Target::Some(target_character_i) = self.target {
            let actor_pos = self.characters.get(self.active_character_id).position_i32();
            let target_pos = self.characters.get(target_character_i).position_i32();
            self.draw_cell_outline(target_pos, TARGET_NPC_COLOR, 1.0);
            draw_circle_lines(
                self.grid_x_to_screen(target_pos.0) + self.cell_w / 2.0,
                self.grid_y_to_screen(target_pos.1) + self.cell_w / 2.0,
                self.cell_w * 0.2,
                2.0,
                TARGET_CROSSHAIR_COLOR,
            );
            draw_arrow(
                (
                    self.grid_x_to_screen(target_pos.0),
                    self.grid_y_to_screen(target_pos.1),
                ),
                self.cell_w,
                (1, 1),
                TARGET_CROSSHAIR_COLOR,
            );
            draw_arrow(
                (
                    self.grid_x_to_screen(target_pos.0),
                    self.grid_y_to_screen(target_pos.1),
                ),
                self.cell_w,
                (-1, -1),
                TARGET_CROSSHAIR_COLOR,
            );

            draw_dashed_line(
                (
                    self.grid_x_to_screen(actor_pos.0) + self.cell_w / 2.0,
                    self.grid_y_to_screen(actor_pos.1) + self.cell_w / 2.0,
                ),
                (
                    self.grid_x_to_screen(target_pos.0) + self.cell_w / 2.0,
                    self.grid_y_to_screen(target_pos.1) + self.cell_w / 2.0,
                ),
                2.0,
                TARGET_CROSSHAIR_COLOR,
            );
        }
    }

    fn draw_static_text(&self) {
        if let Some(StaticText {
            position,
            header,
            details,
        }) = &self.static_text
        {
            self.draw_static_text_lines(*position, header, details, 6.0, 0.0);
        }
    }

    fn draw_static_text_lines(
        &self,
        position: (i32, i32),
        header: &str,
        details: &[(String, Goodness)],
        pad: f32,
        y_offset: f32,
    ) {
        let (mut x, mut y) = (
            self.grid_x_to_screen(position.0),
            self.grid_y_to_screen(position.1) + y_offset,
        );
        let header_font_size = 16;
        let detail_font_size = 18;
        let params = TextParams {
            font: Some(&self.big_font),
            font_size: header_font_size,
            color: WHITE,
            ..Default::default()
        };

        let mut h = 0.0;
        let line_margin = 8.0;

        let text_dimensions = measure_text(header, Some(&self.big_font), header_font_size, 1.0);
        let header_w = text_dimensions.width;
        if text_dimensions.height.is_finite() {
            h += text_dimensions.height + line_margin;
            y -= text_dimensions.height + line_margin;
        }

        let mut details_w = 0.0;
        let mut details_h = 0.0;

        let detail_margin = 8.0;

        for (i, (line, goodness)) in details.iter().enumerate() {
            let text_dimensions =
                measure_text(line, Some(&self.simple_font), detail_font_size, 1.0);
            details_w += text_dimensions.width + detail_margin;
            if text_dimensions.height.is_finite() && text_dimensions.height > details_h {
                details_h = text_dimensions.height;
            }
        }
        details_w = (details_w - detail_margin).max(0.0);
        let w = header_w.max(details_w) + 2.0 * pad;
        h += details_h;
        h += 2.0 * pad;

        if w > self.cell_w {
            x -= (w - self.cell_w) / 2.0;
        }

        y -= details_h;

        draw_rectangle(x, y - pad, w, h, Color::new(0.0, 0.0, 0.0, 0.7));

        y += pad;
        y += 5.0;

        x += pad;

        let dimensions = draw_text_ex(header, x, y, params.clone());
        y += dimensions.height + line_margin;

        for (i, (line, goodness)) in details.iter().enumerate() {
            let mut params = params.clone();
            params.font = Some(&self.simple_font);
            params.font_size = detail_font_size;
            match goodness {
                Goodness::Good => params.color = GREEN,
                Goodness::Neutral => {}
                Goodness::Bad => params.color = Color::new(1.0, 0.5, 0.5, 1.0),
            }
            let dimensions = draw_text_ex(line, x, y, params);
            //y += dimensions.height + line_margin;
            x += dimensions.width + detail_margin;
        }
    }

    fn draw_movement_preview_background(&self) {
        if let Some(_movement_preview) = &self.movement_preview {
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
    }

    fn draw_movement_path_arrow(&self, path: &[(f32, (i32, i32))], color: Color) {
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
                2.0,
                color,
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

    fn draw_out_of_range_indicator(&self, origin: (i32, i32), range: Range, reach: AttackReach) {
        let range_ceil = (f32::from(range)).ceil() as i32;
        let range_squared = range.squared() as i32;
        let color = match reach {
            AttackReach::Yes => ACTION_WITHIN_RANGE_COLOR,
            AttackReach::YesButFarDisadvantage | AttackReach::YesButMeleeDisadvantage => {
                ACTION_WITHIN_RANGE_BUT_DISADV_COLOR
            }
            AttackReach::No => ACTION_OUT_OF_RANGE_COLOR,
        };
        let is_cell_within =
            |x: i32, y: i32| (x - origin.0).pow(2) + (y - origin.1).pow(2) <= range_squared;
        for x in
            (origin.0 - range_ceil).max(0)..=(origin.0 + range_ceil).min(self.grid_dimensions.0 - 1)
        {
            for y in (origin.1 - range_ceil).max(0)
                ..=(origin.1 + range_ceil).min(self.grid_dimensions.1 - 1)
            {
                let thickness = 2.0;
                if is_cell_within(x, y) {
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
        if left {
            // Left border
            draw_dashed_line(
                (self.grid_x_to_screen(x), self.grid_y_to_screen(y)),
                (self.grid_x_to_screen(x), self.grid_y_to_screen(y + 1)),
                thickness,
                color,
            );
        }
        if right {
            // Right border
            draw_dashed_line(
                (self.grid_x_to_screen(x + 1), self.grid_y_to_screen(y)),
                (self.grid_x_to_screen(x + 1), self.grid_y_to_screen(y + 1)),
                thickness,
                color,
            );
        }
        if top {
            // Top border
            draw_dashed_line(
                (self.grid_x_to_screen(x), self.grid_y_to_screen(y)),
                (self.grid_x_to_screen(x + 1), self.grid_y_to_screen(y)),
                thickness,
                color,
            );
        }
        if bottom {
            // Bottom border
            draw_dashed_line(
                (self.grid_x_to_screen(x), self.grid_y_to_screen(y + 1)),
                (self.grid_x_to_screen(x + 1), self.grid_y_to_screen(y + 1)),
                thickness,
                color,
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

#[derive(Default)]
pub struct GridOutcome {
    pub switched_to: Option<GridSwitchedTo>,
    pub hovered_character_id: Option<CharacterId>,
    pub switched_target: bool,
}

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
    Text(String, Font),
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
            EffectGraphics::Text(text, font) => {
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
                text_params.color = YELLOW;
                draw_text_ex(text, x0, y0, text_params);
            }
        }
    }
}
