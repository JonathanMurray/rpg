use std::{borrow::Cow, cell::Cell};

use macroquad::{
    color::{BLACK, WHITE},
    input::mouse_position,
    math::Rect,
    shapes::draw_rectangle,
    text::{Font, TextParams},
};

use crate::{
    base_ui::{
        draw_debug, draw_text_rounded, draw_text_with_font_tags, measure_text_with_font_tags,
        Drawable,
    },
    core::ConditionInfo,
    resources::{draw_status_icon, SIMPLE_FONT},
    tooltip::{draw_tooltip, Keyword, Side, TooltipPositionPreference},
    util::COL_TEXT_REGULAR,
};

pub struct ConditionsList {
    pub infos: Vec<ConditionInfo>,
    size: Cell<(f32, f32)>,
    hovered_tooltip: Cell<Option<(Rect, ConditionInfo)>>,
}

impl ConditionsList {
    pub fn new(infos: Vec<ConditionInfo>) -> Self {
        // We need to start out with an accurate height, to prevent the parent container from "flickering" the first time it renders
        let approx_size = (
            1.0,
            infos.len() as f32 * CONDITIONS_LIST_LINE_H + BOTTOM_PAD,
        );
        Self {
            infos,
            size: Cell::new(approx_size),
            hovered_tooltip: Default::default(),
        }
    }
}

impl Drawable for ConditionsList {
    fn draw(&self, x: f32, y: f32) {
        let size = self.draw_conditions(x, y, &self.infos);
        self.size.set(size);
    }

    fn draw_tooltips(&self, _x: f32, _y: f32) {
        if let Some((rect, condition_info)) = self.hovered_tooltip.get() {
            let populated_description = condition_info.populated_description();
            let content_lines: Vec<&str> = populated_description.split("\n").collect();
            draw_tooltip(
                TooltipPositionPreference::RelativeToRect(rect, Side::Right),
                condition_info.name,
                None,
                &content_lines,
                &[],
                Some(Keyword::Cond(
                    condition_info.condition,
                    condition_info.stacks,
                )),
                None,
            );
        }
    }

    fn size(&self) -> (f32, f32) {
        self.size.get()
    }
}

const BOTTOM_PAD: f32 = 10.0;
const CONDITIONS_LIST_LINE_H: f32 = 22.0;

impl ConditionsList {
    fn draw_conditions(&self, x: f32, y: f32, condition_infos: &[ConditionInfo]) -> (f32, f32) {
        let simple_font = SIMPLE_FONT.get().unwrap();
        self.hovered_tooltip.set(None);
        let text_params = TextParams {
            font: Some(simple_font),
            font_size: 16,
            color: COL_TEXT_REGULAR,
            ..Default::default()
        };
        let (mouse_x, mouse_y) = mouse_position();

        let mut max_w = 0.0;

        let mut y_offset = 0.0;

        let status_w = 20.0;

        for info in condition_infos {
            y_offset += CONDITIONS_LIST_LINE_H;
            let y0 = y + y_offset;
            let icon_text_margin = 6.0;

            let status_y = y0 + 5.0 - status_w;
            draw_rectangle(x, status_y, status_w, status_w, BLACK);
            draw_status_icon(
                info.condition.status_id(),
                x,
                status_y,
                Some((status_w, status_w)),
            );

            let x0 = x + status_w + icon_text_margin;

            let text = &condition_info_str(info);

            let dim =
                measure_text_with_font_tags(text, text_params.font, text_params.font_size, 1.0);
            let text_w = dim.width;
            let text_h = dim.height;
            let w = text_w + status_w + icon_text_margin;
            if (x..x0 + text_w).contains(&mouse_x) && (y0 - text_h..y0).contains(&mouse_y) {
                let rect = Rect::new(x, y0 - text_h, w, text_h);
                draw_rectangle(rect.x, rect.y, rect.w, rect.h, WHITE.with_alpha(0.2));
                self.hovered_tooltip.set(Some((rect, *info)));
            }

            let (text_w, text_h) =
                draw_text_with_font_tags(text, x0, y0, text_params.clone(), true);

            if w > max_w {
                max_w = w;
            }
        }

        let (w, h) = (max_w, y_offset + BOTTOM_PAD);

        draw_debug(x, y, w, h);

        (w, h)
    }
}

fn condition_info_str(info: &ConditionInfo) -> String {
    let mut s = info.name.to_string();

    if let Some(stacks) = info.stacks {
        s.push_str(&format!(" x{stacks}"));
    }
    if let Some(remaining_rounds) = info.remaining_rounds {
        let suffix = if remaining_rounds > 1 { "s" } else { "" };
        s.push_str(&format!(" |<faded>({} round{})|", remaining_rounds, suffix));
    }

    s
}
