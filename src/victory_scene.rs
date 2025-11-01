use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use macroquad::{
    color::{Color, BLACK, DARKGRAY, ORANGE, WHITE},
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
    base_ui::{Drawable, TextLine},
    core::{
        AttackEnhancement, BaseAction, Character, OnAttackedReaction, OnHitReaction, Spell,
        SpellEnhancement,
    },
    data::{
        BRACE, FIREBALL, HEAL, HEALING_NOVA, HEALING_RAIN, LUNGE_ATTACK, MIND_BLAST, QUICK, SCREAM,
        SHACKLED_MIND, SLASHING, SWEEP_ATTACK,
    },
    game_ui::{build_character_ui, UiState},
    game_ui_components::CharacterSheetToggle,
    textures::{EquipmentIconId, IconId},
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

pub async fn run_victory_loop(
    player_character: &Character,
    font: Font,
    equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
    icons: HashMap<IconId, Texture2D>,
) -> Reward {
    let (screen_w, screen_h) = screen_size();
    let x_mid = screen_w / 2.0;

    let transition_duration = 0.5;
    let mut transition_countdown = None;

    let event_queue = Rc::new(RefCell::new(vec![]));

    let player_character = Rc::new(player_character.clone());

    let mut next_button_id = 0;
    let mut character_ui = build_character_ui(
        equipment_icons,
        &icons,
        &event_queue,
        &font,
        &player_character,
        &mut next_button_id,
    );

    let attack_enhancement_context = "Attack";

    let mut rewards: Vec<(ButtonAction, Option<&'static str>)> = vec![
        (
            ButtonAction::AttackEnhancement(QUICK),
            Some(attack_enhancement_context),
        ),
        (
            ButtonAction::AttackEnhancement(SLASHING),
            Some(attack_enhancement_context),
        ),
    ];

    let known_spells = player_character.known_spells();

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
                rewards.push((
                    ButtonAction::SpellEnhancement(enhancement),
                    Some(spell.name),
                ));
            }
        }
    }

    let reward_buttons: Vec<RewardButton> = select_n_random(rewards, 3)
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
                    Some(Rc::clone(&player_character)),
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
    let mut selected_learning = None;

    let character_sheet_toggle = CharacterSheetToggle {
        shown: Cell::new(false),
        text_line: TextLine::new("Character sheet", 18, WHITE, Some(font.clone())),
        padding: 10.0,
    };

    let mut ui_state = UiState::Idle;

    let money_amount = 3;

    loop {
        let elapsed = get_frame_time();

        clear_background(BLACK);

        let text = "You receive";
        let font_size = 32;
        let text_dim = measure_text(text, Some(&font), font_size, 1.0);
        draw_text(
            text,
            screen_w / 2.0 - text_dim.width / 2.0,
            50.0 + (text_dim.height) / 2.0,
            font_size.into(),
            WHITE,
        );

        let card_w = 400.0;
        let card_color = Color::new(0.1, 0.1, 0.1, 1.00);

        draw_rectangle(x_mid - card_w / 2.0, 125.0, card_w, 60.0, card_color);

        let text = &format!("{} gold coins", money_amount);
        let font_size = 32;
        let text_dim = measure_text(text, Some(&font), font_size, 1.0);
        draw_text(
            text,
            screen_w / 2.0 - text_dim.width / 2.0,
            150.0 + (text_dim.height) / 2.0,
            font_size.into(),
            WHITE,
        );

        draw_rectangle(x_mid - card_w / 2.0, 220.0, card_w, 180.0, card_color);

        let text = "one skill";
        let font_size = 32;
        let text_dim = measure_text(text, Some(&font), font_size, 1.0);
        draw_text(
            text,
            screen_w / 2.0 - text_dim.width / 2.0,
            240.0 + (text_dim.height) / 2.0,
            font_size.into(),
            WHITE,
        );

        character_sheet_toggle.draw(x_mid - 60.0, 500.0);

        if character_sheet_toggle.shown.get() {
            let sheet_size = character_ui.character_sheet.container_size();
            let sheet_pos = (x_mid - sheet_size.0 / 2.0, 550.0);
            character_ui.character_sheet.screen_position.set(sheet_pos);
            let padding = 2.0;
            draw_rectangle(
                sheet_pos.0 - padding,
                sheet_pos.1 - padding,
                sheet_size.0 + padding * 2.0,
                sheet_size.1 + padding * 2.0,
                DARKGRAY,
            );
            character_ui.character_sheet.draw(&mut ui_state);
        }

        let mut btn_x = x_mid - row_w / 2.0;
        let btn_y = 320.0;

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
                        if transition_countdown.is_none() {
                            transition_countdown = Some(transition_duration);
                            btn.action_button.selected.set(ButtonSelected::Yes);
                            selected_learning = Some(action_to_reward_choice(btn_action));
                        }
                    }
                }
            }
        }

        if let Some((btn_id, btn_pos)) = hovered_btn {
            //character_ui.draw(y);
            let btn = reward_buttons
                .iter()
                .find(|btn| btn.action_button.id == btn_id)
                .map(|reward_btn| &reward_btn.action_button)
                .unwrap_or_else(|| {
                    character_ui
                        .hoverable_buttons
                        .iter()
                        .find(|btn| btn.id == btn_id)
                        .unwrap()
                });
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
                let learning = selected_learning.unwrap();
                return Reward {
                    learning,
                    money: money_amount,
                };
            }
        }

        next_frame().await;
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
