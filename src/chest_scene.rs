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
use rand::Rng;

use crate::{
    action_button::{
        draw_keyword_tooltips, draw_regular_tooltip, TooltipPositionPreference,
    },
    base_ui::{draw_text_rounded, Drawable},
    core::{ArrowStack, Character, EquipmentEntry},
    data::{
        ADRENALIN_POTION, ARCANE_POTION, BARBED_ARROWS, CHAIN_MAIL, ENERGY_POTION,
        MEDIUM_SHIELD, PENETRATING_ARROWS,
    },
    equipment_ui::equipment_tooltip,
    non_combat_ui::NonCombatPartyUi,
    sounds::SoundPlayer,
    textures::{EquipmentIconId, IconId, PortraitId},
};

pub async fn run_chest_loop(
    player_characters: Vec<Character>,
    font: Font,
    equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
    icons: HashMap<IconId, Texture2D>,
    portrait_textures: &HashMap<PortraitId, Texture2D>,
    items: &mut Vec<ChestEntry>,
) -> Vec<Character> {
    let characters: Vec<Rc<Character>> = player_characters.into_iter().map(Rc::new).collect();

    let sound_player = SoundPlayer::new().await;

    {
        let (screen_w, screen_h) = screen_size();
        let x_mid = screen_w / 2.0;

        let mut ui = NonCombatPartyUi::new(
            &characters[..],
            font.clone(),
            equipment_icons,
            icons.clone(),
            portrait_textures,
            sound_player,
        );

        let transition_duration = 0.5;
        let mut transition_countdown = None;

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
            for entry in items.iter_mut() {
                let rect = Rect::new(icon_x, icon_y, icon_w, icon_w);

                if !entry.has_been_grabbed {
                    some_remaining_rewards = true;
                    draw_rectangle(rect.x, rect.y, rect.w, rect.h, BLUE);
                    let texture = &equipment_icons[&entry.item.icon()];
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

                    let tooltip = equipment_tooltip(&entry.item);
                    let tooltip_rect = draw_regular_tooltip(
                        &font,
                        TooltipPositionPreference::HorCenteredAt((
                            icon_x + icon_w / 2.0,
                            icon_y + 50.0,
                        )),
                        &tooltip.header,
                        None,
                        &tooltip.technical_description,
                    );

                    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, GRAY);

                    if rect.contains(mouse_position().into()) {
                        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 4.0, YELLOW);

                        if !tooltip.keywords.is_empty() {
                            draw_keyword_tooltips(
                                &font,
                                &tooltip.keywords,
                                tooltip_rect.right() + 2.0,
                                tooltip_rect.y,
                            );
                        }

                        if is_mouse_button_pressed(MouseButton::Left) {
                            let character = &characters[ui.selected_character_idx()];
                            let success = character.try_gain_equipment(entry.item);
                            assert!(success);
                            entry.has_been_grabbed = true;
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

#[derive(Clone, Debug)]
pub struct ChestEntry {
    item: EquipmentEntry,
    has_been_grabbed: bool,
}

pub fn generate_chest_content() -> Vec<ChestEntry> {
    let candidate_chest_rewards = vec![
        EquipmentEntry::Armor(CHAIN_MAIL),
        EquipmentEntry::Shield(MEDIUM_SHIELD),
        EquipmentEntry::Arrows(ArrowStack::new(PENETRATING_ARROWS, 3)),
        EquipmentEntry::Arrows(ArrowStack::new(BARBED_ARROWS, 3)),
        EquipmentEntry::Consumable(ARCANE_POTION),
        EquipmentEntry::Consumable(ENERGY_POTION),
        EquipmentEntry::Consumable(ADRENALIN_POTION),
    ];
    let mut rng = rand::rng();
    vec![ChestEntry {
        item: candidate_chest_rewards[rng.random_range(..candidate_chest_rewards.len())],
        has_been_grabbed: false,
    }]
}
