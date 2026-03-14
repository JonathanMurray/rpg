use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    default,
    rc::Rc,
};

use macroquad::{
    color::{Color, BLACK, DARKGRAY, GRAY, LIGHTGRAY, MAGENTA, ORANGE, WHITE, YELLOW},
    input::{is_key_pressed, is_mouse_button_pressed, mouse_position, MouseButton},
    math::Rect,
    miniquad::window::screen_size,
    shapes::{draw_rectangle, draw_rectangle_ex, DrawRectangleParams},
    text::{measure_text, Font, TextParams},
    texture::Texture2D,
    time::get_frame_time,
    window::{clear_background, next_frame, screen_height, screen_width},
};

use crate::{
    action_button::{
        button_action_tooltip, draw_button_tooltip, ActionButton, ButtonAction, ButtonHovered,
        ButtonSelected, InternalUiEvent,
    },
    base_ui::{
        draw_text_rounded, Align, Container, Drawable, Element, LayoutDirection, Style, TextLine,
    },
    core::{
        Ability, AbilityEnhancement, AttackEnhancement, BaseAction, Character, OnAttackedReaction,
        OnHitReaction, Party, PlayerId, WeaponType,
    },
    data::{
        PassiveSkill, BRACE, CRIPPLING_SHOT, FIREBALL, HEAL, HEALING_NOVA, HEALING_RAIN,
        LUNGE_ATTACK, MIND_BLAST, OVERWHELMING, QUICK, RAGE, SCREAM, SHACKLED_MIND, SIDE_STEP,
        SMITE, SWEEP_ATTACK,
    },
    game_ui::ResourceBars,
    game_ui_components::PlayerCharacterPortrait,
    non_combat_ui::{NonCombatCharacterUi, PortraitRow},
    resources::{GameResources, UiResources},
    sounds::SoundPlayer,
    textures::{EquipmentIconId, IconId, PortraitId, StatusId},
    util::select_n_random,
};

pub struct CharacterGrowth {
    new_skills: Vec<ButtonAction>,
    pub is_new_joiner: bool,
}

impl CharacterGrowth {
    pub fn just_new_skills(new_skills: Vec<ButtonAction>) -> Self {
        Self {
            new_skills,
            is_new_joiner: false,
        }
    }

    pub fn unchanged() -> Self {
        Self {
            new_skills: vec![],
            is_new_joiner: false,
        }
    }

    pub fn new_joiner() -> Self {
        Self {
            new_skills: vec![],
            is_new_joiner: true,
        }
    }
}

pub async fn run_transition_loop(
    mut player_characters: Vec<(Rc<Character>, CharacterGrowth)>,
    resources: &GameResources,
    ui_resources: &UiResources,
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
                            char.learn_ability(*ability);
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

            let mut recovered = vec![];

            char.conditions.borrow_mut().clear();

            let health_gain = char
                .health
                .gain(((char.health.max() - char.health.current()) as f32 * 0.1).ceil() as u32);
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

    let portrait_textures = &ui_resources.portrait_textures;
    let status_textures = &resources.status_textures;
    let simple_font = &resources.simple_font;
    let big_font = &resources.big_font;
    let event_queue = Rc::new(RefCell::new(vec![]));

    let mut next_btn_id = 0;

    {
        let mut children = vec![];
        for (char, growth) in &characters {
            let resources_bars = ResourceBars::new(char, simple_font);

            let texture = portrait_textures[&char.portrait].clone();
            let portrait = PlayerCharacterPortrait::new(
                char,
                simple_font.clone(),
                texture,
                status_textures.clone(),
                sound_player.clone(),
            );
            let name = Element::Text(TextLine::new(char.name, 18, WHITE, Some(big_font.clone())));
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
                ..Default::default()
            };
            let mut char_sections = vec![Element::Container(portrait_section)];

            if !growth.new_skills.is_empty() {
                let mut buttons = vec![];
                for action in &growth.new_skills {
                    let btn = ActionButton::new(
                        action.clone(),
                        Some(Rc::clone(&event_queue)),
                        next_btn_id,
                        &ui_resources.icons,
                        Some(Rc::clone(char)),
                        &simple_font,
                    );
                    next_btn_id += 1;
                    buttons.push(Element::boxed(btn));
                }
                let buttons_row = Container {
                    layout_dir: LayoutDirection::Horizontal,
                    margin: 5.0,
                    children: buttons,
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
                    ..Default::default()
                };
                char_sections.push(Element::Container(skills_section));
            }

            let char_row = Container {
                layout_dir: LayoutDirection::Horizontal,
                margin: 70.0,
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
            container.draw(
                mid_x - container_size.0 / 2.0,
                mid_y - container_size.1 / 2.0,
            );

            for event in event_queue.borrow_mut().drain(..) {
                if let InternalUiEvent::ButtonHovered(hover_event) = event {
                    if hover_event.hovered_pos.is_some() {
                        hovered_btn = Some(hover_event);
                    } else {
                        if let Some(prev_hovered_button) = &hovered_btn {
                            if prev_hovered_button.id == hover_event.id {
                                hovered_btn = None;
                            }
                        }
                    }
                }
            }

            if let Some(btn) = &hovered_btn {
                let tooltip = button_action_tooltip(&btn.action);
                draw_button_tooltip(simple_font, btn.hovered_pos.unwrap(), &tooltip, true);
            }

            let text = "Continue";
            let font_size = 30;
            let margin = 25.0;
            let padding = 15.0;
            let text_dim = measure_text(text, Some(&simple_font), font_size, 1.0);
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
                    font: Some(&simple_font),
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

            next_frame().await;
        }
    }

    let characters: Vec<Rc<Character>> = characters
        .into_iter()
        .map(|(character, _new_skills)| character)
        .collect();

    characters
}
