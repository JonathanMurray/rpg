use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    default,
    ops::Rem,
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
    time::{get_frame_time, get_time},
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
    util::{rgb, select_n_random},
};

pub async fn run_game_over_scene(
    resources: &GameResources,
    ui_resources: &UiResources,
    header: &'static str,
) {
    let simple_font = &resources.simple_font;

    let x_mid = screen_width() / 2.0;
    let y_mid = screen_height() / 2.0;

    //let bg = rgb(128, 61, 17);
    //let bg = rgb(122, 50, 67);
    //let bg = rgb(34, 39, 52);
    let bg = rgb(12, 64, 59);

    loop {
        clear_background(bg);

        let header_font = Some(&resources.big_font);
        let header_font_size = 24;

        let t = get_time() * 0.5;
        let t = (t - t.floor()) as f32;
        let y_max_offset = 20.0;
        let y_offset = if t < 0.5 {
            y_max_offset * t / 0.5
        } else {
            y_max_offset * (1.0 - (t - 0.5) / 0.5)
        };

        let font_scale = 1.0;

        let text_dim = measure_text(header, header_font, header_font_size, font_scale);

        draw_text_rounded(
            header,
            x_mid - text_dim.width / 2.0,
            y_mid + y_offset,
            TextParams {
                font: header_font,
                font_size: header_font_size,
                color: WHITE,
                font_scale,

                ..Default::default()
            },
        );

        let btn_text = "Restart";
        let font_size = 30;
        let margin = 25.0;
        let padding = 15.0;
        let text_dim = measure_text(btn_text, Some(&simple_font), font_size, 1.0);
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
            btn_text,
            rect.x + padding,
            rect.y + padding + text_dim.offset_y,
            TextParams {
                font: Some(&simple_font),
                font_size,
                color: YELLOW,
                ..Default::default()
            },
        );
        if rect.contains(mouse_position().into()) && is_mouse_button_pressed(MouseButton::Left) {
            break;
        }
        if is_key_pressed(macroquad::input::KeyCode::Space) {
            break;
        }

        next_frame().await;
    }
}
