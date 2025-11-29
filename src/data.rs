use std::default;

use macroquad::color::{BLACK, BLUE, BROWN, GRAY, GREEN, LIME, MAGENTA, PURPLE, RED, YELLOW};

use crate::{
    core::{
        Ability, AbilityDamage, AbilityEffect, AbilityEnhancement, AbilityId,
        AbilityNegativeEffect, AbilityPositiveEffect, AbilityReach, AbilityRollType, AbilityTarget,
        ApplyCondition, ApplyEffect, AreaEffect, AreaTargetAcquisition, ArmorPiece, Arrow,
        AttackAttribute, AttackCircumstance, AttackEnhancement, AttackEnhancementEffect,
        AttackEnhancementOnHitEffect, AttackHitEffect, Condition, Consumable, DefenseType,
        EquipEffect, EquipmentRequirement, Fraction, OnAttackedReaction, OnAttackedReactionEffect,
        OnAttackedReactionId, OnHitReaction, OnHitReactionEffect, Range, Shield,
        SpellEnhancementEffect, SpellNegativeEffect, Weapon, WeaponGrip, WeaponRange, WeaponType,
    },
    textures::{EquipmentIconId, IconId, SpriteId},
};

pub const SHIRT: ArmorPiece = ArmorPiece {
    name: "Shirt",
    protection: 1,
    limit_evasion_from_agi: None,
    icon: EquipmentIconId::Shirt,
    weight: 1,
    equip: EquipEffect::default(),
};

pub const ROBE: ArmorPiece = ArmorPiece {
    name: "Robe",
    protection: 1,
    limit_evasion_from_agi: None,
    icon: EquipmentIconId::Robe,
    weight: 1,
    equip: EquipEffect {
        bonus_spell_modifier: 1,
        ..EquipEffect::default()
    },
};

pub const LEATHER_ARMOR: ArmorPiece = ArmorPiece {
    name: "Leather armor",
    protection: 2,
    limit_evasion_from_agi: None,
    icon: EquipmentIconId::LeatherArmor,
    weight: 2,
    equip: EquipEffect::default(),
};

pub const CHAIN_MAIL: ArmorPiece = ArmorPiece {
    name: "Chain mail",
    protection: 3,
    limit_evasion_from_agi: Some(4),
    icon: EquipmentIconId::ChainMail,
    weight: 3,
    equip: EquipEffect::default(),
};

pub const LIGHT_CHAIN_MAIL: ArmorPiece = ArmorPiece {
    name: "Light chain mail",
    protection: 3,
    limit_evasion_from_agi: Some(4),
    icon: EquipmentIconId::ChainMail,
    weight: 2,
    equip: EquipEffect::default(),
};

pub const STABBING: AttackEnhancement = AttackEnhancement {
    name: "Stabbing",
    icon: IconId::Stabbing,
    effect: AttackEnhancementEffect {
        roll_modifier: -3,
        inflict_x_condition_per_damage: Some((Fraction::new(1, 1), Condition::Weakened)),
        ..AttackEnhancementEffect::default()
    },
    ..AttackEnhancement::default()
};

pub const BAD_DAGGER: Weapon = Weapon {
    name: "Bad dagger",
    range: WeaponRange::Melee,
    action_point_cost: 3,
    damage: 3,
    grip: WeaponGrip::Light,
    attack_attribute: AttackAttribute::Finesse,
    attack_enhancement: Some(STABBING),
    on_attacked_reaction: None,
    on_true_hit: None,
    sprite: Some(SpriteId::Dagger),
    icon: EquipmentIconId::Dagger,
    weight: 1,
};

pub const DAGGER: Weapon = Weapon {
    name: "Dagger",
    range: WeaponRange::Melee,
    action_point_cost: 2,
    damage: 5,
    grip: WeaponGrip::Light,
    attack_attribute: AttackAttribute::Finesse,
    attack_enhancement: Some(STABBING),
    on_attacked_reaction: None,
    on_true_hit: None,
    sprite: Some(SpriteId::Dagger),
    icon: EquipmentIconId::Dagger,
    weight: 1,
};

pub const SLASHING: AttackEnhancement = AttackEnhancement {
    name: "Slashing",
    icon: IconId::Slashing,
    stamina_cost: 2,
    effect: AttackEnhancementEffect {
        inflict_x_condition_per_damage: Some((Fraction::new(1, 2), Condition::Bleeding)),
        ..AttackEnhancementEffect::default()
    },
    ..AttackEnhancement::default()
};

pub const BAD_SWORD: Weapon = Weapon {
    name: "Bad Sword",
    range: WeaponRange::Melee,
    action_point_cost: 3,
    damage: 5,
    grip: WeaponGrip::Versatile,
    attack_attribute: AttackAttribute::Finesse,
    attack_enhancement: Some(SLASHING),
    on_attacked_reaction: Some(PARRY),
    on_true_hit: None,
    sprite: Some(SpriteId::Sword),
    icon: EquipmentIconId::Sword,
    weight: 2,
};

pub const SWORD: Weapon = Weapon {
    name: "Sword",
    range: WeaponRange::Melee,
    action_point_cost: 3,
    damage: 7,
    grip: WeaponGrip::Versatile,
    attack_attribute: AttackAttribute::Finesse,
    attack_enhancement: Some(SLASHING),
    on_attacked_reaction: Some(PARRY),
    on_true_hit: None,
    sprite: Some(SpriteId::Sword),
    icon: EquipmentIconId::Sword,
    weight: 2,
};

const FEINT: AttackEnhancement = AttackEnhancement {
    name: "Feint",
    description: "Reduce the target's defense by 6 against the next attack",
    icon: IconId::Feint,
    effect: AttackEnhancementEffect {
        roll_modifier: -3,
        on_target: Some(ApplyEffect::Condition(ApplyCondition {
            condition: Condition::Distracted,
            stacks: None,
            duration_rounds: Some(1),
        })),
        ..AttackEnhancementEffect::default()
    },
    ..AttackEnhancement::default()
};

pub const BAD_RAPIER: Weapon = Weapon {
    name: "Bad rapier",
    range: WeaponRange::Melee,
    action_point_cost: 3,
    damage: 5,
    grip: WeaponGrip::MainHand,
    attack_attribute: AttackAttribute::Finesse,
    attack_enhancement: Some(FEINT),
    on_attacked_reaction: Some(PARRY),
    on_true_hit: None,
    sprite: Some(SpriteId::Rapier),
    icon: EquipmentIconId::Rapier,
    weight: 2,
};

pub const RAPIER: Weapon = Weapon {
    name: "Rapier",
    range: WeaponRange::Melee,
    action_point_cost: 3,
    damage: 7,
    grip: WeaponGrip::MainHand,
    attack_attribute: AttackAttribute::Finesse,
    attack_enhancement: Some(FEINT),
    on_attacked_reaction: Some(PARRY),
    on_true_hit: None,
    sprite: Some(SpriteId::Rapier),
    icon: EquipmentIconId::Rapier,
    weight: 2,
};

const ALL_IN: AttackEnhancement = AttackEnhancement {
    name: "All-in",
    description: "Deal additional damage and bypass target's armor",
    icon: IconId::AllIn,
    action_point_cost: 1,
    effect: AttackEnhancementEffect {
        bonus_damage: 1,
        armor_penetration: 2,
        ..AttackEnhancementEffect::default()
    },
    ..AttackEnhancement::default()
};

pub const BAD_WAR_HAMMER: Weapon = Weapon {
    name: "Bad war hammer",
    range: WeaponRange::Melee,
    action_point_cost: 3,
    damage: 5,
    grip: WeaponGrip::TwoHanded,
    attack_attribute: AttackAttribute::Strength,
    attack_enhancement: Some(ALL_IN),
    on_attacked_reaction: Some(PARRY),
    on_true_hit: None,
    sprite: Some(SpriteId::Warhammer),
    icon: EquipmentIconId::Warhammer,
    weight: 5,
};

pub const WAR_HAMMER: Weapon = Weapon {
    name: "War hammer",
    range: WeaponRange::Melee,
    action_point_cost: 3,
    // Note: sword held in 2h deals the same as this
    damage: 8,
    grip: WeaponGrip::TwoHanded,
    attack_attribute: AttackAttribute::Strength,
    attack_enhancement: Some(ALL_IN),
    on_attacked_reaction: Some(PARRY),
    on_true_hit: None,
    sprite: Some(SpriteId::Warhammer),
    icon: EquipmentIconId::Warhammer,
    weight: 5,
};

pub const BONE_CRUSHER: Weapon = Weapon {
    name: "Bone crusher",
    range: WeaponRange::Melee,
    action_point_cost: 3,
    // Note: sword held in 2h deals the same as this
    damage: 8,
    grip: WeaponGrip::TwoHanded,
    attack_attribute: AttackAttribute::Strength,
    attack_enhancement: Some(ALL_IN),
    on_attacked_reaction: Some(PARRY),
    on_true_hit: Some(AttackHitEffect::Apply(ApplyEffect::Condition(
        ApplyCondition {
            condition: Condition::Dazed,
            stacks: None,
            duration_rounds: Some(1),
        },
    ))),
    sprite: Some(SpriteId::Warhammer),
    icon: EquipmentIconId::Warhammer,
    weight: 7,
};

pub const BAD_BOW: Weapon = Weapon {
    name: "Bad bow",
    range: WeaponRange::Ranged(5),
    action_point_cost: 3,
    damage: 4,
    grip: WeaponGrip::TwoHanded,
    attack_attribute: AttackAttribute::Agility,
    attack_enhancement: Some(CAREFUL_AIM),
    on_attacked_reaction: None,
    on_true_hit: None,
    sprite: Some(SpriteId::Bow),
    icon: EquipmentIconId::Bow,
    weight: 2,
};

pub const BOW: Weapon = Weapon {
    name: "Bow",
    range: WeaponRange::Ranged(5),
    action_point_cost: 3,
    damage: 7,
    grip: WeaponGrip::TwoHanded,
    attack_attribute: AttackAttribute::Agility,
    attack_enhancement: Some(CAREFUL_AIM),
    on_attacked_reaction: None,
    on_true_hit: None,
    sprite: Some(SpriteId::Bow),
    icon: EquipmentIconId::Bow,
    weight: 2,
};

pub const ELUSIVE_BOW: Weapon = Weapon {
    name: "Elusive bow",
    range: WeaponRange::Ranged(6),
    action_point_cost: 3,
    damage: 7,
    grip: WeaponGrip::TwoHanded,
    attack_attribute: AttackAttribute::Agility,
    attack_enhancement: Some(CAREFUL_AIM),
    on_attacked_reaction: None,
    on_true_hit: None,
    sprite: Some(SpriteId::Bow),
    icon: EquipmentIconId::Bow,
    weight: 2,
};

pub const PENETRATING_ARROWS: Arrow = Arrow {
    name: "Penetrating arrows",
    sprite: None,
    icon: EquipmentIconId::PenetratingArrow,
    bonus_penetration: 3,
    on_damage_apply: None,
    area_effect: None,
};

pub const BARBED_ARROWS: Arrow = Arrow {
    name: "Barbed arrows",
    sprite: None,
    icon: EquipmentIconId::BarbedArrow,
    bonus_penetration: 0,
    on_damage_apply: Some(ApplyEffect::Condition(ApplyCondition {
        condition: Condition::Bleeding,
        stacks: Some(3),
        duration_rounds: None,
    })),
    area_effect: None,
};

pub const COLD_ARROWS: Arrow = Arrow {
    name: "Cold arrows",
    sprite: None,
    icon: EquipmentIconId::ColdArrow,
    bonus_penetration: 0,
    on_damage_apply: Some(ApplyEffect::Condition(ApplyCondition {
        condition: Condition::Hindered,
        stacks: None,
        duration_rounds: Some(1),
    })),
    area_effect: None,
};

pub const EXPLODING_ARROWS: Arrow = Arrow {
    name: "Exploding arrows",
    sprite: None,
    icon: EquipmentIconId::ExplodingArrow,
    bonus_penetration: 0,
    on_damage_apply: None,
    area_effect: Some(AreaEffect {
        radius: Range::Melee,
        acquisition: AreaTargetAcquisition::Everyone,
        effect: AbilityEffect::Negative(AbilityNegativeEffect::Spell(SpellNegativeEffect {
            defense_type: Some(DefenseType::Toughness),
            damage: Some(AbilityDamage::Static(2)),
            on_hit: None,
        })),
    }),
};

pub const BAD_SMALL_SHIELD: Shield = Shield {
    name: "Bad small shield",
    sprite: Some(SpriteId::Shield),
    icon: EquipmentIconId::SmallShield,
    evasion: 2,
    armor: 0,
    on_hit_reaction: None,
    on_attacked_reaction: None,
    weight: 2,
};

pub const SHIELD_BASH: Ability = Ability {
    id: AbilityId::ShieldBash,
    name: "Shield bash",
    description: "Strike at the enemy with your shield, dazing them",
    icon: IconId::ShieldBash,
    action_point_cost: 2,
    stamina_cost: 1,
    mana_cost: 0,
    requirement: Some(EquipmentRequirement::Shield),
    possible_enhancements: [None, None, None],

    target: AbilityTarget::Enemy {
        reach: AbilityReach::Range(Range::Melee),
        effect: AbilityNegativeEffect::Spell(SpellNegativeEffect {
            defense_type: Some(DefenseType::Toughness),
            damage: Some(AbilityDamage::AtLeast(2)),
            on_hit: Some([
                Some(ApplyEffect::Condition(ApplyCondition {
                    condition: Condition::Dazed,
                    duration_rounds: Some(1),
                    stacks: None,
                })),
                None,
            ]),
        }),
        impact_area: None,
    },
    animation_color: GRAY,
    roll: Some(AbilityRollType::RollAbilityWithAttackModifier),
};

pub const ENEMY_TACKLE: Ability = Ability {
    id: AbilityId::Tackle,
    name: "Tackle",
    description: "",
    icon: IconId::Tackle,
    action_point_cost: 2,
    stamina_cost: 0,
    mana_cost: 0,
    requirement: None,
    possible_enhancements: [None, None, None],

    target: AbilityTarget::Enemy {
        reach: AbilityReach::Range(Range::Melee),
        effect: AbilityNegativeEffect::Spell(SpellNegativeEffect {
            defense_type: Some(DefenseType::Toughness),
            damage: Some(AbilityDamage::AtLeast(1)),
            on_hit: Some([
                Some(ApplyEffect::Condition(ApplyCondition {
                    condition: Condition::Dazed,
                    duration_rounds: Some(1),
                    stacks: None,
                })),
                None,
            ]),
        }),
        impact_area: None,
    },
    animation_color: RED,
    roll: Some(AbilityRollType::RollAbilityWithAttackModifier),
};

pub const SMALL_SHIELD: Shield = Shield {
    name: "Small shield",
    sprite: Some(SpriteId::Shield),
    icon: EquipmentIconId::SmallShield,
    evasion: 3,
    armor: 0,
    on_hit_reaction: None,
    on_attacked_reaction: None,
    weight: 2,
};

pub const MEDIUM_SHIELD: Shield = Shield {
    name: "Medium shield",
    sprite: Some(SpriteId::Shield),
    icon: EquipmentIconId::MediumShield,
    evasion: 3,
    armor: 1,
    on_hit_reaction: None,
    on_attacked_reaction: Some(OnAttackedReaction {
        id: OnAttackedReactionId::Block,
        name: "Block",
        description: "Attempt to block an incoming ranged attack",
        // TODO: make better icon
        icon: IconId::RangedAttack,
        action_point_cost: 1,
        stamina_cost: 1,
        effect: OnAttackedReactionEffect {
            bonus_evasion: 5,
            bonus_armor: 1,
        },
        required_circumstance: Some(AttackCircumstance::Ranged),
    }),
    weight: 3,
};

pub const QUICK: AttackEnhancement = AttackEnhancement {
    name: "Quick strike",
    description: "", //"Strike more quickly",
    icon: IconId::QuickStrike,
    stamina_cost: 2,
    weapon_requirement: Some(WeaponType::Melee),
    effect: AttackEnhancementEffect {
        action_point_discount: 1,
        ..AttackEnhancementEffect::default()
    },
    ..AttackEnhancement::default()
};

pub const SMITE: AttackEnhancement = AttackEnhancement {
    name: "Smite",
    description: "", //"Enhance your strike with magic",
    icon: IconId::Smite,
    mana_cost: 1,
    effect: AttackEnhancementEffect {
        bonus_damage: 2,
        armor_penetration: 1,
        ..AttackEnhancementEffect::default()
    },
    ..AttackEnhancement::default()
};

pub const OVERWHELMING: AttackEnhancement = AttackEnhancement {
    name: "Overwhelm",
    description: "", //"Overwhelm the target",
    icon: IconId::CrushingStrike,
    stamina_cost: 2,
    weapon_requirement: Some(WeaponType::Melee),
    effect: AttackEnhancementEffect {
        on_damage_effect: Some(AttackEnhancementOnHitEffect::Target(
            ApplyEffect::RemoveActionPoints(2),
        )),
        ..AttackEnhancementEffect::default()
    },
    ..AttackEnhancement::default()
};

pub const CAREFUL_AIM: AttackEnhancement = AttackEnhancement {
    name: "Careful aim",
    description: "", // "Spend more time on the attack, aiming carefully",
    icon: IconId::CarefulAim,
    action_point_cost: 1,
    effect: AttackEnhancementEffect {
        roll_advantage: 1,
        ..AttackEnhancementEffect::default()
    },
    ..AttackEnhancement::default()
};

pub const LONGER_REACH: AttackEnhancement = AttackEnhancement {
    name: "Longer reach",
    description: "",
    icon: IconId::Extend,
    weapon_requirement: Some(WeaponType::Ranged),
    effect: AttackEnhancementEffect {
        roll_advantage: -1,
        range_bonus: 2,
        ..AttackEnhancementEffect::default()
    },
    ..AttackEnhancement::default()
};

pub const EMPOWER: AttackEnhancement = AttackEnhancement {
    name: "Empower",
    icon: IconId::TrueStrike,
    stamina_cost: 1,
    effect: AttackEnhancementEffect {
        bonus_damage: 1,
        //armor_penetration: 2,
        ..AttackEnhancementEffect::default()
    },
    ..AttackEnhancement::default()
};

pub const CRIPPLING_SHOT: AttackEnhancement = AttackEnhancement {
    name: "Crippling shot",
    icon: IconId::CripplingShot,
    stamina_cost: 2,
    effect: AttackEnhancementEffect {
        on_damage_effect: Some(AttackEnhancementOnHitEffect::Target(
            ApplyEffect::Condition(ApplyCondition {
                condition: Condition::Hindered,
                stacks: None,
                duration_rounds: Some(1),
            }),
        )),
        ..AttackEnhancementEffect::default()
    },
    weapon_requirement: Some(WeaponType::Ranged),
    ..AttackEnhancement::default()
};

pub const PARRY: OnAttackedReaction = OnAttackedReaction {
    id: OnAttackedReactionId::Parry,
    name: "Parry",
    description: "Attempt to parry an incoming melee attack",
    icon: IconId::Parry,
    action_point_cost: 1,
    stamina_cost: 1,
    effect: OnAttackedReactionEffect {
        bonus_evasion: 5,
        bonus_armor: 1,
    },
    required_circumstance: Some(AttackCircumstance::Melee),
};

pub const SIDE_STEP: OnAttackedReaction = OnAttackedReaction {
    id: OnAttackedReactionId::SideStep,
    name: "Side step",
    description: "Attempt to dodge an incoming attack",
    icon: IconId::Sidestep,
    action_point_cost: 1,
    stamina_cost: 2,
    effect: OnAttackedReactionEffect {
        bonus_evasion: 5,
        bonus_armor: 0,
    },
    required_circumstance: None,
};

pub const RAGE: OnHitReaction = OnHitReaction {
    name: "Rage",
    description: Condition::Raging.description(),
    icon: IconId::Rage,
    action_point_cost: 1,
    stamina_cost: 1,
    effect: OnHitReactionEffect::Rage,
    required_circumstance: None,
};

pub const SWEEP_ATTACK_PRECISE: AbilityEnhancement = AbilityEnhancement {
    ability_id: AbilityId::SweepAttack,
    name: "Precise",
    description: "Increase your precision",
    icon: IconId::Precision,
    action_point_cost: 0,
    mana_cost: 0,
    stamina_cost: 1,
    attack_effect: Some(AttackEnhancementEffect {
        roll_modifier: 3,
        ..AttackEnhancementEffect::default()
    }),
    spell_effect: None,
};
pub const SWEEP_ATTACK: Ability = Ability {
    id: AbilityId::SweepAttack,
    name: "Sweeping attack",
    description: "Attack all surrounding enemies",
    icon: IconId::SweepAttack,
    action_point_cost: 3,
    mana_cost: 0,
    stamina_cost: 3,
    requirement: Some(EquipmentRequirement::Weapon(WeaponType::Melee)),

    roll: Some(AbilityRollType::RollDuringAttack(-3)),
    possible_enhancements: [Some(SWEEP_ATTACK_PRECISE), None, None],
    target: AbilityTarget::None {
        self_area: Some(AreaEffect {
            radius: Range::Melee,
            acquisition: AreaTargetAcquisition::Enemies,
            effect: AbilityEffect::Negative(AbilityNegativeEffect::PerformAttack),
        }),
        self_effect: None,
    },
    animation_color: MAGENTA,
};

pub const LUNGE_ATTACK_HEAVY_IMPACT: AbilityEnhancement = AbilityEnhancement {
    ability_id: AbilityId::LungeAttack,
    name: "Heavy impact",
    description: "",
    icon: IconId::CrushingStrike,
    action_point_cost: 0,
    mana_cost: 0,
    stamina_cost: 1,
    attack_effect: Some(AttackEnhancementEffect {
        on_damage_effect: Some(AttackEnhancementOnHitEffect::Target(
            ApplyEffect::RemoveActionPoints(1),
        )),
        ..AttackEnhancementEffect::default()
    }),
    spell_effect: None,
};
pub const LUNGE_ATTACK_REACH: AbilityEnhancement = AbilityEnhancement {
    ability_id: AbilityId::LungeAttack,
    name: "Reach",
    description: "",
    icon: IconId::Extend,
    action_point_cost: 0,
    mana_cost: 0,
    stamina_cost: 1,
    spell_effect: Some(SpellEnhancementEffect {
        increased_range_tenths: 10,
        ..SpellEnhancementEffect::default()
    }),
    attack_effect: Some(AttackEnhancementEffect {
        range_bonus: 1,
        ..AttackEnhancementEffect::default()
    }),
};
pub const LUNGE_ATTACK: Ability = Ability {
    id: AbilityId::LungeAttack,
    name: "Lunge attack",
    description: "Move to target in an unobstructed path, before attacking",
    icon: IconId::LungeAttack,
    action_point_cost: 2,
    mana_cost: 0,
    stamina_cost: 2,
    requirement: Some(EquipmentRequirement::Weapon(WeaponType::Melee)),

    roll: Some(AbilityRollType::RollDuringAttack(0)),
    possible_enhancements: [
        Some(LUNGE_ATTACK_HEAVY_IMPACT),
        Some(LUNGE_ATTACK_REACH),
        None,
    ],
    target: AbilityTarget::Enemy {
        //reach: AbilityReach::MoveIntoMelee(Range::Float(3.99)),
        reach: AbilityReach::MoveIntoMelee(Range::Float(3.5)),
        effect: AbilityNegativeEffect::PerformAttack,
        impact_area: None,
    },
    animation_color: MAGENTA,
};

// TODO Should not be possible to use Brace if you already have that number of Protected stacks
pub const ENEMY_BRACE: Ability = Ability {
    id: AbilityId::Brace,
    name: "Brace",
    description: "",
    icon: IconId::Brace,
    action_point_cost: 1,
    mana_cost: 0,
    stamina_cost: 0,
    requirement: Some(EquipmentRequirement::Shield),
    roll: None,
    possible_enhancements: [None; 3],
    target: AbilityTarget::None {
        self_area: None,
        self_effect: Some(AbilityPositiveEffect {
            healing: 0,
            apply: Some([
                Some(ApplyEffect::Condition(ApplyCondition {
                    condition: Condition::Protected,
                    stacks: Some(2),
                    duration_rounds: None,
                })),
                None,
            ]),
        }),
    },
    animation_color: MAGENTA,
};

pub const BRACE: Ability = Ability {
    id: AbilityId::Brace,
    name: "Brace",
    description: "",
    icon: IconId::Brace,
    action_point_cost: 1,
    mana_cost: 0,
    stamina_cost: 1,
    requirement: Some(EquipmentRequirement::Shield),
    roll: None,
    possible_enhancements: [None; 3],
    target: AbilityTarget::None {
        self_area: None,
        self_effect: Some(AbilityPositiveEffect {
            healing: 0,
            apply: Some([
                Some(ApplyEffect::Condition(ApplyCondition {
                    condition: Condition::Protected,
                    stacks: Some(2),
                    duration_rounds: None,
                })),
                None,
            ]),
        }),
    },
    animation_color: MAGENTA,
};

pub const SCREAM_SHRIEK: AbilityEnhancement = AbilityEnhancement {
    ability_id: AbilityId::Scream,
    name: "Shriek",
    description: "Increased range",
    icon: IconId::Banshee,
    action_point_cost: 0,
    mana_cost: 1,
    stamina_cost: 0,
    attack_effect: None,
    spell_effect: Some(SpellEnhancementEffect {
        increased_radius_tenths: 15,
        ..SpellEnhancementEffect::default()
    }),
};
pub const SCREAM: Ability = Ability {
    id: AbilityId::Scream,
    name: "Scream",
    description: "Daze nearby enemies",
    icon: IconId::Scream,
    action_point_cost: 2,
    mana_cost: 1,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    target: AbilityTarget::None {
        self_area: Some(AreaEffect {
            radius: Range::Float(2.5),
            acquisition: AreaTargetAcquisition::Enemies,
            effect: AbilityEffect::Negative(AbilityNegativeEffect::Spell(SpellNegativeEffect {
                defense_type: Some(DefenseType::Will),
                damage: None,
                on_hit: Some([
                    Some(ApplyEffect::Condition(ApplyCondition {
                        condition: Condition::Dazed,
                        stacks: None,
                        duration_rounds: Some(1),
                    })),
                    None,
                ]),
            })),
        }),
        self_effect: None,
    },
    possible_enhancements: [Some(SCREAM_SHRIEK), None, None],

    animation_color: BLUE,
};

pub const SHACKLED_MIND: Ability = Ability {
    id: AbilityId::ShackledMind,
    name: "Shackle",
    description: "Shackle an enemy's mind, slowing them and lowering their defenses",
    icon: IconId::ShackledMind,
    action_point_cost: 3,
    mana_cost: 2,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    target: AbilityTarget::Enemy {
        reach: AbilityReach::Range(Range::Float(4.0)),
        effect: AbilityNegativeEffect::Spell(SpellNegativeEffect {
            defense_type: Some(DefenseType::Will),
            damage: None,
            on_hit: Some([
                Some(ApplyEffect::Condition(ApplyCondition {
                    condition: Condition::Slowed,
                    stacks: None,
                    duration_rounds: Some(2),
                })),
                Some(ApplyEffect::Condition(ApplyCondition {
                    condition: Condition::Exposed,
                    stacks: None,
                    duration_rounds: Some(2),
                })),
            ]),
        }),
        impact_area: None,
    },
    possible_enhancements: [
        Some(AbilityEnhancement {
            ability_id: AbilityId::ShackledMind,
            name: "Reach",
            description: "",
            icon: IconId::Extend,
            action_point_cost: 0,
            mana_cost: 1,
            stamina_cost: 0,
            attack_effect: None,
            spell_effect: Some(SpellEnhancementEffect {
                increased_range_tenths: 30,
                ..SpellEnhancementEffect::default()
            }),
        }),
        Some(AbilityEnhancement {
            ability_id: AbilityId::ShackledMind,
            name: "Focus",
            description: "",
            icon: IconId::SpellAdvantage,
            action_point_cost: 0,
            mana_cost: 1,
            stamina_cost: 0,
            attack_effect: None,
            spell_effect: Some(SpellEnhancementEffect {
                bonus_advantage: 1,
                ..SpellEnhancementEffect::default()
            }),
        }),
        None,
    ],

    animation_color: PURPLE,
};

pub const MIND_BLAST: Ability = Ability {
    id: AbilityId::MindBlast,
    name: "Mind blast",
    description: "Assault an enemy's mind, damaging and disrupting them",
    icon: IconId::Mindblast,
    action_point_cost: 2,
    mana_cost: 1,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    possible_enhancements: [
        Some(AbilityEnhancement {
            ability_id: AbilityId::MindBlast,
            name: "Dualcast",
            description: "Spell is cast twice",
            icon: IconId::Dualcast,
            action_point_cost: 1,
            mana_cost: 1,
            stamina_cost: 0,
            attack_effect: None,
            spell_effect: Some(SpellEnhancementEffect {
                cast_twice: true,
                ..SpellEnhancementEffect::default()
            }),
        }),
        None,
        None,
    ],
    target: AbilityTarget::Enemy {
        effect: AbilityNegativeEffect::Spell(SpellNegativeEffect {
            defense_type: Some(DefenseType::Will),
            damage: Some(AbilityDamage::Static(1)),
            on_hit: Some([Some(ApplyEffect::RemoveActionPoints(1)), None]),
        }),
        impact_area: None,
        reach: AbilityReach::Range(Range::Ranged(5)),
    },
    animation_color: PURPLE,
};

pub const INFLICT_WOUNDS_NECROTIC_INFLUENCE: AbilityEnhancement = AbilityEnhancement {
    ability_id: AbilityId::InflictWounds,
    name: "Necrotic influence",
    description: "Converts all bleeding to immediate damage and life-drain",
    icon: IconId::NecroticInfluence,
    action_point_cost: 0,
    mana_cost: 1,
    stamina_cost: 0,
    spell_effect: Some(SpellEnhancementEffect {
        target_on_hit: Some([
            Some(ApplyEffect::PerBleeding {
                damage: 1,
                caster_healing_percentage: 40,
            }),
            Some(ApplyEffect::ConsumeCondition {
                condition: Condition::Bleeding,
            }),
        ]),
        ..SpellEnhancementEffect::default()
    }),
    attack_effect: None,
};
pub const INFLICT_WOUNDS: Ability = Ability {
    id: AbilityId::InflictWounds,
    name: "Inflict wounds",
    description: "",
    icon: IconId::NecroticInfluence,
    action_point_cost: 3,
    mana_cost: 1,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    possible_enhancements: [Some(INFLICT_WOUNDS_NECROTIC_INFLUENCE), None, None],
    target: AbilityTarget::Enemy {
        reach: AbilityReach::Range(Range::Float(3.5)),
        effect: AbilityNegativeEffect::Spell(SpellNegativeEffect {
            defense_type: Some(DefenseType::Toughness),
            damage: None,
            on_hit: Some([
                Some(ApplyEffect::Condition(ApplyCondition {
                    condition: Condition::Bleeding,
                    stacks: Some(6),
                    duration_rounds: None,
                })),
                None,
            ]),
        }),
        impact_area: None,
    },
    animation_color: PURPLE,
};

pub const MAGI_INFLICT_WOUNDS: Ability = Ability {
    id: AbilityId::MagiInflictWounds,
    name: "Inflict wounds",
    description: "",
    icon: IconId::Mindblast,
    action_point_cost: 3,
    mana_cost: 0,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    possible_enhancements: [None, None, None],
    target: AbilityTarget::Enemy {
        effect: AbilityNegativeEffect::Spell(SpellNegativeEffect {
            defense_type: Some(DefenseType::Toughness),
            damage: None,
            on_hit: Some([
                Some(ApplyEffect::Condition(ApplyCondition {
                    condition: Condition::Bleeding,
                    stacks: Some(6),
                    duration_rounds: None,
                })),
                None,
            ]),
        }),
        impact_area: None,
        reach: AbilityReach::Range(Range::Ranged(5)),
    },
    animation_color: BROWN,
};

pub const MAGI_INFLICT_HORRORS: Ability = Ability {
    id: AbilityId::MagiInflictHorrors,
    name: "Inflict horrors",
    description: "",
    icon: IconId::Mindblast,
    action_point_cost: 3,
    mana_cost: 0,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    possible_enhancements: [None, None, None],
    target: AbilityTarget::Enemy {
        effect: AbilityNegativeEffect::Spell(SpellNegativeEffect {
            defense_type: Some(DefenseType::Will),
            damage: None,
            on_hit: Some([
                Some(ApplyEffect::Condition(ApplyCondition {
                    condition: Condition::Slowed,
                    stacks: None,
                    duration_rounds: Some(1),
                })),
                None,
            ]),
        }),
        impact_area: None,
        reach: AbilityReach::Range(Range::Ranged(5)),
    },
    animation_color: PURPLE,
};

pub const MAGI_HEAL: Ability = Ability {
    id: AbilityId::MagiHeal,
    name: "Heal",
    description: "",
    icon: IconId::Heal,
    action_point_cost: 3,
    mana_cost: 0,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    target: AbilityTarget::Ally {
        range: Range::Ranged(5),
        effect: AbilityPositiveEffect {
            healing: 5,
            apply: None,
        },
    },
    possible_enhancements: [None, None, None],
    animation_color: LIME,
};

pub const HEAL_ENERGIZE: AbilityEnhancement = AbilityEnhancement {
    ability_id: AbilityId::Heal,
    name: "Energize",
    description: "",
    icon: IconId::Energize,
    action_point_cost: 0,
    mana_cost: 1,
    stamina_cost: 0,
    attack_effect: None,
    spell_effect: Some(SpellEnhancementEffect {
        target_on_hit: Some([Some(ApplyEffect::GainStamina(2)), None]),
        ..SpellEnhancementEffect::default()
    }),
};
pub const HEAL: Ability = Ability {
    id: AbilityId::Heal,
    name: "Heal",
    description: "Restore an ally's health",
    icon: IconId::Heal,
    action_point_cost: 2,
    mana_cost: 1,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    target: AbilityTarget::Ally {
        range: Range::Ranged(3),
        effect: AbilityPositiveEffect {
            healing: 4,
            apply: Some([
                Some(ApplyEffect::ConsumeCondition {
                    condition: Condition::Bleeding,
                }),
                Some(ApplyEffect::ConsumeCondition {
                    condition: Condition::Burning,
                }),
            ]),
        },
    },
    possible_enhancements: [
        Some(AbilityEnhancement {
            ability_id: AbilityId::Heal,
            name: "Reach",
            description: "",
            icon: IconId::Extend,
            action_point_cost: 0,
            mana_cost: 1,
            stamina_cost: 0,
            attack_effect: None,
            spell_effect: Some(SpellEnhancementEffect {
                increased_range_tenths: 20,
                ..SpellEnhancementEffect::default()
            }),
        }),
        // TODO add enhancement that heals over time (1 per round for 3 turns?)
        Some(HEAL_ENERGIZE),
        None,
    ],

    animation_color: GREEN,
};

pub const HEALING_NOVA: Ability = Ability {
    id: AbilityId::HealingNova,
    name: "Healing nova",
    description: "Restore health to nearby allies",
    icon: IconId::PlusPlus,
    action_point_cost: 3,
    mana_cost: 1,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    possible_enhancements: [None, None, None],
    target: AbilityTarget::None {
        self_area: Some(AreaEffect {
            radius: Range::Ranged(4),
            acquisition: AreaTargetAcquisition::Allies,
            effect: AbilityEffect::Positive(AbilityPositiveEffect {
                healing: 1,
                apply: None,
            }),
        }),
        self_effect: None,
    },
    animation_color: GREEN,
};

pub const ENEMY_SELF_HEAL: Ability = Ability {
    id: AbilityId::SelfHeal,
    name: "Self heal",
    description: "Restore the caster's health and grants protection",
    icon: IconId::PlusPlus,
    action_point_cost: 2,
    mana_cost: 0,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    possible_enhancements: [None, None, None],
    target: AbilityTarget::None {
        self_area: None,
        self_effect: Some(AbilityPositiveEffect {
            healing: 1,
            apply: Some([
                Some(ApplyEffect::Condition(ApplyCondition {
                    condition: Condition::Protected,
                    stacks: Some(1),
                    duration_rounds: None,
                })),
                None,
            ]),
        }),
    },
    animation_color: GREEN,
};

pub const SELF_HEAL: Ability = Ability {
    id: AbilityId::SelfHeal,
    name: "Self heal",
    description: "Restore the caster's health and grants protection",
    icon: IconId::PlusPlus,
    action_point_cost: 2,
    mana_cost: 1,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    possible_enhancements: [None, None, None],
    target: AbilityTarget::None {
        self_area: None,
        self_effect: Some(AbilityPositiveEffect {
            healing: 1,
            apply: Some([
                Some(ApplyEffect::Condition(ApplyCondition {
                    condition: Condition::Protected,
                    stacks: Some(1),
                    duration_rounds: None,
                })),
                None,
            ]),
        }),
    },
    animation_color: GREEN,
};

pub const HEALING_RAIN: Ability = Ability {
    id: AbilityId::HealingRain,
    name: "Healing rain",
    description: "Restore health to allies in an area",
    icon: IconId::PlusPlus,
    action_point_cost: 3,
    mana_cost: 2,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    possible_enhancements: [None, None, None],
    target: AbilityTarget::Area {
        range: Range::Ranged(5),
        area_effect: AreaEffect {
            radius: Range::Float(1.95),
            acquisition: AreaTargetAcquisition::Allies,
            effect: AbilityEffect::Positive(AbilityPositiveEffect {
                healing: 1,
                apply: None,
            }),
        },
    },
    animation_color: GREEN,
};

pub const FIREBALL_REACH: AbilityEnhancement = AbilityEnhancement {
    ability_id: AbilityId::Fireball,
    name: "Reach",
    description: "",
    icon: IconId::Extend,
    action_point_cost: 0,
    mana_cost: 1,
    stamina_cost: 0,
    attack_effect: None,
    spell_effect: Some(SpellEnhancementEffect {
        increased_range_tenths: 30,
        ..SpellEnhancementEffect::default()
    }),
};

pub const FIREBALL_MASSIVE: AbilityEnhancement = AbilityEnhancement {
    ability_id: AbilityId::Fireball,
    name: "Massive",
    description: "",
    icon: IconId::Radius,
    action_point_cost: 0,
    mana_cost: 1,
    stamina_cost: 0,
    attack_effect: None,
    spell_effect: Some(SpellEnhancementEffect {
        increased_radius_tenths: 10,
        ..SpellEnhancementEffect::default()
    }),
};
pub const FIREBALL_INFERNO: AbilityEnhancement = AbilityEnhancement {
    ability_id: AbilityId::Fireball,
    name: "Inferno",
    description: "Targets hit by the impact start burning",
    icon: IconId::Inferno,
    action_point_cost: 0,
    mana_cost: 1,
    stamina_cost: 0,
    attack_effect: None,
    spell_effect: Some(SpellEnhancementEffect {
        area_on_hit: Some([
            Some(ApplyEffect::Condition(ApplyCondition {
                condition: Condition::Burning,
                stacks: Some(3),
                duration_rounds: None,
            })),
            None,
        ]),
        ..SpellEnhancementEffect::default()
    }),
};
pub const FIREBALL: Ability = Ability {
    id: AbilityId::Fireball,
    name: "Fireball",
    description: "Hurl fire at an enemy, dealing area damage",
    icon: IconId::Fireball,
    action_point_cost: 3,
    mana_cost: 2,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    target: AbilityTarget::Enemy {
        reach: AbilityReach::Range(Range::Float(4.5)),
        effect: AbilityNegativeEffect::Spell(SpellNegativeEffect {
            defense_type: Some(DefenseType::Evasion),
            damage: Some(AbilityDamage::AtLeast(3)),
            on_hit: None,
        }),
        impact_area: Some((
            Range::Ranged(2),
            AreaTargetAcquisition::Everyone,
            AbilityNegativeEffect::Spell(SpellNegativeEffect {
                defense_type: Some(DefenseType::Toughness),
                damage: Some(AbilityDamage::AtLeast(3)),
                on_hit: None,
            }),
        )),
    },
    possible_enhancements: [
        Some(FIREBALL_REACH),
        Some(FIREBALL_MASSIVE),
        Some(FIREBALL_INFERNO),
    ],

    animation_color: RED,
};

pub const KILL: Ability = Ability {
    id: AbilityId::Kill,
    name: "Kill",
    description: "Kill an enemy",
    icon: IconId::Fireball,
    action_point_cost: 1,
    mana_cost: 0,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    possible_enhancements: [None; 3],
    target: AbilityTarget::Enemy {
        effect: AbilityNegativeEffect::Spell(SpellNegativeEffect {
            defense_type: None,
            damage: Some(AbilityDamage::Static(99)),
            on_hit: None,
        }),
        impact_area: None,
        reach: AbilityReach::Range(Range::Ranged(10)),
    },
    animation_color: BLACK,
};

pub const SEARING_LIGHT_BURN: AbilityEnhancement = AbilityEnhancement {
    ability_id: AbilityId::SearingLight,
    name: "Burn",
    description: "",
    icon: IconId::Inferno,
    action_point_cost: 0,
    mana_cost: 1,
    stamina_cost: 0,
    spell_effect: Some(SpellEnhancementEffect {
        target_on_hit: Some([
            Some(ApplyEffect::Condition(ApplyCondition {
                condition: Condition::Burning,
                stacks: Some(2),
                duration_rounds: None,
            })),
            None,
        ]),
        ..SpellEnhancementEffect::default()
    }),
    attack_effect: None,
};
pub const SEARING_LIGHT: Ability = Ability {
    id: AbilityId::SearingLight,
    name: "Searing light",
    description: "Envelops the target in blinding light",
    icon: IconId::SearingLight,
    action_point_cost: 3,
    mana_cost: 1,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    possible_enhancements: [Some(SEARING_LIGHT_BURN), None, None],
    target: AbilityTarget::Enemy {
        effect: AbilityNegativeEffect::Spell(SpellNegativeEffect {
            defense_type: Some(DefenseType::Toughness),
            damage: Some(AbilityDamage::AtLeast(3)),
            on_hit: Some([
                Some(ApplyEffect::Condition(ApplyCondition {
                    condition: Condition::Blinded,
                    stacks: None,
                    duration_rounds: Some(1),
                })),
                None,
            ]),
        }),
        impact_area: None,
        reach: AbilityReach::Range(Range::Ranged(3)),
    },
    animation_color: YELLOW,
};

pub const HEALTH_POTION: Consumable = Consumable {
    name: "Health potion",
    icon: EquipmentIconId::HealthPotion,
    effect: Some(ApplyEffect::Condition(ApplyCondition {
        condition: Condition::HealthPotionHealing,
        stacks: Some(3),
        duration_rounds: None,
    })),
    ..Consumable::default()
};

pub const MANA_POTION: Consumable = Consumable {
    name: "Mana potion",
    icon: EquipmentIconId::ManaPotion,
    mana_gain: 5,
    ..Consumable::default()
};

pub const ADRENALIN_POTION: Consumable = Consumable {
    name: "Adrenalin potion",
    icon: EquipmentIconId::AdrenalinPotion,
    effect: Some(ApplyEffect::Condition(ApplyCondition {
        condition: Condition::Adrenalin,
        stacks: None,
        duration_rounds: Some(3),
    })),
    ..Consumable::default()
};

pub const ENERGY_POTION: Consumable = Consumable {
    name: "Energy potion",
    icon: EquipmentIconId::EnergyPotion,
    effect: Some(ApplyEffect::GainStamina(10)),
    ..Consumable::default()
};

pub const ARCANE_POTION: Consumable = Consumable {
    name: "Arcane potion",
    icon: EquipmentIconId::ArcanePotion,
    effect: Some(ApplyEffect::Condition(ApplyCondition {
        condition: Condition::ArcaneProwess,
        stacks: None,
        duration_rounds: Some(2),
    })),
    ..Consumable::default()
};

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PassiveSkill {
    HardenedSkin,
    WeaponProficiency,
    ArcaneSurge,
    Reaper,
    BloodRage,
    ThrillOfBattle,
    Honorless,
    Vigilant,
}

impl PassiveSkill {
    pub fn name(&self) -> &'static str {
        use PassiveSkill::*;
        match self {
            HardenedSkin => "Hardened skin",
            WeaponProficiency => "Weapon proficiency",
            ArcaneSurge => "Arcane surge",
            Reaper => "Reaper",
            BloodRage => "Blood rage",
            ThrillOfBattle => "Thrill of battle",
            Honorless => "Honorless",
            Vigilant => "Vigilant",
        }
    }

    pub fn icon(&self) -> IconId {
        use PassiveSkill::*;
        match self {
            HardenedSkin => IconId::HardenedSkin,
            WeaponProficiency => IconId::WeaponProficiency,
            ArcaneSurge => IconId::ArcaneSurge,
            Reaper => IconId::Reaper,
            // TODO: unique icon
            BloodRage => IconId::Rage,
            // TODO: unique icon
            ThrillOfBattle => IconId::MeleeAttack,
            // TODO: unique icon
            Honorless => IconId::RangedAttack,
            // TODO: unique icon
            Vigilant => IconId::MeleeAttack,
        }
    }

    pub fn description(&self) -> &'static str {
        use PassiveSkill::*;
        match self {
            HardenedSkin => "+1 armor",
            WeaponProficiency => "Attacks gain +1 armor penetration",
            ArcaneSurge => "+3 spell modifier while at/below 50% mana",
            Reaper => "On kill: gain 1 stamina, 1 AP (max 1 AP per turn)",
            BloodRage => "+3 attack modifier while at/below 50% health. Immune to the negative effects of Near-death",
            ThrillOfBattle => "+3 attack/spell modifier while adjacent to more than one enemy. Immune to Flanked.",
            Honorless => "Attacks deal +1 damage against Flanked targets",
            Vigilant => "Can opportunity attack an adjacent enemy even if you are not engaging them"
        }
    }

    pub fn keywords(&self) -> &'static [Condition] {
        use PassiveSkill::*;
        match self {
            BloodRage => &[Condition::NearDeath],
            _ => &[],
        }
    }
}
