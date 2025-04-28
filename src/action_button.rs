use std::{
    cell::{Cell, Ref, RefCell},
    collections::HashMap,
    rc::Rc,
};

use macroquad::{
    color::{Color, GOLD, GRAY, GREEN, LIGHTGRAY, SKYBLUE, WHITE, YELLOW},
    input::{is_mouse_button_pressed, mouse_position, MouseButton},
    math::Rect,
    miniquad::window::screen_size,
    shapes::{draw_rectangle, draw_rectangle_lines},
    text::{draw_text_ex, measure_text, Font, TextParams},
    texture::{draw_texture_ex, DrawTextureParams, Texture2D},
};

use crate::{
    base_ui::{draw_debug, Circle, Container, Drawable, Element, LayoutDirection, Style},
    core::{
        ApplyEffect, AttackEnhancement, BaseAction, Character, OnAttackedReaction, OnHitReaction,
        Spell, SpellAllyEffect, SpellContestType, SpellEffect, SpellEnemyEffect, SpellEnhancement,
        SpellEnhancementEffect, SpellTarget, Weapon,
    },
    drawing::draw_dashed_rectangle_lines,
    textures::IconId,
};

pub struct ActionButtonTooltip {
    pub header: String,
    pub description: Option<&'static str>,
    pub technical_description: Vec<String>,
}

fn button_action_tooltip(action: &ButtonAction) -> ActionButtonTooltip {
    match action {
        ButtonAction::Action(base_action) => base_action_tooltip(base_action),
        ButtonAction::AttackEnhancement(enhancement) => ActionButtonTooltip {
            header: format!(
                "{} ({})",
                enhancement.name,
                cost_string(enhancement.action_point_cost, enhancement.stamina_cost, 0)
            ),
            description: Some(enhancement.description),
            technical_description: Default::default(),
        },

        ButtonAction::SpellEnhancement(enhancement) => spell_enhancement_tooltip(enhancement),
        ButtonAction::OnAttackedReaction(reaction) => ActionButtonTooltip {
            header: format!(
                "{} ({})",
                reaction.name,
                cost_string(reaction.action_point_cost, reaction.stamina_cost, 0)
            ),
            description: Some(reaction.description),
            technical_description: Default::default(),
        },
        ButtonAction::OnHitReaction(reaction) => ActionButtonTooltip {
            header: format!(
                "{} ({})",
                reaction.name,
                cost_string(reaction.action_point_cost, 0, 0)
            ),
            description: Some(reaction.description),
            technical_description: Default::default(),
        },

        ButtonAction::Proceed => ActionButtonTooltip {
            header: "Proceed".to_string(),
            description: None,
            technical_description: Default::default(),
        },
    }
}

fn spell_enhancement_tooltip(enhancement: &SpellEnhancement) -> ActionButtonTooltip {
    let mut technical_description = vec![];

    if enhancement.bonus_damage > 0 {
        technical_description.push(format!("+ {} damage", enhancement.bonus_damage));
    }

    if let Some(effect) = enhancement.effect {
        match effect {
            SpellEnhancementEffect::CastTwice => {}
            SpellEnhancementEffect::OnHit(apply_effect) => {
                describe_apply_effect(apply_effect, &mut technical_description);
            }
            SpellEnhancementEffect::IncreasedRangeTenths(tenths) => {
                technical_description.push(format!("+ {} range", tenths as f32 * 0.1));
            }
            SpellEnhancementEffect::IncreaseRadiusTenths(tenths) => {
                technical_description.push(format!("+ {} radius", tenths as f32 * 0.1));
            }
        }
    }

    ActionButtonTooltip {
        header: format!(
            "{} ({})",
            enhancement.name,
            cost_string(enhancement.action_point_cost, 0, enhancement.mana_cost)
        ),
        description: Some(enhancement.description),
        technical_description,
    }
}

fn base_action_tooltip(base_action: &BaseAction) -> ActionButtonTooltip {
    match base_action {
        BaseAction::Attack { .. } => ActionButtonTooltip {
            header: "No weapon equipped".to_string(), // This is replaced on-the-fly if needed
            description: None,
            technical_description: vec![],
        },
        BaseAction::CastSpell(spell) => spell_tooltip(spell),
        BaseAction::Move => ActionButtonTooltip {
            header: "Move".to_string(),
            description: None,
            technical_description: Default::default(),
        },
        BaseAction::ChangeEquipment => ActionButtonTooltip {
            header: "Equip/unequip (1 AP)".to_string(),
            description: None,
            technical_description: Default::default(),
        },
        BaseAction::EndTurn => ActionButtonTooltip {
            header: "End your turn".to_string(),
            description: None,
            technical_description: Default::default(),
        },
    }
}

fn describe_apply_effect(effect: ApplyEffect, technical_description: &mut Vec<String>) {
    match effect {
        ApplyEffect::RemoveActionPoints(n) => {
            technical_description.push(format!("  Loses {}^ AP", n))
        }
        ApplyEffect::Condition(condition) => {
            technical_description.push(format!("  {}", condition.name()))
        }
    }
}

fn spell_tooltip(spell: &Spell) -> ActionButtonTooltip {
    let header = format!(
        "{} ({})",
        spell.name,
        cost_string(spell.action_point_cost, spell.stamina_cost, spell.mana_cost)
    );
    let mut technical_description = vec![];
    match spell.target {
        SpellTarget::Enemy {
            effect,
            impact_area: area,
            range,
        } => {
            technical_description.push(format!("Target enemy (range {})", range));
            describe_spell_enemy_effect(effect, &mut technical_description);

            if let Some((range, effect)) = area {
                technical_description.push(format!("Impact area (radius {})", range));
                describe_spell_enemy_effect(effect, &mut technical_description);
            }
        }

        SpellTarget::Ally { range, effect } => {
            technical_description.push(format!("Target ally (range {})", range));
            describe_spell_ally_effect(effect, &mut technical_description);
        }

        SpellTarget::None {
            self_area,
            self_effect,
        } => {
            if let Some(effect) = self_effect {
                technical_description.push("Self effect".to_string());
                describe_spell_ally_effect(effect, &mut technical_description);
            }

            if let Some((radius, effect)) = self_area {
                match effect {
                    SpellEffect::Enemy(effect) => {
                        technical_description.push(format!("Nearby enemies (radius {})", radius));
                        describe_spell_enemy_effect(effect, &mut technical_description);
                    }
                    SpellEffect::Ally(effect) => {
                        technical_description.push(format!("Nearby allies (radius {})", radius));
                        describe_spell_ally_effect(effect, &mut technical_description);
                    }
                }
            }
        }

        SpellTarget::Area {
            range,
            radius,
            effect,
        } => match effect {
            SpellEffect::Enemy(effect) => {
                technical_description.push(format!("Enemies (range {}, radius {})", range, radius));
                describe_spell_enemy_effect(effect, &mut technical_description);
            }
            SpellEffect::Ally(effect) => {
                technical_description.push(format!("Allies (range {}, radius {})", range, radius));
                describe_spell_ally_effect(effect, &mut technical_description);
            }
        },
    };
    ActionButtonTooltip {
        header,
        description: Some(spell.description),
        technical_description,
    }
}

fn describe_spell_enemy_effect(effect: SpellEnemyEffect, technical_description: &mut Vec<String>) {
    match effect.damage {
        Some((dmg, true)) => technical_description.push(format!("  {}^ damage", dmg)),
        Some((dmg, false)) => technical_description.push(format!("  {} damage", dmg)),
        None => {}
    }

    for apply_effect in effect.on_hit.unwrap_or_default().iter().flatten() {
        describe_apply_effect(*apply_effect, technical_description);
    }

    match effect.contest_type {
        Some(SpellContestType::Mental) => {
            technical_description.push("  [Will] defense".to_string())
        }
        Some(SpellContestType::Projectile) => {
            technical_description.push("  [Evasion] defense".to_string())
        }
        None => {}
    };
}

fn describe_spell_ally_effect(effect: SpellAllyEffect, technical_description: &mut Vec<String>) {
    if effect.healing > 0 {
        technical_description.push(format!("  {}^ healing", effect.healing));
    }

    if let Some(apply) = effect.apply {
        describe_apply_effect(apply, technical_description);
    }
}

pub struct ActionButton {
    pub id: u32,
    pub action: ButtonAction,
    pub size: (f32, f32),
    texture_draw_size: (f32, f32),
    style: Style,
    hover_border_color: Color,
    points_row: Container,
    hovered: Cell<bool>,
    pub enabled: Cell<bool>,
    pub selected: Cell<ButtonSelected>,
    pub event_sender: Option<EventSender>,
    icon: Texture2D,
    tooltip: RefCell<ActionButtonTooltip>,
    character: Option<Rc<Character>>,
    tooltip_is_based_on_equipped_weapon: Cell<Option<Weapon>>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ButtonSelected {
    No,
    Partially,
    Yes,
}

impl ActionButton {
    pub fn new(
        action: ButtonAction,
        event_queue: &Rc<RefCell<Vec<InternalUiEvent>>>,
        id: u32,
        icons: &HashMap<IconId, Texture2D>,
        character: Option<Rc<Character>>,
    ) -> Self {
        let mana_points = action.mana_cost();
        let stamina_points = action.stamina_cost();
        let action_points = action.action_point_cost();
        let icon: IconId = action.icon();
        let tooltip = button_action_tooltip(&action);

        let (size, texture_draw_size) = match action {
            ButtonAction::Proceed => ((64.0, 52.0), (60.0, 48.0)),
            _ => ((64.0, 64.0), (60.0, 48.0)),
        };

        let style = Style {
            background_color: Some(Color::new(0.4, 0.32, 0.21, 1.0)),
            border_color: Some(LIGHTGRAY),
            ..Default::default()
        };
        let hover_border_color = YELLOW;

        let r = 3.0;
        let mut point_icons = vec![];

        for _ in 0..action_points {
            point_icons.push(Element::Circle(Circle { r, color: GOLD }))
        }
        for _ in 0..mana_points {
            point_icons.push(Element::Circle(Circle { r, color: SKYBLUE }))
        }
        for _ in 0..stamina_points {
            point_icons.push(Element::Circle(Circle { r, color: GREEN }))
        }
        let points_row = Container {
            children: point_icons,
            margin: 4.0,
            layout_dir: LayoutDirection::Horizontal,
            style: Style {
                padding: 1.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let icon = icons[&icon].clone();
        Self {
            id,
            action,
            size,
            texture_draw_size,
            style,
            hover_border_color,
            points_row,
            hovered: Cell::new(false),
            enabled: Cell::new(true),
            selected: Cell::new(ButtonSelected::No),
            event_sender: Some(EventSender {
                queue: Rc::clone(event_queue),
            }),
            icon,
            tooltip: RefCell::new(tooltip),
            character,
            tooltip_is_based_on_equipped_weapon: Default::default(),
        }
    }

    pub fn tooltip(&self) -> Ref<ActionButtonTooltip> {
        if let ButtonAction::Action(BaseAction::Attack { hand, .. }) = self.action {
            let equipped_weapon = self.character.as_ref().unwrap().weapon(hand);

            if self.tooltip_is_based_on_equipped_weapon.get() != equipped_weapon {
                *self.tooltip.borrow_mut() = if let Some(weapon) = equipped_weapon {
                    ActionButtonTooltip {
                        header: format!("{} attack ({} AP)", weapon.name, weapon.action_point_cost),
                        description: None,
                        technical_description: vec![
                            format!("{} damage", weapon.damage),
                            "vs Evasion".to_string(),
                        ],
                    }
                } else {
                    ActionButtonTooltip {
                        header: "No weapon equipped".to_string(),
                        description: None,
                        technical_description: vec![],
                    }
                };
            }

            self.tooltip_is_based_on_equipped_weapon
                .set(equipped_weapon);
        }
        self.tooltip.borrow()
    }

    pub fn toggle_selected(&self) {
        match self.selected.get() {
            ButtonSelected::Yes => self.selected.set(ButtonSelected::No),
            ButtonSelected::No => self.selected.set(ButtonSelected::Yes),
            ButtonSelected::Partially => {}
        }
    }

    pub fn set_selected(&self, value: bool) {
        let selected = if value {
            ButtonSelected::Yes
        } else {
            ButtonSelected::No
        };
        self.selected.set(selected);
    }

    pub fn deselect(&self) {
        self.selected.set(ButtonSelected::No);
    }

    pub fn notify_hidden(&self) {
        if self.hovered.get() {
            if let Some(event_sender) = &self.event_sender {
                // Since this button has become hidden, it's no longer hovered
                event_sender.send(InternalUiEvent::ButtonHovered(self.id, self.action, None));
            }
        }
    }
}

fn cost_string(action_points: u32, stamina: u32, mana: u32) -> String {
    let mut s = String::new();
    if action_points > 0 {
        s.push_str(&format!("{action_points} AP"));
    }
    if stamina > 0 {
        if !s.is_empty() {
            s.push_str(", ");
        }
        s.push_str(&format!("{stamina} stamina"));
    }
    if mana > 0 {
        if !s.is_empty() {
            s.push_str(", ");
        }
        s.push_str(&format!("{mana} mana"));
    }
    s
}

impl Drawable for ActionButton {
    fn draw(&self, x: f32, y: f32) {
        let (w, h) = self.size;

        self.style.draw(x, y, self.size);

        let points_row_size = self.points_row.size();
        let points_row_h_pad = 3.0;

        if points_row_size.1 > 0.0 {
            draw_rectangle(
                x + 1.0,
                y + h - points_row_size.1 - points_row_h_pad * 2.0,
                w - 2.0,
                points_row_size.1 + points_row_h_pad * 2.0 - 1.0,
                Color::new(0.1, 0.1, 0.1, 1.0),
            );
        }

        let (mouse_x, mouse_y) = mouse_position();

        let hovered = (x..=x + w).contains(&mouse_x) && (y..=y + h).contains(&mouse_y);
        if hovered != self.hovered.get() {
            self.hovered.set(hovered);
            if let Some(event_sender) = &self.event_sender {
                event_sender.send(InternalUiEvent::ButtonHovered(
                    self.id,
                    self.action,
                    if hovered { Some((x, y)) } else { None },
                ));
            }
        }

        if !self.enabled.get() {
            draw_rectangle(x, y, w, h, Color::new(0.2, 0.0, 0.0, 0.5));
        }

        let params = DrawTextureParams {
            dest_size: Some(self.texture_draw_size.into()),
            //dest_size: Some((48.0, 38.4).into()),
            //dest_size: Some((24.0, 19.2).into()),
            ..Default::default()
        };
        draw_texture_ex(&self.icon, x + 2.0, y + 2.0, WHITE, params);

        match self.selected.get() {
            ButtonSelected::Yes => {
                let margin = 1.0;
                draw_rectangle_lines(
                    x - margin,
                    y - margin,
                    w + margin * 2.0,
                    h + margin * 2.0,
                    4.0,
                    GREEN,
                );
            }
            ButtonSelected::Partially => {
                let margin = -0.0;
                draw_dashed_rectangle_lines(
                    x - margin,
                    y - margin,
                    w + margin * 2.0,
                    h + margin * 2.0,
                    4.0,
                    GREEN,
                    6.4,
                );
            }
            ButtonSelected::No => {}
        }

        if self.enabled.get() && hovered {
            if is_mouse_button_pressed(MouseButton::Left) {
                if let Some(event_sender) = &self.event_sender {
                    event_sender.send(InternalUiEvent::ButtonClicked(self.id, self.action));
                }
            }
            let margin = -1.0;

            draw_rectangle_lines(
                x - margin,
                y - margin,
                w + margin * 2.0,
                h + margin * 2.0,
                2.0,
                self.hover_border_color,
            );
        }

        self.points_row.draw(
            x + w - points_row_size.0 - 4.0,
            y + h - points_row_h_pad - points_row_size.1 - 1.0,
        );

        draw_debug(x, y, w, h);
    }

    fn size(&self) -> (f32, f32) {
        self.size
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ButtonAction {
    Action(BaseAction),
    OnAttackedReaction(OnAttackedReaction),
    OnHitReaction(OnHitReaction),
    AttackEnhancement(AttackEnhancement),
    SpellEnhancement(SpellEnhancement),
    Proceed,
}

impl ButtonAction {
    fn icon(&self) -> IconId {
        match self {
            ButtonAction::Action(base_action) => match base_action {
                BaseAction::Attack { .. } => IconId::Attack,
                BaseAction::CastSpell(spell) => spell.icon,
                BaseAction::Move => IconId::Move,
                BaseAction::ChangeEquipment => IconId::Equip,
                BaseAction::EndTurn => IconId::Go,
            },
            ButtonAction::AttackEnhancement(enhancement) => enhancement.icon,
            ButtonAction::SpellEnhancement(enhancement) => enhancement.icon,
            ButtonAction::OnAttackedReaction(reaction) => reaction.icon,
            ButtonAction::OnHitReaction(reaction) => reaction.icon,
            ButtonAction::Proceed => IconId::Go,
        }
    }

    pub fn action_point_cost(&self) -> u32 {
        match self {
            ButtonAction::Action(base_action) => base_action.action_point_cost(),
            ButtonAction::OnAttackedReaction(reaction) => reaction.action_point_cost,
            ButtonAction::OnHitReaction(reaction) => reaction.action_point_cost,
            ButtonAction::AttackEnhancement(enhancement) => enhancement.action_point_cost,
            ButtonAction::SpellEnhancement(enhancement) => enhancement.action_point_cost,
            ButtonAction::Proceed => 0,
        }
    }

    pub fn mana_cost(&self) -> u32 {
        match self {
            ButtonAction::Action(base_action) => base_action.mana_cost(),
            ButtonAction::OnAttackedReaction(..) => 0,
            ButtonAction::OnHitReaction(..) => 0,
            ButtonAction::AttackEnhancement(..) => 0,
            ButtonAction::SpellEnhancement(enhancement) => enhancement.mana_cost,
            ButtonAction::Proceed => 0,
        }
    }

    pub fn stamina_cost(&self) -> u32 {
        match self {
            ButtonAction::Action(base_action) => base_action.stamina_cost(),
            ButtonAction::OnAttackedReaction(reaction) => reaction.stamina_cost,
            ButtonAction::OnHitReaction(_reaction) => 0,
            ButtonAction::AttackEnhancement(enhancement) => enhancement.stamina_cost,
            ButtonAction::SpellEnhancement(_enhancement) => 0,
            ButtonAction::Proceed => 0,
        }
    }

    pub fn unwrap_spell(&self) -> Spell {
        match self {
            ButtonAction::Action(BaseAction::CastSpell(spell)) => *spell,
            _ => panic!(),
        }
    }

    pub fn unwrap_spell_enhancement(&self) -> SpellEnhancement {
        match self {
            ButtonAction::SpellEnhancement(enhancement) => *enhancement,
            _ => panic!(),
        }
    }

    pub fn unwrap_attack_enhancement(&self) -> AttackEnhancement {
        match self {
            ButtonAction::AttackEnhancement(enhancement) => *enhancement,
            _ => panic!(),
        }
    }

    pub fn unwrap_on_attacked_reaction(&self) -> OnAttackedReaction {
        match self {
            ButtonAction::OnAttackedReaction(reaction) => *reaction,
            _ => panic!(),
        }
    }

    pub fn unwrap_on_hit_reaction(&self) -> OnHitReaction {
        match self {
            ButtonAction::OnHitReaction(reaction) => *reaction,
            _ => panic!(),
        }
    }
}

pub struct EventSender {
    pub queue: Rc<RefCell<Vec<InternalUiEvent>>>,
}

impl EventSender {
    pub fn send(&self, value: InternalUiEvent) {
        self.queue.borrow_mut().push(value);
    }
}

pub enum InternalUiEvent {
    ButtonHovered(u32, ButtonAction, Option<(f32, f32)>),
    ButtonClicked(u32, ButtonAction),
}

pub fn draw_button_tooltip(
    font: &Font,
    button_position: (f32, f32),
    tooltip: &ActionButtonTooltip,
) {
    let mut lines = vec![tooltip.header.to_string()];
    if let Some(description) = tooltip.description {
        lines.push(description.to_string());
    }
    lines.extend_from_slice(&tooltip.technical_description);

    // TODO
    let button_size = (64.0, 64.0);

    draw_tooltip(
        font,
        Rect::new(
            button_position.0,
            button_position.1,
            button_size.0,
            button_size.1,
        ),
        TooltipPositionPreference::Top,
        &lines,
    );
}

pub enum TooltipPosition {
    TopLeft((f32, f32)),
    BottomLeft((f32, f32)),
    TopRight((f32, f32)),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TooltipPositionPreference {
    Top,
    Right,
    Bottom,
    Left,
}

pub fn draw_tooltip(
    font: &Font,
    rect: Rect,
    mut pos_preference: TooltipPositionPreference,
    lines: &[String],
) {
    let font_size = 18;
    let mut max_line_w = 0.0;
    let text_margin = 8.0;
    for line in lines {
        let dimensions = measure_text(line, Some(font), font_size, 1.0);
        if dimensions.width > max_line_w {
            max_line_w = dimensions.width;
        }
    }

    let tooltip_w = max_line_w + text_margin * 2.0;

    let line_h = 22.0;
    let tooltip_h = lines.len() as f32 * line_h + text_margin * 2.0;

    let (screen_w, screen_h) = screen_size();

    if pos_preference == TooltipPositionPreference::Top && rect.top() - tooltip_h < 0.0 {
        pos_preference = TooltipPositionPreference::Bottom;
    }
    if pos_preference == TooltipPositionPreference::Bottom && rect.bottom() + tooltip_h > screen_h {
        pos_preference = TooltipPositionPreference::Top;
    }
    if pos_preference == TooltipPositionPreference::Left && rect.left() - tooltip_w < 0.0 {
        pos_preference = TooltipPositionPreference::Right;
    }
    if pos_preference == TooltipPositionPreference::Right && rect.right() + tooltip_w > screen_w {
        pos_preference = TooltipPositionPreference::Left;
    }

    let space = 3.0;

    let (x, y) = match pos_preference {
        TooltipPositionPreference::Top => (
            rect.left().min(screen_w - tooltip_w),
            rect.top() - space - tooltip_h,
        ),
        TooltipPositionPreference::Right => {
            (rect.right() + space, rect.top().min(screen_h - tooltip_h))
        }
        TooltipPositionPreference::Bottom => {
            (rect.left().min(screen_w - tooltip_w), rect.bottom() + space)
        }
        TooltipPositionPreference::Left => (
            rect.left() - space - tooltip_w,
            rect.top().min(screen_h - tooltip_h),
        ),
    };

    let tooltip_rect = (x, y, tooltip_w, tooltip_h);
    draw_rectangle(
        tooltip_rect.0,
        tooltip_rect.1,
        tooltip_rect.2,
        tooltip_rect.3,
        Color::new(0.0, 0.0, 0.0, 0.9),
    );
    draw_rectangle_lines(
        tooltip_rect.0,
        tooltip_rect.1,
        tooltip_rect.2,
        tooltip_rect.3,
        1.0,
        GRAY,
    );

    let text_params = TextParams {
        font: Some(font),
        font_size,
        color: WHITE,
        ..Default::default()
    };

    let mut line_y = tooltip_rect.1 + text_margin * 2.0 + 5.0;
    for (i, line) in lines.iter().enumerate() {
        let mut params = text_params.clone();
        if i == 0 {
            params.color = YELLOW;
        }
        draw_text_ex(line, tooltip_rect.0 + text_margin, line_y, params);
        line_y += line_h;
    }
}
