use std::{cell::RefCell, collections::HashMap, rc::Rc};

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
    non_combat_ui::{NonCombatUi, PortraitRow},
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

struct RewardSelectionUi {
    bottom_panel: NonCombatUi,
    attribute_selection_card: Element,
    selected_attribute: Option<AttributeName>,
    str_element: Rc<RefCell<TextLine>>,
    agi_element: Rc<RefCell<TextLine>>,
    int_element: Rc<RefCell<TextLine>>,
    spi_element: Rc<RefCell<TextLine>>,
    character: Rc<Character>,
    reward_buttons: Vec<Rc<RewardButton>>,
    hovered_btn: Option<(u32, (f32, f32))>,
    selected_btn: Option<Rc<RewardButton>>,
    font: Font,
    event_queue: Rc<RefCell<Vec<InternalUiEvent>>>,
}

impl RewardSelectionUi {
    fn new(
        character: Rc<Character>,
        font: Font,
        equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
        icons: HashMap<IconId, Texture2D>,
        portrait_textures: &HashMap<PortraitId, Texture2D>,
        rewards: Vec<(ButtonAction, Option<&'static str>)>,
        next_button_id: &mut u32,
    ) -> Self {
        let bottom_panel = NonCombatUi::new(
            character.clone(),
            &font,
            equipment_icons,
            &icons,
            portrait_textures,
        );

        let event_queue = Rc::new(RefCell::new(vec![]));

        let reward_buttons: Vec<Rc<RewardButton>> = rewards
            .into_iter()
            .map(|(action, context)| {
                let id = *next_button_id;
                *next_button_id += 1;
                let btn = RewardButton {
                    action_button: ActionButton::new(
                        action,
                        &event_queue,
                        id,
                        &icons,
                        Some(Rc::clone(&character)),
                    ),
                    context,
                };

                btn.action_button
                    .enabled
                    .set(can_learn(&character, action_to_reward_choice(action)));

                Rc::new(btn)
            })
            .collect();

        let card_w = 480.0;
        let card_color = Color::new(0.1, 0.1, 0.1, 1.00);

        let attr_font_size = 24;
        let str_element = Rc::new(RefCell::new(
            TextLine::new("Strength", attr_font_size, WHITE, Some(font.clone()))
                .with_padding(15.0, 10.0)
                .with_hover_color(YELLOW),
        ));
        let agi_element = Rc::new(RefCell::new(
            TextLine::new("Agility", attr_font_size, WHITE, Some(font.clone()))
                .with_padding(15.0, 10.0)
                .with_hover_color(YELLOW),
        ));
        let int_element = Rc::new(RefCell::new(
            TextLine::new("Intellect", attr_font_size, WHITE, Some(font.clone()))
                .with_padding(15.0, 10.0)
                .with_hover_color(YELLOW),
        ));
        let spi_element = Rc::new(RefCell::new(
            TextLine::new("Spirit", attr_font_size, WHITE, Some(font.clone()))
                .with_padding(15.0, 10.0)
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

        Self {
            bottom_panel,
            attribute_selection_card,
            selected_attribute: None,
            str_element,
            agi_element,
            int_element,
            spi_element,
            character,
            reward_buttons,
            hovered_btn: None,
            selected_btn: None,
            font,
            event_queue,
        }
    }

    fn has_remaining_rewards(&self) -> bool {
        self.selected_btn.is_none() || self.selected_attribute.is_none()
    }

    fn draw(&mut self) {
        self.bottom_panel.draw_and_handle_input();

        let (screen_w, screen_h) = screen_size();
        let x_mid = screen_w / 2.0;

        let mut y = 300.0;

        let card_w = 480.0;
        let card_color = Color::new(0.1, 0.1, 0.1, 1.00);

        self.attribute_selection_card.draw(x_mid - card_w / 2.0, y);

        let mut hovered_attribute = None;
        if self.str_element.borrow().has_been_hovered.take().is_some() {
            hovered_attribute = Some(AttributeName::Str);
        }
        if self.agi_element.borrow().has_been_hovered.take().is_some() {
            hovered_attribute = Some(AttributeName::Agi);
        }
        if self.int_element.borrow().has_been_hovered.take().is_some() {
            hovered_attribute = Some(AttributeName::Int);
        }
        if self.spi_element.borrow().has_been_hovered.take().is_some() {
            hovered_attribute = Some(AttributeName::Spi);
        }

        if is_mouse_button_pressed(MouseButton::Left) {
            if let Some(new_attribute) = hovered_attribute {
                let mut did_change = true;
                if let Some(old_attribute) = self.selected_attribute {
                    did_change = new_attribute != old_attribute;
                    if did_change {
                        match old_attribute {
                            AttributeName::Str => {
                                self.character.add_to_strength(-1);
                                self.str_element.borrow_mut().set_color(WHITE);
                            }
                            AttributeName::Agi => {
                                self.character.add_to_agility(-1);
                                self.agi_element.borrow_mut().set_color(WHITE);
                            }
                            AttributeName::Int => {
                                self.character.add_to_intellect(-1);
                                self.int_element.borrow_mut().set_color(WHITE);
                            }
                            AttributeName::Spi => {
                                self.character.add_to_spirit(-1);
                                self.spi_element.borrow_mut().set_color(WHITE);
                            }
                        }
                    }
                }

                if did_change {
                    match new_attribute {
                        AttributeName::Str => {
                            self.character.add_to_strength(1);
                            self.str_element.borrow_mut().set_color(ORANGE);
                        }
                        AttributeName::Agi => {
                            self.character.add_to_agility(1);
                            self.agi_element.borrow_mut().set_color(ORANGE);
                        }
                        AttributeName::Int => {
                            self.character.add_to_intellect(1);
                            self.int_element.borrow_mut().set_color(ORANGE);
                        }
                        AttributeName::Spi => {
                            self.character.add_to_spirit(1);
                            self.spi_element.borrow_mut().set_color(ORANGE);
                        }
                    }
                    self.bottom_panel.on_character_stats_changed();
                    //bottom_panel2.draw_and_handle_input();
                    self.selected_attribute = Some(new_attribute);
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

        let button_margin = 60.0;

        let row_w: f32 = self
            .reward_buttons
            .iter()
            .map(|btn| btn.action_button.size.0)
            .sum::<f32>()
            + button_margin * (self.reward_buttons.len() - 1) as f32;

        let mut btn_x = x_mid - row_w / 2.0;
        let btn_y = y + 80.0;

        for btn in &self.reward_buttons {
            btn.action_button.draw(btn_x, btn_y);

            if let Some(context) = btn.context {
                let text = format!("({})", context);
                let font_size = 16;
                let text_dim = measure_text(&text, Some(&self.font), font_size, 1.0);
                draw_text_ex(
                    &text,
                    btn_x + btn.action_button.size.0 / 2.0 - text_dim.width / 2.0,
                    btn_y - 30.0,
                    TextParams {
                        font: Some(&self.font),
                        font_size,
                        color: ORANGE,
                        ..Default::default()
                    },
                );
            }

            let text = btn.action_button.action.name();
            let font_size = 18;
            let text_dim = measure_text(text, Some(&self.font), font_size, 1.0);
            draw_text_ex(
                text,
                btn_x + btn.action_button.size.0 / 2.0 - text_dim.width / 2.0,
                btn_y - 10.0,
                TextParams {
                    font: Some(&self.font),
                    font_size,
                    color: WHITE,
                    ..Default::default()
                },
            );

            btn_x += btn.action_button.size.0 + button_margin;
        }

        for event in self.event_queue.borrow_mut().drain(..) {
            match event {
                InternalUiEvent::ButtonHovered(btn_id, _button_action, maybe_btn_pos) => {
                    if let Some(btn_pos) = maybe_btn_pos {
                        self.hovered_btn = Some((btn_id, btn_pos));
                    } else {
                        self.hovered_btn = None;
                    }
                }
                InternalUiEvent::ButtonClicked(btn_id, _btn_action) => {
                    let reward_btn = self
                        .reward_buttons
                        .iter()
                        .find(|btn| btn.action_button.id == btn_id);

                    if let Some(btn) = reward_btn {
                        if let Some(previously_selected) = &self.selected_btn {
                            previously_selected
                                .action_button
                                .selected
                                .set(ButtonSelected::No);
                        }

                        btn.action_button.selected.set(ButtonSelected::Yes);
                        self.selected_btn = Some(Rc::clone(btn));
                    }
                }
            }
        }

        if let Some((btn_id, btn_pos)) = self.hovered_btn {
            let reward_btn = self
                .reward_buttons
                .iter()
                .find(|btn| btn.action_button.id == btn_id)
                .map(|reward_btn| &reward_btn.action_button)
                .unwrap();
            draw_button_tooltip(&self.font, btn_pos, &reward_btn.tooltip());
        }
    }
}

pub async fn run_victory_loop(
    player_characters: Vec<Character>,
    font: Font,
    equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
    icons: HashMap<IconId, Texture2D>,
    portrait_textures: &HashMap<PortraitId, Texture2D>,
) -> Vec<Character> {
    let characters: Vec<Rc<Character>> = player_characters.into_iter().map(Rc::new).collect();
    let mut selected_learnings: Vec<Option<Learning>> = vec![];
    {
        let mut portrait_row = PortraitRow::new(&characters, portrait_textures);

        let money_amount = 3;

        // TODO: money should probably be on a whole-party level, and not per character
        characters[0].gain_money(money_amount);

        let (screen_w, screen_h) = screen_size();
        let x_mid = screen_w / 2.0;

        let transition_duration = 0.5;
        let mut transition_countdown = None;

        let mut candidate_rewards = vec![];
        for enhancement in vec![QUICK, SMITE, OVERWHELMING] {
            candidate_rewards.push((ButtonAction::AttackEnhancement(enhancement), Some("Attack")));
        }
        for spell in vec![
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
        ] {
            candidate_rewards.push((ButtonAction::Action(BaseAction::CastSpell(spell)), None));
        }
        for character in &characters {
            for spell in character.known_spells() {
                for enhancement in spell.possible_enhancements {
                    if let Some(enhancement) = enhancement {
                        candidate_rewards.push((
                            ButtonAction::SpellEnhancement(enhancement),
                            Some(spell.name),
                        ));
                    }
                }
            }
        }
        {
            let reaction = SIDE_STEP;
            candidate_rewards.push((
                ButtonAction::OnAttackedReaction(reaction),
                Some("On attacked"),
            ));
        }
        {
            let reaction = RAGE;
            candidate_rewards.push((ButtonAction::OnHitReaction(reaction), Some("On hit")));
        }
        let mut rewards: Vec<(ButtonAction, Option<&'static str>)> = vec![];
        for character in &characters {
            let applicable = select_n_random(
                candidate_rewards
                    .iter()
                    .filter(|reward| {
                        !rewards.contains(reward)
                            && can_learn(character, action_to_reward_choice(reward.0))
                    })
                    .copied()
                    .collect(),
                2,
            );
            rewards.extend_from_slice(&applicable);
        }

        let mut next_button_id = 0;

        let mut reward_selection_uis: Vec<RewardSelectionUi> = characters
            .iter()
            .map(|char| {
                RewardSelectionUi::new(
                    Rc::clone(char),
                    font.clone(),
                    equipment_icons,
                    icons.clone(),
                    portrait_textures,
                    rewards.clone(),
                    &mut next_button_id,
                )
            })
            .collect();

        loop {
            let elapsed = get_frame_time();

            clear_background(BLACK);
            portrait_row.draw_and_handle_input();
            reward_selection_uis[portrait_row.selected_idx].draw();

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

            let card_color = Color::new(0.1, 0.1, 0.1, 1.00);
            let card_w = 480.0;

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

            let text = if reward_selection_uis
                .iter()
                .any(|ui| ui.has_remaining_rewards())
            {
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
                    dbg!(selected_learnings.len());
                    dbg!(reward_selection_uis.len());
                    for ui in &reward_selection_uis {
                        let learning: Option<Learning> = ui
                            .selected_btn
                            .as_ref()
                            .map(|btn| action_to_reward_choice(btn.action_button.action));
                        selected_learnings.push(learning);
                    }
                    dbg!(selected_learnings.len());
                    break;
                }
            }

            next_frame().await;
        }
    }

    let mut characters: Vec<Character> = characters
        .into_iter()
        .map(|character| Rc::into_inner(character).unwrap())
        .collect();

    assert_eq!(characters.len(), selected_learnings.len());

    for (i, character) in characters.iter_mut().enumerate() {
        if let Some(learning) = selected_learnings[i] {
            apply_learning(learning, character);
        }
    }

    characters
}

fn can_learn(character: &Character, learning: Learning) -> bool {
    match learning {
        Learning::Spell(spell) => !character.known_spells().contains(&spell),
        Learning::OnAttackedReaction(reaction) => {
            !character.known_attacked_reactions.contains(&reaction)
        }
        Learning::OnHitReaction(reaction) => !character.known_on_hit_reactions.contains(&reaction),
        Learning::AttackEnhancement(enhancement) => {
            !character.known_attack_enhancements.contains(&enhancement)
        }
        Learning::SpellEnhancement(enhancement) => {
            character.knows_spell(enhancement.spell_id)
                && !character.known_spell_enhancements.contains(&enhancement)
        }
    }
}

fn apply_learning(learning: Learning, character: &mut Character) {
    match learning {
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
