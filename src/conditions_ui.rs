use std::{cell::Cell, collections::HashMap};

use macroquad::{
    color::{BLACK, WHITE},
    input::mouse_position,
    math::Rect,
    shapes::draw_rectangle,
    text::{Font, TextParams},
    texture::{draw_texture_ex, DrawTextureParams, Texture2D},
};

use crate::{
    action_button::{draw_regular_tooltip, Side, TooltipPositionPreference},
    base_ui::{draw_text_rounded, Drawable},
    core::ConditionInfo,
    textures::StatusId,
};

pub struct ConditionsList {
    pub font: Font,
    pub infos: Vec<ConditionInfo>,
    size: Cell<(f32, f32)>,
    status_textures: HashMap<StatusId, Texture2D>,
}

impl ConditionsList {
    pub fn new(
        font: Font,
        infos: Vec<ConditionInfo>,
        status_textures: HashMap<StatusId, Texture2D>,
    ) -> Self {
        // We need to start out with an accurate height, to prevent the parent container from "flickering" the first time it renders
        let approx_size = (1.0, infos.len() as f32 * CONDITIONS_LIST_LINE_H);
        Self {
            font,
            infos,
            size: Cell::new(approx_size),
            status_textures,
        }
    }
}

impl Drawable for ConditionsList {
    fn draw(&self, x: f32, y: f32) {
        let size = self.draw_conditions(x, y, &self.font, &self.infos);
        self.size.set(size);
    }

    fn size(&self) -> (f32, f32) {
        self.size.get()
    }
}

const CONDITIONS_LIST_LINE_H: f32 = 22.0;

impl ConditionsList {
    fn draw_conditions(
        &self,
        x: f32,
        y: f32,
        font: &Font,
        condition_infos: &[ConditionInfo],
    ) -> (f32, f32) {
        let text_params = TextParams {
            font: Some(font),
            font_size: 18,
            color: WHITE,
            ..Default::default()
        };
        let (mouse_x, mouse_y) = mouse_position();

        let mut tooltip = None;

        let mut max_w = 0.0;

        let mut y_offset = 0.0;

        let status_w = 20.0;

        for info in condition_infos {
            y_offset += CONDITIONS_LIST_LINE_H;
            let y0 = y + y_offset;
            let x0 = x + status_w + 2.0;

            let texture = &self.status_textures[&info.condition.status_icon()];
            let status_y = y0 + 5.0 - status_w;
            draw_rectangle(x, status_y, status_w, status_w, BLACK);
            draw_texture_ex(
                texture,
                x,
                status_y,
                WHITE,
                DrawTextureParams {
                    dest_size: Some((status_w, status_w).into()),
                    ..Default::default()
                },
            );

            let dimensions = draw_text_rounded(&format!("{}", info), x0, y0, text_params.clone());

            if (x..x0 + dimensions.width).contains(&mouse_x)
                && (y0 - dimensions.height..y0).contains(&mouse_y)
            {
                tooltip = Some((
                    Rect::new(
                        x,
                        y0 - dimensions.height,
                        dimensions.width,
                        dimensions.height,
                    ),
                    info,
                ));
            }

            let w = dimensions.width + status_w + 2.0;

            if w > max_w {
                max_w = w;
            }
        }

        if let Some((rect, condition)) = tooltip {
            draw_regular_tooltip(
                font,
                TooltipPositionPreference::RelativeToRect(rect, Side::Right),
                condition.name,
                None,
                &[condition.description.to_string()],
            );
        }

        (max_w, y_offset)
    }
}
