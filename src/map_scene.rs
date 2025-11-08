use std::collections::HashMap;

use macroquad::{
    color::{Color, BLACK, GRAY, RED, WHITE, YELLOW},
    input::{is_mouse_button_pressed, mouse_position, MouseButton},
    math::Rect,
    miniquad::window::screen_size,
    shapes::{draw_circle, draw_circle_lines, draw_line, draw_rectangle_ex, DrawRectangleParams},
    text::{measure_text, Font, TextParams},
    texture::{draw_texture_ex, DrawTextureParams, Texture2D},
    time::get_frame_time,
    window::{clear_background, next_frame},
};
use rand::Rng;

use crate::{
    base_ui::draw_text_rounded,
    core::EquipmentEntry,
    data::{CHAIN_MAIL, DAGGER, LEATHER_ARMOR, RAPIER, SMALL_SHIELD, SWORD},
    drawing::draw_dashed_line,
    init_fight_map::FightId,
    textures::load_and_init_texture,
};

#[derive(Clone, Debug)]
struct Node {
    map_pos: (u32, u32),
    screen_pos: (f32, f32),
    text: &'static str,
    choice: MapChoice,
    texture: Option<Texture2D>,
}

impl Node {
    fn new(map_pos: (u32, u32), choice: MapChoice) -> Self {
        Self {
            map_pos,
            screen_pos: Default::default(),
            text: Default::default(),
            choice,
            texture: Default::default(),
        }
    }

    fn within_distance(&self, pos: (f32, f32), distance: f32) -> bool {
        (pos.0 - self.screen_pos.0).powf(2.0) + (pos.1 - self.screen_pos.1).powf(2.0)
            < distance.powf(2.0)
    }
}

#[derive(Copy, Clone, Debug)]
pub enum MapChoice {
    Rest,
    Shop,
    Fight(FightId),
    Chest(EquipmentEntry),
}

#[derive(Debug)]
pub struct MapScene {
    player_node_i: Option<usize>,
    visited_nodes: Vec<usize>,
}

impl Default for MapScene {
    fn default() -> Self {
        Self::new()
    }
}

impl MapScene {
    pub fn new() -> Self {
        Self {
            player_node_i: None,
            visited_nodes: vec![],
        }
    }

    pub async fn run_map_loop(&mut self, font: Font) -> MapChoice {
        let (screen_w, screen_h) = screen_size();
        let y_mid = screen_h / 2.0;
        let radius = 60.0;
        let mut selected_node_i = None;

        let fight_texture = load_and_init_texture("map_fight.png").await;
        let fight_elite_texture = load_and_init_texture("map_fight_elite.png").await;
        let rest_texture = load_and_init_texture("map_rest.png").await;
        let chest_texture = load_and_init_texture("map_chest.png").await;
        let shop_texture = load_and_init_texture("map_shop.png").await;

        let candidate_chest_rewards = vec![
            EquipmentEntry::Armor(CHAIN_MAIL),
            EquipmentEntry::Armor(LEATHER_ARMOR),
            EquipmentEntry::Weapon(DAGGER),
            EquipmentEntry::Weapon(SWORD),
            EquipmentEntry::Weapon(RAPIER),
            EquipmentEntry::Shield(SMALL_SHIELD),
        ];
        let mut rng = rand::rng();
        let chest_reward =
            candidate_chest_rewards[rng.random_range(..candidate_chest_rewards.len())];

        let mut nodes = [
            Node::new((0, 0), MapChoice::Fight(FightId::EasyCluster)),
            Node::new((0, 1), MapChoice::Fight(FightId::EasyPair)),
            Node::new((1, 0), MapChoice::Fight(FightId::EasyGuard)),
            Node::new((1, 1), MapChoice::Fight(FightId::EasyRiver)),
            Node::new((1, 2), MapChoice::Shop),
            Node::new((2, 0), MapChoice::Shop),
            Node::new((2, 1), MapChoice::Chest(chest_reward)),
            Node::new((2, 2), MapChoice::Fight(FightId::EliteOgre)),
            Node::new((3, 0), MapChoice::Rest),
            Node::new((3, 1), MapChoice::Chest(chest_reward)),
            Node::new((4, 0), MapChoice::Fight(FightId::EasySurrounded)),
            Node::new((5, 0), MapChoice::Fight(FightId::EliteMagi)),
        ];
        let edges: HashMap<Option<usize>, Vec<usize>> = [
            (None, vec![0, 1]),
            (Some(0), vec![2]),
            (Some(1), vec![3, 4]),
            (Some(2), vec![5]),
            (Some(3), vec![6]),
            (Some(4), vec![7]),
            (Some(5), vec![8]),
            (Some(6), vec![8]),
            (Some(7), vec![9]),
            (Some(8), vec![10]),
            (Some(9), vec![10]),
            (Some(10), vec![11]),
        ]
        .into();

        let mut column_sizes: HashMap<u32, u32> = HashMap::new();
        for node in &mut nodes {
            let n = column_sizes.get(&node.map_pos.0).copied().unwrap_or(0);
            column_sizes.insert(node.map_pos.0, n + 1);
            node.text = match node.choice {
                MapChoice::Rest => "Rest",
                MapChoice::Shop => "Shop",
                MapChoice::Fight(fight_id) => format!("{:?}", fight_id).leak(),
                MapChoice::Chest(..) => "Chest",
            };
            node.texture = match node.choice {
                MapChoice::Rest => Some(rest_texture.clone()),
                MapChoice::Fight(FightId::EliteOgre | FightId::EliteMagi) => {
                    Some(fight_elite_texture.clone())
                }
                MapChoice::Fight(..) => Some(fight_texture.clone()),
                MapChoice::Chest(..) => Some(chest_texture.clone()),
                MapChoice::Shop => Some(shop_texture.clone()),
            };
        }
        for node in &mut nodes {
            let col_size = column_sizes[&node.map_pos.0];
            let vert_dist = 200.0;
            let hor_dist = 190.0;
            node.screen_pos = (
                200.0 + node.map_pos.0 as f32 * hor_dist,
                y_mid - (col_size - 1) as f32 / 2.0 * vert_dist + node.map_pos.1 as f32 * vert_dist,
            )
        }

        let current_pos_color = Color::new(0.2, 0.0, 0.0, 1.0);

        let transition_duration = 0.3;
        let mut transition_countdown = None;

        let start_size = 30.0;
        let start_pos = Rect::new(
            100.0 - start_size / 2.0,
            y_mid - start_size / 2.0,
            start_size,
            start_size,
        );

        let bg_color = BLACK;
        let bg_color = Color::new(0.6, 0.5, 0.3, 1.0);

        loop {
            let elapsed = get_frame_time();

            let mouse_pos = mouse_position();
            clear_background(bg_color);

            let mut hovered_i = None;

            for (node_i, node) in nodes.iter().enumerate() {
                if let Some(valid_next) = edges.get(&self.player_node_i) {
                    if valid_next.contains(&node_i) && node.within_distance(mouse_pos, radius) {
                        hovered_i = Some(node_i);
                    }
                }
            }

            // Draw edges
            for (from_i, to) in &edges {
                let from_pos = match from_i {
                    Some(from_i) => nodes[*from_i].screen_pos,
                    None => start_pos.center().into(),
                };
                for to_i in to {
                    let to_node = &nodes[*to_i];

                    let visited_from = match from_i {
                        Some(from_i) => self.visited_nodes.contains(from_i),
                        None => true,
                    };

                    let line_color = if hovered_i == Some(*to_i) && self.player_node_i == *from_i {
                        WHITE
                    } else if visited_from && self.visited_nodes.contains(to_i) {
                        RED
                    } else {
                        BLACK
                    };
                    draw_dashed_line(from_pos, to_node.screen_pos, 2.0, line_color, 15.0, None);
                }
            }

            // Start position
            //draw_rectangle_lines(start_pos.x, start_pos.y, start_size, start_size, 2.0, RED);
            //if self.player_node_i.is_none() {
            //draw_cross(start_pos.center().into(), start_pos.w / 2.0 - 5.0);
            //}
            let fill_color = current_pos_color;
            draw_circle(start_pos.center().x, start_pos.center().y, 20.0, fill_color);
            let start_color = GRAY;
            draw_circle_lines(
                start_pos.center().x,
                start_pos.center().y,
                20.0,
                2.0,
                start_color,
            );

            // Draw nodes
            for (node_i, node) in nodes.iter().enumerate() {
                let hovered = hovered_i == Some(node_i);

                if transition_countdown.is_none()
                    && is_mouse_button_pressed(MouseButton::Left)
                    && hovered
                {
                    selected_node_i = Some(node_i);
                    transition_countdown = Some(transition_duration);
                }

                let outline_color = if self.player_node_i == Some(node_i) {
                    Some(GRAY)
                } else if selected_node_i == Some(node_i) {
                    Some(YELLOW)
                } else if self.visited_nodes.contains(&node_i) {
                    Some(GRAY)
                } else if hovered {
                    Some(WHITE)
                } else {
                    None
                };

                let fill_color = if self.visited_nodes.contains(&node_i) {
                    current_pos_color
                } else {
                    bg_color
                };

                draw_circle(node.screen_pos.0, node.screen_pos.1, radius, fill_color);
                if let Some(outline_color) = outline_color {
                    draw_circle_lines(
                        node.screen_pos.0,
                        node.screen_pos.1,
                        radius,
                        2.0,
                        outline_color,
                    );
                }

                if let Some(texture) = &node.texture {
                    let (w, h) = (64.0, 64.0);
                    draw_texture_ex(
                        texture,
                        node.screen_pos.0 - w / 2.0,
                        node.screen_pos.1 - h / 2.0,
                        WHITE,
                        DrawTextureParams {
                            dest_size: Some((w, h).into()),
                            ..Default::default()
                        },
                    );
                } else {
                    let font_size = 28;
                    let text_dim = measure_text(node.text, Some(&font), font_size, 1.0);

                    let mut text_color = BLACK;
                    if let Some(valid_next) = edges.get(&self.player_node_i) {
                        if valid_next.contains(&node_i) {
                            text_color = WHITE;
                        }
                    }

                    draw_text_rounded(
                        node.text,
                        node.screen_pos.0 - text_dim.width / 2.0,
                        node.screen_pos.1 + (text_dim.height) / 2.0,
                        TextParams {
                            font: Some(&font),
                            font_size,
                            color: text_color,
                            ..Default::default()
                        },
                    );
                }
            }

            // Transition to other scene
            if let Some(countdown) = &mut transition_countdown {
                *countdown -= elapsed;

                let color = Color::new(
                    0.0,
                    0.0,
                    0.0,
                    1.0 * (transition_duration - *countdown) / transition_duration,
                );
                let params = DrawRectangleParams {
                    offset: Default::default(),
                    rotation: 0.0,
                    color,
                };
                draw_rectangle_ex(0.0, 0.0, screen_w, screen_h, params);

                if *countdown < 0.0 {
                    let node_i = selected_node_i.unwrap();
                    self.player_node_i = Some(node_i);
                    self.visited_nodes.push(node_i);

                    // Make sure to show the last drawn frame
                    next_frame().await;
                    return nodes[selected_node_i.unwrap()].choice;
                }
            }

            next_frame().await;
        }
    }
}

fn draw_cross(pos: (f32, f32), w: f32) {
    let cross_thickness = 3.0;
    draw_line(
        pos.0 - w,
        pos.1 - w,
        pos.0 + w,
        pos.1 + w,
        cross_thickness,
        RED,
    );
    draw_line(
        pos.0 - w,
        pos.1 + w,
        pos.0 + w,
        pos.1 - w,
        cross_thickness,
        RED,
    );
}
