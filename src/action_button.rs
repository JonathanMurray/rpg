use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use macroquad::{
    color::{
        Color, BEIGE, BLACK, BLUE, BROWN, DARKBROWN, DARKGRAY, GOLD, GRAY, GREEN, LIGHTGRAY, PINK,
        SKYBLUE, WHITE, YELLOW,
    },
    input::{is_mouse_button_pressed, mouse_position, MouseButton},
    shapes::{draw_rectangle, draw_rectangle_lines},
    text::{draw_text_ex, measure_text, Font, TextParams},
    texture::{draw_texture_ex, DrawTextureParams, Texture2D},
};

use crate::{
    base_ui::{draw_debug, Container, Drawable, Element, LayoutDirection, Rectangle, Style},
    core::{
        AttackEnhancement, BaseAction, Character, MovementEnhancement, OnAttackedReaction,
        OnHitReaction, SpellEnhancement,
    },
    textures::IconId,
};

pub struct ActionButton {
    pub id: u32,
    pub tooltip_lines: Vec<String>,
    pub action: ButtonAction,
    pub size: (f32, f32),
    style: Style,
    hover_border_color: Color,
    points_row: Container,
    hovered: Cell<bool>,
    pub enabled: Cell<bool>,
    pub highlighted: Cell<bool>,
    pub event_sender: Option<EventSender>,
    icon: Texture2D,
}

impl ActionButton {
    pub fn new(
        action: ButtonAction,
        event_queue: &Rc<RefCell<Vec<InternalUiEvent>>>,
        id: u32,
        icons: &HashMap<IconId, Texture2D>,
        character: Option<&Character>,
    ) -> Self {
        let mut mana_points = 0;
        let mut stamina_points = 0;
        let mut action_points = 0;

        let icon: IconId;
        let mut tooltip_lines = vec![];

        match action {
            ButtonAction::Action(base_action) => match base_action {
                BaseAction::Attack {
                    action_point_cost,
                    hand,
                } => {
                    action_points = action_point_cost;
                    icon = IconId::Attack;

                    let char = character.unwrap();
                    let weapon = char.weapon(hand).unwrap();
                    tooltip_lines.push(format!(
                        "{} attack ({} AP)",
                        weapon.name, weapon.action_point_cost
                    ));
                    tooltip_lines.push(format!("{} damage", weapon.damage));
                }
                BaseAction::SelfEffect(sea) => {
                    action_points = sea.action_point_cost;
                    icon = sea.icon;

                    tooltip_lines.push(format!("{} ({} AP)", sea.name, sea.action_point_cost));
                    tooltip_lines.push(sea.description.to_string());
                }
                BaseAction::CastSpell(spell) => {
                    action_points = spell.action_point_cost;
                    mana_points = spell.mana_cost;
                    icon = spell.icon;

                    tooltip_lines.push(format!(
                        "{} ({} AP, {} mana)",
                        spell.name, spell.action_point_cost, spell.mana_cost
                    ));
                    let mut description = spell.description.to_string();
                    if spell.damage > 0 {
                        description.push_str(&format!(" ({} damage)", spell.damage));
                    }
                    tooltip_lines.push(description);
                }
                BaseAction::Move {
                    action_point_cost,
                    range: _,
                } => {
                    action_points = action_point_cost;
                    icon = IconId::Move;

                    tooltip_lines.push(format!("Movement ({} AP)", action_point_cost));
                }
            },
            ButtonAction::AttackEnhancement(enhancement) => {
                action_points = enhancement.action_point_cost;
                stamina_points = enhancement.stamina_cost;
                icon = enhancement.icon;

                tooltip_lines.push(format!(
                    "{} ({})",
                    enhancement.name,
                    cost_string(enhancement.action_point_cost, enhancement.stamina_cost)
                ));

                tooltip_lines.push("Attack enhancement:".to_string());
                tooltip_lines.push(enhancement.description.to_string());
            }

            ButtonAction::SpellEnhancement(enhancement) => {
                mana_points = enhancement.mana_cost;
                icon = enhancement.icon;

                tooltip_lines.push(format!(
                    "{} ({} mana)",
                    enhancement.name, enhancement.mana_cost
                ));
                tooltip_lines.push("Spell enhancement:".to_string());
                tooltip_lines.push(enhancement.description.to_string());
            }
            ButtonAction::MovementEnhancement(enhancement) => {
                action_points = enhancement.action_point_cost;
                stamina_points = enhancement.stamina_cost;
                icon = enhancement.icon;

                tooltip_lines.push(format!(
                    "{} ({})",
                    enhancement.name,
                    cost_string(enhancement.action_point_cost, enhancement.stamina_cost)
                ));
                tooltip_lines.push("Enhancement:".to_string());
                tooltip_lines.push(format!("+{}% range", enhancement.add_percentage));
            }
            ButtonAction::OnAttackedReaction(reaction) => {
                action_points = reaction.action_point_cost;
                stamina_points = reaction.stamina_cost;
                icon = reaction.icon;

                tooltip_lines.push(format!(
                    "{} ({})",
                    reaction.name,
                    cost_string(reaction.action_point_cost, reaction.stamina_cost)
                ));
                tooltip_lines.push(reaction.description.to_string());
            }
            ButtonAction::OnHitReaction(reaction) => {
                action_points = reaction.action_point_cost;
                icon = reaction.icon;

                tooltip_lines.push(format!(
                    "{} ({} AP)",
                    reaction.name, reaction.action_point_cost
                ));
                tooltip_lines.push(reaction.description.to_string());
            }

            ButtonAction::Proceed => {
                icon = IconId::Go;
                tooltip_lines.push("Proceed".to_string());
            }
        }

        let size = match action {
            ButtonAction::Proceed => (64.0, 52.0),
            _ => (64.0, 64.0),
        };

        let style = Style {
            background_color: Some(Color::new(0.4, 0.32, 0.21, 1.0)),
            border_color: Some(LIGHTGRAY),
            ..Default::default()
        };
        let hover_border_color = YELLOW;

        let r = 4.0;
        let mut point_icons = vec![];
        let border_width = Some(4.0);
        for _ in 0..action_points {
            point_icons.push(Element::Rect(Rectangle {
                size: (r * 2.0, r * 2.0),
                style: Style {
                    background_color: Some(GOLD),
                    border_color: Some(BLACK),
                    border_width,
                    ..Default::default()
                },
            }))
        }
        for _ in 0..mana_points {
            point_icons.push(Element::Rect(Rectangle {
                size: (r * 2.0, r * 2.0),
                style: Style {
                    background_color: Some(SKYBLUE),
                    border_color: Some(BLACK),
                    border_width,
                    ..Default::default()
                },
            }))
        }
        for _ in 0..stamina_points {
            point_icons.push(Element::Rect(Rectangle {
                size: (r * 2.0, r * 2.0),
                style: Style {
                    background_color: Some(GREEN),
                    border_color: Some(BLACK),
                    border_width,
                    ..Default::default()
                },
            }))
        }
        let points_row = Container {
            children: point_icons,
            margin: 1.0,
            layout_dir: LayoutDirection::Horizontal,
            ..Default::default()
        };

        assert!(!tooltip_lines.is_empty());

        let icon = icons[&icon].clone();
        Self {
            id,
            action,
            size,
            style,
            hover_border_color,
            points_row,
            hovered: Cell::new(false),
            enabled: Cell::new(true),
            highlighted: Cell::new(false),
            event_sender: Some(EventSender {
                queue: Rc::clone(event_queue),
            }),
            icon,
            tooltip_lines,
        }
    }

    pub fn toggle_highlighted(&self) {
        self.highlighted.set(!self.highlighted.get());
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

fn cost_string(action_points: u32, stamina: u32) -> String {
    match (action_points, stamina) {
        (0, sta) => format!("{} stamina", sta),
        (ap, 0) => format!("{} AP", ap),
        (ap, sta) => format!("{} AP, {} stamina", ap, sta),
    }
}

impl Drawable for ActionButton {
    fn draw(&self, x: f32, y: f32) {
        let (w, h) = self.size;

        self.style.draw(x, y, self.size);

        let margin_bot = 2.0;
        let points_row_size = self.points_row.size();

        if points_row_size.1 > 0.0 {
            draw_rectangle(
                x + 2.0,
                y + h - margin_bot * 2.0 - points_row_size.1,
                w - 4.0,
                points_row_size.1 + margin_bot * 2.0 - 1.0,
                GRAY,
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

        if self.enabled.get() {
            if hovered {
                if is_mouse_button_pressed(MouseButton::Left) {
                    if let Some(event_sender) = &self.event_sender {
                        event_sender.send(InternalUiEvent::ButtonClicked(self.id, self.action));
                    }
                }
                draw_rectangle_lines(x, y, w, h, 2.0, self.hover_border_color);
            }
        } else {
            draw_rectangle(x, y, w, h, Color::new(0.2, 0.0, 0.0, 0.5));
            //draw_rectangle_lines(x, y, w, h, 1.0, RED);
        }

        let params = DrawTextureParams {
            dest_size: Some((60.0, 48.0).into()),
            ..Default::default()
        };
        draw_texture_ex(&self.icon, x + 2.0, y + 2.0, WHITE, params);

        if self.highlighted.get() {
            draw_rectangle_lines(x, y, w, h, 3.0, GREEN);
        }

        self.points_row.draw(
            x + w - points_row_size.0 - margin_bot,
            y + h - margin_bot - points_row_size.1,
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
    MovementEnhancement(MovementEnhancement),
    Proceed,
}

impl ButtonAction {
    pub fn action_point_cost(&self) -> u32 {
        match self {
            ButtonAction::Action(base_action) => base_action.action_point_cost(),
            ButtonAction::OnAttackedReaction(reaction) => reaction.action_point_cost,
            ButtonAction::OnHitReaction(reaction) => reaction.action_point_cost,
            ButtonAction::AttackEnhancement(enhancement) => enhancement.action_point_cost,
            ButtonAction::SpellEnhancement(..) => 0,
            ButtonAction::Proceed => 0,
            ButtonAction::MovementEnhancement(enhancement) => enhancement.action_point_cost,
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
            ButtonAction::MovementEnhancement(..) => 0,
        }
    }

    pub fn stamina_cost(&self) -> u32 {
        match self {
            ButtonAction::Action(_base_action) => 0,
            ButtonAction::OnAttackedReaction(reaction) => reaction.stamina_cost,
            ButtonAction::OnHitReaction(_reaction) => 0,
            ButtonAction::AttackEnhancement(enhancement) => enhancement.stamina_cost,
            ButtonAction::SpellEnhancement(_enhancement) => 0,
            ButtonAction::Proceed => 0,
            ButtonAction::MovementEnhancement(enhancement) => enhancement.stamina_cost,
        }
    }

    pub fn unwrap_movement_enhancement(&self) -> MovementEnhancement {
        match self {
            ButtonAction::MovementEnhancement(enhancement) => *enhancement,
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

pub fn draw_button_tooltip(font: &Font, button_position: (f32, f32), lines: &[String]) {
    draw_tooltip(
        font,
        TooltipPosition::BottomLeft((button_position.0, button_position.1 - 3.0)),
        lines,
    );
}

pub enum TooltipPosition {
    TopLeft((f32, f32)),
    BottomLeft((f32, f32)),
}

pub fn draw_tooltip(font: &Font, position: TooltipPosition, lines: &[String]) {
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

    let (x, y) = match position {
        TooltipPosition::TopLeft((x, y)) => (x, y),
        TooltipPosition::BottomLeft((x, y)) => (x, y - tooltip_h),
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
