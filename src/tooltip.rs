use std::sync::atomic::{AtomicBool, AtomicU32};

use macroquad::{
    color::{Color, GRAY, LIGHTGRAY, ORANGE, RED, WHITE, YELLOW},
    math::Rect,
    shapes::{draw_line, draw_rectangle},
    text::{Font, TextParams},
    texture::draw_texture,
    time::{get_frame_time, get_time},
};

use crate::{
    action_button,
    base_ui::{draw_text_with_font_tags, measure_text_with_font_tags},
    core::{
        cond_description_with_populated_stacks, AttackEnhancement, Condition, ConditionInfo,
        Goodness, OnAttackedReaction, OPPORTUNITY_ATTACK_AP_COST, OPPORTUNITY_ATTACK_STAMINA_COST,
    },
    drawing::draw_rounded_rectangle_lines,
    resources::{draw_icon, draw_status_icon, IconId, StatusId, BIG_FONT, SIMPLE_FONT},
    util::{screen_size, COL_TEXT_HEADER, COL_TEXT_REGULAR},
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Keyword {
    Cond(Condition, Option<u32>),
    SpikesSurface,
    AttackEnhancement(AttackEnhancement),
    OnAttackedReaction(OnAttackedReaction),
    OpportunityAttack,
    Advantage,
    Pushed,
    WeakHit,
    Crit,
    Flanked,
}

impl Keyword {
    fn name(&self) -> String {
        match self {
            Keyword::Cond(condition, _amount) => condition.name().to_string(),
            Keyword::AttackEnhancement(enhancement) => format!(
                "{}{}",
                enhancement.name,
                cost_string(
                    enhancement.action_point_cost,
                    enhancement.stamina_cost,
                    enhancement.mana_cost
                )
            ),
            Keyword::OnAttackedReaction(reaction) => format!(
                "{}{}",
                reaction.name,
                cost_string(reaction.action_point_cost, reaction.stamina_cost, 0)
            ),
            Keyword::OpportunityAttack => format!(
                "Opportunity attack{}",
                cost_string(
                    OPPORTUNITY_ATTACK_AP_COST,
                    OPPORTUNITY_ATTACK_STAMINA_COST,
                    0
                )
            ),
            Keyword::Advantage => "Advantage / Disadvantage".to_string(),
            Keyword::Pushed => "Pushed".to_string(),
            Keyword::WeakHit => "Weak hit".to_string(),
            Keyword::Crit => "Crit".to_string(),
            Keyword::Flanked => "Flanked".to_string(),
            Keyword::SpikesSurface => "spikes (surface)".to_string(),
        }
    }

    fn description(&self) -> String {
        match self {
            Keyword::Cond(condition, amount) => {
                if let Some(stacks) = amount {
                    cond_description_with_populated_stacks(*condition, *stacks)
                } else{
                    condition.description().to_string()
                }
            },
            Keyword::AttackEnhancement(enhancement) => format!("|<enhancement>| |<keyword>Enhancement| |<faded>(attack)|\n\n{}",  enhancement.description),
            Keyword::OnAttackedReaction(reaction) => format!("|<reaction>| |<keyword>Reaction| |<faded>(incoming attack)|\n\n{}", reaction.description),
            Keyword::OpportunityAttack => "|<reaction>| |<keyword>Reaction|\n\nAttack an enemy that drops their guard".to_string(),
            Keyword::Advantage => "Roll extra dice and take the highest / lowest result".to_string(),
            Keyword::Pushed => {
                "Distance: |<value>x| |<faded>(affected by target's |<toughness>||<stat>Toughness|<faded>)|\nOn collision: take |<value>1| damage per remaining distance.".to_string()
            }
            Keyword::WeakHit => {
                "|<value>-50%| damage or effectiveness.\nTriggers when |<mixed_dice>| outcome is |<value>5| or lower.".to_string()
            }
            Keyword::Crit => {
                "|<value>+75%| damage or effectiveness.\nTriggers when |<mixed_dice>| roll is |<value>20| (before modifiers).".to_string()
            }
            Keyword::Flanked => "Critical hit chance increased by |<value>20%|".to_string(),
            Keyword::SpikesSurface => "|<value>50%| slower movement and |<value>1| damage per step".to_string()
        }
    }

    fn icon(&self) -> Option<IconId> {
        let icon = match self {
            Keyword::AttackEnhancement(attack_enhancement) => attack_enhancement.icon,
            Keyword::OnAttackedReaction(on_attacked_reaction) => on_attacked_reaction.icon,
            Keyword::OpportunityAttack => IconId::MeleeAttack,
            _ => return None,
        };
        Some(icon)
    }

    fn goodness(&self) -> Goodness {
        match self {
            Keyword::Cond(condition, _amount) => {
                if condition.is_positive() {
                    Goodness::Good
                } else {
                    Goodness::Bad
                }
            }
            Keyword::AttackEnhancement(..) => Goodness::Neutral,
            Keyword::OnAttackedReaction(..) => Goodness::Neutral,
            Keyword::OpportunityAttack => Goodness::Neutral,
            Keyword::Advantage => Goodness::Neutral,
            Keyword::Pushed => Goodness::Bad,
            Keyword::WeakHit => Goodness::Bad,
            Keyword::Crit => Goodness::Good,
            Keyword::Flanked => Goodness::Bad,
            Keyword::SpikesSurface => Goodness::Bad,
        }
    }
}

pub fn cost_string(action_points: u32, stamina: u32, mana: u32) -> String {
    let mut s = String::new();

    if action_points > 0 {
        s.push_str(&format!("|<faded>{action_points}||<ap>|"));
    }
    if stamina > 0 {
        if !s.is_empty() {
            s.push_str(" ");
        }
        s.push_str(&format!("|<faded>{stamina}||<stamina>|"));
    }
    if mana > 0 {
        if !s.is_empty() {
            s.push_str(" ");
        }
        s.push_str(&format!("|<faded>{mana}||<mana>|"));
    }

    if !s.is_empty() {
        s = format!(" {s}");
    }
    s
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
        pos_preference,
        header,
        error,
        content_lines,
        &[],
        None,
        None,
    )
}

pub fn draw_tooltip(
    pos_preference: TooltipPositionPreference,
    header: &str,
    error: Option<&'static str>,
    content_lines: &[impl AsRef<str>],
    keywords: &[Keyword],
    header_keyword: Option<Keyword>,
    icon: Option<IconId>,
) -> Rect {
    let font = SIMPLE_FONT.get().unwrap();
    let header_font = BIG_FONT.get().unwrap();

    let header_font_size = if header_keyword.is_some() { 12 } else { 24 };
    let font_size = 16;
    let mut max_line_w = 0.0;
    let text_margin = 8.0;

    let goodness = if let Some(keyword) = header_keyword {
        keyword.goodness()
    } else {
        Goodness::Neutral
    };

    let header_status_icon = header_keyword.and_then(|keyword| match keyword {
        Keyword::Cond(condition, _amount) => Some(condition.status_id()),
        _ => None,
    });

    const STATUS_ICON_W: f32 = 20.0;
    const STATUS_ICON_MARGIN: f32 = 8.0;

    const ICON_MARGIN: f32 = 5.0;
    const ICON_SIZE: (f32, f32) = (30.0, 24.0);

    let mut measure_width = |line, font, size, status_icon: bool, icon: bool| {
        let dimensions = measure_text_with_font_tags(line, Some(font), size, 1.0);
        let mut w = dimensions.width;
        if status_icon {
            w += STATUS_ICON_W + STATUS_ICON_MARGIN;
        }

        if icon {
            w += ICON_MARGIN + ICON_SIZE.0;
        }

        if w > max_line_w {
            max_line_w = w;
        }
    };

    measure_width(
        header,
        header_font,
        header_font_size,
        header_status_icon.is_some(),
        icon.is_some(),
    );
    if let Some(error) = error.as_ref() {
        measure_width(error, font, font_size, false, false)
    }

    // The lines provided by the caller can be longer than desired, so we introduce line breaks here to limit
    // the width of the tooltip window.
    let line_len_limit = if header_keyword.is_some() { 30 } else { 40 };
    let mut physical_content_lines = vec![];
    // "|<example>hello|  |<hey>| yo"
    for line in content_lines {
        let mut line = line.as_ref();
        let mut keep_splitting = true;
        while keep_splitting {
            let mut inside_tagged_part = false;
            let mut inside_tag = false;
            let mut line_len = 0;
            keep_splitting = false;
            for (i, ch) in line.chars().enumerate() {
                if ch == '<' {
                    inside_tag = true;
                } else if ch == '>' {
                    inside_tag = false;
                    inside_tagged_part = true;
                } else if ch == '|' {
                    inside_tagged_part = false;
                } else if !inside_tag {
                    line_len += 1;
                    if line_len >= line_len_limit && !inside_tagged_part {
                        if let Some(whitespace_i) = line[i..].find(" ") {
                            let (left, right) = line.split_at(i + whitespace_i);
                            physical_content_lines.push(left);
                            line = &right[1..];
                            keep_splitting = true;
                        }
                        break;
                    }
                }
            }
        }

        physical_content_lines.push(line);
    }

    for line in &physical_content_lines {
        measure_width(line, font, font_size, false, false);
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
    let header_margin = 10.0;
    let tooltip_h = header_margin
        + num_real_lines as f32 * line_h
        + text_margin * 2.0
        + num_empty_lines as f32 * empty_line_h;

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
        Goodness::Good => Color::new(0.0, 0.2, 0.0, 0.9),
        Goodness::Neutral => Color::new(0.1, 0.1, 0.1, 0.95),
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

    let mut line_y = tooltip_rect.y + text_margin * 2.0 + 8.0;

    let mut draw_text_line =
        |line: &str, color: Option<Color>, is_header: bool, status_icon: Option<StatusId>| {
            let text_x = tooltip_rect.x + text_margin;
            let mut params = text_params.clone();
            if let Some(c) = color {
                params.color = c;
            }
            if is_header {
                params.font = Some(header_font);
                params.font_size = header_font_size;
            }
            if line.is_empty() {
                line_y += empty_line_h;
                0.0
            } else {
                let w = draw_text_with_font_tags(line, text_x, line_y, params, true);

                if let Some(status) = status_icon {
                    draw_status_icon(
                        status,
                        text_x + w + STATUS_ICON_MARGIN,
                        line_y - STATUS_ICON_W + 5.0,
                        Some((STATUS_ICON_W, STATUS_ICON_W)),
                    );
                }

                if is_header {
                    line_y += header_margin;
                    draw_line(
                        text_x,
                        line_y,
                        tooltip_rect.right() - text_margin,
                        line_y,
                        1.0,
                        Color::new(0.51, 0.51, 0.51, 0.6),
                    );
                }

                line_y += line_h;

                w
            }
        };

    let header_color = match header_keyword {
        None
        | Some(
            Keyword::AttackEnhancement(..)
            | Keyword::OnAttackedReaction(..)
            | Keyword::OpportunityAttack,
        ) => COL_TEXT_HEADER,
        _ => ORANGE,
    };

    draw_text_line(header, Some(header_color), true, header_status_icon);

    if let Some(error) = error {
        draw_text_line(error, Some(RED), false, None);
    }
    for line in physical_content_lines {
        draw_text_line(line, Some(COL_TEXT_REGULAR), false, None);
    }
    if !keywords.is_empty() {
        DID_DRAW_KEYWORD_TOOLTIP_THIS_FRAME.store(true, std::sync::atomic::Ordering::Relaxed);
        if !DID_DRAW_KEYWORD_TOOLTIP_LAST_FRAME.load(std::sync::atomic::Ordering::Relaxed) {
            // The keyword tooltip will be shown soon
            KEYWORD_TOOLTIP_COUNTER.store(0, std::sync::atomic::Ordering::Relaxed);
        }
        let elapsed_ms = (get_frame_time() * 1000.0) as u32;

        let shown_for =
            KEYWORD_TOOLTIP_COUNTER.fetch_add(elapsed_ms, std::sync::atomic::Ordering::Relaxed);
        if shown_for > 200 {
            draw_keyword_tooltips_relative_to_rect(keywords, tooltip_rect);
        }
    }

    if let Some(icon) = icon {
        draw_icon(
            icon,
            tooltip_rect.right() - ICON_SIZE.0 - ICON_MARGIN,
            y + ICON_MARGIN,
            None,
            false,
        );
    }

    tooltip_rect
}

pub static DID_DRAW_KEYWORD_TOOLTIP_LAST_FRAME: AtomicBool = AtomicBool::new(false);
pub static DID_DRAW_KEYWORD_TOOLTIP_THIS_FRAME: AtomicBool = AtomicBool::new(false);
pub static KEYWORD_TOOLTIP_COUNTER: AtomicU32 = AtomicU32::new(0);

pub fn handle_keyword_tooltip_end_of_frame() {
    let did_draw = DID_DRAW_KEYWORD_TOOLTIP_THIS_FRAME.load(std::sync::atomic::Ordering::Relaxed);
    DID_DRAW_KEYWORD_TOOLTIP_LAST_FRAME.store(did_draw, std::sync::atomic::Ordering::Relaxed);
    DID_DRAW_KEYWORD_TOOLTIP_THIS_FRAME.store(false, std::sync::atomic::Ordering::Relaxed);
}

pub fn draw_keyword_tooltips_relative_to_rect(keywords: &[Keyword], mut rect: Rect) {
    for (i, keyword) in keywords.iter().enumerate() {
        let pos_preference = if i == 0 {
            TooltipPositionPreference::RelativeToRect(rect, Side::Right)
        } else {
            TooltipPositionPreference::At((rect.x, rect.y + rect.h))
        };
        let description = keyword.description();
        let content_lines: Vec<&str> = description.split("\n").collect();

        rect = draw_tooltip(
            pos_preference,
            &keyword.name(),
            None,
            &content_lines,
            &[],
            Some(*keyword),
            keyword.icon(),
        );
        rect.y += 2.0;
        //y += rect.h;
    }
}

pub fn draw_keyword_tooltips(keywords: &[Keyword], x: f32, mut y: f32) {
    for keyword in keywords {
        let description = keyword.description();
        let content_lines: Vec<&str> = description.split("\n").collect();
        let rect = draw_tooltip(
            TooltipPositionPreference::At((x, y)),
            &keyword.name(),
            None,
            &content_lines,
            &[],
            Some(*keyword),
            None,
        );
        y += rect.h;
    }
}
