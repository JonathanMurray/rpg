use macroquad::color::{BLACK, BLUE, BROWN, GREEN, LIME, MAGENTA, PURPLE, RED};

use crate::{
    core::{
        Ability, AbilityDamage, AbilityEffect, AbilityEnhancement, AbilityId,
        AbilityNegativeEffect, AbilityPositiveEffect, AbilityReach, AbilityRollType, AbilityTarget,
        ApplyEffect, AreaTargetAcquisition, ArmorPiece, AttackAttribute, AttackCircumstance,
        AttackEnhancement, AttackEnhancementEffect, AttackEnhancementOnHitEffect, Condition,
        Consumable, DefenseType, EquipEffect, OnAttackedReaction, OnAttackedReactionEffect,
        OnAttackedReactionId, OnHitReaction, OnHitReactionEffect, Range, Shield,
        SpellEnhancementEffect, SpellNegativeEffect, Weapon, WeaponGrip, WeaponRange, WeaponType,
    },
    textures::{EquipmentIconId, IconId, SpriteId},
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
    protection: 4,
    limit_evasion_from_agi: Some(4),
    icon: EquipmentIconId::ChainMail,
    weight: 3,
    equip: EquipEffect::default(),
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

pub const STABBING: AttackEnhancement = AttackEnhancement {
    name: "Stabbing",
    icon: IconId::Stabbing,
    effect: AttackEnhancementEffect {
        roll_modifier: -3,
        inflict_condition_per_damage: Some(Condition::Weakened(1)),
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
    action_point_cost: 3,
    damage: 4,
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
    effect: AttackEnhancementEffect {
        roll_modifier: -3,
        inflict_condition_per_damage: Some(Condition::Bleeding(1)),
        ..AttackEnhancementEffect::default()
    },
    ..AttackEnhancement::default()
};

pub const BAD_SWORD: Weapon = Weapon {
    name: "Bad Sword",
    range: WeaponRange::Melee,
    action_point_cost: 3,
    damage: 3,
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
    damage: 4,
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
        on_target: Some(ApplyEffect::Condition(Condition::Distracted)),
        ..AttackEnhancementEffect::default()
    },
    ..AttackEnhancement::default()
};

pub const BAD_RAPIER: Weapon = Weapon {
    name: "Bad rapier",
    range: WeaponRange::Melee,
    action_point_cost: 3,
    damage: 3,
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
    damage: 4,
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
    damage: 4,
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

pub const BAD_BOW: Weapon = Weapon {
    name: "Bad bow",
    range: WeaponRange::Ranged(5),
    action_point_cost: 3,
    damage: 3,
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

pub const BAD_SMALL_SHIELD: Shield = Shield {
    name: "Bad small shield",
    sprite: Some(SpriteId::Shield),
    icon: EquipmentIconId::SmallShield,
    evasion: 1,
    on_hit_reaction: None,
    on_attacked_reaction: None,
    weight: 2,
};

pub const SMALL_SHIELD: Shield = Shield {
    name: "Small shield",
    sprite: Some(SpriteId::Shield),
    icon: EquipmentIconId::SmallShield,
    evasion: 3,
    on_hit_reaction: Some(OnHitReaction {
        name: "Shield bash",
        description: "Strike back at the attacker with your shield",
        icon: IconId::ShieldBash,
        action_point_cost: 1,
        stamina_cost: 0,
        effect: OnHitReactionEffect::ShieldBash,
        required_circumstance: Some(AttackCircumstance::Melee),
    }),
    on_attacked_reaction: Some(OnAttackedReaction {
        id: OnAttackedReactionId::Block,
        name: "Block",
        description: "Attempt to block an incoming ranged attack",
        // TODO: make better icon
        icon: IconId::RangedAttack,
        action_point_cost: 1,
        stamina_cost: 1,
        effect: OnAttackedReactionEffect { bonus_evasion: 5 },
        required_circumstance: Some(AttackCircumstance::Ranged),
    }),
    weight: 2,
};

pub const QUICK: AttackEnhancement = AttackEnhancement {
    name: "Quick strike",
    description: "", //"Strike more quickly",
    icon: IconId::QuickStrike,
    stamina_cost: 3,
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
        bonus_damage: 1,
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

pub const TRUE_STRIKE: AttackEnhancement = AttackEnhancement {
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
            ApplyEffect::Condition(Condition::Hindered(1)),
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
    effect: OnAttackedReactionEffect { bonus_evasion: 5 },
    required_circumstance: Some(AttackCircumstance::Melee),
};

pub const SIDE_STEP: OnAttackedReaction = OnAttackedReaction {
    id: OnAttackedReactionId::SideStep,
    name: "Side step",
    description: "Attempt to dodge an incoming attack",
    icon: IconId::Sidestep,
    action_point_cost: 1,
    stamina_cost: 2,
    effect: OnAttackedReactionEffect { bonus_evasion: 5 },
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
    stamina_cost: 2,
    attack_effect: Some(AttackEnhancementEffect {
        roll_modifier: 3,
        ..AttackEnhancementEffect::default()
    }),
    spell_effect: None,
};
pub const SWEEP_ATTACK: Ability = Ability {
    id: AbilityId::SweepAttack,
    name: "Sweeping attack",
    description: "Target all enemies around you",
    icon: IconId::SweepAttack,
    action_point_cost: 2,
    mana_cost: 0,
    stamina_cost: 2,
    weapon_requirement: Some(WeaponType::Melee),

    roll: Some(AbilityRollType::Attack(-3)),
    possible_enhancements: [Some(SWEEP_ATTACK_PRECISE), None, None],
    target: AbilityTarget::None {
        self_area: Some((
            Range::Melee,
            AreaTargetAcquisition::Enemies,
            AbilityEffect::Negative(AbilityNegativeEffect::Attack),
        )),
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
    stamina_cost: 2,
    attack_effect: Some(AttackEnhancementEffect {
        on_damage_effect: Some(AttackEnhancementOnHitEffect::Target(
            ApplyEffect::RemoveActionPoints(1),
        )),
        ..AttackEnhancementEffect::default()
    }),
    spell_effect: None,
};
pub const LUNGE_ATTACK: Ability = Ability {
    id: AbilityId::LungeAttack,
    name: "Lunge attack",
    description: "Move to target in an unobstructed path, before attacking",
    icon: IconId::LungeAttack,
    action_point_cost: 2,
    mana_cost: 0,
    stamina_cost: 2,
    weapon_requirement: Some(WeaponType::Melee),

    roll: Some(AbilityRollType::Attack(0)),
    // TODO enhancement that adds range; the base range could be 2.5, which also means it wouldn't allow diagonal movement
    possible_enhancements: [Some(LUNGE_ATTACK_HEAVY_IMPACT), None, None],
    target: AbilityTarget::Enemy {
        //reach: AbilityReach::MoveIntoMelee(Range::Float(2.99)),
        reach: AbilityReach::MoveIntoMelee(Range::Float(2.5)),
        effect: AbilityNegativeEffect::Attack,
        impact_area: None,
    },
    animation_color: MAGENTA,
};

pub const BRACE: Ability = Ability {
    id: AbilityId::Brace,
    name: "Brace",
    description: Condition::Protected(1).description(),
    icon: IconId::Brace,
    action_point_cost: 1,
    mana_cost: 0,
    stamina_cost: 1,
    weapon_requirement: None,
    roll: None,
    possible_enhancements: [None; 3],
    target: AbilityTarget::None {
        self_area: None,
        self_effect: Some(AbilityPositiveEffect {
            healing: 0,
            apply: Some(ApplyEffect::Condition(Condition::Protected(2))),
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
    weapon_requirement: None,

    roll: Some(AbilityRollType::Spell),
    target: AbilityTarget::None {
        self_area: Some((
            Range::Float(2.5),
            AreaTargetAcquisition::Enemies,
            AbilityEffect::Negative(AbilityNegativeEffect::Spell(SpellNegativeEffect {
                defense_type: Some(DefenseType::Will),
                damage: None,
                on_hit: Some([Some(ApplyEffect::Condition(Condition::Dazed(1))), None]),
            })),
        )),
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
    mana_cost: 1,
    stamina_cost: 0,
    weapon_requirement: None,

    roll: Some(AbilityRollType::Spell),
    target: AbilityTarget::Enemy {
        reach: AbilityReach::Range(Range::Float(4.0)),
        effect: AbilityNegativeEffect::Spell(SpellNegativeEffect {
            defense_type: Some(DefenseType::Will),
            damage: None,
            on_hit: Some([
                Some(ApplyEffect::Condition(Condition::Slowed(2))),
                Some(ApplyEffect::Condition(Condition::Exposed(2))),
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
    weapon_requirement: None,

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

pub const NECROTIC_INFLUENCE_ENHANCEMENT: AbilityEnhancement = AbilityEnhancement {
    ability_id: AbilityId::NecroticInfluence,
    name: "Necrotic influence",
    description: "Converts all bleeding to immediate damage and life-drain",
    icon: IconId::NecroticInfluence,
    action_point_cost: 0,
    mana_cost: 1,
    stamina_cost: 0,
    spell_effect: Some(SpellEnhancementEffect {
        on_hit: Some([
            Some(ApplyEffect::PerBleeding {
                damage: 1,
                caster_healing_percentage: 40,
            }),
            Some(ApplyEffect::ConsumeCondition {
                condition: Condition::Bleeding(0),
            }),
        ]),
        ..SpellEnhancementEffect::default()
    }),
    attack_effect: None,
};
pub const NECROTIC_INFLUENCE: Ability = Ability {
    id: AbilityId::NecroticInfluence,
    name: "Inflict wounds",
    description: "",
    icon: IconId::NecroticInfluence,
    action_point_cost: 3,
    mana_cost: 1,
    stamina_cost: 0,
    weapon_requirement: None,

    roll: Some(AbilityRollType::Spell),
    possible_enhancements: [Some(NECROTIC_INFLUENCE_ENHANCEMENT), None, None],
    target: AbilityTarget::Enemy {
        effect: AbilityNegativeEffect::Spell(SpellNegativeEffect {
            defense_type: Some(DefenseType::Toughness),
            damage: None,
            on_hit: Some([Some(ApplyEffect::Condition(Condition::Bleeding(4))), None]),
        }),
        impact_area: None,
        reach: AbilityReach::Range(Range::Float(4.5)),
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
    weapon_requirement: None,

    roll: Some(AbilityRollType::Spell),
    possible_enhancements: [None, None, None],
    target: AbilityTarget::Enemy {
        effect: AbilityNegativeEffect::Spell(SpellNegativeEffect {
            defense_type: Some(DefenseType::Toughness),
            damage: None,
            on_hit: Some([Some(ApplyEffect::Condition(Condition::Bleeding(3))), None]),
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
    weapon_requirement: None,

    roll: Some(AbilityRollType::Spell),
    possible_enhancements: [None, None, None],
    target: AbilityTarget::Enemy {
        effect: AbilityNegativeEffect::Spell(SpellNegativeEffect {
            defense_type: Some(DefenseType::Will),
            damage: None,
            on_hit: Some([Some(ApplyEffect::Condition(Condition::Slowed(1))), None]),
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
    weapon_requirement: None,

    roll: Some(AbilityRollType::Spell),
    target: AbilityTarget::Ally {
        range: Range::Ranged(5),
        effect: AbilityPositiveEffect {
            healing: 3,
            apply: None,
        },
    },
    possible_enhancements: [None, None, None],
    animation_color: LIME,
};

pub const HEAL: Ability = Ability {
    id: AbilityId::Heal,
    name: "Heal",
    description: "Restore an ally's health",
    icon: IconId::Heal,
    action_point_cost: 3,
    mana_cost: 1,
    stamina_cost: 0,
    weapon_requirement: None,

    roll: Some(AbilityRollType::Spell),
    target: AbilityTarget::Ally {
        range: Range::Ranged(3),
        effect: AbilityPositiveEffect {
            healing: 2,
            apply: None,
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
        Some(AbilityEnhancement {
            ability_id: AbilityId::Heal,
            name: "Energize",
            description: "",
            icon: IconId::Energize,
            action_point_cost: 0,
            mana_cost: 1,
            stamina_cost: 0,
            attack_effect: None,
            spell_effect: Some(SpellEnhancementEffect {
                on_hit: Some([Some(ApplyEffect::GainStamina(2)), None]),
                ..SpellEnhancementEffect::default()
            }),
        }),
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
    weapon_requirement: None,

    roll: Some(AbilityRollType::Spell),
    possible_enhancements: [None, None, None],
    target: AbilityTarget::None {
        self_area: Some((
            Range::Ranged(4),
            AreaTargetAcquisition::Allies,
            AbilityEffect::Positive(AbilityPositiveEffect {
                healing: 1,
                apply: None,
            }),
        )),
        self_effect: None,
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
    weapon_requirement: None,

    roll: Some(AbilityRollType::Spell),
    possible_enhancements: [None, None, None],
    target: AbilityTarget::None {
        self_area: None,
        self_effect: Some(AbilityPositiveEffect {
            healing: 1,
            apply: Some(ApplyEffect::Condition(Condition::Protected(1))),
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
    weapon_requirement: None,

    roll: Some(AbilityRollType::Spell),
    possible_enhancements: [None, None, None],
    target: AbilityTarget::Area {
        range: Range::Ranged(5),
        radius: Range::Float(1.95),
        acquisition: AreaTargetAcquisition::Allies,
        effect: AbilityEffect::Positive(AbilityPositiveEffect {
            healing: 1,
            apply: None,
        }),
    },
    animation_color: GREEN,
};

pub const FIREBALL_REACH: AbilityEnhancement = AbilityEnhancement {
    ability_id: AbilityId::Fireballl,
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
    ability_id: AbilityId::Fireballl,
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
    ability_id: AbilityId::Fireballl,
    name: "Inferno",
    description: "More deadly impact",
    icon: IconId::Inferno,
    action_point_cost: 0,
    mana_cost: 1,
    stamina_cost: 0,
    attack_effect: None,
    spell_effect: Some(SpellEnhancementEffect {
        bonus_area_damage: 1,
        ..SpellEnhancementEffect::default()
    }),
};
pub const FIREBALL: Ability = Ability {
    id: AbilityId::Fireballl,
    name: "Fireball",
    description: "Hurl fire at an enemy, damaging them",
    icon: IconId::Fireball,
    action_point_cost: 3,
    mana_cost: 1,
    stamina_cost: 0,
    weapon_requirement: None,

    roll: Some(AbilityRollType::Spell),
    target: AbilityTarget::Enemy {
        reach: AbilityReach::Range(Range::Float(4.5)),
        effect: AbilityNegativeEffect::Spell(SpellNegativeEffect {
            defense_type: Some(DefenseType::Evasion),
            damage: Some(AbilityDamage::AtLeast(2)),
            on_hit: None,
        }),
        impact_area: Some((
            Range::Ranged(2),
            AreaTargetAcquisition::Everyone,
            AbilityNegativeEffect::Spell(SpellNegativeEffect {
                defense_type: Some(DefenseType::Toughness),
                damage: Some(AbilityDamage::AtLeast(1)),
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
    weapon_requirement: None,

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

pub const HEALTH_POTION: Consumable = Consumable {
    name: "Health potion",
    health_gain: 4,
    mana_gain: 0,
    icon: EquipmentIconId::HealthPotion,
    weight: 0,
};

pub const MANA_POTION: Consumable = Consumable {
    name: "Mana potion",
    health_gain: 0,
    mana_gain: 5,
    icon: EquipmentIconId::ManaPotion,
    weight: 0,
};

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PassiveSkill {
    HardenedSkin,
    WeaponProficiency,
    ArcaneSurge,
    Reaper,
    BloodRage,
}

impl PassiveSkill {
    pub fn name(&self) -> &'static str {
        match self {
            Self::HardenedSkin => "Hardened skin",
            Self::WeaponProficiency => "Weapon proficiency",
            Self::ArcaneSurge => "Arcane surge",
            Self::Reaper => "Reaper",
            Self::BloodRage => "Blood rage",
        }
    }

    pub fn icon(&self) -> IconId {
        match self {
            Self::HardenedSkin => IconId::HardenedSkin,
            Self::WeaponProficiency => IconId::WeaponProficiency,
            Self::ArcaneSurge => IconId::ArcaneSurge,
            Self::Reaper => IconId::Reaper,
            // TODO: unique icon
            Self::BloodRage => IconId::Rage,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::HardenedSkin => "+1 armor",
            Self::WeaponProficiency => "Attacks gain +1 armor penetration",
            Self::ArcaneSurge => "+3 spell modifier while at/below 50% mana",
            Self::Reaper => "On kill: gain 1 stamina, 1 AP (max 1 AP per turn)",
            Self::BloodRage => "+3 attack modifier while at/below 50% health. Immune to the negative effects of Near-death"
        }
    }
}
