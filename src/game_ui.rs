use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use indexmap::IndexMap;
use macroquad::{
    color::{Color, BLACK, BLUE, DARKGRAY, GRAY, GREEN, LIGHTGRAY, MAGENTA, RED, WHITE},
    conf,
    input::{is_key_down, is_key_pressed, mouse_position, KeyCode},
    math::Rect,
    shapes::draw_rectangle,
    text::{self, Font},
    texture::{draw_texture, Texture2D},
    window::{screen_height, screen_width},
};

use crate::{
    action_button::{
        draw_button_tooltip, draw_tooltip, ActionButton, ButtonAction, ButtonContext,
        ButtonHovered, ButtonSelected, InternalUiEvent, Side, TooltipPositionPreference,
        ACTION_BUTTON_BG_COLOR, REGULAR_ACTION_BUTTON_SIZE,
    },
    activity_popup::{ActivityPopup, ActivityPopupOutcome},
    banner::Banner,
    base_ui::{Align, Container, Drawable, Element, LayoutDirection, Rectangle, Style, TextLine},
    character_sheet::{CharacterSheet, CHARACTER_SHEET_BG_COLOR},
    conditions_ui::ConditionsList,
    core::{
        distance_between, predict_ability, predict_attack, Ability, AbilityAreaOutcome,
        AbilityEnhancement, AbilityId, AbilityResolvedEvent, AbilityRollType, AbilityTarget,
        AbilityTargetOutcome, Action, ActionReach, ActionTarget, ApplyEffect, AreaShape,
        AttackAction, AttackEnhancement, AttackEnhancementEffect, AttackHitType, AttackOutcome,
        AttackedEvent, BaseAction, Character, CharacterId, Characters, Condition, CoreGame,
        DamageInterval, DamageSource, GameEvent, Goodness, HandType, MovementType,
        OnAttackedReaction, OnHitReaction, Position, TargetPrediction,
    },
    equipment_ui::{EquipmentConsumption, EquipmentDrag},
    game_ui_components::{
        ActionPointsRow, CharacterSheetToggle, LabelledResourceBar, Log, PlayerPortraits,
        TopCharacterPortraits,
    },
    grid::{
        Effect, EffectGraphics, EffectPosition, EffectVariant, GameGrid, GridOutcome, NewState,
        TargetEffectPreview, TextEffectStyle,
    },
    init_fight_map::GameInitState,
    resources::{GameResources, UiResources},
    settings::build_settings,
    sounds::{SoundId, SoundPlayer},
    target_ui::TargetUi,
    textures::{EquipmentIconId, IconId, PortraitId, SpriteId, StatusId, UI_TEXTURE},
    util::{COL_BLACK_BLUE, COL_BLUE, COL_GREEN_0, COL_GREEN_1, COL_GREEN_2, COL_GREEN_3, COL_RED},
};

#[derive(Debug, Clone, PartialEq)]
pub enum UiState {
    ChoosingAction,
    ConfiguringAction(ConfiguredAction),
    ReactingToAttack {
        hand: HandType,
        attacker: CharacterId,
        defender: CharacterId,
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
    ReactingToMovementAttackOpportunity {
        reactor: CharacterId,
        target: CharacterId,
        movement: ((i32, i32), (i32, i32)),
        selected: bool,
    },
    ReactingToRangedAttackOpportunity {
        reactor: CharacterId,
        attacker: CharacterId,
        victim: CharacterId,
        selected: bool,
    },
    Idle,
}

impl UiState {
    pub fn action_usability_problem(
        &self,
        relevant_character: &Character,
        characters: &Characters,
    ) -> Option<&'static str> {
        match self {
            UiState::ConfiguringAction(configured_action) => {
                configured_action.usability_problem(relevant_character, characters)
            }

            _ => None,
        }
    }

    pub fn players_action_target(&self) -> ActionTarget {
        match self {
            UiState::ConfiguringAction(configured_action) => match configured_action {
                ConfiguredAction::Attack { target, .. } => target
                    .map(|id| ActionTarget::Character(id, None))
                    .unwrap_or(ActionTarget::None),
                ConfiguredAction::UseAbility { target, .. } => target.clone(),
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
                ConfiguredAction::UseAbility { target, .. } => *target = new_target,

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
    UseAbility {
        ability: Ability,
        selected_enhancements: Vec<AbilityEnhancement>,
        target: ActionTarget,
    },
    Move {
        // Including the actor's current location, going all the way to the destination. Each pos is annotated with "total dist from start"
        selected_movement_path: Vec<(f32, Position)>,
        cost: u32,
    },
    ChangeEquipment {
        drag: Rc<RefCell<Option<EquipmentDrag>>>,
    },
    UseConsumable(Option<EquipmentConsumption>),
}

const OUT_OF_REACH: &'static str = "Out of reach";

impl ConfiguredAction {
    fn has_target(&self) -> bool {
        match self {
            ConfiguredAction::Attack { target, .. } => target.is_some(),
            ConfiguredAction::UseAbility { target, .. } => !matches!(target, ActionTarget::None),
            ConfiguredAction::Move { .. } => false,
            ConfiguredAction::ChangeEquipment { .. } => false,
            ConfiguredAction::UseConsumable(..) => false,
        }
    }

    fn usability_problem(
        &self,
        relevant_character: &Character,
        characters: &Characters,
    ) -> Option<&'static str> {
        match self {
            ConfiguredAction::Attack {
                target,
                attack,
                selected_enhancements,
                ..
            } => match target {
                Some(target_id) => {
                    let target_char = characters.get(*target_id);
                    let (_range, reach) = relevant_character.reaches_with_attack(
                        attack.hand,
                        target_char.position.get(),
                        selected_enhancements.iter().map(|e| e.effect),
                    );
                    if matches!(
                        reach,
                        ActionReach::Yes | ActionReach::YesButDisadvantage(..)
                    ) {
                        None
                    } else {
                        Some(OUT_OF_REACH)
                    }
                }
                None => Some("Select an enemy"),
            },

            ConfiguredAction::UseAbility {
                target,
                ability,
                selected_enhancements,
                ..
            } => match target {
                ActionTarget::Character(target_id, movement) => {
                    if let Some(positions) = movement {
                        if positions.is_empty() {
                            return Some("Select movement");
                        }
                    }
                    let target_char = characters.get(*target_id);

                    if relevant_character.reaches_with_ability(
                        *ability,
                        selected_enhancements,
                        target_char.position.get(),
                    ) {
                        None
                    } else {
                        Some(OUT_OF_REACH)
                    }
                }

                ActionTarget::Position(target_pos) => {
                    assert!(matches!(ability.target, AbilityTarget::Area { .. }));
                    if relevant_character.reaches_with_ability(
                        *ability,
                        selected_enhancements,
                        *target_pos,
                    ) {
                        None
                    } else {
                        Some(OUT_OF_REACH)
                    }
                }

                ActionTarget::None => match ability.target {
                    AbilityTarget::None { .. } => None,
                    AbilityTarget::Enemy { .. } => Some("Select an enemy"),
                    AbilityTarget::Ally { .. } => Some("Select an ally"),
                    AbilityTarget::Area { .. } => Some("Select an area"),
                },
            },

            ConfiguredAction::Move {
                selected_movement_path,
                ..
            } => {
                if selected_movement_path.is_empty() {
                    Some("Select movement")
                } else {
                    None
                }
            }

            ConfiguredAction::ChangeEquipment { drag } => {
                if matches!(
                    *drag.borrow(),
                    Some(EquipmentDrag {
                        to_idx: Some(_),
                        ..
                    })
                ) {
                    None
                } else {
                    Some("Drag an item")
                }
            }

            ConfiguredAction::UseConsumable(consumable) => {
                if consumable.is_some() {
                    None
                } else {
                    Some("Select a consumable")
                }
            }
        }
    }

    pub fn from_base_action(base_action: BaseAction) -> Option<Self> {
        match base_action {
            BaseAction::Attack(attack) => Some(Self::Attack {
                attack,
                selected_enhancements: vec![],
                target: None,
            }),
            BaseAction::UseAbility(ability) => Some(Self::UseAbility {
                ability,
                selected_enhancements: vec![],
                target: ActionTarget::None,
            }),
            BaseAction::Move => Some(Self::Move {
                cost: 0,
                selected_movement_path: Default::default(),
            }),
            BaseAction::ChangeEquipment => Some(Self::ChangeEquipment {
                drag: Rc::new(RefCell::new(None)),
            }),
            BaseAction::UseConsumable => Some(Self::UseConsumable(None)),
        }
    }

    pub fn base_action(&self) -> BaseAction {
        match self {
            ConfiguredAction::Attack { attack, .. } => BaseAction::Attack(*attack),
            ConfiguredAction::UseAbility { ability, .. } => BaseAction::UseAbility(*ability),
            ConfiguredAction::Move { .. } => BaseAction::Move,
            ConfiguredAction::ChangeEquipment { .. } => BaseAction::ChangeEquipment,
            ConfiguredAction::UseConsumable { .. } => BaseAction::UseConsumable,
        }
    }

    pub fn base_action_point_cost(&self) -> i32 {
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
            ConfiguredAction::UseAbility {
                ability,
                selected_enhancements,
                ..
            } => {
                let mut ap = ability.action_point_cost;
                for enhancement in selected_enhancements {
                    ap += enhancement.action_point_cost;
                }
                ap
            }
            ConfiguredAction::Move { cost: ap_cost, .. } => *ap_cost,
            ConfiguredAction::ChangeEquipment { .. } => 1,
            ConfiguredAction::UseConsumable { .. } => 1,
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
    tracked_action_buttons: IndexMap<String, Rc<ActionButton>>,
    action_points_row: ActionPointsRow,
    pub hoverable_buttons: Vec<Rc<ActionButton>>,
    actions_section: Container,
    pub character_sheet: CharacterSheet,
    health_bar: Rc<RefCell<LabelledResourceBar>>,
    mana_bar: Rc<RefCell<LabelledResourceBar>>,
    stamina_bar: Rc<RefCell<LabelledResourceBar>>,
    pub resource_bars: Container,
    font: Font,
}

fn resources_mid_x() -> f32 {
    screen_width() / 2.0 - 260.0
}

impl CharacterUi {
    pub fn draw(&self, y: f32) {
        let y0 = y + 5.0;
        self.actions_section.draw(
            screen_width() / 2.0 - self.actions_section.size().0 / 2.0,
            y0,
        );

        self.action_points_row.draw(
            resources_mid_x() - self.action_points_row.size().0 / 2.0,
            screen_height() - 140.0,
        );
        let resource_bars_y = screen_height() - 110.0;
        let resource_bars_x = resources_mid_x() - self.resource_bars.size().0 / 2.0;
        self.resource_bars.draw(resource_bars_x, resource_bars_y);

        let tooltip = if self.action_points_row.hovered.get() {
            Some((
                "Action Points (AP)",
                "Used for actions. 4 AP is restored after ending your turn.",
            ))
        } else if self.health_bar.borrow().bar.borrow().hovered.get() {
            Some((
                "Health |<heart>|",
                "Does not regenerate on its own. (Max |<heart>| is affected by |<stat>Strength|.)",
            ))
        } else if self.stamina_bar.borrow().bar.borrow().hovered.get() {
            Some((
                "Stamina |<stamina>|",
                "Used for movement and certain actions. |<value>25%| is restored after ending your turn. (Max |<stamina>| is affected by |<stat>Strength| and |<stat>Agility|.)"
            ))
        } else if self.mana_bar.borrow().bar.borrow().hovered.get() {
            Some((
                "Mana |<mana>|",
                "Used for spells. Regenerates a small amount after every encounter. (Max |<mana>| is affected by |<stat>Spirit|.)",
            ))
        } else {
            None
        };

        if let Some((header, content)) = tooltip {
            draw_tooltip(
                &self.font,
                TooltipPositionPreference::RelativeToRect(
                    Rect::new(
                        resource_bars_x,
                        resource_bars_y,
                        self.resource_bars.size().0,
                        self.resource_bars.size().1,
                    ),
                    Side::Left,
                ),
                header,
                None,
                &[content.to_string()],
                &[],
                false,
            );
        }
    }
}

pub struct UserInterface {
    characters: Characters,
    event_queue: Rc<RefCell<Vec<InternalUiEvent>>>,
    state: Rc<RefCell<UiState>>,
    animation_stopwatch: StopWatch,

    banner: Banner,

    simple_font: Font,
    big_font: Font,

    hovered_button: Option<ButtonHovered>,
    active_character_id: CharacterId,
    remembered_attack_enhancements: HashMap<CharacterId, Vec<AttackEnhancement>>,

    game_grid: GameGrid,
    activity_popup: ActivityPopup,
    top_character_portraits: TopCharacterPortraits,
    player_portraits: PlayerPortraits,
    character_sheet_toggle: CharacterSheetToggle,
    character_uis: HashMap<CharacterId, CharacterUi>,
    target_ui: TargetUi,
    log: Log,
    sound_player: SoundPlayer,
    status_textures: HashMap<StatusId, Texture2D>,

    faster_movement: Rc<Cell<bool>>,
    settings: Container,
}

impl UserInterface {
    pub fn new(
        game: &CoreGame,
        resources: GameResources,
        ui_resources: UiResources,
        init_state: GameInitState,
        sound_player: SoundPlayer,
    ) -> Self {
        let characters = game.characters.clone();
        let active_character_id = game.active_character_id;

        let event_queue = Rc::new(RefCell::new(vec![]));

        let character_uis = build_character_uis(
            &ui_resources.equipment_icons,
            &ui_resources.icons,
            &event_queue,
            &resources.simple_font,
            characters.iter(),
            resources.status_textures.clone(),
            sound_player.clone(),
        );

        let ui_state = Rc::new(RefCell::new(UiState::Idle));

        let first_player_character_id = characters
            .iter()
            .find(|ch| ch.player_controlled())
            .unwrap()
            .id();

        let mut game_grid = GameGrid::new(
            first_player_character_id,
            characters.as_map(),
            resources.sprites,
            resources.big_font.clone(),
            resources.simple_font.clone(),
            resources.terrain_atlas,
            init_state.pathfind_grid.clone(),
            init_state.background,
            init_state.terrain_objects,
            init_state.decorations,
            resources.status_textures.clone(),
            sound_player.clone(),
        );
        game_grid.auto_tile_terrain_objects();

        let player_portraits = PlayerPortraits::new(
            &characters,
            first_player_character_id,
            active_character_id,
            resources.simple_font.clone(),
            ui_resources.portrait_textures.clone(),
            resources.status_textures.clone(),
            sound_player.clone(),
        );

        let character_sheet_toggle = CharacterSheetToggle {
            shown: Cell::new(false),
            text_line: TextLine::new(
                "Character sheet",
                18,
                WHITE,
                Some(resources.simple_font.clone()),
            ),
            padding: 7.0,
            sound_player: sound_player.clone(),
        };

        let character_portraits = TopCharacterPortraits::new(
            &game.characters,
            game.active_character_id,
            resources.simple_font.clone(),
            //decorative_font.clone(),
            ui_resources.portrait_textures.clone(),
        );

        let target_ui = TargetUi::new(
            resources.big_font.clone(),
            resources.simple_font.clone(),
            ui_resources.icons.clone(),
            resources.status_textures.clone(),
            ui_resources.portrait_textures.clone(),
        );

        let activity_popup = ActivityPopup::new(
            resources.simple_font.clone(),
            ui_state.clone(),
            ui_resources.icons,
            characters.clone(),
            active_character_id,
            sound_player.clone(),
        );

        let mut banner = Banner::new();
        banner.set("Battle!", 2.0);

        let faster_movement = Rc::new(Cell::new(false));

        let settings = build_settings(
            &resources.big_font,
            &resources.simple_font,
            sound_player.clone(),
            faster_movement.clone(),
        );

        Self {
            game_grid,
            characters,
            top_character_portraits: character_portraits,
            player_portraits,
            character_sheet_toggle,
            active_character_id,
            remembered_attack_enhancements: Default::default(),
            animation_stopwatch: StopWatch::default(),

            banner,

            simple_font: resources.simple_font.clone(),
            big_font: resources.big_font.clone(),

            hovered_button: None,
            log: Log::new(resources.simple_font.clone()),
            character_uis,
            event_queue: Rc::clone(&event_queue),
            activity_popup,
            target_ui,
            state: ui_state,
            sound_player,
            status_textures: resources.status_textures,
            faster_movement,
            settings,
        }
    }

    fn set_character_sheet_shown(&self, shown: bool) {
        self.character_sheet_toggle.set_shown(shown);
    }

    pub fn draw(&mut self) -> Option<PlayerChose> {
        let ui_y = screen_height() - 230.0;
        let ui_x0 = screen_width() / 2.0 - 350.0;
        let ui_x1 = screen_width() / 2.0 + 350.0;

        let popup_rect = self.activity_popup.last_drawn_rectangle;
        let target_ui_rect = self.target_ui.last_drawn_rectangle.get();
        let settings_rect = self.settings.last_drawn_rectangle.get();

        let mouse_pos = mouse_position();
        let is_grid_obstructed = popup_rect.contains(mouse_pos.into())
            || target_ui_rect.contains(mouse_pos.into())
            || settings_rect.contains(mouse_pos.into())
            || self.character_sheet_toggle.is_shown()
            || (mouse_pos.1 >= ui_y - 1.0 && mouse_pos.0 >= ui_x0 - 1.0);
        let is_grid_receptive_to_dragging = !is_grid_obstructed;

        let mut hovered_action = None;

        if let Some(btn) = self.hovered_button.as_ref() {
            if let ButtonAction::Action(base_action) = btn.action {
                let selected_char_id = self.player_portraits.selected_id();
                // Don't allow "hovering" action of dead character as there may be assumptions in rendering code
                // that the character exists physically on the grid
                if self.characters.contains_alive(selected_char_id) {
                    hovered_action = Some((selected_char_id, base_action));
                }
            }
        }

        if hovered_action.is_none() {
            if let Some(ButtonAction::Action(base_action)) = self.target_ui.hovered_action() {
                let target_char_id = self.target_ui.get_character_id().unwrap();
                hovered_action = Some((target_char_id, base_action));
            }
        }

        let character_ui = self
            .character_uis
            .get_mut(&self.player_portraits.selected_id())
            .unwrap();

        let grid_outcome = self.game_grid.draw(
            is_grid_receptive_to_dragging,
            &mut self.state.borrow_mut(),
            is_grid_obstructed,
            hovered_action,
            character_ui.action_points_row.reserved_and_hovered_ap,
        );

        let mut player_chose = self.handle_grid_outcome(grid_outcome);

        draw_rectangle(
            ui_x0,
            ui_y + 5.0,
            ui_x1 - ui_x0,
            screen_height() - ui_y,
            BLACK,
        );
        //draw_line(ui_x0, ui_y, ui_x1, ui_y, 1.0, ORANGE);
        //draw_line(ui_x0, ui_y, ui_x0, screen_height(), 1.0, ORANGE);
        //draw_line(ui_x1, ui_y, ui_x1, screen_height(), 1.0, ORANGE);

        draw_texture(UI_TEXTURE.get().unwrap(), ui_x0 - 10.0, ui_y - 10.0, WHITE);

        self.activity_popup.draw(570.0, ui_y + 1.0);

        let may_show_end_turn_button = matches!(
            &*self.state.borrow(),
            UiState::ChoosingAction | UiState::ConfiguringAction(..)
        );
        let player_portraits_outcome = self.player_portraits.draw(
            screen_width() / 2.0 - 120.0,
            screen_height() - 120.0,
            may_show_end_turn_button,
        );
        let mut changed_character = player_portraits_outcome.changed_character;
        self.character_sheet_toggle.draw(
            resources_mid_x() - self.character_sheet_toggle.size().0 / 2.0,
            screen_height() - 35.0,
        );

        let clicked_character_id = self.top_character_portraits.draw(0.0, 0.0);
        if let Some(id) = clicked_character_id {
            if self.characters.get(id).player_controlled() {
                self.sound_player.play(SoundId::ClickButton);
                self.player_portraits.set_selected_id(id);
                changed_character = true;
            }
        }

        let selected_character_id = self.player_portraits.selected_id();
        if changed_character {
            self.on_new_selected_character();
            if matches!(*self.state.borrow(), UiState::ConfiguringAction(..)) {
                self.set_state(UiState::ChoosingAction);
            }

            let new_selected_char = self.characters.get(selected_character_id);

            if selected_character_id != self.active_character_id
                && matches!(*self.state.borrow(), UiState::ChoosingAction)
                && !new_selected_char.has_taken_a_turn_this_round.get()
            {
                if player_chose.is_some() {
                    println!(
                        "Warning: overriding {:?} with new choice (switch char)",
                        player_chose
                    );
                }

                player_chose = Some(PlayerChose::SwitchTo(selected_character_id));
            }
        }
        if player_portraits_outcome.clicked_end_turn
            && matches!(
                *self.state.borrow(),
                UiState::ChoosingAction | UiState::ConfiguringAction(..)
            )
        {
            if player_chose.is_some() {
                println!(
                    "Warning: overriding {:?} with new choice (end turn)",
                    player_chose
                );
            }
            self.sound_player.play(SoundId::ClickButton);
            player_chose = Some(PlayerChose::Action(None));
        }

        let character_ui = self.character_uis.get_mut(&selected_character_id).unwrap();

        character_ui.draw(ui_y + 5.0);

        let log_x = screen_width() - self.log.width();
        self.log.draw(log_x, ui_y);

        self.log.draw_tooltips(log_x, ui_y);

        self.target_ui
            .draw(screen_width() - self.target_ui.size().0 - 10.0, 10.0);

        if self.character_sheet_toggle.is_shown() {
            let is_showing_active = self.active_character_id == selected_character_id;

            let outcome = character_ui
                .character_sheet
                .draw(&mut self.state.borrow_mut(), is_showing_active);

            self.set_character_sheet_shown(!outcome.clicked_close);

            if outcome.changed_state {
                println!("REQUESTED EQ CHANGE; new state");
                // Maybe drag was changed, or maybe the entire state; should be fine to assume the latter
                self.on_new_state();
            }
        } else {
            // Some actions don't make sense to show unless the character sheet is shown
            let mut should_cancel_action = false;
            if matches!(
                *self.state.borrow(),
                UiState::ConfiguringAction(ConfiguredAction::UseConsumable(None))
            ) {
                should_cancel_action = true;
            } else if let UiState::ConfiguringAction(ConfiguredAction::ChangeEquipment { drag }) =
                &*self.state.borrow()
            {
                if drag.borrow().is_none() {
                    should_cancel_action = true;
                }
            }
            if should_cancel_action {
                self.set_state(UiState::ChoosingAction);
            }
        }

        let character_ui = self
            .character_uis
            .get_mut(&self.player_portraits.selected_id())
            .unwrap();

        if let Some(btn_hovered) = &self.hovered_button {
            if let Some(btn) = character_ui
                .hoverable_buttons
                .iter()
                .find(|btn| btn.id == btn_hovered.id)
            {
                let detailed_tooltip = is_key_down(KeyCode::LeftAlt);
                draw_button_tooltip(
                    &self.simple_font,
                    btn_hovered.hovered_pos.unwrap(),
                    &btn.tooltip(),
                    detailed_tooltip,
                );
            }
        }

        self.banner.draw(&self.big_font);

        let h = self.settings.size().1;
        self.settings.draw(0.0, screen_height() - h);

        player_chose
    }

    fn handle_grid_outcome(&mut self, outcome: GridOutcome) -> Option<PlayerChose> {
        let mut player_chose = None;
        self.top_character_portraits
            .set_grid_hovered_character_id(outcome.hovered_character_id);

        if let Some(new_inspect_target) = outcome.switched_inspect_target {
            let target = new_inspect_target.map(|id| self.characters.get_rc(id));
            self.target_ui.set_character(target);
        }

        if let Some(new_selected_player_char) = outcome.tried_switching_selected_player_char {
            if matches!(*self.state.borrow(), UiState::ChoosingAction)
                && !self
                    .characters
                    .get(new_selected_player_char)
                    .has_taken_a_turn_this_round
                    .get()
            {
                self.sound_player.play(SoundId::ClickButton);
                return Some(PlayerChose::SwitchTo(new_selected_player_char));
            } else {
                self.set_selected_character(new_selected_player_char);
            }
        }

        if let Some(grid_switched_to) = outcome.switched_state {
            self.on_new_state();
            if let NewState::Move { commit_movement } = grid_switched_to {
                if commit_movement {
                    let UiState::ConfiguringAction(ConfiguredAction::Move {
                        selected_movement_path,
                        cost,
                    }) = &*self.state.borrow()
                    else {
                        unreachable!()
                    };
                    let dst = selected_movement_path.last().unwrap();
                    let total_distance = dst.0;
                    let positions = selected_movement_path
                        .iter()
                        .map(|(_dist_from_start, pos)| *pos)
                        .collect();

                    player_chose = Some(PlayerChose::Action(Some(Action::Move {
                        total_distance,
                        positions,
                        extra_cost: *cost,
                    })));
                }
                self.activity_popup.on_new_movement_ap_cost();
            }
        }

        if outcome.switched_players_action_target {
            if let UiState::ConfiguringAction(configured_action) = &*self.state.borrow() {
                if configured_action.has_target() {
                    self.sound_player.play(SoundId::SelectTarget);
                }
            }

            self.refresh_target_state();
        }

        /*
        if outcome.switched_movement_path {
            println!("GRID SWITCHED MOVE PATH");

            self.refresh_movement_state();
        }
        */

        if let Some(move_cost) = &outcome.hovered_move_path_cost {
            let switched_state_from_movement = matches!(
                outcome.switched_state,
                Some(NewState::Attack | NewState::ChoosingAction)
            );
            if !switched_state_from_movement {
                self.activity_popup.set_movement_cost(*move_cost);
            }
        }

        player_chose
    }

    fn refresh_movement_state(&mut self) {
        if let UiState::ConfiguringAction(ConfiguredAction::Move {
            selected_movement_path,
            ..
        }) = &*self.state.borrow()
        {
            self.activity_popup.on_new_movement_ap_cost();

            /*
            if !selected_movement_path.is_empty() {
                self.target_ui.clear_action();
            } else {
                self.target_ui
                    .set_action("Select a destination".to_string(), vec![], false);
            }
             */

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
            UiState::ConfiguringAction(ConfiguredAction::UseAbility { .. })
        ) {
            self.refresh_use_ability_state();
        }
    }

    fn refresh_attack_state(&mut self) {
        let UiState::ConfiguringAction(ConfiguredAction::Attack {
            attack,
            selected_enhancements,
            target,
        }) = &*self.state.borrow()
        else {
            unreachable!()
        };

        self.remembered_attack_enhancements
            .insert(self.active_character_id, selected_enhancements.clone());

        match target {
            Some(target_id) => {
                let target_char = self.characters.get(*target_id);

                let (_range, reach) = self.active_character().reaches_with_attack(
                    attack.hand,
                    target_char.position.get(),
                    selected_enhancements.iter().map(|e| e.effect),
                );

                let mut details: Vec<(&'static str, Goodness)> = vec![];

                if matches!(reach, ActionReach::No) {
                    details.push(("Out of reach!", Goodness::Bad));
                }

                let selected_enhancement_effects: Vec<(&'static str, AttackEnhancementEffect)> =
                    selected_enhancements
                        .iter()
                        .map(|e| (e.name, e.effect))
                        .collect();

                let prediction = predict_attack(
                    &self.characters,
                    self.characters.get_rc(self.active_character_id),
                    attack.hand,
                    &selected_enhancement_effects,
                    target_char,
                    None,
                    0,
                );

                for (term, bonus) in self.active_character().outgoing_attack_bonuses(
                    attack.hand,
                    &selected_enhancement_effects,
                    target_char,
                ) {
                    details.push((term, bonus.goodness()));
                }
                for (term, bonus) in target_char.incoming_attack_bonuses(None) {
                    details.push((term, bonus.goodness()));
                }

                let header = format!(
                    "|<sword>| {}-{}",
                    prediction.min_damage, prediction.max_damage,
                );

                self.game_grid.clear_target_damage_previews();
                self.game_grid
                    .set_target_effect_preview(TargetEffectPreview {
                        character_id: *target_id,
                        prediction: TargetPrediction::from(prediction),
                    });

                //self.target_ui.set_action(header, details, true);
            }

            None => {
                self.game_grid.clear_target_damage_previews();
                //self.target_ui
                //.set_action("Select an enemy".to_string(), vec![], false);
            }
        }

        //self.activity_popup.refresh_enabled_state();
    }

    fn refresh_use_ability_state(&mut self) {
        let UiState::ConfiguringAction(configured_action) = &*self.state.borrow() else {
            panic!()
        };

        let ConfiguredAction::UseAbility {
            ability,
            selected_enhancements,
            target,
        } = configured_action
        else {
            panic!()
        };

        let ability = *ability;

        println!("REFRESH CAST_ABILITY STATE : {}", ability.name);

        /*
        let mut details = vec![];
        match target {
            ActionTarget::Character(target_id, movement, ..) => {
                let target_char = self.characters.get(*target_id);

                if !self.active_character().reaches_with_ability(
                    ability,
                    selected_enhancements,
                    target_char.pos(),
                ) {
                    details.push(("Out of reach!".to_string(), Goodness::Bad));
                }

                if let Some(movement) = movement {
                    if movement.is_empty() {
                        details.push(("No valid path!".to_string(), Goodness::Bad));
                    }
                }

                if let Some(ability_roll) = ability.roll {
                    // TODO For attack-based abilities, these details SHOULD use attack rules, and not ability rules (?)
                    // For example, the below probably doesn't account correctly for flanking?
                    for (term, bonus) in self
                        .active_character()
                        .outgoing_ability_roll_bonuses(selected_enhancements, ability_roll)
                    {
                        details.push((term.to_string(), bonus.goodness()));
                    }
                    for (term, bonus) in target_char.incoming_ability_bonuses() {
                        details.push((term.to_string(), bonus.goodness()));
                    }
                }

                let action_text = match ability.target {
                    AbilityTarget::Enemy { .. } | AbilityTarget::Ally { .. } => {
                        ability.name.to_string()
                    }
                    AbilityTarget::None { .. } | AbilityTarget::Area { .. } => {
                        unreachable!()
                    }
                };

                self.target_ui.set_action(action_text, details.clone(), true);
            }

        }
         */

        self.game_grid.clear_target_damage_previews();

        let usability_problem =
            configured_action.usability_problem(self.active_character(), &self.characters);

        if matches!(usability_problem, None | Some(OUT_OF_REACH)) {
            let prediction = predict_ability(
                &self.characters,
                self.characters.get_rc(self.active_character_id),
                ability,
                selected_enhancements,
                target,
            );

            for (target_id, prediction) in prediction.targets {
                self.game_grid
                    .set_target_effect_preview(TargetEffectPreview {
                        character_id: target_id,
                        prediction,
                    });
            }
        }
    }

    fn set_allowed_to_use_action_buttons(&self, allowed: bool) {
        for btn in self.character_uis[&self.player_portraits.selected_id()]
            .tracked_action_buttons
            .values()
        {
            let base_action = match btn.action {
                ButtonAction::Action(base_action) => base_action,
                _ => unreachable!(),
            };

            btn.set_has_enough_ap(
                self.active_character()
                    .has_enough_ap_for_action(base_action),
            );

            if allowed {
                btn.enabled
                    .set(self.active_character().can_use_action(base_action));
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
                .usability_problem(self.active_character(), &self.characters)
                .is_none();
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

        if self.active_character().player_controlled() {
            if self.player_portraits.selected_id() != self.active_character_id {
                println!("SWITCHING CHAR IN PORTRAITS");
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
        if let UiState::ConfiguringAction(configured_action) = &*self.state.borrow() {
            match configured_action {
                ConfiguredAction::ChangeEquipment { drag } => {
                    // Clear the drag; it's shared with the character sheet equipment UI
                    *drag.borrow_mut() = None;
                    self.character_sheet_toggle.set_shown(false);
                }
                ConfiguredAction::UseConsumable(..) => {
                    self.character_sheet_toggle.set_shown(false);
                }
                _ => {}
            }
        }

        *self.state.borrow_mut() = state;

        self.on_new_state();
    }

    fn on_new_state(&mut self) {
        dbg!(&self.state.borrow());
        println!("^^^ on_new_state() ^^^");

        self.activity_popup.additional_line = None;

        self.game_grid.clear_target_damage_previews();

        let mut relevant_action_button = None;

        let mut is_reacting = None;
        let mut is_reacting_to_attack = false;

        self.sound_player.stop(SoundId::FireCrackle);
        self.sound_player.stop(SoundId::MechanicNoise);

        match &mut *self.state.borrow_mut() {
            UiState::ConfiguringAction(ref mut configured_action) => {
                self.set_allowed_to_use_action_buttons(true);

                if let ConfiguredAction::UseAbility { ability, .. } = configured_action {
                    if ability.id == AbilityId::Fireball {
                        self.sound_player.play_looping(SoundId::FireCrackle);
                    } else {
                        self.sound_player.play_looping(SoundId::MechanicNoise);
                    }
                }

                relevant_action_button = self.character_uis[&self.active_character_id]
                    .tracked_action_buttons
                    .get(&button_action_id(ButtonAction::Action(
                        configured_action.base_action(),
                    )))
                    .cloned();
                assert!(relevant_action_button.is_some(), "{:?}", configured_action);

                if let ConfiguredAction::Attack {
                    selected_enhancements,
                    attack,
                    ..
                } = configured_action
                {
                    let usable = self
                        .active_character()
                        .usable_attack_enhancements(attack.hand);

                    if let Some(remembered) = self
                        .remembered_attack_enhancements
                        .get_mut(&self.active_character_id)
                    {
                        // Forget enhancements that are no longer usable (e.g. due to lack of resources)
                        remembered.retain(|e| usable.contains(e));
                        *selected_enhancements = remembered.clone();
                    }
                } else if let ConfiguredAction::Move { .. } = configured_action {
                    //movement_cost = *cost;
                }
            }

            UiState::ReactingToAttack {
                hand,
                attacker,
                defender,
                reactor,
                is_within_melee,
                selected,
            } => {
                is_reacting = Some(*reactor);
                is_reacting_to_attack = true;
            }

            UiState::ReactingToHit { victim, .. } => {
                is_reacting = Some(*victim);
            }

            UiState::ReactingToMovementAttackOpportunity { reactor, .. } => {
                is_reacting = Some(*reactor);
            }

            UiState::ReactingToRangedAttackOpportunity { reactor, .. } => {
                is_reacting = Some(*reactor);
            }

            UiState::ChoosingAction => {
                /*
                self.target_ui
                    .set_action("Select an action".to_string(), vec![], false);
                */

                self.set_allowed_to_use_action_buttons(true);
            }

            UiState::Idle => {
                //self.target_ui.clear_action();
                self.set_allowed_to_use_action_buttons(false);
            }
        }

        if let Some(reactor) = is_reacting {
            //self.target_ui
            //    .set_action("Reaction!".to_string(), vec![], false);
            self.set_allowed_to_use_action_buttons(false);
            self.player_portraits.set_selected_id(reactor);

            if is_reacting_to_attack {
                self.activity_popup.refresh_on_attacked_state();
                self.refresh_reaction_state();
            }
        }

        self.player_portraits.set_reacting_character(is_reacting);

        self.activity_popup
            .on_new_state(self.active_character_id, relevant_action_button);

        self.maybe_refresh_equipment_state();

        self.refresh_target_state();
        self.refresh_movement_state();

        self.game_grid.update_move_speed(self.active_character_id);

        self.refresh_selected_action_button();
        self.target_ui.rebuild_character_ui();
    }

    fn refresh_reaction_state(&mut self) {
        let UiState::ReactingToAttack {
            hand,
            attacker,
            defender,
            reactor,
            is_within_melee,
            selected,
        } = &*self.state.borrow()
        else {
            unreachable!()
        };
        let attacker = self.characters.get_rc(*attacker);
        let defender = self.characters.get(*defender);

        dbg!(&selected);
        let prediction = predict_attack(
            &self.characters,
            attacker,
            *hand,
            &[],
            defender,
            selected.map(|r| (*reactor, r)),
            0,
        );
        dbg!(&prediction);
        self.game_grid
            .set_target_effect_preview(TargetEffectPreview {
                character_id: defender.id(),
                prediction: TargetPrediction::from(prediction),
            });
    }

    fn maybe_refresh_equipment_state(&mut self) {
        match &*self.state.borrow() {
            UiState::ConfiguringAction(ConfiguredAction::ChangeEquipment { drag }) => {
                //self.target_ui
                //.set_action("Change equipment".to_string(), vec![], false);

                if let Some(
                    drag @ EquipmentDrag {
                        to_idx: Some(_), ..
                    },
                ) = *drag.borrow()
                {
                    let description = self.character_uis[&self.active_character_id]
                        .character_sheet
                        .describe_requested_equipment_change(drag);
                    self.activity_popup.additional_line = Some(description);
                } else {
                    self.activity_popup.additional_line =
                        Some("Drag something in your inventory to equip/unequip it".to_string());
                    self.set_character_sheet_shown(true);
                }
            }

            UiState::ConfiguringAction(ConfiguredAction::UseConsumable(consumption)) => {
                //self.target_ui
                //  .set_action("Use consumable".to_string(), vec![], false);

                if let Some(consumption) = consumption {
                    self.activity_popup.additional_line =
                        Some(format!("Use {}", consumption.consumable.name));
                } else {
                    self.activity_popup.additional_line = Some(
                        "Select a consumable in your inventory by right-clicking it".to_string(),
                    );
                    self.set_character_sheet_shown(true);
                }
            }

            _ => {}
        };
    }

    pub fn has_ongoing_animation(&self) -> bool {
        self.animation_stopwatch.remaining.is_some()
    }

    pub fn handle_game_event(&mut self, event: Box<GameEvent>) {
        self.target_ui.rebuild_character_ui();

        dbg!(&event);

        match *event {
            GameEvent::LogLine(line) => {
                self.log.add(line);
            }
            GameEvent::CharacterReactedToAttacked {
                reactor,
                with_shield,
            } => {
                let reactor_pos = self.characters.get(reactor).pos();
                self.game_grid.add_text_effect(
                    reactor_pos,
                    0.0,
                    0.5,
                    None,
                    "!".to_string(),
                    TextEffectStyle::ReactionExclamation,
                );

                self.game_grid
                    .animate_character_acting(reactor, with_shield, 0.8);

                self.animation_stopwatch.set_to_at_least(0.4);
            }
            GameEvent::CharacterReactedWithOpportunityAttack { reactor } => {
                let reactor = self.characters.get(reactor);
                self.log.add("Opportunity attack:".to_string());
                self.game_grid.add_text_effect(
                    reactor.pos(),
                    0.0,
                    0.5,
                    None,
                    "!".to_string(),
                    TextEffectStyle::ReactionExclamation,
                );

                self.animation_stopwatch.set_to_at_least(0.4);
            }
            GameEvent::CharacterReactedToHit {
                main_line,
                detail_lines,
                reactor,
                outcome,
            } => {
                self.log.add_with_details(main_line, &detail_lines);

                let reactor_pos = self.characters.get(reactor).pos();

                if let Some(condition) = outcome.received_condition {
                    self.game_grid.add_text_effect(
                        reactor_pos,
                        0.0,
                        1.0,
                        None,
                        condition.name().to_string(),
                        TextEffectStyle::HostileHit,
                    );
                }

                let attacker_pos = self.active_character().pos();
                if let Some(offensive) = outcome.offensive {
                    if let Some((condition, _duration)) = offensive.inflicted_condition {
                        self.game_grid.add_text_effect(
                            attacker_pos,
                            0.0,
                            1.0,
                            None,
                            condition.name().to_string(),
                            TextEffectStyle::HostileHit,
                        );
                    } else {
                        self.game_grid.add_text_effect(
                            attacker_pos,
                            0.0,
                            1.0,
                            None,
                            "Miss".to_string(),
                            TextEffectStyle::Miss,
                        );
                    }
                }
                self.animation_stopwatch.set_to_at_least(0.5);
            }
            GameEvent::AttackWasInitiated { actor, target } => {
                self.handle_attack_initiated(actor, target);
            }
            GameEvent::Attacked(event) => {
                self.handle_attacked_event(&event);
            }
            GameEvent::AbilityWasInitiated {
                actor,
                ability,
                target,
                area_at,
            } => {
                if let Some(sound_id) = ability.initiate_sound {
                    self.sound_player.play(sound_id);
                }

                let with_shield = ability.id == AbilityId::ShieldBash;
                self.game_grid
                    .animate_character_acting(actor, with_shield, 0.3);

                let duration;

                let animation_color = ability.animation_color;
                let caster_pos = self.characters.get(actor).pos();
                if let Some(target) = &target {
                    let target_pos = self.characters.get(*target).pos();

                    duration = 0.04 * distance_between(caster_pos, target_pos);

                    self.add_circle_projectile_effect(
                        0.1,
                        duration,
                        animation_color,
                        caster_pos,
                        target_pos,
                    );
                } else if let Some((shape, area_pos)) = area_at {
                    duration = 0.05 * distance_between(caster_pos, area_pos);
                    match shape {
                        AreaShape::Circle(range) => {
                            self.add_circle_projectile_effect(
                                0.0,
                                duration,
                                animation_color,
                                caster_pos,
                                area_pos,
                            );
                        }
                        AreaShape::Line => {
                            self.game_grid.add_effect(
                                caster_pos,
                                area_pos,
                                Effect {
                                    start_time: 0.0,
                                    end_time: duration,
                                    variant: EffectVariant::Line {
                                        color: animation_color,
                                        thickness: 10.0,
                                        end_thickness: None,
                                        extend_gradually: true,
                                    },
                                },
                            );
                        }
                    }
                } else {
                    duration = 0.3;
                }
                self.animation_stopwatch.set_to_at_least(duration + 0.1);
            }
            GameEvent::AbilityResolved(AbilityResolvedEvent {
                actor,
                target_outcome,
                area_outcome,
                ability,
                mut detail_lines,
            }) => {
                if let Some(sound_id) = ability.initiate_sound {
                    self.sound_player.stop(sound_id);
                }

                let actor_name = self.characters.get(actor).name;
                let verb = if matches!(ability.roll, Some(AbilityRollType::Spell)) {
                    "cast"
                } else {
                    "used"
                };
                let mut line = format!("|{}| {} {}", actor_name, verb, ability.name);
                if let Some((target_id, _outcome)) = &target_outcome {
                    let target_name = self.characters.get(*target_id).name;
                    line.push_str(&format!(" on |{}|", target_name));
                }

                let mut attacks = vec![];

                let mut resist = false;
                if let Some((target, outcome)) = &target_outcome {
                    match outcome {
                        AbilityTargetOutcome::HitEnemy {
                            damage,
                            graze,
                            applied_effects,
                        } => {
                            if let Some(dmg) = damage {
                                line.push_str(&format!(" ({} damage)", dmg))
                            } else if applied_effects.is_empty() {
                                if *graze {
                                    line.push_str(" (graze)");
                                } else {
                                    line.push_str(" (hit)");
                                }
                            } else if applied_effects.len() == 1 {
                                line.push_str(&format!("  ({})", applied_effects[0]));
                            }
                        }
                        AbilityTargetOutcome::Resisted => {
                            resist = true;
                            line.push_str(" (miss)");
                        }
                        AbilityTargetOutcome::AffectedAlly { applied_effects } => {
                            if applied_effects.len() == 1 {
                                line.push_str(&format!("  ({})", applied_effects[0]));
                            }
                        }
                        AbilityTargetOutcome::AttackedEnemy(event) => {
                            attacks.push(event);
                        }
                    }
                }

                if let Some(sound_id) = ability.resolve_sound {
                    if resist {
                        self.sound_player.play(SoundId::Resist);
                    } else {
                        self.sound_player.play(sound_id);
                    }
                }

                if let Some(AbilityAreaOutcome { targets, .. }) = &area_outcome {
                    for (_, outcome) in targets {
                        if let AbilityTargetOutcome::AttackedEnemy(attacked_event) = &outcome {
                            attacks.push(attacked_event);
                        }
                    }
                }

                if !attacks.is_empty() {
                    // The provided details are misleading; they report the dice-roll used when performing the ability, but that
                    // dice roll is effectively ignored since the ability instead proceeded to perform an attack (which uses
                    // its own dice roll)
                    detail_lines.clear();
                    if attacks.len() == 1 {
                        line.push_str(" (1 target)");
                    } else {
                        line.push_str(&format!(" ({} targets)", attacks.len()));
                    }
                }

                self.log.add_with_details(line, &detail_lines);

                let animation_color = ability.animation_color;
                if let Some((target, outcome)) = &target_outcome {
                    let target_pos = self.characters.get(*target).pos();
                    self.game_grid.animate_character_shaking(*target, 0.2);
                    self.add_effect_for_ability_target_outcome(outcome, 0.0, *target, target_pos);
                    self.animation_stopwatch.set_to_at_least(0.3);
                }

                if let Some(AbilityAreaOutcome {
                    center,
                    targets,
                    shape,
                }) = &area_outcome
                {
                    self.add_effects_for_area_outcomes(
                        0.0,
                        animation_color,
                        &center,
                        Some(*shape),
                        targets,
                    );
                }

                for event in attacks {
                    self.handle_attacked_event(event);
                }
            }
            GameEvent::ConsumableWasUsed {
                user,
                consumable,
                detail_lines,
            } => {
                self.sound_player.play(SoundId::Powerup);
                self.log.add_with_details(
                    format!(
                        "|{}| used {}",
                        self.characters.get(user).name,
                        consumable.name
                    ),
                    &detail_lines,
                );
            }
            GameEvent::CharactersDying { characters } => {
                let duration = 0.5;
                self.sound_player.play(SoundId::Death);
                for char_id in characters {
                    self.game_grid.animate_death(char_id, duration);
                    self.animation_stopwatch.set_to_at_least(duration);
                }
            }
            GameEvent::CharactersDied {
                characters,
                new_active,
            } => {
                for char_id in characters {
                    self.log
                        .add(format!("|{}| died", self.characters.get(char_id).name));
                }

                self.target_ui.clear_character_if_dead();

                self.game_grid.remove_dead();
                self.top_character_portraits.remove_dead();

                // TODO: Ideally the UI shouldn't show a new active character until the death "animation" is complete.
                self.animation_stopwatch.set_to_at_least(0.5);

                if let Some(new_active) = new_active {
                    self.set_new_active_character_id(new_active);
                }
            }
            GameEvent::PlayerCharacterEndedTheirTurn {
                gained_ap,
                gained_stamina: _,
            } => {
                let character_ui = self
                    .character_uis
                    .get_mut(&self.active_character_id)
                    .unwrap();
                let duration = gained_ap as f32 * 0.07;
                character_ui
                    .action_points_row
                    .animate_gain(gained_ap, duration);

                character_ui.stamina_bar.borrow_mut().animate_gain(duration);

                self.animation_stopwatch.set_to_at_least(duration + 0.2);
            }
            GameEvent::NewActiveCharacter { new_active } => {
                let was_players_turn = self.active_character().player_controlled();
                self.set_new_active_character_id(new_active);
                if self.active_character().player_controlled() {
                    if !was_players_turn {
                        self.banner.set("Your turn", 1.5);
                        self.sound_player.play(SoundId::YourTurn);
                    }
                } else {
                    if was_players_turn {
                        self.banner.set("Enemy turn", 1.5);
                    }
                    self.animation_stopwatch.set_to_at_least(0.6);
                }
            }
            GameEvent::CharacterReceivedKnockback { character } => {
                /*
                let char = self.characters.get(character);
                self.game_grid.add_text_effect(
                    char.pos(),
                    0.0,
                    1.5,
                    None,
                    "Knockback",
                    TextEffectStyle::HostileHit,
                );
                 */
            }
            GameEvent::CharacterGainedAP { character } => {
                let char = self.characters.get(character);
                self.game_grid.add_text_effect(
                    char.pos(),
                    0.0,
                    1.5,
                    None,
                    "+AP",
                    TextEffectStyle::FriendlyEffect,
                );
            }
            GameEvent::Moved {
                character,
                from,
                to,
                movement_type,
                step_idx,
            } => {
                let mut duration = if self.faster_movement.get() {
                    0.07
                } else {
                    0.11
                };

                if from.0 != to.0 || from.1 != to.1 {
                    // diagonal takes longer
                    duration *= 1.41;
                }

                self.game_grid
                    .set_character_motion(character, from, to, duration, movement_type);
                if movement_type != MovementType::KnockedBack && step_idx % 2 == 0 {
                    self.sound_player.play(SoundId::Walk);
                }
                self.animation_stopwatch.set_to_at_least(duration);
            }
            GameEvent::CharacterTookDamage {
                character,
                amount,
                source,
            } => {
                let character = self.characters.get(character);
                self.log.add(format!(
                    "|{}| took {} damage from {}",
                    character.name,
                    amount,
                    source.name()
                ));
                if source == DamageSource::Condition(Condition::Burning) {
                    self.sound_player.play(SoundId::Burning);
                }

                self.game_grid.add_text_effect(
                    character.pos(),
                    0.0,
                    1.5,
                    None,
                    format!("{}", amount),
                    TextEffectStyle::HostileHit,
                );
                self.animate_character_damage(character.id(), amount);
            }
            GameEvent::CharacterReceivedCondition {
                character,
                condition,
            } => {
                let character = self.characters.get(character);
                let texture = self
                    .status_textures
                    .get(&condition.status_icon())
                    .unwrap()
                    .clone();
                self.game_grid.add_text_effect(
                    character.pos(),
                    0.0,
                    2.0,
                    Some(texture),
                    condition.name().to_string(),
                    TextEffectStyle::HostileEffect,
                );
            }
        }
    }

    fn add_circle_projectile_effect(
        &mut self,
        start_time: f32,
        duration: f32,
        animation_color: Color,
        caster_pos: (i32, i32),
        target_pos: (i32, i32),
    ) {
        self.game_grid.add_effect(
            caster_pos,
            target_pos,
            Effect {
                start_time: start_time,
                end_time: start_time + duration,
                variant: EffectVariant::At(
                    EffectPosition::Projectile,
                    EffectGraphics::Circle {
                        radius: 10.0,
                        end_radius: None,
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
                start_time: start_time + 0.025,
                end_time: start_time + duration,
                variant: EffectVariant::At(
                    EffectPosition::Projectile,
                    EffectGraphics::Circle {
                        radius: 8.0,
                        end_radius: None,
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
                start_time: start_time + 0.05,
                end_time: start_time + duration,
                variant: EffectVariant::At(
                    EffectPosition::Projectile,
                    EffectGraphics::Circle {
                        radius: 6.0,
                        end_radius: None,
                        fill: Some(animation_color),
                        stroke: None,
                    },
                ),
            },
        );
    }

    fn animate_character_damage(&mut self, character_id: CharacterId, actual_health_lost: u32) {
        if actual_health_lost > 0 {
            let prev_health =
                self.characters.get(character_id).health.current() + actual_health_lost;
            self.game_grid
                .animate_character_health_change(character_id, prev_health, 0.9);
        }
    }

    fn add_effects_for_area_outcomes(
        &mut self,
        start_time: f32,
        animation_color: Color,
        area_center_pos: &(i32, i32),
        shape: Option<AreaShape>,
        outcomes: &[(CharacterId, AbilityTargetOutcome)],
    ) {
        let area_duration = 0.2;

        if let Some(AreaShape::Circle(radius)) = shape {
            self.game_grid.add_effect(
                *area_center_pos,
                *area_center_pos,
                Effect {
                    start_time,
                    end_time: start_time + area_duration,
                    variant: EffectVariant::At(
                        EffectPosition::Destination,
                        EffectGraphics::Circle {
                            radius: f32::from(radius) * self.game_grid.cell_w,
                            end_radius: None,
                            fill: Some(Color::new(1.0, 1.0, 1.0, 0.3)),
                            stroke: None,
                        },
                    ),
                },
            );
        }

        let mut delay = 0.1;
        for (target_id, outcome) in outcomes {
            let target_pos = self.characters.get(*target_id).pos();

            let start = start_time + delay;
            let end = start + 0.2;

            self.game_grid.add_effect(
                (area_center_pos.0, area_center_pos.1),
                target_pos,
                Effect {
                    start_time: start,
                    end_time: end,
                    variant: EffectVariant::At(
                        EffectPosition::Destination,
                        EffectGraphics::Circle {
                            radius: 20.0,
                            end_radius: Some(25.0),
                            fill: Some(Color::new(0.0, 0.0, 0.0, 0.3)),
                            stroke: Some((animation_color, 4.0)),
                        },
                    ),
                },
            );

            self.game_grid.animate_character_shaking(*target_id, 0.2);

            self.add_effect_for_ability_target_outcome(outcome, start, *target_id, target_pos);

            self.animation_stopwatch.set_to_at_least(start_time + 0.3);

            delay += 0.1;
        }
    }

    fn handle_attack_initiated(&mut self, attacker: CharacterId, target: CharacterId) {
        if self.characters.get(attacker).has_equipped_ranged_weapon() {
            self.sound_player.play(SoundId::ShootArrow);
        }

        let attacker_pos = self.characters.get(attacker).pos();
        let target_pos = self.characters.get(target).pos();

        let projectile_duration = (0.03 * distance_between(attacker_pos, target_pos)).max(0.3);

        self.game_grid
            .animate_character_acting(attacker, false, projectile_duration.max(0.2));

        self.game_grid.add_effect(
            attacker_pos,
            target_pos,
            Effect {
                start_time: 0.0,
                end_time: projectile_duration,
                variant: EffectVariant::Line {
                    thickness: 1.0,
                    end_thickness: Some(4.0),
                    color: RED,
                    extend_gradually: true,
                },
            },
        );
        self.game_grid.add_effect(
            attacker_pos,
            target_pos,
            Effect {
                start_time: projectile_duration,
                end_time: projectile_duration + 0.2,
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

        self.animation_stopwatch
            .set_to_at_least(projectile_duration);
    }

    fn handle_attacked_event(&mut self, event: &AttackedEvent) {
        let attacker = event.attacker;
        let target = event.target;
        let detail_lines = &event.detail_lines;

        let mut applied_effects = vec![];

        let verb;
        match &event.outcome {
            AttackOutcome::Hit {
                hit_type,
                damage,
                applied_effects: effects,
                ..
            } => {
                if *damage == 0 {
                    self.sound_player.play(SoundId::ArmorAbsorbed);
                } else if self.characters.get(attacker).has_equipped_ranged_weapon() {
                    self.sound_player.play(SoundId::HitArrow);
                } else {
                    self.sound_player.play(SoundId::MeleeAttack);
                }
                verb = match hit_type {
                    AttackHitType::Regular => "hit",
                    AttackHitType::Graze => "grazed",
                    AttackHitType::Critical => "crit",
                };
                applied_effects = effects.clone();
            }
            AttackOutcome::Block | AttackOutcome::Parry => {
                self.sound_player.play(SoundId::ArmorAbsorbed);
                verb = "missed";
            }
            AttackOutcome::Dodge | AttackOutcome::Miss => {
                self.sound_player.play(SoundId::AttackMiss);
                verb = "missed";
            }
        };

        let mut line = format!(
            "|{}| {} |{}|",
            self.characters.get(attacker).name,
            verb,
            self.characters.get(target).name
        );

        let mut damage_was_dealt = false;
        match event.outcome {
            AttackOutcome::Hit {
                damage,
                actual_health_lost,
                ..
            } => {
                self.animate_character_damage(target, actual_health_lost);
                damage_was_dealt = damage > 0;
                line.push_str(&format!(" ({} damage)", damage))
            }
            AttackOutcome::Dodge => line.push_str(" (dodge)"),
            AttackOutcome::Parry => line.push_str(" (parry)"),
            AttackOutcome::Block => line.push_str(" (block)"),
            AttackOutcome::Miss => {}
        }

        self.log.add_with_details(line, detail_lines);

        let target_pos = self.characters.get(target).pos();

        let (impact_text, text_style) = match event.outcome {
            AttackOutcome::Hit {
                damage,
                hit_type: AttackHitType::Regular,
                ..
            } => (format!("{}", damage), TextEffectStyle::HostileHit),
            AttackOutcome::Hit {
                damage,
                hit_type: AttackHitType::Graze,
                ..
            } => (format!("{}", damage), TextEffectStyle::HostileGraze),
            AttackOutcome::Hit {
                damage,
                hit_type: AttackHitType::Critical,
                ..
            } => (format!("{}!", damage), TextEffectStyle::HostileCrit),
            AttackOutcome::Dodge => ("Dodge".to_string(), TextEffectStyle::Miss),
            AttackOutcome::Parry => ("Parry".to_string(), TextEffectStyle::Miss),
            AttackOutcome::Miss => ("Miss".to_string(), TextEffectStyle::Miss),
            AttackOutcome::Block => ("Block".to_string(), TextEffectStyle::Miss),
        };

        self.game_grid
            .add_text_effect(target_pos, 0.0, 1.0, None, impact_text, text_style);

        if !applied_effects.is_empty() {
            let mut s = String::new();
            let mut texture = None;
            for apply_effect in applied_effects {
                if let ApplyEffect::Condition(condition) = apply_effect {
                    texture = Some(
                        self.status_textures
                            .get(&condition.condition.status_icon())
                            .unwrap()
                            .clone(),
                    );
                }
                s.push_str(&format!("{} ", apply_effect));
            }
            self.game_grid.add_text_effect(
                target_pos,
                0.0,
                2.0,
                texture,
                s,
                TextEffectStyle::HostileEffect,
            );
        };

        if damage_was_dealt {
            self.game_grid.animate_character_shaking(target, 0.2);
        }

        if let Some(outcomes) = &event.area_outcomes {
            self.add_effects_for_area_outcomes(0.0, MAGENTA, &target_pos, None, outcomes);
        }

        self.animation_stopwatch.set_to_at_least(0.3);
    }

    fn add_effect_for_ability_target_outcome(
        &mut self,
        outcome: &AbilityTargetOutcome,
        start_time: f32,
        target: CharacterId,
        target_pos: (i32, i32),
    ) {
        let mut effects = vec![];

        match &outcome {
            AbilityTargetOutcome::HitEnemy {
                damage,
                graze,
                applied_effects,
            } => {
                if let Some(dmg) = damage {
                    self.animate_character_damage(target, *dmg);
                    effects.push((None, format!("{}", dmg), TextEffectStyle::HostileHit, 1.0));
                } else if applied_effects.is_empty() {
                    if *graze {
                        effects.push((
                            None,
                            "Graze".to_string(),
                            TextEffectStyle::HostileGraze,
                            1.0,
                        ));
                    } else {
                        effects.push((None, "Hit".to_string(), TextEffectStyle::HostileHit, 1.0));
                    }
                }

                if !applied_effects.is_empty() {
                    let mut s = String::new();
                    let mut texture = None;
                    for apply_effect in applied_effects {
                        if let ApplyEffect::Condition(condition) = *apply_effect {
                            texture = Some(
                                self.status_textures
                                    .get(&condition.condition.status_icon())
                                    .unwrap()
                                    .clone(),
                            );
                        }
                        s.push_str(&format!("{} ", apply_effect));
                    }
                    effects.push((texture, s, TextEffectStyle::HostileEffect, 2.0));
                };
            }
            AbilityTargetOutcome::Resisted => {
                effects.push((None, "Resist".to_string(), TextEffectStyle::Miss, 1.0))
            }
            AbilityTargetOutcome::AffectedAlly { applied_effects } => {
                let mut s = String::new();
                let mut texture = None;
                for apply_effect in applied_effects {
                    if let ApplyEffect::Condition(condition) = *apply_effect {
                        texture = Some(
                            self.status_textures
                                .get(&condition.condition.status_icon())
                                .unwrap()
                                .clone(),
                        );
                    }
                    s.push_str(&format!("{} ", apply_effect));
                }

                effects.push((texture, s, TextEffectStyle::FriendlyEffect, 2.0))
            }
            AbilityTargetOutcome::AttackedEnemy(..) => {
                // The text effect is handled by the AttackedEvent; we shouldn't do anything additional here.
            }
        };

        for (texture, target_text, goodness, duration) in effects {
            self.game_grid.add_text_effect(
                target_pos,
                start_time,
                duration,
                texture,
                target_text,
                goodness,
            );
        }
    }

    fn set_new_active_character_id(&mut self, new_active_id: CharacterId) {
        if new_active_id != self.active_character_id {
            // When control switches to a new player controlled character, make the UI show that character
            if self.characters.get(new_active_id).player_controlled() {
                self.set_selected_character(new_active_id);
            }
        }

        self.active_character_id = new_active_id;
    }

    fn set_selected_character(&mut self, new_active_id: CharacterId) {
        self.player_portraits.set_selected_id(new_active_id);
        self.on_new_selected_character();
    }

    fn selected_character(&self) -> &Character {
        self.characters.get(self.player_portraits.selected_id())
    }

    fn on_new_selected_character(&mut self) {
        // In case we're hovering a button that will no longer be shown due to the character switch,
        // we need to clear it, so that we don't panic trying to render its tooltip for example
        self.hovered_button = None;
    }

    pub fn update(&mut self, game: &CoreGame, elapsed: f32) -> Option<PlayerChose> {
        self.set_allowed_to_use_action_buttons(
            self.player_portraits.selected_id() == self.active_character_id,
        );

        for (_id, character_ui) in &mut self.character_uis {
            character_ui
                .action_points_row
                .update(elapsed, &self.sound_player);
            character_ui.stamina_bar.borrow_mut().update(elapsed);
        }

        let selected_char = self.selected_character();
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
            Some(ActivityPopupOutcome::ChangedAbilityEnhancements) => {
                // TODO update hit chance?
                self.refresh_use_ability_state();
            }
            Some(ActivityPopupOutcome::ChangedAttackEnhancements) => {
                self.refresh_attack_state();
            }
            Some(ActivityPopupOutcome::ChangedMovementSprint(_sprint_usage)) => {
                self.game_grid.update_move_speed(self.active_character_id);
            }
            Some(ActivityPopupOutcome::ChangedReaction) => {
                self.refresh_reaction_state();
            }
            None => {}
        }

        let mut action_button_clicked = None;
        let mut invalid_action = false;

        self.event_queue
            .take()
            .into_iter()
            .for_each(|event| match event {
                InternalUiEvent::ButtonHovered(
                    event @ ButtonHovered {
                        id, hovered_pos, ..
                    },
                ) => {
                    if hovered_pos.is_some() {
                        self.sound_player.play(SoundId::HoverButton);
                        self.hovered_button = Some(event);
                    } else if let Some(previously_hovered_button) = &self.hovered_button {
                        if id == previously_hovered_button.id {
                            self.hovered_button = None
                        }
                    }
                }

                InternalUiEvent::ButtonClicked {
                    action: btn_action,
                    context,
                    ..
                } => {
                    if context != Some(ButtonContext::CharacterSheet) {
                        match btn_action {
                            ButtonAction::Action(base_action) => {
                                self.sound_player.play(SoundId::ClickButton);
                                action_button_clicked = Some(base_action);
                            }
                            unexpected => panic!("{:?}", unexpected),
                        }
                    }
                }

                InternalUiEvent::ButtonInvalidClicked { context } => {
                    if context != Some(ButtonContext::CharacterSheet) {
                        invalid_action = true;
                    }
                }
            });

        let character_ui = self
            .character_uis
            .get_mut(&self.player_portraits.selected_id())
            .unwrap();

        for (_id, btn) in &character_ui.tracked_action_buttons {
            if let Some((keycode, _font)) = btn.hotkey.borrow().as_ref() {
                if is_key_pressed(*keycode) {
                    if btn.enabled.get() {
                        match btn.action {
                            ButtonAction::Action(base_action) => {
                                self.sound_player.play(SoundId::ClickButton);
                                action_button_clicked = Some(base_action)
                            }
                            _ => unreachable!("button clicked via hotkey: {:?}", btn.action),
                        }
                    } else {
                        invalid_action = true;
                    }
                }
            }
        }

        if invalid_action {
            self.on_invalid_action_clicked();
        }

        if let Some(base_action) = action_button_clicked {
            let may_choose_action = matches!(
                &*self.state.borrow(),
                UiState::ChoosingAction | UiState::ConfiguringAction(..)
            );

            if may_choose_action && self.active_character().can_use_action(base_action) {
                if let Some(s) = ConfiguredAction::from_base_action(base_action) {
                    let already_configuring_it = match &*self.state.borrow() {
                        UiState::ConfiguringAction(configured_action) => configured_action == &s,
                        _ => false,
                    };
                    if already_configuring_it {
                        self.set_state(UiState::ChoosingAction);
                    } else {
                        self.set_state(UiState::ConfiguringAction(s));
                    }
                } else {
                    assert!(player_choice.is_none());
                    // The player ends their turn
                    player_choice = Some(PlayerChose::Action(None));
                }
            } else {
                println!("warning: tried to use {:?}, when not allowed", base_action);
                // yes this can still happen, for example if spam clicking "End Turn" (probably there's a short window after end of turn when
                // the button click still registers, even though it shouldn't.)
            }
        }

        self.top_character_portraits
            .set_selected_character(self.player_portraits.selected_id());
        self.player_portraits.update(game);

        self.update_character_status(&game.characters);

        let character_ui = self
            .character_uis
            .get_mut(&self.player_portraits.selected_id())
            .unwrap();
        if let Some(hovered_btn) = &self.hovered_button {
            if hovered_btn.context != Some(ButtonContext::CharacterSheet) {
                character_ui.action_points_row.reserved_and_hovered_ap = (
                    self.activity_popup.reserved_and_hovered_action_points().0,
                    hovered_btn.action.action_point_cost(),
                );
                character_ui
                    .mana_bar
                    .borrow_mut()
                    .set_reserved(hovered_btn.action.mana_cost());
                character_ui
                    .stamina_bar
                    .borrow_mut()
                    .set_reserved(hovered_btn.action.stamina_cost());
            }
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

    fn on_invalid_action_clicked(&mut self) {
        self.sound_player.play(SoundId::Invalid);
        self.game_grid.animate_character_speaking(
            self.player_portraits.selected_id(),
            0.7,
            "Can't do that!",
        );
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
                    } => {
                        self.remembered_attack_enhancements
                            .insert(self.active_character_id, selected_enhancements.clone());

                        Some(Action::Attack {
                            hand: attack.hand,
                            enhancements: selected_enhancements.clone(),
                            target: target.unwrap(),
                        })
                    }
                    &ConfiguredAction::UseAbility {
                        ability,
                        selected_enhancements,
                        target,
                    } => Some(Action::UseAbility {
                        ability: *ability,
                        enhancements: selected_enhancements.clone(),
                        target: target.clone(),
                    }),
                    &ConfiguredAction::Move {
                        cost,
                        selected_movement_path,
                    } => {
                        let dst = selected_movement_path.last().unwrap();
                        let total_distance = dst.0;
                        let positions = selected_movement_path
                            .iter()
                            .map(|(_dist_from_start, pos)| *pos)
                            .collect();

                        Some(Action::Move {
                            total_distance,
                            extra_cost: *cost,
                            positions,
                        })
                    }
                    &ConfiguredAction::ChangeEquipment { drag } => {
                        let (from, to) = self
                            .character_uis
                            .get_mut(&self.active_character_id)
                            .unwrap()
                            .character_sheet
                            .resolve_drag_to_slots(drag.borrow().unwrap());

                        *drag.borrow_mut() = None;

                        Some(Action::ChangeEquipment { from, to })
                    }
                    &ConfiguredAction::UseConsumable(consumption) => Some(Action::UseConsumable {
                        inventory_equipment_index: consumption.unwrap().equipment_idx,
                    }),
                };
                PlayerChose::Action(action)
            }

            UiState::ReactingToAttack { selected, .. } => PlayerChose::AttackedReaction(*selected),
            UiState::ReactingToHit { selected, .. } => PlayerChose::HitReaction(*selected),
            UiState::ReactingToMovementAttackOpportunity { selected, .. } => {
                PlayerChose::OpportunityAttack(*selected)
            }
            UiState::ReactingToRangedAttackOpportunity { selected, .. } => {
                PlayerChose::OpportunityAttack(*selected)
            }

            UiState::ChoosingAction | UiState::Idle => unreachable!(),
        }
    }

    fn update_character_status(&mut self, characters: &Characters) {
        for character in characters.iter() {
            let id = &character.id();
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

                ui.character_sheet.conditions_list.borrow_mut().infos = character.condition_infos();

                ui.action_points_row.current_ap = self
                    .characters
                    .get(self.player_portraits.selected_id())
                    .action_points
                    .current();
                ui.action_points_row.is_characters_turn = *id == self.active_character_id;

                let statuses = &character.condition_infos();
                self.player_portraits.set_statuses(*id, statuses);
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
    status_textures: HashMap<StatusId, Texture2D>,
    sound_player: SoundPlayer,
) -> HashMap<u32, CharacterUi> {
    let mut next_button_id = 1;

    let mut character_uis: HashMap<CharacterId, CharacterUi> = Default::default();

    let character_sheet_screen_pos = Rc::new(RefCell::new((100.0, 100.0)));

    for character in characters {
        if !character.player_controlled() {
            continue;
        }

        let character_ui = build_character_ui(
            equipment_icons,
            icons,
            event_queue,
            simple_font,
            character,
            &mut next_button_id,
            status_textures.clone(),
            Rc::clone(&character_sheet_screen_pos),
            sound_player.clone(),
        );

        character_uis.insert(character.id(), character_ui);
    }
    character_uis
}

fn build_character_ui(
    equipment_icons: &HashMap<EquipmentIconId, Texture2D>,
    icons: &HashMap<IconId, Texture2D>,
    event_queue: &Rc<RefCell<Vec<InternalUiEvent>>>,
    simple_font: &Font,
    character: &Rc<Character>,
    next_button_id: &mut u32,
    status_textures: HashMap<StatusId, Texture2D>,
    character_sheet_screen_pos: Rc<RefCell<(f32, f32)>>,
    sound_player: SoundPlayer,
) -> CharacterUi {
    let mut new_button =
        |btn_action, character: Option<Rc<Character>>, in_character_sheet: bool| {
            let event_queue = Some(Rc::clone(event_queue));
            let mut btn = ActionButton::new(
                btn_action,
                event_queue,
                *next_button_id,
                icons,
                character,
                simple_font,
            );
            *next_button_id += 1;
            if in_character_sheet {
                btn.context = Some(ButtonContext::CharacterSheet);
                if !matches!(btn_action, ButtonAction::Action(..)) {
                    btn.set_parent_bg_color(CHARACTER_SHEET_BG_COLOR);
                };
            }
            btn
        };

    let buttons_row_bg = LIGHTGRAY;

    let mut tracked_action_buttons = IndexMap::new();
    let mut hoverable_buttons = vec![];
    let mut basic_buttons = vec![];
    let mut change_eq_btn = None;
    let mut use_consumable_btn = None;
    let mut ability_buttons = vec![];

    let mut attack_button_for_character_sheet = None;
    let mut ability_buttons_for_character_sheet = vec![];
    let mut attack_enhancement_buttons_for_character_sheet = vec![];
    let mut passive_buttons_for_character_sheet = vec![];

    let basic_hotkeys = [
        KeyCode::Key1,
        KeyCode::Key2,
        KeyCode::Key3,
        KeyCode::Key4,
        KeyCode::Key5,
    ];
    let ability_hotkeys = [KeyCode::Q, KeyCode::W, KeyCode::E, KeyCode::R, KeyCode::T];

    for action in character.known_actions() {
        let btn_action = ButtonAction::Action(action);
        let mut btn = new_button(btn_action, Some(character.clone()), false);
        //btn.set_parent_bg_color(buttons_row_bg);
        let btn = Rc::new(btn);
        tracked_action_buttons.insert(button_action_id(btn_action), Rc::clone(&btn));
        hoverable_buttons.push(Rc::clone(&btn));
        match action {
            BaseAction::Attack { .. } => {
                *btn.hotkey.borrow_mut() = basic_hotkeys
                    .get(basic_buttons.len())
                    .map(|key| (*key, simple_font.clone()));
                basic_buttons.push(btn);

                let btn = Rc::new(new_button(btn_action, Some(character.clone()), true));
                attack_button_for_character_sheet = Some(btn.clone());
                hoverable_buttons.push(btn);
            }
            BaseAction::UseAbility(ability) => {
                *btn.hotkey.borrow_mut() = ability_hotkeys
                    .get(ability_buttons.len())
                    .map(|key| (*key, simple_font.clone()));
                ability_buttons.push(btn);

                let btn = Rc::new(new_button(btn_action, Some(character.clone()), true));

                let enhancement_buttons: Vec<Rc<ActionButton>> = ability
                    .possible_enhancements
                    .iter()
                    .filter_map(|maybe_enhancement| *maybe_enhancement)
                    .filter_map(|enhancement| {
                        if character.knows_ability_enhancement(enhancement) {
                            let enhancement_btn = Rc::new(new_button(
                                ButtonAction::AbilityEnhancement(enhancement),
                                None,
                                true,
                            ));
                            hoverable_buttons.push(enhancement_btn.clone());
                            Some(enhancement_btn)
                        } else {
                            None
                        }
                    })
                    .collect();
                ability_buttons_for_character_sheet.push((btn.clone(), enhancement_buttons));

                hoverable_buttons.push(btn);
            }
            BaseAction::Move => {
                *btn.hotkey.borrow_mut() = basic_hotkeys
                    .get(basic_buttons.len())
                    .map(|key| (*key, simple_font.clone()));
                basic_buttons.push(btn);
            }
            BaseAction::ChangeEquipment => {
                change_eq_btn = Some(btn);
            }
            BaseAction::UseConsumable => {
                use_consumable_btn = Some(btn);
            }
        }
    }

    let mut reaction_buttons_for_character_sheet = vec![];
    for reaction in character.known_on_attacked_reactions() {
        let btn_action = ButtonAction::OnAttackedReaction(reaction);
        let btn = Rc::new(new_button(btn_action, None, true));
        hoverable_buttons.push(Rc::clone(&btn));
        reaction_buttons_for_character_sheet.push(btn);
    }
    for (_subtext, reaction) in character.known_on_hit_reactions() {
        let btn_action = ButtonAction::OnHitReaction(reaction);
        let btn = Rc::new(new_button(btn_action, None, true));
        hoverable_buttons.push(Rc::clone(&btn));
        reaction_buttons_for_character_sheet.push(btn);
    }

    // TODO: Only include inherently known enhancements here; not those gained from weapons (since weapons can be unequipped
    // without the character sheet being updated)
    for (_subtext, enhancement) in character.known_attack_enhancements(HandType::MainHand) {
        let btn_action = ButtonAction::AttackEnhancement(enhancement);
        let btn = Rc::new(new_button(btn_action, None, true));
        hoverable_buttons.push(Rc::clone(&btn));
        attack_enhancement_buttons_for_character_sheet.push(btn);
    }

    for passive_skill in &character.known_passive_skills {
        let btn_action = ButtonAction::Passive(*passive_skill);
        let btn = Rc::new(new_button(btn_action, Some(character.clone()), true));
        hoverable_buttons.push(Rc::clone(&btn));
        passive_buttons_for_character_sheet.push(Rc::clone(&btn));
    }

    let character_sheet = CharacterSheet::new(
        simple_font,
        Rc::clone(character),
        equipment_icons,
        attack_button_for_character_sheet,
        reaction_buttons_for_character_sheet,
        attack_enhancement_buttons_for_character_sheet,
        ability_buttons_for_character_sheet,
        passive_buttons_for_character_sheet,
        ConditionsList::new(simple_font.clone(), vec![], status_textures.clone()),
        character_sheet_screen_pos,
        sound_player,
    );

    let mut buttons = basic_buttons;
    buttons.extend_from_slice(&ability_buttons);
    let mut buttons: Vec<Element> = buttons.into_iter().map(|btn| Element::Rc(btn)).collect();
    let button_placeholder = Rectangle {
        size: REGULAR_ACTION_BUTTON_SIZE,
        style: Style {
            border_color: Some(GRAY),
            background_color: Some(ACTION_BUTTON_BG_COLOR),
            ..Default::default()
        },
        ..Default::default()
    };
    while buttons.len() < 8 {
        buttons.push(Element::Rect(button_placeholder.clone()));
    }
    buttons.push(Element::Rc(change_eq_btn.unwrap()));
    buttons.push(Element::Rc(use_consumable_btn.unwrap()));

    let button_row = buttons_row(buttons);

    let actions_section = Container {
        layout_dir: LayoutDirection::Vertical,
        margin: 0.0,
        children: vec![button_row],
        style: Style {
            background_color: Some(buttons_row_bg),
            padding: 2.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let resource_bars = ResourceBars::new(character, simple_font);

    let action_points_row = ActionPointsRow::new(
        (20.0, 20.0),
        0.25,
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
        font: simple_font.clone(),
    }
}

pub struct ResourceBars {
    pub container: Container,
    pub health_bar: Rc<RefCell<LabelledResourceBar>>,
    pub mana_bar: Rc<RefCell<LabelledResourceBar>>,
    pub stamina_bar: Rc<RefCell<LabelledResourceBar>>,
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
            COL_RED,
            font.clone(),
            "<heart>",
        )));

        let mana_bar = Rc::new(RefCell::new(LabelledResourceBar::new(
            character.mana.current(),
            character.mana.max(),
            "Mana",
            COL_BLUE,
            font.clone(),
            "<mana>",
        )));

        let stamina_bar = Rc::new(RefCell::new(LabelledResourceBar::new(
            character.stamina.current(),
            character.stamina.max(),
            "Stamina",
            COL_GREEN_0,
            font.clone(),
            "<stamina>",
        )));

        let container = Container {
            layout_dir: LayoutDirection::Vertical,
            margin: 9.0,
            align: Align::Start,
            children: vec![
                Element::RcRefCell(health_bar.clone()),
                Element::RcRefCell(stamina_bar.clone()),
                Element::RcRefCell(mana_bar.clone()),
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
            BaseAction::UseAbility(ability) => format!("ABILITY_{}", ability.name),
            BaseAction::Move => "MOVE".to_string(),
            BaseAction::ChangeEquipment => "CHANGING_EQUIPMENT".to_string(),
            BaseAction::UseConsumable => "USING_CONSUMABLE".to_string(),
        },

        _ => unreachable!(),
    }
}

fn buttons_row(buttons: Vec<Element>) -> Element {
    Element::Container(Container {
        layout_dir: LayoutDirection::Horizontal,
        margin: 3.0,
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
    SwitchTo(CharacterId),
}
