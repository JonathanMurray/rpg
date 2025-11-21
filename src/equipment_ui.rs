use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use macroquad::{
    color::{Color, BLACK, DARKGRAY, RED, SKYBLUE, YELLOW},
    input::{is_mouse_button_down, is_mouse_button_pressed, is_mouse_button_released, MouseButton},
    math::Rect,
    shapes::{draw_rectangle, draw_rectangle_lines},
    text::{measure_text, TextParams},
    texture::{draw_texture_ex, DrawTextureParams},
};

use macroquad::{
    color::{GRAY, WHITE},
    input::mouse_position,
    text::Font,
    texture::Texture2D,
};

use crate::{
    action_button::{
        describe_apply_effect, describe_area_effect, draw_regular_tooltip, draw_tooltip, Side,
        Tooltip, TooltipPositionPreference,
    },
    base_ui::{
        draw_text_rounded, table, Align, Container, Drawable, Element, LayoutDirection, Style,
        TableCell, TableStyle, TextLine,
    },
    character_sheet::MoneyText,
    core::{
        ApplyEffect, ArmorPiece, Arrow, ArrowStack, AttackHitEffect, Character, Consumable,
        EquipmentEntry, EquipmentSlotRole, HandType, Party, Shield, Weapon, WeaponGrip,
        WeaponRange,
    },
    drawing::{draw_dashed_line, draw_dashed_rectangle_lines},
    textures::EquipmentIconId,
};

const INVENTORY_SIZE: usize = 6;
const EQUIPPED_SIZE: usize = 4;

pub fn equipment_tooltip(entry: &EquipmentEntry) -> Tooltip {
    match entry {
        EquipmentEntry::Weapon(weapon) => weapon_tooltip(weapon),
        EquipmentEntry::Shield(shield) => shield_tooltip(shield),
        EquipmentEntry::Armor(armor) => armor_tooltip(armor),
        EquipmentEntry::Consumable(consumable) => consumable_tooltip(consumable),
        EquipmentEntry::Arrows(stack) => arrow_tooltip(stack),
    }
}

fn arrow_tooltip(stack: &ArrowStack) -> Tooltip {
    let mut t = Tooltip::new(stack.arrow.name);
    let penetration = stack.arrow.bonus_penetration;
    if penetration > 0 {
        t.technical_description
            .push(format!("{} armor penetration", penetration));
    }
    if let Some(effect) = stack.arrow.on_damage_apply {
        t.technical_description.push("Target:".to_string());
        describe_apply_effect(effect, &mut t);
    }
    if let Some(area_effect) = stack.arrow.area_effect {
        describe_area_effect(None, area_effect, &mut t);
    }
    t
}

fn consumable_tooltip(consumable: &Consumable) -> Tooltip {
    let mut t = Tooltip::new(consumable.name);

    if consumable.health_gain > 0 {
        t.technical_description
            .push(format!("Restores {} health", consumable.health_gain));
    }
    if consumable.mana_gain > 0 {
        t.technical_description
            .push(format!("Restores {} mana", consumable.mana_gain));
    }
    //lines.push("<Right-click to use>".to_string());
    t.technical_description
        .push(format!("Weight: {}", consumable.weight));

    t
}

fn weapon_tooltip(weapon: &Weapon) -> Tooltip {
    let mut t = Tooltip::new(weapon.name);
    t.technical_description.push(format!(
        "{} damage / {} AP",
        weapon.damage, weapon.action_point_cost
    ));

    if weapon.grip == WeaponGrip::TwoHanded {
        t.technical_description.push("Two-handed".to_string());
    }

    if weapon.range != WeaponRange::Melee {
        t.technical_description
            .push(format!("Range: {}", weapon.range));
    }
    if let Some(effect) = weapon.on_true_hit {
        t.technical_description.push(format!("[true hit] {effect}"));
        if let AttackHitEffect::Apply(apply_effect) = effect {
            match apply_effect {
                ApplyEffect::Condition(condition) => t.keywords.push(condition),
                ApplyEffect::ConsumeCondition { condition } => t.keywords.push(condition),
                _ => {}
            }
        }
    }
    if let Some(reaction) = weapon.on_attacked_reaction {
        t.technical_description
            .push(format!("[attacked?] {}", reaction.name));
    }
    if let Some(enhancement) = weapon.attack_enhancement {
        t.technical_description
            .push(format!("~ {}", enhancement.name));
    }
    t.technical_description
        .push(format!("Weight: {}", weapon.weight));

    t
}

fn shield_tooltip(shield: &Shield) -> Tooltip {
    let mut t = Tooltip::new(shield.name);
    t.technical_description
        .push(format!("+{} evasion", shield.evasion));
    if shield.armor > 0 {
        t.technical_description
            .push(format!("+{} armor", shield.armor));
    }

    if let Some(reaction) = shield.on_attacked_reaction {
        t.technical_description
            .push(format!("[attacked?] {}", reaction.name));
    }
    if let Some(reaction) = shield.on_hit_reaction {
        t.technical_description
            .push(format!("[hit?] {}", reaction.name));
    }
    t.technical_description
        .push(format!("Weight: {}", shield.weight));
    t
}

fn armor_tooltip(armor: &ArmorPiece) -> Tooltip {
    let mut t = Tooltip::new(armor.name);
    t.technical_description
        .push(format!("{} armor", armor.protection));
    if let Some(limit) = armor.limit_evasion_from_agi {
        t.technical_description
            .push(format!("Max {} evasion from agi", limit));
    }
    if armor.equip.bonus_spell_modifier > 0 {
        t.technical_description
            .push(format!("+{} spell mod", armor.equip.bonus_spell_modifier));
    }
    t.technical_description
        .push(format!("Weight: {}", armor.weight));
    t
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct EquipmentDrag {
    pub from_idx: usize,
    pub to_idx: Option<usize>,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct EquipmentConsumption {
    pub equipment_idx: usize,
    pub consumable: Consumable,
}

pub struct EquipmentSection {
    pub element: Element,
    pub equipment_slots: Vec<Rc<RefCell<EquipmentSlot>>>,
    equipment_stats_table: Rc<RefCell<EquipmentStatsTable>>,
    font: Font,
    character: Rc<Character>,
    equipment_icons: HashMap<EquipmentIconId, Texture2D>,
    include_stash: bool,
}

impl EquipmentSection {
    pub fn new(
        font: &Font,
        character: &Rc<Character>,
        equipment_icons: HashMap<EquipmentIconId, Texture2D>,
        include_stash: bool,
    ) -> Self {
        let (inventory_section, mut equipment_slots) = build_inventory_section(
            font,
            &equipment_icons,
            &character.inventory,
            InventoryType::Personal,
        );

        let (equipped_section, equipped_slots, equipment_stats_table) =
            build_equipped_section(font, character, &equipment_icons);
        equipment_slots.extend_from_slice(&equipped_slots);

        let stash_column = if include_stash {
            let (stash_section, stash_slots) = build_inventory_section(
                font,
                &equipment_icons,
                &character.party_stash(),
                InventoryType::PartyStash,
            );
            equipment_slots.extend_from_slice(&stash_slots);

            Some(Element::Container(Container {
                layout_dir: LayoutDirection::Vertical,
                margin: 15.0,
                align: Align::Center,
                style: Style {
                    padding: 10.0,
                    ..Default::default()
                },
                children: vec![
                    Element::Text(
                        TextLine::new("Party stash", 22, WHITE, Some(font.clone()))
                            .with_depth(BLACK, 2.0),
                    ),
                    stash_section,
                ],
                ..Default::default()
            }))
        } else {
            None
        };

        let money_text = MoneyText {
            character: Rc::clone(&character),
            font: font.clone(),
        };

        let char_eq_column = Element::Container(Container {
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
                    TextLine::new("Equipped", 22, WHITE, Some(font.clone())).with_depth(BLACK, 2.0),
                ),
                equipped_section,
                Element::Empty(0.0, 2.0),
                Element::Box(Box::new(money_text)),
            ],
            ..Default::default()
        });

        let stats_column = Element::Container(Container {
            layout_dir: LayoutDirection::Vertical,
            margin: 15.0,
            align: Align::Center,
            style: Style {
                padding: 10.0,
                ..Default::default()
            },
            children: vec![
                Element::Empty(0.0, 15.0),
                Element::RcRefCell(equipment_stats_table.clone()),
            ],
            ..Default::default()
        });

        let mut children = vec![stats_column, char_eq_column];
        if let Some(col) = stash_column {
            children.push(col);
        }
        let element = Element::Container(Container {
            layout_dir: LayoutDirection::Horizontal,
            margin: 5.0,
            //align: Align::Center,
            //border_between_children: Some(DARKGRAY),
            children,
            ..Default::default()
        });

        Self {
            element,
            equipment_slots,
            equipment_stats_table,
            font: font.clone(),
            character: Rc::clone(character),
            equipment_icons,
            include_stash,
        }
    }

    pub fn repopulate_character_equipment(&mut self) {
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
            EquipmentSlotRole::Arrows,
        ];
        for (i, role) in roles.iter().enumerate() {
            self.equipment_slots[INVENTORY_SIZE + i]
                .borrow_mut()
                .content = self.character.equipment(*role).map(|entry| {
                let texture = self.equipment_icons[&entry.icon()].clone();
                EquipmentSlotContent::new(texture, entry)
            });
        }

        if self.include_stash {
            for (i, maybe_entry) in self.character.party_stash().iter().enumerate() {
                self.equipment_slots[INVENTORY_SIZE + EQUIPPED_SIZE + i]
                    .borrow_mut()
                    .content = maybe_entry.get().map(|entry| {
                    let texture = self.equipment_icons[&entry.icon()].clone();
                    EquipmentSlotContent::new(texture, entry)
                });
            }
        }
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

    pub fn handle_equipment_drag_and_consumption(
        &mut self,
        mut drag: Option<EquipmentDrag>,
        mut requested_consumption: Option<EquipmentConsumption>,
        is_allowed_to_change_equipment: bool,
    ) -> EquipmentSectionOutcome {
        let (mouse_x, mouse_y) = mouse_position();

        let previous_requested_consumption = requested_consumption;
        let previous_drag = drag;

        let mouse_pos = (mouse_x, mouse_y);

        for idx in 0..self.equipment_slots.len() {
            let slot = self.equipment_slots[idx].borrow_mut();
            let rect = slot.screen_area();
            let is_hovered = rect.contains(mouse_pos.into());

            let drag_validity = match drag {
                Some(EquipmentDrag { from_idx, .. }) if from_idx != idx => {
                    let dragged_slot = &mut self.equipment_slots[from_idx].borrow_mut();

                    if !is_allowed_to_change_equipment
                        && [dragged_slot, &slot]
                            .iter()
                            .any(|slot| slot.role().is_equipped())
                    {
                        Some(false)
                    } else {
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
                }
                _ => None,
            };

            if is_hovered {
                if is_mouse_button_pressed(MouseButton::Right) {
                    if let Some(content) = &slot.content {
                        if let EquipmentEntry::Consumable(consumable) = content.equipment {
                            requested_consumption = Some(EquipmentConsumption {
                                equipment_idx: idx,
                                consumable,
                            });
                        }
                    }
                } else if is_mouse_button_pressed(MouseButton::Left) {
                    if slot.content.is_some() {
                        drag = Some(EquipmentDrag {
                            from_idx: idx,
                            to_idx: None,
                        });
                    }
                } else if is_mouse_button_released(MouseButton::Left) {
                    if let Some(EquipmentDrag { from_idx, to_idx }) = &mut drag {
                        if to_idx.is_none() && *from_idx != idx {
                            let dragged_slot = &mut self.equipment_slots[*from_idx].borrow_mut();

                            if drag_validity.unwrap() {
                                let slots = [dragged_slot, &slot];

                                if slots.iter().any(|slot| slot.role().is_equipped()) {
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
                                    drag = None;
                                }
                            }
                        } else {
                            println!("WILL NOT DRAG");

                            drag = None;
                        }
                    }
                }
            }

            if let Some(consumption) = requested_consumption {
                if consumption.equipment_idx == idx {
                    draw_dashed_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 3.0, YELLOW, 4.0);
                }
            }

            if matches!(drag, Some(EquipmentDrag{ from_idx, .. }) if from_idx == idx) {
                draw_dashed_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 3.0, YELLOW, 4.0);
            } else if matches!(drag, Some(EquipmentDrag{ to_idx: Some(to_idx), .. }) if to_idx == idx)
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

        if let Some(EquipmentDrag { from_idx, to_idx }) = drag {
            match to_idx {
                Some(to_idx) => {
                    let to = self.equipment_slots[to_idx].borrow().screen_area().center();
                    let from = self.equipment_slots[from_idx]
                        .borrow()
                        .screen_area()
                        .center();

                    draw_dashed_line(from.into(), to.into(), 5.0, YELLOW, 5.0, None);
                }
                None => {
                    if is_mouse_button_down(MouseButton::Left) {
                        let slot = self.equipment_slots[from_idx].borrow();
                        // TODO: this can crash if trying to drag equipment on a character who's not active (which should not be allowed in the first place)
                        let texture = &slot.content.as_ref().unwrap().texture;
                        let params = DrawTextureParams {
                            dest_size: Some((40.0, 40.0).into()),
                            ..Default::default()
                        };
                        draw_texture_ex(texture, mouse_pos.0, mouse_pos.1, WHITE, params);
                    } else {
                        println!("NOT DRAGGING ANYMORE");
                        drag = None;
                    }
                }
            }
        }

        let changed =
            drag != previous_drag || requested_consumption != previous_requested_consumption;

        EquipmentSectionOutcome {
            changed,
            equipment_drag: drag,
            requested_consumption,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EquipmentSectionOutcome {
    pub changed: bool,
    pub equipment_drag: Option<EquipmentDrag>,
    pub requested_consumption: Option<EquipmentConsumption>,
}

impl Drawable for EquipmentSection {
    fn draw(&self, x: f32, y: f32) {
        self.element.draw(x, y)
    }

    fn draw_tooltips(&self, x: f32, y: f32) {
        self.element.draw_tooltips(x, y)
    }

    fn size(&self) -> (f32, f32) {
        self.element.size()
    }
}

enum InventoryType {
    Personal,
    PartyStash,
}

fn build_inventory_section(
    font: &Font,
    equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
    inventory: &[Cell<Option<EquipmentEntry>>; 6],
    inventory_type: InventoryType,
) -> (Element, Vec<Rc<RefCell<EquipmentSlot>>>) {
    let mut slots: Vec<Rc<RefCell<EquipmentSlot>>> = inventory
        .iter()
        .enumerate()
        .map(|(i, maybe_entry)| {
            let content = maybe_entry
                .get()
                .map(|entry| Some((equipment_icons[&entry.icon()].clone(), entry)))
                .unwrap_or(None);

            let role = match inventory_type {
                InventoryType::Personal => EquipmentSlotRole::Inventory(i),
                InventoryType::PartyStash => EquipmentSlotRole::PartyStash(i),
            };
            EquipmentSlot::new(font.clone(), content, role, None)
        })
        .map(|slot| Rc::new(RefCell::new(slot)))
        .collect();

    let cloned_slots: Vec<Rc<RefCell<EquipmentSlot>>> = slots.iter().map(Rc::clone).collect();

    let mut rows = vec![];
    while !slots.is_empty() {
        let slots_in_row = slots
            .drain(0..3)
            .take(3)
            .map(|cell| Element::RcRefCell(cell))
            .collect();
        let row = Container {
            layout_dir: LayoutDirection::Horizontal,
            children: slots_in_row,
            margin: 2.0,
            ..Default::default()
        };
        rows.push(Element::Container(row))
    }

    let section = Element::Container(Container {
        layout_dir: LayoutDirection::Vertical,
        children: rows,
        align: Align::Center,
        margin: 5.0,
        ..Default::default()
    });

    (section, cloned_slots)
}

pub fn build_equipped_section(
    font: &Font,
    character: &Character,
    equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
) -> (
    Element,
    Vec<Rc<RefCell<EquipmentSlot>>>,
    Rc<RefCell<EquipmentStatsTable>>,
) {
    let placeholder_text = "Draw something from your inventory to equip it";
    let mut slots: Vec<Rc<RefCell<EquipmentSlot>>> = [
        EquipmentSlot::new(
            font.clone(),
            None,
            EquipmentSlotRole::MainHand,
            Some((
                equipment_icons[&EquipmentIconId::PlaceholderMainhand].clone(),
                vec!["(Main-hand)".to_string(), placeholder_text.to_string()],
            )),
        ),
        EquipmentSlot::new(
            font.clone(),
            None,
            EquipmentSlotRole::Armor,
            Some((
                equipment_icons[&EquipmentIconId::PlaceholderArmor].clone(),
                vec!["(Armor)".to_string(), placeholder_text.to_string()],
            )),
        ),
        EquipmentSlot::new(
            font.clone(),
            None,
            EquipmentSlotRole::OffHand,
            Some((
                equipment_icons[&EquipmentIconId::PlaceholderOffhand].clone(),
                vec!["(Off-hand)".to_string(), placeholder_text.to_string()],
            )),
        ),
        EquipmentSlot::new(
            font.clone(),
            None,
            EquipmentSlotRole::Arrows,
            Some((
                equipment_icons[&EquipmentIconId::PlaceholderArrows].clone(),
                vec!["(Arrows)".to_string(), placeholder_text.to_string()],
            )),
        ),
    ]
    .into_iter()
    .map(|slot| Rc::new(RefCell::new(slot)))
    .collect();

    for hand in [HandType::MainHand, HandType::OffHand] {
        if let Some(weapon) = character.weapon(hand) {
            let texture = equipment_icons[&weapon.icon].clone();
            slots[0].borrow_mut().content = Some(EquipmentSlotContent::new(
                texture,
                EquipmentEntry::Weapon(weapon),
            ));
        }
    }
    if let Some(shield) = character.shield() {
        let texture = equipment_icons[&shield.icon].clone();
        slots[2].borrow_mut().content = Some(EquipmentSlotContent::new(
            texture,
            EquipmentEntry::Shield(shield),
        ));
    }
    if let Some(armor) = character.armor_piece.get() {
        let texture = equipment_icons[&armor.icon].clone();
        slots[1].borrow_mut().content = Some(EquipmentSlotContent::new(
            texture,
            EquipmentEntry::Armor(armor),
        ));
    }
    if let Some(stack) = character.arrows.get() {
        let texture = equipment_icons[&stack.arrow.icon].clone();
        slots[3].borrow_mut().content = Some(EquipmentSlotContent::new(
            texture,
            EquipmentEntry::Arrows(stack),
        ));
    }

    let cloned_slots: Vec<Rc<RefCell<EquipmentSlot>>> = slots.iter().map(Rc::clone).collect();

    let mut rows = vec![];
    while !slots.is_empty() {
        let slots_in_row = slots
            .drain(0..(3).min(slots.len()))
            .take(3)
            .map(|cell| Element::RcRefCell(cell))
            .collect();
        let row = Container {
            layout_dir: LayoutDirection::Horizontal,
            children: slots_in_row,
            margin: 2.0,
            ..Default::default()
        };
        rows.push(Element::Container(row))
    }

    let slots_container = Element::Container(Container {
        layout_dir: LayoutDirection::Vertical,
        children: rows,
        margin: 5.0,
        ..Default::default()
    });

    let stats_table = Rc::new(RefCell::new(EquipmentStatsTable::new(character, font)));

    (slots_container, cloned_slots, stats_table)

    //(container, cloned_slots, stats_table)
}

pub struct EquipmentStatsTable {
    element: Element,
}

impl EquipmentStatsTable {
    fn new(character: &Character, font: &Font) -> Self {
        Self {
            element: Element::Container(Self::build_table(character, font)),
        }
    }

    pub fn rebuild(&mut self, character: &Character, font: &Font) {
        self.element = Element::Container(Self::build_table(character, font));
    }

    pub fn build_table(character: &Character, font: &Font) -> Container {
        let mut cells: Vec<TableCell> = vec![];
        let mut has_weapon = false;
        for hand in [HandType::MainHand, HandType::OffHand] {
            if let Some(weapon) = character.weapon(hand) {
                has_weapon = true;
                cells.push("Attack dmg".into());
                cells.push(format!("{}", weapon.damage).into());

                cells.push("Attack mod".into());
                cells.push(format!("+{}", character.attack_modifier(hand)).into());
            }
        }
        if !has_weapon {
            cells.push("Attack dmg".into());
            cells.push("".into());

            cells.push("Attack mod".into());
            cells.push("".into());
        }

        if let Some(shield) = character.shield() {
            cells.push("Evasion bonus".into());
            cells.push(format!("{}", shield.evasion).into());
        }
        cells.push("Armor".into());
        cells.push(format!("{}", character.protection_from_armor()).into());

        cells.push("Weight".into());

        let text = format!(
            "{} / {}",
            character.equipment_weight(),
            character.capacity.get()
        );
        let color_override = if character.equipment_weight() > character.capacity.get() {
            //Some(Color::new(1.0, 0.0, 0.0, 1.0))
            Some(RED)
        } else {
            None
        };

        cells.push(TableCell::new(text, color_override, None));

        table(
            cells,
            vec![Align::End, Align::Start],
            font.clone(),
            TableStyle {
                background_color: Some(SKYBLUE),
                default_text_color: BLACK,
                ..Default::default()
            },
        )
    }
}

impl Drawable for EquipmentStatsTable {
    fn draw(&self, x: f32, y: f32) {
        self.element.draw(x, y)
    }

    fn size(&self) -> (f32, f32) {
        self.element.size()
    }
}

#[derive(Debug)]
pub struct EquipmentSlot {
    style: Style,
    size: (f32, f32),
    font: Font,
    pub content: Option<EquipmentSlotContent>,
    last_drawn_rect: Cell<Rect>,
    role: EquipmentSlotRole,
    placeholder: Option<(Texture2D, Vec<String>)>,
}

#[derive(Debug)]
pub struct EquipmentSlotContent {
    pub equipment: EquipmentEntry,
    pub texture: Texture2D,
    tooltip: Tooltip,
}

impl EquipmentSlotContent {
    pub fn new(texture: Texture2D, equipment: EquipmentEntry) -> Self {
        Self {
            texture,
            tooltip: equipment_tooltip(&equipment),
            equipment,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum SlotMouseEvent {
    Pressed,
    Released,
}

impl EquipmentSlot {
    pub fn new(
        font: Font,
        content: Option<(Texture2D, EquipmentEntry)>,
        role: EquipmentSlotRole,
        placeholder: Option<(Texture2D, Vec<String>)>,
    ) -> Self {
        Self {
            font,
            style: Style {
                background_color: Some(SKYBLUE),
                border_color: Some(GRAY),
                border_width: Some(2.0),
                ..Default::default()
            },
            size: (40.0, 40.0),
            content: content
                .map(|(texture, equipment)| EquipmentSlotContent::new(texture, equipment)),
            last_drawn_rect: Default::default(),
            role,
            placeholder,
        }
    }

    pub fn role(&self) -> EquipmentSlotRole {
        self.role
    }

    pub fn screen_area(&self) -> Rect {
        self.last_drawn_rect.get()
    }
}

impl Drawable for EquipmentSlot {
    fn draw(&self, x: f32, y: f32) {
        let x = x.floor();
        let y = y.floor();
        self.style.draw(x, y, self.size);
        let params = DrawTextureParams {
            dest_size: Some(self.size.into()),
            ..Default::default()
        };
        if let Some(content) = &self.content {
            draw_texture_ex(&content.texture, x, y, WHITE, params);

            let quantity = match content.equipment {
                EquipmentEntry::Arrows(arrow_stack) => Some(arrow_stack.quantity),
                _ => None,
            };

            if let Some(q) = quantity {
                let text = format!("{}", q);
                let margin = 1.0;
                let font_size = 16;
                let text_dim = measure_text(&text, Some(&self.font), font_size, 1.0);
                let x0 = x + margin;
                let y0 = y + margin;
                let padding = 2.0;
                draw_rectangle(
                    x0,
                    y0,
                    text_dim.width + padding * 2.0,
                    text_dim.height + padding * 2.0,
                    Color::new(0.0, 0.0, 0.0, 0.4),
                );
                draw_text_rounded(
                    &text,
                    x0 + padding,
                    y0 + padding + text_dim.offset_y,
                    TextParams {
                        font: Some(&self.font),
                        font_size,
                        color: WHITE,
                        ..Default::default()
                    },
                );
            }
        } else if let Some((texture, _tooltip)) = &self.placeholder {
            draw_rectangle(
                x,
                y,
                self.size.0,
                self.size.1,
                Color::new(0.0, 0.0, 0.0, 0.4),
            );
            draw_texture_ex(texture, x, y, WHITE, params);
        }

        self.last_drawn_rect
            .set(Rect::new(x, y, self.size.0, self.size.1));
    }

    fn draw_tooltips(&self, x: f32, y: f32) {
        let (mouse_x, mouse_y) = mouse_position();
        let hover =
            (x..x + self.size.0).contains(&mouse_x) && (y..y + self.size.1).contains(&mouse_y);
        let rect = Rect::new(x, y, self.size.0, self.size.1);
        if let Some(content) = &self.content {
            if hover {
                draw_tooltip(
                    &self.font,
                    TooltipPositionPreference::RelativeToRect(rect, Side::Bottom),
                    &content.tooltip.header,
                    None,
                    &content.tooltip.technical_description,
                    &content.tooltip.keywords,
                    false,
                );
            }
        } else if let Some((_texture, tooltip_lines)) = &self.placeholder {
            if hover {
                draw_regular_tooltip(
                    &self.font,
                    TooltipPositionPreference::RelativeToRect(rect, Side::Bottom),
                    tooltip_lines[0].as_ref(),
                    None,
                    &tooltip_lines[1..],
                );
            }
        }
    }

    fn size(&self) -> (f32, f32) {
        self.size
    }
}
