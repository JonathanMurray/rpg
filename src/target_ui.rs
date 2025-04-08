use std::{ops::Deref, vec};

use macroquad::{
    color::{Color, BLACK, LIGHTGRAY, RED, WHITE},
    text::Font,
};

use crate::{
    base_ui::{Container, Drawable, Element, LayoutDirection, Style, TextLine},
    conditions_ui::ConditionsList,
    core::Character,
    game_ui::{ActionPointsRow, ResourceBar},
    stats_ui::{build_stats_table, StatValue},
};

pub struct TargetUi {
    shown: bool,
    font: Font,

    container: Container,
}

impl TargetUi {
    pub fn new(font: Font) -> Self {
        Self {
            shown: true,
            font: font.clone(),
            container: Container::default(),
        }
    }

    pub fn set_character(&mut self, character: Option<impl Deref<Target = Character>>) {
        if let Some(char) = character.as_deref() {
            self.shown = true;
            let mut name_text_line = TextLine::new(char.name, 22, WHITE, Some(self.font.clone()));
            name_text_line.set_depth(BLACK, 2.0);
            name_text_line.set_min_height(22.0);

            let conditions_list =
                ConditionsList::new(self.font.clone(), char.condition_descriptions());

            let armor_text_line = TextLine::new(
                format!("Armor: {}", char.protection_from_armor()),
                18,
                WHITE,
                Some(self.font.clone()),
            );

            let stats_table = build_stats_table(
                &self.font,
                20,
                &[
                    (
                        ("STR", char.base_attributes.strength),
                        &[("Sturdiness", StatValue::U32(char.physical_resistence()))],
                    ),
                    (
                        ("DEX", char.base_attributes.dexterity),
                        &[("Evasion", StatValue::U32(char.defense()))],
                    ),
                    (
                        ("INT", char.base_attributes.intellect),
                        &[("Awareness", StatValue::U32(char.mental_resistence()))],
                    ),
                ],
            );

            let mut action_points_row = ActionPointsRow::new(
                (15.0, 15.0),
                0.25,
                Style {
                    border_color: Some(WHITE),
                    ..Default::default()
                },
            );
            action_points_row.current = char.action_points;
            let mut health_bar = ResourceBar::horizontal(char.health.max, RED, (96.0, 12.0));
            health_bar.current = char.health.current;

            self.container = Container {
                layout_dir: LayoutDirection::Vertical,
                children: vec![
                    Element::Text(name_text_line),
                    Element::Box(Box::new(action_points_row)),
                    Element::Box(Box::new(health_bar)),
                    stats_table,
                    Element::Text(armor_text_line),
                    Element::Box(Box::new(conditions_list)),
                ],
                margin: 10.0,
                style: Style {
                    background_color: Some(Color::new(0.2, 0.2, 0.2, 1.0)),
                    border_color: Some(LIGHTGRAY),
                    padding: 12.0,
                    ..Default::default()
                },

                ..Default::default()
            };
        } else {
            self.shown = false;
        }
    }
}

impl Drawable for TargetUi {
    fn draw(&self, x: f32, y: f32) {
        if !self.shown {
            return;
        }

        self.container.draw(x, y);
    }

    fn size(&self) -> (f32, f32) {
        self.container.size()
    }
}
