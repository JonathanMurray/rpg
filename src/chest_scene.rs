use macroquad::{
    color::{Color, BLACK, GRAY, WHITE, YELLOW},
    input::{is_mouse_button_pressed, mouse_position, MouseButton},
    math::Rect,
    miniquad::window::screen_size,
    shapes::{
        draw_rectangle_ex, draw_rectangle_lines,
        DrawRectangleParams,
    },
    text::{draw_text, measure_text, Font},
    time::get_frame_time,
    window::{clear_background, next_frame},
};

use crate::core::EquipmentEntry;

pub async fn run_chest_loop(font: Font, reward: EquipmentEntry) {
    let (screen_w, screen_h) = screen_size();
    let y_mid = screen_h / 2.0;
    let x_mid = screen_w / 2.0;

    let reward_text = reward.name();
    let reward_size = 100.0;
    let reward_rect = Rect::new(
        x_mid - reward_size / 2.0,
        y_mid - reward_size / 2.0,
        reward_size,
        reward_size,
    );

    let transition_duration = 0.5;
    let mut transition_countdown = None;

    loop {
        let elapsed = get_frame_time();

        let mouse_pos = mouse_position();
        clear_background(BLACK);

        let text = "Claim your reward!";
        let font_size = 32;
        let text_dim = measure_text(text, Some(&font), font_size, 1.0);
        draw_text(
            text,
            screen_w / 2.0 - text_dim.width / 2.0,
            50.0 + (text_dim.height) / 2.0,
            font_size.into(),
            WHITE,
        );

        let hovered = reward_rect.contains(mouse_pos.into());

        if transition_countdown.is_none() && is_mouse_button_pressed(MouseButton::Left) && hovered {
            transition_countdown = Some(transition_duration);
        }

        let outline_color = if transition_countdown.is_some() {
            YELLOW
        } else if hovered {
            WHITE
        } else {
            GRAY
        };

        draw_rectangle_lines(
            reward_rect.x,
            reward_rect.y,
            reward_rect.w,
            reward_rect.h,
            2.0,
            outline_color,
        );
        let font_size = 28;
        let text_dim = measure_text(reward_text, Some(&font), font_size, 1.0);
        draw_text(
            reward_text,
            reward_rect.center().x - text_dim.width / 2.0,
            reward_rect.center().y + (text_dim.height) / 2.0,
            font_size.into(),
            WHITE,
        );

        // Transition to other scene
        if let Some(countdown) = &mut transition_countdown {
            let hypothenuse = (screen_w.powf(2.0) + screen_h.powf(2.0)).sqrt();
            let w = hypothenuse * (transition_duration - *countdown) / transition_duration;
            let color = Color::new(0.5, 1.0, 0.5, 0.2);
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
