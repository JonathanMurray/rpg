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
        _ => unreachable!(),
    }
}

pub fn draw_dashed_line(from: (f32, f32), to: (f32, f32), thickness: f32, color: Color) {
    let len = ((to.0 - from.0).powf(2.0) + (to.1 - from.1).powf(2.0)).sqrt();
    let n = (len / 5.0) as u32;
    let (mut prev_x, mut prev_y) = from;
    for i in 0..n {
        let x = from.0 + (to.0 - from.0) * i as f32 / n as f32;
        let y = from.1 + (to.1 - from.1) * i as f32 / n as f32;
        if i % 2 == 0 {
            draw_line(prev_x, prev_y, x, y, thickness, color);
        }
        prev_x = x;
        prev_y = y;
    }
    draw_line(prev_x, prev_y, to.0, to.1, thickness, color);
}
