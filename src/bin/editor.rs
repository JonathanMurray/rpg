use std::cell::{self, Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::ops::Index;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicBool, Ordering};

use macroquad::color::{
    Color, BLUE, BROWN, DARKGRAY, GOLD, GRAY, GREEN, LIGHTGRAY, MAGENTA, RED, SKYBLUE, WHITE,
    YELLOW,
};
use macroquad::input::{
    get_keys_pressed, is_key_down, is_key_pressed, is_key_released, is_mouse_button_pressed,
    is_mouse_button_released, mouse_position, mouse_wheel,
};
use macroquad::miniquad::window::{self, screen_size, set_window_position, set_window_size};
use macroquad::miniquad::{KeyCode, MouseButton};

use macroquad::shapes::{
    draw_circle, draw_circle_lines, draw_line, draw_rectangle, draw_rectangle_lines,
};
use macroquad::text::{
    draw_text, draw_text_ex, load_ttf_font, measure_text, Font, TextDimensions, TextParams,
};
use macroquad::texture::{draw_texture, draw_texture_ex, DrawTextureParams, FilterMode, Texture2D};
use macroquad::ui::widgets::Button;
use macroquad::window::next_frame;
use macroquad::{
    color::BLACK,
    miniquad,
    rand::{self},
    time::get_frame_time,
    window::{clear_background, Conf},
};

use rpg::action_button::{draw_button_tooltip, ActionButton, ButtonAction, InternalUiEvent};
use rpg::base_ui::{draw_text_rounded, Container, Drawable, Element, LayoutDirection, Style};
use rpg::bot::{bot_choose_attack_reaction, bot_choose_hit_reaction};
use rpg::chest_scene::run_chest_loop;
use rpg::core::{
    Ability, Action, ArrowStack, AttackEnhancement, Attributes, BaseAction, Character, CharacterId,
    CharacterKind, Condition, CoreGame, EquipmentEntry, HandType, OnAttackedReaction,
    OnHitReaction, Party,
};

use rpg::data::{
    PassiveSkill, ADRENALIN_POTION, ARCANE_POTION, BARBED_ARROWS, BONE_CRUSHER, BOW, BRACE,
    COLD_ARROWS, CRIPPLING_SHOT, DAGGER, EMPOWER, ENERGY_POTION, EXPLODING_ARROWS, FIREBALL,
    FIREBALL_INFERNO, FIREBALL_MASSIVE, FIREBALL_REACH, HEAL, HEALING_NOVA, HEALING_RAIN,
    HEALTH_POTION, INFLICT_WOUNDS, INFLICT_WOUNDS_NECROTIC_INFLUENCE, KILL, LEATHER_ARMOR,
    LONGER_REACH, LUNGE_ATTACK, LUNGE_ATTACK_HEAVY_IMPACT, LUNGE_ATTACK_REACH, MANA_POTION,
    MEDIUM_SHIELD, MIND_BLAST, OVERWHELMING, PENETRATING_ARROWS, QUICK, RAGE, ROBE, SCREAM,
    SCREAM_SHRIEK, SEARING_LIGHT, SEARING_LIGHT_BURN, SHACKLED_MIND, SHIRT, SIDE_STEP,
    SMALL_SHIELD, SMITE, SWEEP_ATTACK, SWEEP_ATTACK_PRECISE, SWORD,
};
use rpg::drawing::{draw_dashed_line, draw_dashed_rectangle_lines};
use rpg::game_ui::{PlayerChose, UiState, UserInterface};
use rpg::game_ui_connection::GameUserInterfaceConnection;
use rpg::map_scene::{MapChoice, MapScene};
use rpg::rest_scene::run_rest_loop;
use rpg::shop_scene::{generate_shop_contents, run_shop_loop};
use rpg::skill_tree::run_editor;
use rpg::textures::{
    load_all_equipment_icons, load_all_icons, load_all_portraits, load_all_sprites,
    load_and_init_texture, EquipmentIconId, IconId, PortraitId, SpriteId,
};
use rpg::victory_scene::{run_victory_loop, Learning};
use serde::{Deserialize, Serialize};

async fn load_font(path: &str) -> Font {
    let path = format!("fonts/{path}");
    let mut font = load_ttf_font(&path).await.unwrap();
    font.set_filter(FilterMode::Nearest);
    font
}

#[macroquad::main(window_conf)]
async fn main() {
    run_editor().await;
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Editor".to_owned(),
        window_width: 1920,
        //window_height: 960,
        window_height: 1200,
        high_dpi: true,

        window_resizable: false,
        ..Default::default()
    }
}
