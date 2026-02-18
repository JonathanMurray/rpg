use std::{cell::Cell, default, rc::Rc};

use macroquad::{
    color::{Color, BLACK, LIGHTGRAY, MAGENTA, WHITE},
    shapes::draw_rectangle,
    text::Font,
    window::{screen_height, screen_width},
};

use crate::{
    base_ui::{Align, Checkbox, Container, Element, LayoutDirection, Style, TextLine},
    sounds::SoundPlayer,
};

pub fn build_settings(
    big_font: &Font,
    simple_font: &Font,
    sound_player: SoundPlayer,
    faster_movement: Rc<Cell<bool>>,
) -> Container {
    Container {
        layout_dir: LayoutDirection::Vertical,
        align: Align::Center,
        border_between_children: Some(LIGHTGRAY),
        margin: 40.0,
        style: Style {
            background_color: Some(Color::new(0.0, 0.0, 0.0, 0.7)),
            padding: 15.0,
            ..Default::default()
        },
        children: vec![
            Element::Container(Container {
                layout_dir: LayoutDirection::Vertical,
                align: Align::End,
                margin: 5.0,
                children: vec![
                    Element::Container(Container {
                        layout_dir: LayoutDirection::Horizontal,
                        align: Align::Center,
                        margin: 5.0,
                        children: vec![
                            Element::Text(TextLine::new(
                                "Sound",
                                16,
                                WHITE,
                                Some(simple_font.clone()),
                            )),
                            Element::Box(Box::new(Checkbox::new(
                                (16.0, 16.0),
                                sound_player.clone(),
                                sound_player.enabled.clone(),
                            ))),
                        ],
                        ..Default::default()
                    }),
                    Element::Container(Container {
                        layout_dir: LayoutDirection::Horizontal,
                        align: Align::Center,
                        margin: 5.0,
                        children: vec![
                            Element::Text(TextLine::new(
                                "Faster movement",
                                16,
                                WHITE,
                                Some(simple_font.clone()),
                            )),
                            Element::Box(Box::new(Checkbox::new(
                                (16.0, 16.0),
                                sound_player.clone(),
                                faster_movement,
                            ))),
                        ],
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            }),
            Element::Text(TextLine::new("Settings", 12, WHITE, Some(big_font.clone()))),
        ],

        ..Default::default()
    }
}
