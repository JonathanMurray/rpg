use std::cell::Cell;

use macroquad::{
    color::WHITE,
    input::mouse_position,
    math::Rect,
    text::{draw_text_ex, Font, TextParams},
};

use crate::{
    action_button::{draw_tooltip, TooltipPositionPreference},
    base_ui::Drawable,
    core::ConditionInfo,
};

pub struct ConditionsList {
    pub font: Font,
    pub descriptions: Vec<(ConditionInfo, Option<u32>)>,
    size: Cell<(f32, f32)>,
}

impl ConditionsList {
    pub fn new(font: Font, descriptions: Vec<(ConditionInfo, Option<u32>)>) -> Self {
        Self {
            font,
            descriptions,
            size: Cell::new((0.0, 0.0)),
        }
    }
}

impl Drawable for ConditionsList {
    fn draw(&self, x: f32, y: f32) {
        let size = draw_conditions(x, y, &self.font, &self.descriptions);
        self.size.set(size);
    }

    fn size(&self) -> (f32, f32) {
        self.size.get()
    }
}

fn draw_conditions(
    x: f32,
    y: f32,
    font: &Font,
    condition_descriptions: &[(ConditionInfo, Option<u32>)],
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

    let line_height = 20.0;

    let mut y_offset = 0.0;

    for (condition, stacks) in condition_descriptions {
        y_offset += line_height;
        let y0 = y + y_offset;
        let dimensions = if let Some(stacks) = stacks {
            draw_text_ex(
                &format!("{} ({})", condition.name, stacks),
                x,
                y0,
                text_params.clone(),
            )
        } else {
            draw_text_ex(condition.name, x, y0, text_params.clone())
        };

        if (x..x + dimensions.width).contains(&mouse_x)
            && (y0 - dimensions.height..y0).contains(&mouse_y)
        {
            tooltip = Some((
                Rect::new(
                    x,
                    y0 - dimensions.height,
                    dimensions.width,
                    dimensions.height,
                ),
                condition,
            ));
        }

        if dimensions.width > max_w {
            max_w = dimensions.width;
        }
    }

    if let Some((rect, condition)) = tooltip {
        draw_tooltip(
            font,
            rect,
            TooltipPositionPreference::Right,
            &[
                condition.name.to_string(),
                condition.description.to_string(),
            ],
        );
    }

    (max_w, y_offset)
}
