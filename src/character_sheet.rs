use std::cell::{Cell, RefCell};
use std::{collections::HashMap, rc::Rc};

use macroquad::color::{DARKBLUE, DARKGRAY, SKYBLUE};

use macroquad::input::{
    is_mouse_button_down, is_mouse_button_pressed, mouse_position, MouseButton,
};
use macroquad::shapes::{draw_rectangle, draw_rectangle_lines};
use macroquad::text::{draw_text_ex, measure_text, TextParams};
use macroquad::window::{screen_height, screen_width};
use macroquad::{
    color::{Color, BLACK, LIGHTGRAY, WHITE},
    text::Font,
    texture::Texture2D,
};

use crate::base_ui::Drawable;
use crate::core::EquipmentSlotRole;
use crate::drawing::draw_cross;
use crate::equipment_ui::{EquipmentDrag, EquipmentSection};
use crate::game_ui::{ConfiguredAction, UiState};
use crate::stats_ui::build_character_stats_table;
use crate::{
    action_button::ActionButton,
    base_ui::{Align, Container, ContainerScroll, Element, LayoutDirection, Style, TextLine},
    core::Character,
    textures::EquipmentIconId,
};

pub struct CharacterSheet {
    pub screen_position: Cell<(f32, f32)>,
    sheet_dragged_offset: Cell<Option<(f32, f32)>>,

    equipment_changed: Rc<Cell<bool>>,
    equipment_section: Rc<RefCell<EquipmentSection>>,

    container: Container,
    top_bar_h: f32,
}

impl CharacterSheet {
    pub fn new(
        font: &Font,
        character: Rc<Character>,
        equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
        attack_button: Option<Rc<ActionButton>>,
        reaction_buttons: Vec<Rc<ActionButton>>,
        attack_enhancement_buttons: Vec<Rc<ActionButton>>,
        spell_buttons: Vec<(Rc<ActionButton>, Vec<Rc<ActionButton>>)>,
        passive_skill_buttons: Vec<Rc<ActionButton>>,
    ) -> Self {
        let spell_book_rows = build_spell_book(
            font,
            attack_button,
            reaction_buttons,
            attack_enhancement_buttons,
            spell_buttons,
            passive_skill_buttons,
            450.0,
        );

        let stats_table = build_character_stats_table(font, Rc::clone(&character));

        let equipment_section = Rc::new(RefCell::new(EquipmentSection::new(
            font,
            &character,
            equipment_icons.clone(),
        )));

        let contents = Element::Container(Container {
            layout_dir: LayoutDirection::Horizontal,
            margin: 5.0,
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
                    children: vec![
                        Element::Text(
                            TextLine::new("Spell book", 22, WHITE, Some(font.clone()))
                                .with_depth(BLACK, 2.0),
                        ),
                        Element::Container(spell_book_rows),
                    ],
                    ..Default::default()
                }),
                Element::Box(Box::new(stats_table)),
                Element::RcRefCell(equipment_section.clone()),
            ],
            ..Default::default()
        });

        let container = Container {
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
                        .with_padding(10.0, 10.0),
                ),
                contents,
            ],
            ..Default::default()
        };

        let top_bar_h = container.children[0].size().1;

        let equipment_changed = character.listen_to_changed_equipment();

        Self {
            container,
            top_bar_h,
            screen_position: Cell::new((100.0, 100.0)),
            sheet_dragged_offset: Cell::new(None),
            equipment_changed,

            equipment_section,
        }
    }

    pub fn draw(&mut self, ui_state: &mut UiState) -> CharacterSheetOutcome {
        if self.equipment_changed.take() {
            println!("CHAR EQUIPMENT CHANGED. UPDATING CHARACTER SHEET...");
            self.equipment_section
                .borrow_mut()
                .repopulate_character_equipment();
        }

        let (x, y) = self.screen_position.get();
        let (w, h) = self.container.draw(x, y);
        let clicked_close = self.draw_close_button(x, y);

        let (mouse_x, mouse_y) = mouse_position();

        self.container.draw_tooltips(x, y);

        if let Some((x_offset, y_offset)) = self.sheet_dragged_offset.get() {
            if is_mouse_button_down(MouseButton::Left) {
                let new_x = (mouse_x - x_offset).max(0.0).min(screen_width() - w);
                let new_y = (mouse_y - y_offset).max(0.0).min(screen_height() - h);
                self.screen_position.set((new_x, new_y));
            } else {
                self.sheet_dragged_offset.set(None);
            }
        }
        if is_mouse_button_pressed(MouseButton::Left)
            && (x..x + w).contains(&mouse_x)
            && (y..y + self.top_bar_h).contains(&mouse_y)
        {
            self.sheet_dragged_offset
                .set(Some((mouse_x - x, mouse_y - y)));
        }

        if clicked_close {
            self.sheet_dragged_offset.set(None);
        }

        let is_allowed_to_change_equipment = matches!(
            ui_state,
            UiState::ConfiguringAction(..) | UiState::ChoosingAction
        );
        let is_allowed_to_use_consumable = matches!(
            ui_state,
            UiState::ConfiguringAction(..) | UiState::ChoosingAction
        );

        let mut equipment_drag = None;
        let mut requested_consumption = None;
        match ui_state {
            UiState::ConfiguringAction(ConfiguredAction::ChangeEquipment { drag }) => {
                equipment_drag = *drag
            }
            UiState::ConfiguringAction(ConfiguredAction::UseConsumable(consumption)) => {
                requested_consumption = *consumption
            }
            _ => {}
        };

        let outcome = self
            .equipment_section
            .borrow_mut()
            .handle_equipment_drag_and_consumption(equipment_drag, requested_consumption);

        if outcome.equipment_drag != equipment_drag && is_allowed_to_change_equipment {
            // TODO: currently it's not possible to move around items even in the inventory on another character's
            // turn. Ideally that should be possible.
            // TODO: ^ and it should also be possible from the chest/shop/victory scenes.
            *ui_state = UiState::ConfiguringAction(ConfiguredAction::ChangeEquipment {
                drag: outcome.equipment_drag,
            });
        } else if requested_consumption.is_none()
            && outcome.requested_consumption.is_some()
            && is_allowed_to_use_consumable
        {
            *ui_state = UiState::ConfiguringAction(ConfiguredAction::UseConsumable(
                outcome.requested_consumption,
            ));
        }

        CharacterSheetOutcome {
            clicked_close,
            changed_state: outcome.changed,
        }
    }

    pub fn container_size(&self) -> (f32, f32) {
        self.container.size()
    }

    fn draw_close_button(&self, x: f32, y: f32) -> bool {
        let container_size = self.container.size();

        let button_size = (20.0, 20.0);
        let button_margin = 5.0;

        let btn_x = x + container_size.0 - button_margin - button_size.0;
        let btn_y = y + button_margin;
        let btn_w = button_size.0;
        let btn_h = button_size.1;

        let (mouse_x, mouse_y) = mouse_position();

        let hover =
            (btn_x..btn_x + btn_w).contains(&mouse_x) && (btn_y..btn_y + btn_h).contains(&mouse_y);

        let bg_color = DARKGRAY;
        draw_rectangle(btn_x, btn_y, btn_w, btn_h, bg_color);

        let cross_color = LIGHTGRAY;
        draw_cross(btn_x, btn_y, btn_w, btn_h, cross_color, 1.0, 2.0);
        if hover {
            draw_rectangle_lines(btn_x, btn_y, btn_w, btn_h, 2.0, WHITE);
        } else {
            draw_rectangle_lines(btn_x, btn_y, btn_w, btn_h, 1.0, cross_color);
        }

        hover && is_mouse_button_pressed(MouseButton::Left)
    }

    pub fn resolve_drag_to_slots(
        &self,
        drag: EquipmentDrag,
    ) -> (EquipmentSlotRole, EquipmentSlotRole) {
        self.equipment_section.borrow().resolve_drag_to_slots(drag)
    }

    pub fn describe_requested_equipment_change(&self, drag: EquipmentDrag) -> String {
        self.equipment_section
            .borrow()
            .describe_requested_equipment_change(drag)
    }
}

pub fn build_spell_book(
    font: &Font,
    attack_button: Option<Rc<ActionButton>>,
    reaction_buttons: Vec<Rc<ActionButton>>,
    attack_enhancement_buttons: Vec<Rc<ActionButton>>,
    spell_buttons: Vec<(Rc<ActionButton>, Vec<Rc<ActionButton>>)>,
    passive_skill_buttons: Vec<Rc<ActionButton>>,
    max_height: f32,
) -> Container {
    let mut spell_book_rows = Container {
        layout_dir: LayoutDirection::Vertical,
        margin: 5.0,
        children: vec![],
        scroll: Some(ContainerScroll::new(40.0)),
        max_height: Some(max_height),
        style: Style {
            padding: 10.0,
            border_color: Some(LIGHTGRAY),
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

    for (spell_btn, enhancement_buttons) in spell_buttons.into_iter() {
        let spell = spell_btn.action.unwrap_spell();
        spell_book_rows.children.push(Element::Text(TextLine::new(
            spell.name,
            16,
            WHITE,
            Some(font.clone()),
        )));

        let mut row_buttons = vec![spell_btn.clone()];
        for enhancement_btn in enhancement_buttons {
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

    if !passive_skill_buttons.is_empty() {
        let row = buttons_row(
            passive_skill_buttons
                .into_iter()
                .map(|btn| Element::Rc(btn))
                .collect(),
        );

        spell_book_rows.children.push(Element::Text(TextLine::new(
            "Passive",
            16,
            WHITE,
            Some(font.clone()),
        )));
        spell_book_rows.children.push(row);
    }

    spell_book_rows
}

pub struct MoneyText {
    pub character: Rc<Character>,
    pub font: Font,
}

impl MoneyText {
    fn text(&self) -> String {
        format!("gold: {}", self.character.money.get())
    }
}

const MONEY_FONT_SIZE: u16 = 18;

impl Drawable for MoneyText {
    fn draw(&self, x: f32, y: f32) {
        let text_dim = measure_text(&self.text(), Some(&self.font), MONEY_FONT_SIZE, 1.0);
        draw_text_ex(
            &self.text(),
            x,
            y + text_dim.offset_y,
            TextParams {
                font: Some(&self.font),
                font_size: MONEY_FONT_SIZE,
                color: WHITE,
                ..Default::default()
            },
        );
    }

    fn size(&self) -> (f32, f32) {
        let text_dim = measure_text(&self.text(), Some(&self.font), MONEY_FONT_SIZE, 1.0);
        (text_dim.width, text_dim.height)
    }
}

pub struct CharacterSheetOutcome {
    pub clicked_close: bool,
    pub changed_state: bool,
}

fn buttons_row(buttons: Vec<Element>) -> Element {
    Element::Container(Container {
        layout_dir: LayoutDirection::Horizontal,
        margin: 5.0,
        children: buttons,
        ..Default::default()
    })
}

fn has_drag(ui_state: &UiState) -> bool {
    matches!(
        ui_state,
        UiState::ConfiguringAction(ConfiguredAction::ChangeEquipment { drag: Some(_) })
    )
}
