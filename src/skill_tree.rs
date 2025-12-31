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
    get_keys_pressed, is_key_down, is_key_pressed, is_key_released, is_mouse_button_down,
    is_mouse_button_pressed, is_mouse_button_released, mouse_position, mouse_wheel,
};
use macroquad::math::Rect;
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

use crate::action_button::{
    button_action_tooltip, draw_button_tooltip, draw_tooltip, ActionButton, ButtonAction,
    ButtonHovered, InternalUiEvent, Side, TooltipPositionPreference,
};
use crate::base_ui::{draw_text_rounded, Container, Drawable, Element, LayoutDirection, Style};
use crate::bot::{bot_choose_attack_reaction, bot_choose_hit_reaction};
use crate::chest_scene::run_chest_loop;
use crate::core::{
    Ability, Action, ArrowStack, AttackEnhancement, Attributes, BaseAction, Character, CharacterId,
    CharacterKind, Condition, CoreGame, EquipmentEntry, HandType, OnAttackedReaction,
    OnHitReaction, Party,
};

use crate::data::{
    PassiveSkill, ADRENALIN_POTION, ARCANE_POTION, BARBED_ARROWS, BONE_CRUSHER, BOW, BRACE,
    COLD_ARROWS, CRIPPLING_SHOT, DAGGER, EMPOWER, ENERGY_POTION, EXPLODING_ARROWS, FIREBALL,
    FIREBALL_INFERNO, FIREBALL_MASSIVE, FIREBALL_REACH, HEAL, HEALING_NOVA, HEALING_RAIN,
    HEALTH_POTION, INFLICT_WOUNDS, INFLICT_WOUNDS_NECROTIC_INFLUENCE, KILL, LEATHER_ARMOR,
    LONGER_REACH, LUNGE_ATTACK, LUNGE_ATTACK_HEAVY_IMPACT, LUNGE_ATTACK_REACH, MANA_POTION,
    MEDIUM_SHIELD, MIND_BLAST, OVERWHELMING, PENETRATING_ARROWS, QUICK, RAGE, ROBE, SCREAM,
    SCREAM_SHRIEK, SEARING_LIGHT, SEARING_LIGHT_BURN, SHACKLED_MIND, SHIRT, SIDE_STEP,
    SMALL_SHIELD, SMITE, SWEEP_ATTACK, SWEEP_ATTACK_PRECISE, SWORD,
};
use crate::drawing::{draw_dashed_line, draw_dashed_rectangle_lines};
use crate::game_ui::{PlayerChose, UiState, UserInterface};
use crate::game_ui_connection::GameUserInterfaceConnection;
use crate::map_scene::{MapChoice, MapScene};
use crate::rest_scene::run_rest_loop;
use crate::shop_scene::{generate_shop_contents, run_shop_loop};
use crate::textures::{
    load_all_equipment_icons, load_all_icons, load_all_portraits, load_all_sprites,
    load_and_init_texture, EquipmentIconId, IconId, PortraitId, SpriteId,
};
use crate::victory_scene::{run_victory_loop, Learning};
use serde::{Deserialize, Serialize};

async fn load_font(path: &str) -> Font {
    let path = format!("fonts/{path}");
    let mut font = load_ttf_font(&path).await.unwrap();
    font.set_filter(FilterMode::Nearest);
    font
}

const COLOR_STR: Color = Color::new(0.70, 0.1, 0.1, 1.00);
const COLOR_AGI: Color = Color::new(0.0, 0.6, 0.2, 1.0);
const COLOR_INT: Color = Color::new(0.6, 0.5, 0.1, 1.00);
const COLOR_SPI: Color = Color::new(0.00, 0.3, 0.8, 1.00);

const SAVE_FILE_NAME: &'static str = "skill_tree.json";

const CELL_WIDTHS: [f32; 5] = [40.0, 50.0, 60.0, 70.0, 80.0];

pub async fn run_editor() {
    // Seed the random numbers
    rand::srand(miniquad::date::now() as u64);

    // Without this, the window seems to start on a random position on the screen, sometimes with the bottom obscured
    set_window_position(100, 100);

    let font_path = "delicatus/Delicatus.ttf"; // <-- not bad! very thin and readable
    let font = load_font(font_path).await;

    let portraits = load_all_portraits().await;

    let icons = load_all_icons().await;

    let (screen_w, screen_h) = screen_size();

    let mut nodes: Vec<Rc<RefCell<Node>>> = Default::default();
    let mut edges: HashSet<(u32, u32)> = Default::default();

    let file_contents = fs::read(SAVE_FILE_NAME).expect("Reading from save file");
    let json: String = String::from_utf8(file_contents).expect("Valid file json");

    let grid_events = Rc::new(RefCell::new(vec![]));

    let skill_buttons: RefCell<HashMap<u32, ActionButton>> = Default::default();

    let skills = skills_mapping();

    let skill_to_btn_action = |skill| skills.iter().find(|(s, _)| s == &skill).unwrap().1;

    let mut next_node_id = 0;

    let mut create_node = |content, pos| {
        let n: Node = Node::new(next_node_id, content, pos);
        next_node_id += 1;

        if let NodeContent::Skill(skill) = &content {
            let mut btn = ActionButton::new(
                skill_to_btn_action(*skill),
                Some(Rc::clone(&grid_events)),
                n.id,
                &icons,
                None,
                &font,
            );
            btn.hover_border_color = LIGHTGRAY;

            skill_buttons.borrow_mut().insert(n.id, btn);
        }

        Rc::new(RefCell::new(n))
    };

    match serde_json::from_str::<FlatTree>(&json) {
        Ok(tree) => {
            let mut id_mappings: HashMap<u32, u32> = Default::default();
            nodes = tree
                .nodes
                .iter()
                .map(|node| {
                    let new_node = create_node(node.content, node.pos);

                    id_mappings.insert(node.id, new_node.borrow().id);
                    new_node
                    //Rc::new(RefCell::new(*node))
                })
                .collect();
            // The edges stored in the file may have had gaps between them, whereas our new nodes have
            // monotonically increasing IDs; so the edges must be updated or they would point completely
            // wrong
            edges = tree
                .edges
                .iter()
                .map(|(from, to)| (id_mappings[from], id_mappings[to]))
                .collect();
            //next_node_id = nodes.iter().map(|node| node.borrow().id).max().unwrap() + 1;
        }
        Err(e) => {
            println!("File contents: {}", json);
            println!("Failed to read from file: {:?}", e);

            nodes.push(create_node(NodeContent::Origin, (0, 0)));
            nodes.push(create_node(NodeContent::Attr(Attribute::Str), (1, 0)));
            edges.insert((0, 1));

            /*
            nodes.push(Rc::new(RefCell::new(Node::new(
                0,
                NodeContent::Origin,
                (0, 0),
            ))));
            nodes.push(Rc::new(RefCell::new(Node::new(
                1,
                NodeContent::Attr(Attribute::Str),
                (1, 0),
            ))));
            next_node_id = 2;
             */
        }
    }

    let mut abilities = vec![
        SWEEP_ATTACK,
        LUNGE_ATTACK,
        BRACE,
        SCREAM,
        SHACKLED_MIND,
        MIND_BLAST,
        INFLICT_WOUNDS,
        HEAL,
        SEARING_LIGHT,
        FIREBALL,
    ];

    let btn_action_to_skill = |btn_action| {
        skills
            .iter()
            .find(|(_, a)| a == &btn_action)
            .copied()
            .expect(&format!("converting to skill: {:?}", btn_action))
            .0
    };

    let mut passives = vec![
        PassiveSkill::HardenedSkin,
        PassiveSkill::WeaponProficiency,
        PassiveSkill::ArcaneSurge,
        PassiveSkill::Reaper,
        PassiveSkill::BloodRage,
        PassiveSkill::ThrillOfBattle,
        PassiveSkill::Honorless,
        PassiveSkill::Vigilant,
    ];

    let mut rows = vec![];

    let ui_events = Rc::new(RefCell::new(vec![]));

    let mut state = State::None;

    fn create_ui_row(elements: Vec<Element>) -> Element {
        Element::Container(Container {
            layout_dir: LayoutDirection::Horizontal,
            margin: 10.0,
            style: Style {
                background_color: Some(Color::new(0.1, 0.1, 0.1, 1.0)),
                padding: 5.0,
                ..Default::default()
            },
            children: elements,
            ..Default::default()
        })
    }
    let btn_id = 0;
    let create_action_btn = |btn_action| {
        Element::Box(Box::new(ActionButton::new(
            btn_action,
            Some(Rc::clone(&ui_events)),
            btn_id,
            &icons,
            None,
            &font,
        )))
    };

    const BUTTONS_PER_ROW: usize = 4;

    while !abilities.is_empty() {
        let ability_buttons = abilities
            .drain(0..BUTTONS_PER_ROW.min(abilities.len()))
            .map(|a| create_action_btn(ButtonAction::Action(BaseAction::UseAbility(a))))
            .collect();
        rows.push(create_ui_row(ability_buttons));
    }

    let attack_enhancements = vec![QUICK, SMITE, CRIPPLING_SHOT];
    let atk_enhancement_buttons = attack_enhancements
        .into_iter()
        .map(|e| create_action_btn(ButtonAction::AttackEnhancement(e)))
        .collect();
    rows.push(create_ui_row(atk_enhancement_buttons));

    let attacked_reactions = vec![SIDE_STEP];
    let atk_reaction_buttons = attacked_reactions
        .into_iter()
        .map(|r| create_action_btn(ButtonAction::OnAttackedReaction(r)))
        .collect();
    rows.push(create_ui_row(atk_reaction_buttons));

    let hit_reactions = vec![RAGE];
    let hit_reaction_buttons = hit_reactions
        .into_iter()
        .map(|r| create_action_btn(ButtonAction::OnHitReaction(r)))
        .collect();
    rows.push(create_ui_row(hit_reaction_buttons));

    let clicked_attributes = Rc::new(RefCell::new(vec![]));
    let attributes = [
        Attribute::Str,
        Attribute::Agi,
        Attribute::Int,
        Attribute::Spi,
    ];
    let attribute_buttons = attributes
        .into_iter()
        .map(|a| Element::Box(Box::new(AttributeButton::new(a, &clicked_attributes))))
        .collect();
    rows.push(create_ui_row(attribute_buttons));

    while !passives.is_empty() {
        let passive_buttons = passives
            .drain(0..BUTTONS_PER_ROW.min(passives.len()))
            .map(|p| create_action_btn(ButtonAction::Passive(p)))
            .collect();
        rows.push(create_ui_row(passive_buttons));
    }

    let ui = Container {
        layout_dir: LayoutDirection::Vertical,
        margin: 10.0,
        style: Style {
            background_color: Some(Color::new(0.1, 0.1, 0.1, 1.0)),
            border_color: Some(LIGHTGRAY),
            padding: 5.0,
            ..Default::default()
        },
        children: rows,
        ..Default::default()
    };

    let mut zoom_i = 2;

    let mut show_grid = true;

    let mut hovered_btn_id_pos = None;

    loop {
        let mid = (screen_w / 2.0, screen_h / 2.0);
        let (mouse_x, mouse_y) = mouse_position();

        let cell_w = CELL_WIDTHS[zoom_i];
        let icon_w = cell_w * 1.0;
        let origin_r = cell_w / 3.0;
        let attr_r = cell_w * 0.13;

        let mouse_grid_x: i32 = ((mouse_x - mid.0) / cell_w).round() as i32;
        let mouse_grid_y: i32 = ((mouse_y - mid.1) / cell_w).round() as i32;
        let mouse_grid_pos: (i32, i32) = (mouse_grid_x, mouse_grid_y);

        let min_x = -15;
        let max_x = 15;
        let min_y = -15;
        let max_y = 15;

        let is_mouse_within_grid =
            (min_x..=max_x).contains(&mouse_grid_x) && (min_y..=max_y).contains(&mouse_grid_y);

        let (_dx, dy) = mouse_wheel();
        if dy < 0.0 {
            zoom_i = zoom_i.saturating_sub(1);
        } else if dy > 0.0 {
            zoom_i = (zoom_i + 1).min(CELL_WIDTHS.len() - 1);
        }

        if is_mouse_button_pressed(MouseButton::Right) {
            state = State::None;
        } else if is_mouse_button_pressed(MouseButton::Left) && is_mouse_within_grid {
            if let State::PlacingAttribute(attr) = state {
                nodes.push(create_node(NodeContent::Attr(attr), mouse_grid_pos));
            } else if let State::PlacingSkill(skill) = state {
                nodes.push(create_node(NodeContent::Skill(skill), mouse_grid_pos));
                state = State::None;
            } else if matches!(state, State::DeletingNode) {
                let to_be_removed: Vec<u32> = nodes
                    .iter()
                    .filter(|node| {
                        node.borrow().pos() == mouse_grid_pos
                            && node.borrow().content != NodeContent::Origin
                    })
                    .map(|node| node.borrow().id)
                    .collect();

                nodes.retain(|n| !to_be_removed.contains(&n.borrow().id));
                edges.retain(|(a, b)| !to_be_removed.contains(a) && !to_be_removed.contains(b));
            } else if matches!(state, State::None) {
                if let Some(node) = nodes
                    .iter()
                    .find(|node| node.borrow().pos() == mouse_grid_pos)
                {
                    state = State::Dragging(Rc::clone(node));
                }
            } else if let State::EditingEdge(node) = &state {
                if let Some(clicked_node) = nodes
                    .iter()
                    .find(|node| node.borrow().pos == mouse_grid_pos)
                {
                    if let Some(from) = node {
                        if clicked_node.borrow().pos != from.borrow().pos {
                            let (a, b) = (from.borrow().id, clicked_node.borrow().id);

                            if a < b {
                                edges.insert((a, b));
                            } else if b < a {
                                edges.insert((b, a));
                            } else {
                                panic!("Cannot add edge from {} to {}", a, b);
                            }

                            state = State::EditingEdge(Some(Rc::clone(clicked_node)));
                        }
                    } else {
                        state = State::EditingEdge(Some(Rc::clone(clicked_node)));
                    }
                }
            }
        } else if is_key_pressed(KeyCode::G) {
            show_grid = !show_grid;
        } else if is_key_pressed(KeyCode::Space) {
            if matches!(state, State::DeletingNode) {
                state = State::None;
            } else {
                state = State::DeletingNode;
            }
        } else if is_key_pressed(KeyCode::S) {
            if is_key_down(KeyCode::LeftControl) {
                let tree = FlatTree {
                    nodes: nodes.iter().map(|node| node.borrow().clone()).collect(),
                    edges: edges.clone(),
                };
                let json = serde_json::to_string_pretty(&tree).unwrap();
                fs::write(SAVE_FILE_NAME, json).expect("Writing json to file");
                println!("Saved skill tree to {}", SAVE_FILE_NAME);
            }
        } else if is_key_pressed(KeyCode::LeftShift) {
            if matches!(state, State::EditingEdge(..)) {
                state = State::None;
            } else {
                state = State::EditingEdge(None);
            }
        } else if is_mouse_button_released(MouseButton::Left) {
            if matches!(state, State::Dragging(..)) {
                state = State::None;
            }
        } else if let State::Dragging(node) = &mut state {
            node.borrow_mut().pos = mouse_grid_pos;
        }

        clear_background(BLACK);
        if show_grid {
            draw_grid_lines(mid, cell_w, min_x, max_x, min_y, max_y);
        }

        for (from, to) in &edges {
            let node = nodes.iter().find(|n| n.borrow().id == *from).unwrap();
            let neighbor = nodes.iter().find(|n| n.borrow().id == *to).unwrap();

            draw_edge(mid, cell_w, &node.borrow(), &neighbor.borrow(), 1.0, GOLD);
        }

        for node in &nodes {
            let x = mid.0 + node.borrow().pos().0 as f32 * cell_w;
            let y = mid.1 + node.borrow().pos().1 as f32 * cell_w;
            match node.borrow().content {
                NodeContent::Origin => {
                    draw_origin_node(&portraits, cell_w, origin_r, x, y);
                }
                NodeContent::Attr(attribute) => {
                    draw_attr_node(attr_r, x, y, attribute);
                }
                NodeContent::Skill(..) => {
                    let mut skill_buttons = skill_buttons.borrow_mut();
                    let btn = skill_buttons.get_mut(&node.borrow().id).unwrap();
                    draw_skill_node2(icon_w, x, y, btn);
                }
            }
        }

        ui.draw(5.0, 5.0);

        let mut text = None;

        if let State::PlacingAttribute(attribute) = state {
            let size = (32.0, 32.0);

            draw_rectangle(mouse_x, mouse_y, size.0, size.1, attribute.color());

            text = Some(format!("PLACING: {:?}", attribute));
        } else if let State::PlacingSkill(skill) = state {
            let btn_action = skill_to_btn_action(skill);
            let texture = &icons[&btn_action.icon(None)];
            let size = (32.0, 32.0);
            draw_texture_ex(
                texture,
                mouse_x,
                mouse_y,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(size.into()),
                    ..Default::default()
                },
            );
            draw_dashed_rectangle_lines(mouse_x, mouse_y, size.0, size.1, 1.0, WHITE, 5.0);

            text = Some(format!("PLACING: {}", btn_action.name()));
        } else if matches!(state, State::DeletingNode) {
            text = Some("DELETING".to_string());
        } else if let State::Dragging(..) = &state {
            text = Some("DRAGGING".to_string());
        } else if let State::EditingEdge(from) = &state {
            text = Some("ADDING EDGE".to_string());
            if let Some(node) = from {
                let x1 = mid.0 + node.borrow().pos.0 as f32 * cell_w;
                let y1 = mid.1 + node.borrow().pos.1 as f32 * cell_w;
                let x2 = mid.0 + mouse_grid_x as f32 * cell_w;
                let y2 = mid.1 + mouse_grid_y as f32 * cell_w;
                draw_dashed_line((x1, y1), (x2, y2), 1.0, LIGHTGRAY, 5.0, None);
                //draw_line(x1, y1, x2, y2, 1.0, GREEN);
            }
        }

        if let Some(text) = &text {
            let font_size = 32;
            let text_dim = measure_text(&text, Some(&font), font_size, 1.0);
            draw_text_ex(
                &text,
                mid.0 - text_dim.width / 2.0,
                40.0,
                TextParams {
                    font: Some(&font),
                    font_size,
                    color: WHITE,
                    ..Default::default()
                },
            );
        }

        for event in grid_events.borrow_mut().drain(..) {
            match event {
                InternalUiEvent::ButtonHovered(ButtonHovered {
                    id,
                    action,
                    hovered_pos,
                    ..
                }) => {
                    if let Some(pos) = hovered_pos {
                        hovered_btn_id_pos = Some((id, pos));
                    } else {
                        if let Some(already_hovered) = hovered_btn_id_pos {
                            if already_hovered.0 == id {
                                hovered_btn_id_pos = None;
                            }
                        }
                    }
                }
                InternalUiEvent::ButtonClicked { id, .. } => {
                    dbg!("grid button CLICKED", id);
                }
            }
        }

        for event in ui_events.borrow_mut().drain(..) {
            match event {
                InternalUiEvent::ButtonHovered { .. } => {
                    //dbg!("HOVERED", hovered_pos);
                }
                InternalUiEvent::ButtonClicked { action, .. } => {
                    state = State::PlacingSkill(btn_action_to_skill(action));
                }
            }
        }

        if let Some((btn_id, pos)) = hovered_btn_id_pos {
            let btn = &skill_buttons.borrow()[&btn_id];
            draw_button_tooltip(&font, pos, &btn.tooltip(), true);
        }

        for attribute in clicked_attributes.borrow_mut().drain(..) {
            state = State::PlacingAttribute(attribute)
        }

        next_frame().await;
    }
}

fn draw_edge(mid: (f32, f32), cell_w: f32, from: &Node, to: &Node, thickness: f32, color: Color) {
    let x1 = mid.0 + from.pos.0 as f32 * cell_w;
    let y1 = mid.1 + from.pos.1 as f32 * cell_w;
    let x2 = mid.0 + to.pos.0 as f32 * cell_w;
    let y2 = mid.1 + to.pos.1 as f32 * cell_w;

    draw_line(x1, y1, x2, y2, thickness, color);
}

pub async fn run_skill_tree_scene() {
    let font_path = "delicatus/Delicatus.ttf"; // <-- not bad! very thin and readable
    let font = load_font(font_path).await;

    let portraits = load_all_portraits().await;

    let icons = load_all_icons().await;

    let (screen_w, screen_h) = screen_size();

    let nodes: Vec<Rc<RefCell<Node>>>;
    let edges: HashSet<(u32, u32)>;

    let file_contents = fs::read(SAVE_FILE_NAME).expect("Reading from save file");
    let json: String = String::from_utf8(file_contents).expect("Valid file json");

    match serde_json::from_str::<FlatTree>(&json) {
        Ok(tree) => {
            nodes = tree
                .nodes
                .iter()
                .map(|node| Rc::new(RefCell::new(*node)))
                .collect();
            edges = tree.edges;
        }
        Err(e) => {
            println!("File contents: {}", json);
            panic!("Failed to read from file: {:?}", e);
        }
    }

    let grid_events = Rc::new(RefCell::new(vec![]));

    let skills = skills_mapping();

    let skill_to_btn_action = |skill| skills.iter().find(|(s, _)| s == &skill).unwrap().1;

    let mut zoom_i = 1;

    let mut show_grid = false;

    let mut claimed_nodes: HashSet<u32> = [0, 1].into();

    let mut dragging_camera_from = None;
    let mut camera_pos = (0.0, 0.0);

    loop {
        let mid = (screen_w / 2.0 + camera_pos.0, screen_h / 2.0 + camera_pos.1);

        let (mouse_x, mouse_y) = mouse_position();

        let cell_w = CELL_WIDTHS[zoom_i];
        let icon_w = cell_w * 1.0;
        let attr_r = cell_w * 0.13;

        let mouse_grid_x: i32 = ((mouse_x - mid.0) / cell_w).round() as i32;
        let mouse_grid_y: i32 = ((mouse_y - mid.1) / cell_w).round() as i32;
        let mouse_grid_pos: (i32, i32) = (mouse_grid_x, mouse_grid_y);

        let min_x = -15;
        let max_x = 15;
        let min_y = -15;
        let max_y = 15;

        let is_mouse_within_grid =
            (min_x..=max_x).contains(&mouse_grid_x) && (min_y..=max_y).contains(&mouse_grid_y);

        let (_dx, dy) = mouse_wheel();
        if dy < 0.0 {
            zoom_i = zoom_i.saturating_sub(1);
        } else if dy > 0.0 {
            zoom_i = (zoom_i + 1).min(CELL_WIDTHS.len() - 1);
        }

        clear_background(BLACK);

        if show_grid {
            draw_grid_lines(mid, cell_w, min_x, max_x, min_y, max_y);
        }

        let mut claimable = vec![];

        let hovered_node_id: Option<u32> = nodes
            .iter()
            .find(|node| {
                node.borrow().pos() == mouse_grid_pos
                    && node.borrow().content != NodeContent::Origin
            })
            .map(|node| node.borrow().id);

        for (from, to) in &edges {
            let node = nodes.iter().find(|n| n.borrow().id == *from).unwrap();
            let neighbor = nodes.iter().find(|n| n.borrow().id == *to).unwrap();

            let color;
            let thickness;
            let node = node.borrow();
            let claimed_node = claimed_nodes.contains(&node.id);
            let neighbor = neighbor.borrow();
            let claimed_neighbor = claimed_nodes.contains(&neighbor.id);
            if claimed_node && claimed_neighbor {
                color = GOLD;
                thickness = 5.0;
            } else if claimed_node || claimed_neighbor {
                if (hovered_node_id == Some(node.id) && !claimed_nodes.contains(&node.id))
                    || (hovered_node_id == Some(neighbor.id)
                        && !claimed_nodes.contains(&neighbor.id))
                {
                    color = WHITE;
                    thickness = 2.0;
                } else {
                    color = DARKGRAY;
                    thickness = 1.0;
                }

                if claimed_node {
                    claimable.push(neighbor.id);
                } else {
                    claimable.push(node.id);
                }
            } else {
                color = DARKGRAY;
                thickness = 1.0;
            }

            draw_edge(mid, cell_w, &node, &neighbor, thickness, color);
        }

        let mut text = None;

        if is_mouse_button_pressed(MouseButton::Left) && is_mouse_within_grid {
            if let Some(id) = hovered_node_id {
                if claimable.contains(&id) {
                    claimed_nodes.insert(id);
                }
            }
        } else if is_key_pressed(KeyCode::G) {
            show_grid = !show_grid;
        } else if is_mouse_button_pressed(MouseButton::Right) {
            dragging_camera_from = Some((mouse_x, mouse_y));
        } else if is_mouse_button_down(MouseButton::Right) {
            if let Some((from_x, from_y)) = dragging_camera_from {
                let dx = mouse_x - from_x;
                let dy = mouse_y - from_y;
                let min_allowed = -250.0;
                let max_allowed = 250.0;
                camera_pos = (
                    (camera_pos.0 + dx).max(min_allowed).min(max_allowed),
                    (camera_pos.1 + dy).max(min_allowed).min(max_allowed),
                );
                dragging_camera_from = Some((mouse_x, mouse_y));
            }
        } else {
            dragging_camera_from = None;
        }

        for node in &nodes {
            let node = node.borrow();
            let x = mid.0 + node.pos().0 as f32 * cell_w;
            let y = mid.1 + node.pos().1 as f32 * cell_w;
            match node.content {
                NodeContent::Origin => {
                    let texture = &portraits[&PortraitId::Alice];
                    let x = x - icon_w / 2.0;
                    let y = y - icon_w / 2.0;
                    draw_rectangle(x, y, cell_w, cell_w, DARKGRAY);

                    draw_texture_ex(
                        texture,
                        x,
                        y,
                        WHITE,
                        DrawTextureParams {
                            dest_size: Some((cell_w, cell_w).into()),
                            ..Default::default()
                        },
                    );

                    draw_rectangle_lines(x, y, icon_w, icon_w, 5.0, WHITE);
                }
                NodeContent::Attr(attribute) => {
                    draw_attr_node(attr_r, x, y, attribute);

                    if claimed_nodes.contains(&node.id) {
                        draw_circle_lines(x, y, attr_r, 4.0, GOLD);
                    } else if hovered_node_id == Some(node.id) && claimable.contains(&node.id) {
                        draw_circle_lines(x, y, attr_r, 2.0, WHITE);
                    } else if hovered_node_id == Some(node.id) {
                        draw_circle_lines(x, y, attr_r, 1.0, LIGHTGRAY);
                    }
                }
                NodeContent::Skill(skill) => {
                    draw_skill_node(
                        &icons,
                        &grid_events,
                        skill_to_btn_action,
                        icon_w,
                        x,
                        y,
                        skill,
                        &font,
                    );

                    if claimed_nodes.contains(&node.id) {
                        let x = x - icon_w / 2.0;
                        let y = y - icon_w / 2.0;
                        draw_rectangle_lines(x, y, icon_w, icon_w, 5.0, WHITE);
                    }
                }
            }
        }

        for event in grid_events.borrow_mut().drain(..) {
            match event {
                InternalUiEvent::ButtonHovered(ButtonHovered {
                    id,
                    action: button_action,
                    hovered_pos,
                    ..
                }) => {
                    if let Some(pos) = hovered_pos {
                        let rect = Rect::new(pos.0, pos.1, cell_w, cell_w);
                        let tooltip = button_action_tooltip(&button_action);
                        draw_tooltip(
                            &font,
                            TooltipPositionPreference::RelativeToRect(rect, Side::Bottom),
                            &tooltip.header,
                            tooltip.error,
                            &tooltip.content_lines(true),
                            &tooltip.keywords,
                            false,
                        );
                    }
                }
                InternalUiEvent::ButtonClicked { id, .. } => {
                    dbg!("grid button CLICKED", id);
                }
            }
        }

        if let Some(id) = hovered_node_id {
            let node = nodes.iter().find(|n| n.borrow().id == id).unwrap().borrow();
            match node.content {
                NodeContent::Origin => {}
                NodeContent::Attr(attribute) => {
                    let (header, description) = attribute.tooltip();
                    draw_tooltip(
                        &font,
                        TooltipPositionPreference::At((mouse_x + 20.0, mouse_y)),
                        header,
                        None,
                        &[description.to_string()],
                        &[],
                        false,
                    );
                    if claimable.contains(&id) {
                        text = Some("Click to allocate attribute point");
                    }
                }
                NodeContent::Skill(_skill) => {
                    if claimable.contains(&id) {
                        text = Some("Click to acquire skill");
                    }
                }
            }
        }

        if let Some(text) = &text {
            let font_size = 32;
            let text_dim = measure_text(&text, Some(&font), font_size, 1.0);
            draw_text_ex(
                &text,
                mid.0 - text_dim.width / 2.0,
                40.0,
                TextParams {
                    font: Some(&font),
                    font_size,
                    color: WHITE,
                    ..Default::default()
                },
            );
        }

        next_frame().await;
    }
}

fn draw_skill_node2(icon_w: f32, x: f32, y: f32, btn: &mut ActionButton) {
    btn.change_scale(icon_w / 60.0);
    let x = x - btn.size.0 / 2.0;
    let y = y - btn.size.1 / 2.0;
    btn.draw(x, y);
}

fn draw_skill_node(
    icons: &HashMap<IconId, Texture2D>,
    grid_events: &Rc<RefCell<Vec<InternalUiEvent>>>,
    skill_to_btn_action: impl Fn(Skill) -> ButtonAction,
    icon_w: f32,
    x: f32,
    y: f32,
    skill: Skill,
    font: &Font,
) {
    let mut btn = ActionButton::new(
        skill_to_btn_action(skill),
        Some(Rc::clone(grid_events)),
        0,
        icons,
        None,
        font,
    );
    btn.change_scale(icon_w / 60.0);
    btn.hover_border_color = LIGHTGRAY;
    let x = x - btn.size.0 / 2.0;
    let y = y - btn.size.1 / 2.0;
    btn.draw(x, y);
}

fn draw_attr_node(attr_r: f32, x: f32, y: f32, attribute: Attribute) {
    let color = match attribute {
        Attribute::Str => COLOR_STR,
        Attribute::Agi => COLOR_AGI,
        Attribute::Int => COLOR_INT,
        Attribute::Spi => COLOR_SPI,
    };
    draw_circle_lines(x, y, attr_r, 1.0, GRAY);
    draw_circle(x, y, attr_r, color);
}

fn draw_origin_node(
    portraits: &HashMap<PortraitId, Texture2D>,
    cell_w: f32,
    origin_r: f32,
    x: f32,
    y: f32,
) {
    draw_circle(x, y, origin_r, DARKGRAY);
    draw_circle_lines(x, y, origin_r, 1.0, LIGHTGRAY);
    let texture = &portraits[&PortraitId::Alice];
}

fn draw_grid_lines(mid: (f32, f32), cell_w: f32, min_x: i32, max_x: i32, min_y: i32, max_y: i32) {
    let grid_color = Color::new(0.1, 0.1, 0.1, 1.00);

    for x in min_x..=max_x + 1 {
        let x0 = mid.0 + (x as f32 - 0.5) * cell_w;
        let y1 = mid.1 + (min_y as f32 - 0.5) * cell_w;
        let y2 = mid.1 + (max_y as f32 + 0.5) * cell_w;
        draw_line(x0, y1, x0, y2, 1.0, grid_color);
    }
    for y in min_y..=max_y + 1 {
        let y0 = mid.1 + (y as f32 - 0.5) * cell_w;
        let x1 = mid.0 + (min_x as f32 - 0.5) * cell_w;
        let x2 = mid.0 + (max_x as f32 + 0.5) * cell_w;
        draw_line(x1, y0, x2, y0, 1.0, grid_color);
    }
}

fn skills_mapping() -> Vec<(Skill, ButtonAction)> {
    {
        let ability = |a| ButtonAction::Action(BaseAction::UseAbility(a));
        let atk_enhancement = |e| ButtonAction::AttackEnhancement(e);
        let passive = |p| ButtonAction::Passive(p);
        let atk_react = |r| ButtonAction::OnAttackedReaction(r);
        let atk_hit = |r| ButtonAction::OnHitReaction(r);
        use PassiveSkill::*;
        [
            (Skill::SweepAttack, ability(SWEEP_ATTACK)),
            (Skill::LungeAttack, ability(LUNGE_ATTACK)),
            (Skill::Brace, ability(BRACE)),
            (Skill::Scream, ability(SCREAM)),
            (Skill::ShackledMind, ability(SHACKLED_MIND)),
            (Skill::MindBlast, ability(MIND_BLAST)),
            (Skill::InflictWounds, ability(INFLICT_WOUNDS)),
            (Skill::Heal, ability(HEAL)),
            (Skill::SearingLight, ability(SEARING_LIGHT)),
            (Skill::Fireball, ability(FIREBALL)),
            (Skill::Quick, atk_enhancement(QUICK)),
            (Skill::Smite, atk_enhancement(SMITE)),
            (Skill::CripplingShot, atk_enhancement(CRIPPLING_SHOT)),
            (Skill::HardenedSkin, passive(HardenedSkin)),
            (Skill::WeaponProficiency, passive(WeaponProficiency)),
            (Skill::ArcaneSurge, passive(ArcaneSurge)),
            (Skill::Reaper, passive(Reaper)),
            (Skill::BloodRage, passive(BloodRage)),
            (Skill::ThrillOfBattle, passive(ThrillOfBattle)),
            (Skill::Honorless, passive(Honorless)),
            (Skill::Vigilant, passive(Vigilant)),
            (Skill::Sidestep, atk_react(SIDE_STEP)),
            (Skill::Rage, atk_hit(RAGE)),
        ]
    }
    .to_vec()
}

#[derive(Debug)]
enum State {
    None,
    PlacingAttribute(Attribute),
    PlacingSkill(Skill),
    DeletingNode,
    Dragging(Rc<RefCell<Node>>),
    EditingEdge(Option<Rc<RefCell<Node>>>),
}

type Pos = (i32, i32);

#[derive(Serialize, Deserialize, Debug)]
struct FlatTree {
    nodes: Vec<Node>,
    edges: HashSet<(u32, u32)>,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
struct Node {
    id: u32,
    content: NodeContent,
    pos: Pos,
}

impl Node {
    fn new(id: u32, content: NodeContent, pos: Pos) -> Self {
        Self { id, content, pos }
    }

    fn pos(&self) -> Pos {
        self.pos
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
enum NodeContent {
    Origin,
    Attr(Attribute),
    Skill(Skill),
}

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
enum Skill {
    SweepAttack,
    LungeAttack,
    Brace,
    Scream,
    ShackledMind,
    MindBlast,
    InflictWounds,
    Heal,
    SearingLight,
    Fireball,
    Quick,
    Smite,
    CripplingShot,
    HardenedSkin,
    WeaponProficiency,
    ArcaneSurge,
    Reaper,
    BloodRage,
    ThrillOfBattle,
    Honorless,
    Vigilant,
    Sidestep,
    Rage,
}

#[derive(Debug, PartialEq, Clone, Copy, Serialize, Deserialize)]
enum Attribute {
    Str,
    Agi,
    Int,
    Spi,
}

impl Attribute {
    fn color(&self) -> Color {
        match self {
            Attribute::Str => COLOR_STR,
            Attribute::Agi => COLOR_AGI,
            Attribute::Int => COLOR_INT,
            Attribute::Spi => COLOR_SPI,
        }
    }

    fn tooltip(&self) -> (&'static str, &'static str) {
        match self {
            Attribute::Str => (
                "Strength",
                "Max health, Toughness, Stamina, carrying capacity, attack modifier",
            ),
            Attribute::Agi => ("Agility", "Move speed, Evasion, Stamina, attack modifier"),
            Attribute::Int => (
                "Intellect",
                "Will, Evasion, attack modifier, spell modifier",
            ),
            Attribute::Spi => ("Spirit", "Max mana, spell modifier"),
        }
    }
}

struct AttributeButton {
    attr: Attribute,
    clicked_events: Rc<RefCell<Vec<Attribute>>>,
}

impl AttributeButton {
    fn new(attr: Attribute, clicked_events: &Rc<RefCell<Vec<Attribute>>>) -> Self {
        Self {
            attr,
            clicked_events: Rc::clone(clicked_events),
        }
    }
}

impl Drawable for AttributeButton {
    fn draw(&self, x: f32, y: f32) {
        let (w, h) = self.size();
        draw_rectangle(x, y, w, h, self.attr.color());

        let (mx, my) = mouse_position();
        if (x..x + w).contains(&mx) && (y..y + h).contains(&my) {
            draw_rectangle_lines(x, y, w, h, 4.0, WHITE);

            if is_mouse_button_pressed(MouseButton::Left) {
                self.clicked_events.borrow_mut().push(self.attr);
            }
        } else {
            draw_rectangle_lines(x, y, w, h, 1.0, LIGHTGRAY);
        }
    }

    fn size(&self) -> (f32, f32) {
        (64.0, 64.0)
    }
}
