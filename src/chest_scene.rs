use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use macroquad::{
    color::{Color, BLACK, BLUE, DARKGRAY, GRAY, LIGHTGRAY, WHITE, YELLOW},
    input::{is_mouse_button_pressed, mouse_position, MouseButton},
    math::Rect,
    miniquad::window::screen_size,
    shapes::{draw_rectangle, draw_rectangle_ex, draw_rectangle_lines, DrawRectangleParams},
    text::{draw_text, draw_text_ex, measure_text, Font, TextParams},
    texture::{draw_texture_ex, DrawTextureParams, Texture2D},
    time::get_frame_time,
    window::{clear_background, next_frame},
};

use crate::{
    action_button::{draw_tooltip, TooltipPositionPreference},
    base_ui::{Drawable, TextLine},
    character_sheet::EquipmentSection,
    core::{Character, EquipmentEntry},
    data::{HEALTH_POTION, MANA_POTION},
    equipment_ui::{build_equipped_section, build_inventory_section, equipment_tooltip_lines},
    game_ui::{build_character_ui, ConfiguredAction, UiState},
    game_ui_components::CharacterSheetToggle,
    stats_ui::build_character_stats_table,
    textures::{EquipmentIconId, IconId, PortraitId},
    util::select_n_random,
};

pub async fn run_chest_loop(
    player_character: Character,
    font: Font,
    equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
    icons: HashMap<IconId, Texture2D>,
    portrait_textures: &HashMap<PortraitId, Texture2D>,
) -> Character {
    let (screen_w, screen_h) = screen_size();
    let x_mid = screen_w / 2.0;

    let transition_duration = 0.5;
    let mut transition_countdown = None;

    let event_queue = Rc::new(RefCell::new(vec![]));

    let character = Rc::new(player_character);

    let mut next_button_id = 0;
    let mut character_ui = build_character_ui(
        equipment_icons,
        &icons,
        &event_queue,
        &font,
        &character,
        &mut next_button_id,
    );

    let candidate_items = vec![
        /*
        EquipmentEntry::Weapon(WAR_HAMMER),
        EquipmentEntry::Weapon(DAGGER),
        EquipmentEntry::Armor(LEATHER_ARMOR),
         */
        EquipmentEntry::Consumable(HEALTH_POTION),
        EquipmentEntry::Consumable(MANA_POTION),
    ];

    let mut items: Vec<Option<EquipmentEntry>> = select_n_random(candidate_items, 1)
        .into_iter()
        .map(Some)
        .collect();

    let icon_margin = 140.0;
    let icon_w = 40.0;
    let row_w: f32 = items.len() as f32 * icon_w + (items.len() - 1) as f32 * icon_margin;

    let character_sheet_toggle = CharacterSheetToggle {
        shown: Cell::new(false),
        text_line: TextLine::new("Character sheet", 18, WHITE, Some(font.clone())),
        padding: 10.0,
    };

    let portrait = portrait_textures[&character.portrait].clone();

    let stats_table = build_character_stats_table(&font, &character);

    let mut equipment_section = EquipmentSection::new(&font, &character, equipment_icons.clone());

    /*
       let (inventory_section, equipment_slots) =
           build_inventory_section(&font, &character, equipment_icons);

       let (equipped_section, equipped_slots, equipment_stats_table) =
           build_equipped_section(&font, &character, equipment_icons);
    */

    let mut ui_state = UiState::ConfiguringAction(ConfiguredAction::ChangeEquipment { drag: None });

    loop {
        let elapsed = get_frame_time();

        clear_background(BLACK);

        stats_table.draw(10.0, 10.0); //TODO

        equipment_section.draw(10.0, 300.0);
        equipment_section.draw_tooltips(10.0, 300.0);
        equipment_section.handle_equipment_drag_and_consumption(&mut ui_state);

        let text = "You find:";
        let font_size = 32;
        let text_dim = measure_text(text, Some(&font), font_size, 1.0);
        draw_text(
            text,
            screen_w / 2.0 - text_dim.width / 2.0,
            60.0 + (text_dim.height) / 2.0,
            font_size.into(),
            WHITE,
        );

        let text = "Leave";
        let font_size = 30;
        let margin = 25.0;
        let padding = 15.0;
        let text_dim = measure_text(text, Some(&font), font_size, 1.0);
        let rect = Rect::new(
            screen_w - margin - text_dim.width - padding * 2.0,
            screen_h - margin - text_dim.height - padding * 2.0,
            text_dim.width + padding * 2.0,
            text_dim.height + padding * 2.0,
        );
        let rect_color = if rect.contains(mouse_position().into()) {
            LIGHTGRAY
        } else {
            GRAY
        };
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, rect_color);
        draw_text_ex(
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
        if rect.contains(mouse_position().into()) && is_mouse_button_pressed(MouseButton::Left) {
            transition_countdown = Some(transition_duration);
        }

        character_sheet_toggle.draw(x_mid - 60.0, 450.0);

        if character_sheet_toggle.shown.get() {
            let sheet_size = character_ui.character_sheet.container_size();
            let sheet_pos = (x_mid - sheet_size.0 / 2.0, 500.0);
            character_ui.character_sheet.screen_position.set(sheet_pos);
            let padding = 2.0;
            draw_rectangle(
                sheet_pos.0 - padding,
                sheet_pos.1 - padding,
                sheet_size.0 + padding * 2.0,
                sheet_size.1 + padding * 2.0,
                DARKGRAY,
            );
            character_ui.character_sheet.draw(&mut UiState::Idle);
        }

        let portrait_rect = Rect::new(40.0, 700.0, 64.0, 80.0);
        draw_rectangle_lines(
            portrait_rect.x,
            portrait_rect.y,
            portrait_rect.w,
            portrait_rect.h,
            1.0,
            GRAY,
        );
        draw_texture_ex(
            &portrait,
            portrait_rect.x,
            portrait_rect.y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(portrait_rect.size()),
                ..Default::default()
            },
        );
        character_ui
            .resource_bars
            .draw(10.0, portrait_rect.bottom() + 10.0);

        let mut icon_x = x_mid - row_w / 2.0;
        let icon_y = 150.0;

        for item_slot in &mut items {
            let rect = Rect::new(icon_x, icon_y, icon_w, icon_w);

            if let Some(equipment_entry) = item_slot {
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

        // Note: we have to drain the UI events, to prevent memory leak
        for event in event_queue.borrow_mut().drain(..) {
            dbg!(event);
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
                drop(character_ui); // Release Rc<Character> held by the UI, so that we can unwrap and return the inner value
                return Rc::into_inner(character).unwrap();
            }
        }

        next_frame().await;
    }
}
