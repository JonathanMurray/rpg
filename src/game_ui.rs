use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use indexmap::IndexMap;
use macroquad::{
    color::{Color, BLACK, DARKGRAY, GRAY, GREEN, LIGHTGRAY, MAGENTA, RED, WHITE},
    input::{
        is_key_down, is_key_pressed, is_mouse_button_pressed, mouse_position, KeyCode, MouseButton,
    },
    math::Rect,
    shapes::{draw_line, draw_rectangle},
    text::Font,
    texture::draw_texture,
    window::{screen_height, screen_width},
};

use crate::{
    action_button::{
        draw_button_tooltip, ActionButton, ActionTooltip, ButtonAction, ButtonContext,
        ButtonHovered, ButtonSelected, InternalUiEvent, ACTION_BUTTON_BG_COLOR,
        REGULAR_ACTION_BUTTON_SIZE,
    },
    activity_popup::{ActivityPopup, ActivityPopupOutcome},
    banner::Banner,
    base_ui::{Align, Border, Container, Drawable, Element, LayoutDirection, Rectangle, Style},
    character_sheet::{CharacterSheet, CHARACTER_SHEET_BG_COLOR},
    conditions_ui::ConditionsList,
    core::{
        distance_between, predict_ability, predict_attack, Ability, AbilityAreaOutcome,
        AbilityEnhancement, AbilityId, AbilityResolvedEvent, AbilityRollType, AbilityTarget,
        AbilityTargetOutcome, Action, ActionReach, ActionTarget, AppliedEffect, ApplyCondition,
        ApplyEffect, AreaShape, AttackAction, AttackEnhancement, AttackEnhancementEffect,
        AttackOutcome, AttackType, AttackedEvent, BaseAction, Character, CharacterFacing,
        CharacterId, Characters, CircleEffect, ConcreteCircle, Condition, CoreGame, DamageInterval,
        DamageSource, DamageType, DecoratedTargetOutcome, GameEvent, GameOverType, HandType,
        HitType, IncludeCircleEffects, MovedEvent, MovementType, OnAttackedReaction, OnHitReaction,
        Position, TargetAcquisition, TargetPoint, TargetPrediction, MOVE_COST_FACTOR_IN_LIQUID,
    },
    data::{INSPIRE, KILL},
    drawing::draw_dashed_line_ex,
    equipment_ui::{EquipmentConsumption, EquipmentDrag},
    game_ui_components::{
        ActionPointsRow, CharacterSheetToggle, LabelledResourceBar, Log, PlayerPortraits,
        TopCharacterPortraits,
    },
    grid::{
        EffectData, EffectGraphics, EffectPosition, EffectVariant, GameGrid, GridOutcome, NewState,
        TargetEffectPreview, TextEffect,
    },
    init_fight_map::GameInitState,
    pathfind::{Elevation, Occupation, PathfindGrid, Surface, TerrainType},
    resources::GameResources,
    settings::build_settings,
    sounds::{SoundId, SoundPlayer},
    target_ui::TargetUi,
    textures::{EffectId, StatusId, TerrainId, UI_BG_TEXTURE},
    tooltip::{
        handle_keyword_tooltip_end_of_frame, Keyword, DID_DRAW_KEYWORD_TOOLTIP_THIS_FRAME,
        KEYWORD_TOOLTIP_COUNTER,
    },
    util::{
        are_entities_within_melee, extend_to_range, line_visitor, COL_BLUE, COL_DARK, COL_GREEN_0,
        COL_GREEN_1, COL_GREEN_2, COL_LIGHT_BLUE, COL_RED,
    },
};
use crate::{
    pathfind::PathNode,
    tooltip::{draw_tooltip, Side, TooltipPositionPreference},
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
        movement: Vec<Position>,
        selected: bool,
    },
    ReactingToRangedAttackOpportunity {
        reactor: CharacterId,
        attacker: CharacterId,
        victim: CharacterId,
        selected: bool,
    },
    Idle {
        recently_committed_action: Option<PlayerChose>,
    },
}

impl UiState {
    pub fn action_usability_problem(
        &self,
        relevant_character: &Character,
        characters: &Characters,
        pathfind_grid: &PathfindGrid,
    ) -> Option<UsabilityProblem> {
        match self {
            UiState::ConfiguringAction(configured_action) => {
                configured_action.usability_problem(relevant_character, characters, pathfind_grid)
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
                ConfiguredAction::UseAbility { target, .. } => {
                    println!("Previous ui target: {:?}", target);
                    *target = new_target
                }

                unexpected => panic!("Action has no target: {:?}", unexpected),
            },
            unexpected => panic!("State has no target: {:?}", unexpected),
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
        ability: &'static Ability,
        selected_enhancements: Vec<&'static AbilityEnhancement>,
        target: ActionTarget,
        equipment_idx_to_consume: Option<usize>,
    },
    Move {
        // Including the actor's current location, going all the way to the destination.
        selected_movement_path: Vec<PathNode>,
        cost: u32,
    },
    ChangeEquipment {
        drag: Rc<RefCell<Option<EquipmentDrag>>>,
    },
    SelectConsumableFromInventory,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum UsabilityProblem {
    OutOfReach,
    NoLineOfSight,
    NotEnoughAp,
    SelectMovement,
    SelectEnemy,
    SelectAlly,
    SelectArea,
    SelectDestination,
    DragItem,
    SelectConsumable,
}

impl UsabilityProblem {
    pub fn message(&self) -> &str {
        match self {
            UsabilityProblem::OutOfReach => "Out of reach",
            UsabilityProblem::NoLineOfSight => "No line of sight",
            UsabilityProblem::NotEnoughAp => "Not enough AP",
            UsabilityProblem::SelectMovement => "Select movement",
            UsabilityProblem::SelectEnemy => "Select an enemy",
            UsabilityProblem::SelectAlly => "Select an ally",
            UsabilityProblem::SelectArea => "Select an area",
            UsabilityProblem::SelectDestination => "Select destination",
            UsabilityProblem::DragItem => "Drag an item",
            UsabilityProblem::SelectConsumable => "Select a consumable",
        }
    }
}

fn ability_usability_problem(
    relevant_character: &Character,
    characters: &Characters,
    pathfind_grid: &PathfindGrid,
    ability: &'static Ability,
    target: &ActionTarget,
    selected_enhancements: &[&AbilityEnhancement],
) -> Option<UsabilityProblem> {
    if !relevant_character.can_use_ability(ability) {
        // TODO There could be other explanations!
        return Some(UsabilityProblem::NotEnoughAp);
    }

    let usability_check_reach_and_line_of_sight = |point: TargetPoint| -> Option<UsabilityProblem> {
        if relevant_character.reaches_with_ability(ability, selected_enhancements, point) {
            if pathfind_grid.obstructed_line_of_sight(relevant_character.pos(), point.pos()) {
                Some(UsabilityProblem::NoLineOfSight)
            } else {
                None
            }
        } else {
            Some(UsabilityProblem::OutOfReach)
        }
    };

    match target {
        ActionTarget::Character(target_id, movement) => {
            if let Some(positions) = movement {
                if positions.is_empty() {
                    return Some(UsabilityProblem::SelectMovement);
                }
            }
            let target_char = characters.get(*target_id);
            usability_check_reach_and_line_of_sight(TargetPoint::Char(target_char))
        }

        ActionTarget::Position(target_pos) => {
            assert!(matches!(
                ability.target,
                AbilityTarget::PositionCircle { .. } | AbilityTarget::Movement { .. }
            ));
            usability_check_reach_and_line_of_sight(TargetPoint::Pos(*target_pos))
        }

        ActionTarget::Projectile(tl) => {
            assert!(matches!(ability.target, AbilityTarget::Projectile { .. }));
            usability_check_reach_and_line_of_sight(TargetPoint::Pos(*&tl.destination))
        }
        ActionTarget::Line(tl) => {
            assert!(matches!(ability.target, AbilityTarget::Line { .. }));
            usability_check_reach_and_line_of_sight(TargetPoint::Pos(*&tl.destination))
        }

        // There is a usability problem; we will not be able to call predict_ability for example,
        // but we don't want the usability problem to be shown as a warning text above the activity popup
        ActionTarget::None => match ability.target {
            AbilityTarget::None { .. } => None,
            AbilityTarget::Character { acquisition, .. } => match acquisition {
                TargetAcquisition::Enemies => Some(UsabilityProblem::SelectEnemy),
                _ => Some(UsabilityProblem::SelectAlly),
            },
            AbilityTarget::PositionCircle { .. }
            | AbilityTarget::Line { .. }
            | AbilityTarget::Projectile { .. } => Some(UsabilityProblem::SelectArea),
            AbilityTarget::Movement { .. } => Some(UsabilityProblem::SelectDestination),
        },
    }
}

impl ConfiguredAction {
    fn has_target(&self) -> bool {
        match self {
            ConfiguredAction::Attack { target, .. } => target.is_some(),
            ConfiguredAction::UseAbility { target, .. } => !matches!(target, ActionTarget::None),
            ConfiguredAction::Move { .. } => false,
            ConfiguredAction::ChangeEquipment { .. } => false,
            ConfiguredAction::SelectConsumableFromInventory { .. } => false,
        }
    }

    fn usability_problem(
        &self,
        relevant_character: &Character,
        characters: &Characters,
        pathfind_grid: &PathfindGrid,
    ) -> Option<UsabilityProblem> {
        match self {
            ConfiguredAction::Attack {
                target,
                attack,
                selected_enhancements,
                ..
            } => match target {
                Some(target_id) => {
                    let target_char = characters.get(*target_id);
                    if relevant_character.can_attack(*attack) {
                        let (_range, reach) = relevant_character.reaches_with_attack(
                            attack.hand,
                            target_char.pos(),
                            selected_enhancements.iter().map(|e| e.effect),
                        );
                        if matches!(
                            reach,
                            ActionReach::Yes | ActionReach::YesButDisadvantage(..)
                        ) {
                            if pathfind_grid.obstructed_line_of_sight(
                                relevant_character.pos(),
                                target_char.pos(),
                            ) {
                                Some(UsabilityProblem::NoLineOfSight)
                            } else {
                                None
                            }
                        } else {
                            Some(UsabilityProblem::OutOfReach)
                        }
                    } else {
                        Some(UsabilityProblem::NotEnoughAp)
                    }
                }
                None => Some(UsabilityProblem::SelectEnemy),
            },

            ConfiguredAction::UseAbility {
                target,
                ability,
                selected_enhancements,
                ..
            } => ability_usability_problem(
                relevant_character,
                characters,
                pathfind_grid,
                ability,
                target,
                selected_enhancements,
            ),
            ConfiguredAction::Move { .. } => {
                None
                /*
                if selected_movement_path.is_empty() {
                    Some("Select movement")
                } else {
                    None
                }
                 */
            }

            ConfiguredAction::ChangeEquipment { drag } => {
                if matches!(
                    *drag.borrow(),
                    Some(EquipmentDrag {
                        to_idx: Some(_),
                        ..
                    })
                ) {
                    if relevant_character.can_change_equipment() {
                        None
                    } else {
                        Some(UsabilityProblem::NotEnoughAp)
                    }
                } else {
                    Some(UsabilityProblem::DragItem)
                }
            }

            ConfiguredAction::SelectConsumableFromInventory { .. } => {
                // As soon as the player chooses a consumable, the UI switches to ConfiguredAction::UseAbility
                Some(UsabilityProblem::SelectConsumable)
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
                equipment_idx_to_consume: None,
            }),
            BaseAction::Move => Some(Self::Move {
                cost: 0,
                selected_movement_path: Default::default(),
            }),
            BaseAction::ChangeEquipment => Some(Self::ChangeEquipment {
                drag: Rc::new(RefCell::new(None)),
            }),
            BaseAction::UseConsumable => Some(Self::SelectConsumableFromInventory {}),
        }
    }

    pub fn base_action(&self) -> BaseAction {
        match self {
            ConfiguredAction::Attack { attack, .. } => BaseAction::Attack(*attack),
            ConfiguredAction::UseAbility {
                ability,
                equipment_idx_to_consume,
                ..
            } => {
                if equipment_idx_to_consume.is_some() {
                    // This configured ability comes from using a consumble, so the "use consumable" button should highlight
                    // in the UI. There's no other dedicated button for the ability
                    BaseAction::UseConsumable
                } else {
                    BaseAction::UseAbility(ability)
                }
            }
            ConfiguredAction::Move { .. } => BaseAction::Move,
            ConfiguredAction::ChangeEquipment { .. } => BaseAction::ChangeEquipment,
            ConfiguredAction::SelectConsumableFromInventory { .. } => BaseAction::UseConsumable,
        }
    }

    pub fn base_ap_cost(&self) -> i32 {
        match self {
            ConfiguredAction::UseAbility {
                ability,
                equipment_idx_to_consume: Some(..),
                ..
            } => ability.action_point_cost as i32,
            _ => self.base_action().action_point_cost(),
        }
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
            ConfiguredAction::SelectConsumableFromInventory { .. } => 0,
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

pub const UI_HEIGHT: f32 = 230.0;
const UI_WIDTH: f32 = 700.0;

impl CharacterUi {
    pub fn draw(&self, ui_x0: f32, y: f32) {
        let y0 = y + 5.0;
        let resources_mid_x = ui_x0 + 90.0;
        self.actions_section.draw(
            ui_x0 + UI_WIDTH / 2.0 - self.actions_section.size().0 / 2.0,
            //screen_width() / 2.0 - self.actions_section.size().0 / 2.0,
            y0,
        );

        self.action_points_row.draw(
            resources_mid_x - self.action_points_row.size().0 / 2.0,
            screen_height() - 140.0,
        );

        let resource_bars_y = screen_height() - 105.0;
        let resource_bars_x = resources_mid_x - self.resource_bars.size().0 / 2.0;
        self.resource_bars.draw(resource_bars_x, resource_bars_y);

        let tooltip = if self.action_points_row.hovered.get() {
            Some((
                "Action Points (AP)",
                vec!["Used for actions.".to_string(), "".to_string(), "|<value>4| AP is restored after ending your turn, although this amount can be changed by certain conditions.".to_string()],
                vec![]
            ))
        } else if self.health_bar.borrow().hovered.get() {
            Some((
                "Health |<heart>|",
                vec!["If this reaches |<value>0|, you die.".to_string(), "".to_string(), "|<value>25%| of missing |<heart>| is restored after combat. (Max |<heart>| is affected by |<stat>Strength|.)".to_string(),
                "Being |<heart>| < 20% causes |<keyword>Near-death|.".to_string()],
                vec![Keyword::Cond(Condition::NearDeath, None)]
            ))
        } else if self.stamina_bar.borrow().hovered.get() {
            Some((
                "Stamina |<stamina>|",
                vec!["Used for movement and certain actions.".to_string(), "".to_string(), "|<value>25%| |<stamina>| is restored after ending your turn. (Max |<stamina>| is affected by |<stat>Strength| and |<stat>Agility|.)".to_string()],
                vec![]
            ))
        } else if self.mana_bar.borrow().hovered.get() {
            Some((
                "Mana |<mana>|",
                vec!["Used for spells.".to_string(), "".to_string(), "|<value>25%| |<mana>| is restored after combat. (Max |<mana>| is affected by |<stat>Spirit|.)".to_string()],
                vec![]
            ))
        } else {
            None
        };

        if let Some((header, content, keywords)) = tooltip {
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
                &content,
                &keywords,
                None,
                None,
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

    faster_movement: Rc<Cell<bool>>,
    slow_motion: Rc<Cell<bool>>,
    settings: Container,
}

impl UserInterface {
    pub fn new(
        game: &CoreGame,
        resources: GameResources,
        init_state: GameInitState,
        sound_player: SoundPlayer,
    ) -> Self {
        let characters = game.characters.clone();
        let active_character_id = game.active_character_id;

        let event_queue = Rc::new(RefCell::new(vec![]));

        let character_uis = build_character_uis(
            &event_queue,
            &resources.simple_font,
            &resources.big_font,
            characters.iter(),
            sound_player.clone(),
        );

        let ui_state = Rc::new(RefCell::new(UiState::Idle {
            recently_committed_action: None,
        }));

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
            resources.tiny_font.clone(),
            resources.terrain_atlas.clone(),
            Rc::clone(&init_state.pathfind_grid),
            init_state.background,
            init_state.terrain_objects,
            init_state.decorations,
            init_state.surfaces,
            init_state.control_points,
            sound_player.clone(),
        );

        game_grid.refresh_tiles_and_pathfind_grid();

        let player_portraits = PlayerPortraits::new(
            &characters,
            first_player_character_id,
            active_character_id,
            resources.simple_font.clone(),
            resources.big_font.clone(),
            sound_player.clone(),
        );

        let character_sheet_toggle =
            CharacterSheetToggle::new(resources.big_font.clone(), 12, sound_player.clone());

        let character_portraits = TopCharacterPortraits::new(
            &game.characters,
            game.active_character_id,
            resources.big_font.clone(),
            //decorative_font.clone(),
        );

        let target_ui = TargetUi::new(resources.big_font.clone(), resources.simple_font.clone());

        let activity_popup = ActivityPopup::new(
            resources.big_font.clone(),
            resources.simple_font.clone(),
            ui_state.clone(),
            characters.clone(),
            active_character_id,
            sound_player.clone(),
            Rc::clone(&init_state.pathfind_grid),
        );

        let mut banner = Banner::new();
        banner.set("Battle!", 2.0);

        //sound_player.play(SoundId::Battle);
        sound_player.play(SoundId::YourTurn);

        let faster_movement = Rc::new(Cell::new(false));
        let slow_motion = Rc::new(Cell::new(false));

        let settings = build_settings(
            &resources.big_font,
            &resources.simple_font,
            sound_player.clone(),
            faster_movement.clone(),
            slow_motion.clone(),
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
            faster_movement,
            slow_motion,
            settings,
        }
    }

    fn set_character_sheet_shown(&self, shown: bool) {
        self.character_sheet_toggle.set_shown(shown);
    }

    pub fn draw(&mut self) -> Option<PlayerChose> {
        let ui_y = screen_height() - UI_HEIGHT;

        let ui_x0 =
            (screen_width() / 2.0 - 350.0).min(screen_width() - self.log.width() - 5.0 - UI_WIDTH);
        let ui_x1 = ui_x0 + UI_WIDTH;

        let resources_mid_x = ui_x0 + 90.0;

        let popup_rect = self.activity_popup.last_drawn_rectangle;
        let target_ui_rect = self.target_ui.last_drawn_rectangle.get();
        let settings_rect = self.settings.last_drawn_rectangle.get();
        let log_rect = self.log.last_drawn_rect();

        let mouse_pos = mouse_position();

        let character_ui = self
            .character_uis
            .get(&self.player_portraits.selected_id())
            .unwrap();

        let obstructed_by_character_sheet = self.character_sheet_toggle.is_shown()
            && character_ui
                .character_sheet
                .rect()
                .contains(mouse_pos.into());

        let ui_bg_texture = UI_BG_TEXTURE.get().unwrap();

        let main_ui_rect = Rect::new(
            ui_x0 - 10.0,
            ui_y - 10.0,
            ui_bg_texture.width(),
            ui_bg_texture.height(),
        );

        let is_grid_obstructed = popup_rect.contains(mouse_pos.into())
            || target_ui_rect.contains(mouse_pos.into())
            || settings_rect.contains(mouse_pos.into())
            || log_rect.contains(mouse_pos.into())
            || obstructed_by_character_sheet
            || main_ui_rect.contains(mouse_pos.into());
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
            !is_grid_obstructed,
            hovered_action,
            character_ui.action_points_row.reserved_and_hovered_ap,
            false,
        );

        let mut player_chose = self.handle_grid_outcome(grid_outcome);

        draw_texture(
            UI_BG_TEXTURE.get().unwrap(),
            main_ui_rect.x,
            main_ui_rect.y,
            WHITE,
        );

        self.activity_popup.draw(ui_x0 + 64.0, ui_y - 1.0);

        let may_show_end_turn_button = matches!(
            &*self.state.borrow(),
            UiState::ChoosingAction | UiState::ConfiguringAction(..)
        );
        let player_portraits_outcome = self.player_portraits.draw(
            ui_x0 + 230.0,
            screen_height() - 120.0,
            may_show_end_turn_button,
        );
        let mut changed_character = player_portraits_outcome.changed_character;
        self.character_sheet_toggle.draw(
            resources_mid_x - self.character_sheet_toggle.size().0 / 2.0,
            screen_height() - 35.0,
        );

        let hovered_character_id = self.top_character_portraits.draw(0.0, 0.0);
        self.game_grid.set_hovered_portrait(hovered_character_id);
        if let Some(id) = hovered_character_id {
            if self.characters.get(id).player_controlled()
                && is_mouse_button_pressed(MouseButton::Left)
            {
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
                && !new_selected_char.is_dead()
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

        self.settings
            .draw(0.0, screen_height() - self.settings.size().1);

        let character_ui = self.character_uis.get_mut(&selected_character_id).unwrap();

        character_ui.draw(ui_x0, ui_y + 5.0);

        let log_x = screen_width() - self.log.width();
        let log_y = screen_height() - 300.0; //TODO
        self.log.draw(log_x, log_y);

        self.log.draw_tooltips(log_x, log_y);

        self.target_ui
            .draw(screen_width() - self.target_ui.size().0 - 10.0, 10.0);

        if is_key_pressed(KeyCode::A) {
            self.character_sheet_toggle.toggle_shown();

            if !self.character_sheet_toggle.is_shown() {
                if let Some(button_hovered) = &self.hovered_button {
                    if button_hovered.context == Some(ButtonContext::CharacterSheet) {
                        self.hovered_button = None;
                    }
                }
            }
        }

        if self.character_sheet_toggle.is_shown() {
            let is_showing_active = self.active_character_id == selected_character_id;

            let outcome = character_ui
                .character_sheet
                .draw(&mut self.state.borrow_mut(), is_showing_active);

            self.set_character_sheet_shown(!outcome.clicked_close);

            if outcome.changed_state_or_drag {
                println!("REQUESTED EQ CHANGE; new state");
                // Maybe drag was changed, or maybe the entire state; should be fine to assume the latter
                self.on_new_state();
            }
        } else {
            // Some actions don't make sense to show unless the character sheet is shown
            let mut should_cancel_action = false;
            if matches!(
                *self.state.borrow(),
                UiState::ConfiguringAction(ConfiguredAction::SelectConsumableFromInventory)
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

        handle_keyword_tooltip_end_of_frame();

        player_chose
    }

    fn handle_grid_outcome(&mut self, outcome: GridOutcome) -> Option<PlayerChose> {
        let mut player_chose = None;
        self.top_character_portraits
            .set_grid_hovered_character_id(outcome.hovered_character_id);

        if outcome.committed_action {
            player_chose = Some(self.commit_player_action());
        }

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
                    let total_distance = selected_movement_path.last().unwrap().distance_from_start;
                    let positions = selected_movement_path
                        .iter()
                        .map(|node| node.position)
                        .collect();

                    player_chose = Some(PlayerChose::Action(Some(Box::new(Action::Move {
                        total_distance,
                        positions,
                        extra_cost: *cost,
                    }))));
                }
            }
        }

        if outcome.switched_players_action_target {
            if let UiState::ConfiguringAction(configured_action) = &*self.state.borrow() {
                if configured_action.has_target() {
                    self.sound_player.play(SoundId::SelectTarget);
                }
            }

            self.refresh_actor_direction();
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
        } else {
            self.activity_popup.set_movement_cost(0);
        }

        player_chose
    }

    fn refresh_actor_direction(&self) {
        let mut target_pos = None;
        if let UiState::ConfiguringAction(configured_action) = &*self.state.borrow() {
            if configured_action.has_target() {
                target_pos = match configured_action {
                    ConfiguredAction::Attack {
                        target: Some(character_id),
                        ..
                    } => Some(self.characters.get(*character_id).pos()),
                    ConfiguredAction::UseAbility { target, .. } => match target {
                        ActionTarget::Character(character_id, _) => {
                            Some(self.characters.get(*character_id).pos())
                        }
                        ActionTarget::Position(pos) => Some(*pos),
                        ActionTarget::Projectile(tl) => Some(tl.destination),
                        ActionTarget::Line(tl) => Some(tl.destination),
                        ActionTarget::None => None,
                    },
                    _ => None,
                };
            }
        }

        let character = self.active_character();

        target_pos = target_pos.or_else(|| {
            character
                .engagement_target
                .get()
                .map(|target_id| self.characters.get(target_id).pos())
        });

        if let Some(pos) = target_pos {
            character.set_facing_toward(pos);
        }

        for engager in character.is_engaged_by.borrow().values() {
            engager.set_facing_toward(character.pos());
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
                let target_char = self.characters.get_rc(*target_id);

                let selected_enhancement_effects: Vec<(&'static str, AttackEnhancementEffect)> =
                    selected_enhancements
                        .iter()
                        .map(|e| (e.name, e.effect))
                        .collect();

                let attacker = self.characters.get_rc(self.active_character_id);

                let prediction = predict_attack(
                    &self.game_grid.pathfind_grid,
                    &self.characters,
                    attacker,
                    attack.hand,
                    &selected_enhancement_effects,
                    target_char,
                    None,
                    0,
                );

                self.game_grid.clear_target_effect_previews();
                for (ch_id, damage) in &prediction.area_targets {
                    self.game_grid
                        .set_target_effect_preview(TargetEffectPreview {
                            character_id: *ch_id,
                            prediction: TargetPrediction {
                                damage: *damage,
                                is_buff: false,
                                details: vec![],
                                // TODO
                                expected_roll_outcome: None,
                                effective_armor_value: None,
                            },
                        });
                }
                self.game_grid
                    .set_target_effect_preview(TargetEffectPreview {
                        character_id: *target_id,
                        prediction: TargetPrediction::from(prediction),
                    });
            }

            None => {
                self.game_grid.clear_target_effect_previews();
            }
        }
    }

    fn refresh_use_ability_state(&mut self) {
        // TODO we shouldn't need to "refresh" the line length - that should be handled correctly from inside grid.rs already?
        //self.refresh_line_ability_target();

        let UiState::ConfiguringAction(configured_action) = &*self.state.borrow() else {
            panic!()
        };
        let ConfiguredAction::UseAbility {
            ability,
            selected_enhancements,
            target,
            ..
        } = configured_action
        else {
            panic!()
        };

        self.game_grid.clear_target_effect_previews();

        let usability_problem = configured_action.usability_problem(
            self.active_character(),
            &self.characters,
            &self.game_grid.pathfind_grid,
        );

        if matches!(usability_problem, None | Some(UsabilityProblem::OutOfReach)) {
            let prediction = predict_ability(
                &self.game_grid.pathfind_grid,
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
            let base_action = match btn.action_tooltip.action {
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
                .usability_problem(
                    self.active_character(),
                    &self.characters,
                    &self.game_grid.pathfind_grid,
                )
                .is_none();
            self.set_selected_action(Some((
                ButtonAction::Action(configured_action.base_action()),
                fully_selected,
            )));
        } else if let UiState::Idle {
            recently_committed_action: Some(PlayerChose::Action(Some(action))),
        } = &*self.state.borrow()
        {
            self.set_selected_action(Some((ButtonAction::Action(action.base_action()), true)));
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
                    //self.character_sheet_toggle.set_shown(false);
                }

                _ => {}
            }
        }

        *self.state.borrow_mut() = state;

        self.on_new_state();
    }

    fn on_new_state(&mut self) {
        //dbg!(&self.state.borrow());
        //println!("^^^ on_new_state() ^^^");

        self.activity_popup.additional_line = None;

        self.game_grid.clear_target_effect_previews();

        let mut relevant_tooltip = None;

        let mut is_reacting = None;
        let mut is_reacting_to_attack = false;

        self.sound_player.stop(SoundId::FireCrackle);
        self.sound_player.stop(SoundId::MechanicNoise);

        match &mut *self.state.borrow_mut() {
            UiState::ConfiguringAction(ref mut configured_action) => {
                self.set_allowed_to_use_action_buttons(true);

                // An interesting idea with a "charging up" sound, but needs better (and non-looping) sound effects
                //
                /*
                if let ConfiguredAction::UseAbility { ability, .. } = configured_action {
                    if ability.id == AbilityId::Fireball {
                        self.sound_player.play_looping(SoundId::FireCrackle);
                    } else {
                        self.sound_player.play_looping(SoundId::MechanicNoise);
                    }
                }
                 */

                let tooltip = if let ConfiguredAction::UseAbility {
                    ability,
                    equipment_idx_to_consume: Some(..),
                    ..
                } = configured_action
                {
                    // When the configured ability comes from the "use consumable" base action (that button in the UI)
                    // we have to build a new tooltip here on-the-fly instead of relying on the one tracked by the button.
                    // The "use consumable" button's UI doesn't have any knowledge of the ability, so using that one would
                    // give a useless tooltip
                    Rc::new(ActionTooltip::new(
                        ButtonAction::Action(BaseAction::UseAbility(*ability)),
                        None,
                    ))
                } else {
                    let action_button = self.character_uis[&self.active_character_id]
                        .tracked_action_buttons
                        .get(&button_action_id(ButtonAction::Action(
                            configured_action.base_action(),
                        )))
                        .unwrap();

                    action_button.action_tooltip.clone()
                };

                relevant_tooltip = Some(tooltip);

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

            UiState::ReactingToAttack { reactor, .. } => {
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
                self.set_allowed_to_use_action_buttons(true);
            }

            UiState::Idle { .. } => {
                self.set_allowed_to_use_action_buttons(false);
            }
        }

        if let Some(reactor) = is_reacting {
            self.sound_player.play(SoundId::ChooseReaction);

            self.set_allowed_to_use_action_buttons(false);
            self.player_portraits.set_selected_id(reactor);

            self.refresh_reaction_state();
        }

        self.player_portraits.set_reacting_character(is_reacting);

        self.activity_popup
            .on_new_state(self.active_character_id, relevant_tooltip.as_deref());

        self.maybe_refresh_equipment_state();

        self.refresh_actor_direction();
        self.refresh_target_state();

        self.refresh_selected_action_button();
        self.target_ui.rebuild_character_ui();
    }

    fn refresh_reaction_state(&mut self) {
        let attacker_char;
        let defender_char;
        let mut defender_reaction = None;
        let mut attack_hand = HandType::MainHand;

        self.game_grid.clear_target_effect_previews();

        match &*self.state.borrow() {
            UiState::ReactingToAttack {
                attacker,
                defender,
                reactor,
                hand,
                selected,
                ..
            } => {
                attacker_char = self.characters.get_rc(*attacker);
                defender_char = self.characters.get_rc(*defender);
                attack_hand = *hand;
                defender_reaction = selected.map(|r| (*reactor, r));
            }
            UiState::ReactingToMovementAttackOpportunity {
                reactor,
                target,
                selected,
                ..
            } => {
                if !selected {
                    // No attack
                    return;
                }
                attacker_char = self.characters.get_rc(*reactor);
                defender_char = self.characters.get_rc(*target);
            }
            UiState::ReactingToRangedAttackOpportunity {
                reactor,
                attacker,
                selected,
                ..
            } => {
                if !selected {
                    // No attack
                    return;
                }
                attacker_char = self.characters.get_rc(*reactor);
                defender_char = self.characters.get_rc(*attacker);
            }
            UiState::ReactingToHit { .. } => {
                // No attack
                return;
            }
            _ => unreachable!(),
        }

        let prediction = predict_attack(
            &self.game_grid.pathfind_grid,
            &self.characters,
            attacker_char,
            attack_hand,
            &[],
            defender_char,
            defender_reaction,
            0,
        );
        self.game_grid
            .set_target_effect_preview(TargetEffectPreview {
                character_id: defender_char.id(),
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

            UiState::ConfiguringAction(ConfiguredAction::SelectConsumableFromInventory {
                ..
            }) => {
                self.activity_popup.additional_line =
                    Some("Select a consumable in your inventory by right-clicking it".to_string());
                self.set_character_sheet_shown(true);
            }

            _ => {}
        };
    }

    pub fn has_ongoing_animation(&self) -> bool {
        self.animation_stopwatch.remaining.is_some()
    }

    pub fn handle_game_event(&mut self, event: Box<GameEvent>) {
        self.target_ui.rebuild_character_ui();

        //dbg!(&event);

        match *event {
            GameEvent::LogLine(line) => {
                self.log.add(line);
            }
            GameEvent::CharacterReactedToAttacked { reactor } => {
                self.animate_reaction(reactor);
            }
            GameEvent::CharacterReactedWithOpportunityAttack { reactor } => {
                let reactor = self.characters.get(reactor);

                self.log.add(format!(
                    "|{}| made an opportunity attack:",
                    reactor.name_tag()
                ));
                self.animate_reaction(reactor.id());
            }
            GameEvent::CharacterReactedToHit {
                main_line,
                detail_lines,
                reactor,
            } => {
                let reactor = self.characters.get(reactor);
                self.log.add_with_details(main_line, &detail_lines);
                self.animate_reaction(reactor.id());
            }
            GameEvent::AttackWasInitiated {
                actor,
                target,
                target_reaction,
            } => {
                self.handle_attack_initiated(actor, target, target_reaction);
            }
            GameEvent::Attacked(event) => {
                self.handle_attacked_event(&event, 0);
            }
            GameEvent::AfterOpportunityAttack => {
                // There should be a small pause after an opportunity attack and the action that it interrupted
                self.animation_stopwatch.set_to_at_least(0.5);
            }
            GameEvent::AbilityWasInitiated {
                actor,
                ability,
                destination,
            } => {
                let (delay, duration) =
                    self.game_grid
                        .animate_initiating_ability(actor, &ability, destination);

                if let Some(sound_id) = ability.audio_visuals.initiate_sound {
                    self.sound_player.play_delayed(sound_id, delay);
                }

                self.animation_stopwatch.set_to_at_least(duration);
            }
            GameEvent::AbilityResolved(AbilityResolvedEvent {
                actor,
                target_outcome,
                area_outcome,
                ability,
                extra_logs,
            }) => {
                self.handle_ability_resolved(
                    actor,
                    target_outcome,
                    area_outcome,
                    ability,
                    extra_logs,
                );
            }
            GameEvent::ConsumableWasUsed { user, consumable } => {
                //self.sound_player.play(SoundId::Powerup);
                self.log.add(format!(
                    "|{}| lost item: {}",
                    self.characters.get(user).name_tag(),
                    consumable.name
                ));
            }
            GameEvent::CharactersDying { characters } => {
                let mut someone_died = false;
                for char_id in characters {
                    let char = self.characters.get(char_id);
                    if !char.has_escaped_from_battle.get() {
                        self.game_grid.animate_death(char_id);
                        someone_died = true;
                    }
                }
                if someone_died {
                    self.sound_player.play(SoundId::Death);
                }
            }
            GameEvent::CharactersDied {
                characters,
                new_active,
            } => {
                for char_id in characters {
                    let char = self.characters.get(char_id);
                    if !char.has_escaped_from_battle.get() {
                        self.log.add(format!(
                            "|{}| died |<status>{:?}|",
                            char.name_tag(),
                            StatusId::Dead
                        ));
                    }
                }

                self.target_ui.clear_character_if_dead();

                self.game_grid.remove_dead();
                self.top_character_portraits.remove_dead();

                self.animation_stopwatch.set_to_at_least(0.5);

                if let Some(new_active) = new_active {
                    self.set_new_active_character_id(new_active);
                }
            }
            GameEvent::PlayerGainedMoney { amount, source } => {
                self.log.add_with_details(
                    format!("Player gained money: |<value>{}|", amount),
                    &[format!("Source: |<keyword>{}|", source)],
                );
                self.sound_player.play(SoundId::Coin);
                for char in self.characters.iter() {
                    if char.player_controlled() {
                        self.game_grid.add_text_effect(
                            char.id(),
                            0.0,
                            TextEffect::Effect {
                                text: "Treasure".to_string(),
                                icon: Some(StatusId::Treasure),
                                positive: true,
                            },
                        );
                    }
                }
            }
            GameEvent::CharacterEndedTheirTurn { gained_ap } => {
                if self.active_character().player_controlled() {
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
                } else {
                    // Slight pause after bot ends their turn
                    self.animation_stopwatch.set_to_at_least(0.5);
                }
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
                        // If all player chars are dead, we'll want to immediately show "Defeat" instead of
                        // first showing "Enemy turn"
                        if self.characters.any_alive_player_chars() {
                            self.banner.set("Enemy turn", 1.5);
                        }
                    }
                    self.animation_stopwatch.set_to_at_least(0.6);
                }
            }

            GameEvent::MovementWasInitiated {
                character,
                positions,
            } => {
                // Don't show preview for player movement; they issued it and it should feel snappy.
                if !self.characters.get(character).player_controlled() {
                    self.game_grid
                        .set_character_motion_preview(character, positions, 0.4);

                    self.animation_stopwatch.set_to_at_least(0.3);
                }
            }
            GameEvent::CharacterBalancedOnEdge {
                character,
                direction,
            } => {
                let _duration = self
                    .game_grid
                    .animate_character_balancing_on_edge(character, direction);
                //self.animation_stopwatch.set_to_at_least(duration);
            }
            GameEvent::CharactersCollided { collisions } => {
                let mut delay = 0.0;
                for collision in collisions {
                    let damage_source = DamageSource::KnockbackCollision {
                        move_direction: collision.move_direction,
                    };
                    self.handle_effect_was_applied(
                        delay,
                        collision.character_id,
                        AppliedEffect::LostHealth(
                            collision.damage,
                            damage_source,
                            DamageType::Regular,
                        ),
                        &[collision.description],
                    );
                    delay += 0.15;
                }
                //self.animation_stopwatch.set_to_at_least(0.1);
            }
            GameEvent::ManyMoved(moved_events) => {
                let mut max_duration: f32 = 0.0;
                for e in &moved_events {
                    let duration = self.calculate_movement_duration(
                        e.character,
                        e.from,
                        e.to,
                        e.movement_type,
                    );
                    max_duration = max_duration.max(duration);
                }

                for e in moved_events {
                    // Since the characters are meant to perform synchronized movement, we must slow down anyone who's faster
                    // to prevent them from finishing their (partial) move before the others and then standing still waiting
                    // for the others to catch up.
                    // For example, falling a long distance still just counts as one "movement step", and therefore travels
                    // at higher speed
                    self.do_movement(
                        e.character,
                        e.from,
                        e.to,
                        e.movement_type,
                        e.step_idx,
                        max_duration,
                    );
                }

                self.animation_stopwatch.set_to_at_least(max_duration);
            }
            GameEvent::Moved(MovedEvent {
                character,
                from,
                to,
                movement_type,
                step_idx,
            }) => {
                let duration = self.calculate_movement_duration(character, from, to, movement_type);
                self.do_movement(character, from, to, movement_type, step_idx, duration);
                self.animation_stopwatch.set_to_at_least(duration);
            }
            GameEvent::CharactersSpawning { characters } => {
                let mut duration = 0.0;
                for ch_id in characters {
                    duration = self.game_grid.animate_character_spawn_from_ground(ch_id);
                }
                self.sound_player.play_delayed(SoundId::UndeadRise, 0.5);

                self.animation_stopwatch.set_to_at_least(duration);
            }
            GameEvent::CharacterEffectWasApplied {
                character,
                applied_effect,
                source,
            } => {
                self.handle_effect_was_applied(
                    0.0,
                    character,
                    applied_effect,
                    &[format!("Source: |<keyword>{source}|")],
                );
            }
            GameEvent::GameOver(type_) => {
                let text = match type_ {
                    GameOverType::Victory => "Victory",
                    GameOverType::Defeat => "Defeat",
                };
                match type_ {
                    GameOverType::Victory => self.sound_player.play(SoundId::Victory),
                    GameOverType::Defeat => self.sound_player.play(SoundId::Defeat),
                }
                self.banner.set(text, 2.5);

                //self.animation_stopwatch.set_to_at_least(3.0);
            }
            GameEvent::SurfaceWasConverted {
                positions,
                from,
                to,
            } => {
                self.game_grid.convert_surface(positions, from, to);
                if matches!(to, Surface::Poison) {
                    self.sound_player.play(SoundId::Poison);
                }
            }
            GameEvent::SurfaceWasCreated { positions, surface } => {
                for pos in positions {
                    self.game_grid.set_surface(pos, surface);
                }
            }
            GameEvent::CharacterChangedEquipment => {
                self.sound_player.play(SoundId::DropEquipment);
            }
        }
    }

    fn do_movement(
        &mut self,
        mover_id: u32,
        from: Position,
        to: Position,
        movement_type: MovementType,
        step_idx: u32,
        duration: f32,
    ) {
        let surface = self.game_grid.pathfind_grid.surface(to);
        self.game_grid.set_character_motion(
            mover_id,
            from,
            to,
            duration,
            movement_type,
            step_idx,
            surface,
        );
        if movement_type == MovementType::Falling {
            self.game_grid
                .animate_character_speaking(mover_id, 0.0, duration, "Aaahh");
        }
        if movement_type == MovementType::Dash || movement_type == MovementType::Falling {
            self.sound_player.play(SoundId::Dash);
        } else if movement_type != MovementType::KnockedBack && step_idx % 2 == 0 {
            if surface.map(|s| s.is_liquid()).unwrap_or(false) {
                self.sound_player.play(SoundId::WalkWater);
            } else {
                self.sound_player.play(SoundId::Walk);
            }
        }
    }

    fn calculate_movement_duration(
        &mut self,
        mover_id: u32,
        from: Position,
        to: Position,
        movement_type: MovementType,
    ) -> f32 {
        let mover = self.characters.get(mover_id);
        let base_duration = if self.faster_movement.get() {
            //0.07
            0.04
        } else {
            0.10
        };

        let mut duration = base_duration;

        let surface = self.game_grid.pathfind_grid.surface(to);

        if movement_type == MovementType::Dash {
            // Dash makes an immediate jump between two positions that aren't necessarily adjacent,
            let dist = distance_between(from, to);
            // Dash should appear significantly faster than regular movement
            duration = base_duration * dist * 0.5;
        } else if movement_type == MovementType::Falling {
            // Falling makes an immediate jump between two positions that aren't necessarily adjacent,
            let dist = distance_between(from, to);
            // Falling in slow motion
            duration = base_duration * dist * 1.5;
        } else {
            if from.0 != to.0 || from.1 != to.1 {
                // diagonal takes longer
                duration *= 1.41;
            }

            // Indicate when the character has a temporarily modified move speed (from some Condition)
            if movement_type != MovementType::KnockedBack {
                duration /= mover.move_speed_modifier();
            }

            if movement_type == MovementType::AbilityEngage {
                // Ability engage (e.g. lunge attack) should appear faster than regular movement, and is not slowed down by special surface
                duration *= 0.7;
            } else if surface.is_some() && movement_type != MovementType::KnockedBack {
                duration *= MOVE_COST_FACTOR_IN_LIQUID;
            }
        }
        duration
    }

    fn handle_effect_was_applied(
        &mut self,
        start_time: f32,
        character: u32,
        applied_effect: AppliedEffect,
        detail_lines: &[String],
    ) {
        self.log_applied_effect(character, applied_effect, detail_lines);
        self.show_applied_effect(applied_effect, character, start_time);
    }

    fn log_applied_effect(
        &mut self,
        character: u32,
        applied_effect: AppliedEffect,
        detail_lines: &[String],
    ) {
        //dbg!(character, applied_effect, detail_lines);
        let name_tag = self.characters.get(character).name_tag();
        let verb_phrase = match applied_effect {
            AppliedEffect::LostActionPoints(n) => "".to_string(),
            AppliedEffect::GainedActionPoints(n) => {
                format!("gained {n} AP")
            }
            AppliedEffect::LostHealth(n, source, dmg_type) => match source {
                DamageSource::Attack(..) | DamageSource::Ability(..) => {
                    format!("was hit for |<value>{}| {}", n, dmg_type.dmg_str())
                }
                DamageSource::KnockbackCollision { .. } => {
                    format!("took |<value>{}| collision damage", n)
                }
                DamageSource::Falling => format!("took |<value>{}| falling damage", n),
                _ => format!("lost |<value>{}| health", n),
            },
            AppliedEffect::GainedHealth(n) => {
                format!("gained |<value>{}| health", n,)
            }
            AppliedEffect::GainedStamina(n) => format!("gained {n} stamina"),
            AppliedEffect::GainedMana(n) => format!("gained {n} mana"),
            AppliedEffect::GainedCondition(ApplyCondition {
                condition,
                stacks,
                duration_rounds,
            }) => {
                let mut line = format!(
                    "gained: |<status>{:?}| |<keyword>{}|",
                    condition.status_id(),
                    condition.name()
                );

                if let Some(stacks) = stacks {
                    line.push_str(&format!(" x{}", stacks));
                }

                if let Some(duration) = duration_rounds {
                    line.push_str(&format!(" ({})", duration));
                }

                line
            }
            AppliedEffect::LostCondition { condition } => {
                format!(
                    "lost: |<status>{:?}| |<keyword>{}|",
                    condition.status_id(),
                    condition.name()
                )
            }
            AppliedEffect::Dodged => "dodged".to_string(),
            AppliedEffect::WasPushed(_) => "was pushed".to_string(),
            AppliedEffect::Escaped => todo!(),
        };

        if !verb_phrase.is_empty() {
            let line = format!("|{name_tag}| {verb_phrase}");
            if detail_lines.is_empty() {
                self.log.add(line);
            } else {
                self.log.add_with_details(line, detail_lines);
            }
        }
    }

    fn animate_reaction(&mut self, reactor: CharacterId) {
        self.game_grid
            .add_text_effect(reactor, 0.0, TextEffect::ReactionExclamation);
        self.sound_player.play(SoundId::React);
        self.animation_stopwatch.set_to_at_least(0.5);
    }

    fn handle_area_outcomes(
        &mut self,
        label: &str,
        actor: CharacterId,
        start_time: f32,
        animation_color: Color,
        circle: Option<ConcreteCircle>,
        outcomes: &[DecoratedTargetOutcome],
        ability: Option<&Ability>,
    ) {
        let area_duration = 1.5;

        let ability_id = ability.map(|a| a.id);
        if let Some(circle) = circle {
            let mut show_area_circle = true;
            if matches!(ability_id, Some(AbilityId::Inspire) | Some(AbilityId::Kill)) {
                show_area_circle = false;
            }
            // TODO: This type of rendering code should be moved to grid along with the ability initation rendering code
            if show_area_circle {
                self.game_grid.add_effect(
                    circle.center,
                    circle.center,
                    EffectData {
                        start_time,
                        end_time: start_time + area_duration,
                        variant: EffectVariant::At(
                            EffectPosition::Destination,
                            EffectGraphics::Circle {
                                radius: f32::from(circle.radius) * self.game_grid.cell_w() * 0.1,
                                end_radius: Some(
                                    f32::from(circle.radius) * self.game_grid.cell_w(),
                                ),
                                //fill: Some(Color::new(1.0, 1.0, 1.0, 0.3)),
                                fill: Some(animation_color.with_alpha(0.5)),
                                //fill: Some(fill_color),
                                stroke: None,
                                fade: true,
                            },
                        ),
                    },
                );
            }
        }

        let mut delay = 0.0;
        for (i, o) in outcomes.iter().enumerate() {
            let details_header = format!("|<keyword>{}| |<faded>AoE (target {})|:", label, i + 1);
            let mut lines = vec![details_header];
            lines.extend_from_slice(&o.detail_lines);

            //dbg!((i, o));

            let mut effects = vec![];
            o.outcome
                .collect_applied_effects(actor, o.target_id, &mut effects);

            if let Some(a) = ability {
                if o.outcome.hit_type() != HitType::Miss {
                    self.game_grid.animate_ability_target_hit(
                        o.target_id,
                        &a.audio_visuals,
                        start_time + delay,
                    );
                }
            }

            if o.outcome.hit_type() == HitType::Critical {
                self.game_grid.add_text_effect(
                    o.target_id,
                    start_time + delay,
                    TextEffect::CriticalHit,
                );
            }

            for (ch_id, e) in effects {
                self.handle_effect_was_applied(start_time + delay, ch_id, e, &lines);
            }

            delay += 0.1;
        }
    }

    fn handle_ability_resolved(
        &mut self,
        actor_id: CharacterId,
        target_outcome: Option<DecoratedTargetOutcome>,
        area_outcome: Option<AbilityAreaOutcome>,
        ability: &'static Ability,
        extra_logs: Vec<String>,
    ) {
        // TODO
        //dbg!(&target_outcome, &area_outcome);

        if let Some(sound_id) = ability.audio_visuals.initiate_sound {
            self.sound_player.stop(sound_id);
        }

        let actor = self.characters.get(actor_id);
        let actor_name_tag = actor.name_tag();
        if ability.id == AbilityId::EnemyEscape {
            let gold = actor.get_condition_stacks(Condition::Treasure);
            let with_gold = if gold > 0 {
                format!(" with |<value>{}| gold", gold)
            } else {
                "".to_string()
            };
            let line = format!("|{}| escaped{}", actor_name_tag, with_gold);

            self.log.add(line);
            return;
        }

        let verb = if matches!(ability.roll, Some(AbilityRollType::Spell)) {
            "cast"
        } else {
            "used"
        };
        let mut line = format!("|{}| {} |<keyword>{}|", actor_name_tag, verb, ability.name);
        if let Some(o) = &target_outcome {
            let target_name_tag = self.characters.get(o.target_id).name_tag();
            line.push_str(&format!(" on |{}|", target_name_tag));
            self.log.add_with_details(line, &o.detail_lines);
        } else {
            if let Some(o) = &area_outcome {
                let num_targets = o.targets.len();
                let suffix = if num_targets > 1 { "s" } else { "" };
                line.push_str(&format!(" |<faded>({} target{})|", num_targets, suffix));
            }
            self.log.add(line);
        }

        if let Some(sound_id) = ability.audio_visuals.resolve_sound {
            self.sound_player.play(sound_id);
        }

        if let Some(o) = &target_outcome {
            let mut effects = vec![];
            o.outcome
                .collect_applied_effects(actor_id, o.target_id, &mut effects);
            let mut detail_lines = vec![format!("|<keyword>{}|:", ability.name)];
            detail_lines.extend_from_slice(&o.detail_lines);
            for (ch_id, e) in effects {
                self.handle_effect_was_applied(0.0, ch_id, e, &detail_lines);
            }
            if o.outcome.hit_type() != HitType::Miss {
                self.game_grid
                    .animate_ability_target_hit(o.target_id, &ability.audio_visuals, 0.0);
            }
            if o.outcome.hit_type() == HitType::Critical {
                self.game_grid
                    .add_text_effect(o.target_id, 0.0, TextEffect::CriticalHit);
            }
            //self.animation_stopwatch.set_to_at_least(0.15);
        }

        if let Some(AbilityAreaOutcome { targets, circle }) = &area_outcome {
            self.handle_area_outcomes(
                ability.name,
                actor_id,
                0.0,
                ability.audio_visuals.animation_color,
                *circle,
                targets,
                Some(ability),
            );
        }

        for line in extra_logs {
            self.log.add(line);
        }

        // Tiny delay before we proceed to e.g. pushing targets
        self.animation_stopwatch.set_to_at_least(0.1);
    }

    fn handle_attack_initiated(
        &mut self,
        attacker: CharacterId,
        target: CharacterId,
        target_reaction: Option<(CharacterId, bool)>,
    ) {
        let ranged = self.characters.get(attacker).has_equipped_ranged_weapon();

        let (delay, duration) =
            self.game_grid
                .animate_character_attacking(attacker, target, ranged, target_reaction);

        if ranged {
            self.sound_player.play_delayed(SoundId::ShootArrow, delay);
        }

        self.animation_stopwatch.set_to_at_least(duration);
    }

    fn play_attack_hit_sound(&self, attack_type: AttackType, hit_type: HitType, start_time: f32) {
        let sound = match attack_type {
            AttackType::Melee => SoundId::MeleeAttack,
            AttackType::Ranged => SoundId::HitArrow,
        };
        self.sound_player.play_delayed(sound, start_time);
        if matches!(hit_type, HitType::Critical) {
            self.sound_player
                .play_delayed(SoundId::Crit, start_time + 0.02);
        }
    }

    fn handle_attacked_event(&mut self, event: &AttackedEvent, attack_index: usize) {
        //dbg!(event);

        let attacker_id = event.attacker;
        let target = event.target;
        let detail_lines = &event.detail_lines;

        let start_time = attack_index as f32 * 0.13;

        let target_pos = self.characters.get(target).pos();

        let attacker = self.characters.get(attacker_id);

        if event.outcome.hit_type != HitType::Miss {
            let attack_type = attacker.equipped_weapon_type().unwrap();
            self.play_attack_hit_sound(attack_type, event.outcome.hit_type, start_time);
        }
        if event.outcome.hit_type == HitType::Critical {
            self.game_grid
                .add_text_effect(target, 0.0, TextEffect::CriticalHit);
        }

        let line = format!(
            "|{}| attacked |{}|",
            attacker.name_tag(),
            self.characters.get(target).name_tag()
        );

        self.log.add_with_details(line, detail_lines);

        let mut effects = vec![];
        // Don't collect circle effects here - they are handled separately further down (code reuse with ability AoE effects)
        event.collect_applied_effects(attacker_id, target, &mut effects, IncludeCircleEffects::No);

        for (ch_id, e) in effects {
            self.handle_effect_was_applied(0.0, ch_id, e, detail_lines);
        }

        if let Some(effect_id) = event.outcome.visual_effect {
            self.game_grid.animate_effect(target, 0.5, effect_id);
        }

        if let Some((name, radius, outcomes)) = &event.circle_outcome {
            self.handle_area_outcomes(
                &format!("Attack - {}", name),
                attacker_id,
                start_time,
                RED,
                Some(ConcreteCircle {
                    center: target_pos,
                    radius: *radius,
                }),
                outcomes,
                None,
            );
        }

        self.animation_stopwatch.set_to_at_least(0.1);
    }

    fn show_applied_effect(
        &mut self,
        applied_effect: AppliedEffect,
        character_id: CharacterId,
        start_time: f32,
    ) -> bool {
        match applied_effect {
            AppliedEffect::LostActionPoints(n) => {
                self.game_grid.animate_character_lost_ap(character_id, n);
            }
            AppliedEffect::GainedActionPoints(n) => {
                self.sound_player
                    .play_delayed(SoundId::GainedAP, start_time);
            }
            AppliedEffect::LostHealth(amount, source, _dmg_type) => {
                let character = self.characters.get(character_id);
                if amount > 0 {
                    if source == DamageSource::Condition(Condition::Burning) {
                        self.sound_player.play_delayed(SoundId::Burning, start_time);
                    }
                    self.sound_player
                        .play_delayed(character.hurt_sound, start_time);
                    let prev_health = self.characters.get(character_id).health.current() + amount;
                    self.game_grid
                        .animate_character_health_change(character_id, prev_health, 0.9);

                    match source {
                        DamageSource::KnockbackCollision { move_direction } => {
                            if move_direction != (0, 0) {
                                self.game_grid.animate_character_bumping_into_something(
                                    character_id,
                                    start_time,
                                    move_direction,
                                );
                            } else {
                                self.game_grid
                                    .animate_character_shaking(character_id, start_time);
                                self.game_grid
                                    .animate_effect(character_id, 0.5, EffectId::Pow);
                            }
                        }
                        DamageSource::Falling => {
                            self.game_grid
                                .animate_character_shaking(character_id, start_time);
                            self.game_grid
                                .animate_effect(character_id, 0.5, EffectId::Pow);
                        }
                        DamageSource::Attack(attack_type, hit_type) => {
                            self.game_grid
                                .animate_character_shaking(character_id, start_time);
                            self.play_attack_hit_sound(attack_type, hit_type, start_time);
                        }

                        _ => {}
                    }
                } else {
                    if matches!(source, DamageSource::Attack(..)) {
                        self.sound_player
                            .play_delayed(SoundId::ArmorAbsorbed, start_time);
                        self.game_grid
                            .animate_character_shaking(character_id, start_time);
                    }
                }
            }
            AppliedEffect::Dodged => {
                self.sound_player
                    .play_delayed(SoundId::AttackMiss, start_time);
                self.game_grid
                    .animate_character_shaking(character_id, start_time);
            }
            AppliedEffect::GainedCondition(ApplyCondition { condition, .. }) => match condition {
                Condition::Ferocity => {
                    self.sound_player.play_delayed(SoundId::Execute, start_time);
                }
                Condition::Burning => {
                    self.sound_player.play_delayed(SoundId::Burning, start_time);
                }
                _ => {}
            },
            _ => {}
        };

        self.game_grid.add_text_effect(
            character_id,
            start_time,
            TextEffect::ApplyEffect(applied_effect),
        )
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
        let game_speed = if self.slow_motion.get() { 0.1 } else { 1.0 };
        let elapsed = elapsed * game_speed;

        self.sound_player.update(elapsed);

        self.set_allowed_to_use_action_buttons(
            self.player_portraits.selected_id() == self.active_character_id,
        );

        for character_ui in self.character_uis.values_mut() {
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

        let usability_problem = self.state.borrow().action_usability_problem(
            self.active_character(),
            &self.characters,
            &self.game_grid.pathfind_grid,
        );

        self.game_grid.update(
            self.active_character_id,
            selected_in_grid,
            usability_problem,
            elapsed,
        );

        let popup_outcome = self.activity_popup.update();

        let mut player_choice = None;
        if let Some(outcome) = popup_outcome {
            match outcome {
                ActivityPopupOutcome::ClickedProceed => {
                    player_choice = Some(self.commit_player_action());
                }
                ActivityPopupOutcome::ChangedAbilityEnhancements => {
                    // TODO update hit chance?
                    self.refresh_use_ability_state();
                }
                ActivityPopupOutcome::ChangedAttackEnhancements => {
                    self.refresh_attack_state();
                }
                ActivityPopupOutcome::ChangedReaction => {
                    self.refresh_reaction_state();
                }
                ActivityPopupOutcome::ClickedInvalid => {
                    self.on_invalid_action_clicked();
                }
            }
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
                        match btn.action_tooltip.action {
                            ButtonAction::Action(base_action) => {
                                self.sound_player.play(SoundId::ClickButton);
                                action_button_clicked = Some(base_action)
                            }
                            _ => unreachable!(
                                "button clicked via hotkey: {:?}",
                                btn.action_tooltip.action
                            ),
                        }
                    } else {
                        invalid_action = true;
                    }
                    btn.on_clicked();
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
                /*if base_action == BaseAction::ToggleQuickActions {
                    assert!(player_choice.is_none());
                    println!("Toggling quick actions");
                    self.active_character().toggle_quick_actions();
                    // Movement range/speed is affected by either stamina or AP, depending on 'quick actions'
                    self.game_grid.update_move_speed(self.active_character_id);
                } else
                 */
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
                    println!("warning: unhandled base action: {:?}", base_action);
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

        let enabled_quick_actions = self.active_character().enabled_quick_actions.get();

        let character_ui = self
            .character_uis
            .get_mut(&self.player_portraits.selected_id())
            .unwrap();

        character_ui
            .mana_bar
            .borrow_mut()
            .reset_hovered_and_reserved_amounts();
        character_ui
            .stamina_bar
            .borrow_mut()
            .reset_hovered_and_reserved_amounts();

        let (reserved_sta, hovered_sta) = self.activity_popup.reserved_and_hovered_stamina();
        let (reserved_mana, hovered_mana) = self.activity_popup.reserved_and_hovered_mana();

        character_ui
            .mana_bar
            .borrow_mut()
            .set_reserved_amount(reserved_mana);
        character_ui
            .mana_bar
            .borrow_mut()
            .set_hovered_amount(hovered_mana);
        character_ui
            .stamina_bar
            .borrow_mut()
            .set_reserved_amount(reserved_sta);
        character_ui
            .stamina_bar
            .borrow_mut()
            .set_hovered_amount(hovered_sta);

        // HOVERING END OF TURN
        if self.player_portraits.is_hovering_end_turn.get() {
            let ap_gain = self
                .characters
                .get(self.active_character_id)
                .end_of_turn_ap_gain() as i32;
            character_ui.action_points_row.reserved_and_hovered_ap = (0, -ap_gain);
        }
        // HOVERING ACTION
        else if let Some(hovered_btn) = &self.hovered_button {
            if hovered_btn.context != Some(ButtonContext::CharacterSheet) {
                let mut hovered_ap = hovered_btn.action.action_point_cost();
                if !enabled_quick_actions {
                    hovered_ap += hovered_btn.action.quick_point_cost() as i32;
                }
                character_ui.action_points_row.reserved_and_hovered_ap = (
                    self.activity_popup.reserved_and_hovered_action_points().0,
                    hovered_ap,
                );
                character_ui
                    .mana_bar
                    .borrow_mut()
                    .set_hovered_amount(hovered_btn.action.mana_cost());

                let mut hovered_sta = hovered_btn.action.stamina_cost();
                if enabled_quick_actions {
                    hovered_sta += hovered_btn.action.quick_point_cost();
                }
                character_ui
                    .stamina_bar
                    .borrow_mut()
                    .set_hovered_amount(hovered_sta);
            }
        }
        // CONFIGURING ACTION ETC
        else {
            character_ui.action_points_row.reserved_and_hovered_ap =
                self.activity_popup.reserved_and_hovered_action_points();
        };

        self.animation_stopwatch.update(elapsed);

        player_choice
    }

    fn on_invalid_action_clicked(&mut self) {
        self.sound_player.play(SoundId::Invalid);
        let selected_char_id = self.player_portraits.selected_id();
        if !self.characters.get(selected_char_id).is_dead() {
            self.game_grid
                .animate_character_speaking(selected_char_id, 0.0, 0.7, "Can't do that!");
        }
    }

    fn commit_player_action(&mut self) -> PlayerChose {
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
                        equipment_idx_to_consume,
                    } => Some(Action::UseAbility {
                        ability,
                        enhancements: selected_enhancements.clone(),
                        target: target.clone(),
                        equipment_index_to_consume: *equipment_idx_to_consume,
                    }),
                    &ConfiguredAction::Move {
                        cost,
                        selected_movement_path,
                    } => {
                        let total_distance =
                            selected_movement_path.last().unwrap().distance_from_start;
                        let positions: Vec<Position> = selected_movement_path
                            .iter()
                            .map(|node| node.position)
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
                    &ConfiguredAction::SelectConsumableFromInventory { .. } => {
                        unreachable!("This is not a committable action. After consumable has been selected, we transition to UseAbility");
                    }
                };
                PlayerChose::Action(action.map(Box::new))
            }

            UiState::ReactingToAttack { selected, .. } => PlayerChose::AttackedReaction(*selected),
            UiState::ReactingToHit { selected, .. } => PlayerChose::HitReaction(*selected),
            UiState::ReactingToMovementAttackOpportunity { selected, .. } => {
                PlayerChose::OpportunityAttack(*selected)
            }
            UiState::ReactingToRangedAttackOpportunity { selected, .. } => {
                PlayerChose::OpportunityAttack(*selected)
            }

            UiState::ChoosingAction | UiState::Idle { .. } => unreachable!(),
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
    event_queue: &Rc<RefCell<Vec<InternalUiEvent>>>,
    simple_font: &Font,
    big_font: &Font,
    characters: impl Iterator<Item = &'a Rc<Character>>,
    sound_player: SoundPlayer,
) -> HashMap<u32, CharacterUi> {
    let mut next_button_id = 1;

    let mut character_uis: HashMap<CharacterId, CharacterUi> = Default::default();

    // Will be overridden by the character sheets when they are constructed and know their own dimensions
    let character_sheet_screen_pos = Rc::new(RefCell::new((0.0, 0.0)));

    for character in characters {
        if !character.player_controlled() {
            continue;
        }

        let character_ui = build_character_ui(
            event_queue,
            simple_font,
            big_font,
            character,
            &mut next_button_id,
            Rc::clone(&character_sheet_screen_pos),
            sound_player.clone(),
        );

        character_uis.insert(character.id(), character_ui);
    }
    character_uis
}

fn build_character_ui(
    event_queue: &Rc<RefCell<Vec<InternalUiEvent>>>,
    simple_font: &Font,
    big_font: &Font,
    character: &Rc<Character>,
    next_button_id: &mut u32,
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

    let mut tracked_action_buttons = IndexMap::new();
    let mut hoverable_buttons = vec![];
    let mut basic_buttons = vec![];
    let mut change_eq_btn = None;
    let mut use_consumable_btn = None;
    //let mut toggle_quick_btn = None;
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
    let hotkey_font = big_font;
    let ability_hotkeys = [KeyCode::Q, KeyCode::W, KeyCode::E, KeyCode::R, KeyCode::T];

    for action in character.known_actions() {
        let btn_action = ButtonAction::Action(action);
        let btn = new_button(btn_action, Some(character.clone()), false);
        //btn.set_parent_bg_color(buttons_row_bg);
        let btn = Rc::new(btn);
        tracked_action_buttons.insert(button_action_id(btn_action), Rc::clone(&btn));
        hoverable_buttons.push(Rc::clone(&btn));
        match action {
            BaseAction::Attack { .. } => {
                *btn.hotkey.borrow_mut() = basic_hotkeys
                    .get(basic_buttons.len())
                    .map(|key| (*key, hotkey_font.clone()));
                basic_buttons.push(btn);

                let btn = Rc::new(new_button(btn_action, Some(character.clone()), true));
                attack_button_for_character_sheet = Some(btn.clone());
                hoverable_buttons.push(btn);
            }
            BaseAction::UseAbility(ability) => {
                *btn.hotkey.borrow_mut() = ability_hotkeys
                    .get(ability_buttons.len())
                    .map(|key| (*key, hotkey_font.clone()));
                ability_buttons.push(btn);

                let btn = Rc::new(new_button(btn_action, Some(character.clone()), true));

                let enhancement_buttons: Vec<Rc<ActionButton>> = ability
                    .possible_enhancements
                    .iter()
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
                    .map(|key| (*key, hotkey_font.clone()));
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
    for reaction in character.inherently_known_on_attacked_reactions() {
        let btn_action = ButtonAction::OnAttackedReaction(reaction);
        let btn = Rc::new(new_button(btn_action, None, true));
        hoverable_buttons.push(Rc::clone(&btn));
        reaction_buttons_for_character_sheet.push(btn);
    }
    for (_subtext, reaction) in character.inherently_known_on_hit_reactions() {
        let btn_action = ButtonAction::OnHitReaction(reaction);
        let btn = Rc::new(new_button(btn_action, None, true));
        hoverable_buttons.push(Rc::clone(&btn));
        reaction_buttons_for_character_sheet.push(btn);
    }

    for (_subtext, enhancement) in character.inherently_known_attack_enhancements() {
        let btn_action = ButtonAction::AttackEnhancement(enhancement);
        let btn = Rc::new(new_button(btn_action, None, true));
        hoverable_buttons.push(Rc::clone(&btn));
        attack_enhancement_buttons_for_character_sheet.push(btn);
    }

    for passive_skill in character.known_passive_skills.borrow().iter() {
        let btn_action = ButtonAction::Passive(*passive_skill);
        let btn = Rc::new(new_button(btn_action, Some(character.clone()), true));
        hoverable_buttons.push(Rc::clone(&btn));
        passive_buttons_for_character_sheet.push(Rc::clone(&btn));
    }

    let character_sheet = CharacterSheet::new(
        simple_font,
        big_font,
        Rc::clone(character),
        attack_button_for_character_sheet,
        reaction_buttons_for_character_sheet,
        attack_enhancement_buttons_for_character_sheet,
        ability_buttons_for_character_sheet,
        passive_buttons_for_character_sheet,
        ConditionsList::new(simple_font.clone(), vec![]),
        character_sheet_screen_pos,
        sound_player,
    );

    let mut buttons = basic_buttons;
    buttons.extend_from_slice(&ability_buttons);
    let mut buttons: Vec<Element> = buttons.into_iter().map(|btn| Element::Rc(btn)).collect();
    let button_placeholder = Rectangle {
        size: REGULAR_ACTION_BUTTON_SIZE,
        style: Style {
            border: Some(Border::with_color(GRAY)),
            bg_color: Some(ACTION_BUTTON_BG_COLOR),
            ..Default::default()
        },
        ..Default::default()
    };
    while buttons.len() < 8 {
        buttons.push(Element::Rect(button_placeholder.clone()));
    }
    //buttons.push(Element::Rc(toggle_quick_btn.unwrap()));
    buttons.push(Element::Rc(change_eq_btn.unwrap()));
    buttons.push(Element::Rc(use_consumable_btn.unwrap()));

    let button_row = buttons_row(buttons);

    let actions_section = Container {
        layout_dir: LayoutDirection::Vertical,
        margin: 0.0,
        children: vec![button_row],
        style: Style {
            //bg_color: Some(buttons_row_bg),
            padding: 2.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let resource_bars = ResourceBars::new(character, big_font);

    let action_points_row = ActionPointsRow::new(
        (20.0, 20.0),
        0.25,
        Style {
            border: Some(Border::with_color(GRAY)),
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
            COL_RED,
            font.clone(),
            "<heart>",
        )));

        let mana_bar = Rc::new(RefCell::new(LabelledResourceBar::new(
            character.mana.current(),
            character.mana.max(),
            COL_BLUE,
            font.clone(),
            "<mana>",
        )));

        let stamina_bar = Rc::new(RefCell::new(LabelledResourceBar::new(
            character.stamina.current(),
            character.stamina.max(),
            COL_GREEN_1,
            font.clone(),
            "<stamina>",
        )));

        let mut children = vec![Element::RcRefCell(health_bar.clone())];

        if character.stamina.max() > 0 {
            children.push(Element::RcRefCell(stamina_bar.clone()));
        }
        if character.mana.max() > 0 {
            children.push(Element::RcRefCell(mana_bar.clone()));
        }

        let container = Container {
            layout_dir: LayoutDirection::Vertical,
            margin: 9.0,
            align: Align::Start,
            children,
            style: Style {
                border: Some(Border {
                    color: DARKGRAY,
                    inner_rounding: Some(3.0),
                    outer_rounding: Some((BLACK, 2.0)),
                    ..Default::default()
                }),
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
            //BaseAction::ToggleQuickActions => "TOGGLE_QUICK_ACTIONS".to_string(),
        },

        _ => unreachable!(),
    }
}

fn buttons_row(buttons: Vec<Element>) -> Element {
    Element::Container(Container {
        layout_dir: LayoutDirection::Horizontal,
        margin: 4.0,
        children: buttons,
        ..Default::default()
    })
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerChose {
    AttackedReaction(Option<OnAttackedReaction>),
    HitReaction(Option<OnHitReaction>),
    OpportunityAttack(bool),
    Action(Option<Box<Action>>),
    SwitchTo(CharacterId),
}

pub fn draw_rectangle_lines2(x: f32, y: f32, w: f32, h: f32, mut thickness: f32, color: Color) {
    // https://github.com/not-fl3/macroquad/issues/704
    // https://github.com/not-fl3/macroquad/issues/271
    // On MacOS retina display, thickness=1.0 only draws half of the rectangle sides
    if thickness == 1.0 {
        thickness = 2.0;
    }
    macroquad::shapes::draw_rectangle_lines(x, y, w, h, thickness, color);
}
