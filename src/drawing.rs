use macroquad::{
    color::Color,
    shapes::{draw_circle_lines, draw_line},
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
) {
    if let Some((color, offset)) = depth {
        draw_dashed_line(
            (from.0 + offset, from.1 + offset),
            (to.0 + offset, to.1 + offset),
            thickness,
            color,
            segment_len,
            None,
        );
    }

    let line_len = ((to.0 - from.0).powf(2.0) + (to.1 - from.1).powf(2.0)).sqrt();

    // "Segments" alternate between "drawn" and "skipped over" to create the dash effect
    let num_segments = (line_len / segment_len) as u32;
    //let num_dashes = (len / 2.0) as u32;
    //let num_segments = 8;
    let (mut prev_x, mut prev_y) = from;
    for i in 0..num_segments {
        let x = from.0 + (to.0 - from.0) * i as f32 / num_segments as f32;
        let y = from.1 + (to.1 - from.1) * i as f32 / num_segments as f32;
        if i % 2 == 0 {
            draw_line(prev_x, prev_y, x, y, thickness, color);
        }
        prev_x = x;
        prev_y = y;
    }
    draw_line(prev_x, prev_y, to.0, to.1, thickness, color);
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
    draw_dashed_line((x, y), (x + w, y), thickness, color, segment_len, None);

    // right
    draw_dashed_line(
        (x + w, y),
        (x + w, y + h),
        thickness,
        color,
        segment_len,
        None,
    );

    // bottom
    draw_dashed_line(
        (x + w, y + h),
        (x, y + h),
        thickness,
        color,
        segment_len,
        None,
    );

    // left
    draw_dashed_line((x, y + h), (x, y), thickness, color, segment_len, None);
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
    let left = x + margin;
    let top = y + margin;
    let right = x + w - margin;
    let bot = y + h - margin;

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
        draw_dashed_line((x, y), (x, y + h), thickness, color, segment_len, None);
    }
    if right {
        draw_dashed_line(
            (x + w, y),
            (x + w, y + h),
            thickness,
            color,
            segment_len,
            None,
        );
    }
    if top {
        draw_dashed_line((x, y), (x + w, y), thickness, color, segment_len, None);
    }
    if bottom {
        draw_dashed_line(
            (x, y + h),
            (x + w, y + h),
            thickness,
            color,
            segment_len,
            None,
        );
    }
}

pub fn draw_crosshair((x, y): (f32, f32), width: f32, crosshair_color: Color) {
    draw_circle_lines(
        x + width / 2.0,
        y + width / 2.0,
        width * 0.15,
        3.0,
        crosshair_color,
    );
    draw_arrow((x, y), width, (1, 1), crosshair_color, 2.0);
    draw_arrow((x, y), width, (-1, -1), crosshair_color, 2.0);
}

pub fn draw_cross(x: f32, y: f32, w: f32, h: f32, color: Color, thickness: f32, margin: f32) {
    let left = x + margin;
    let right = x + w - margin;
    let top = y + margin;
    let bot = y + h - margin;

    draw_line(left, top, right, bot, thickness, color);
    draw_line(left, bot, right, top, thickness, color);
}
