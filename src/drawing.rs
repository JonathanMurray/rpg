use macroquad::{color::Color, shapes::draw_line};

pub fn draw_arrow((x, y): (f32, f32), width: f32, direction: (i32, i32), color: Color) {
    let w = x;
    let n = y;
    let e = w + width;
    let s = n + width;
    let mid = (w + width * 0.5, n + width * 0.5);

    let space = width * 0.3;
    let thickness = 2.0;

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
) {
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
    draw_dashed_line((x, y), (x + w, y), thickness, color, segment_len);

    // right
    draw_dashed_line((x + w, y), (x + w, y + h), thickness, color, segment_len);

    // bottom
    draw_dashed_line((x + w, y + h), (x, y + h), thickness, color, segment_len);

    // left
    draw_dashed_line((x, y + h), (x, y), thickness, color, segment_len);
}
