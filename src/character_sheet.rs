use std::{
    collections::HashMap,
    rc::Rc,
};

use macroquad::color::{DARKBLUE, SKYBLUE};

use macroquad::{
    color::{
        Color, BLACK, LIGHTGRAY, WHITE,
    },
    text::Font,
    texture::Texture2D,
};

use crate::{
    action_button::ActionButton,
    base_ui::{
        Align, Container, ContainerScroll, Element, LayoutDirection, Style,
        TextLine,
    },
    core::Character,
    equipment_ui::create_equipment_ui,
    stats_ui::{build_stats_table, StatValue},
    textures::EquipmentIconId,
};

pub fn build_character_sheet(
    font: &Font,
    character: &Character,
    equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
    attack_button: Option<Rc<ActionButton>>,
    reaction_buttons: Vec<Rc<ActionButton>>,
    attack_enhancement_buttons: Vec<Rc<ActionButton>>,
    spell_buttons: Vec<(Rc<ActionButton>, Option<Rc<ActionButton>>)>,
) -> Container {
    let stats_table = build_stats_table(
        font,
        20,
        &[
            (
                Some(("Strength", character.base_attributes.strength)),
                &[
                    ("Health", StatValue::U32(character.health.max)),
                    ("Toughness", StatValue::U32(character.toughness())),
                ],
            ),
            (None, &[("Stamina", StatValue::U32(character.stamina.max))]),
            (
                Some(("Agility", character.base_attributes.agility)),
                &[("Movement", StatValue::F32(character.move_range))],
            ),
            (None, &[("Evasion", StatValue::U32(character.evasion()))]),
            (
                Some(("Intellect", character.base_attributes.intellect)),
                &[
                    ("Will", StatValue::U32(character.will())),
                    (
                        "Reactive AP",
                        StatValue::U32(character.reactive_action_points),
                    ),
                ],
            ),
            (
                None,
                &[("Spell mod", StatValue::U32(character.spell_modifier()))],
            ),
            (
                Some(("Spirit", character.base_attributes.spirit)),
                &[("Mana", StatValue::U32(character.mana.max))],
            ),
        ],
    );

    let equipment_section = create_equipment_ui(font, character, equipment_icons);

    let mut spell_book_rows = Container {
        layout_dir: LayoutDirection::Vertical,
        margin: 5.0,
        children: vec![],
        scroll: Some(ContainerScroll::default()),
        max_height: Some(450.0),
        style: Style {
            padding: 10.0,
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

    for (spell_btn, enhancement_btn) in spell_buttons.into_iter() {
        let spell = spell_btn.action.unwrap_spell();
        spell_book_rows.children.push(Element::Text(TextLine::new(
            spell.name,
            16,
            WHITE,
            Some(font.clone()),
        )));

        let mut row_buttons = vec![spell_btn.clone()];
        if let Some(enhancement_btn) = enhancement_btn {
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

    Container {
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
                    .with_padding(10.0),
            ),
            Element::Container(Container {
                layout_dir: LayoutDirection::Horizontal,
                margin: 20.0,
                border_between_children: Some(LIGHTGRAY),
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
                        border_between_children: Some(LIGHTGRAY),
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
                            Element::Text(
                                TextLine::new("Equipment", 22, WHITE, Some(font.clone()))
                                    .with_depth(BLACK, 2.0),
                            ),
                            equipment_section,
                        ],

                        ..Default::default()
                    }),
                ],
                ..Default::default()
            }),
        ],
        ..Default::default()
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
