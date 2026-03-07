use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
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
        OnHitReaction, Party, WeaponType,
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
    is_new_joiner: bool,
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
    mut player_characters: Vec<(Character, CharacterGrowth)>,
    resources: &GameResources,
    ui_resources: &UiResources,
    party: &Party,
) -> Vec<Character> {
    for (char, growth) in &mut player_characters {
        for new_skill in &growth.new_skills {
            match new_skill {
                ButtonAction::Action(base_action) => match base_action {
                    BaseAction::UseAbility(ability) => {
                        char.learn_ability(*ability);
                    }
                    other => panic!("{:?}", other),
                },
                ButtonAction::AttackEnhancement(attack_enhancement) => {
                    char.known_attack_enhancements.push(*attack_enhancement);
                }
                other => todo!("handle growth: {:?}", other),
            }
        }
    }

    let characters: Vec<(Rc<Character>, CharacterGrowth)> = player_characters
        .into_iter()
        .map(|(char, new_skills)| (Rc::new(char), new_skills))
        .collect();

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
            );
            let name = Element::Text(TextLine::new(char.name, 18, WHITE, Some(big_font.clone())));
            let portrait_section = Container {
                layout_dir: LayoutDirection::Vertical,
                align: Align::Center,
                margin: 10.0,
                children: vec![
                    Element::boxed(portrait),
                    name,
                    Element::boxed(resources_bars),
                ],
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
                        18,
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

            next_frame().await;

            if is_key_pressed(macroquad::input::KeyCode::Space) {
                break;
            }
        }
    }

    let characters: Vec<Character> = characters
        .into_iter()
        .map(|(character, _new_skills)| Rc::into_inner(character).unwrap())
        .collect();

    characters
}
