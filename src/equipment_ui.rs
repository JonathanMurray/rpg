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
    core::{Character, HandType, Range},
    textures::EquipmentIconId,
};

pub fn create_equipment_ui(
    font: &Font,
    character: &Character,
    equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
) -> Element {
    let mut eq_text_cells = vec![];
    let mut eq_icon_cells = vec![EquipmentIcon::new(None, font.clone(), vec![]); 3];
    for hand in [HandType::MainHand, HandType::OffHand] {
        if let Some(weapon) = character.weapon(hand) {
            eq_text_cells.push("Weapon dmg".to_string());
            eq_text_cells.push(format!("{}", weapon.damage));

            eq_text_cells.push("Attack mod".to_string());
            eq_text_cells.push(format!("{}", character.attack_modifier(hand)));

            let texture = Some(equipment_icons[&weapon.icon].clone());
            let icon_cell = match hand {
                HandType::MainHand => &mut eq_icon_cells[0],
                HandType::OffHand => &mut eq_icon_cells[2],
            };

            icon_cell.texture = texture;
            icon_cell.tooltip_lines = vec![
                weapon.name.to_string(),
                format!(
                    "{} dmg ({} AP) [{}]",
                    weapon.damage, weapon.action_point_cost, weapon.attack_attribute
                ),
            ];

            if weapon.range != Range::Melee {
                icon_cell
                    .tooltip_lines
                    .push(format!("Range: {}", weapon.range));
            }

            if let Some(effect) = weapon.on_true_hit {
                icon_cell.tooltip_lines.push(format!("[true hit] {effect}"));
            }
            if let Some(reaction) = weapon.on_attacked_reaction {
                icon_cell
                    .tooltip_lines
                    .push(format!("[attacked?] {}", reaction.name));
            }
            if let Some(enhancement) = weapon.attack_enhancement {
                icon_cell
                    .tooltip_lines
                    .push(format!("[+] {}", enhancement.name));
            }
        }
    }
    if let Some(shield) = character.shield() {
        eq_text_cells.push("+ Evasion".to_string());
        eq_text_cells.push(format!("{}", shield.evasion));
        let icon_cell = &mut eq_icon_cells[2];
        icon_cell.texture = Some(equipment_icons[&EquipmentIconId::SmallShield].clone());
        icon_cell.tooltip_lines = vec![shield.name.to_string(), format!("{} def", shield.evasion)];
        if let Some(reaction) = shield.on_hit_reaction {
            icon_cell
                .tooltip_lines
                .push(format!("[hit?] {}", reaction.name));
        }
    }
    if let Some(armor) = character.armor {
        eq_text_cells.push("Armor".to_string());
        eq_text_cells.push(format!("{}", armor.protection));
        let icon_cell = &mut eq_icon_cells[1];
        icon_cell.texture = Some(equipment_icons[&armor.icon].clone());
        icon_cell.tooltip_lines = vec![
            armor.name.to_string(),
            format!("{} armor", armor.protection),
        ];
        if let Some(limit) = armor.limit_evasion_from_agi {
            icon_cell
                .tooltip_lines
                .push(format!("Max {} evasion from agi", limit));
        }
    }

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
        margin: 5.0,
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
