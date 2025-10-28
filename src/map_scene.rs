use macroquad::{
    color::{BLACK, Color, GRAY, RED, WHITE, YELLOW}, input::{MouseButton, is_mouse_button_pressed, mouse_position}, math::{Rect, Vec2}, miniquad::window::screen_size, shapes::{DrawRectangleParams, draw_circle, draw_circle_lines, draw_line, draw_rectangle, draw_rectangle_ex, draw_rectangle_lines}, text::{Font, draw_text, measure_text}, time::get_frame_time, window::{clear_background, next_frame}
};

use crate::{drawing::draw_dashed_line};

struct Node {
    pos: (f32, f32),
    text: &'static str,
}

impl Node {
    fn new(pos: (f32, f32), text: &'static str) -> Self {
        Self { pos, text }
    }

    fn within_distance(&self, pos: (f32, f32), distance: f32) -> bool {
        (pos.0 - self.pos.0).powf(2.0) + (pos.1 - self.pos.1).powf(2.0) < distance.powf(2.0)
    }
}

pub async fn run_map_loop(font: Font) {
    let (screen_w, screen_h) = screen_size();
    let x0 = 300.0;
    let y_mid = screen_h / 2.0;
    let radius = 50.0;
    let mut selected_node_i = None;
    let margin = 25.0;
    let nodes = vec![
        Node::new((x0, y_mid - radius - margin), "Fight"),
        Node::new((x0, y_mid + radius + margin), "Ditto"),
    ];

    let transition_duration = 0.5;
    let mut transition_countdown = None;

    let start_size = 30.0;
    let start_pos = Rect::new(
        100.0 - start_size / 2.0,
        y_mid - start_size / 2.0,
        start_size,
        start_size,
    );

    loop {

        let elapsed = get_frame_time();

        let mouse_pos = mouse_position();
        clear_background(BLACK);

        let text = "Choose!";
        let font_size = 32;
        let text_dim = measure_text(text, Some(&font), font_size, 1.0);
        draw_text(
            text,
            screen_w / 2.0 - text_dim.width / 2.0,
            50.0 + (text_dim.height) / 2.0,
            font_size.into(),
            WHITE,
        );

        draw_rectangle_lines(start_pos.x, start_pos.y, start_size, start_size, 2.0, WHITE);
        let padding = 5.0;
        let cross_thickness = 3.0;
        draw_line(
            start_pos.x + padding,
            start_pos.y + padding,
            start_pos.right() - padding,
            start_pos.bottom() - padding,
            cross_thickness,
            RED,
        );
        draw_line(
            start_pos.x + padding,
            start_pos.bottom() - padding,
            start_pos.right() - padding,
            start_pos.y + padding,
            cross_thickness,
            RED,
        );

        for (node_i, node) in nodes.iter().enumerate() {
            let hovered = node.within_distance(mouse_pos, radius);

            let line_color = if hovered { WHITE } else { GRAY };
            draw_dashed_line(
                start_pos.center().into(),
                node.pos,
                2.0,
                line_color,
                15.0,
                None,
            );

             if transition_countdown.is_none() && is_mouse_button_pressed(MouseButton::Left) && hovered {
                selected_node_i = Some(node_i);
                transition_countdown = Some(transition_duration);
            }

            let outline_color = if selected_node_i == Some(node_i) {
                YELLOW
            } else if hovered {
                WHITE
            } else {
                GRAY
            };

            draw_circle(node.pos.0, node.pos.1, radius, BLACK);
            draw_circle_lines(node.pos.0, node.pos.1, radius, 2.0, outline_color);
            let font_size = 28;
            let text_dim = measure_text(node.text, Some(&font), font_size, 1.0);
            draw_text(
                node.text,
                node.pos.0 - text_dim.width / 2.0,
                node.pos.1 + (text_dim.height) / 2.0,
                font_size.into(),
                WHITE,
            );
        }

        // Transition to other scene
        if let Some(countdown) = &mut transition_countdown {
            let hypothenuse = (screen_w.powf(2.0) + screen_h.powf(2.0)).sqrt();
            let w = hypothenuse * (transition_duration - *countdown) / transition_duration;
            let color = Color::new(1.0, 0.5, 0.5, 0.3);
            let params = DrawRectangleParams { offset: Default::default(), rotation: 1.0, color};
            draw_rectangle_ex(screen_w, -screen_h, w, screen_h + screen_w,  params);

            *countdown -= elapsed;
            if *countdown < 0.0 {
                return;
            }
        }

        next_frame().await;
    }
}
