use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
};

use macroquad::{
    color::{Color, BLACK, BLUE, GRAY, LIGHTGRAY, WHITE, YELLOW},
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
    action_button::{
        ActionButton, ButtonAction, InternalUiEvent, TooltipPositionPreference, draw_button_tooltip, draw_tooltip
    },
    base_ui::{Align, Container, Drawable, Element, LayoutDirection, Style},
    character_sheet::build_spell_book,
    core::{BaseAction, Character, EquipmentEntry, HandType},
    data::{HEALTH_POTION, MANA_POTION},
    equipment_ui::{EquipmentSection, equipment_tooltip_lines},
    game_ui::ResourceBars,
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
    let character = Rc::new(player_character);

    {
        let (screen_w, screen_h) = screen_size();
        let x_mid = screen_w / 2.0;

        let transition_duration = 0.5;
        let mut transition_countdown = None;

        let event_queue = Rc::new(RefCell::new(vec![]));

        let resource_bars = ResourceBars::new(&character, &font);

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

        let portrait = portrait_textures[&character.portrait].clone();

        let mut next_button_id = 0;

        let mut new_button = |btn_action, character: Option<Rc<Character>>, enabled: bool| {
            let btn =
                ActionButton::new(btn_action, &event_queue, next_button_id, &icons, character);
            btn.enabled.set(enabled);
            next_button_id += 1;
            btn
        };

        let mut hovered_button = None;

        let mut hoverable_buttons = vec![];
        let mut attack_button = None;
        let mut spell_buttons = vec![];
        let mut attack_enhancement_buttons = vec![];

        for action in character.known_actions() {
            let btn_action = ButtonAction::Action(action);
            let btn = Rc::new(new_button(btn_action, Some(character.clone()), false));
            hoverable_buttons.push(Rc::clone(&btn));
            match action {
                BaseAction::Attack { .. } => {
                    attack_button = Some(btn.clone());
                }
                BaseAction::CastSpell(spell) => {
                    let enhancement_buttons: Vec<Rc<ActionButton>> = spell
                        .possible_enhancements
                        .iter()
                        .filter_map(|maybe_enhancement| *maybe_enhancement)
                        .filter_map(|enhancement| {
                            if character.knows_spell_enhancement(enhancement) {
                                let enhancement_btn = Rc::new(new_button(
                                    ButtonAction::SpellEnhancement(enhancement),
                                    None,
                                    false,
                                ));
                                hoverable_buttons.push(Rc::clone(&enhancement_btn));
                                Some(enhancement_btn)
                            } else {
                                None
                            }
                        })
                        .collect();
                    spell_buttons.push((btn.clone(), enhancement_buttons));
                }
                _ => {}
            }
        }

        let mut reaction_buttons = vec![];
        for (_subtext, reaction) in character.known_on_attacked_reactions() {
            let btn_action = ButtonAction::OnAttackedReaction(reaction);
            let btn = Rc::new(new_button(btn_action, None, false));
            hoverable_buttons.push(Rc::clone(&btn));
            reaction_buttons.push(btn);
        }
        for (_subtext, reaction) in character.known_on_hit_reactions() {
            let btn_action = ButtonAction::OnHitReaction(reaction);
            let btn = Rc::new(new_button(btn_action, None, false));
            hoverable_buttons.push(Rc::clone(&btn));
            reaction_buttons.push(btn);
        }

        // TODO: Only include inherently known enhancements here; not those gained from weapons (since weapons can be unequipped
        // without the character sheet being updated)
        for (_subtext, enhancement) in character.known_attack_enhancements(HandType::MainHand) {
            let btn_action = ButtonAction::AttackEnhancement(enhancement);
            let btn = Rc::new(new_button(btn_action, None, false));
            hoverable_buttons.push(Rc::clone(&btn));
            attack_enhancement_buttons.push(btn);
        }

        let spell_book = build_spell_book(
            &font,
            attack_button,
            reaction_buttons,
            attack_enhancement_buttons,
            spell_buttons,
        );

        let stats_table = build_character_stats_table(&font, &character);

        let equipment_section = Rc::new(RefCell::new(EquipmentSection::new(
            &font,
            &character,
            equipment_icons.clone(),
        )));

        let equipment_changed = character.listen_to_changed_equipment();
        let mut equipment_drag = None;

        //
        // ---------------------------
        // TODO:
        //
        // Extract the bottom_panel UI to a new module, and re-use it across all non-combat scenes
        //
        // "NonFightCharacterUi" ? 
        //
        // ---------------------------




        let bottom_panel = Element::Container(Container {
            layout_dir: LayoutDirection::Horizontal,
               style: Style {
                background_color: Some(Color::new(0.00, 0.3, 0.4, 1.00)),
                padding: 10.0,
                ..Default::default()
            },
            min_width: Some(screen_w),
            children: vec![
                Element::Container(Container {
                    layout_dir: LayoutDirection::Vertical,
                    children: vec![
                        Element::Texture(portrait, Some((64.0, 80.0))),
                        Element::Container(resource_bars.container),
                    ],
                    align: Align::Center,
                    margin: 5.0,
                    ..Default::default()
                }),
                Element::Container(spell_book),
                stats_table,
                Element::RcRefCell(equipment_section.clone()),
            ],
            margin: 20.0,
            ..Default::default()
        });

        loop {
            let elapsed = get_frame_time();

            clear_background(BLACK);

            let bottom_panel_pos = (0.0,  screen_h - 270.0);
            bottom_panel.draw(bottom_panel_pos.0, bottom_panel_pos.1);
            bottom_panel.draw_tooltips(bottom_panel_pos.0, bottom_panel_pos.1);

            let outcome = equipment_section
                .borrow_mut()
                .handle_equipment_drag_and_consumption(equipment_drag, None);
            equipment_drag = outcome.equipment_drag;
            //requested_consumption = outcome.requested_consumption;
            if let Some(drag) = equipment_drag {
                dbg!(&outcome);

                if drag.to_idx.is_some() {
                    let (from, to) = equipment_section.borrow().resolve_drag_to_slots(drag);
                    character.swap_equipment_slots(from, to);
                    equipment_drag = None;
                }
            }

            if equipment_changed.take() {
                println!("CHAR EQUIPMENT CHANGED. UPDATING CHARACTER SHEET...");
                equipment_section
                    .borrow_mut()
                    .repopulate_character_equipment();
            }

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
                bottom_panel_pos.1 - margin - text_dim.height - padding * 2.0,
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
            if rect.contains(mouse_position().into()) && is_mouse_button_pressed(MouseButton::Left)
            {
                transition_countdown = Some(transition_duration);
            }

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
                match event {
                    InternalUiEvent::ButtonHovered(button_id, _button_action, btn_pos) => {
                        if let Some(btn_pos) = btn_pos {
                            hovered_button = Some((button_id, btn_pos));
                        } else if let Some(previously_hovered_button) = hovered_button {
                            if button_id == previously_hovered_button.0 {
                                hovered_button = None
                            }
                        }
                    }
                    _ => {
                        dbg!(event);
                    }
                }
            }

            if let Some((id, btn_pos)) = hovered_button {
                let btn = hoverable_buttons.iter().find(|btn| btn.id == id).unwrap();
                draw_button_tooltip(&font, btn_pos, &btn.tooltip());
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

    Rc::into_inner(character).unwrap()
}
