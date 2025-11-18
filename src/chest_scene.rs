use std::{collections::HashMap, rc::Rc};

use macroquad::{
    color::{Color, BLACK, BLUE, GRAY, LIGHTGRAY, WHITE, YELLOW},
    input::{is_mouse_button_pressed, mouse_position, MouseButton},
    math::Rect,
    miniquad::window::screen_size,
    shapes::{draw_rectangle, draw_rectangle_ex, draw_rectangle_lines, DrawRectangleParams},
    text::{measure_text, Font, TextParams},
    texture::{draw_texture_ex, DrawTextureParams, Texture2D},
    time::get_frame_time,
    window::{clear_background, next_frame},
};

use crate::{
    action_button::{draw_tooltip, TooltipPositionPreference},
    base_ui::{draw_text_rounded, Drawable},
    core::{Character, EquipmentEntry},
    data::{BONE_CRUSHER, ELUSIVE_BOW, HEALTH_POTION, LIGHT_CHAIN_MAIL, MANA_POTION},
    equipment_ui::equipment_tooltip_lines,
    non_combat_ui::{NonCombatCharacterUi, NonCombatPartyUi, PortraitRow},
    textures::{EquipmentIconId, IconId, PortraitId},
    util::select_n_random,
};

pub async fn run_chest_loop(
    player_characters: Vec<Character>,
    font: Font,
    equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
    icons: HashMap<IconId, Texture2D>,
    portrait_textures: &HashMap<PortraitId, Texture2D>,
) -> Vec<Character> {
    let characters: Vec<Rc<Character>> = player_characters.into_iter().map(Rc::new).collect();

    {
        let (screen_w, screen_h) = screen_size();
        let x_mid = screen_w / 2.0;

        let mut ui = NonCombatPartyUi::new(
            &characters[..],
            font.clone(),
            equipment_icons,
            icons.clone(),
            portrait_textures,
        );

        let transition_duration = 0.5;
        let mut transition_countdown = None;

        let candidate_items = vec![
            /*
            EquipmentEntry::Weapon(WAR_HAMMER),
            EquipmentEntry::Weapon(DAGGER),
            EquipmentEntry::Armor(LEATHER_ARMOR),
             */
            EquipmentEntry::Consumable(HEALTH_POTION),
            EquipmentEntry::Consumable(MANA_POTION),
            EquipmentEntry::Weapon(BONE_CRUSHER),
            EquipmentEntry::Weapon(ELUSIVE_BOW),
            EquipmentEntry::Armor(LIGHT_CHAIN_MAIL),
        ];

        let mut items: Vec<Option<EquipmentEntry>> = select_n_random(candidate_items, 1)
            .into_iter()
            .map(Some)
            .collect();

        let icon_margin = 140.0;
        let icon_w = 40.0;
        let row_w: f32 = items.len() as f32 * icon_w + (items.len() - 1) as f32 * icon_margin;

        loop {
            let elapsed = get_frame_time();

            clear_background(BLACK);
            ui.draw_and_handle_input();

            let text = "You find:";
            let font_size = 32;
            let text_dim = measure_text(text, Some(&font), font_size, 1.0);
            draw_text_rounded(
                text,
                screen_w / 2.0 - text_dim.width / 2.0,
                60.0 + (text_dim.height) / 2.0,
                TextParams {
                    font: Some(&font),
                    font_size,
                    color: WHITE,
                    ..Default::default()
                },
            );

            let mut icon_x = x_mid - row_w / 2.0;
            let icon_y = 150.0;

            let mut some_remaining_rewards = false;
            for item_slot in &mut items {
                let rect = Rect::new(icon_x, icon_y, icon_w, icon_w);

                if let Some(equipment_entry) = item_slot {
                    some_remaining_rewards = true;
                    draw_rectangle(rect.x, rect.y, rect.w, rect.h, BLUE);
                    let texture = &equipment_icons[&equipment_entry.icon()];
                    draw_texture_ex(
                        texture,
                        rect.x,
                        rect.y,
                        WHITE,
                        DrawTextureParams {
                            dest_size: Some(rect.size()),
                            ..Default::default()
                        },
                    );

                    let tooltip_lines = equipment_tooltip_lines(equipment_entry);
                    draw_tooltip(
                        &font,
                        TooltipPositionPreference::HorCenteredAt((
                            icon_x + icon_w / 2.0,
                            icon_y + 50.0,
                        )),
                        &tooltip_lines[0],
                        None,
                        &tooltip_lines[1..],
                    );

                    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, GRAY);

                    if rect.contains(mouse_position().into()) {
                        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 4.0, YELLOW);

                        if is_mouse_button_pressed(MouseButton::Left) {
                            let character = &characters[ui.selected_character_idx()];
                            let success = character.try_gain_equipment(*equipment_entry);
                            assert!(success);
                            *item_slot = None;
                        }
                    }
                } else {
                    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, GRAY);
                }

                icon_x += icon_w + icon_margin;
            }

            let text = if some_remaining_rewards {
                "Skip rewards"
            } else {
                "Proceed"
            };
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
