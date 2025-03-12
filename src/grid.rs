use std::collections::HashMap;

use macroquad::color::Color;

use std::{
    cell::{Cell, Ref, RefCell},
    rc::Rc,
};

use macroquad::math::Rect;
use macroquad::{color::PURPLE, input::is_key_down};

use macroquad::miniquad::KeyCode;
use macroquad::texture::{draw_texture_ex, load_texture, DrawTextureParams, FilterMode, Texture2D};
use macroquad::{
    color::{
        self, BLACK, BLUE, DARKBROWN, DARKGRAY, GOLD, GRAY, GREEN, LIGHTGRAY, MAGENTA, ORANGE, RED,
        WHITE, YELLOW,
    },
    input::{is_mouse_button_down, is_mouse_button_pressed, mouse_position, MouseButton},
    shapes::{draw_circle, draw_circle_lines, draw_line, draw_rectangle, draw_rectangle_lines},
    text::{draw_text, measure_text},
};

use crate::pathfind::PathfindGrid;
use crate::{
    core::{CharacterId, Characters, HandType, Range, TextureId},
    drawing::{draw_arrow, draw_dashed_line},
};

#[derive(Debug, Copy, Clone)]
struct CharacterMotion {
    character_id: CharacterId,
    from: (i32, i32),
    to: (i32, i32),
    remaining_duration: f32,
    duration: f32,
}

pub struct GameGrid {
    textures: HashMap<TextureId, Texture2D>,
    pathfind_grid: PathfindGrid,
    pub characters: Characters,
    camera_position: (Cell<f32>, Cell<f32>),
    dragging_camera_from: Option<(f32, f32)>,

    effects: Vec<VisualEffect>,

    active_character_id: CharacterId,
    movement_range: f32,
    pub movement_preview: Option<Vec<(f32, (i32, i32))>>,
    pub target_character_id: Option<CharacterId>,
    pub range_indicator: Option<Range>,

    pub receptive_to_input: bool,
    pub grid_dimensions: (i32, i32),
    pub position_on_screen: (f32, f32),

    character_motion: Option<CharacterMotion>,

    cell_w: f32,
    size: (f32, f32),
}

impl GameGrid {
    pub fn new(
        characters: &Characters,
        textures: HashMap<TextureId, Texture2D>,
        size: (f32, f32),
    ) -> Self {
        let characters = characters.clone();

        let grid_dimensions = (16, 12);
        Self {
            textures,
            pathfind_grid: PathfindGrid::new(grid_dimensions),
            dragging_camera_from: None,
            camera_position: (Cell::new(0.0), Cell::new(0.0)),
            characters,
            effects: vec![],
            active_character_id: 0,
            movement_range: 0.0,
            movement_preview: Default::default(),
            target_character_id: None,
            range_indicator: None,
            receptive_to_input: true,
            cell_w: 64.0,
            grid_dimensions,
            position_on_screen: (0.0, 0.0), // is set later
            character_motion: None,
            size,
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

    pub fn update(
        &mut self,
        active_character_id: CharacterId,
        characters: &Characters,
        elapsed: f32,
    ) {
        self.pathfind_grid.blocked_positions.clear();

        self.active_character_id = active_character_id;
        for character in characters.iter() {
            let character = character.borrow();
            let pos = character.position_i32();

            self.pathfind_grid.blocked_positions.insert(pos);
        }

        let pos = self.characters.get(self.active_character_id).position_i32();
        self.pathfind_grid.run(pos, self.movement_range);

        let mut projectile_impacts = vec![];
        for effect in &mut self.effects {
            effect.remaining_duration -= elapsed;
            if effect.remaining_duration <= 0.0 {
                if let VisualEffectContent::Projectile {
                    destination,
                    color,
                    impact_text,
                    radius: _,
                } = &effect.content
                {
                    projectile_impacts.push((*destination, *color, impact_text.clone()));
                }
            }
        }
        self.effects.retain(|e| e.remaining_duration > 0.0);
        for (position, color, text) in projectile_impacts {
            self.effects.push(VisualEffect::new(position, text, 1.0));

            self.effects.push(VisualEffect::new(
                position,
                VisualEffectContent::Circle(color),
                0.1,
            ));
        }

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
        text: impl Into<String>,
        duration: f32,
    ) {
        self.effects
            .push(VisualEffect::new(position, text, duration));
    }

    pub fn add_projectile_effect(
        &mut self,
        source: (i32, i32),
        destination: (i32, i32),
        color: Color,
        duration: f32,
        impact_text: impl Into<String>,
        radius: f32,
    ) {
        self.effects.push(VisualEffect::new(
            source,
            VisualEffectContent::Projectile {
                destination,
                color,
                impact_text: impact_text.into(),
                radius,
            },
            duration,
        ));
    }

    pub fn set_movement_range(&mut self, range: f32) {
        self.movement_range = range;
        if let Some(movement_preview) = &mut self.movement_preview {
            while !movement_preview.is_empty() && movement_preview[0].0 > range {
                movement_preview.remove(0);
            }
        }

        let pos = self.characters.get(self.active_character_id).position_i32();
        self.pathfind_grid.run(pos, self.movement_range);
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
            self.grid_x_to_screen(character_pos.1),
        )
    }

    fn draw_square(&self, (grid_x, grid_y): (i32, i32), color: Color) {
        let margin = -1.0;
        draw_rectangle_lines(
            self.grid_x_to_screen(grid_x) - margin,
            self.grid_y_to_screen(grid_y) - margin,
            self.cell_w + margin * 2.0,
            self.cell_w + margin * 2.0,
            2.0,
            color,
        )
    }

    pub fn draw(&mut self, blocked_screen_area: Rect) -> GridOutcome {
        let (w, h) = self.size;

        let bg_color = GRAY;
        let grid_color = Color::new(0.4, 0.4, 0.4, 1.00);

        let (x, y) = self.position_on_screen;

        draw_rectangle(x, y, w, h, bg_color);

        let mouse_relative_to_grid = |(x, y): (f32, f32)| {
            (
                ((self.camera_position.0.get() + x) / self.cell_w).floor() as i32,
                ((self.camera_position.1.get() + y) / self.cell_w).floor() as i32,
            )
        };

        let active_character_pos = self.characters.get(self.active_character_id).position_i32();

        for col in 0..self.grid_dimensions.0 + 1 {
            let x0 = self.grid_x_to_screen(col);
            draw_line(
                x0,
                self.grid_y_to_screen(0),
                x0,
                self.grid_y_to_screen(self.grid_dimensions.1),
                1.0,
                grid_color,
            );
            for row in 0..self.grid_dimensions.1 + 1 {
                let y0 = self.grid_y_to_screen(row);
                draw_line(
                    self.grid_x_to_screen(0),
                    y0,
                    self.grid_x_to_screen(self.grid_dimensions.0),
                    y0,
                    1.0,
                    grid_color,
                );
            }
        }

        if self.movement_preview.is_some() {
            for pos in self.pathfind_grid.routes.keys() {
                if (0..self.grid_dimensions.0).contains(&pos.0)
                    && (0..self.grid_dimensions.1).contains(&pos.1)
                    && *pos != active_character_pos
                {
                    self.draw_square(*pos, LIGHTGRAY);
                }
            }
        }

        let active_char_pos = self.characters.get(self.active_character_id).position_i32();

        if let Some(range) = self.range_indicator {
            self.draw_range_indicator(active_char_pos, range);
        }

        for ch in self.characters.iter() {
            let ch = ch.borrow();

            let position = ch.position_i32();

            let params = DrawTextureParams {
                dest_size: Some((self.cell_w, self.cell_w).into()),
                ..Default::default()
            };

            let (x, y) = self.character_screen_pos(ch.id(), position);

            draw_texture_ex(&self.textures[&ch.texture], x, y, WHITE, params.clone());

            if let Some(weapon) = ch.weapon(HandType::MainHand) {
                if let Some(texture) = weapon.texture_id {
                    draw_texture_ex(&self.textures[&texture], x, y, WHITE, params.clone());
                }
            }

            if let Some(shield) = ch.shield() {
                if let Some(texture) = shield.texture_id {
                    draw_texture_ex(&self.textures[&texture], x, y, WHITE, params);
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

        let receptive_to_input = self.receptive_to_input
            && self
                .characters
                .get(self.active_character_id)
                .player_controlled;

        if is_mouse_within_grid && receptive_to_input {
            if let Some(dragging_from) = self.dragging_camera_from {
                if is_mouse_button_down(MouseButton::Right) {
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

            if is_mouse_button_pressed(MouseButton::Right) {
                self.dragging_camera_from = Some(mouse_relative);
            }
        }

        let mut outcome = GridOutcome {
            switched_to_move: false,
            switched_to_attack: false,
            hovered_character_id: None,
        };

        if is_mouse_within_grid && !is_mouse_blocked && receptive_to_input {
            let collision = character_positions.contains(&(mouse_grid_x, mouse_grid_y));

            let valid_move_destination =
                match self.pathfind_grid.routes.get(&(mouse_grid_x, mouse_grid_y)) {
                    Some(route) => route.distance_from_start <= self.movement_range,
                    _ => false,
                } && !collision;

            let mut hovered_npc_id = None;
            for character in self.characters.iter() {
                if character.borrow().position_i32() == (mouse_grid_x, mouse_grid_y) {
                    outcome.hovered_character_id = Some(character.borrow().id());
                    if !character.borrow().player_controlled {
                        hovered_npc_id = Some(character.borrow().id());
                    }
                }
            }

            if valid_move_destination {
                let destination = (mouse_grid_x, mouse_grid_y);
                self.draw_square(destination, YELLOW);
                if is_mouse_button_pressed(MouseButton::Left) {
                    if self.movement_preview.is_none() {
                        outcome.switched_to_move = true;
                    }

                    let route = self.pathfind_grid.routes.get(&destination).unwrap();
                    let mut dist = route.distance_from_start;
                    let mut movement_preview = vec![(dist, destination)];
                    let mut pos = route.came_from;

                    loop {
                        let route = self.pathfind_grid.routes.get(&pos).unwrap();
                        dist = route.distance_from_start;
                        movement_preview.push((dist, pos));
                        if pos == active_char_pos {
                            break;
                        }
                        pos = route.came_from;
                    }
                    self.movement_preview = Some(movement_preview);
                }
            } else if let Some(id) = hovered_npc_id {
                self.draw_square((mouse_grid_x, mouse_grid_y), MAGENTA);
                if is_mouse_button_pressed(MouseButton::Left) {
                    if self.target_character_id.is_none() {
                        outcome.switched_to_attack = true;
                    }
                    self.target_character_id = Some(id);
                    self.movement_preview = None;
                }
            } else if self.movement_preview.is_none() && self.dragging_camera_from.is_none() {
                self.draw_square((mouse_grid_x, mouse_grid_y), RED);
            }
        }

        if let Some(movement_preview) = &self.movement_preview {
            if !movement_preview.is_empty() {
                let arrow_color = ORANGE;
                for i in 0..movement_preview.len() - 1 {
                    let a = movement_preview[i].1;
                    let b = movement_preview[i + 1].1;
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
                        arrow_color,
                    );
                }

                let end = movement_preview[0].1;
                let last_direction = (
                    end.0 - movement_preview[1].1 .0,
                    end.1 - movement_preview[1].1 .1,
                );

                draw_arrow(
                    (self.grid_x_to_screen(end.0), self.grid_y_to_screen(end.1)),
                    self.cell_w,
                    last_direction,
                    arrow_color,
                );
            }
        }

        if let Some(target_character_i) = self.target_character_id {
            let actor_pos = self.characters.get(self.active_character_id).position_i32();
            let target_pos = self.characters.get(target_character_i).position_i32();
            self.draw_square(target_pos, MAGENTA);
            draw_circle_lines(
                self.grid_x_to_screen(target_pos.0) + self.cell_w / 2.0,
                self.grid_y_to_screen(target_pos.1) + self.cell_w / 2.0,
                self.cell_w * 0.2,
                2.0,
                WHITE,
            );
            draw_arrow(
                (
                    self.grid_x_to_screen(target_pos.0),
                    self.grid_y_to_screen(target_pos.1),
                ),
                self.cell_w,
                (1, 1),
                WHITE,
            );
            draw_arrow(
                (
                    self.grid_x_to_screen(target_pos.0),
                    self.grid_y_to_screen(target_pos.1),
                ),
                self.cell_w,
                (-1, -1),
                WHITE,
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
                WHITE,
            );
        }

        {
            let (x, y) = self.character_screen_pos(self.active_character_id, active_character_pos);
            let margin = 2.0;
            draw_rectangle_lines(
                x - margin,
                y - margin,
                self.cell_w + margin * 2.0,
                self.cell_w + margin * 2.0,
                2.0,
                GOLD,
            );
        }

        for effect in &self.effects {
            match &effect.content {
                VisualEffectContent::Text(text) => {
                    let font_size = 24;
                    let text_dimensions = measure_text(text, None, font_size, 1.0);

                    let x0 = self.grid_x_to_screen(effect.position.0) + self.cell_w / 2.0
                        - text_dimensions.width / 2.0;
                    let y0 = self.grid_y_to_screen(effect.position.1)
                        - self.cell_w * 0.3 * (1.0 - effect.remaining_duration / effect.duration);

                    draw_text(text, x0, y0, font_size as f32, YELLOW);
                }

                VisualEffectContent::Circle(color) => {
                    let r = self.cell_w
                        * (0.2 + 0.3 * (1.0 - effect.remaining_duration / effect.duration));
                    let x0 = self.grid_x_to_screen(effect.position.0) + self.cell_w / 2.0;
                    let y0 = self.grid_y_to_screen(effect.position.1) + self.cell_w / 2.0;
                    draw_circle(x0, y0, r, *color);
                }

                VisualEffectContent::Projectile {
                    destination,
                    color,
                    impact_text: _,
                    radius,
                } => {
                    let x0 = self.grid_x_to_screen(effect.position.0) + self.cell_w / 2.0;
                    let y0 = self.grid_y_to_screen(effect.position.1) + self.cell_w / 2.0;

                    let x1 = self.grid_x_to_screen(destination.0) + self.cell_w / 2.0;
                    let y1 = self.grid_y_to_screen(destination.1) + self.cell_w / 2.0;

                    let x = x1 - (x1 - x0) * effect.remaining_duration / effect.duration;
                    let y = y1 - (y1 - y0) * effect.remaining_duration / effect.duration;

                    draw_circle(x, y, self.cell_w * radius, *color);
                }
            }
        }

        outcome
    }

    fn draw_range_indicator(&self, origin: (i32, i32), range: Range) {
        let range_ceil = (f32::from(range)).ceil() as i32;
        let range_squared = range.squared() as i32;
        let within =
            |x: i32, y: i32| (x - origin.0).pow(2) + (y - origin.1).pow(2) <= range_squared;
        for x in
            (origin.0 - range_ceil).max(0)..=(origin.0 + range_ceil).min(self.grid_dimensions.0 - 1)
        {
            for y in (origin.1 - range_ceil).max(0)
                ..=(origin.1 + range_ceil).min(self.grid_dimensions.1 - 1)
            {
                if within(x, y) {
                    let color = RED;
                    let thickness = 2.0;
                    if !within(x - 1, y) {
                        // Left border
                        draw_dashed_line(
                            (self.grid_x_to_screen(x), self.grid_y_to_screen(y)),
                            (self.grid_x_to_screen(x), self.grid_y_to_screen(y + 1)),
                            thickness,
                            color,
                        );
                    }
                    if !within(x + 1, y) {
                        // Right border
                        draw_dashed_line(
                            (self.grid_x_to_screen(x + 1), self.grid_y_to_screen(y)),
                            (self.grid_x_to_screen(x + 1), self.grid_y_to_screen(y + 1)),
                            thickness,
                            color,
                        );
                    }
                    if !within(x, y - 1) {
                        // Top border
                        draw_dashed_line(
                            (self.grid_x_to_screen(x), self.grid_y_to_screen(y)),
                            (self.grid_x_to_screen(x + 1), self.grid_y_to_screen(y)),
                            thickness,
                            color,
                        );
                    }
                    if !within(x, y + 1) {
                        // Bottom border
                        draw_dashed_line(
                            (self.grid_x_to_screen(x), self.grid_y_to_screen(y + 1)),
                            (self.grid_x_to_screen(x + 1), self.grid_y_to_screen(y + 1)),
                            thickness,
                            color,
                        );
                    }
                }
            }
        }
    }

    fn pan_camera(&self, dx: f32, dy: f32) {
        let new_x = self.camera_position.0.get() + dx;
        let new_y = self.camera_position.1.get() + dy;
        let max_space = 300.0;
        let max_x = self.grid_dimensions.0 as f32 * self.cell_w + max_space - self.size.0;
        let max_y = self.grid_dimensions.1 as f32 * self.cell_w + max_space - self.size.1;
        self.camera_position.0.set(new_x.max(-max_space).min(max_x));
        self.camera_position.1.set(new_y.max(-max_space).min(max_y));
    }
}

struct VisualEffect {
    position: (i32, i32),
    content: VisualEffectContent,
    remaining_duration: f32,
    duration: f32,
}

enum VisualEffectContent {
    Text(String),
    Circle(Color),
    Projectile {
        destination: (i32, i32),
        color: Color,
        impact_text: String,
        radius: f32,
    },
}

impl<T> From<T> for VisualEffectContent
where
    T: Into<String>,
{
    fn from(t: T) -> Self {
        Self::Text(t.into())
    }
}

impl VisualEffect {
    fn new(position: (i32, i32), content: impl Into<VisualEffectContent>, duration: f32) -> Self {
        Self {
            position,
            content: content.into(),
            remaining_duration: duration,
            duration,
        }
    }
}

pub struct GridOutcome {
    pub switched_to_move: bool,
    pub switched_to_attack: bool,
    pub hovered_character_id: Option<CharacterId>,
}
