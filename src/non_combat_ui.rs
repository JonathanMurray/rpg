use std::{cell::RefCell, collections::HashMap, rc::Rc};

use macroquad::{
    color::Color,
    miniquad::window::screen_size,
    text::Font,
    texture::Texture2D,
};

use crate::{
    action_button::{
        draw_button_tooltip, ActionButton, ButtonAction, InternalUiEvent,
    },
    base_ui::{Align, Container, Drawable, Element, LayoutDirection, Style},
    character_sheet::build_spell_book,
    core::{BaseAction, Character, HandType},
    equipment_ui::{EquipmentDrag, EquipmentSection},
    game_ui::ResourceBars,
    stats_ui::build_character_stats_table,
    textures::{EquipmentIconId, IconId, PortraitId},
};

pub struct NonCombatUi {
    bottom_panel: Element,
    equipment_section: Rc<RefCell<EquipmentSection>>,
    equipment_drag: Option<EquipmentDrag>,
    character: Rc<Character>,
    equipment_changed: Rc<std::cell::Cell<bool>>,
    event_queue: Rc<RefCell<Vec<InternalUiEvent>>>,
    hovered_button: Option<(u32, (f32, f32))>,
    hoverable_buttons: Vec<Rc<ActionButton>>,
    font: Font,
}

impl NonCombatUi {
    pub fn new(
        character: Rc<Character>,
        font: &Font,
        equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
        icons: &HashMap<IconId, Texture2D>,
        portrait_textures: &HashMap<PortraitId, Texture2D>,
    ) -> Self {
        let (screen_w, screen_h) = screen_size();

        let event_queue = Rc::new(RefCell::new(vec![]));

        let resource_bars = ResourceBars::new(&character, font);

        let mut next_button_id = 0;

        let mut new_button = |btn_action, character: Option<Rc<Character>>, enabled: bool| {
            let btn = ActionButton::new(btn_action, &event_queue, next_button_id, icons, character);
            btn.enabled.set(enabled);
            next_button_id += 1;
            btn
        };

        let portrait = portrait_textures[&character.portrait].clone();

        let mut hoverable_buttons = vec![];
        let mut attack_button = None;
        let mut spell_buttons = vec![];
        let mut attack_enhancement_buttons = vec![];

        for action in character.known_actions() {
            let btn_action = ButtonAction::Action(action);
            let btn = Rc::new(new_button(btn_action, Some(character.clone()), false));
            hoverable_buttons.push(Rc::clone(&btn));
            match action {
                BaseAction::Attack { .. } => {
                    attack_button = Some(btn.clone());
                }
                BaseAction::CastSpell(spell) => {
                    let enhancement_buttons: Vec<Rc<ActionButton>> = spell
                        .possible_enhancements
                        .iter()
                        .filter_map(|maybe_enhancement| *maybe_enhancement)
                        .filter_map(|enhancement| {
                            if character.knows_spell_enhancement(enhancement) {
                                let enhancement_btn = Rc::new(new_button(
                                    ButtonAction::SpellEnhancement(enhancement),
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
                    spell_buttons.push((btn.clone(), enhancement_buttons));
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

        let spell_book = build_spell_book(
            font,
            attack_button,
            reaction_buttons,
            attack_enhancement_buttons,
            spell_buttons,
        );

        let stats_table = build_character_stats_table(font, &character);

        let equipment_section = Rc::new(RefCell::new(EquipmentSection::new(
            font,
            &character,
            equipment_icons.clone(),
        )));

        let equipment_changed = character.listen_to_changed_equipment();

        let bottom_panel = Element::Container(Container {
            layout_dir: LayoutDirection::Horizontal,
            style: Style {
                background_color: Some(Color::new(0.00, 0.3, 0.4, 1.00)),
                padding: 10.0,
                ..Default::default()
            },
            min_width: Some(screen_w),
            children: vec![
                Element::Container(Container {
                    layout_dir: LayoutDirection::Vertical,
                    children: vec![
                        Element::Texture(portrait, Some((64.0, 80.0))),
                        Element::Container(resource_bars.container),
                    ],
                    align: Align::Center,
                    margin: 5.0,
                    ..Default::default()
                }),
                Element::Container(spell_book),
                stats_table,
                Element::RcRefCell(equipment_section.clone()),
            ],
            margin: 20.0,
            ..Default::default()
        });

        Self {
            bottom_panel,
            equipment_section,
            equipment_drag: None,
            character,
            equipment_changed,
            event_queue,
            hovered_button: None,
            hoverable_buttons,
            font: font.clone(),
        }
    }

    pub fn draw_and_handle_input(&mut self) {
        let (_screen_w, screen_h) = screen_size();

        let pos = (0.0, screen_h - 270.0);
        self.bottom_panel.draw(pos.0, pos.1);
        self.bottom_panel.draw_tooltips(pos.0, pos.1);

        let outcome = self
            .equipment_section
            .borrow_mut()
            .handle_equipment_drag_and_consumption(self.equipment_drag, None);
        self.equipment_drag = outcome.equipment_drag;
        //requested_consumption = outcome.requested_consumption;
        if let Some(drag) = self.equipment_drag {
            dbg!(&outcome);

            if drag.to_idx.is_some() {
                let (from, to) = self.equipment_section.borrow().resolve_drag_to_slots(drag);
                self.character.swap_equipment_slots(from, to);
                self.equipment_drag = None;
            }
        }

        if self.equipment_changed.take() {
            println!("CHAR EQUIPMENT CHANGED. UPDATING CHARACTER SHEET...");
            self.equipment_section
                .borrow_mut()
                .repopulate_character_equipment();
        }

        // Note: we have to drain the UI events, to prevent memory leak
        for event in self.event_queue.borrow_mut().drain(..) {
            match event {
                InternalUiEvent::ButtonHovered(button_id, _button_action, btn_pos) => {
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

        if let Some((id, btn_pos)) = self.hovered_button {
            let btn = self
                .hoverable_buttons
                .iter()
                .find(|btn| btn.id == id)
                .unwrap();
            draw_button_tooltip(&self.font, btn_pos, &btn.tooltip());
        }
    }
}
