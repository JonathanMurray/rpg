use std::cell::{Cell, RefCell};
use std::{collections::HashMap, rc::Rc};

use macroquad::color::{DARKBLUE, DARKGRAY, RED, SKYBLUE, YELLOW};

use macroquad::input::{
    is_mouse_button_down, is_mouse_button_pressed, is_mouse_button_released, mouse_position,
    MouseButton,
};
use macroquad::shapes::{draw_rectangle, draw_rectangle_lines};
use macroquad::texture::{draw_texture_ex, DrawTextureParams};
use macroquad::window::{screen_height, screen_width};
use macroquad::{
    color::{Color, BLACK, LIGHTGRAY, WHITE},
    text::Font,
    texture::Texture2D,
};

use crate::core::EquipmentSlotRole;
use crate::drawing::{draw_cross, draw_dashed_line, draw_dashed_rectangle_lines};
use crate::equipment_ui::{
    build_inventory_section, EquipmentSlot, EquipmentSlotContent, EquipmentStatsTable,
};
use crate::game_ui::{ConfiguredAction, UiState};
use crate::{
    action_button::ActionButton,
    base_ui::{Align, Container, ContainerScroll, Element, LayoutDirection, Style, TextLine},
    core::Character,
    equipment_ui::build_equipped_section,
    stats_ui::{build_stats_table, StatValue},
    textures::EquipmentIconId,
};

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct EquipmentDrag {
    pub from_idx: usize,
    pub to_idx: Option<usize>,
}

const INVENTORY_SIZE: usize = 6;

pub struct CharacterSheet {
    character: Rc<Character>,
    equipment_slots: Vec<Rc<RefCell<EquipmentSlot>>>,
    screen_position: Cell<(f32, f32)>,
    sheet_dragged_offset: Cell<Option<(f32, f32)>>,
    //equipment_drag: Option<EquipmentDrag>,
    equipment_changed: Rc<Cell<bool>>,
    equipment_icons: HashMap<EquipmentIconId, Texture2D>,

    container: Container,
    top_bar_h: f32,
    equipment_stats_table: Rc<RefCell<EquipmentStatsTable>>,
    font: Font,
}

impl CharacterSheet {
    pub fn new(
        font: &Font,
        character: Rc<Character>,
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

        let (inventory_section, mut inventory_slots) =
            build_inventory_section(font, &character, equipment_icons);

        let (equipped_section, equipped_slots, equipment_stats_table) =
            build_equipped_section(font, &character, equipment_icons);

        inventory_slots.extend_from_slice(&equipped_slots);

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

        let equipment_changed = character.listen_to_changed_equipment();

        Self {
            character,
            container,
            top_bar_h,
            screen_position: Cell::new((100.0, 100.0)),
            sheet_dragged_offset: Cell::new(None),
            equipment_changed,
            equipment_icons: equipment_icons.clone(),

            equipment_slots: inventory_slots,
            //equipment_drag: Default::default(),
            equipment_stats_table,
            font: font.clone(),
        }
    }

    pub fn draw(&mut self, ui_state: &mut UiState) -> CharacterSheetOutcome {
        if self.equipment_changed.take() {
            println!("CHAR EQUIPMENT CHANGED. UPDATING CHARACTER SHEET...");
            self.repopulate_character_equipment();
        }

        let (x, y) = self.screen_position.get();
        let (w, h) = self.container.draw(x, y);
        let clicked_close = self.draw_close_button(x, y);

        let (mouse_x, mouse_y) = mouse_position();

        /*
        if let Some(EquipmentDrag {
            to_idx: Some(_), ..
        }) = &self.equipment_drag
        {
            if !matches!(
                ui_state,
                UiState::ConfiguringAction(ConfiguredAction::ChangeEquipment)
            ) {
                // The drag operation has been cancelled from outside of the character sheet.
                self.equipment_drag = None;
            }
        }
         */

        let is_allowed_to_change_equipment = matches!(
            ui_state,
            UiState::ConfiguringAction(..) | UiState::ChoosingAction
        );

        let drag = match ui_state {
            UiState::ConfiguringAction(ConfiguredAction::ChangeEquipment { drag }) => drag,
            _ => &mut Option::<EquipmentDrag>::None,
        };

        let previous_drag = *drag;

        self.handle_equipment_dragging((mouse_x, mouse_y), drag);

        self.container.draw_tooltips(x, y);

        if let Some((x_offset, y_offset)) = self.sheet_dragged_offset.get() {
            if is_mouse_button_down(MouseButton::Left) {
                let new_x = (mouse_x - x_offset).max(0.0).min(screen_width() - w);
                let new_y = (mouse_y - y_offset).max(0.0).min(screen_height() - h);
                self.screen_position.set((new_x, new_y));
            } else {
                self.sheet_dragged_offset.set(None);
            }
        }

        if is_mouse_button_pressed(MouseButton::Left)
            && (x..x + w).contains(&mouse_x)
            && (y..y + self.top_bar_h).contains(&mouse_y)
        {
            self.sheet_dragged_offset
                .set(Some((mouse_x - x, mouse_y - y)));
        }

        if clicked_close {
            self.sheet_dragged_offset.set(None);
        }

        let changed_drag = *drag != previous_drag;

        if drag.is_some() && previous_drag.is_none() && is_allowed_to_change_equipment {
            // TODO: currently it's not possible to move around items even in the inventory on another character's
            // turn. Ideally that should be possible.
            *ui_state =
                UiState::ConfiguringAction(ConfiguredAction::ChangeEquipment { drag: *drag });
        }

        CharacterSheetOutcome {
            clicked_close,
            changed_drag,
        }
    }

    fn repopulate_character_equipment(&mut self) {
        self.equipment_stats_table
            .borrow_mut()
            .rebuild(&self.character, &self.font);

        for (i, maybe_entry) in self.character.inventory.iter().enumerate() {
            self.equipment_slots[i].borrow_mut().content = maybe_entry.get().map(|entry| {
                let texture = self.equipment_icons[&entry.icon()].clone();
                EquipmentSlotContent::new(texture, entry)
            });
        }

        let roles = [
            EquipmentSlotRole::MainHand,
            EquipmentSlotRole::Armor,
            EquipmentSlotRole::OffHand,
        ];
        for (i, role) in roles.iter().enumerate() {
            self.equipment_slots[INVENTORY_SIZE + i]
                .borrow_mut()
                .content = self.character.equipment(*role).map(|entry| {
                let texture = self.equipment_icons[&entry.icon()].clone();
                EquipmentSlotContent::new(texture, entry)
            });
        }
    }

    fn handle_equipment_dragging(
        &mut self,
        mouse_pos: (f32, f32),
        equipment_drag: &mut Option<EquipmentDrag>,
    ) {
        for idx in 0..self.equipment_slots.len() {
            let slot = self.equipment_slots[idx].borrow_mut();
            let rect = slot.screen_area();
            let is_hovered = rect.contains(mouse_pos.into());

            let drag_validity = match equipment_drag {
                Some(EquipmentDrag { from_idx, .. }) if *from_idx != idx => {
                    let dragged_slot = &mut self.equipment_slots[*from_idx].borrow_mut();
                    let valid_forward = dragged_slot
                        .content
                        .as_ref()
                        .map(|content| {
                            self.character
                                .can_equipment_fit(content.equipment, slot.role())
                        })
                        .unwrap_or(true);

                    let valid_reverse = slot
                        .content
                        .as_ref()
                        .map(|content| {
                            self.character
                                .can_equipment_fit(content.equipment, dragged_slot.role())
                        })
                        .unwrap_or(true);

                    Some(valid_forward && valid_reverse)
                }
                _ => None,
            };

            if is_hovered {
                if is_mouse_button_pressed(MouseButton::Left) {
                    if slot.content.is_some() {
                        *equipment_drag = Some(EquipmentDrag {
                            from_idx: idx,
                            to_idx: None,
                        });
                    }
                } else if is_mouse_button_released(MouseButton::Left) {
                    if let Some(EquipmentDrag { from_idx, to_idx }) = equipment_drag {
                        if to_idx.is_none() && *from_idx != idx {
                            let dragged_slot = &mut self.equipment_slots[*from_idx].borrow_mut();

                            if drag_validity.unwrap() {
                                let slots = [dragged_slot, &slot];

                                if slots.iter().any(|slot| slot.role().is_equipped()) {
                                    //did_change_equipment = true;

                                    println!("PREVIEW DRAG");

                                    *to_idx = Some(idx);
                                } else {
                                    println!("PERFORM DRAG");

                                    for i in [0, 1] {
                                        let entry_a = slots[i]
                                            .content
                                            .as_ref()
                                            .map(|content| content.equipment);
                                        let role_b = slots[(i + 1) % 2].role();

                                        self.character.set_equipment(entry_a, role_b);
                                    }

                                    //std::mem::swap(&mut dragged_slot.content, &mut slot.content);

                                    *equipment_drag = None;
                                }

                                /*

                                self.equipment_stats_table
                                    .borrow_mut()
                                    .rebuild(&self.character, &self.font);
                                */
                            }
                        } else {
                            println!("WILL NOT DRAG");

                            *equipment_drag = None;
                        }
                    }
                }
            }

            if matches!(equipment_drag, Some(EquipmentDrag{ from_idx: i, .. }) if *i == idx) {
                draw_dashed_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 3.0, YELLOW, 4.0);
            } else if matches!(equipment_drag, Some(EquipmentDrag{ to_idx: Some(i), .. }) if *i == idx)
            {
                draw_dashed_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 3.0, YELLOW, 4.0);
            } else if is_hovered {
                if let Some(valid) = drag_validity {
                    let color = if valid { YELLOW } else { RED };
                    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 3.0, color);
                } else if slot.content.is_some() {
                    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 1.0, WHITE);
                }
            }
        }

        if let Some(EquipmentDrag { from_idx, to_idx }) = equipment_drag {
            match to_idx {
                Some(to_idx) => {
                    let to = self.equipment_slots[*to_idx]
                        .borrow()
                        .screen_area()
                        .center();
                    let from = self.equipment_slots[*from_idx]
                        .borrow()
                        .screen_area()
                        .center();

                    draw_dashed_line(from.into(), to.into(), 5.0, YELLOW, 5.0, None);
                    //draw_line(from.x, from.y, to.x, to.y, 5.0, MAGENTA);
                }
                None => {
                    if is_mouse_button_down(MouseButton::Left) {
                        let slot = self.equipment_slots[*from_idx].borrow();
                        let texture = &slot.content.as_ref().unwrap().texture;
                        let params = DrawTextureParams {
                            dest_size: Some((40.0, 40.0).into()),
                            ..Default::default()
                        };
                        draw_texture_ex(texture, mouse_pos.0, mouse_pos.1, WHITE, params);
                    } else {
                        println!("NOT DRAGGING ANYMORE");
                        *equipment_drag = None;
                    }
                }
            }
        }
    }

    fn draw_close_button(&self, x: f32, y: f32) -> bool {
        let container_size = self.container.size();

        let button_size = (20.0, 20.0);
        let button_margin = 5.0;

        let btn_x = x + container_size.0 - button_margin - button_size.0;
        let btn_y = y + button_margin;
        let btn_w = button_size.0;
        let btn_h = button_size.1;

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

    pub fn resolve_drag_to_slots(
        &self,
        drag: EquipmentDrag,
    ) -> (EquipmentSlotRole, EquipmentSlotRole) {
        let from_slot = self.equipment_slots[drag.from_idx].borrow();
        let to_slot = self.equipment_slots[drag.to_idx.unwrap()].borrow();

        (from_slot.role(), to_slot.role())
    }

    pub fn describe_requested_equipment_change(&self, drag: EquipmentDrag) -> String {
        let from = self.equipment_slots[drag.from_idx].borrow();
        let to = self.equipment_slots[drag.to_idx.unwrap()].borrow();
        let from_content = from.content.as_ref().unwrap();

        let s = if let Some(to_content) = to.content.as_ref() {
            if from.role().is_equipped() {
                format!(
                    "Switch from {} to {}",
                    from_content.equipment.name(),
                    to_content.equipment.name()
                )
            } else {
                format!(
                    "Switch from {} to {}",
                    to_content.equipment.name(),
                    from_content.equipment.name()
                )
            }
        } else if to.role().is_equipped() {
            format!("Equip {}", from_content.equipment.name())
        } else {
            format!("Unequip {}", from_content.equipment.name())
        };
        s
    }
}

pub struct CharacterSheetOutcome {
    pub clicked_close: bool,
    pub changed_drag: bool,
}

fn buttons_row(buttons: Vec<Element>) -> Element {
    Element::Container(Container {
        layout_dir: LayoutDirection::Horizontal,
        margin: 5.0,
        children: buttons,
        ..Default::default()
    })
}

fn has_drag(ui_state: &UiState) -> bool {
    matches!(
        ui_state,
        UiState::ConfiguringAction(ConfiguredAction::ChangeEquipment { drag: Some(_) })
    )
}
