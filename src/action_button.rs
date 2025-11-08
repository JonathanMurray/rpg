use std::{
    cell::{Cell, Ref, RefCell},
    collections::HashMap,
    rc::Rc,
};

use macroquad::{
    color::{Color, GOLD, GRAY, GREEN, LIGHTGRAY, RED, SKYBLUE, WHITE, YELLOW},
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
        Ability, AbilityAllyEffect, AbilityDamage, AbilityEffect, AbilityEnemyEffect,
        AbilityEnhancement, AbilityModifier, AbilityReach, AbilityTarget, ApplyEffect,
        AttackEnhancement, AttackEnhancementEffect, AttackEnhancementOnHitEffect, BaseAction,
        Character, DefenseType, HandType, OnAttackedReaction, OnHitReaction, PassiveSkill,
        SpellEnemyEffect, Weapon, WeaponType,
    },
    drawing::draw_dashed_rectangle_lines,
    textures::IconId,
};

#[derive(Default)]
pub struct ActionButtonTooltip {
    pub header: String,
    pub description: Option<&'static str>,
    pub error: Option<&'static str>,
    pub technical_description: Vec<String>,
}

fn button_action_tooltip(action: &ButtonAction) -> ActionButtonTooltip {
    match action {
        ButtonAction::Action(base_action) => base_action_tooltip(base_action),
        ButtonAction::AttackEnhancement(enhancement) => attack_enhancement_tooltip(enhancement),
        ButtonAction::AbilityEnhancement(enhancement) => ability_enhancement_tooltip(enhancement),
        ButtonAction::OnAttackedReaction(reaction) => on_attacked_reaction_tooltip(reaction),
        ButtonAction::OnHitReaction(reaction) => on_hit_reaction_tooltip(reaction),
        ButtonAction::Proceed => ActionButtonTooltip {
            header: "Proceed".to_string(),
            ..Default::default()
        },
        ButtonAction::OpportunityAttack => ActionButtonTooltip {
            header: "THIS SHOULD NOT BE SHOWN".to_string(), // This is replaced on-the-fly if needed
            ..Default::default()
        },
        ButtonAction::Passive(skill) => passive_skill_tooltip(skill),
    }
}

fn passive_skill_tooltip(skill: &PassiveSkill) -> ActionButtonTooltip {
    ActionButtonTooltip {
        header: skill.name().to_string(),
        description: Some(skill.description()),
        ..Default::default()
    }
}

fn on_attacked_reaction_tooltip(reaction: &OnAttackedReaction) -> ActionButtonTooltip {
    let mut technical_description = vec![];

    if reaction.effect.bonus_evasion > 0 {
        technical_description.push(format!("+ {} evasion", reaction.effect.bonus_evasion));
    }

    ActionButtonTooltip {
        header: format!(
            "{} {}",
            reaction.name,
            cost_string(reaction.action_point_cost, reaction.stamina_cost, 0)
        ),
        description: Some(reaction.description),
        error: None,
        technical_description,
    }
}

fn on_hit_reaction_tooltip(reaction: &OnHitReaction) -> ActionButtonTooltip {
    ActionButtonTooltip {
        header: format!(
            "{} {}",
            reaction.name,
            cost_string(reaction.action_point_cost, reaction.stamina_cost, 0)
        ),
        description: Some(reaction.description),
        ..Default::default()
    }
}

fn attack_enhancement_tooltip(enhancement: &AttackEnhancement) -> ActionButtonTooltip {
    let mut technical_description = vec![];

    describe_attack_enhancement_effect(&enhancement.effect, &mut technical_description);

    ActionButtonTooltip {
        header: format!(
            "{} {}",
            enhancement.name,
            cost_string(
                enhancement.action_point_cost,
                enhancement.stamina_cost,
                enhancement.mana_cost
            )
        ),
        description: Some(enhancement.description),
        error: None,
        technical_description,
    }
}

fn describe_attack_enhancement_effect(
    effect: &AttackEnhancementEffect,
    technical_description: &mut Vec<String>,
) {
    /*
    if let Some(weapon_requirement) = enhancement.weapon_requirement {
        match weapon_requirement {
            WeaponType::Melee => technical_description.push("[ melee ]".to_string()),
            WeaponType::Ranged => technical_description.push("[ ranged ]".to_string()),
        }
    }
     */

    if effect.roll_modifier != 0 {
        if effect.roll_modifier > 0 {
            technical_description.push(format!("+ {} to attack roll", effect.roll_modifier));
        } else {
            technical_description.push(format!("- {} to attack roll", -effect.roll_modifier));
        }
    }

    if effect.action_point_discount > 0 {
        technical_description.push(format!("- {} AP cost", effect.action_point_discount));
    }
    if effect.bonus_damage > 0 {
        technical_description.push(format!("+ {} damage", effect.bonus_damage));
    }
    if effect.bonus_advantage > 0 {
        technical_description.push(format!("+ {} advantage", effect.bonus_advantage));
    }
    if let Some(mut condition) = effect.inflict_condition_per_damage {
        let stacks = *condition.stacks().unwrap();
        technical_description.push(format!(
            "inflicts {} {} per damage dealt",
            stacks,
            condition.name()
        ));
    }

    if effect.armor_penetration > 0 {
        technical_description.push(format!("{} armor penetration", effect.armor_penetration));
    }

    if let Some(effect) = effect.on_target {
        technical_description.push("Target:".to_string());
        describe_apply_effect(effect, technical_description);
    }

    if let Some(effect) = effect.on_damage_effect {
        match effect {
            AttackEnhancementOnHitEffect::RegainActionPoint => {
                technical_description.push("Regain AP".to_string())
            }
            AttackEnhancementOnHitEffect::Target(apply_effect) => {
                technical_description.push("Target (on hit):".to_string());
                describe_apply_effect(apply_effect, technical_description);
            }
        }
    }
}

fn ability_enhancement_tooltip(enhancement: &AbilityEnhancement) -> ActionButtonTooltip {
    let mut technical_description = vec![];

    if let Some(effect) = enhancement.spell_effect {
        if effect.roll_bonus > 0 {
            technical_description.push(format!("+ {} to dice roll", effect.roll_bonus));
        }

        if effect.bonus_advantage > 0 {
            technical_description.push(format!("+ {} advantage", effect.bonus_advantage));
        }

        if effect.bonus_target_damage > 0 {
            technical_description.push(format!("+ {} damage (target)", effect.bonus_target_damage));
        }

        if effect.bonus_area_damage > 0 {
            technical_description.push(format!("+ {} damage (area)", effect.bonus_area_damage));
        }

        if let Some(apply_effect) = effect.on_hit {
            describe_apply_effect(apply_effect, &mut technical_description);
        }

        if effect.increased_range_tenths > 0 {
            technical_description.push(format!(
                "+ {} range",
                effect.increased_range_tenths as f32 * 0.1
            ));
        }

        if effect.increased_radius_tenths > 0 {
            technical_description.push(format!(
                "+ {} radius",
                effect.increased_radius_tenths as f32 * 0.1
            ));
        }
    }

    if let Some(effect) = enhancement.attack_effect {
        describe_attack_enhancement_effect(&effect, &mut technical_description);
    }

    ActionButtonTooltip {
        header: format!(
            "{} {}",
            enhancement.name,
            cost_string(
                enhancement.action_point_cost,
                enhancement.stamina_cost,
                enhancement.mana_cost
            )
        ),
        description: Some(enhancement.description),
        error: None,
        technical_description,
    }
}

fn base_action_tooltip(base_action: &BaseAction) -> ActionButtonTooltip {
    match base_action {
        BaseAction::Attack { .. } => ActionButtonTooltip {
            header: "No weapon equipped".to_string(), // This is replaced on-the-fly if needed
            ..Default::default()
        },
        BaseAction::UseAbility(ability) => ability_tooltip(ability),
        BaseAction::Move => ActionButtonTooltip {
            header: "Move".to_string(),
            ..Default::default()
        },
        BaseAction::ChangeEquipment => ActionButtonTooltip {
            header: "Equip/unequip (1 AP)".to_string(),
            ..Default::default()
        },
        BaseAction::UseConsumable => ActionButtonTooltip {
            header: "Use consumable (1 AP)".to_string(),
            ..Default::default()
        },
        BaseAction::EndTurn => ActionButtonTooltip {
            header: "End your turn".to_string(),
            ..Default::default()
        },
    }
}

fn describe_apply_effect(effect: ApplyEffect, technical_description: &mut Vec<String>) {
    match effect {
        ApplyEffect::RemoveActionPoints(n) => {
            technical_description.push(format!("  Loses {}+ AP", n))
        }
        ApplyEffect::GainStamina(n) => {
            technical_description.push(format!("  Gains {}+ stamina", n))
        }
        ApplyEffect::Condition(mut condition) => {
            let line = if let Some(stacks) = condition.stacks().copied() {
                format!("  {} ({})", condition.name(), stacks)
            } else {
                format!("  {}", condition.name())
            };
            technical_description.push(line);
        }
    }
}

fn ability_tooltip(ability: &Ability) -> ActionButtonTooltip {
    let header = format!(
        "{} {}",
        ability.name,
        cost_string(
            ability.action_point_cost,
            ability.stamina_cost,
            ability.mana_cost
        )
    );
    let mut technical_description = vec![];

    match ability.modifier {
        AbilityModifier::Spell => technical_description.push("[ spell roll ]".to_string()),
        AbilityModifier::Attack(bonus) if bonus < 0 => {
            technical_description.push(format!("[ attack roll - {} ]", -bonus))
        }
        AbilityModifier::Attack(bonus) if bonus > 0 => {
            technical_description.push(format!("[ attack roll + {} ]", bonus))
        }
        AbilityModifier::Attack(_) => technical_description.push("[ attack roll ]".to_string()),
    }

    match ability.target {
        AbilityTarget::Enemy {
            effect,
            impact_area: area,
            reach,
        } => {
            match reach {
                AbilityReach::Range(range) => {
                    technical_description.push(format!("Target enemy (range {})", range));
                }
                AbilityReach::MoveIntoMelee(range) => {
                    technical_description.push(format!("Engage enemy (range {})", range));
                }
            }
            describe_ability_enemy_effect(effect, &mut technical_description);

            if let Some((range, effect)) = area {
                technical_description.push(format!("Impact area (radius {})", range));
                describe_ability_enemy_effect(effect, &mut technical_description);
            }
        }

        AbilityTarget::Ally { range, effect } => {
            technical_description.push(format!("Target ally (range {})", range));
            describe_ability_ally_effect(effect, &mut technical_description);
        }

        AbilityTarget::None {
            self_area,
            self_effect,
        } => {
            if let Some(effect) = self_effect {
                technical_description.push("Self effect".to_string());
                describe_ability_ally_effect(effect, &mut technical_description);
            }

            if let Some((radius, effect)) = self_area {
                match effect {
                    AbilityEffect::Enemy(effect) => {
                        technical_description.push(format!("Nearby enemies (radius {})", radius));
                        describe_ability_enemy_effect(effect, &mut technical_description);
                    }
                    AbilityEffect::Ally(effect) => {
                        technical_description.push(format!("Nearby allies (radius {})", radius));
                        describe_ability_ally_effect(effect, &mut technical_description);
                    }
                }
            }
        }

        AbilityTarget::Area {
            range,
            radius,
            effect,
        } => match effect {
            AbilityEffect::Enemy(effect) => {
                technical_description.push(format!("Enemies (range {}, radius {})", range, radius));
                describe_ability_enemy_effect(effect, &mut technical_description);
            }
            AbilityEffect::Ally(effect) => {
                technical_description.push(format!("Allies (range {}, radius {})", range, radius));
                describe_ability_ally_effect(effect, &mut technical_description);
            }
        },
    };
    ActionButtonTooltip {
        header,
        description: Some(ability.description),
        error: None,
        technical_description,
    }
}

fn describe_ability_enemy_effect(
    effect: AbilityEnemyEffect,
    technical_description: &mut Vec<String>,
) {
    match effect {
        AbilityEnemyEffect::Spell(effect) => {
            match effect.defense_type {
                Some(DefenseType::Will) => technical_description.push("  [ will ]".to_string()),
                Some(DefenseType::Evasion) => {
                    technical_description.push("  [ evasion ]".to_string())
                }
                Some(DefenseType::Toughness) => {
                    technical_description.push("  [ toughness ]".to_string())
                }
                None => {}
            };

            match effect.damage {
                Some(AbilityDamage::Static(n)) => {
                    technical_description.push(format!("  {} damage", n))
                }
                Some(AbilityDamage::AtLeast(n)) => {
                    technical_description.push(format!("  {}+ damage", n))
                }
                None => {}
            }

            for apply_effect in effect.on_hit.unwrap_or_default().iter().flatten() {
                describe_apply_effect(*apply_effect, technical_description);
            }
        }

        AbilityEnemyEffect::Attack => {
            technical_description.push("  [ evasion ]".to_string());
            technical_description.push("  weapon damage".to_string());
        }
    }
}

fn describe_ability_ally_effect(
    effect: AbilityAllyEffect,
    technical_description: &mut Vec<String>,
) {
    if effect.healing > 0 {
        technical_description.push(format!("  {}+ healing", effect.healing));
    }

    if let Some(apply) = effect.apply {
        describe_apply_effect(apply, technical_description);
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ButtonSelected {
    No,
    Partially,
    Yes,
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
    dynamic_icons: HashMap<IconId, Texture2D>,
    tooltip: RefCell<ActionButtonTooltip>,
    character: Option<Rc<Character>>,
    tooltip_is_based_on_equipped_weapon: Cell<Option<Weapon>>,
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
        let icon: IconId = action.icon(character.as_deref());
        let tooltip = button_action_tooltip(&action);

        let (size, texture_draw_size) = match action {
            ButtonAction::Proceed | ButtonAction::Passive(..) => ((64.0, 52.0), (60.0, 48.0)),
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

        let padding = if point_icons.is_empty() { 0.0 } else { 1.0 };
        let points_row = Container {
            children: point_icons,
            margin: 4.0,
            layout_dir: LayoutDirection::Horizontal,
            style: Style {
                padding,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut dynamic_icons: HashMap<IconId, Texture2D> = Default::default();
        if matches!(action, ButtonAction::Action(BaseAction::Attack(..))) {
            for icon_id in [IconId::MeleeAttack, IconId::RangedAttack] {
                dynamic_icons.insert(icon_id, icons[&icon_id].clone());
            }
            dynamic_icons.insert(IconId::RangedAttack, icons[&IconId::RangedAttack].clone());
        }

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
            dynamic_icons,
            tooltip: RefCell::new(tooltip),
            character,
            tooltip_is_based_on_equipped_weapon: Default::default(),
        }
    }

    pub fn tooltip(&self) -> Ref<ActionButtonTooltip> {
        // TODO: if action requires melee weapon and none is equipped, add error message to tooltip
        if let ButtonAction::Action(BaseAction::UseAbility(ability)) = self.action {
            if ability.weapon_requirement == Some(WeaponType::Melee) {
                let equipped_weapon = self.character.as_ref().unwrap().weapon(HandType::MainHand);

                if self.tooltip_is_based_on_equipped_weapon.get() != equipped_weapon {
                    if self.character.as_ref().unwrap().has_equipped_melee_weapon() {
                        self.tooltip.borrow_mut().error = None;
                    } else {
                        self.tooltip.borrow_mut().error = Some("Requires melee weapon!");
                    }

                    self.tooltip_is_based_on_equipped_weapon
                        .set(equipped_weapon);
                }
            }
        }

        if let ButtonAction::Action(BaseAction::Attack(attack)) = self.action {
            let equipped_weapon = self.character.as_ref().unwrap().weapon(attack.hand);

            if self.tooltip_is_based_on_equipped_weapon.get() != equipped_weapon {
                *self.tooltip.borrow_mut() = if let Some(weapon) = equipped_weapon {
                    let attack_type = if weapon.is_melee() { "Melee" } else { "Ranged" };
                    let mut technical_description = vec!["[ attack roll ]".to_string()];
                    let range = if weapon.is_melee() {
                        "melee".to_string()
                    } else {
                        format!("range {}", weapon.range.into_range())
                    };
                    technical_description.push(format!("Target enemy ({})", range));
                    technical_description.push("  [ evasion ]".to_string());
                    technical_description.push(format!("  {} damage", weapon.damage));
                    ActionButtonTooltip {
                        header: format!("{} attack ({} AP)", attack_type, weapon.action_point_cost),

                        technical_description,
                        ..Default::default()
                    }
                } else {
                    ActionButtonTooltip {
                        header: "No weapon equipped".to_string(),
                        ..Default::default()
                    }
                };
            }

            self.tooltip_is_based_on_equipped_weapon
                .set(equipped_weapon);
        }

        if let ButtonAction::OpportunityAttack = self.action {
            let equipped_weapon = self.character.as_ref().unwrap().weapon(HandType::MainHand);
            if self.tooltip_is_based_on_equipped_weapon.get() != equipped_weapon {
                *self.tooltip.borrow_mut() = ActionButtonTooltip {
                    header: "Opportunity attack (1 AP)".to_string(),
                    technical_description: vec![
                        format!("{} damage", equipped_weapon.unwrap().damage),
                        "vs Evasion".to_string(),
                    ],
                    ..Default::default()
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

    if !s.is_empty() {
        s = format!("({s})");
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
            //dest_size: Some((240.0, 192.0).into()),
            //dest_size: Some((24.0, 19.2).into()),
            ..Default::default()
        };

        let icon = if matches!(self.action, ButtonAction::Action(BaseAction::Attack(..))) {
            let icon_id = if self
                .character
                .as_ref()
                .unwrap()
                .has_equipped_ranged_weapon()
            {
                IconId::RangedAttack
            } else {
                IconId::MeleeAttack
            };
            &self.dynamic_icons[&icon_id]
        } else {
            &self.icon
        };

        draw_texture_ex(icon, x + 2.0, y + 2.0, WHITE, params);

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
    AbilityEnhancement(AbilityEnhancement),
    OpportunityAttack,
    Proceed,
    Passive(PassiveSkill),
}

impl ButtonAction {
    pub fn name(&self) -> &'static str {
        match self {
            ButtonAction::Action(base_action) => match base_action {
                BaseAction::Attack(..) => "Attack",
                BaseAction::UseAbility(ability) => ability.name,
                BaseAction::Move => "Move",
                BaseAction::ChangeEquipment => "Change equipment",
                BaseAction::UseConsumable => "Use consumable",
                BaseAction::EndTurn => "End turn",
            },
            ButtonAction::OnAttackedReaction(reaction) => reaction.name,
            ButtonAction::OnHitReaction(reaction) => reaction.name,
            ButtonAction::AttackEnhancement(enhancement) => enhancement.name,
            ButtonAction::AbilityEnhancement(enhancement) => enhancement.name,
            ButtonAction::OpportunityAttack => "Opportunity attack",
            ButtonAction::Proceed => "Proceed",
            ButtonAction::Passive(skill) => skill.name(),
        }
    }

    pub fn context_explanation(&self) -> Option<String> {
        match self {
            ButtonAction::AttackEnhancement(_enhancement) => Some("Attack enhancement".to_string()),
            ButtonAction::AbilityEnhancement(enhancement) => {
                Some(format!("{} enhancement", enhancement.name))
            }
            _ => None,
        }
    }

    fn icon(&self, character: Option<&Character>) -> IconId {
        match self {
            ButtonAction::Action(base_action) => match base_action {
                BaseAction::Attack(..) => {
                    if character.unwrap().has_equipped_ranged_weapon() {
                        IconId::RangedAttack
                    } else {
                        IconId::MeleeAttack
                    }
                }
                BaseAction::UseAbility(ability) => ability.icon,
                BaseAction::Move => IconId::Move,
                BaseAction::ChangeEquipment => IconId::Equip,
                BaseAction::UseConsumable => IconId::UseConsumable,
                BaseAction::EndTurn => IconId::EndTurn,
            },
            ButtonAction::AttackEnhancement(enhancement) => enhancement.icon,
            ButtonAction::AbilityEnhancement(enhancement) => enhancement.icon,
            ButtonAction::OnAttackedReaction(reaction) => reaction.icon,
            ButtonAction::OnHitReaction(reaction) => reaction.icon,
            ButtonAction::Proceed => IconId::Go,
            ButtonAction::OpportunityAttack => IconId::MeleeAttack,
            ButtonAction::Passive(skill) => skill.icon(),
        }
    }

    pub fn action_point_cost(&self) -> u32 {
        match self {
            ButtonAction::Action(base_action) => base_action.action_point_cost(),
            ButtonAction::OnAttackedReaction(reaction) => reaction.action_point_cost,
            ButtonAction::OnHitReaction(reaction) => reaction.action_point_cost,
            ButtonAction::AttackEnhancement(enhancement) => enhancement.action_point_cost,
            ButtonAction::AbilityEnhancement(enhancement) => enhancement.action_point_cost,
            ButtonAction::Proceed => 0,
            ButtonAction::OpportunityAttack => 1,
            ButtonAction::Passive(..) => 0,
        }
    }

    pub fn action_point_discount(&self) -> u32 {
        match self {
            ButtonAction::AttackEnhancement(enhancement) => {
                enhancement.effect.action_point_discount
            }
            _ => 0,
        }
    }

    pub fn mana_cost(&self) -> u32 {
        match self {
            ButtonAction::Action(base_action) => base_action.mana_cost(),
            ButtonAction::OnAttackedReaction(..) => 0,
            ButtonAction::OnHitReaction(..) => 0,
            ButtonAction::AttackEnhancement(enhancement) => enhancement.mana_cost,
            ButtonAction::AbilityEnhancement(enhancement) => enhancement.mana_cost,
            ButtonAction::Proceed => 0,
            ButtonAction::OpportunityAttack => 0,
            ButtonAction::Passive(..) => 0,
        }
    }

    pub fn stamina_cost(&self) -> u32 {
        match self {
            ButtonAction::Action(base_action) => base_action.stamina_cost(),
            ButtonAction::OnAttackedReaction(reaction) => reaction.stamina_cost,
            ButtonAction::OnHitReaction(reaction) => reaction.stamina_cost,
            ButtonAction::AttackEnhancement(enhancement) => enhancement.stamina_cost,
            ButtonAction::AbilityEnhancement(enhancement) => enhancement.stamina_cost,
            ButtonAction::Proceed => 0,
            ButtonAction::OpportunityAttack => 0,
            ButtonAction::Passive(..) => 0,
        }
    }

    pub fn unwrap_ability(&self) -> Ability {
        match self {
            ButtonAction::Action(BaseAction::UseAbility(ability)) => *ability,
            _ => panic!(),
        }
    }

    pub fn unwrap_ability_enhancement(&self) -> AbilityEnhancement {
        match self {
            ButtonAction::AbilityEnhancement(enhancement) => *enhancement,
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
            other => panic!("Expected on hit reaction, but got: {:?}", other),
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

#[derive(Debug)]
pub enum InternalUiEvent {
    ButtonHovered(u32, ButtonAction, Option<(f32, f32)>),
    ButtonClicked(u32, ButtonAction),
}

pub fn draw_button_tooltip(
    font: &Font,
    button_position: (f32, f32),
    tooltip: &ActionButtonTooltip,
) {
    let mut lines = vec![];
    if let Some(description) = tooltip.description {
        if !description.is_empty() {
            lines.push(description.to_string());
            if !tooltip.technical_description.is_empty() {
                lines.push("".to_string());
            }
        }
    }

    lines.extend_from_slice(&tooltip.technical_description);

    let button_size = (64.0, 64.0);

    draw_tooltip(
        font,
        TooltipPositionPreference::RelativeToRect(
            Rect::new(
                button_position.0,
                button_position.1,
                button_size.0,
                button_size.1,
            ),
            Side::Top,
        ),
        &tooltip.header,
        tooltip.error,
        &lines,
    );
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TooltipPositionPreference {
    RelativeToRect(Rect, Side),
    HorCenteredAt((f32, f32)),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

pub fn draw_tooltip(
    font: &Font,
    pos_preference: TooltipPositionPreference,
    header: &str,
    error: Option<&'static str>,
    content_lines: &[String],
) {
    let font_size = 18;
    let mut max_line_w = 0.0;
    let text_margin = 8.0;

    let mut measure_width = |line| {
        let dimensions = measure_text(line, Some(font), font_size, 1.0);
        if dimensions.width > max_line_w {
            max_line_w = dimensions.width;
        }
    };

    measure_width(header);
    if let Some(error) = error.as_ref() {
        measure_width(error)
    }
    for line in content_lines {
        measure_width(line);
    }

    let tooltip_w = max_line_w + text_margin * 2.0;

    let empty_line_h = 12.0;

    let line_h = 22.0;
    let num_real_lines = 1
        + content_lines.iter().filter(|line| !line.is_empty()).count()
        + error.map(|_| 1).unwrap_or(0);
    let num_empty_lines = content_lines.iter().filter(|line| line.is_empty()).count();
    let tooltip_h =
        num_real_lines as f32 * line_h + text_margin * 2.0 + num_empty_lines as f32 * empty_line_h;

    let (screen_w, screen_h) = screen_size();

    let (x, y) = match pos_preference {
        TooltipPositionPreference::RelativeToRect(rect, mut pos_preference) => {
            if pos_preference == Side::Top && rect.top() - tooltip_h < 0.0 {
                pos_preference = Side::Bottom;
            }
            if pos_preference == Side::Bottom && rect.bottom() + tooltip_h > screen_h {
                pos_preference = Side::Top;
            }
            if pos_preference == Side::Left && rect.left() - tooltip_w < 0.0 {
                pos_preference = Side::Right;
            }
            if pos_preference == Side::Right && rect.right() + tooltip_w > screen_w {
                pos_preference = Side::Left;
            }

            let space = 3.0;

            match pos_preference {
                Side::Top => (
                    rect.left().min(screen_w - tooltip_w),
                    rect.top() - space - tooltip_h,
                ),
                Side::Right => (rect.right() + space, rect.top().min(screen_h - tooltip_h)),
                Side::Bottom => (rect.left().min(screen_w - tooltip_w), rect.bottom() + space),
                Side::Left => (
                    rect.left() - space - tooltip_w,
                    rect.top().min(screen_h - tooltip_h),
                ),
            }
        }
        TooltipPositionPreference::HorCenteredAt((x, y)) => (x - tooltip_w / 2.0, y),
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

    let mut draw_line = |line, color: Option<Color>| {
        let mut params = text_params.clone();
        if let Some(c) = color {
            params.color = c;
        }
        draw_text_ex(line, tooltip_rect.0 + text_margin, line_y, params);
        if line.is_empty() {
            line_y += empty_line_h;
        } else {
            line_y += line_h;
        }
    };

    draw_line(header, Some(YELLOW));
    if let Some(error) = error {
        draw_line(error, Some(RED))
    }
    for line in content_lines {
        draw_line(line, None)
    }
}
