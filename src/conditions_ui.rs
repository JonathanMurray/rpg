use std::cell::Cell;

use macroquad::{
    color::{BLACK, WHITE},
    input::mouse_position,
    math::Rect,
    shapes::draw_rectangle,
    text::{Font, TextParams},
};

use crate::{
    base_ui::{draw_text_rounded, Drawable},
    core::ConditionInfo,
    textures::draw_status_icon,
    tooltip::{draw_tooltip, Keyword, Side, TooltipPositionPreference},
};

pub struct ConditionsList {
    pub font: Font,
    pub infos: Vec<ConditionInfo>,
    size: Cell<(f32, f32)>,
    hovered_tooltip: Cell<Option<(Rect, ConditionInfo)>>,
}

impl ConditionsList {
    pub fn new(font: Font, infos: Vec<ConditionInfo>) -> Self {
        // We need to start out with an accurate height, to prevent the parent container from "flickering" the first time it renders
        let approx_size = (1.0, infos.len() as f32 * CONDITIONS_LIST_LINE_H);
        Self {
            font,
            infos,
            size: Cell::new(approx_size),
            hovered_tooltip: Default::default(),
        }
    }
}

impl Drawable for ConditionsList {
    fn draw(&self, x: f32, y: f32) {
        let size = self.draw_conditions(x, y, &self.font, &self.infos);
        self.size.set(size);
    }

    fn draw_tooltips(&self, _x: f32, _y: f32) {
        if let Some((rect, condition_info)) = self.hovered_tooltip.get() {
            let populated_description = condition_info.populated_description();
            let content_lines: Vec<&str> = populated_description.split("\n").collect();
            draw_tooltip(
                &self.font,
                TooltipPositionPreference::RelativeToRect(rect, Side::Right),
                condition_info.name,
                None,
                &content_lines,
                &[],
                Some(Keyword::Cond(condition_info.condition)),
            );
        }
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
        self.hovered_tooltip.set(None);
        let text_params = TextParams {
            font: Some(font),
            font_size: 18,
            color: WHITE,
            ..Default::default()
        };
        let (mouse_x, mouse_y) = mouse_position();

        let mut max_w = 0.0;

        let mut y_offset = 0.0;

        let status_w = 20.0;

        for info in condition_infos {
            y_offset += CONDITIONS_LIST_LINE_H;
            let y0 = y + y_offset;
            let x0 = x + status_w + 2.0;

            let status_y = y0 + 5.0 - status_w;
            draw_rectangle(x, status_y, status_w, status_w, BLACK);
            draw_status_icon(
                info.condition.status_icon(),
                x,
                status_y,
                Some((status_w, status_w)),
            );
            /*
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
             */

            let dimensions = draw_text_rounded(&format!("{}", info), x0, y0, text_params.clone());

            if (x..x0 + dimensions.width).contains(&mouse_x)
                && (y0 - dimensions.height..y0).contains(&mouse_y)
            {
                self.hovered_tooltip.set(Some((
                    Rect::new(
                        x,
                        y0 - dimensions.height,
                        dimensions.width,
                        dimensions.height,
                    ),
                    *info,
                )));
            }

            let w = dimensions.width + status_w + 2.0;

            if w > max_w {
                max_w = w;
            }
        }

        (max_w, y_offset)
    }
}
