use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use macroquad::{
    color::{BLACK, BLUE, DARKGRAY, GREEN, MAGENTA, ORANGE, RED, WHITE},
    input::mouse_position,
    shapes::{draw_line, draw_rectangle},
    text::Font,
    texture::Texture2D,
    window::{screen_height, screen_width},
};

use crate::{
    action_button::{
        draw_button_tooltip, ActionButton, ButtonAction, ButtonSelected, InternalUiEvent,
    },
    activity_popup::{ActivityPopup, ActivityPopupOutcome},
    base_ui::{Align, Container, Drawable, Element, LayoutDirection, Style, TextLine},
    character_sheet::CharacterSheet,
    conditions_ui::ConditionsList,
    core::{
        as_percentage, distance_between, prob_attack_hit, prob_spell_hit, Action, ActionReach,
        ActionTarget, AttackAction, AttackEnhancement, AttackOutcome, BaseAction, Character,
        CharacterId, Characters, CoreGame, GameEvent, Goodness, HandType, OnAttackedReaction,
        OnHitReaction, Position, Spell, SpellEnhancement, SpellModifier, SpellTarget,
        SpellTargetOutcome,
    },
    equipment_ui::{EquipmentConsumption, EquipmentDrag},
    game_ui_components::{
        ActionPointsRow, CharacterPortraits, CharacterSheetToggle, LabelledResourceBar, Log,
        PlayerPortraits,
    },
    grid::{
        Effect, EffectGraphics, EffectPosition, EffectVariant, GameGrid, GridOutcome, NewState,
    },
    init_fight_map::GameInitState,
    target_ui::TargetUi,
    textures::{EquipmentIconId, IconId, PortraitId, SpriteId},
};

#[derive(Debug, Clone, PartialEq)]
pub enum UiState {
    ChoosingAction,
    ConfiguringAction(ConfiguredAction),
    ReactingToAttack {
        hand: HandType,
        attacker: CharacterId,
        reactor: CharacterId,
        is_within_melee: bool,
        selected: Option<OnAttackedReaction>,
    },
    ReactingToHit {
        attacker: CharacterId,
        victim: CharacterId,
        damage: u32,
        is_within_melee: bool,
        selected: Option<OnHitReaction>,
    },
    ReactingToOpportunity {
        reactor: CharacterId,
        target: u32,
        movement: ((i32, i32), (i32, i32)),
        selected: bool,
    },
    Idle,
}

impl UiState {
    pub fn has_required_player_input(
        &self,
        relevant_character: &Character,
        characters: &Characters,
    ) -> bool {
        match self {
            UiState::ConfiguringAction(configured_action) => {
                configured_action.has_required_player_input(relevant_character, characters)
            }
            _ => true,
        }
    }

    pub fn players_action_target(&self) -> ActionTarget {
        match self {
            UiState::ConfiguringAction(configured_action) => match configured_action {
                ConfiguredAction::Attack { target, .. } => target
                    .map(|id| ActionTarget::Character(id, None))
                    .unwrap_or(ActionTarget::None),
                ConfiguredAction::CastSpell { target, .. } => target.clone(),
                _ => ActionTarget::None,
            },
            _ => ActionTarget::None,
        }
    }

    pub fn set_target(&mut self, new_target: ActionTarget) {
        match self {
            UiState::ConfiguringAction(configured_action) => match configured_action {
                ConfiguredAction::Attack { target, .. } => {
                    *target = match new_target {
                        ActionTarget::Character(id, None) => Some(id),
                        ActionTarget::None => None,
                        _ => panic!(),
                    };
                }
                ConfiguredAction::CastSpell { target, .. } => *target = new_target,

                action => panic!("Action has no target: {:?}", action),
            },
            state => panic!("State has no target: {:?}", state),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfiguredAction {
    Attack {
        attack: AttackAction,
        selected_enhancements: Vec<AttackEnhancement>,
        target: Option<CharacterId>,
    },
    CastSpell {
        spell: Spell,
        selected_enhancements: Vec<SpellEnhancement>,
        target: ActionTarget,
    },
    Move {
        selected_movement_path: Vec<(f32, Position)>,
        ap_cost: u32,
    },
    ChangeEquipment {
        drag: Option<EquipmentDrag>,
    },
    UseConsumable(Option<EquipmentConsumption>),
    EndTurn,
}

impl ConfiguredAction {
    fn has_required_player_input(
        &self,
        relevant_character: &Character,
        characters: &Characters,
    ) -> bool {
        match self {
            ConfiguredAction::Attack { target, attack, .. } => match target {
                Some(target_id) => {
                    let target_char = characters.get(*target_id);
                    let (_range, reach) = relevant_character
                        .reaches_with_attack(attack.hand, target_char.position.get());
                    matches!(
                        reach,
                        ActionReach::Yes | ActionReach::YesButDisadvantage(..)
                    )
                }
                None => false,
            },

            ConfiguredAction::CastSpell {
                target,
                spell,
                selected_enhancements,
                ..
            } => match target {
                ActionTarget::Character(target_id, movement) => {
                    if let Some(positions) = movement {
                        if positions.is_empty() {
                            return false;
                        }
                    }
                    let target_char = characters.get(*target_id);

                    relevant_character.reaches_with_spell(
                        *spell,
                        selected_enhancements,
                        target_char.position.get(),
                    )
                }

                ActionTarget::Position(target_pos) => {
                    assert!(matches!(spell.target, SpellTarget::Area { .. }));
                    relevant_character.reaches_with_spell(
                        *spell,
                        selected_enhancements,
                        *target_pos,
                    )
                }

                ActionTarget::None => match spell.target {
                    SpellTarget::Enemy { .. } => false,
                    SpellTarget::Ally { .. } => false,
                    SpellTarget::None { .. } => true,
                    SpellTarget::Area { .. } => false,
                },
            },

            ConfiguredAction::Move {
                selected_movement_path,
                ..
            } => !selected_movement_path.is_empty(),

            ConfiguredAction::ChangeEquipment { drag } => matches!(
                drag,
                Some(EquipmentDrag {
                    to_idx: Some(_),
                    ..
                })
            ),

            ConfiguredAction::UseConsumable(consumable) => consumable.is_some(),

            ConfiguredAction::EndTurn => true,
        }
    }

    pub fn from_base_action(base_action: BaseAction) -> Self {
        match base_action {
            BaseAction::Attack(attack) => Self::Attack {
                attack,
                selected_enhancements: vec![],
                target: None,
            },
            BaseAction::CastSpell(spell) => Self::CastSpell {
                spell,
                selected_enhancements: vec![],
                target: ActionTarget::None,
            },
            BaseAction::Move => Self::Move {
                ap_cost: 0,
                selected_movement_path: Default::default(),
            },
            BaseAction::ChangeEquipment => Self::ChangeEquipment { drag: None },
            BaseAction::UseConsumable => Self::UseConsumable(None),
            BaseAction::EndTurn => Self::EndTurn,
        }
    }

    pub fn base_action(&self) -> BaseAction {
        match self {
            ConfiguredAction::Attack { attack, .. } => BaseAction::Attack(*attack),
            ConfiguredAction::CastSpell { spell, .. } => BaseAction::CastSpell(*spell),
            ConfiguredAction::Move { .. } => BaseAction::Move,
            ConfiguredAction::ChangeEquipment { .. } => BaseAction::ChangeEquipment,
            ConfiguredAction::UseConsumable { .. } => BaseAction::UseConsumable,
            ConfiguredAction::EndTurn => BaseAction::EndTurn,
        }
    }

    pub fn base_action_point_cost(&self) -> u32 {
        self.base_action().action_point_cost()
    }

    pub fn enhanced_action_point_cost(&self) -> u32 {
        match self {
            ConfiguredAction::Attack {
                attack,
                selected_enhancements,
                ..
            } => {
                let mut ap = attack.action_point_cost;
                for enhancement in selected_enhancements {
                    ap += enhancement.action_point_cost;
                    ap -= enhancement.effect.action_point_discount;
                }
                ap
            }
            ConfiguredAction::CastSpell {
                spell,
                selected_enhancements,
                ..
            } => {
                let mut ap = spell.action_point_cost;
                for enhancement in selected_enhancements {
                    ap += enhancement.action_point_cost;
                }
                ap
            }
            ConfiguredAction::Move { ap_cost, .. } => *ap_cost,
            ConfiguredAction::ChangeEquipment { .. } => 1,
            ConfiguredAction::UseConsumable { .. } => 1,
            ConfiguredAction::EndTurn => 0,
        }
    }

    pub fn mana_cost(&self) -> u32 {
        self.base_action().mana_cost()
    }

    pub fn stamina_cost(&self) -> u32 {
        self.base_action().stamina_cost()
    }
}

#[derive(Debug, Copy, Clone, Default)]
struct StopWatch {
    remaining: Option<f32>,
}

impl StopWatch {
    fn set_to_at_least(&mut self, value: f32) {
        if let Some(remaining) = self.remaining {
            self.remaining = Some(remaining.max(value));
        } else {
            self.remaining = Some(value);
        }
    }

    fn update(&mut self, elapsed: f32) -> bool {
        if let Some(remaining) = &mut self.remaining {
            *remaining = (*remaining - elapsed).max(0.0);
            if *remaining <= 0.0 {
                self.remaining = None;
                return true;
            }
        }
        false
    }
}

pub struct CharacterUi {
    tracked_action_buttons: HashMap<String, Rc<ActionButton>>,
    action_points_row: ActionPointsRow,
    pub hoverable_buttons: Vec<Rc<ActionButton>>,
    actions_section: Container,
    pub character_sheet: CharacterSheet,
    health_bar: Rc<RefCell<LabelledResourceBar>>,
    mana_bar: Rc<RefCell<LabelledResourceBar>>,
    stamina_bar: Rc<RefCell<LabelledResourceBar>>,
    pub resource_bars: Container,
    conditions_list: ConditionsList,
}

impl CharacterUi {
    pub fn draw(&self, y: f32) {
        self.actions_section.draw(10.0, y + 5.0);
        self.action_points_row.draw(430.0, y);
        self.resource_bars
            .draw(480.0 - self.resource_bars.size().0 / 2.0, y + 40.0);
    }
}

pub struct UserInterface {
    characters: Characters,
    event_queue: Rc<RefCell<Vec<InternalUiEvent>>>,
    state: Rc<RefCell<UiState>>,
    animation_stopwatch: StopWatch,

    font: Font,

    hovered_button: Option<(u32, ButtonAction, (f32, f32))>,
    active_character_id: CharacterId,

    pub game_grid: GameGrid,
    activity_popup: ActivityPopup,
    character_portraits: CharacterPortraits,
    player_portraits: PlayerPortraits,
    character_sheet_toggle: CharacterSheetToggle,
    character_uis: HashMap<CharacterId, CharacterUi>,
    target_ui: TargetUi,
    log: Log,
}

impl UserInterface {
    pub fn new(
        game: &CoreGame,
        sprites: HashMap<SpriteId, Texture2D>,
        icons: HashMap<IconId, Texture2D>,
        equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
        portrait_textures: HashMap<PortraitId, Texture2D>,
        terrain_atlas: Texture2D,
        simple_font: Font,
        decorative_font: Font,
        big_font: Font,
        init_state: GameInitState,
    ) -> Self {
        let characters = game.characters.clone();
        let active_character_id = game.active_character_id;

        let event_queue = Rc::new(RefCell::new(vec![]));

        let character_uis = build_character_uis(
            equipment_icons,
            &icons,
            &event_queue,
            &simple_font,
            characters.iter(),
        );

        let ui_state = Rc::new(RefCell::new(UiState::Idle));

        let first_player_character_id = characters
            .iter_with_ids()
            .find(|(_id, ch)| ch.player_controlled)
            .unwrap()
            .0;

        let terrain_objects = init_state.terrain_objects;
        let background = init_state.background;

        let game_grid = GameGrid::new(
            first_player_character_id,
            characters.clone(),
            sprites,
            big_font.clone(),
            simple_font.clone(),
            terrain_atlas,
            init_state.pathfind_grid.clone(),
            background,
            terrain_objects,
        );

        let player_portraits = PlayerPortraits::new(
            &characters,
            first_player_character_id,
            active_character_id,
            decorative_font.clone(),
            portrait_textures.clone(),
        );

        let character_sheet_toggle = CharacterSheetToggle {
            shown: Cell::new(false),
            text_line: TextLine::new("Character sheet", 18, WHITE, Some(simple_font.clone())),
            padding: 10.0,
        };

        let character_portraits = CharacterPortraits::new(
            &game.characters,
            game.active_character_id,
            simple_font.clone(),
            //decorative_font.clone(),
            portrait_textures,
        );

        let target_ui = TargetUi::new(big_font.clone(), simple_font.clone());

        let activity_popup = ActivityPopup::new(
            simple_font.clone(),
            ui_state.clone(),
            icons,
            characters.clone(),
            active_character_id,
        );

        Self {
            game_grid,
            characters,
            character_portraits,
            player_portraits,
            character_sheet_toggle,
            active_character_id,
            animation_stopwatch: StopWatch::default(),

            font: simple_font.clone(),

            hovered_button: None,
            log: Log::new(simple_font.clone()),
            character_uis,
            event_queue: Rc::clone(&event_queue),
            activity_popup,
            target_ui,
            state: ui_state,
        }
    }

    pub fn draw(&mut self) {
        let ui_y = screen_height() - 160.0;

        let popup_rectangle = self.activity_popup.last_drawn_rectangle;

        let mouse_pos = mouse_position();
        let is_grid_obstructed = popup_rectangle.contains(mouse_pos.into())
            || self.character_sheet_toggle.shown.get()
            || mouse_pos.1 >= ui_y - 1.0;
        let is_grid_receptive_to_input = !matches!(&*self.state.borrow(), UiState::Idle)
            && self.active_character().player_controlled
            && !is_grid_obstructed;
        let is_grid_receptive_to_dragging = !is_grid_obstructed;
        let grid_outcome = self.game_grid.draw(
            is_grid_receptive_to_input,
            is_grid_receptive_to_dragging,
            &mut self.state.borrow_mut(),
            is_grid_obstructed,
        );

        self.handle_grid_outcome(grid_outcome);

        draw_rectangle(0.0, ui_y, screen_width(), screen_height() - ui_y, BLACK);
        draw_line(0.0, ui_y, screen_width(), ui_y, 1.0, ORANGE);

        self.activity_popup.draw(20.0, ui_y + 1.0);

        self.player_portraits.draw(570.0, ui_y + 5.0);
        self.character_sheet_toggle.draw(570.0, ui_y + 90.0);

        let character_ui = self
            .character_uis
            .get_mut(&self.player_portraits.selected_id())
            .unwrap();

        character_ui.draw(ui_y + 5.0);

        /*
        character_ui.actions_section.draw(10.0, ui_y + 10.0);
        character_ui.action_points_row.draw(430.0, ui_y + 5.0);
        character_ui.resource_bars.draw(
            480.0 - character_ui.resource_bars.size().0 / 2.0,
            ui_y + 40.0,
        );
         */

        //self.log.draw(800.0, ui_y);
        let log_x = 950.0;
        self.log.draw(log_x, ui_y);

        // We draw this late to ensure that any hover popups are shown above other UI elements
        character_ui.conditions_list.draw(800.0, ui_y + 5.0);

        self.log.draw_tooltips(log_x, ui_y);

        self.character_portraits.draw(0.0, 0.0);

        self.target_ui
            .draw(screen_width() - self.target_ui.size().0 - 10.0, 10.0);

        if self.character_sheet_toggle.shown.get() {
            let outcome = character_ui
                .character_sheet
                .draw(&mut self.state.borrow_mut());

            self.character_sheet_toggle
                .shown
                .set(!outcome.clicked_close);

            if outcome.changed_state {
                println!("REQUESTED EQ CHANGE; new state");
                // Maybe drag was changed, or maybe the entire state; should be fine to assume the latter
                self.on_new_state();
            }
        }

        let character_ui = self
            .character_uis
            .get_mut(&self.player_portraits.selected_id())
            .unwrap();

        if let Some((btn_id, _btn_action, btn_pos)) = self.hovered_button {
            let btn = character_ui
                .hoverable_buttons
                .iter()
                .find(|btn| btn.id == btn_id)
                .unwrap();

            draw_button_tooltip(&self.font, btn_pos, &btn.tooltip());
        }
    }

    fn handle_grid_outcome(&mut self, outcome: GridOutcome) {
        self.character_portraits
            .set_hovered_character_id(outcome.hovered_character_id);

        if let Some(new_inspect_target) = outcome.switched_inspect_target {
            dbg!(new_inspect_target);
            let target = new_inspect_target.map(|id| self.characters.get_rc(id));
            self.target_ui.set_character(target);
        }

        if let Some(grid_switched_to) = outcome.switched_state {
            dbg!(&grid_switched_to);

            match grid_switched_to {
                NewState::Move => {
                    self.on_new_state();
                    self.activity_popup.on_new_movement_ap_cost();
                }
                NewState::Attack | NewState::ChoosingAction => {
                    self.on_new_state();
                }
            }
        }

        if outcome.switched_players_action_target {
            self.refresh_target_state();
        }

        if outcome.switched_movement_path {
            println!("SWITCHED MOVE PATH");
            self.refresh_movement_state();
        }
    }

    fn refresh_movement_state(&mut self) {
        if let UiState::ConfiguringAction(ConfiguredAction::Move {
            selected_movement_path,
            ..
        }) = &*self.state.borrow()
        {
            if !selected_movement_path.is_empty() {
                self.target_ui.clear_action();
            } else {
                self.target_ui
                    .set_action("Select a destination".to_string(), vec![], false);
            }

            //self.activity_popup.refresh_enabled_state();
        }
    }

    fn refresh_target_state(&mut self) {
        if matches!(
            &*self.state.borrow(),
            UiState::ConfiguringAction(ConfiguredAction::Attack { .. })
        ) {
            self.refresh_attack_state();
        }

        // Potentially change a "partially selected" (that was waiting for a proper target) to "fully selected"
        self.refresh_selected_action_button();

        if matches!(
            &*self.state.borrow(),
            UiState::ConfiguringAction(ConfiguredAction::CastSpell { .. })
        ) {
            self.refresh_cast_spell_state();
        }
    }

    fn refresh_attack_state(&mut self) {
        let UiState::ConfiguringAction(ConfiguredAction::Attack {
            attack,
            selected_enhancements,
            target,
        }) = &*self.state.borrow()
        else {
            panic!()
        };

        match target {
            Some(target_id) => {
                let target_char = self.characters.get(*target_id);

                let (_range, reach) = self
                    .active_character()
                    .reaches_with_attack(attack.hand, target_char.position.get());

                let mut details = vec![];

                if matches!(reach, ActionReach::No) {
                    details.push(("Can not reach!".to_string(), Goodness::Bad));
                }

                // We cannot know yet if the defender will react
                let defender_reaction = None;

                let chance = as_percentage(prob_attack_hit(
                    self.active_character(),
                    attack.hand,
                    target_char,
                    0,
                    selected_enhancements,
                    defender_reaction,
                ));

                for (term, bonus) in self.active_character().outgoing_attack_bonuses(
                    attack.hand,
                    selected_enhancements,
                    target_char.pos(),
                ) {
                    details.push((term.to_string(), bonus.goodness()));
                }
                for (term, bonus) in target_char.incoming_attack_bonuses(defender_reaction) {
                    details.push((term.to_string(), bonus.goodness()));
                }

                self.target_ui
                    .set_action(format!("Attack: {}", chance), details, true);
            }

            None => {
                self.target_ui
                    .set_action("Select an enemy".to_string(), vec![], false);
            }
        }

        //self.activity_popup.refresh_enabled_state();
    }

    fn refresh_cast_spell_state(&mut self) {
        let UiState::ConfiguringAction(ConfiguredAction::CastSpell {
            spell,
            selected_enhancements,
            target,
        }) = &*self.state.borrow()
        else {
            panic!()
        };

        let spell = *spell;

        match target {
            ActionTarget::Character(target_id, ..) => {
                let target_char = self.characters.get(*target_id);

                let mut details = vec![];

                let reaches = self.active_character().reaches_with_spell(
                    spell,
                    selected_enhancements,
                    target_char.pos(),
                );

                if !reaches {
                    details.push(("Can not reach!".to_string(), Goodness::Bad));
                }

                for (term, bonus) in self
                    .active_character()
                    .outgoing_spell_bonuses(selected_enhancements)
                {
                    details.push((term.to_string(), bonus.goodness()));
                }
                for (term, bonus) in target_char.incoming_spell_bonuses() {
                    details.push((term.to_string(), bonus.goodness()));
                }

                let action_text = match spell.target {
                    SpellTarget::Enemy { effect, .. } => {
                        let prob = effect
                            .defense_type
                            .map(|def| {
                                prob_spell_hit(
                                    self.active_character(),
                                    def,
                                    target_char,
                                    selected_enhancements,
                                )
                            })
                            .unwrap_or(1.0);
                        let chance = as_percentage(prob);

                        format!("{}: {}", spell.name, chance)
                    }
                    SpellTarget::Ally { .. } => spell.name.to_string(),
                    SpellTarget::None { .. } | SpellTarget::Area { .. } => {
                        unreachable!()
                    }
                };

                self.target_ui.set_action(action_text, details, true);
            }

            ActionTarget::Position(..) => {
                assert!(matches!(spell.target, SpellTarget::Area { .. }));
                self.target_ui
                    .set_action(format!("{} (AoE)", spell.name), vec![], false);
            }

            ActionTarget::None => {
                match spell.target {
                    SpellTarget::Enemy { .. } => {
                        self.target_ui
                            .set_action("Select an enemy".to_string(), vec![], false);
                    }

                    SpellTarget::Ally { .. } => {
                        self.target_ui
                            .set_action("Select an ally".to_string(), vec![], false);
                    }

                    SpellTarget::None { .. } => {
                        let header = spell.name.to_string();
                        self.target_ui.set_action(header, vec![], false);
                    }

                    SpellTarget::Area { .. } => {
                        self.target_ui
                            .set_action("Select an area".to_string(), vec![], false);
                    }
                };
            }
        }
    }

    fn set_allowed_to_use_action_buttons(&self, allowed: bool) {
        for btn in self.character_uis[&self.player_portraits.selected_id()]
            .tracked_action_buttons
            .values()
        {
            if allowed {
                let enabled = match btn.action {
                    ButtonAction::Action(base_action) => {
                        self.active_character().can_use_action(base_action)
                    }
                    _ => unreachable!(),
                };
                btn.enabled.set(enabled);
            } else {
                btn.enabled.set(false);
            }
        }
    }

    fn active_character(&self) -> &Character {
        self.characters.get(self.active_character_id)
    }

    fn refresh_selected_action_button(&mut self) {
        if let UiState::ConfiguringAction(configured_action) = &*self.state.borrow() {
            let fully_selected = configured_action
                .has_required_player_input(self.active_character(), &self.characters);
            self.set_selected_action(Some((
                ButtonAction::Action(configured_action.base_action()),
                fully_selected,
            )));
        } else {
            self.set_selected_action(None);
        }
    }

    fn set_selected_action(&self, selected_action: Option<(ButtonAction, bool)>) {
        let mut selected_id = None;
        let mut fully_selected = false;
        if let Some((action, fully)) = selected_action {
            selected_id = Some(button_action_id(action));
            fully_selected = fully;
        }

        if self.active_character().player_controlled {
            if self.player_portraits.selected_id() != self.active_character_id {
                println!("SWITCHING");
                self.player_portraits
                    .set_selected_id(self.active_character_id);
            }

            for (btn_action_id, btn) in
                &self.character_uis[&self.active_character_id].tracked_action_buttons
            {
                if selected_id.as_ref() == Some(btn_action_id) {
                    if fully_selected {
                        btn.selected.set(ButtonSelected::Yes);
                    } else {
                        btn.selected.set(ButtonSelected::Partially);
                    }
                } else {
                    btn.selected.set(ButtonSelected::No);
                }
            }
        }
    }

    pub fn set_state(&mut self, state: UiState) {
        dbg!(&state);
        *self.state.borrow_mut() = state;

        self.on_new_state();
    }

    fn on_new_state(&mut self) {
        dbg!(&self.state.borrow());

        self.activity_popup.additional_line = None;

        let mut relevant_action_button = None;

        let mut on_attacked = false;

        match &*self.state.borrow() {
            UiState::ConfiguringAction(configured_action) => {
                self.set_allowed_to_use_action_buttons(true);

                relevant_action_button = self.character_uis[&self.active_character_id]
                    .tracked_action_buttons
                    .get(&button_action_id(ButtonAction::Action(
                        configured_action.base_action(),
                    )))
                    .cloned();
                assert!(
                    relevant_action_button.is_some(),
                    "No button found for action: {:?}",
                    configured_action
                );

                if let ConfiguredAction::EndTurn = configured_action {
                    self.target_ui.clear_action();
                }
            }

            UiState::ReactingToAttack { .. } => {
                self.target_ui
                    .set_action("Select a reaction".to_string(), vec![], false);
                self.set_allowed_to_use_action_buttons(false);
                on_attacked = true;
            }

            UiState::ReactingToHit { .. } => {
                self.target_ui
                    .set_action("Select a reaction".to_string(), vec![], false);
                self.set_allowed_to_use_action_buttons(false);
            }

            UiState::ReactingToOpportunity { .. } => {
                self.target_ui
                    .set_action("Select a reaction".to_string(), vec![], false);
                self.set_allowed_to_use_action_buttons(false);
            }

            UiState::ChoosingAction => {
                self.target_ui
                    .set_action("Select an action".to_string(), vec![], false);

                self.set_allowed_to_use_action_buttons(true);
            }

            UiState::Idle => {
                self.target_ui.clear_action();
                self.set_allowed_to_use_action_buttons(false);
            }
        }

        if on_attacked {
            self.activity_popup.refresh_on_attacked_state();
        }

        self.activity_popup
            .on_new_state(self.active_character_id, relevant_action_button);

        self.maybe_refresh_equipment_state();

        self.refresh_target_state();
        self.refresh_movement_state();
        self.game_grid.update_move_speed(self.active_character_id);
        self.refresh_selected_action_button();
        self.target_ui.rebuild_character_ui();
    }

    fn maybe_refresh_equipment_state(&mut self) {
        match &*self.state.borrow() {
            UiState::ConfiguringAction(ConfiguredAction::ChangeEquipment { drag }) => {
                self.target_ui
                    .set_action("Change equipment".to_string(), vec![], false);

                if let Some(EquipmentDrag {
                    to_idx: Some(_), ..
                }) = drag
                {
                    let description = self.character_uis[&self.active_character_id]
                        .character_sheet
                        .describe_requested_equipment_change(drag.unwrap());
                    self.activity_popup.additional_line = Some(description);
                } else {
                    self.activity_popup.additional_line =
                        Some("Drag something to equip/unequip it".to_string());
                    self.character_sheet_toggle.shown.set(true);
                }
            }

            UiState::ConfiguringAction(ConfiguredAction::UseConsumable(consumption)) => {
                self.target_ui
                    .set_action("Use consumable".to_string(), vec![], false);

                if let Some(consumption) = consumption {
                    self.activity_popup.additional_line =
                        Some(format!("Use {}", consumption.consumable.name));
                } else {
                    self.activity_popup.additional_line =
                        Some("Right click a consumable in your inventory to use it".to_string());
                    self.character_sheet_toggle.shown.set(true);
                }
            }

            _ => {}
        };
    }

    pub fn has_ongoing_animation(&self) -> bool {
        self.animation_stopwatch.remaining.is_some()
    }

    pub fn handle_game_event(&mut self, event: GameEvent) {
        dbg!(&event);

        self.target_ui.rebuild_character_ui();

        match event {
            GameEvent::LogLine(line) => {
                self.log.add(line);
            }

            GameEvent::CharacterReactedToAttacked { reactor } => {
                let reactor_pos = self.characters.get(reactor).pos();
                self.game_grid.add_text_effect(
                    reactor_pos,
                    0.0,
                    0.5,
                    "!".to_string(),
                    Goodness::Neutral,
                );

                self.animation_stopwatch.set_to_at_least(0.4);
            }

            GameEvent::CharacterReactedWithOpportunityAttack { reactor } => {
                let reactor = self.characters.get(reactor);
                self.log.add("Opportunity attack:".to_string());
                self.game_grid.add_text_effect(
                    reactor.pos(),
                    0.0,
                    0.5,
                    "!".to_string(),
                    Goodness::Neutral,
                );

                self.animation_stopwatch.set_to_at_least(0.4);
            }

            GameEvent::CharacterReactedToHit {
                main_line,
                detail_lines,
                reactor,
                outcome,
            } => {
                self.log.add_with_details(main_line, detail_lines);

                let reactor_pos = self.characters.get(reactor).pos();

                if let Some(condition) = outcome.received_condition {
                    self.game_grid.add_text_effect(
                        reactor_pos,
                        0.0,
                        1.0,
                        format!("{:?}", condition),
                        Goodness::Neutral,
                    );
                }

                let attacker_pos = self.active_character().pos();
                if let Some(offensive) = outcome.offensive {
                    if let Some(condition) = offensive.inflicted_condition {
                        self.game_grid.add_text_effect(
                            attacker_pos,
                            0.0,
                            1.0,
                            format!("{:?}", condition),
                            Goodness::Neutral,
                        );
                    } else {
                        self.game_grid.add_text_effect(
                            attacker_pos,
                            0.0,
                            1.0,
                            "Miss".to_string(),
                            Goodness::Neutral,
                        );
                    }
                }
                self.animation_stopwatch.set_to_at_least(0.5);
            }

            GameEvent::Attacked {
                attacker,
                target,
                outcome,
                detail_lines,
            } => {
                let mut line = format!(
                    "{} attacked {}",
                    self.characters.get(attacker).name,
                    self.characters.get(target).name
                );

                match outcome {
                    AttackOutcome::Hit(dmg) => line.push_str(&format!(" ({} damage)", dmg)),
                    AttackOutcome::Dodge => line.push_str(" (dodge)"),
                    AttackOutcome::Parry => line.push_str(" (parry)"),
                    AttackOutcome::Miss => line.push_str(" (miss)"),
                }

                self.log.add_with_details(line, detail_lines);

                let attacker_pos = self.characters.get(attacker).pos();
                let target_pos = self.characters.get(target).pos();

                let duration = 0.15 * distance_between(attacker_pos, target_pos);

                self.animation_stopwatch.set_to_at_least(duration + 0.6);
                let impact_text = match outcome {
                    AttackOutcome::Hit(damage) => format!("{}", damage),
                    AttackOutcome::Dodge => "Dodge".to_string(),
                    AttackOutcome::Parry => "Parry".to_string(),
                    AttackOutcome::Miss => "Miss".to_string(),
                };
                let goodness = if matches!(outcome, AttackOutcome::Hit(..)) {
                    Goodness::Bad
                } else {
                    Goodness::Neutral
                };

                self.game_grid.add_effect(
                    attacker_pos,
                    target_pos,
                    Effect {
                        start_time: 0.0,
                        end_time: duration,
                        variant: EffectVariant::Line {
                            thickness: 1.0,
                            end_thickness: Some(10.0),
                            color: RED,
                            extend_gradually: true,
                        },
                    },
                );
                self.game_grid.add_effect(
                    attacker_pos,
                    target_pos,
                    Effect {
                        start_time: duration,
                        end_time: duration + 0.2,
                        variant: EffectVariant::At(
                            EffectPosition::Destination,
                            EffectGraphics::Circle {
                                radius: 25.0,
                                end_radius: Some(5.0),
                                fill: None,
                                stroke: Some((MAGENTA, 2.0)),
                            },
                        ),
                    },
                );

                self.game_grid
                    .add_text_effect(target_pos, duration, 0.5, impact_text, goodness);
            }

            GameEvent::SpellWasCast {
                caster,
                target_outcome,
                area_outcomes,
                spell,
                detail_lines,
            } => {
                let mut line = if let Some((target_id, _outcome)) = target_outcome {
                    format!(
                        "{} cast {} on {}",
                        self.characters.get(caster).name,
                        spell.name,
                        self.characters.get(target_id).name
                    )
                } else if matches!(spell.modifier, SpellModifier::Spell) {
                    format!("{} cast {}", self.characters.get(caster).name, spell.name,)
                } else {
                    format!("{} used {}", self.characters.get(caster).name, spell.name,)
                };

                if let Some((_target_id, outcome)) = target_outcome {
                    match outcome {
                        SpellTargetOutcome::HitEnemy { damage } => {
                            if let Some(dmg) = damage {
                                line.push_str(&format!(" ({} damage)", dmg))
                            } else {
                                line.push_str(" (hit)");
                            }
                        }
                        SpellTargetOutcome::Resist => line.push_str(" (miss)"),
                        SpellTargetOutcome::HealedAlly(healing) => {
                            line.push_str(&format!(" ({} healing)", healing))
                        }
                    }
                }

                self.log.add_with_details(line, detail_lines);

                let mut duration = 0.0;

                let animation_color = spell.animation_color;
                if let Some((target, outcome)) = target_outcome {
                    let caster_pos = self.characters.get(caster).pos();
                    let target_pos = self.characters.get(target).pos();

                    duration = 0.15 * distance_between(caster_pos, target_pos);

                    self.game_grid.add_effect(
                        caster_pos,
                        target_pos,
                        Effect {
                            start_time: 0.0,
                            end_time: duration,
                            variant: EffectVariant::At(
                                EffectPosition::Projectile,
                                EffectGraphics::Circle {
                                    radius: 10.0,
                                    end_radius: Some(15.0),
                                    fill: Some(animation_color),
                                    stroke: None,
                                },
                            ),
                        },
                    );
                    self.game_grid.add_effect(
                        caster_pos,
                        target_pos,
                        Effect {
                            start_time: 0.025,
                            end_time: duration + 0.025,
                            variant: EffectVariant::At(
                                EffectPosition::Projectile,
                                EffectGraphics::Circle {
                                    radius: 8.0,
                                    end_radius: Some(13.0),
                                    fill: Some(animation_color),
                                    stroke: None,
                                },
                            ),
                        },
                    );
                    self.game_grid.add_effect(
                        caster_pos,
                        target_pos,
                        Effect {
                            start_time: 0.05,
                            end_time: duration + 0.05,
                            variant: EffectVariant::At(
                                EffectPosition::Projectile,
                                EffectGraphics::Circle {
                                    radius: 6.0,
                                    end_radius: Some(11.0),
                                    fill: Some(animation_color),
                                    stroke: None,
                                },
                            ),
                        },
                    );

                    let (target_text, goodness) = match outcome {
                        SpellTargetOutcome::HitEnemy { damage } => {
                            if let Some(dmg) = damage {
                                (format!("{}", dmg), Goodness::Bad)
                            } else {
                                ("Hit".to_string(), Goodness::Bad)
                            }
                        }
                        SpellTargetOutcome::Resist => ("Resist".to_string(), Goodness::Neutral),
                        SpellTargetOutcome::HealedAlly(healing) => {
                            (format!("{}", healing), Goodness::Good)
                        }
                    };

                    self.game_grid.add_text_effect(
                        target_pos,
                        duration,
                        0.5,
                        target_text,
                        goodness,
                    );

                    self.animation_stopwatch.set_to_at_least(duration + 0.3);
                }

                if let Some((area_center_pos, outcomes)) = area_outcomes {
                    let area_duration = 0.2;

                    for (target_id, outcome) in outcomes {
                        let target_pos = self.characters.get(target_id).pos();

                        self.game_grid.add_effect(
                            (area_center_pos.0, area_center_pos.1),
                            target_pos,
                            Effect {
                                start_time: duration,
                                end_time: duration + area_duration,
                                variant: EffectVariant::At(
                                    EffectPosition::Destination,
                                    EffectGraphics::Circle {
                                        radius: 20.0,
                                        stroke: Some((animation_color, 4.0)),
                                        end_radius: Some(25.0),
                                        fill: None,
                                    },
                                ),
                            },
                        );

                        let (target_text, goodness) = match outcome {
                            SpellTargetOutcome::HitEnemy { damage } => {
                                if let Some(dmg) = damage {
                                    (format!("{}", dmg), Goodness::Bad)
                                } else {
                                    ("Hit".to_string(), Goodness::Bad)
                                }
                            }
                            SpellTargetOutcome::Resist => ("Resist".to_string(), Goodness::Neutral),
                            SpellTargetOutcome::HealedAlly(healing) => {
                                (format!("{}", healing), Goodness::Good)
                            }
                        };

                        self.game_grid.add_text_effect(
                            target_pos,
                            duration,
                            0.5,
                            target_text,
                            goodness,
                        );

                        self.animation_stopwatch.set_to_at_least(duration + 0.3);
                    }
                }
            }

            GameEvent::ConsumableWasUsed { user, consumable } => {
                self.log.add(format!(
                    "{} used {}",
                    self.characters.get(user).name,
                    consumable.name
                ));
            }

            GameEvent::CharacterDied {
                character,
                new_active,
            } => {
                self.log
                    .add(format!("{} died", self.characters.get(character).name));

                self.target_ui.clear_character_if_dead();

                self.game_grid.remove_dead();
                self.character_portraits.remove_dead();

                if self.characters.get(character).player_controlled {
                    self.player_portraits.mark_as_dead(character);
                }

                if let Some(new_active) = new_active {
                    self.set_new_active_character_id(new_active);
                }
            }
            GameEvent::NewTurn { new_active } => {
                self.log.add("---".to_string());
                self.animation_stopwatch.set_to_at_least(0.5);
                self.set_new_active_character_id(new_active);
            }
            GameEvent::Moved {
                character,
                from,
                to,
            } => {
                let mut duration = 0.4;
                if from.0 != to.0 || from.1 != to.1 {
                    // diagonal takes longer
                    duration *= 1.41;
                }

                self.game_grid
                    .set_character_motion(character, from, to, duration);
                self.animation_stopwatch.set_to_at_least(duration);
            }
        }
    }

    fn set_new_active_character_id(&mut self, new_active_id: CharacterId) {
        if new_active_id != self.active_character_id {
            // When control switches to a new player controlled character, make the UI show that character
            println!(
                "Switching shown char from {} to {}",
                self.active_character_id, new_active_id
            );
            if self.characters.get(new_active_id).player_controlled {
                self.player_portraits.set_selected_id(new_active_id);

                // In case we're hovering a button that will no longer be shown due to the character switch,
                // we need to clear it, so that we don't panic trying to render its tooltip for example
                self.hovered_button = None;
            }
        }

        self.active_character_id = new_active_id;
    }

    pub fn update(&mut self, game: &CoreGame, elapsed: f32) -> Option<PlayerChose> {
        self.set_allowed_to_use_action_buttons(
            self.player_portraits.selected_id() == self.active_character_id,
        );

        let selected_char = self.characters.get(self.player_portraits.selected_id());
        let selected_in_grid = if selected_char.is_dead() {
            None
        } else {
            Some(selected_char.id())
        };

        self.game_grid
            .update(self.active_character_id, selected_in_grid, elapsed);

        let popup_outcome = self.activity_popup.update();

        let mut player_choice = None;
        match popup_outcome {
            Some(ActivityPopupOutcome::ClickedProceed) => {
                player_choice = Some(self.handle_popup_proceed());
            }
            Some(ActivityPopupOutcome::ChangedSpellEnhancements) => {
                // TODO update hit chance?
                self.refresh_cast_spell_state();
            }
            Some(ActivityPopupOutcome::ChangedAttackEnhancements) => {
                self.refresh_attack_state();
            }
            None => {}
        }

        self.event_queue
            .take()
            .into_iter()
            .for_each(|event| self.handle_internal_ui_event(event));

        self.character_portraits.update(game);
        self.player_portraits.update(game);

        self.update_character_status(&game.characters);

        let character_ui = self
            .character_uis
            .get_mut(&self.player_portraits.selected_id())
            .unwrap();
        if let Some(hovered_btn) = self.hovered_button {
            character_ui.action_points_row.reserved_and_hovered_ap = (
                self.activity_popup.reserved_and_hovered_action_points().0,
                hovered_btn.1.action_point_cost(),
            );
            character_ui
                .mana_bar
                .borrow_mut()
                .set_reserved(hovered_btn.1.mana_cost());
            character_ui
                .stamina_bar
                .borrow_mut()
                .set_reserved(hovered_btn.1.stamina_cost());
        } else {
            character_ui.action_points_row.reserved_and_hovered_ap =
                self.activity_popup.reserved_and_hovered_action_points();
            character_ui
                .mana_bar
                .borrow_mut()
                .set_reserved(self.activity_popup.mana_points());
            character_ui
                .stamina_bar
                .borrow_mut()
                .set_reserved(self.activity_popup.stamina_points());
        };

        self.animation_stopwatch.update(elapsed);

        player_choice
    }

    fn handle_popup_proceed(&mut self) -> PlayerChose {
        // Action button is highlighted while the action is being configured in the popup. That should be cleared now.
        // TODO shouldn't we rather change the state, and rely on refresh_selected_action_button to clear this?
        self.set_selected_action(None);

        match &*self.state.borrow() {
            UiState::ConfiguringAction(configured_action) => {
                let action = match &configured_action {
                    ConfiguredAction::Attack {
                        attack,
                        selected_enhancements,
                        target,
                        ..
                    } => Some(Action::Attack {
                        hand: attack.hand,
                        enhancements: selected_enhancements.clone(),
                        target: target.unwrap(),
                    }),
                    &ConfiguredAction::CastSpell {
                        spell,
                        selected_enhancements,
                        target,
                    } => Some(Action::CastSpell {
                        spell: *spell,
                        enhancements: selected_enhancements.clone(),
                        target: target.clone(),
                    }),
                    &ConfiguredAction::Move {
                        ap_cost,
                        selected_movement_path,
                    } => {
                        let mut reversed_path = selected_movement_path.clone();
                        // Remove the character's current position; it should not be part of the movement path
                        reversed_path.remove(reversed_path.len() - 1);

                        let positions = reversed_path
                            .into_iter()
                            .rev()
                            .map(|(_dist, (x, y))| (x, y))
                            .collect();

                        let stamina_cost = self.activity_popup.movement_stamina_cost();
                        Some(Action::Move {
                            action_point_cost: *ap_cost - stamina_cost,
                            stamina_cost,
                            positions,
                        })
                    }
                    &ConfiguredAction::ChangeEquipment { drag } => {
                        let (from, to) = self
                            .character_uis
                            .get_mut(&self.active_character_id)
                            .unwrap()
                            .character_sheet
                            .resolve_drag_to_slots(drag.unwrap());

                        Some(Action::ChangeEquipment { from, to })
                    }
                    &ConfiguredAction::UseConsumable(consumption) => Some(Action::UseConsumable {
                        inventory_equipment_index: consumption.unwrap().equipment_idx,
                    }),
                    &ConfiguredAction::EndTurn => None,
                };
                PlayerChose::Action(action)
            }

            UiState::ReactingToAttack { selected, .. } => PlayerChose::AttackedReaction(*selected),
            UiState::ReactingToHit { selected, .. } => PlayerChose::HitReaction(*selected),
            UiState::ReactingToOpportunity { selected, .. } => {
                PlayerChose::OpportunityAttack(*selected)
            }

            UiState::ChoosingAction | UiState::Idle => unreachable!(),
        }
    }

    fn handle_internal_ui_event(&mut self, event: InternalUiEvent) {
        match event {
            InternalUiEvent::ButtonHovered(button_id, button_action, hovered) => {
                if let Some(pos) = hovered {
                    self.hovered_button = Some((button_id, button_action, pos));
                } else if let Some(previously_hovered_button) = self.hovered_button {
                    if button_id == previously_hovered_button.0 {
                        self.hovered_button = None
                    }
                }
            }

            InternalUiEvent::ButtonClicked(_button_id, btn_action) => match btn_action {
                ButtonAction::Action(base_action) => {
                    let may_choose_action = matches!(
                        &*self.state.borrow(),
                        UiState::ChoosingAction | UiState::ConfiguringAction(..)
                    );

                    if may_choose_action && self.active_character().can_use_action(base_action) {
                        self.set_state(UiState::ConfiguringAction(
                            ConfiguredAction::from_base_action(base_action),
                        ));
                    } else {
                        println!("Cannot choose this action at this time");
                    }
                }

                _ => unreachable!(),
            },
        }
    }

    fn update_character_status(&mut self, characters: &Characters) {
        for (id, character) in characters.iter_with_ids() {
            if let Some(ui) = self.character_uis.get_mut(id) {
                ui.health_bar
                    .borrow_mut()
                    .set_current(character.health.current());
                ui.mana_bar
                    .borrow_mut()
                    .set_current(character.mana.current());
                ui.stamina_bar
                    .borrow_mut()
                    .set_current(character.stamina.current());

                ui.conditions_list.descriptions = character.condition_infos();

                ui.action_points_row.current_ap = self
                    .characters
                    .get(self.player_portraits.selected_id())
                    .action_points
                    .current();
                ui.action_points_row.is_characters_turn = *id == self.active_character_id;
            }
        }
    }
}

fn build_character_uis<'a>(
    equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
    icons: &HashMap<IconId, Texture2D>,
    event_queue: &Rc<RefCell<Vec<InternalUiEvent>>>,
    simple_font: &Font,
    characters: impl Iterator<Item = &'a Rc<Character>>,
) -> HashMap<u32, CharacterUi> {
    let mut next_button_id = 1;

    let mut character_uis: HashMap<CharacterId, CharacterUi> = Default::default();

    for character in characters {
        if !character.player_controlled {
            continue;
        }

        let character_ui = build_character_ui(
            equipment_icons,
            icons,
            event_queue,
            simple_font,
            character,
            &mut next_button_id,
        );

        character_uis.insert(character.id(), character_ui);
    }
    character_uis
}

pub fn build_character_ui(
    equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
    icons: &HashMap<IconId, Texture2D>,
    event_queue: &Rc<RefCell<Vec<InternalUiEvent>>>,
    simple_font: &Font,
    character: &Rc<Character>,
    next_button_id: &mut u32,
) -> CharacterUi {
    let mut new_button = |btn_action, character: Option<Rc<Character>>, enabled: bool| {
        let btn = ActionButton::new(btn_action, event_queue, *next_button_id, icons, character);
        btn.enabled.set(enabled);
        *next_button_id += 1;
        btn
    };

    let mut tracked_action_buttons = HashMap::new();
    let mut hoverable_buttons = vec![];
    let mut basic_buttons = vec![];
    let mut spell_buttons = vec![];

    let mut attack_button_for_character_sheet = None;
    let mut spell_buttons_for_character_sheet = vec![];
    let mut attack_enhancement_buttons_for_character_sheet = vec![];

    for action in character.known_actions() {
        let btn_action = ButtonAction::Action(action);
        let btn = Rc::new(new_button(btn_action, Some(character.clone()), true));
        tracked_action_buttons.insert(button_action_id(btn_action), Rc::clone(&btn));
        hoverable_buttons.push(Rc::clone(&btn));
        match action {
            BaseAction::Attack { .. } => {
                basic_buttons.push(btn);

                let btn = Rc::new(new_button(btn_action, Some(character.clone()), false));
                attack_button_for_character_sheet = Some(btn.clone());
                hoverable_buttons.push(btn);
            }
            BaseAction::CastSpell(spell) => {
                spell_buttons.push(btn);

                let btn = Rc::new(new_button(btn_action, Some(character.clone()), false));

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
                            hoverable_buttons.push(enhancement_btn.clone());
                            Some(enhancement_btn)
                        } else {
                            None
                        }
                    })
                    .collect();
                spell_buttons_for_character_sheet.push((btn.clone(), enhancement_buttons));

                hoverable_buttons.push(btn);
            }
            BaseAction::Move => {
                basic_buttons.push(btn);
            }
            BaseAction::ChangeEquipment => basic_buttons.push(btn),
            BaseAction::UseConsumable => basic_buttons.push(btn),
            BaseAction::EndTurn => basic_buttons.push(btn),
        }
    }

    let mut reaction_buttons_for_character_sheet = vec![];
    for (_subtext, reaction) in character.known_on_attacked_reactions() {
        let btn_action = ButtonAction::OnAttackedReaction(reaction);
        let btn = Rc::new(new_button(btn_action, None, false));
        hoverable_buttons.push(Rc::clone(&btn));
        reaction_buttons_for_character_sheet.push(btn);
    }
    for (_subtext, reaction) in character.known_on_hit_reactions() {
        let btn_action = ButtonAction::OnHitReaction(reaction);
        let btn = Rc::new(new_button(btn_action, None, false));
        hoverable_buttons.push(Rc::clone(&btn));
        reaction_buttons_for_character_sheet.push(btn);
    }

    // TODO: Only include inherently known enhancements here; not those gained from weapons (since weapons can be unequipped
    // without the character sheet being updated)
    for (_subtext, enhancement) in character.known_attack_enhancements(HandType::MainHand) {
        let btn_action = ButtonAction::AttackEnhancement(enhancement);
        let btn = Rc::new(new_button(btn_action, None, false));
        hoverable_buttons.push(Rc::clone(&btn));
        attack_enhancement_buttons_for_character_sheet.push(btn);
    }

    let character_sheet = CharacterSheet::new(
        simple_font,
        Rc::clone(character),
        equipment_icons,
        attack_button_for_character_sheet,
        reaction_buttons_for_character_sheet,
        attack_enhancement_buttons_for_character_sheet,
        spell_buttons_for_character_sheet,
    );

    let mut upper_buttons = basic_buttons;
    let mut lower_buttons = spell_buttons;
    while lower_buttons.len() > upper_buttons.len() + 1 {
        upper_buttons.push(lower_buttons.pop().unwrap());
    }

    let upper_row = buttons_row(
        upper_buttons
            .into_iter()
            .map(|btn| Element::Rc(btn))
            .collect(),
    );
    let lower_row = buttons_row(
        lower_buttons
            .into_iter()
            .map(|btn| Element::Rc(btn))
            .collect(),
    );

    let actions_section = Container {
        layout_dir: LayoutDirection::Vertical,
        margin: 5.0,
        children: vec![upper_row, lower_row],
        ..Default::default()
    };

    let resource_bars = ResourceBars::new(character, simple_font);

    let action_points_row = ActionPointsRow::new(
        character.max_reactive_action_points,
        (20.0, 20.0),
        0.3,
        Style {
            border_color: Some(WHITE),
            ..Default::default()
        },
    );

    CharacterUi {
        tracked_action_buttons,
        action_points_row,
        hoverable_buttons,
        actions_section,
        character_sheet,
        health_bar: resource_bars.health_bar,
        mana_bar: resource_bars.mana_bar,
        stamina_bar: resource_bars.stamina_bar,
        resource_bars: resource_bars.container,
        conditions_list: ConditionsList::new(simple_font.clone(), vec![]),
    }
}

pub struct ResourceBars {
    pub container: Container,
    health_bar: Rc<RefCell<LabelledResourceBar>>,
    mana_bar: Rc<RefCell<LabelledResourceBar>>,
    stamina_bar: Rc<RefCell<LabelledResourceBar>>,
}

impl Drawable for ResourceBars {
    fn draw(&self, x: f32, y: f32) {
        self.container.draw(x, y);
    }

    fn size(&self) -> (f32, f32) {
        self.container.size()
    }
}

impl ResourceBars {
    pub fn new(character: &Character, font: &Font) -> Self {
        let health_bar = Rc::new(RefCell::new(LabelledResourceBar::new(
            character.health.current(),
            character.health.max(),
            "Health",
            RED,
            font.clone(),
        )));

        let mana_bar = Rc::new(RefCell::new(LabelledResourceBar::new(
            character.mana.current(),
            character.mana.max(),
            "Mana",
            BLUE,
            font.clone(),
        )));

        let stamina_bar = Rc::new(RefCell::new(LabelledResourceBar::new(
            character.stamina.current(),
            character.stamina.max(),
            "Stamina",
            GREEN,
            font.clone(),
        )));

        let container = Container {
            layout_dir: LayoutDirection::Horizontal,
            margin: 9.0,
            align: Align::End,
            children: vec![
                Element::RcRefCell(health_bar.clone()),
                Element::RcRefCell(mana_bar.clone()),
                Element::RcRefCell(stamina_bar.clone()),
            ],
            style: Style {
                border_color: Some(DARKGRAY),
                padding: 5.0,
                ..Default::default()
            },
            ..Default::default()
        };

        Self {
            container,
            health_bar,
            mana_bar,
            stamina_bar,
        }
    }
}

fn button_action_id(btn_action: ButtonAction) -> String {
    match btn_action {
        ButtonAction::Action(base_action) => match base_action {
            BaseAction::Attack(attack) => format!("ATTACK_{:?}", attack.hand),
            BaseAction::CastSpell(spell) => format!("SPELL_{}", spell.name),
            BaseAction::Move => "MOVE".to_string(),
            BaseAction::ChangeEquipment => "CHANGING_EQUIPMENT".to_string(),
            BaseAction::UseConsumable => "USING_CONSUMABLE".to_string(),
            BaseAction::EndTurn => "END_TURN".to_string(),
        },
        _ => unreachable!(),
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

#[derive(Debug)]
pub enum PlayerChose {
    AttackedReaction(Option<OnAttackedReaction>),
    HitReaction(Option<OnHitReaction>),
    OpportunityAttack(bool),
    Action(Option<Action>),
}
