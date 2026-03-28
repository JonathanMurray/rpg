use macroquad::{
    color::{Color, GRAY, ORANGE, RED, WHITE, YELLOW},
    math::Rect,
    miniquad::window::screen_size,
    shapes::draw_rectangle,
    text::{Font, TextParams},
};

use crate::{
    base_ui::{draw_text_with_font_tags, measure_text_with_font_tags},
    core::{Condition, Goodness},
    drawing::draw_rounded_rectangle_lines,
    textures::{draw_status_icon, StatusId},
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Keyword {
    Cond(Condition),
    Advantage,
    Pushed,
    Graze,
    Crit,
}

impl Keyword {
    fn name(&self) -> &str {
        match self {
            Keyword::Cond(condition) => condition.name(),
            Keyword::Advantage => "Advantage / Disadvantage",
            Keyword::Pushed => "Pushed",
            Keyword::Graze => "Graze",
            Keyword::Crit => "Crit",
        }
    }

    fn description(&self) -> &str {
        match self {
            Keyword::Cond(condition) => condition.description(),
            Keyword::Advantage => "Roll extra dice and take the highest / lowest result",
            Keyword::Pushed => {
                "Moves up to |<value>x| steps. On collision: |<value>1| damage per remaining distance"
            }
            Keyword::Graze => "|<value>-50%| effect. Triggers when |<dice>| roll is 5 or lower",
            Keyword::Crit => "|<value>+50%| effect. Triggers on |<dice>| roll is 16 or higher",
        }
    }

    fn goodness(&self) -> Goodness {
        match self {
            Keyword::Cond(condition) => {
                if condition.is_positive() {
                    Goodness::Good
                } else {
                    Goodness::Bad
                }
            }
            Keyword::Advantage => Goodness::Neutral,
            Keyword::Pushed => Goodness::Bad,
            Keyword::Graze => Goodness::Bad,
            Keyword::Crit => Goodness::Good,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TooltipPositionPreference {
    RelativeToRect(Rect, Side),
    HorCenteredAt((f32, f32)),
    At((f32, f32)),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

pub fn draw_regular_tooltip(
    font: &Font,
    pos_preference: TooltipPositionPreference,
    header: &str,
    error: Option<&'static str>,
    content_lines: &[String],
) -> Rect {
    draw_tooltip(
        font,
        pos_preference,
        header,
        error,
        content_lines,
        &[],
        None,
    )
}

pub fn draw_tooltip(
    font: &Font,
    pos_preference: TooltipPositionPreference,
    header: &str,
    error: Option<&'static str>,
    content_lines: &[String],
    has_keywords: &[Keyword],
    header_keyword: Option<Keyword>,
) -> Rect {
    let header_font_size = if header_keyword.is_some() { 16 } else { 24 };
    let font_size = 16;
    let mut max_line_w = 0.0;
    let text_margin = 8.0;

    let goodness = if let Some(keyword) = header_keyword {
        keyword.goodness()
    } else {
        Goodness::Neutral
    };

    let mut measure_width = |line, size| {
        let dimensions = measure_text_with_font_tags(line, Some(font), size, 1.0);
        if dimensions.width > max_line_w {
            max_line_w = dimensions.width;
        }
    };

    measure_width(header, header_font_size);
    if let Some(error) = error.as_ref() {
        measure_width(error, font_size)
    }

    // The lines provided by the caller can be longer than desired, so we introduce line breaks here to limit
    // the width of the tooltip window.
    let line_width_limit = if header_keyword.is_some() { 25 } else { 40 };
    let mut physical_content_lines = vec![];
    for line in content_lines {
        let mut line = &line[..];
        while line.len() > line_width_limit {
            if let Some(whitespace_i) = line[line_width_limit..].find(" ") {
                let (left, right) = line.split_at(line_width_limit + whitespace_i);
                physical_content_lines.push(left);
                line = &right[1..];
            } else {
                // No whitespace found. We'll allow the entire line then.
                break;
            }
        }
        physical_content_lines.push(line);
    }

    for line in &physical_content_lines {
        measure_width(line, font_size);
    }

    let tooltip_w = max_line_w + text_margin * 2.0;

    let empty_line_h = 12.0;
    let line_h = 22.0;

    let num_real_lines = 1
        + physical_content_lines
            .iter()
            .filter(|line| !line.is_empty())
            .count()
        + error.map(|_| 1).unwrap_or(0);
    let num_empty_lines = physical_content_lines
        .iter()
        .filter(|line| line.is_empty())
        .count();
    let tooltip_h =
        num_real_lines as f32 * line_h + text_margin * 2.0 + num_empty_lines as f32 * empty_line_h;

    let (screen_w, screen_h) = screen_size();

    let (x, y) = match pos_preference {
        TooltipPositionPreference::RelativeToRect(rect, mut pos_preference) => {
            if pos_preference == Side::Top && rect.top() - tooltip_h < 0.0 {
                pos_preference = Side::Bottom;
            }
            if pos_preference == Side::Bottom && rect.bottom() + tooltip_h > screen_h {
                pos_preference = Side::Top;
            }
            if pos_preference == Side::Left && rect.left() - tooltip_w < 0.0 {
                pos_preference = Side::Right;
            }
            if pos_preference == Side::Right && rect.right() + tooltip_w > screen_w {
                pos_preference = Side::Left;
            }

            let space = 3.0;

            match pos_preference {
                Side::Top => (
                    rect.left().min(screen_w - tooltip_w),
                    rect.top() - space - tooltip_h,
                ),
                Side::Right => (rect.right() + space, rect.top().min(screen_h - tooltip_h)),
                Side::Bottom => (rect.left().min(screen_w - tooltip_w), rect.bottom() + space),
                Side::Left => (
                    rect.left() - space - tooltip_w,
                    rect.top().min(screen_h - tooltip_h),
                ),
            }
        }
        TooltipPositionPreference::HorCenteredAt((x, y)) => (x - tooltip_w / 2.0, y),
        TooltipPositionPreference::At(pos) => pos,
    };

    let tooltip_rect = Rect::new(x, y, tooltip_w, tooltip_h);
    let bg_color = match goodness {
        Goodness::Good => Color::new(0.0, 0.2, 0.0, 0.8),
        Goodness::Neutral => Color::new(0.0, 0.0, 0.0, 0.8),
        Goodness::Bad => Color::new(0.2, 0.0, 0.0, 0.9),
    };
    draw_rectangle(
        tooltip_rect.x,
        tooltip_rect.y,
        tooltip_rect.w,
        tooltip_rect.h,
        bg_color,
    );
    draw_rounded_rectangle_lines(
        tooltip_rect.x,
        tooltip_rect.y,
        tooltip_rect.w,
        tooltip_rect.h,
        1.0,
        GRAY,
        3.0,
        None,
    );

    let text_params = TextParams {
        font: Some(font),
        font_size,
        color: WHITE,
        ..Default::default()
    };

    let mut line_y = tooltip_rect.y + text_margin * 2.0 + 5.0;

    let mut draw_line =
        |line: &str, color: Option<Color>, is_header: bool, status_icon: Option<StatusId>| {
            let text_x = tooltip_rect.x + text_margin;
            let mut params = text_params.clone();
            if let Some(c) = color {
                params.color = c;
            }
            if is_header {
                params.font_size = header_font_size;
            }
            if line.is_empty() {
                line_y += empty_line_h;
                0.0
            } else {
                let w = draw_text_with_font_tags(line, text_x, line_y, params, true);

                if let Some(status) = status_icon {
                    let status_w = 20.0;
                    draw_status_icon(
                        status,
                        text_x + w + 8.0,
                        line_y - status_w + 5.0,
                        Some((status_w, status_w)),
                    );
                }

                line_y += line_h;
                w
            }
        };

    let header_color = if header_keyword.is_some() {
        ORANGE
    } else {
        YELLOW
    };

    let header_status_icon = header_keyword.and_then(|keyword| match keyword {
        Keyword::Cond(condition) => Some(condition.status_icon()),
        _ => None,
    });

    draw_line(header, Some(header_color), true, header_status_icon);

    if let Some(error) = error {
        draw_line(error, Some(RED), false, None);
    }
    for line in physical_content_lines {
        draw_line(line, None, false, None);
    }
    draw_keyword_tooltips_relative_to_rect(font, has_keywords, tooltip_rect);

    tooltip_rect
}

pub fn draw_keyword_tooltips_relative_to_rect(font: &Font, keywords: &[Keyword], mut rect: Rect) {
    for (i, keyword) in keywords.iter().enumerate() {
        let pos_preference = if i == 0 {
            TooltipPositionPreference::RelativeToRect(rect, Side::Right)
        } else {
            TooltipPositionPreference::At((rect.x, rect.y + rect.h))
        };
        rect = draw_tooltip(
            font,
            pos_preference,
            keyword.name(),
            None,
            &[keyword.description().to_string()],
            &[],
            Some(*keyword),
        );
        rect.y += 2.0;
        //y += rect.h;
    }
}

pub fn draw_keyword_tooltips(font: &Font, keywords: &[Keyword], x: f32, mut y: f32) {
    for keyword in keywords {
        let rect = draw_tooltip(
            font,
            TooltipPositionPreference::At((x, y)),
            keyword.name(),
            None,
            &[keyword.description().to_string()],
            &[],
            Some(*keyword),
        );
        y += rect.h;
    }
}
