
use macroquad::{
    color::{Color, BLACK, WHITE},
    input::{get_keys_pressed, is_mouse_button_pressed, MouseButton},
    miniquad::window::screen_size,
    shapes::{draw_rectangle_ex, DrawRectangleParams},
    text::{draw_text, measure_text, Font},
    time::get_frame_time,
    window::{clear_background, next_frame},
};


pub async fn run_rest_loop(font: Font) {
    let (screen_w, screen_h) = screen_size();
    let x_mid = screen_w / 2.0;

    let transition_duration = 0.5;
    let mut transition_countdown = None;

    loop {
        let elapsed = get_frame_time();

        clear_background(BLACK);

        let text = "You regained 50% health and 100% mana";
        let font_size = 32;
        let text_dim = measure_text(text, Some(&font), font_size, 1.0);
        draw_text(
            text,
            screen_w / 2.0 - text_dim.width / 2.0,
            150.0 + (text_dim.height) / 2.0,
            font_size.into(),
            WHITE,
        );

        if !get_keys_pressed().is_empty() || is_mouse_button_pressed(MouseButton::Left) {
            transition_countdown = Some(transition_duration);
        }

        // Transition to other scene
        if let Some(countdown) = &mut transition_countdown {
            let hypothenuse = (screen_w.powf(2.0) + screen_h.powf(2.0)).sqrt();
            let w = hypothenuse * (transition_duration - *countdown) / transition_duration;
            let color = Color::new(1.0, 0.5, 0.5, 0.3);
            let params = DrawRectangleParams {
                offset: Default::default(),
                rotation: 1.0,
                color,
            };
            draw_rectangle_ex(screen_w, -screen_h, w, screen_h + screen_w, params);

            *countdown -= elapsed;
            if *countdown < 0.0 {
                return;
            }
        }

        next_frame().await;
    }
}
