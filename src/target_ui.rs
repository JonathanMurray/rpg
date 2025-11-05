use std::{rc::Rc, vec};

use macroquad::{
    color::{Color, BLACK, GRAY, LIGHTGRAY, RED, WHITE},
    shapes::{draw_rectangle, draw_rectangle_lines},
    text::{draw_text_ex, measure_text, Font, TextParams},
    window::screen_width,
};

use crate::{
    base_ui::{
        Align, Container, Drawable, Element, LayoutDirection, Style, TableStyle, TextLine, table
    },
    conditions_ui::ConditionsList,
    core::{Character, Goodness, HandType},
    game_ui_components::{ActionPointsRow, ResourceBar},
};

pub struct TargetUi {
    target: Option<Rc<Character>>,
    big_font: Font,
    simple_font: Font,

    container: Container,

    action: Option<(String, Vec<(String, Goodness)>, bool)>,
}

impl TargetUi {
    pub fn new(big_font: Font, simple_font: Font) -> Self {
        Self {
            target: Default::default(),
            big_font,
            simple_font,
            container: Container::default(),
            action: None,
        }
    }

    pub fn rebuild_character_ui(&mut self) {
        if let Some(target) = self.target.take() {
            self.set_character(Some(&target));
        }
    }

    pub fn clear_character_if_dead(&mut self) {
        if let Some(character) = &self.target {
            if character.is_dead() {
                self.target = None;
            }
        }
    }

    pub fn set_character(&mut self, character: Option<&Rc<Character>>) {
        if let Some(char) = character {
            self.target = Some(Rc::clone(char));
            let mut name_text_line =
                TextLine::new(char.name, 16, WHITE, Some(self.big_font.clone()));
            name_text_line.set_depth(BLACK, 2.0);
            name_text_line.set_min_height(20.0);

            let conditions_list =
                ConditionsList::new(self.simple_font.clone(), char.condition_infos());

            let armor_text_line = TextLine::new(
                format!("Armor: {}", char.protection_from_armor()),
                22,
                WHITE,
                Some(self.simple_font.clone()),
            );


            let def_table = table(
                vec![
                    "Toughness".into(),
                    "Evasion".into(),
                    "Will".into(),
                    char.toughness().to_string().into(),
                    char.evasion().to_string().into(),
                    char.will().to_string().into(),
                ],
                vec![Align::Center, Align::Center, Align::Center],
                self.simple_font.clone(),
                TableStyle {
                    outer_border_color: None,
                    inner_border_color: None,
                    all_columns_same_width: true,
                    row_font_sizes: &[16, 24],
                    cell_padding: (3.0, 5.0),
                    ..Default::default()
                },
            );

            let mut action_points_row = ActionPointsRow::new(
                char.max_reactive_action_points,
                (15.0, 15.0),
                0.25,
                Style {
                    border_color: Some(WHITE),
                    ..Default::default()
                },
            );
            action_points_row.current_ap = char.action_points.current();
            let mut health_bar = ResourceBar::horizontal(char.health.max(), RED, (80.0, 10.0));
            health_bar.current = char.health.current();

            let health_text_line = TextLine::new(
                format!("{} / {}", char.health.current(), char.health.max()),
                18,
                WHITE,
                Some(self.simple_font.clone()),
            );
            //health_text_line.set_depth(BLACK, 2.0);
            //health_text_line.set_min_height(20.0);

            let centered_list = Container {
                layout_dir: LayoutDirection::Vertical,
                align: Align::Center,
                children: vec![
                    Element::Text(name_text_line),
                    Element::Box(Box::new(health_bar)),
                    Element::Text(health_text_line),
                    Element::Box(Box::new(action_points_row)),
                    Element::Empty(1.0, 4.0),
                    Element::Container(def_table),
                    Element::Empty(1.0, 4.0),
                    Element::Text(armor_text_line),
                    Element::Empty(1.0, 4.0),
                ],
                margin: 3.0,

                ..Default::default()
            };

            
            let movement_text_line = TextLine::new(
                format!("Movement: {}", char.move_speed()),
                16,
                LIGHTGRAY,
                Some(self.simple_font.clone()),
            );
            let attack_text_line = TextLine::new(
                format!("Attack mod: {}", char.attack_modifier(HandType::MainHand)),
                16,
                LIGHTGRAY,
                Some(self.simple_font.clone()),
            );
            let detailed_stats = Container {
                layout_dir: LayoutDirection::Vertical,
                children: vec![
                    Element::Text(movement_text_line),
                    Element::Text(attack_text_line),
                ],
                margin: 7.0,
                style: Style { padding: 5.0, ..Default::default() },
                ..Default::default()
            };

            self.container = Container {
                layout_dir: LayoutDirection::Vertical,
                align: Align::Start,
                children: vec![
                    Element::Container(centered_list),
                    Element::Container(detailed_stats),
                    Element::Box(Box::new(conditions_list)),
                ],
                margin: 10.0,
                style: Style {
                    background_color: Some(Color::new(0.4, 0.3, 0.2, 1.0)),
                    border_color: Some(LIGHTGRAY),
                    padding: 10.0,
                    ..Default::default()
                },
                border_between_children: Some(GRAY),

                ..Default::default()
            }
        } else {
            self.target = None;
        }
    }

    pub fn clear_action(&mut self) {
        self.action = None;
    }

    pub fn set_action(
        &mut self,
        header: String,
        details: Vec<(String, Goodness)>,
        only_show_with_target: bool,
    ) {
        self.action = Some((header, details, only_show_with_target));
    }

    fn draw_action(&self, container_pos: (f32, f32)) {
        let Some((header, details, only_show_with_target)) = &self.action else {
            return;
        };

        if *only_show_with_target && self.target.is_none() {
            return;
        }

        let (mut x, y) = container_pos;

        let header_font_size = 20;
        let detail_font_size = 20;
        let params = TextParams {
            font: Some(&self.big_font),
            font_size: header_font_size,
            color: WHITE,
            ..Default::default()
        };

        let vert_margin = 3.0;
        let detail_hor_margin = 5.0;
        let header_pad = 8.0;
        let detail_pad = 5.0;

        let header_dimensions = measure_text(header, Some(&self.big_font), header_font_size, 1.0);
        let header_w = header_dimensions.width + header_pad * 2.0;
        let mut header_h = 0.0;
        if header_dimensions.height.is_finite() {
            header_h = header_dimensions.height + header_pad * 2.0;
        }

        let mut details_w = 0.0;
        let mut details_h = 0.0;
        let mut details_max_offset = 0.0;
        if !details.is_empty() {
            let mut details_relative_y_interval = [f32::MAX, f32::MIN];

            for (line, _goodness) in details.iter() {
                let dim = measure_text(line, Some(&self.simple_font), detail_font_size, 1.0);
                details_w += dim.width;
                if dim.height.is_finite() {
                    if dim.offset_y > details_max_offset {
                        details_max_offset = dim.offset_y;
                    }
                    let top = -dim.offset_y;
                    let bot = -dim.offset_y + dim.height;
                    if top < details_relative_y_interval[0] {
                        details_relative_y_interval[0] = top;
                    }
                    if bot > details_relative_y_interval[1] {
                        details_relative_y_interval[1] = bot;
                    }
                }
            }
            details_w += details.len() as f32 * detail_pad * 2.0
                + (details.len() - 1) as f32 * detail_hor_margin;
            details_h =
                details_relative_y_interval[1] - details_relative_y_interval[0] + detail_pad * 2.0;
        }

        let h = if details.is_empty() {
            header_h
        } else {
            header_h + vert_margin + details_h
        };

        x -= header_w / 2.0;

        let mut x0 = x;
        let mut y0 = y - h;
        draw_rectangle(x0, y0, header_w, header_h, Color::new(0.0, 0.0, 0.0, 0.7));

        let dim = draw_text_ex(
            header,
            x0 + header_pad,
            y0 + header_pad + header_dimensions.offset_y,
            params.clone(),
        );
        y0 += dim.height + header_pad * 2.0 + vert_margin;

        x0 += (header_w - details_w) / 2.0;

        for (line, goodness) in details {
            let mut params = params.clone();
            params.font = Some(&self.simple_font);
            params.font_size = detail_font_size;

            let dim = measure_text(line, Some(&self.simple_font), params.font_size, 1.0);

            let bg_color = match goodness {
                Goodness::Good => Color::new(0.0, 0.4, 0.0, 1.0),
                Goodness::Neutral => BLACK,
                Goodness::Bad => Color::new(0.5, 0.0, 0.0, 1.0),
            };
            draw_rectangle(
                x0,
                y0,
                dim.width + 2.0 * detail_pad,
                dim.height + 2.0 * detail_pad,
                bg_color,
            );
            draw_rectangle_lines(
                x0,
                y0,
                dim.width + 2.0 * detail_pad,
                dim.height + 2.0 * detail_pad,
                1.0,
                BLACK,
            );

            params.color = BLACK;
            draw_text_ex(
                line,
                x0 + detail_pad,
                y0 + detail_pad + details_max_offset,
                params.clone(),
            );
            params.color = WHITE;
            draw_text_ex(
                line,
                x0 + detail_pad - 1.0,
                y0 + detail_pad + details_max_offset - 1.0,
                params,
            );

            x0 += dim.width + detail_pad * 2.0 + detail_hor_margin;
        }
    }
}

impl Drawable for TargetUi {
    fn draw(&self, x: f32, y: f32) {
        if self.target.is_some() {
            self.container.draw(x, y);
        }

        self.draw_action((screen_width() / 2.0, 60.0));
    }

    fn size(&self) -> (f32, f32) {
        self.container.size()
    }
}
