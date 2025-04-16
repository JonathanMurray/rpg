use std::collections::HashMap;

use macroquad::{
    color::SKYBLUE,
    texture::{draw_texture_ex, DrawTextureParams},
};

use macroquad::{
    color::{GRAY, WHITE},
    input::mouse_position,
    text::Font,
    texture::Texture2D,
};

use crate::{
    action_button::{draw_tooltip, TooltipPosition},
    base_ui::{table, Align, Container, Drawable, Element, LayoutDirection, Style},
    core::{ArmorPiece, Character, HandType, Shield, Weapon, WeaponRange},
    data::{BOW, CHAIN_MAIL, DAGGER, LEATHER_ARMOR, RAPIER, SMALL_SHIELD, SWORD, WAR_HAMMER},
    textures::{EquipmentIconId, IconId},
};

#[derive(Copy, Clone, Debug)]
enum InventorySlot {
    Weapon(Weapon),
    Shield(Shield),
    Armor(ArmorPiece),
}

impl InventorySlot {
    fn icon(&self) -> EquipmentIconId {
        match self {
            InventorySlot::Weapon(weapon) => weapon.icon,
            InventorySlot::Shield(_shield) => EquipmentIconId::SmallShield,
            InventorySlot::Armor(armor) => armor.icon,
        }
    }
}

fn tooltip(inventory_slot: &InventorySlot) -> Vec<String> {
    match inventory_slot {
        InventorySlot::Weapon(weapon) => weapon_tooltip(weapon),
        InventorySlot::Shield(shield) => shield_tooltip(shield),
        InventorySlot::Armor(armor) => armor_tooltip(armor),
    }
}

fn weapon_tooltip(weapon: &Weapon) -> Vec<String> {
    let mut lines = vec![
        weapon.name.to_string(),
        format!("{} damage / {} AP", weapon.damage, weapon.action_point_cost),
        format!("[{}]", weapon.attack_attribute),
    ];

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
    lines.push(format!("Weight: {}", armor.weight));
    lines
}

pub fn build_inventory_section(
    font: &Font,
    character: &Character,
    equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
) -> Element {
    let mut icons = vec![EquipmentIcon::new(None, font.clone(), vec![]); 9];

    let inventory_slots = [
        Some(InventorySlot::Weapon(DAGGER)),
        Some(InventorySlot::Weapon(SWORD)),
        Some(InventorySlot::Weapon(RAPIER)),
        Some(InventorySlot::Weapon(WAR_HAMMER)),
        Some(InventorySlot::Weapon(BOW)),
        Some(InventorySlot::Shield(SMALL_SHIELD)),
        Some(InventorySlot::Armor(CHAIN_MAIL)),
        Some(InventorySlot::Armor(LEATHER_ARMOR)),
    ];

    for i in 0..inventory_slots.len() {
        if let Some(Some(inventory_slot)) = inventory_slots.get(i) {
            let icon_cell = &mut icons[i];
            icon_cell.texture = Some(equipment_icons[&inventory_slot.icon()].clone());
            icon_cell.tooltip_lines = tooltip(inventory_slot);
        }
    }

    let mut rows = vec![];

    while !icons.is_empty() {
        rows.push(Element::Container(Container {
            layout_dir: LayoutDirection::Horizontal,
            children: icons
                .drain(0..3)
                .take(3)
                .map(|cell| Element::Box(Box::new(cell)))
                .collect(),
            margin: 2.0,
            ..Default::default()
        }))
    }

    Element::Container(Container {
        layout_dir: LayoutDirection::Vertical,
        children: rows,
        align: Align::Center,
        margin: 5.0,
        ..Default::default()
    })
}

pub fn build_equipped_section(
    font: &Font,
    character: &Character,
    equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
) -> Element {
    let mut eq_text_cells = vec![];
    let mut eq_icon_cells = vec![EquipmentIcon::new(None, font.clone(), vec![]); 3];
    for hand in [HandType::MainHand, HandType::OffHand] {
        if let Some(weapon) = character.weapon(hand) {
            eq_text_cells.push("Attack dmg".to_string());
            eq_text_cells.push(format!("{}", weapon.damage));

            eq_text_cells.push("Attack mod".to_string());
            eq_text_cells.push(format!("+{}", character.attack_modifier(hand)));

            let texture = Some(equipment_icons[&weapon.icon].clone());
            let icon_cell = match hand {
                HandType::MainHand => &mut eq_icon_cells[0],
                HandType::OffHand => &mut eq_icon_cells[2],
            };

            icon_cell.texture = texture;
            icon_cell.tooltip_lines = weapon_tooltip(&weapon);
        }
    }
    if let Some(shield) = character.shield() {
        eq_text_cells.push("Evasion bonus".to_string());
        eq_text_cells.push(format!("{}", shield.evasion));
        let icon_cell = &mut eq_icon_cells[2];
        icon_cell.texture = Some(equipment_icons[&EquipmentIconId::SmallShield].clone());
        icon_cell.tooltip_lines = shield_tooltip(&shield);
    }
    if let Some(armor) = character.armor {
        eq_text_cells.push("Armor".to_string());
        eq_text_cells.push(format!("{}", armor.protection));
        let icon_cell = &mut eq_icon_cells[1];
        icon_cell.texture = Some(equipment_icons[&armor.icon].clone());
        icon_cell.tooltip_lines = armor_tooltip(&armor);
    }

    eq_text_cells.push("Weight".to_string());
    eq_text_cells.push(format!(
        "{} / {}",
        character.equipment_weight(),
        character.capacity
    ));

    let equipment_icons = Element::Container(Container {
        layout_dir: LayoutDirection::Horizontal,
        children: eq_icon_cells
            .into_iter()
            .map(|cell| Element::Box(Box::new(cell)))
            .collect(),
        margin: 2.0,
        ..Default::default()
    });

    let equipment_table = table(eq_text_cells, vec![Align::End, Align::Start], font.clone());

    Element::Container(Container {
        layout_dir: LayoutDirection::Vertical,
        children: vec![equipment_icons, equipment_table],
        align: Align::Center,
        margin: 15.0,
        ..Default::default()
    })
}

#[derive(Clone)]
struct EquipmentIcon {
    texture: Option<Texture2D>,
    style: Style,
    size: (f32, f32),
    font: Font,
    tooltip_lines: Vec<String>,
}

impl EquipmentIcon {
    pub fn new(texture: Option<Texture2D>, font: Font, tooltip_lines: Vec<String>) -> Self {
        Self {
            texture,
            font,
            tooltip_lines,
            style: Style {
                background_color: Some(SKYBLUE),
                border_color: Some(GRAY),
                border_width: Some(2.0),
                ..Default::default()
            },
            size: (40.0, 40.0),
        }
    }
}

impl Drawable for EquipmentIcon {
    fn draw(&self, x: f32, y: f32) {
        self.style.draw(x, y, self.size);
        let params = DrawTextureParams {
            dest_size: Some(self.size.into()),
            ..Default::default()
        };
        if let Some(texture) = &self.texture {
            draw_texture_ex(texture, x, y, WHITE, params);
        }
    }

    fn draw_tooltips(&self, x: f32, y: f32) {
        let (mouse_x, mouse_y) = mouse_position();
        if (x..x + self.size.0).contains(&mouse_x)
            && (y..y + self.size.1).contains(&mouse_y)
            && !self.tooltip_lines.is_empty()
        {
            draw_tooltip(
                &self.font,
                TooltipPosition::BottomLeft((x, y)),
                &self.tooltip_lines,
            );
        }
    }

    fn size(&self) -> (f32, f32) {
        self.size
    }
}
