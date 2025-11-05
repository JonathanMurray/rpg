use macroquad::color::{BLACK, BLUE, BROWN, GREEN, LIME, MAGENTA, PURPLE, RED};

use crate::{
    core::{
        ApplyEffect, ArmorPiece, AttackAttribute, AttackEnhancement, AttackEnhancementEffect,
        AttackEnhancementOnHitEffect, Condition, Consumable, DefenseType, EquipEffect,
        OnAttackedReaction, OnAttackedReactionEffect, OnAttackedReactionId, OnHitReaction,
        OnHitReactionEffect, Range, Shield, Spell, SpellAllyEffect, SpellDamage, SpellEffect,
        SpellEnemyEffect, SpellEnhancement, SpellEnhancementEffect, SpellId, SpellModifier,
        SpellReach, SpellTarget, Weapon, WeaponGrip, WeaponRange, WeaponType,
    },
    textures::{EquipmentIconId, IconId, SpriteId},
};

pub const LEATHER_ARMOR: ArmorPiece = ArmorPiece {
    name: "Leather armor",
    protection: 3,
    limit_evasion_from_agi: None,
    icon: EquipmentIconId::LeatherArmor,
    weight: 2,
    equip: EquipEffect::default(),
};

pub const CHAIN_MAIL: ArmorPiece = ArmorPiece {
    name: "Chain mail",
    protection: 5,
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
    damage: 1,
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
    damage: 1,
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
    action_point_cost: 2,
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
    damage: 1,
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
    action_point_cost: 2,
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
    damage: 3,
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
    action_point_cost: 2,
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

pub const BAD_BOW: Weapon = Weapon {
    name: "Bad bow",
    range: WeaponRange::Ranged(5),
    action_point_cost: 3,
    damage: 1,
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
    action_point_cost: 2,
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

pub const BAD_SMALL_SHIELD: Shield = Shield {
    name: "Bad small shield",
    sprite: Some(SpriteId::Shield),
    icon: EquipmentIconId::SmallShield,
    evasion: 1,
    on_hit_reaction: None,
    weight: 2,
};

pub const SMALL_SHIELD: Shield = Shield {
    name: "Small shield",
    sprite: Some(SpriteId::Shield),
    icon: EquipmentIconId::SmallShield,
    evasion: 3,
    on_hit_reaction: Some(OnHitReaction {
        name: "Shield bash",
        description: "Possibly daze attacker (str vs [toughness])",
        icon: IconId::ShieldBash,
        action_point_cost: 1,
        effect: OnHitReactionEffect::ShieldBash,
        must_be_melee: true,
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
    stamina_cost: 1,
    mana_cost: 1,
    effect: AttackEnhancementEffect {
        bonus_damage: 1,
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
        on_hit_effect: Some(AttackEnhancementOnHitEffect::Target(
            ApplyEffect::RemoveActionPoints(1),
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
        bonus_advantage: 1,
        ..AttackEnhancementEffect::default()
    },
    ..AttackEnhancement::default()
};

pub const CRIPPLING_SHOT: AttackEnhancement = AttackEnhancement {
    name: "Crippling shot",
    icon: IconId::CripplingShot,
    stamina_cost: 1,
    effect: AttackEnhancementEffect {
        on_hit_effect: Some(AttackEnhancementOnHitEffect::Target(
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
    description: "Gain bonus evasion against one melee attack",
    icon: IconId::Parry,
    action_point_cost: 1,
    stamina_cost: 0,
    effect: OnAttackedReactionEffect { bonus_evasion: 4 },
    must_be_melee: true,
};

pub const SIDE_STEP: OnAttackedReaction = OnAttackedReaction {
    id: OnAttackedReactionId::SideStep,
    name: "Side step",
    description: "Gain bonus evasion against one attack",
    icon: IconId::Sidestep,
    action_point_cost: 1,
    stamina_cost: 1,
    effect: OnAttackedReactionEffect { bonus_evasion: 4 },
    must_be_melee: false,
};

pub const RAGE: OnHitReaction = OnHitReaction {
    name: "Rage",
    description: Condition::Raging.description(),
    icon: IconId::Rage,
    action_point_cost: 1,
    effect: OnHitReactionEffect::Rage,
    must_be_melee: false,
};

pub const SWEEP_ATTACK: Spell = Spell {
    id: SpellId::SweepAttack,
    name: "Sweeping attack",
    description: "Target all enemies around you",
    icon: IconId::SweepAttack,
    action_point_cost: 3,
    mana_cost: 0,
    stamina_cost: 1,
    weapon_requirement: Some(WeaponType::Melee),

    modifier: SpellModifier::Attack(-3),
    possible_enhancements: [
        Some(SpellEnhancement {
            spell_id: SpellId::SweepAttack,
            name: "Precise",
            description: "Increase your precision",
            icon: IconId::Precision,
            action_point_cost: 0,
            mana_cost: 0,
            stamina_cost: 2,
            effect: SpellEnhancementEffect {
                roll_bonus: 3,
                ..SpellEnhancementEffect::default()
            },
        }),
        None,
        None,
    ],
    target: SpellTarget::None {
        self_area: Some((
            Range::Melee,
            SpellEffect::Enemy(SpellEnemyEffect {
                defense_type: Some(DefenseType::Evasion),
                damage: Some(SpellDamage::Weapon),
                on_hit: None,
            }),
        )),
        self_effect: None,
    },
    animation_color: MAGENTA,
};

pub const LUNGE_ATTACK: Spell = Spell {
    id: SpellId::LungeAttack,
    name: "Lunge attack",
    description: "Move to target in an unobstructed path, before attacking",
    icon: IconId::LungeAttack,
    action_point_cost: 2,
    mana_cost: 0,
    stamina_cost: 2,
    weapon_requirement: Some(WeaponType::Melee),

    modifier: SpellModifier::Attack(0),
    // TODO enhancement that adds range; the base range could be 2.5, which also means it wouldn't allow diagonal movement
    possible_enhancements: [
        Some(SpellEnhancement {
            spell_id: SpellId::LungeAttack,
            name: "Heavy impact",
            description: "Apply more force on impact",
            icon: IconId::CrushingStrike,
            action_point_cost: 0,
            mana_cost: 0,
            stamina_cost: 2,
            effect: SpellEnhancementEffect {
                on_hit: Some(ApplyEffect::RemoveActionPoints(1)),
                ..SpellEnhancementEffect::default()
            },
        }),
        None,
        None,
    ],
    target: SpellTarget::Enemy {
        reach: SpellReach::MoveIntoMelee(Range::Float(2.99)),
        effect: SpellEnemyEffect {
            defense_type: Some(DefenseType::Evasion),
            damage: Some(SpellDamage::Weapon),
            on_hit: None,
        },
        impact_area: None,
    },
    animation_color: MAGENTA,
};

pub const BRACE: Spell = Spell {
    id: SpellId::Brace,
    name: "Brace",
    description: Condition::Braced.description(),
    icon: IconId::Brace,
    action_point_cost: 1,
    mana_cost: 0,
    stamina_cost: 1,
    weapon_requirement: None,

    modifier: SpellModifier::Spell,
    possible_enhancements: [None; 3],
    target: SpellTarget::None {
        self_area: None,
        self_effect: Some(SpellAllyEffect {
            healing: 0,
            apply: Some(ApplyEffect::Condition(Condition::Braced)),
        }),
    },
    animation_color: MAGENTA,
};

pub const SCREAM: Spell = Spell {
    id: SpellId::Scream,
    name: "Scream",
    description: "Daze nearby enemies",
    icon: IconId::Scream,
    action_point_cost: 2,
    mana_cost: 1,
    stamina_cost: 0,
    weapon_requirement: None,

    modifier: SpellModifier::Spell,
    target: SpellTarget::None {
        self_area: Some((
            Range::Ranged(3),
            SpellEffect::Enemy(SpellEnemyEffect {
                defense_type: Some(DefenseType::Will),
                damage: None,
                on_hit: Some([Some(ApplyEffect::Condition(Condition::Dazed(1))), None]),
            }),
        )),
        self_effect: None,
    },
    possible_enhancements: [
        Some(SpellEnhancement {
            spell_id: SpellId::Scream,
            name: "Shriek",
            description: "Increased range",
            icon: IconId::Banshee,
            action_point_cost: 0,
            mana_cost: 1,
            stamina_cost: 0,
            effect: SpellEnhancementEffect {
                increased_range_tenths: 15,
                ..SpellEnhancementEffect::default()
            },
        }),
        None,
        None,
    ],

    animation_color: BLUE,
};

pub const SHACKLED_MIND: Spell = Spell {
    id: SpellId::ShackledMind,
    name: "Shackle",
    description: "Shackle an enemy's mind, slowing them and lowering their defenses",
    icon: IconId::ShackledMind,
    action_point_cost: 3,
    mana_cost: 1,
    stamina_cost: 0,
    weapon_requirement: None,

    modifier: SpellModifier::Spell,
    target: SpellTarget::Enemy {
        reach: SpellReach::Range(Range::Float(4.0)),
        effect: SpellEnemyEffect {
            defense_type: Some(DefenseType::Will),
            damage: None,
            on_hit: Some([
                Some(ApplyEffect::Condition(Condition::Slowed(2))),
                Some(ApplyEffect::Condition(Condition::Exposed(2))),
            ]),
        },
        impact_area: None,
    },
    possible_enhancements: [
        Some(SpellEnhancement {
            spell_id: SpellId::ShackledMind,
            name: "Reach",
            description: "",
            icon: IconId::Extend,
            action_point_cost: 0,
            mana_cost: 1,
            stamina_cost: 0,
            effect: SpellEnhancementEffect {
                increased_range_tenths: 30,
                ..SpellEnhancementEffect::default()
            },
        }),
        Some(SpellEnhancement {
            spell_id: SpellId::ShackledMind,
            name: "Focus",
            description: "",
            icon: IconId::SpellAdvantage,
            action_point_cost: 0,
            mana_cost: 1,
            stamina_cost: 0,
            effect: SpellEnhancementEffect {
                bonus_advantage: 1,
                ..SpellEnhancementEffect::default()
            },
        }),
        None,
    ],

    animation_color: PURPLE,
};

pub const MIND_BLAST: Spell = Spell {
    id: SpellId::MindBlast,
    name: "Mind blast",
    description: "Assault an enemy's mind, damaging and disrupting them",
    icon: IconId::Mindblast,
    action_point_cost: 2,
    mana_cost: 1,
    stamina_cost: 0,
    weapon_requirement: None,

    modifier: SpellModifier::Spell,
    possible_enhancements: [
        Some(SpellEnhancement {
            spell_id: SpellId::MindBlast,
            name: "Dualcast",
            description: "Spell is cast twice",
            icon: IconId::Dualcast,
            action_point_cost: 1,
            mana_cost: 1,
            stamina_cost: 0,
            effect: SpellEnhancementEffect {
                cast_twice: true,
                ..SpellEnhancementEffect::default()
            },
        }),
        None,
        None,
    ],
    target: SpellTarget::Enemy {
        effect: SpellEnemyEffect {
            defense_type: Some(DefenseType::Will),
            damage: Some(SpellDamage::Static(1)),
            on_hit: Some([Some(ApplyEffect::RemoveActionPoints(1)), None]),
        },
        impact_area: None,
        reach: SpellReach::Range(Range::Ranged(5)),
    },
    animation_color: PURPLE,
};

pub const MAGI_INFLICT_WOUNDS: Spell = Spell {
    id: SpellId::MagiInflictWounds,
    name: "Inflict wounds",
    description: "",
    icon: IconId::Mindblast,
    action_point_cost: 3,
    mana_cost: 0,
    stamina_cost: 0,
    weapon_requirement: None,

    modifier: SpellModifier::Spell,
    possible_enhancements: [None, None, None],
    target: SpellTarget::Enemy {
        effect: SpellEnemyEffect {
            defense_type: Some(DefenseType::Toughness),
            damage: None,
            on_hit: Some([Some(ApplyEffect::Condition(Condition::Bleeding(3))), None]),
        },
        impact_area: None,
        reach: SpellReach::Range(Range::Ranged(5)),
    },
    animation_color: BROWN,
};

pub const MAGI_INFLICT_HORRORS: Spell = Spell {
    id: SpellId::MagiInflictHorrors,
    name: "Inflict horrors",
    description: "",
    icon: IconId::Mindblast,
    action_point_cost: 3,
    mana_cost: 0,
    stamina_cost: 0,
    weapon_requirement: None,

    modifier: SpellModifier::Spell,
    possible_enhancements: [None, None, None],
    target: SpellTarget::Enemy {
        effect: SpellEnemyEffect {
            defense_type: Some(DefenseType::Will),
            damage: None,
            on_hit: Some([Some(ApplyEffect::Condition(Condition::Slowed(1))), None]),
        },
        impact_area: None,
        reach: SpellReach::Range(Range::Ranged(5)),
    },
    animation_color: PURPLE,
};

pub const MAGI_HEAL: Spell = Spell {
    id: SpellId::MagiHeal,
    name: "Heal",
    description: "",
    icon: IconId::Heal,
    action_point_cost: 3,
    mana_cost: 0,
    stamina_cost: 0,
    weapon_requirement: None,

    modifier: SpellModifier::Spell,
    target: SpellTarget::Ally {
        range: Range::Ranged(5),
        effect: SpellAllyEffect {
            healing: 4,
            apply: None,
        },
    },
    possible_enhancements: [None, None, None],
    animation_color: LIME,
};

pub const HEAL: Spell = Spell {
    id: SpellId::Heal,
    name: "Heal",
    description: "Restore an ally's health",
    icon: IconId::Heal,
    action_point_cost: 3,
    mana_cost: 1,
    stamina_cost: 0,
    weapon_requirement: None,

    modifier: SpellModifier::Spell,
    target: SpellTarget::Ally {
        range: Range::Ranged(3),
        effect: SpellAllyEffect {
            healing: 3,
            apply: None,
        },
    },
    possible_enhancements: [
        Some(SpellEnhancement {
            spell_id: SpellId::Heal,
            name: "Reach",
            description: "",
            icon: IconId::Extend,
            action_point_cost: 0,
            mana_cost: 1,
            stamina_cost: 0,
            effect: SpellEnhancementEffect {
                increased_range_tenths: 20,
                ..SpellEnhancementEffect::default()
            },
        }),
        // TODO add enhancement that heals over time (1 per round for 3 turns?)
        Some(SpellEnhancement {
            spell_id: SpellId::Heal,
            name: "Energize",
            description: "",
            icon: IconId::Energize,
            action_point_cost: 0,
            mana_cost: 1,
            stamina_cost: 0,
            effect: SpellEnhancementEffect {
                on_hit: Some(ApplyEffect::GainStamina(2)),
                ..SpellEnhancementEffect::default()
            },
        }),
        None,
    ],

    animation_color: GREEN,
};

pub const HEALING_NOVA: Spell = Spell {
    id: SpellId::HealingNova,
    name: "Healing nova",
    description: "Restore health to nearby allies",
    icon: IconId::PlusPlus,
    action_point_cost: 3,
    mana_cost: 1,
    stamina_cost: 0,
    weapon_requirement: None,

    modifier: SpellModifier::Spell,
    possible_enhancements: [None, None, None],
    target: SpellTarget::None {
        self_area: Some((
            Range::Ranged(4),
            SpellEffect::Ally(SpellAllyEffect {
                healing: 1,
                apply: None,
            }),
        )),
        self_effect: None,
    },
    animation_color: GREEN,
};

pub const SELF_HEAL: Spell = Spell {
    id: SpellId::SelfHeal,
    name: "Self heal",
    description: "Restore the caster's health and grants protection",
    icon: IconId::PlusPlus,
    action_point_cost: 2,
    mana_cost: 1,
    stamina_cost: 0,
    weapon_requirement: None,

    modifier: SpellModifier::Spell,
    possible_enhancements: [None, None, None],
    target: SpellTarget::None {
        self_area: None,
        self_effect: Some(SpellAllyEffect {
            healing: 1,
            apply: Some(ApplyEffect::Condition(Condition::Protected(1))),
        }),
    },
    animation_color: GREEN,
};

pub const HEALING_RAIN: Spell = Spell {
    id: SpellId::HealingRain,
    name: "Healing rain",
    description: "Restore health to allies in an area",
    icon: IconId::PlusPlus,
    action_point_cost: 3,
    mana_cost: 2,
    stamina_cost: 0,
    weapon_requirement: None,

    modifier: SpellModifier::Spell,
    possible_enhancements: [None, None, None],
    target: SpellTarget::Area {
        range: Range::Ranged(5),
        radius: Range::Float(1.95),
        effect: SpellEffect::Ally(SpellAllyEffect {
            healing: 1,
            apply: None,
        }),
    },
    animation_color: GREEN,
};

pub const FIREBALL_REACH: SpellEnhancement = SpellEnhancement {
    spell_id: SpellId::Fireballl,
    name: "Reach",
    description: "",
    icon: IconId::Extend,
    action_point_cost: 0,
    mana_cost: 1,
    stamina_cost: 0,
    effect: SpellEnhancementEffect {
        increased_range_tenths: 30,
        ..SpellEnhancementEffect::default()
    },
};

pub const FIREBALL_MASSIVE: SpellEnhancement = SpellEnhancement {
    spell_id: SpellId::Fireballl,
    name: "Massive",
    description: "",
    icon: IconId::Radius,
    action_point_cost: 0,
    mana_cost: 1,
    stamina_cost: 0,
    effect: SpellEnhancementEffect {
        increased_radius_tenths: 10,
        ..SpellEnhancementEffect::default()
    },
};
pub const FIREBALL_INFERNO: SpellEnhancement = SpellEnhancement {
    spell_id: SpellId::Fireballl,
    name: "Inferno",
    description: "More deadly impact",
    icon: IconId::Inferno,
    action_point_cost: 0,
    mana_cost: 1,
    stamina_cost: 0,
    effect: SpellEnhancementEffect {
        bonus_area_damage: 1,
        ..SpellEnhancementEffect::default()
    },
};
pub const FIREBALL: Spell = Spell {
    id: SpellId::Fireballl,
    name: "Fireball",
    description: "Hurl fire at an enemy, damaging them",
    icon: IconId::Fireball,
    action_point_cost: 3,
    mana_cost: 1,
    stamina_cost: 0,
    weapon_requirement: None,

    modifier: SpellModifier::Spell,
    target: SpellTarget::Enemy {
        reach: SpellReach::Range(Range::Float(4.0)),
        effect: SpellEnemyEffect {
            defense_type: Some(DefenseType::Evasion),
            damage: Some(SpellDamage::AtLeast(2)),
            on_hit: None,
        },
        impact_area: Some((
            Range::Ranged(2),
            SpellEnemyEffect {
                defense_type: None,
                damage: Some(SpellDamage::Static(2)),
                on_hit: None,
            },
        )),
    },
    possible_enhancements: [
        Some(FIREBALL_REACH),
        Some(FIREBALL_MASSIVE),
        Some(FIREBALL_INFERNO),
    ],

    animation_color: RED,
};

pub const KILL: Spell = Spell {
    id: SpellId::Kill,
    name: "Kill",
    description: "Kill an enemy",
    icon: IconId::Fireball,
    action_point_cost: 4,
    mana_cost: 0,
    stamina_cost: 0,
    weapon_requirement: None,

    modifier: SpellModifier::Spell,
    possible_enhancements: [None; 3],
    target: SpellTarget::Enemy {
        effect: SpellEnemyEffect {
            defense_type: None,
            damage: Some(SpellDamage::Static(99)),
            on_hit: None,
        },
        impact_area: None,
        reach: SpellReach::Range(Range::Ranged(10)),
    },
    animation_color: BLACK,
};

pub const HEALTH_POTION: Consumable = Consumable {
    name: "Health potion",
    health_gain: 4,
    mana_gain: 0,
    icon: EquipmentIconId::HealthPotion,
    weight: 1,
};

pub const MANA_POTION: Consumable = Consumable {
    name: "Mana potion",
    health_gain: 0,
    mana_gain: 5,
    icon: EquipmentIconId::ManaPotion,
    weight: 1,
};
