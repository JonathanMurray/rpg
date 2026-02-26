use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    vec,
};

use macroquad::{
    color::{Color, BLACK, DARKGRAY, GRAY, LIGHTGRAY, MAGENTA, RED, WHITE},
    math::Rect,
    shapes::{draw_rectangle, draw_rectangle_lines},
    text::{measure_text, Font, TextParams},
    texture::Texture2D,
    window::screen_width,
};

use crate::{
    action_button::{
        draw_button_tooltip, ActionButton, ButtonAction, ButtonHovered, InternalUiEvent,
    },
    base_ui::{
        table, Align, Container, Drawable, Element, LayoutDirection, Style, TableCell, TableStyle,
        TextLine,
    },
    conditions_ui::ConditionsList,
    core::{AbilityRollType, BaseAction, Character, CharacterId, Goodness, HandType},
    game_ui_components::{ActionPointsRow, ResourceBar},
    textures::{IconId, PortraitId, StatusId},
    util::COL_RED,
};

const BG_COLOR: Color = Color::new(0.4, 0.3, 0.2, 1.0);

pub struct TargetUi {
    target: Option<Rc<Character>>,
    big_font: Font,
    simple_font: Font,

    container: Container,

    //action: Option<(String, Vec<(&'static str, Goodness)>, bool)>,
    icons: HashMap<IconId, Texture2D>,
    button_events: Rc<RefCell<Vec<InternalUiEvent>>>,
    hovered_btn: RefCell<Option<(u32, (f32, f32))>>,
    buttons: HashMap<u32, Rc<RefCell<ActionButton>>>,
    status_textures: HashMap<StatusId, Texture2D>,
    portrait_textures: HashMap<PortraitId, Texture2D>,
    pub last_drawn_rectangle: Cell<Rect>,
}

impl TargetUi {
    pub fn new(
        big_font: Font,
        simple_font: Font,
        icons: HashMap<IconId, Texture2D>,
        status_textures: HashMap<StatusId, Texture2D>,
        portrait_textures: HashMap<PortraitId, Texture2D>,
    ) -> Self {
        Self {
            target: Default::default(),
            big_font,
            simple_font,
            container: Container::default(),
            //action: None,
            icons,
            button_events: Default::default(),
            hovered_btn: Default::default(),
            buttons: Default::default(),
            status_textures,
            portrait_textures,
            last_drawn_rectangle: Default::default(),
        }
    }

    pub fn hovered_action(&self) -> Option<ButtonAction> {
        let hovered_btn = *self.hovered_btn.borrow();
        hovered_btn.map(|(btn_id, _)| {
            let btn = self.buttons.get(&btn_id).unwrap();
            btn.borrow().action
        })
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

    pub fn get_character_id(&self) -> Option<CharacterId> {
        self.target.as_ref().map(|ch| ch.id())
    }

    pub fn set_character(&mut self, character: Option<&Rc<Character>>) {
        if let Some(char) = character {
            self.target = Some(Rc::clone(char));
            let mut name_text_line =
                TextLine::new(char.name, 16, WHITE, Some(self.big_font.clone()));
            name_text_line.set_depth(BLACK, 2.0);
            name_text_line.set_min_height(20.0);

            let armor_text_line = TextLine::new(
                format!("Armor: {}", char.protection_from_armor()),
                22,
                WHITE,
                Some(self.simple_font.clone()),
            );

            let def_table = table(
                vec![
                    TableCell::new("Toughness", Some(LIGHTGRAY), None),
                    TableCell::new("Evasion", Some(LIGHTGRAY), None),
                    TableCell::new("Will", Some(LIGHTGRAY), None),
                    TableCell::new(char.toughness().to_string(), Some(WHITE), Some(BLACK)),
                    TableCell::new(char.evasion().to_string(), Some(WHITE), Some(BLACK)),
                    TableCell::new(char.will().to_string(), Some(WHITE), Some(BLACK)),
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
                (16.0, 16.0),
                0.25,
                Style {
                    background_color: Some(BLACK),
                    border_color: Some(WHITE),
                    ..Default::default()
                },
            );
            action_points_row.current_ap = char.action_points.current();
            let mut health_bar = ResourceBar::horizontal(char.health.max(), COL_RED, (80.0, 10.0));
            health_bar.current = char.health.current();

            let health_text_line = TextLine::new(
                format!("{} / {}", char.health.current(), char.health.max()),
                18,
                WHITE,
                Some(self.simple_font.clone()),
            );
            //health_text_line.set_depth(BLACK, 2.0);
            //health_text_line.set_min_height(20.0);

            let portrait = Element::Container(Container {
                style: Style {
                    background_color: Some(Color::new(0.5, 0.5, 0.5, 0.3)),
                    border_color: Some(LIGHTGRAY),
                    ..Default::default()
                },
                children: vec![Element::Texture(
                    self.portrait_textures[&char.portrait].clone(),
                    None,
                )],
                ..Default::default()
            });

            let defense_header: Element = Element::Text(TextLine::new(
                "|<shield>| Defenses",
                16,
                WHITE,
                Some(self.simple_font.clone()),
            ));

            let defense_header_row = Element::Container(Container {
                layout_dir: LayoutDirection::Vertical,
                align: Align::Center,
                style: Style {
                    background_color: Some(Color::new(1.0, 1.0, 1.0, 0.1)),
                    padding: 2.0,
                    ..Default::default()
                },
                min_width: Some(150.0),
                children: vec![defense_header],
                ..Default::default()
            });

            let centered_list = Container {
                layout_dir: LayoutDirection::Vertical,
                align: Align::Center,
                children: vec![
                    Element::Text(name_text_line),
                    portrait,
                    Element::Box(Box::new(health_bar)),
                    Element::Text(health_text_line),
                    Element::Box(Box::new(action_points_row)),
                    Element::Empty(1.0, 4.0),
                    defense_header_row,
                    Element::Container(def_table),
                    Element::Empty(1.0, 4.0),
                    Element::Text(armor_text_line),
                    Element::Empty(1.0, 4.0),
                ],
                margin: 3.0,

                ..Default::default()
            };

            let mut actions_row = None;
            let mut passives_row = None;
            let mut bot_using_spells = false;

            self.buttons.clear();
            if !char.player_controlled() {
                let mut next_btn_id = 0;
                let mut new_btn = |action| {
                    let mut btn = ActionButton::new(
                        action,
                        Some(Rc::clone(&self.button_events)),
                        next_btn_id,
                        &self.icons,
                        Some(Rc::clone(char)),
                        &self.simple_font,
                    );
                    next_btn_id += 1;

                    btn.set_parent_bg_color(BG_COLOR);
                    btn
                };

                let mut children: Vec<Element> = vec![];
                let mut passive_children: Vec<Element> = vec![];

                let btn = new_btn(ButtonAction::Action(BaseAction::Move));
                let id = btn.id;
                let btn = Rc::new(RefCell::new(btn));
                children.push(Element::RcRefCell(btn.clone()));
                self.buttons.insert(id, btn);

                if let Some(attack) = char.attack_action() {
                    let btn = new_btn(ButtonAction::Action(BaseAction::Attack(attack)));
                    let id = btn.id;
                    let btn = Rc::new(RefCell::new(btn));
                    children.push(Element::RcRefCell(btn.clone()));
                    self.buttons.insert(id, btn);
                }
                for ability in char.known_abilities() {
                    if matches!(ability.roll, Some(AbilityRollType::Spell)) {
                        bot_using_spells = true;
                    }
                    let btn = new_btn(ButtonAction::Action(BaseAction::UseAbility(ability)));
                    let id = btn.id;
                    let btn = Rc::new(RefCell::new(btn));
                    children.push(Element::RcRefCell(btn.clone()));
                    self.buttons.insert(id, btn);
                }
                for skill in &char.known_passive_skills {
                    let btn = new_btn(ButtonAction::Passive(*skill));
                    let id = btn.id;
                    let btn = Rc::new(RefCell::new(btn));
                    passive_children.push(Element::RcRefCell(btn.clone()));
                    self.buttons.insert(id, btn);
                }

                actions_row = Some(Element::Container(Container {
                    layout_dir: LayoutDirection::Horizontal,
                    children,
                    ..Default::default()
                }));
                if !passive_children.is_empty() {
                    passives_row = Some(Element::Container(Container {
                        layout_dir: LayoutDirection::Horizontal,
                        children: passive_children,
                        ..Default::default()
                    }));
                }
            }

            let mut rows = vec![Element::Container(centered_list)];

            if !char.player_controlled() {
                let movement_text_line = TextLine::new(
                    format!("|<boot>| Move: {:.1}", char.move_speed()),
                    16,
                    LIGHTGRAY,
                    Some(self.simple_font.clone()),
                );
                let attack_text_line = TextLine::new(
                    format!(
                        "|<dice>| Attack: +{}",
                        char.attack_modifier(HandType::MainHand)
                    ),
                    16,
                    LIGHTGRAY,
                    Some(self.simple_font.clone()),
                );
                let damage_text_line = TextLine::new(
                    format!(
                        "|<sword>| Damage: {}",
                        char.weapon(HandType::MainHand).unwrap().damage
                    ),
                    16,
                    LIGHTGRAY,
                    Some(self.simple_font.clone()),
                );
                let mut detailed_stats_lines = vec![
                    Element::Text(movement_text_line),
                    Element::Text(damage_text_line),
                    Element::Text(attack_text_line),
                ];
                if bot_using_spells {
                    detailed_stats_lines.push(Element::Text(TextLine::new(
                        format!("|<dice>| Spell: +{}", char.spell_modifier()),
                        16,
                        LIGHTGRAY,
                        Some(self.simple_font.clone()),
                    )));
                }

                let detailed_stats = Container {
                    layout_dir: LayoutDirection::Vertical,
                    children: detailed_stats_lines,
                    margin: 9.0,
                    style: Style {
                        padding: 5.0,
                        ..Default::default()
                    },
                    ..Default::default()
                };

                rows.push(Element::Container(detailed_stats));
            }

            if let Some(row) = actions_row {
                rows.push(row);
            }
            if let Some(row) = passives_row {
                rows.push(row);
            }

            if !char.condition_infos().is_empty() {
                let conditions_list = ConditionsList::new(
                    self.simple_font.clone(),
                    char.condition_infos(),
                    self.status_textures.clone(),
                );

                rows.push(Element::Box(Box::new(conditions_list)));
            }

            self.container = Container {
                layout_dir: LayoutDirection::Vertical,
                align: Align::Start,
                children: rows,
                margin: 15.0,
                style: Style {
                    background_color: Some(BG_COLOR),
                    border_color: Some(LIGHTGRAY),
                    border_inner_rounding: Some(6.0),
                    padding: 10.0,
                    ..Default::default()
                },
                border_between_children: Some(GRAY),

                ..Default::default()
            }
        } else {
            self.target = None;
            *self.hovered_btn.borrow_mut() = None;
        }
    }
}

impl TargetUi {
    pub fn draw(&self, x: f32, y: f32) {
        if self.target.is_some() {
            let (w, h) = self.container.draw(x, y);
            self.container.draw_tooltips(x, y);

            self.last_drawn_rectangle.set(Rect { x, y, w, h });
        } else {
            self.last_drawn_rectangle.set(Rect {
                x,
                y,
                w: 0.0,
                h: 0.0,
            });
        }

        for event in self.button_events.borrow_mut().drain(..) {
            match event {
                InternalUiEvent::ButtonHovered(ButtonHovered {
                    id,
                    action,
                    hovered_pos,
                    ..
                }) => {
                    if let Some(pos) = hovered_pos {
                        *self.hovered_btn.borrow_mut() = Some((id, pos));
                    } else {
                        let mut was_hovered = false;
                        if let Some(existing) = self.hovered_btn.borrow_mut().as_ref() {
                            if existing.0 == id {
                                was_hovered = true;
                            }
                        }

                        if was_hovered {
                            *self.hovered_btn.borrow_mut() = None;
                        }
                    }
                }
                InternalUiEvent::ButtonClicked { .. } => {}
                InternalUiEvent::ButtonInvalidClicked { .. } => {}
            }
        }

        if let Some((id, pos)) = self.hovered_btn.borrow().as_ref() {
            let btn = self.buttons.get(id).unwrap();

            draw_button_tooltip(&self.simple_font, *pos, &btn.borrow().tooltip(), true);
        }
    }

    pub fn size(&self) -> (f32, f32) {
        self.container.size()
    }
}
