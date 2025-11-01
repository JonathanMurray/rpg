use std::fmt;

use macroquad::{
    color::{GRAY, WHITE},
    text::Font,
};

use crate::{
    base_ui::{Align, Container, Drawable, Element, LayoutDirection, Style, TextLine},
    core::Character,
};

type AttributeCell = (&'static str, u32);

pub fn build_character_stats_table(font: &Font, character: &Character) -> Element {
    build_stats_table(
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
    )
}

pub fn build_stats_table(
    font: &Font,
    attribute_font_size: u16,
    rows: &[(Option<AttributeCell>, &[(&'static str, StatValue)])],
) -> Element {
    let mut attribute_rows = vec![];

    let mut attribute_text_width = 0.0;
    let mut attribute_text_lines = vec![];

    for row in rows {
        let text = if let Some(attribute) = row.0 {
            format!("{}: {}", attribute.0, attribute.1)
        } else {
            " ".to_string()
        };

        let attribute_text_line =
            TextLine::new(text, attribute_font_size, WHITE, Some(font.clone()));

        if attribute_text_line.size().0 > attribute_text_width {
            attribute_text_width = attribute_text_line.size().0;
        }
        attribute_text_lines.push(attribute_text_line);
    }

    // Make the rows of the right colum aligned with eachother
    for line in &mut attribute_text_lines {
        line.set_min_width(attribute_text_width, true);
    }

    for row in rows {
        attribute_rows.push(Element::Container(attribute_row(
            attribute_text_lines.remove(0),
            row.1,
            font.clone(),
        )));
    }

    Element::Container(Container {
        layout_dir: LayoutDirection::Vertical,
        children: attribute_rows,
        //border_between_children: Some(DARKGRAY),
        margin: 5.0,
        style: Style {
            border_color: Some(GRAY),
            padding: 5.0,
            ..Default::default()
        },
        ..Default::default()
    })
}

pub enum StatValue {
    U32(u32),
    F32(f32),
    String(String),
}

impl fmt::Display for StatValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StatValue::U32(x) => f.write_fmt(format_args!("{}", x)),
            StatValue::F32(x) => f.write_fmt(format_args!("{:.1}", x)),
            StatValue::String(s) => f.write_str(s),
        }
    }
}

fn attribute_row(
    attribute_text_line: TextLine,
    stats: &[(&'static str, StatValue)],
    font: Font,
) -> Container {
    let stat_rows: Vec<Element> = stats
        .iter()
        .map(|(name, value)| {
            Element::Text(TextLine::new(
                format!("{} = {}", name, value),
                16,
                WHITE,
                Some(font.clone()),
            ))
        })
        .collect();

    let stats_list = Element::Container(Container {
        layout_dir: LayoutDirection::Vertical,
        margin: 8.0,
        children: stat_rows,
        ..Default::default()
    });
    Container {
        layout_dir: LayoutDirection::Horizontal,
        margin: 20.0,
        align: Align::Center,
        children: vec![Element::Text(attribute_text_line), stats_list],
        style: Style {
            padding: 5.0,
            ..Default::default()
        },
        ..Default::default()
    }
}
