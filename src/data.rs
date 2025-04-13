use crate::{
    core::{
        ApplyEffect, ArmorPiece, AttackAttribute, AttackEnhancement, AttackEnhancementOnHitEffect,
        AttackHitEffect, Condition, ConditionDescription, OnAttackedReaction,
        OnAttackedReactionEffect, OnHitReaction, OnHitReactionEffect, Range, SelfEffectAction,
        Shield, Spell, SpellEnhancement, SpellEnhancementEffect, SpellType, Weapon, WeaponGrip,
        WeaponRange,
    },
    textures::{EquipmentIconId, IconId, SpriteId},
};

pub const LEATHER_ARMOR: ArmorPiece = ArmorPiece {
    name: "Leather armor",
    protection: 3,
    limit_evasion_from_agi: None,
    icon: EquipmentIconId::LeatherArmor,
    weight: 1,
};

pub const CHAIN_MAIL: ArmorPiece = ArmorPiece {
    name: "Chain mail",
    protection: 5,
    limit_evasion_from_agi: Some(4),
    icon: EquipmentIconId::ChainMail,
    weight: 3,
};

pub const DAGGER: Weapon = Weapon {
    name: "Dagger",
    range: WeaponRange::Melee,
    action_point_cost: 2,
    damage: 2,
    grip: WeaponGrip::Light,
    attack_attribute: AttackAttribute::Finesse,
    attack_enhancement: None,
    on_attacked_reaction: None,
    on_true_hit: Some(AttackHitEffect::Apply(ApplyEffect::Condition(
        Condition::Weakened(1),
    ))),
    sprite: Some(SpriteId::Dagger),
    icon: EquipmentIconId::Dagger,
    weight: 1,
};

pub const SWORD: Weapon = Weapon {
    name: "Sword",
    range: WeaponRange::Melee,
    action_point_cost: 2,
    damage: 2,
    grip: WeaponGrip::Versatile,
    attack_attribute: AttackAttribute::Finesse,
    attack_enhancement: None,
    on_attacked_reaction: Some(PARRY),
    on_true_hit: Some(AttackHitEffect::Apply(ApplyEffect::Condition(
        Condition::Bleeding,
    ))),
    sprite: Some(SpriteId::Sword),
    icon: EquipmentIconId::Sword,
    weight: 2,
};

pub const RAPIER: Weapon = Weapon {
    name: "Rapier",
    range: WeaponRange::Melee,
    action_point_cost: 2,
    damage: 2,
    grip: WeaponGrip::MainHand,
    attack_attribute: AttackAttribute::Finesse,
    attack_enhancement: None,
    on_attacked_reaction: Some(PARRY),
    on_true_hit: Some(AttackHitEffect::SkipExertion),
    sprite: Some(SpriteId::Rapier),
    icon: EquipmentIconId::Rapier,
    weight: 2,
};

pub const WAR_HAMMER: Weapon = Weapon {
    name: "War hammer",
    range: WeaponRange::Melee,
    action_point_cost: 2,
    damage: 3,
    grip: WeaponGrip::TwoHanded,
    attack_attribute: AttackAttribute::Strength,
    attack_enhancement: Some(AttackEnhancement {
        name: "All-in",
        description: "+1 damage",
        icon: IconId::AllIn,
        action_point_cost: 1,
        regain_action_points: 0,
        stamina_cost: 0,
        bonus_damage: 1,
        bonus_advantage: 0,
        on_hit_effect: None,
    }),
    on_attacked_reaction: Some(PARRY),
    on_true_hit: Some(AttackHitEffect::Apply(ApplyEffect::Condition(
        Condition::Dazed(1),
    ))),
    sprite: Some(SpriteId::Warhammer),
    icon: EquipmentIconId::Warhammer,
    weight: 5,
};

pub const BOW: Weapon = Weapon {
    name: "Bow",
    range: WeaponRange::Ranged(5),
    action_point_cost: 2,
    damage: 3,
    grip: WeaponGrip::TwoHanded,
    attack_attribute: AttackAttribute::Agility,
    attack_enhancement: Some(CAREFULLY_AIMED),
    on_attacked_reaction: None,
    on_true_hit: Some(AttackHitEffect::Apply(ApplyEffect::Condition(
        Condition::Weakened(1),
    ))),
    sprite: Some(SpriteId::Bow),
    icon: EquipmentIconId::Bow,
    weight: 2,
};

pub const SMALL_SHIELD: Shield = Shield {
    name: "Small shield",
    sprite: Some(SpriteId::Shield),
    evasion: 3,
    on_hit_reaction: Some(OnHitReaction {
        name: "Shield bash",
        description: "Possibly daze attacker",
        icon: IconId::ShieldBash,
        action_point_cost: 1,
        effect: OnHitReactionEffect::ShieldBash,
        must_be_melee: true,
    }),
    weight: 2,
};

pub const EFFICIENT: AttackEnhancement = AttackEnhancement {
    name: "Efficient strike",
    description: "On hit: regain 1 AP",
    icon: IconId::Plus,
    action_point_cost: 0,
    regain_action_points: 1,
    stamina_cost: 3,
    bonus_damage: 0,
    bonus_advantage: 0,
    on_hit_effect: Some(AttackEnhancementOnHitEffect::RegainActionPoint),
};

pub const OVERWHELMING: AttackEnhancement = AttackEnhancement {
    name: "Overwhelm",
    description: "Target loses 1 AP",
    icon: IconId::CrushingStrike,
    action_point_cost: 0,
    regain_action_points: 0,
    stamina_cost: 2,
    bonus_damage: 0,
    bonus_advantage: 0,
    on_hit_effect: Some(AttackEnhancementOnHitEffect::Target(
        ApplyEffect::RemoveActionPoints(1),
    )),
};

pub const PARRY_EVASION_BONUS: u32 = 3;
pub const PARRY: OnAttackedReaction = OnAttackedReaction {
    name: "Parry",
    description: "Gain +3 evasion against one melee attack",
    icon: IconId::Parry,
    action_point_cost: 1,
    stamina_cost: 0,
    effect: OnAttackedReactionEffect::Parry,
    must_be_melee: true,
};

pub const CAREFULLY_AIMED: AttackEnhancement = AttackEnhancement {
    name: "Carefully aimed",
    description: "Gain advantage",
    icon: IconId::CarefulAim,
    action_point_cost: 1,
    regain_action_points: 0,
    stamina_cost: 0,
    bonus_damage: 0,
    bonus_advantage: 1,
    on_hit_effect: None,
};

pub const SIDE_STEP: OnAttackedReaction = OnAttackedReaction {
    name: "Side step",
    description: "Double your evasion gained from agility",
    icon: IconId::Sidestep,
    action_point_cost: 1,
    stamina_cost: 1,
    effect: OnAttackedReactionEffect::SideStep,
    must_be_melee: false,
};

pub const RAGE: OnHitReaction = OnHitReaction {
    name: "Rage",
    description: RAGING_DESCRIPTION.description,
    icon: IconId::Rage,
    action_point_cost: 1,
    effect: OnHitReactionEffect::Rage,
    must_be_melee: false,
};

pub const RAGING_DESCRIPTION: ConditionDescription = ConditionDescription {
    name: "Raging",
    description: "Gains advantage on the next attack",
};

pub const BRACED_DEFENSE_BONUS: u32 = 3;
pub const BRACE: SelfEffectAction = SelfEffectAction {
    name: "Brace",
    description: "Gain +3 evasion against the next incoming attack",
    icon: IconId::Brace,
    action_point_cost: 1,
    stamina_cost: 1,
    effect: ApplyEffect::Condition(Condition::Braced),
};
pub const BRACED_DESCRIPTION: ConditionDescription = ConditionDescription {
    name: "Braced",
    description: "Has +3 evasion against the next incoming attack",
};

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SpellId {
    Scream,
    Mindblast,
    Fireball,
    Kill,
}

pub const SCREAM: Spell = Spell {
    id: SpellId::Scream,
    name: "Scream",
    description: "Daze the enemy",
    icon: IconId::Scream,
    action_point_cost: 2,
    mana_cost: 1,
    damage: 0,
    on_hit_effect: Some(ApplyEffect::Condition(Condition::Dazed(1))),
    spell_type: SpellType::Mental,
    possible_enhancements: [
        Some(SpellEnhancement {
            name: "Shriek",
            description: "Target loses 1 AP",
            icon: IconId::Banshee,
            mana_cost: 1,
            bonus_damage: 0,
            effect: Some(SpellEnhancementEffect::OnHitEffect(
                ApplyEffect::RemoveActionPoints(1),
            )),
        }),
        None,
    ],
    range: Range::Ranged(4),
};

pub const MIND_BLAST: Spell = Spell {
    id: SpellId::Mindblast,
    name: "Mind blast",
    description: "Damage and stagger the enemy",
    icon: IconId::Mindblast,
    action_point_cost: 2,
    mana_cost: 1,
    damage: 1,
    on_hit_effect: Some(ApplyEffect::RemoveActionPoints(1)),
    spell_type: SpellType::Mental,
    possible_enhancements: [
        Some(SpellEnhancement {
            name: "Dualcast",
            description: "Spell is cast twice",
            icon: IconId::Dualcast,
            mana_cost: 1,
            bonus_damage: 0,
            effect: Some(SpellEnhancementEffect::CastTwice),
        }),
        None,
    ],
    range: Range::Ranged(5),
};

pub const FIREBALL: Spell = Spell {
    id: SpellId::Fireball,
    name: "Fireball",
    description: "Hurl a fireball that damages the target",
    icon: IconId::Fireball,
    action_point_cost: 3,
    mana_cost: 1,
    damage: 2,
    on_hit_effect: None,
    spell_type: SpellType::Projectile,
    possible_enhancements: [
        Some(SpellEnhancement {
            name: "Big",
            description: "+1 damage",
            icon: IconId::Plus,
            mana_cost: 1,
            bonus_damage: 1,
            effect: None,
        }),
        Some(SpellEnhancement {
            name: "Massive",
            description: "+2 damage",
            icon: IconId::PlusPlus,
            mana_cost: 1,
            bonus_damage: 2,
            effect: None,
        }),
    ],
    range: Range::Ranged(5),
};

pub const KILL: Spell = Spell {
    id: SpellId::Kill,
    name: "Kill",
    description: "Kill the enemy",
    icon: IconId::Fireball,
    action_point_cost: 5,
    mana_cost: 0,
    damage: 99,
    on_hit_effect: None,
    spell_type: SpellType::Projectile,
    possible_enhancements: [None; 2],
    range: Range::Ranged(99),
};
