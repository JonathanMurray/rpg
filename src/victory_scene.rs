use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
};

use macroquad::{
    color::{Color, BLACK, GRAY, LIGHTGRAY, ORANGE, WHITE, YELLOW},
    input::{is_mouse_button_pressed, mouse_position, MouseButton},
    math::Rect,
    miniquad::window::screen_size,
    shapes::{draw_rectangle, draw_rectangle_ex, DrawRectangleParams},
    text::{draw_text, draw_text_ex, measure_text, Font, TextParams},
    texture::Texture2D,
    time::get_frame_time,
    window::{clear_background, next_frame},
};

use crate::{
    action_button::{
        draw_button_tooltip, ActionButton, ButtonAction, ButtonSelected, InternalUiEvent,
    },
    base_ui::{Align, Container, Drawable, Element, LayoutDirection, Style, TextLine},
    core::{
        AttackEnhancement, BaseAction, Character, OnAttackedReaction, OnHitReaction, Spell,
        SpellEnhancement,
    },
    data::{
        BRACE, FIREBALL, HEAL, HEALING_NOVA, HEALING_RAIN, LUNGE_ATTACK, MIND_BLAST, OVERWHELMING,
        QUICK, RAGE, SCREAM, SHACKLED_MIND, SIDE_STEP, SMITE, SWEEP_ATTACK,
    },
    non_combat_ui::NonCombatUi,
    textures::{EquipmentIconId, IconId, PortraitId},
    util::select_n_random,
};

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Learning {
    Spell(Spell),
    OnAttackedReaction(OnAttackedReaction),
    OnHitReaction(OnHitReaction),
    AttackEnhancement(AttackEnhancement),
    SpellEnhancement(SpellEnhancement),
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Reward {
    pub learning: Learning,
    pub money: u32,
}

struct RewardButton {
    action_button: ActionButton,
    context: Option<&'static str>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum AttributeName {
    Str,
    Agi,
    Int,
    Spi,
}

struct AttributeLine {
    element: Element,
    attribute: AttributeName,
}

impl AttributeLine {
    fn new(label: &'static str, attribute: AttributeName, font: &Font) -> Self {
        let text_line = TextLine::new(label, 20, WHITE, Some(font.clone())).with_padding(5.0, 5.0);
        Self {
            element: Element::Text(text_line),
            attribute,
        }
    }
}

impl Drawable for AttributeLine {
    fn draw(&self, x: f32, y: f32) {
        self.element.draw(x, y);
    }

    fn size(&self) -> (f32, f32) {
        self.element.size()
    }
}

pub async fn run_victory_loop(
    player_character: Character,
    font: Font,
    equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
    icons: HashMap<IconId, Texture2D>,

    portrait_textures: &HashMap<PortraitId, Texture2D>,
) -> Character {
    let character = Rc::new(player_character);
    let mut selected_learning = None;
    {
        let mut selected_btn: Option<&RewardButton> = None;
        let money_amount = 3;

        character.gain_money(money_amount);

        let (screen_w, screen_h) = screen_size();
        let x_mid = screen_w / 2.0;

        let transition_duration = 0.5;
        let mut transition_countdown = None;

        let mut bottom_panel = NonCombatUi::new(
            character.clone(),
            &font,
            equipment_icons,
            &icons,
            portrait_textures,
        );

        let event_queue = Rc::new(RefCell::new(vec![]));

        let mut next_button_id = 0;

        let mut rewards = vec![];

        let candidate_attack_enhancements = vec![QUICK, SMITE, OVERWHELMING];
        for enhancement in candidate_attack_enhancements {
            if !character.known_attack_enhancements.contains(&enhancement) {
                rewards.push((ButtonAction::AttackEnhancement(enhancement), Some("Attack")));
            }
        }

        let known_spells = character.known_spells();
        let candidate_spells = vec![
            FIREBALL,
            SWEEP_ATTACK,
            LUNGE_ATTACK,
            BRACE,
            SCREAM,
            SHACKLED_MIND,
            MIND_BLAST,
            HEAL,
            HEALING_NOVA,
            HEALING_RAIN,
        ];
        for spell in candidate_spells {
            if !known_spells.contains(&spell) {
                rewards.push((ButtonAction::Action(BaseAction::CastSpell(spell)), None));
            }
        }
        for spell in &known_spells {
            for enhancement in spell.possible_enhancements {
                if let Some(enhancement) = enhancement {
                    if !character.known_spell_enhancements.contains(&enhancement) {
                        rewards.push((
                            ButtonAction::SpellEnhancement(enhancement),
                            Some(spell.name),
                        ));
                    }
                }
            }
        }

        let candidate_attacked_reactions = vec![SIDE_STEP];
        for reaction in candidate_attacked_reactions {
            if !character.known_attacked_reactions.contains(&reaction) {
                rewards.push((
                    ButtonAction::OnAttackedReaction(reaction),
                    Some("On attacked"),
                ));
            }
        }

        let candidate_on_hit_reactions = vec![RAGE];
        for reaction in candidate_on_hit_reactions {
            if !character.known_on_hit_reactions.contains(&reaction) {
                rewards.push((ButtonAction::OnHitReaction(reaction), Some("On hit")));
            }
        }

        let reward_buttons: Vec<RewardButton> = select_n_random(rewards, 4)
            .into_iter()
            .map(|(action, context)| {
                let id = next_button_id;
                next_button_id += 1;
                RewardButton {
                    action_button: ActionButton::new(
                        action,
                        &event_queue,
                        id,
                        &icons,
                        Some(Rc::clone(&character)),
                    ),
                    context,
                }
            })
            .collect();

        let button_margin = 60.0;
        let row_w: f32 = reward_buttons
            .iter()
            .map(|btn| btn.action_button.size.0)
            .sum::<f32>()
            + button_margin * (reward_buttons.len() - 1) as f32;

        let mut hovered_btn = None;

        let header_color = Color::new(0.7, 0.9, 0.7, 1.0);

        let card_w = 480.0;
        let card_color = Color::new(0.1, 0.1, 0.1, 1.00);

        let mut selected_attribute = None;

        let str_element = Rc::new(RefCell::new(
            TextLine::new("Strength", 20, WHITE, Some(font.clone()))
                .with_padding(15.0, 15.0)
                .with_hover_color(YELLOW),
        ));
        let agi_element = Rc::new(RefCell::new(
            TextLine::new("Agility", 20, WHITE, Some(font.clone()))
                .with_padding(15.0, 15.0)
                .with_hover_color(YELLOW),
        ));
        let int_element = Rc::new(RefCell::new(
            TextLine::new("Intellect", 20, WHITE, Some(font.clone()))
                .with_padding(15.0, 15.0)
                .with_hover_color(YELLOW),
        ));
        let spi_element = Rc::new(RefCell::new(
            TextLine::new("Spirit", 20, WHITE, Some(font.clone()))
                .with_padding(15.0, 15.0)
                .with_hover_color(YELLOW),
        ));

        let attribute_selection_card = Element::Container(Container {
            layout_dir: LayoutDirection::Vertical,
            style: Style {
                background_color: Some(card_color),
                padding: 20.0,
                ..Default::default()
            },
            min_width: Some(card_w),
            align: Align::Center,
            children: vec![
                //Element::Text(TextLine::new("1 stat increase", 32, header_color, None)),
                Element::Empty(0.0, 0.0),
                Element::RcRefCell(str_element.clone()),
                Element::RcRefCell(agi_element.clone()),
                Element::RcRefCell(int_element.clone()),
                Element::RcRefCell(spi_element.clone()),
            ],
            ..Default::default()
        });

        loop {
            let elapsed = get_frame_time();

            clear_background(BLACK);

            bottom_panel.draw_and_handle_input();

            let text = "Rewards!";
            let font_size = 32;
            let text_dim = measure_text(text, Some(&font), font_size, 1.0);
            draw_text(
                text,
                screen_w / 2.0 - text_dim.width / 2.0,
                50.0 + (text_dim.height) / 2.0,
                font_size.into(),
                WHITE,
            );

            let mut y = 125.0;
            draw_rectangle(x_mid - card_w / 2.0, y, card_w, 60.0, card_color);
            let text = &format!("{} gold coins", money_amount);
            let font_size = 28;
            let text_dim = measure_text(text, Some(&font), font_size, 1.0);
            y += 25.0;
            draw_text(
                text,
                screen_w / 2.0 - text_dim.width / 2.0,
                y + (text_dim.height) / 2.0,
                font_size.into(),
                WHITE,
            );

            y += 70.0;
            attribute_selection_card.draw(x_mid - card_w / 2.0, y);

            let mut hovered_attribute = None;
            if str_element.borrow().has_been_hovered.take().is_some() {
                hovered_attribute = Some(AttributeName::Str);
            }
            if agi_element.borrow().has_been_hovered.take().is_some() {
                hovered_attribute = Some(AttributeName::Agi);
            }
            if int_element.borrow().has_been_hovered.take().is_some() {
                hovered_attribute = Some(AttributeName::Int);
            }
            if spi_element.borrow().has_been_hovered.take().is_some() {
                hovered_attribute = Some(AttributeName::Spi);
            }

            if is_mouse_button_pressed(MouseButton::Left) {
                if let Some(new_attribute) = hovered_attribute {
                    let mut did_change = true;
                    if let Some(old_attribute) = selected_attribute {
                        did_change = new_attribute != old_attribute;
                        if did_change {
                            match old_attribute {
                                AttributeName::Str => {
                                    character.add_to_strength(-1);
                                    str_element.borrow_mut().set_color(WHITE);
                                }
                                AttributeName::Agi => {
                                    character.add_to_agility(-1);
                                    agi_element.borrow_mut().set_color(WHITE);
                                }
                                AttributeName::Int => {
                                    character.add_to_intellect(-1);
                                    int_element.borrow_mut().set_color(WHITE);
                                }
                                AttributeName::Spi => {
                                    character.add_to_spirit(-1);
                                    spi_element.borrow_mut().set_color(WHITE);
                                }
                            }
                        }
                    }

                    if did_change {
                        match new_attribute {
                            AttributeName::Str => {
                                character.add_to_strength(1);
                                str_element.borrow_mut().set_color(ORANGE);
                            }
                            AttributeName::Agi => {
                                character.add_to_agility(1);
                                agi_element.borrow_mut().set_color(ORANGE);
                            }
                            AttributeName::Int => {
                                character.add_to_intellect(1);
                                int_element.borrow_mut().set_color(ORANGE);
                            }
                            AttributeName::Spi => {
                                character.add_to_spirit(1);
                                spi_element.borrow_mut().set_color(ORANGE);
                            }
                        }
                        bottom_panel.on_character_stats_changed();
                        bottom_panel.draw_and_handle_input();
                        selected_attribute = Some(new_attribute);
                    }
                }
            }

            y += 240.0;

            draw_rectangle(x_mid - card_w / 2.0, y, card_w, 180.0, card_color);
            /*
            let text = "1 new skill";
            let font_size = 32;
            let text_dim = measure_text(text, Some(&font), font_size, 1.0);
            y += 20.0;
            draw_text(
                text,
                screen_w / 2.0 - text_dim.width / 2.0,
                y + (text_dim.height) / 2.0,
                font_size.into(),
                header_color,
            );
             */

            let mut btn_x = x_mid - row_w / 2.0;
            let btn_y = y + 80.0;

            for btn in &reward_buttons {
                btn.action_button.draw(btn_x, btn_y);

                if let Some(context) = btn.context {
                    let text = format!("({})", context);
                    let font_size = 16;
                    let text_dim = measure_text(&text, Some(&font), font_size, 1.0);
                    draw_text_ex(
                        &text,
                        btn_x + btn.action_button.size.0 / 2.0 - text_dim.width / 2.0,
                        btn_y - 30.0,
                        TextParams {
                            font: Some(&font),
                            font_size,
                            color: ORANGE,
                            ..Default::default()
                        },
                    );
                }

                let text = btn.action_button.action.name();
                let font_size = 18;
                let text_dim = measure_text(text, Some(&font), font_size, 1.0);
                draw_text_ex(
                    text,
                    btn_x + btn.action_button.size.0 / 2.0 - text_dim.width / 2.0,
                    btn_y - 10.0,
                    TextParams {
                        font: Some(&font),
                        font_size,
                        color: WHITE,
                        ..Default::default()
                    },
                );

                btn_x += btn.action_button.size.0 + button_margin;
            }

            for event in event_queue.borrow_mut().drain(..) {
                match event {
                    InternalUiEvent::ButtonHovered(btn_id, _button_action, maybe_btn_pos) => {
                        if let Some(btn_pos) = maybe_btn_pos {
                            hovered_btn = Some((btn_id, btn_pos));
                        } else {
                            hovered_btn = None;
                        }
                    }
                    InternalUiEvent::ButtonClicked(btn_id, btn_action) => {
                        let reward_btn = reward_buttons
                            .iter()
                            .find(|btn| btn.action_button.id == btn_id);

                        if let Some(btn) = reward_btn {
                            if let Some(previously_selected) = selected_btn {
                                previously_selected
                                    .action_button
                                    .selected
                                    .set(ButtonSelected::No);
                            }

                            btn.action_button.selected.set(ButtonSelected::Yes);
                            selected_btn = Some(btn);
                        }
                    }
                }
            }

            if let Some((btn_id, btn_pos)) = hovered_btn {
                //character_ui.draw(y);
                let reward_btn = reward_buttons
                    .iter()
                    .find(|btn| btn.action_button.id == btn_id)
                    .map(|reward_btn| &reward_btn.action_button)
                    .unwrap();
                draw_button_tooltip(&font, btn_pos, &reward_btn.tooltip());
            }

            let some_remaining_rewards = selected_btn.is_none() || selected_attribute.is_none();

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
                    selected_learning = Some(action_to_reward_choice(
                        selected_btn.unwrap().action_button.action,
                    ));

                    break;
                }
            }

            next_frame().await;
        }
    }

    let mut character = Rc::into_inner(character).unwrap();

    match selected_learning.unwrap() {
        Learning::Spell(spell) => character.known_actions.push(BaseAction::CastSpell(spell)),
        Learning::OnAttackedReaction(reaction) => character.known_attacked_reactions.push(reaction),
        Learning::OnHitReaction(reaction) => character.known_on_hit_reactions.push(reaction),
        Learning::AttackEnhancement(enhancement) => {
            character.known_attack_enhancements.push(enhancement)
        }
        Learning::SpellEnhancement(enhancement) => {
            character.known_spell_enhancements.push(enhancement)
        }
    }

    character
}

fn action_to_reward_choice(btn_action: ButtonAction) -> Learning {
    match btn_action {
        ButtonAction::Action(BaseAction::CastSpell(spell)) => Learning::Spell(spell),
        ButtonAction::Action(..) => unreachable!(),
        ButtonAction::OnAttackedReaction(reaction) => Learning::OnAttackedReaction(reaction),
        ButtonAction::OnHitReaction(reaction) => Learning::OnHitReaction(reaction),
        ButtonAction::AttackEnhancement(enhancement) => Learning::AttackEnhancement(enhancement),
        ButtonAction::SpellEnhancement(enhancement) => Learning::SpellEnhancement(enhancement),
        ButtonAction::OpportunityAttack | ButtonAction::Proceed => unreachable!(),
    }
}
