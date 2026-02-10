use std::cell::Cell;

use macroquad::{
    color::{Color, GOLD},
    miniquad::window::screen_size,
    shapes::draw_rectangle,
    text::{draw_text, draw_text_ex, measure_text, Font, TextParams},
    time::get_frame_time,
    window::{screen_height, screen_width},
};

pub struct Banner {
    inner: Option<_Banner>,
}

impl Banner {
    pub fn new() -> Self {
        Self { inner: None }
    }

    pub fn set(&mut self, text: &'static str, duration: f32) {
        self.inner = Some(_Banner {
            age: 0.0,
            duration,
            text,
        });
    }
}

struct _Banner {
    age: f32,
    duration: f32,
    text: &'static str,
}

impl Banner {
    pub fn draw(&mut self, font: &Font) {
        if let Some(banner) = &mut self.inner {
            let text = banner.text;
            let h = 60.0;
            let y_mid = screen_height() / 2.0;
            let x_mid = screen_width() / 2.0;

            let start_zoom_duration = 0.1;
            let font_scale = if banner.age < start_zoom_duration {
                3.0 - 2.0 * (banner.age / start_zoom_duration)
            } else {
                1.0
            };

            let font_size = 40;
            let text_dim = measure_text(text, Some(font), font_size, font_scale);

            let remaining = banner.duration - banner.age;

            let x_text;
            let end_scroll_duration = 0.3;
            if remaining < end_scroll_duration {
                x_text = screen_width()
                    * (0.5 + 0.5 * (end_scroll_duration - remaining) / end_scroll_duration);
            } else {
                x_text = x_mid - text_dim.width / 2.0;
            };

            let mut text_color = GOLD;

            let bg_alpha;
            let end_fade_duration = 0.2;
            if remaining < end_fade_duration {
                bg_alpha = (remaining / end_fade_duration) * 0.4;
                text_color.a = remaining / end_fade_duration;
            } else {
                bg_alpha = 0.4;
            }

            draw_rectangle(
                0.0,
                y_mid - h / 2.0,
                screen_width(),
                h,
                Color::new(0.0, 0.0, 0.0, bg_alpha),
            );

            draw_text_ex(
                text,
                x_text,
                y_mid + text_dim.offset_y / 2.0,
                TextParams {
                    font: Some(font),
                    font_size,
                    font_scale,
                    color: text_color,
                    ..Default::default()
                },
            );

            let t = get_frame_time();
            banner.age += t;
            if banner.age >= banner.duration {
                self.inner = None;
            }
        }
    }
}
