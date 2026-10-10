use std::{cell::RefCell, collections::HashMap, rc::Rc, vec};

use macroquad::{
    color::{Color, DARKGRAY, GRAY, LIGHTGRAY, RED, WHITE, YELLOW},
    input::{is_key_pressed, is_mouse_button_pressed, mouse_position, MouseButton},
    math::Rect,
    shapes::draw_rectangle,
    text::{measure_text, TextParams},
    window::{next_frame, screen_height, screen_width},
};

use crate::{
    action_button::{
        button_action_tooltip, draw_button_tooltip, ActionButton, ButtonAction, InternalUiEvent,
    },
    base_ui::{
        draw_text_rounded, Align, Container, Drawable, Element, LayoutDirection, Style, TextLine,
    },
    core::{
        Ability, BaseAction, Character, EquipmentEntry, EquipmentSlotRole, HandType, Party,
        PlayerId,
    },
    equipment_ui::EquipmentSlot,
    game_ui::ResourceBars,
    game_ui_components::PlayerCharacterPortrait,
    resources::GameResources,
    sounds::{SoundId, SoundPlayer},
    textures::UI_ARROW_TEXTURE,
    tooltip::handle_keyword_tooltip_end_of_frame,
};

pub struct CharacterGrowth {
    new_skills: Vec<ButtonAction>,
    new_equipment: Vec<EquipmentEntry>,
    pub is_new_joiner: bool,
}

impl CharacterGrowth {
    pub fn of(new_skills: Vec<ButtonAction>, new_equipment: Vec<EquipmentEntry>) -> Self {
        Self {
            new_skills,
            new_equipment,
            is_new_joiner: false,
        }
    }

    pub fn unchanged() -> Self {
        Self {
            new_skills: vec![],
            is_new_joiner: false,
            new_equipment: vec![],
        }
    }

    pub fn new_joiner() -> Self {
        Self {
            new_skills: vec![],
            is_new_joiner: true,
            new_equipment: vec![],
        }
    }
}

pub async fn run_transition_loop(
    mut player_characters: Vec<(Rc<Character>, CharacterGrowth)>,
    resources: &GameResources,
    party: &Party,
    sound_player: SoundPlayer,
) -> Vec<Rc<Character>> {
    let mut restoration_texts = HashMap::<PlayerId, Vec<String>>::default();
    for (char, growth) in &mut player_characters {
        char.has_taken_a_turn_this_round.set(false); // to prevent "Done" from showing under portrait
        if char.is_dead() {
            restoration_texts.insert(
                char.player_id(),
                vec![format!("Dead until revived at an altar...")],
            );
        } else {
            for new_skill in &growth.new_skills {
                match new_skill {
                    ButtonAction::Action(base_action) => match base_action {
                        BaseAction::UseAbility(ability) => {
                            char.learn_ability(ability);
                        }
                        other => panic!("{:?}", other),
                    },
                    ButtonAction::AttackEnhancement(attack_enhancement) => {
                        char.learn_attack_enhancement(*attack_enhancement);
                    }
                    ButtonAction::AbilityEnhancement(ability_enhancement) => {
                        char.learn_ability_enhancement(*ability_enhancement)
                    }
                    ButtonAction::Passive(passive) => {
                        char.learn_passive(*passive);
                    }
                    other => todo!("handle growth: {:?}", other),
                }
            }

            for new_eq in &growth.new_equipment {
                match *new_eq {
                    EquipmentEntry::Weapon(weapon) => char.set_weapon(HandType::MainHand, weapon),
                    EquipmentEntry::Shield(shield) => char.set_shield(shield),
                    EquipmentEntry::Armor(armor_piece) => char.armor_piece.set(Some(armor_piece)),
                    EquipmentEntry::Trinket(trinket) => char.trinket.set(Some(trinket)),
                    _ => {
                        let gained = char.try_gain_equipment(*new_eq);
                        if !gained {
                            println!("WARN: {} couldn't gain equipment: {:?}", char.name, new_eq)
                        }
                    }
                }
            }

            let mut recovered = vec![];

            char.conditions.borrow_mut().clear();

            let health_gain = char
                .health
                .gain(((char.health.max() - char.health.current()) as f32 * 0.25).ceil() as u32);
            if health_gain > 0 {
                recovered.push(format!("{} |<heart>|", health_gain));
            }

            let mana_gain = char
                .mana
                .gain((char.mana.max() as f32 * 0.25).ceil() as u32);
            if mana_gain > 0 {
                recovered.push(format!("{} |<mana>|", mana_gain));
            }

            char.stamina.set_to_max();

            if !recovered.is_empty() {
                restoration_texts.insert(
                    char.player_id(),
                    vec![format!("Restored: {}", recovered.join("  "))],
                );
            }
        }
    }

    let characters: Vec<(Rc<Character>, CharacterGrowth)> = player_characters;

    let simple_font = &resources.simple_font;
    let big_font = &resources.big_font;
    let event_queue = Rc::new(RefCell::new(vec![]));

    let mut next_btn_id = 0;

    let mut hoverable_buttons = vec![];

    {
        let mut children = vec![];
        for (char, growth) in &characters {
            let resources_bars = ResourceBars::new(char, big_font);

            let portrait = PlayerCharacterPortrait::new(
                char,
                simple_font.clone(),
                char.portrait,
                sound_player.clone(),
            );
            let name = Element::Text(TextLine::new(&char.name, 18, WHITE, Some(big_font.clone())));
            let mut portrait_rows = vec![
                Element::boxed(portrait),
                name,
                Element::boxed(resources_bars),
            ];
            if let Some(lines) = restoration_texts.get(&char.player_id()) {
                for line in lines {
                    portrait_rows.push(Element::Text(TextLine::new(
                        line,
                        16,
                        WHITE,
                        Some(simple_font.clone()),
                    )));
                }
            }
            let portrait_section = Container {
                layout_dir: LayoutDirection::Vertical,
                align: Align::Center,
                margin: 10.0,
                children: portrait_rows,
                min_width: Some(200.0),
                ..Default::default()
            };
            let mut char_sections = vec![Element::Container(portrait_section)];

            if !growth.new_skills.is_empty() {
                let mut button_cols = vec![];
                for action in &growth.new_skills {
                    let mut col_children = vec![];

                    let parent_action = match action {
                        ButtonAction::AbilityEnhancement(enhancement) => {
                            let ability: &'static Ability = char
                                .known_abilities()
                                .iter()
                                .filter(|a| a.id == enhancement.ability_id)
                                .next()
                                .unwrap();
                            Some(ButtonAction::Action(BaseAction::UseAbility(ability)))
                        }
                        ButtonAction::AttackEnhancement(..) => Some(ButtonAction::Action(
                            BaseAction::Attack(char.attack_action().unwrap()),
                        )),
                        _ => None,
                    };

                    if let Some(action) = parent_action {
                        let parent_btn = Rc::new(ActionButton::new(
                            action,
                            Some(Rc::clone(&event_queue)),
                            next_btn_id,
                            Some(Rc::clone(char)),
                            simple_font,
                        ));
                        hoverable_buttons.push(Rc::clone(&parent_btn));
                        next_btn_id += 1;

                        col_children.push(Element::Container(Container {
                            children: vec![Element::Rc(parent_btn)],
                            style: Style {
                                fg_color: Some(Color::new(0.0, 0.0, 0.0, 0.5)),
                                ..Default::default()
                            },
                            ..Default::default()
                        }));
                        col_children.push(Element::Texture(
                            UI_ARROW_TEXTURE.get().unwrap().clone(),
                            None,
                        ));
                    }

                    let btn = Rc::new(ActionButton::new(
                        *action,
                        Some(Rc::clone(&event_queue)),
                        next_btn_id,
                        Some(Rc::clone(char)),
                        simple_font,
                    ));
                    hoverable_buttons.push(Rc::clone(&btn));
                    next_btn_id += 1;
                    col_children.push(Element::Rc(btn));

                    let col = Container {
                        layout_dir: LayoutDirection::Vertical,
                        margin: 5.0,
                        children: col_children,
                        align: Align::Center,
                        ..Default::default()
                    };
                    button_cols.push(Element::Container(col));
                }
                let buttons_row = Container {
                    layout_dir: LayoutDirection::Horizontal,
                    margin: 10.0,
                    children: button_cols,
                    ..Default::default()
                };
                let header = if growth.new_skills.len() == 1 {
                    "New Skill"
                } else {
                    "New Skills"
                };
                let skills_section = Container {
                    layout_dir: LayoutDirection::Vertical,
                    margin: 15.0,
                    align: Align::Center,
                    children: vec![
                        Element::Text(TextLine::new(header, 18, WHITE, Some(big_font.clone()))),
                        Element::Container(buttons_row),
                    ],
                    min_width: Some(200.0),
                    ..Default::default()
                };
                char_sections.push(Element::Container(skills_section));
            }

            if !growth.new_equipment.is_empty() {
                let mut slots: Vec<Element> = vec![];
                for eq_entry in &growth.new_equipment {
                    let slot = EquipmentSlot::new(
                        simple_font.clone(),
                        Some(*eq_entry),
                        EquipmentSlotRole::Inventory(0),
                        None,
                    );
                    slots.push(Element::boxed(slot));
                }
                let slots_row = Container {
                    layout_dir: LayoutDirection::Horizontal,
                    margin: 5.0,
                    children: slots,
                    ..Default::default()
                };
                let header = "New equipment";
                let eq_section = Container {
                    layout_dir: LayoutDirection::Vertical,
                    margin: 15.0,
                    align: Align::Center,
                    children: vec![
                        Element::Text(TextLine::new(header, 18, WHITE, Some(big_font.clone()))),
                        Element::Container(slots_row),
                    ],
                    ..Default::default()
                };
                char_sections.push(Element::Container(eq_section));
            }

            let char_row = Container {
                layout_dir: LayoutDirection::Horizontal,
                margin: 10.0,
                children: char_sections,
                ..Default::default()
            };
            let mut rows = vec![
                //Element::Text(TextLine::new(char.name, 18, WHITE, Some(big_font.clone()))),
                Element::Container(char_row),
            ];

            if growth.is_new_joiner {
                rows.insert(
                    0,
                    Element::Text(TextLine::new(
                        "New character:",
                        24,
                        WHITE,
                        Some(big_font.clone()),
                    )),
                );
            }

            children.push(Element::Container(Container {
                layout_dir: LayoutDirection::Vertical,
                margin: 40.0,
                children: rows,
                ..Default::default()
            }));
        }

        let container = Container {
            layout_dir: LayoutDirection::Vertical,
            style: Style {
                border_color: Some(DARKGRAY),
                border_inner_rounding: Some(5.0),
                padding: 80.0,
                ..Default::default()
            },
            margin: 130.0,
            border_between_children: Some(DARKGRAY),

            children,
            ..Default::default()
        };
        let container_size = container.size();

        let mid_x = screen_width() / 2.0;
        let mid_y = screen_height() / 2.0;
        let mut hovered_btn = None;
        loop {
            let x = mid_x - container_size.0 / 2.0;
            let y = mid_y - container_size.1 / 2.0;
            container.draw(x, y);
            container.draw_tooltips(x, y);

            for event in event_queue.borrow_mut().drain(..) {
                if let InternalUiEvent::ButtonHovered(hover_event) = event {
                    if hover_event.hovered_pos.is_some() {
                        sound_player.play(SoundId::HoverButton);
                        hovered_btn = Some(hover_event);
                    } else if let Some(prev_hovered_button) = &hovered_btn {
                        if prev_hovered_button.id == hover_event.id {
                            hovered_btn = None;
                        }
                    }
                }
            }

            if let Some(hovered) = &hovered_btn {
                let btn = hoverable_buttons
                    .iter()
                    .filter(|btn| btn.id == hovered.id)
                    .next()
                    .unwrap();
                //let tooltip = button_action_tooltip(&hovered.action);
                let tooltip = btn.tooltip();
                draw_button_tooltip(simple_font, hovered.hovered_pos.unwrap(), &tooltip, true);
            }

            let text = "Continue";
            let font_size = 30;
            let margin = 25.0;
            let padding = 15.0;
            let text_dim = measure_text(text, Some(simple_font), font_size, 1.0);
            let rect = Rect::new(
                screen_width() - margin - text_dim.width - padding * 2.0,
                screen_height() - margin - text_dim.height - padding * 2.0,
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
                    font: Some(simple_font),
                    font_size,
                    color: YELLOW,
                    ..Default::default()
                },
            );
            if rect.contains(mouse_position().into()) && is_mouse_button_pressed(MouseButton::Left)
            {
                break;
            }
            if is_key_pressed(macroquad::input::KeyCode::Space) {
                break;
            }

            handle_keyword_tooltip_end_of_frame();

            next_frame().await;
        }
    }

    let characters: Vec<Rc<Character>> = characters
        .into_iter()
        .map(|(character, _new_skills)| character)
        .collect();

    characters
}
