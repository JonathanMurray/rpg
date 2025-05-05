use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use macroquad::{
    color::{Color, BLACK, RED, SKYBLUE},
    math::Rect,
    shapes::draw_rectangle,
    texture::{draw_texture_ex, DrawTextureParams},
};

use macroquad::{
    color::{GRAY, WHITE},
    input::mouse_position,
    text::Font,
    texture::Texture2D,
};

use crate::{
    action_button::{draw_tooltip, TooltipPositionPreference},
    base_ui::{
        table, Align, Container, Drawable, Element, LayoutDirection, Style, TableCell, TableStyle,
    },
    core::{
        ArmorPiece, Character, EquipmentEntry, EquipmentSlotRole, HandType, Shield, Weapon,
        WeaponGrip, WeaponRange,
    },
    textures::EquipmentIconId,
};

fn tooltip(entry: &EquipmentEntry) -> Vec<String> {
    match entry {
        EquipmentEntry::Weapon(weapon) => weapon_tooltip(weapon),
        EquipmentEntry::Shield(shield) => shield_tooltip(shield),
        EquipmentEntry::Armor(armor) => armor_tooltip(armor),
    }
}

fn weapon_tooltip(weapon: &Weapon) -> Vec<String> {
    let mut lines = vec![
        weapon.name.to_string(),
        format!("{} damage / {} AP", weapon.damage, weapon.action_point_cost),
    ];

    if weapon.grip == WeaponGrip::TwoHanded {
        lines.push("Two-handed".to_string());
    }

    if weapon.range != WeaponRange::Melee {
        lines.push(format!("Range: {}", weapon.range));
    }
    if let Some(effect) = weapon.on_true_hit {
        lines.push(format!("[true hit] {effect}"));
    }
    if let Some(reaction) = weapon.on_attacked_reaction {
        lines.push(format!("[attacked?] {}", reaction.name));
    }
    if let Some(enhancement) = weapon.attack_enhancement {
        lines.push(format!("[+] {}", enhancement.name));
    }
    lines.push(format!("Weight: {}", weapon.weight));

    lines
}

fn shield_tooltip(shield: &Shield) -> Vec<String> {
    let mut lines = vec![
        shield.name.to_string(),
        format!("+{} evasion", shield.evasion),
    ];

    if let Some(reaction) = shield.on_hit_reaction {
        lines.push(format!("[hit?] {}", reaction.name));
    }
    lines.push(format!("Weight: {}", shield.weight));
    lines
}

fn armor_tooltip(armor: &ArmorPiece) -> Vec<String> {
    let mut lines = vec![
        armor.name.to_string(),
        format!("{} armor", armor.protection),
    ];
    if let Some(limit) = armor.limit_evasion_from_agi {
        lines.push(format!("Max {} evasion from agi", limit));
    }
    if armor.equip.bonus_spell_modifier > 0 {
        lines.push(format!("+{} spell mod", armor.equip.bonus_spell_modifier));
    }
    lines.push(format!("Weight: {}", armor.weight));
    lines
}

pub fn build_inventory_section(
    font: &Font,
    character: &Character,
    equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
) -> (Element, Vec<Rc<RefCell<EquipmentSlot>>>) {
    let mut slots: Vec<Rc<RefCell<EquipmentSlot>>> = character
        .inventory
        .iter()
        .enumerate()
        .map(|(i, maybe_entry)| {
            maybe_entry
                .get()
                .map(|entry| {
                    EquipmentSlot::new(
                        font.clone(),
                        Some((equipment_icons[&entry.icon()].clone(), entry)),
                        EquipmentSlotRole::Inventory(i),
                        None,
                    )
                })
                .unwrap_or(EquipmentSlot::new(
                    font.clone(),
                    None,
                    EquipmentSlotRole::Inventory(i),
                    None,
                ))
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
    let mut slots = [
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
    ];

    for hand in [HandType::MainHand, HandType::OffHand] {
        if let Some(weapon) = character.weapon(hand) {
            let texture = equipment_icons[&weapon.icon].clone();
            slots[0].content = Some(EquipmentSlotContent::new(
                texture,
                EquipmentEntry::Weapon(weapon),
            ));
        }
    }
    if let Some(shield) = character.shield() {
        let texture = equipment_icons[&shield.icon].clone();
        slots[2].content = Some(EquipmentSlotContent::new(
            texture,
            EquipmentEntry::Shield(shield),
        ));
    }
    if let Some(armor) = character.armor.get() {
        let texture = equipment_icons[&armor.icon].clone();
        slots[1].content = Some(EquipmentSlotContent::new(
            texture,
            EquipmentEntry::Armor(armor),
        ));
    }

    let slots: Vec<Rc<RefCell<EquipmentSlot>>> = slots
        .into_iter()
        .map(|slot| Rc::new(RefCell::new(slot)))
        .collect();

    let cloned_slots: Vec<Rc<RefCell<EquipmentSlot>>> = slots.iter().map(Rc::clone).collect();

    let slots_container = Element::Container(Container {
        layout_dir: LayoutDirection::Horizontal,
        children: slots
            .into_iter()
            .map(|slot| Element::RcRefCell(slot))
            .collect(),
        margin: 2.0,
        ..Default::default()
    });

    let stats_table = Rc::new(RefCell::new(EquipmentStatsTable::new(character, font)));

    let container = Element::Container(Container {
        layout_dir: LayoutDirection::Vertical,
        children: vec![slots_container, Element::RcRefCell(stats_table.clone())],
        align: Align::Center,
        margin: 15.0,
        ..Default::default()
    });

    (container, cloned_slots, stats_table)
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
        for hand in [HandType::MainHand, HandType::OffHand] {
            if let Some(weapon) = character.weapon(hand) {
                cells.push("Attack dmg".into());
                cells.push(format!("{}", weapon.damage).into());

                cells.push("Attack mod".into());
                cells.push(format!("+{}", character.attack_modifier(hand)).into());
            }
        }
        if let Some(shield) = character.shield() {
            cells.push("Evasion bonus".into());
            cells.push(format!("{}", shield.evasion).into());
        }
        if let Some(armor) = character.armor.get() {
            cells.push("Armor".into());
            cells.push(format!("{}", armor.protection).into());
        }

        cells.push("Weight".into());

        let text = format!("{} / {}", character.equipment_weight(), character.capacity);
        let color_override = if character.equipment_weight() > character.capacity {
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
    tooltip_lines: Vec<String>,
}

impl EquipmentSlotContent {
    pub fn new(texture: Texture2D, equipment: EquipmentEntry) -> Self {
        Self {
            texture,
            tooltip_lines: tooltip(&equipment),
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
        self.style.draw(x, y, self.size);
        let params = DrawTextureParams {
            dest_size: Some(self.size.into()),
            ..Default::default()
        };
        if let Some(content) = &self.content {
            draw_texture_ex(&content.texture, x, y, WHITE, params);
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
            if hover && !content.tooltip_lines.is_empty() {
                draw_tooltip(
                    &self.font,
                    rect,
                    TooltipPositionPreference::Bottom,
                    &content.tooltip_lines,
                );
            }
        } else if let Some((_texture, tooltip_lines)) = &self.placeholder {
            if hover {
                draw_tooltip(
                    &self.font,
                    rect,
                    TooltipPositionPreference::Bottom,
                    &tooltip_lines,
                );
            }
        }
    }

    fn size(&self) -> (f32, f32) {
        self.size
    }
}
