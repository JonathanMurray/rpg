use std::{collections::HashMap, rc::Rc};

use macroquad::{
    color::{Color, BLACK, BLUE, DARKGRAY, GRAY, LIGHTGRAY, RED, WHITE, YELLOW},
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
    action_button::{draw_keyword_tooltips, draw_regular_tooltip, TooltipPositionPreference},
    base_ui::{draw_text_rounded, Drawable, TextLine},
    core::{ArrowStack, Character, EquipmentEntry, Party},
    data::{
        ADRENALIN_POTION, ARCANE_POTION, BARBED_ARROWS, BOW, CHAIN_MAIL, COLD_ARROWS, DAGGER,
        ENERGY_POTION, EXPLODING_ARROWS, HEALTH_POTION, LEATHER_ARMOR, MANA_POTION, MEDIUM_SHIELD,
        PENETRATING_ARROWS, RAPIER, SMALL_SHIELD, SWORD, WAR_HAMMER,
    },
    equipment_ui::equipment_tooltip,
    non_combat_ui::NonCombatPartyUi,
    sounds::SoundPlayer,
    textures::{EquipmentIconId, IconId, PortraitId},
    util::select_n_random,
};

pub async fn run_shop_loop(
    player_characters: Vec<Character>,
    font: Font,
    equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
    icons: HashMap<IconId, Texture2D>,
    portrait_textures: &HashMap<PortraitId, Texture2D>,
    party: &Party,
    entries: &mut Vec<ShopEntry>,
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
        let row_w: f32 = entries.len() as f32 * icon_w + (entries.len() - 1) as f32 * icon_margin;

        loop {
            let elapsed = get_frame_time();

            clear_background(BLACK);

            ui.draw_and_handle_input();

            let text = "Buy something?";
            let font_size = 32;
            let text_dim = measure_text(text, Some(&font), font_size, 1.0);
            draw_text_rounded(
                text,
                screen_w / 2.0 - text_dim.width / 2.0,
                40.0 + (text_dim.height) / 2.0,
                TextParams {
                    font: Some(&font),
                    font_size,
                    color: WHITE,
                    ..Default::default()
                },
            );

            let text = "Leave shop";
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

            let mut icon_x = x_mid - row_w / 2.0;
            let icon_y = 150.0;

            let mut hovered = None;

            for entry in entries.iter_mut() {
                let rect = Rect::new(icon_x, icon_y, icon_w, icon_w);

                if !entry.has_been_bought {
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

                    let character = &characters[ui.selected_character_idx()];

                    let can_afford = party.money.get() >= entry.cost;

                    let cost_color = if can_afford { WHITE } else { RED };
                    let text = format!("{}", entry.cost);
                    let font_size = 24;
                    let text_dim = measure_text(&text, Some(&font), font_size, 1.0);
                    TextLine::new(
                        format!("{}", entry.cost),
                        font_size,
                        cost_color,
                        Some(font.clone()),
                    )
                    .with_depth(DARKGRAY, 1.0)
                    .draw(icon_x + icon_w / 2.0 - text_dim.width / 2.0, icon_y - 22.0);

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
                        let has_space = character.has_space_in_inventory();
                        if can_afford && has_space {
                            draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 4.0, YELLOW);

                            if is_mouse_button_pressed(MouseButton::Left) {
                                let success = character.try_gain_equipment(entry.item);
                                assert!(success);
                                party.spend_money(entry.cost);
                                entry.has_been_bought = true;
                            }
                        } else {
                            draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 2.0, RED);
                        }

                        hovered = Some((tooltip, tooltip_rect));
                    }
                } else {
                    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, GRAY);
                }

                icon_x += icon_w + icon_margin;
            }

            if let Some((tooltip, rect)) = hovered {
                if !tooltip.keywords.is_empty() {
                    draw_keyword_tooltips(&font, &tooltip.keywords, rect.x, rect.bottom() + 2.0);
                }
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

pub fn generate_shop_contents() -> Vec<ShopEntry> {
    let candidate_items = vec![
        (EquipmentEntry::Weapon(WAR_HAMMER), 3),
        (EquipmentEntry::Weapon(DAGGER), 3),
        (EquipmentEntry::Weapon(SWORD), 8),
        (EquipmentEntry::Weapon(RAPIER), 8),
        (EquipmentEntry::Weapon(BOW), 11),
        (EquipmentEntry::Armor(LEATHER_ARMOR), 4),
        (EquipmentEntry::Armor(CHAIN_MAIL), 12),
        (EquipmentEntry::Shield(SMALL_SHIELD), 5),
        (EquipmentEntry::Shield(MEDIUM_SHIELD), 5),
        (EquipmentEntry::Consumable(HEALTH_POTION), 4),
        (EquipmentEntry::Consumable(MANA_POTION), 4),
        (EquipmentEntry::Consumable(ADRENALIN_POTION), 6),
        (EquipmentEntry::Consumable(ENERGY_POTION), 6),
        (EquipmentEntry::Consumable(ARCANE_POTION), 4),
        (
            EquipmentEntry::Arrows(ArrowStack::new(PENETRATING_ARROWS, 3)),
            4,
        ),
        (EquipmentEntry::Arrows(ArrowStack::new(BARBED_ARROWS, 3)), 4),
        (EquipmentEntry::Arrows(ArrowStack::new(COLD_ARROWS, 3)), 4),
        (
            EquipmentEntry::Arrows(ArrowStack::new(EXPLODING_ARROWS, 3)),
            4,
        ),
    ];

    select_n_random(candidate_items, 5)
        .into_iter()
        .map(|(item, cost)| ShopEntry {
            item,
            cost,
            has_been_bought: false,
        })
        .collect()
}

#[derive(Debug, Copy, Clone)]
pub struct ShopEntry {
    item: EquipmentEntry,
    cost: u32,
    has_been_bought: bool,
}
