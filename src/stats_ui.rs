use std::{fmt, rc::Rc};

use macroquad::{
    color::{Color, BLACK, GRAY, LIGHTGRAY, WHITE},
    text::Font,
};

use crate::{
    base_ui::{
        Align, Border, Container, Drawable, Element, LayoutDirection, SimpleTooltip, Style,
        TextLine,
    },
    core::{Character, HandType},
    game_ui_components::character_defense_section,
    resources::{BIG_FONT, SIMPLE_FONT},
    tooltip::Keyword,
    util::COL_TEXT_FADED,
};

pub struct CharacterStatsTable {
    element: Element,
    simple_font: Font,
    big_font: Font,
    character: Rc<Character>,
}

impl Drawable for CharacterStatsTable {
    fn draw(&self, x: f32, y: f32) {
        self.element.draw(x, y)
    }

    fn draw_tooltips(&self, x: f32, y: f32) {
        self.element.draw_tooltips(x, y);
    }

    fn size(&self) -> (f32, f32) {
        self.element.size()
    }
}

impl CharacterStatsTable {
    pub fn rebuild(&mut self) {
        *self = build_character_stats_table(
            &self.simple_font.clone(),
            &self.big_font,
            self.character.clone(),
        )
    }
}

pub fn build_character_stats_table(
    simple_font: &Font,
    big_font: &Font,

    character: Rc<Character>,
) -> CharacterStatsTable {
    let mut main_stat_cells = vec![];
    for (label, value, name, tooltip_lines) in [
        (
            "STR",
            character.base_attributes.strength.get(),
            "Strength",
            &[
                "Each point grants:",
                "|<heart>| |<value>2|",
                "|<stamina>| |<value>1|",
                "|<red_dice>| |<value>+1| |<faded>(melee attack)|",
                "|<toughness>| |<stat>Toughness| |<value>2|",
                "|<weight>| |<value>2| |<faded>(capacity)|",
            ][..],
        ),
        (
            "AGI",
            character.base_attributes.agility.get(),
            "Agility",
            &[
                "Each point grants:",
                "|<stamina>| |<value>1|",
                "|<boot>| |<value>0.5|",
                "|<red_dice>| |<value>+1| |<faded>(ranged attack)|",
                "|<evasion>| |<stat>Evasion| |<value>1|",
            ],
        ),
        (
            "INT",
            character.base_attributes.intellect.get(),
            "Intellect",
            &[
                "Each point grants:",
                "|<mixed_dice>| |<value>+1| |<faded>(spell & attack)|",
                "|<will>| |<stat>Will| |<value>2|",
                "|<evasion>| |<stat>Evasion| |<value>0.5|",
            ],
        ),
        (
            "SPI",
            character.base_attributes.spirit.get(),
            "Spirit",
            &[
                "Each point grants:",
                "|<mana>| |<value>2|",
                "|<blue_dice>| |<value>+1| |<faded>(spell)|",
            ],
        ),
    ] {
        let label_line =
            TextLine::new(label, 16, LIGHTGRAY, Some(simple_font.clone())).with_depth(BLACK, 1.0);
        let value_line = TextLine::new(format!("{}", value), 12, WHITE, Some(big_font.clone()))
            .with_depth(BLACK, 2.0);
        let tooltip = SimpleTooltip::new(
            name,
            tooltip_lines.iter().map(|s| s.to_string()).collect(),
            vec![],
        );
        let cell = Element::Container(Container {
            layout_dir: LayoutDirection::Vertical,
            align: Align::Center,
            margin: 10.0,
            children: vec![Element::Text(label_line), Element::Text(value_line)],
            tooltip: Some(tooltip),
            ..Default::default()
        });
        main_stat_cells.push(cell);
    }

    let main_stat_row = Element::Container(Container {
        layout_dir: LayoutDirection::Horizontal,
        margin: 20.0,
        children: main_stat_cells,
        ..Default::default()
    });

    let stats = vec![
        (
            "Move |<boot>|",
            "Movement |<boot>|",
            format!("{}", character.base_free_movement.get()),
            vec!["You can move this far every turn without spending |<ap>| AP.".to_string()],
            vec![],
        ),
        (
            "Spell mod |<blue_dice>|",
            "Spell modifier |<blue_dice>|",
            format!("+{}", character.spell_modifier()),
            vec![
                "Added to your |<blue_dice>| |<stat>Spell| rolls.".to_string(),
                "Higher rolls mean more favourable outcomes.".to_string(),
            ],
            vec![Keyword::Crit, Keyword::StrongWeak],
        ),
        (
            "Atk mod |<red_dice>|",
            "Attack modifier |<red_dice>|",
            format!("+{}", character.base_attack_modifier(HandType::MainHand)),
            vec![
                "Added to your |<red_dice>| |<stat>Attack| rolls.".to_string(),
                "Higher rolls mean more favourable outcomes.".to_string(),
            ],
            vec![Keyword::Crit, Keyword::StrongWeak],
        ),
        (
            "Atk dmg |<sword>|",
            "Attack damage |<sword>|",
            character
                .weapon_damage_str(HandType::MainHand)
                .unwrap_or("-".to_string()),
            vec!["The unmodified base damage of one attack.".to_string()],
            vec![],
        ),
        (
            "Armor |<shield>|",
            "Armor |<shield>|",
            character.armor_str(),
            vec!["The damage of incoming attacks is reduced by this amount.".to_string()],
            vec![],
        ),
        (
            "Capacity |<weight>|",
            "Capacity |<weight>|",
            format!(
                "{} / {}",
                character.equipment_weight(),
                character.capacity.get()
            ),
            vec![
                "Carrying or equipping more than your capacity makes you |<keyword>Encumbered|."
                    .to_string(),
            ],
            vec![],
        ),
    ];

    let stat_row = |(short_header, long_header, value, tooltip, keywords): (
        &str,
        &str,
        String,
        Vec<String>,
        Vec<Keyword>,
    )| {
        let mut left = TextLine::new(short_header, 16, COL_TEXT_FADED, Some(simple_font.clone()));
        left.set_min_width(100.0, true);
        let right = Container {
            children: vec![Element::Text(TextLine::new(
                value,
                16,
                WHITE,
                Some(simple_font.clone()),
            ))],
            align: Align::Center,
            layout_dir: LayoutDirection::Vertical,
            min_width: Some(40.0),
            ..Default::default()
        };
        Element::Container(Container {
            children: vec![Element::Text(left), Element::Container(right)],

            tooltip: Some(SimpleTooltip::new(
                long_header.to_string(),
                tooltip,
                keywords,
            )),
            margin: 15.0,
            align: Align::Center,
            style: Style {
                padding: 4.0,
                ..Default::default()
            },
            ..Default::default()
        })
    };

    let mut children = vec![
        Element::Empty(0.0, 4.0),
        main_stat_row,
        Element::Empty(0.0, 18.0),
    ];
    for stat in stats {
        children.push(stat_row(stat));
    }
    children.extend([
        Element::Empty(0.0, 12.0),
        character_defense_section(&character),
    ]);

    let element = Element::Container(Container {
        layout_dir: LayoutDirection::Vertical,
        align: Align::Center,
        margin: 2.0,
        style: Style {
            border: Some(Border {
                color: GRAY,
                double: Some((GRAY.with_alpha(0.5), 3.0)),
                ..Default::default()
            }),
            bg_color: Some(BLACK.with_alpha(0.2)),
            padding: 14.0,
            ..Default::default()
        },
        children,
        ..Default::default()
    });

    CharacterStatsTable {
        element,
        simple_font: simple_font.clone(),
        big_font: big_font.clone(),
        character,
    }
}
