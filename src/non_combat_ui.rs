use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use macroquad::{
    color::{Color, GRAY, WHITE, YELLOW},
    input::{is_mouse_button_pressed, mouse_position, MouseButton},
    math::Rect,
    miniquad::window::screen_size,
    shapes::draw_rectangle_lines,
    text::Font,
    texture::{draw_texture_ex, DrawTextureParams, Texture2D},
};

use crate::{
    action_button::{
        draw_button_tooltip, ActionButton, ButtonAction, ButtonHovered, InternalUiEvent,
    },
    base_ui::{Align, Container, Drawable, Element, LayoutDirection, Style},
    character_sheet::{build_spell_book, MoneyText},
    core::{BaseAction, Character, HandType},
    equipment_ui::{EquipmentDrag, EquipmentSection},
    game_ui::ResourceBars,
    stats_ui::build_character_stats_table,
    textures::{EquipmentIconId, IconId, PortraitId},
};

pub struct PortraitRow {
    pub selected_idx: usize,
    portraits: Vec<Texture2D>,
}

impl PortraitRow {
    pub fn new(
        characters: &[impl AsRef<Character>],
        portrait_textures: &HashMap<PortraitId, Texture2D>,
    ) -> Self {
        let selected_idx = 0;
        let portraits = characters
            .iter()
            .map(|char| portrait_textures[&char.as_ref().portrait].clone())
            .collect();
        Self {
            selected_idx,
            portraits,
        }
    }

    pub fn draw_and_handle_input(&mut self) {
        let (screen_w, screen_h) = screen_size();

        for (i, portrait) in self.portraits.iter().enumerate() {
            let border_color = if i == self.selected_idx { YELLOW } else { GRAY };

            let w = 64.0;
            let h = 80.0;
            let x = 10.0 + w * i as f32 + 10.0;
            let y = screen_h - 400.0;
            let rect = Rect::new(x, y, w, h);
            draw_texture_ex(
                portrait,
                x,
                y,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(rect.size()),
                    ..Default::default()
                },
            );
            draw_rectangle_lines(x, y, w, h, 3.0, border_color);

            if rect.contains(mouse_position().into()) && is_mouse_button_pressed(MouseButton::Left)
            {
                self.selected_idx = i;
            }
        }
    }
}

const UI_HEIGHT: f32 = 290.0;

pub struct NonCombatPartyUi {
    portrait_row: PortraitRow,
    bottom_panels: Vec<NonCombatCharacterUi>,
    equipment_changed: Vec<Rc<Cell<bool>>>,
}

impl NonCombatPartyUi {
    pub fn new(
        characters: &[Rc<Character>],
        font: Font,
        equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
        icons: HashMap<IconId, Texture2D>,
        portrait_textures: &HashMap<PortraitId, Texture2D>,
    ) -> Self {
        let portrait_row = PortraitRow::new(characters, portrait_textures);
        let bottom_panels: Vec<NonCombatCharacterUi> = characters
            .iter()
            .map(|character| {
                NonCombatCharacterUi::new(
                    character.clone(),
                    &font,
                    equipment_icons,
                    &icons,
                    portrait_textures,
                )
            })
            .collect();

        let equipment_changed = characters
            .iter()
            .map(|ch| ch.listen_to_changed_equipment())
            .collect();

        Self {
            portrait_row,
            bottom_panels,
            equipment_changed,
        }
    }

    pub fn draw_and_handle_input(&mut self) {
        let mut equipment_changed = false;
        for event in &self.equipment_changed {
            if event.take() {
                equipment_changed = true;
            }
        }

        if equipment_changed {
            println!("CHAR EQUIPMENT CHANGED. UPDATING CHARACTER UIs...");
            for panel in &mut self.bottom_panels {
                panel.on_equipment_changed();
            }
        }

        self.portrait_row.draw_and_handle_input();
        self.bottom_panels[self.portrait_row.selected_idx].draw_and_handle_input();
        self.bottom_panels[self.portrait_row.selected_idx].draw_tooltips();
    }

    pub fn selected_character_idx(&self) -> usize {
        self.portrait_row.selected_idx
    }
}

pub struct NonCombatCharacterUi {
    bottom_panel: Element,
    stats_table: Rc<RefCell<crate::stats_ui::CharacterStatsTable>>,
    resource_bars: Rc<RefCell<ResourceBars>>,
    equipment_section: Rc<RefCell<EquipmentSection>>,
    equipment_drag: Option<EquipmentDrag>,
    character: Rc<Character>,

    event_queue: Rc<RefCell<Vec<InternalUiEvent>>>,
    hovered_button: Option<(u32, (f32, f32))>,
    hoverable_buttons: Vec<Rc<ActionButton>>,
    font: Font,
}

impl NonCombatCharacterUi {
    pub fn new(
        character: Rc<Character>,
        font: &Font,
        equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
        icons: &HashMap<IconId, Texture2D>,
        portrait_textures: &HashMap<PortraitId, Texture2D>,
    ) -> Self {
        let (screen_w, _screen_h) = screen_size();

        let event_queue = Rc::new(RefCell::new(vec![]));

        let resource_bars = Rc::new(RefCell::new(ResourceBars::new(&character, font)));

        let mut next_button_id = 0;

        let mut new_button = |btn_action, character: Option<Rc<Character>>, enabled: bool| {
            let btn = ActionButton::new(
                btn_action,
                Some(Rc::clone(&event_queue)),
                next_button_id,
                icons,
                character,
            );
            btn.enabled.set(enabled);
            next_button_id += 1;
            btn
        };

        let portrait = portrait_textures[&character.portrait].clone();

        let mut hoverable_buttons = vec![];
        let mut attack_button = None;
        let mut ability_buttons = vec![];
        let mut attack_enhancement_buttons = vec![];
        let mut passive_skill_buttons = vec![];

        for action in character.known_actions() {
            let btn_action = ButtonAction::Action(action);
            let btn = Rc::new(new_button(btn_action, Some(character.clone()), false));
            hoverable_buttons.push(Rc::clone(&btn));
            match action {
                BaseAction::Attack { .. } => {
                    attack_button = Some(btn.clone());
                }
                BaseAction::UseAbility(ability) => {
                    let enhancement_buttons: Vec<Rc<ActionButton>> = ability
                        .possible_enhancements
                        .iter()
                        .filter_map(|maybe_enhancement| *maybe_enhancement)
                        .filter_map(|enhancement| {
                            if character.knows_ability_enhancement(enhancement) {
                                let enhancement_btn = Rc::new(new_button(
                                    ButtonAction::AbilityEnhancement(enhancement),
                                    None,
                                    false,
                                ));
                                hoverable_buttons.push(Rc::clone(&enhancement_btn));
                                Some(enhancement_btn)
                            } else {
                                None
                            }
                        })
                        .collect();
                    ability_buttons.push((btn.clone(), enhancement_buttons));
                }
                _ => {}
            }
        }

        let mut reaction_buttons = vec![];
        for (_subtext, reaction) in character.known_on_attacked_reactions() {
            let btn_action = ButtonAction::OnAttackedReaction(reaction);
            let btn = Rc::new(new_button(btn_action, None, false));
            hoverable_buttons.push(Rc::clone(&btn));
            reaction_buttons.push(btn);
        }
        for (_subtext, reaction) in character.known_on_hit_reactions() {
            let btn_action = ButtonAction::OnHitReaction(reaction);
            let btn = Rc::new(new_button(btn_action, None, false));
            hoverable_buttons.push(Rc::clone(&btn));
            reaction_buttons.push(btn);
        }

        // TODO: Only include inherently known enhancements here; not those gained from weapons (since weapons can be unequipped
        // without the character sheet being updated)
        for (_subtext, enhancement) in character.known_attack_enhancements(HandType::MainHand) {
            let btn_action = ButtonAction::AttackEnhancement(enhancement);
            let btn = Rc::new(new_button(btn_action, None, false));
            hoverable_buttons.push(Rc::clone(&btn));
            attack_enhancement_buttons.push(btn);
        }
        for passive_skill in &character.known_passive_skills {
            let btn_action = ButtonAction::Passive(*passive_skill);
            let btn = Rc::new(new_button(btn_action, Some(character.clone()), false));
            hoverable_buttons.push(Rc::clone(&btn));
            passive_skill_buttons.push(Rc::clone(&btn));
        }

        let spell_book = build_spell_book(
            font,
            attack_button,
            reaction_buttons,
            attack_enhancement_buttons,
            ability_buttons,
            passive_skill_buttons,
            UI_HEIGHT - 20.0,
        );

        let stats_table = Rc::new(RefCell::new(build_character_stats_table(
            font,
            Rc::clone(&character),
        )));

        let equipment_section = Rc::new(RefCell::new(EquipmentSection::new(
            font,
            &character,
            equipment_icons.clone(),
            true,
        )));

        let bottom_panel = Element::Container(Container {
            layout_dir: LayoutDirection::Horizontal,
            style: Style {
                background_color: Some(Color::new(0.00, 0.3, 0.4, 1.00)),
                padding: 10.0,
                ..Default::default()
            },
            min_width: Some(screen_w),
            min_height: Some(UI_HEIGHT),
            children: vec![
                Element::Container(Container {
                    layout_dir: LayoutDirection::Vertical,
                    children: vec![
                        Element::Texture(portrait, Some((64.0, 80.0))),
                        Element::RcRefCell(resource_bars.clone()),
                    ],
                    align: Align::Center,
                    margin: 5.0,
                    ..Default::default()
                }),
                Element::Container(spell_book),
                Element::RcRefCell(stats_table.clone()),
                Element::RcRefCell(equipment_section.clone()),
            ],
            margin: 20.0,
            ..Default::default()
        });

        Self {
            bottom_panel,
            stats_table,
            resource_bars,
            equipment_section,
            equipment_drag: None,
            character,
            event_queue,
            hovered_button: None,
            hoverable_buttons,
            font: font.clone(),
        }
    }

    pub fn on_character_stats_changed(&mut self) {
        self.stats_table.borrow_mut().rebuild();

        *self.resource_bars.borrow_mut() = ResourceBars::new(&self.character, &self.font);

        self.equipment_section
            .borrow_mut()
            .repopulate_character_equipment();
    }

    pub fn on_equipment_changed(&mut self) {
        self.equipment_section
            .borrow_mut()
            .repopulate_character_equipment();
    }

    pub fn draw_tooltips(&mut self) {
        let (_screen_w, screen_h) = screen_size();
        let pos = (0.0, screen_h - UI_HEIGHT);
        self.bottom_panel.draw_tooltips(pos.0, pos.1);

        if let Some((id, btn_pos)) = self.hovered_button {
            let btn = self
                .hoverable_buttons
                .iter()
                .find(|btn| btn.id == id)
                .unwrap();
            draw_button_tooltip(&self.font, btn_pos, &btn.tooltip());
        }
    }

    pub fn draw_and_handle_input(&mut self) {
        let (_screen_w, screen_h) = screen_size();

        let pos = (0.0, screen_h - UI_HEIGHT);
        self.bottom_panel.draw(pos.0, pos.1);

        let is_allowed_to_change_equipment = true;

        let outcome = self
            .equipment_section
            .borrow_mut()
            .handle_equipment_drag_and_consumption(
                self.equipment_drag,
                None,
                is_allowed_to_change_equipment,
            );
        self.equipment_drag = outcome.equipment_drag;
        //requested_consumption = outcome.requested_consumption;
        if let Some(drag) = self.equipment_drag {
            if drag.to_idx.is_some() {
                let (from, to) = self.equipment_section.borrow().resolve_drag_to_slots(drag);
                self.character.swap_equipment_slots(from, to);
                self.equipment_drag = None;
            }
        }

        // Note: we have to drain the UI events, to prevent memory leak
        for event in self.event_queue.borrow_mut().drain(..) {
            match event {
                InternalUiEvent::ButtonHovered(ButtonHovered {
                    id: button_id,
                    hovered_pos: btn_pos,
                    ..
                }) => {
                    if let Some(btn_pos) = btn_pos {
                        self.hovered_button = Some((button_id, btn_pos));
                    } else if let Some(previously_hovered_button) = self.hovered_button {
                        if button_id == previously_hovered_button.0 {
                            self.hovered_button = None
                        }
                    }
                }
                _ => {
                    dbg!(event);
                }
            }
        }
    }
}
