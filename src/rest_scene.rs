use std::{collections::HashMap, rc::Rc};

use macroquad::{
    color::{Color, BLACK, GRAY, LIGHTGRAY, WHITE, YELLOW},
    input::{is_mouse_button_pressed, mouse_position, MouseButton},
    math::Rect,
    miniquad::window::screen_size,
    shapes::{draw_rectangle, draw_rectangle_ex, DrawRectangleParams},
    text::{draw_text, measure_text, Font, TextParams},
    texture::Texture2D,
    time::get_frame_time,
    window::{clear_background, next_frame},
};

use crate::{
    base_ui::draw_text_rounded,
    core::Character,
    non_combat_ui::{NonCombatUi, PortraitRow},
    textures::{EquipmentIconId, IconId, PortraitId},
};

pub async fn run_rest_loop(
    mut player_characters: Vec<Character>,
    font: Font,
    equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
    icons: HashMap<IconId, Texture2D>,
    portrait_textures: &HashMap<PortraitId, Texture2D>,
) -> Vec<Character> {
    for character in &mut player_characters {
        character.health.gain(character.health.max() / 2);
        character.mana.set_to_max();
    }

    let characters: Vec<Rc<Character>> = player_characters.into_iter().map(Rc::new).collect();

    {
        let (screen_w, screen_h) = screen_size();

        let mut portrait_row = PortraitRow::new(&characters[..], portrait_textures);
        let mut bottom_panels: Vec<NonCombatUi> = characters
            .iter()
            .map(|character| {
                NonCombatUi::new(
                    character.clone(),
                    &font,
                    equipment_icons,
                    &icons,
                    portrait_textures,
                )
            })
            .collect();

        let transition_duration = 0.5;
        let mut transition_countdown = None;

        loop {
            let elapsed = get_frame_time();

            clear_background(BLACK);
            portrait_row.draw_and_handle_input();
            bottom_panels[portrait_row.selected_idx].draw_and_handle_input();
            bottom_panels[portrait_row.selected_idx].draw_tooltips();

            let text = "You regained 50% health and 100% mana";
            let font_size = 32;
            let text_dim = measure_text(text, Some(&font), font_size, 1.0);
            draw_text_rounded(
                text,
                screen_w / 2.0 - text_dim.width / 2.0,
                150.0 + (text_dim.height) / 2.0,
                TextParams {
                    font: Some(&font),
                    font_size,
                    color: WHITE,
                    ..Default::default()
                },
            );

            let text = "Proceed";
            let font_size = 30;
            let margin = 25.0;
            let padding = 15.0;
            let text_dim = measure_text(text, Some(&font), font_size, 1.0);
            let rect = Rect::new(
                screen_w - margin - text_dim.width - padding * 2.0,
                screen_h - 270.0 - margin - text_dim.height - padding * 2.0,
                text_dim.width + padding * 2.0,
                text_dim.height + padding * 2.0,
            );
            let rect_color = if rect.contains(mouse_position().into()) {
                LIGHTGRAY
            } else {
                GRAY
            };
            draw_rectangle(rect.x, rect.y, rect.w, rect.h, rect_color);
            draw_text_rounded(
                text,
                rect.x + padding,
                rect.y + padding + text_dim.offset_y,
                TextParams {
                    font: Some(&font),
                    font_size,
                    color: YELLOW,
                    ..Default::default()
                },
            );
            if rect.contains(mouse_position().into()) && is_mouse_button_pressed(MouseButton::Left)
            {
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
                    break;
                }
            }

            next_frame().await;
        }
    }

    characters
        .into_iter()
        .map(|character| Rc::into_inner(character).unwrap())
        .collect()
}
