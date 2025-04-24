use std::cell::Cell;
use std::{collections::HashMap, rc::Rc};

use macroquad::color::{DARKBLUE, DARKGRAY, SKYBLUE};

use macroquad::input::{
    is_mouse_button_down, is_mouse_button_pressed, mouse_position, MouseButton,
};
use macroquad::shapes::{draw_rectangle, draw_rectangle_lines};
use macroquad::window::{screen_height, screen_width};
use macroquad::{
    color::{Color, BLACK, LIGHTGRAY, WHITE},
    text::Font,
    texture::Texture2D,
};

use crate::drawing::draw_cross;
use crate::equipment_ui::build_inventory_section;
use crate::{
    action_button::ActionButton,
    base_ui::{Align, Container, ContainerScroll, Element, LayoutDirection, Style, TextLine},
    core::Character,
    equipment_ui::build_equipped_section,
    stats_ui::{build_stats_table, StatValue},
    textures::EquipmentIconId,
};

pub struct CharacterSheet {
    container: Container,
    close_button_rect: (f32, f32, f32, f32),
    top_bar_h: f32,
    screen_position: Cell<(f32, f32)>,
    dragged_offset: Cell<Option<(f32, f32)>>,
}

impl CharacterSheet {
    pub fn new(
        font: &Font,
        character: &Character,
        equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
        attack_button: Option<Rc<ActionButton>>,
        reaction_buttons: Vec<Rc<ActionButton>>,
        attack_enhancement_buttons: Vec<Rc<ActionButton>>,
        spell_buttons: Vec<(Rc<ActionButton>, Vec<Rc<ActionButton>>)>,
    ) -> Self {
        let stats_table = build_stats_table(
            font,
            20,
            &[
                (
                    Some(("Strength", character.base_attributes.strength)),
                    &[
                        ("Health", StatValue::U32(character.health.max)),
                        ("Toughness", StatValue::U32(character.toughness())),
                        ("Capacity", StatValue::U32(character.capacity)),
                    ],
                ),
                (None, &[("Stamina", StatValue::U32(character.stamina.max))]),
                (
                    Some(("Agility", character.base_attributes.agility)),
                    &[("Movement", StatValue::F32(character.move_speed))],
                ),
                (None, &[("Evasion", StatValue::U32(character.evasion()))]),
                (
                    Some(("Intellect", character.base_attributes.intellect)),
                    &[
                        ("Will", StatValue::U32(character.will())),
                        (
                            "Reaction AP",
                            StatValue::String(format!("{}", character.max_reactive_action_points)),
                        ),
                    ],
                ),
                (
                    None,
                    &[(
                        "Spell mod",
                        StatValue::String(format!("+{}", character.spell_modifier())),
                    )],
                ),
                (
                    Some(("Spirit", character.base_attributes.spirit)),
                    &[("Mana", StatValue::U32(character.mana.max))],
                ),
            ],
        );

        let inventory_section = build_inventory_section(font, character, equipment_icons);

        let equipped_section = build_equipped_section(font, character, equipment_icons);

        let mut spell_book_rows = Container {
            layout_dir: LayoutDirection::Vertical,
            margin: 5.0,
            children: vec![],
            scroll: Some(ContainerScroll::new(40.0)),
            max_height: Some(450.0),
            style: Style {
                padding: 10.0,
                border_color: Some(LIGHTGRAY),
                ..Default::default()
            },
            ..Default::default()
        };

        if !reaction_buttons.is_empty() {
            let row = buttons_row(
                reaction_buttons
                    .into_iter()
                    .map(|btn| Element::Rc(btn))
                    .collect(),
            );

            spell_book_rows.children.push(Element::Text(TextLine::new(
                "Reactions",
                16,
                WHITE,
                Some(font.clone()),
            )));
            spell_book_rows.children.push(row);
        }

        if let Some(attack_button) = attack_button {
            spell_book_rows.children.push(Element::Text(TextLine::new(
                "Attack",
                16,
                WHITE,
                Some(font.clone()),
            )));

            let mut buttons = attack_enhancement_buttons;
            buttons.insert(0, attack_button);

            let row = buttons_row(buttons.into_iter().map(|btn| Element::Rc(btn)).collect());

            spell_book_rows.children.push(row);
        }

        for (spell_btn, enhancement_buttons) in spell_buttons.into_iter() {
            let spell = spell_btn.action.unwrap_spell();
            spell_book_rows.children.push(Element::Text(TextLine::new(
                spell.name,
                16,
                WHITE,
                Some(font.clone()),
            )));

            let mut row_buttons = vec![spell_btn.clone()];
            for enhancement_btn in enhancement_buttons {
                row_buttons.push(Rc::clone(&enhancement_btn));
            }
            let spell_row = buttons_row(
                row_buttons
                    .into_iter()
                    .map(|btn| Element::Rc(btn))
                    .collect(),
            );
            spell_book_rows.children.push(spell_row);
        }

        let container = Container {
            layout_dir: LayoutDirection::Vertical,
            align: Align::Center,
            border_between_children: Some(LIGHTGRAY),
            style: Style {
                padding: 3.0,
                background_color: Some(BLACK),
                ..Default::default()
            },
            children: vec![
                Element::Text(
                    TextLine::new(character.name, 28, SKYBLUE, Some(font.clone()))
                        .with_depth(DARKBLUE, 1.0)
                        .with_padding(10.0, 10.0),
                ),
                Element::Container(Container {
                    layout_dir: LayoutDirection::Horizontal,
                    margin: 3.0,
                    style: Style {
                        background_color: Some(Color::new(0.00, 0.3, 0.4, 1.00)),
                        padding: 10.0,
                        ..Default::default()
                    },
                    children: vec![
                        Element::Container(Container {
                            layout_dir: LayoutDirection::Vertical,
                            margin: 10.0,
                            align: Align::Center,
                            style: Style {
                                padding: 10.0,

                                ..Default::default()
                            },
                            children: vec![
                                Element::Text(
                                    TextLine::new("Spell book", 22, WHITE, Some(font.clone()))
                                        .with_depth(BLACK, 2.0),
                                ),
                                Element::Container(spell_book_rows),
                            ],
                            ..Default::default()
                        }),
                        Element::Container(Container {
                            layout_dir: LayoutDirection::Vertical,
                            margin: 15.0,
                            align: Align::Center,
                            style: Style {
                                padding: 10.0,
                                ..Default::default()
                            },
                            children: vec![
                                Element::Text(
                                    TextLine::new("Attributes", 22, WHITE, Some(font.clone()))
                                        .with_depth(BLACK, 2.0),
                                ),
                                stats_table,
                            ],

                            ..Default::default()
                        }),
                        Element::Container(Container {
                            layout_dir: LayoutDirection::Vertical,
                            margin: 15.0,
                            align: Align::Center,
                            style: Style {
                                padding: 10.0,
                                ..Default::default()
                            },
                            children: vec![
                                Element::Text(
                                    TextLine::new("Inventory", 22, WHITE, Some(font.clone()))
                                        .with_depth(BLACK, 2.0),
                                ),
                                inventory_section,
                                Element::Text(
                                    TextLine::new("Equipped", 22, WHITE, Some(font.clone()))
                                        .with_depth(BLACK, 2.0),
                                ),
                                equipped_section,
                            ],

                            ..Default::default()
                        }),
                    ],
                    ..Default::default()
                }),
            ],
            ..Default::default()
        };

        let top_bar_h = container.children[0].size().1;

        let button_size = (20.0, 20.0);
        let button_margin = 5.0;
        let close_button_rect = (
            container.size().0 - button_margin - button_size.0,
            button_margin,
            button_size.0,
            button_size.1,
        );

        Self {
            container,
            close_button_rect,
            top_bar_h,
            screen_position: Cell::new((100.0, 100.0)),
            dragged_offset: Cell::new(None),
        }
    }

    pub fn draw(&self) -> bool {
        let (x, y) = self.screen_position.get();
        let (w, h) = self.container.draw(x, y);
        let clicked_close = self.draw_close_button(x, y);
        self.container.draw_tooltips(x, y);

        let (mouse_x, mouse_y) = mouse_position();
        if let Some((x_offset, y_offset)) = self.dragged_offset.get() {
            if is_mouse_button_down(MouseButton::Left) {
                let new_x = (mouse_x - x_offset).max(0.0).min(screen_width() - w);
                let new_y = (mouse_y - y_offset).max(0.0).min(screen_height() - h);
                self.screen_position.set((new_x, new_y));
            } else {
                self.dragged_offset.set(None);
            }
        }

        if is_mouse_button_pressed(MouseButton::Left)
            && (x..x + w).contains(&mouse_x)
            && (y..y + self.top_bar_h).contains(&mouse_y)
        {
            self.dragged_offset.set(Some((mouse_x - x, mouse_y - y)));
        }

        if clicked_close {
            self.dragged_offset.set(None);
        }

        clicked_close
    }

    fn draw_close_button(&self, x: f32, y: f32) -> bool {
        let btn_x = x + self.close_button_rect.0;
        let btn_y = y + self.close_button_rect.1;
        let btn_w = self.close_button_rect.2;
        let btn_h = self.close_button_rect.3;

        let (mouse_x, mouse_y) = mouse_position();

        let hover =
            (btn_x..btn_x + btn_w).contains(&mouse_x) && (btn_y..btn_y + btn_h).contains(&mouse_y);

        let bg_color = DARKGRAY;
        draw_rectangle(btn_x, btn_y, btn_w, btn_h, bg_color);

        let cross_color = LIGHTGRAY;
        draw_cross(btn_x, btn_y, btn_w, btn_h, cross_color, 1.0, 2.0);
        if hover {
            draw_rectangle_lines(btn_x, btn_y, btn_w, btn_h, 2.0, WHITE);
        } else {
            draw_rectangle_lines(btn_x, btn_y, btn_w, btn_h, 1.0, cross_color);
        }

        hover && is_mouse_button_pressed(MouseButton::Left)
    }
}

fn buttons_row(buttons: Vec<Element>) -> Element {
    Element::Container(Container {
        layout_dir: LayoutDirection::Horizontal,
        margin: 5.0,
        children: buttons,
        ..Default::default()
    })
}
