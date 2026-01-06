use macroquad::{
    color::{Color, WHITE},
    shapes::{draw_circle_lines, draw_line, draw_rectangle, draw_rectangle_lines, draw_triangle},
    time::get_time,
};

pub fn draw_arrow(
    (x, y): (f32, f32),
    width: f32,
    direction: (i32, i32),
    color: Color,
    thickness: f32,
) {
    let w = x;
    let n = y;
    let e = w + width;
    let s = n + width;
    let mid = (w + width * 0.5, n + width * 0.5);

    let space = width * 0.3;

    let direction = (direction.0.signum(), direction.1.signum());

    match direction {
        (1, 0) => {
            draw_line(w + space, n + space, mid.0, mid.1, thickness, color);
            draw_line(w + space, s - space, mid.0, mid.1, thickness, color);
        }
        (1, 1) => {
            draw_line(mid.0, n + space / 1.4, mid.0, mid.1, thickness, color);
            draw_line(w + space / 1.4, mid.1, mid.0, mid.1, thickness, color);
        }
        (0, 1) => {
            draw_line(w + space, n + space, mid.0, mid.1, thickness, color);
            draw_line(e - space, n + space, mid.0, mid.1, thickness, color);
        }
        (-1, 1) => {
            draw_line(mid.0, n + space / 1.4, mid.0, mid.1, thickness, color);
            draw_line(e - space / 1.4, mid.1, mid.0, mid.1, thickness, color);
        }
        (-1, 0) => {
            draw_line(e - space, n + space, mid.0, mid.1, thickness, color);
            draw_line(e - space, s - space, mid.0, mid.1, thickness, color);
        }
        (-1, -1) => {
            draw_line(mid.0, s - space / 1.4, mid.0, mid.1, thickness, color);
            draw_line(e - space / 1.4, mid.1, mid.0, mid.1, thickness, color);
        }
        (0, -1) => {
            draw_line(w + space, s - space, mid.0, mid.1, thickness, color);
            draw_line(e - space, s - space, mid.0, mid.1, thickness, color);
        }
        (1, -1) => {
            draw_line(mid.0, s - space / 1.4, mid.0, mid.1, thickness, color);
            draw_line(w + space / 1.4, mid.1, mid.0, mid.1, thickness, color);
        }
        unhandled => panic!("Unhandled: {:?}", unhandled),
    }
}

pub fn draw_dashed_line(
    from: (f32, f32),
    to: (f32, f32),
    thickness: f32,
    color: Color,
    segment_len: f32,
    depth: Option<(Color, f32)>,
    animated: bool,
) {
    draw_dashed_line_ex(
        from,
        to,
        thickness,
        color,
        segment_len,
        depth,
        None,
        animated,
    );
}

pub fn draw_dashed_line_ex(
    from: (f32, f32),
    to: (f32, f32),
    thickness: f32,
    color: Color,
    segment_len: f32,
    depth: Option<(Color, f32)>,
    trim_start_and_end: Option<f32>,
    animated: bool,
) {
    if let Some((color, offset)) = depth {
        draw_dashed_line_ex(
            (from.0 + offset, from.1 + offset),
            (to.0 + offset, to.1 + offset),
            thickness,
            color,
            segment_len,
            None,
            trim_start_and_end,
            animated,
        );
    }

    let start_offset = if animated {
        let game_time = get_time();
        (game_time - game_time.floor()) as f32 * segment_len * 2.0
    } else {
        0.0
    };

    let line_len = ((to.0 - from.0).powf(2.0) + (to.1 - from.1).powf(2.0)).sqrt();
    // "Segments" alternate between "drawn" and "skipped over" to create the dash effect
    let num_segments = (line_len / segment_len) as u32;
    let dx = (to.0 - from.0) / num_segments as f32;
    let dy = (to.1 - from.1) / num_segments as f32;

    let from = (
        from.0 + (to.0 - from.0) * (start_offset / line_len),
        from.1 + (to.1 - from.1) * (start_offset / line_len),
    );

    let draw_dash = |start: (f32, f32), end: (f32, f32)| {
        let mut skip = false;
        if let Some(trim) = trim_start_and_end {
            if (start.0 - from.0).abs() < trim && (start.1 - from.1).abs() < trim {
                skip = true;
            }
            if (end.0 - to.0).abs() < trim && (end.1 - to.1).abs() < trim {
                skip = true;
            }
        }
        if !skip {
            draw_line(start.0, start.1, end.0, end.1, thickness, color);
        }
    };

    let (mut prev_x, mut prev_y) = from;
    for i in 0..num_segments {
        let x = from.0 + dx * i as f32;
        let y = from.1 + dy * i as f32;
        if i % 2 == 0 {
            draw_dash((prev_x, prev_y), (x, y));
        }
        prev_x = x;
        prev_y = y;
    }

    draw_dash((prev_x, prev_y), to);
}

pub fn draw_dashed_rectangle_lines(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    thickness: f32,
    color: Color,
    segment_len: f32,
) {
    // top
    draw_dashed_line(
        (x, y),
        (x + w, y),
        thickness,
        color,
        segment_len,
        None,
        false,
    );

    // right
    draw_dashed_line(
        (x + w, y),
        (x + w, y + h),
        thickness,
        color,
        segment_len,
        None,
        false,
    );

    // bottom
    draw_dashed_line(
        (x + w, y + h),
        (x, y + h),
        thickness,
        color,
        segment_len,
        None,
        false,
    );

    // left
    draw_dashed_line(
        (x, y + h),
        (x, y),
        thickness,
        color,
        segment_len,
        None,
        false,
    );
}

pub fn draw_cornered_rectangle_lines(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    thickness: f32,
    color: Color,
    margin: f32,
    len: f32,
) {
    let left = (x + margin).floor();
    let top = (y + margin).floor();
    let right = (x + w - margin).floor();
    let bot = (y + h - margin).floor();

    draw_line(left, top, left, top + len, thickness, color);
    draw_line(left, top, left + len, top, thickness, color);
    draw_line(right - len, top, right, top, thickness, color);
    draw_line(right, top, right, top + len, thickness, color);
    draw_line(right, bot - len, right, bot, thickness, color);
    draw_line(right - len, bot, right, bot, thickness, color);
    draw_line(left, bot, left + len, bot, thickness, color);
    draw_line(left, bot, left, bot - len, thickness, color);
}

pub fn draw_dashed_rectangle_sides(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    thickness: f32,
    color: Color,
    segment_len: f32,
    left: bool,
    right: bool,
    top: bool,
    bottom: bool,
) {
    if left {
        draw_dashed_line(
            (x, y),
            (x, y + h),
            thickness,
            color,
            segment_len,
            None,
            false,
        );
    }
    if right {
        draw_dashed_line(
            (x + w, y),
            (x + w, y + h),
            thickness,
            color,
            segment_len,
            None,
            false,
        );
    }
    if top {
        draw_dashed_line(
            (x, y),
            (x + w, y),
            thickness,
            color,
            segment_len,
            None,
            false,
        );
    }
    if bottom {
        draw_dashed_line(
            (x, y + h),
            (x + w, y + h),
            thickness,
            color,
            segment_len,
            None,
            false,
        );
    }
}

pub fn draw_rounded_rectangle_lines(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    thickness: f32,
    color: Color,
    inner_rounding: f32,
    outer: Option<(Color, f32)>,
) {
    draw_rectangle_lines(x, y, w, h, thickness, color);

    let mut passes = vec![(color, inner_rounding)];
    if let Some((outer_color, outer_rounding)) = outer {
        passes.push((outer_color, outer_rounding));
    }

    for (color, rounding) in passes {
        draw_triangle(
            (x, y).into(),
            (x + rounding, y).into(),
            (x, y + rounding).into(),
            color,
        );
        draw_triangle(
            (x + w, y).into(),
            (x + w - rounding, y).into(),
            (x + w, y + rounding).into(),
            color,
        );
        draw_triangle(
            (x, y + h).into(),
            (x, y + h - rounding).into(),
            (x + rounding, y + h).into(),
            color,
        );
        draw_triangle(
            (x + w, y + h).into(),
            (x + w - rounding, y + h).into(),
            (x + w, y + h - rounding).into(),
            color,
        );
    }
}

pub fn draw_crosshair((x, y): (f32, f32), r: f32, color: Color) {
    let x = x.floor();
    let y = y.floor();
    draw_circle_lines(x, y, r, 3.0, color);
    let len = r * 1.7;
    let space = 4.0;
    draw_line(x - len, y, x - space, y, 2.0, color);
    draw_line(x + space, y, x + len, y, 2.0, color);

    draw_line(x, y - len, x, y - space, 2.0, color);
    draw_line(x, y + space, x, y + len, 2.0, color);

    draw_rectangle(x - 1.0, y - 1.0, 3.0, 3.0, color);
}

pub fn draw_cross(x: f32, y: f32, w: f32, h: f32, color: Color, thickness: f32, margin: f32) {
    let left = x + margin;
    let right = x + w - margin;
    let top = y + margin;
    let bot = y + h - margin;

    draw_line(left, top, right, bot, thickness, color);
    draw_line(left, bot, right, top, thickness, color);
}
