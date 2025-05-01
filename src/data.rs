use macroquad::color::{BLACK, BLUE, GREEN, MAGENTA, PURPLE, RED};

use crate::{
    core::{
        ApplyEffect, ArmorPiece, AttackAttribute, AttackEnhancement, AttackEnhancementOnHitEffect,
        AttackHitEffect, Condition, DefenseType, OnAttackedReaction, OnAttackedReactionEffect,
        OnAttackedReactionId, OnHitReaction, OnHitReactionEffect, Range, Shield, Spell,
        SpellAllyEffect, SpellDamage, SpellEffect, SpellEnemyEffect, SpellEnhancement,
        SpellEnhancementEffect, SpellModifier, SpellReach, SpellTarget, Weapon, WeaponGrip,
        WeaponRange,
    },
    textures::{EquipmentIconId, IconId, SpriteId},
};

pub const LEATHER_ARMOR: ArmorPiece = ArmorPiece {
    name: "Leather armor",
    protection: 3,
    limit_evasion_from_agi: None,
    icon: EquipmentIconId::LeatherArmor,
    weight: 2,
};

pub const CHAIN_MAIL: ArmorPiece = ArmorPiece {
    name: "Chain mail",
    protection: 5,
    limit_evasion_from_agi: Some(4),
    icon: EquipmentIconId::ChainMail,
    weight: 3,
};

pub const SHIRT: ArmorPiece = ArmorPiece {
    name: "Shirt",
    protection: 1,
    limit_evasion_from_agi: None,
    icon: EquipmentIconId::Shirt,
    weight: 1,
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
        Condition::Bleeding(1),
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

const ALL_IN: AttackEnhancement = AttackEnhancement {
    name: "All-in",
    description: "Charge up the attack, dealing additional damage",
    icon: IconId::AllIn,
    action_point_cost: 1,
    stamina_cost: 0,
    mana_cost: 0,
    action_point_discount: 0,
    bonus_damage: 1,
    bonus_advantage: 0,
    on_hit_effect: None,
};

pub const WAR_HAMMER: Weapon = Weapon {
    name: "War hammer",
    range: WeaponRange::Melee,
    action_point_cost: 2,
    damage: 3,
    grip: WeaponGrip::TwoHanded,
    attack_attribute: AttackAttribute::Strength,
    attack_enhancement: Some(ALL_IN),
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
    description: "Strike more quickly",
    icon: IconId::QuickStrike,
    action_point_cost: 0,
    stamina_cost: 3,
    mana_cost: 0,
    action_point_discount: 1,
    bonus_damage: 0,
    bonus_advantage: 0,
    on_hit_effect: None,
};

pub const SMITE: AttackEnhancement = AttackEnhancement {
    name: "Smite",
    description: "Enhance your strike with magic",
    icon: IconId::Smite,
    action_point_cost: 0,
    stamina_cost: 1,
    mana_cost: 1,
    action_point_discount: 0,
    bonus_damage: 1,
    bonus_advantage: 0,
    on_hit_effect: None,
};

pub const OVERWHELMING: AttackEnhancement = AttackEnhancement {
    name: "Overwhelm",
    description: "Overwhelm the target",
    icon: IconId::CrushingStrike,
    action_point_cost: 0,
    stamina_cost: 2,
    mana_cost: 0,
    action_point_discount: 0,
    bonus_damage: 0,
    bonus_advantage: 0,
    on_hit_effect: Some(AttackEnhancementOnHitEffect::Target(
        ApplyEffect::RemoveActionPoints(1),
    )),
};

pub const CAREFULLY_AIMED: AttackEnhancement = AttackEnhancement {
    name: "Carefully aimed",
    description: "Spend more time on the attack, aiming carefully",
    icon: IconId::CarefulAim,
    action_point_cost: 1,
    stamina_cost: 0,
    mana_cost: 0,
    action_point_discount: 0,
    bonus_damage: 0,
    bonus_advantage: 1,
    on_hit_effect: None,
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
    name: "Sweeping attack",
    description: "Attack all enemies around you",
    icon: IconId::SweepAttack,
    action_point_cost: 3,
    mana_cost: 0,
    stamina_cost: 1,

    modifier: SpellModifier::Attack(-3),
    possible_enhancements: [
        Some(SpellEnhancement {
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
    name: "Lunge attack",
    description: "Lunge at an enemy in an unobstructed path, and attack",
    icon: IconId::LungeAttack,
    action_point_cost: 2,
    mana_cost: 0,
    stamina_cost: 2,

    modifier: SpellModifier::Attack(0),
    // TODO enhancement that adds range; the base range could be 2.5, which also means it wouldn't allow diagonal movement
    possible_enhancements: [
        Some(SpellEnhancement {
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
    name: "Brace",
    description: Condition::Braced.description(),
    icon: IconId::Brace,
    action_point_cost: 1,
    mana_cost: 0,
    stamina_cost: 1,

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
    name: "Scream",
    description: "Daze nearby enemies",
    icon: IconId::Scream,
    action_point_cost: 2,
    mana_cost: 1,
    stamina_cost: 0,

    modifier: SpellModifier::Spell,
    possible_enhancements: [
        Some(SpellEnhancement {
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
    animation_color: BLUE,
};

pub const SHACKLED_MIND: Spell = Spell {
    name: "Shackled Mind",
    description: "Shackle an enemy's mind, slowing them and lowering their defenses",
    icon: IconId::ShackledMind,
    action_point_cost: 3,
    mana_cost: 1,
    stamina_cost: 0,

    modifier: SpellModifier::Spell,
    target: SpellTarget::Enemy {
        reach: SpellReach::Range(Range::Float(4.0)),
        effect: SpellEnemyEffect {
            defense_type: Some(DefenseType::Will),
            damage: None,
            on_hit: Some([
                Some(ApplyEffect::Condition(Condition::Slowed(2))),
                Some(ApplyEffect::Condition(Condition::Exposed(1))),
            ]),
        },
        impact_area: None,
    },
    possible_enhancements: [
        Some(SpellEnhancement {
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
        Some(SpellEnhancement {
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
    name: "Mind blast",
    description: "Assault an enemy's mind, damaging and disrupting them",
    icon: IconId::Mindblast,
    action_point_cost: 2,
    mana_cost: 1,
    stamina_cost: 0,

    modifier: SpellModifier::Spell,
    possible_enhancements: [
        Some(SpellEnhancement {
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

pub const HEAL: Spell = Spell {
    name: "Heal",
    description: "Restore an ally's health",
    icon: IconId::Heal,
    action_point_cost: 3,
    mana_cost: 1,
    stamina_cost: 0,

    modifier: SpellModifier::Spell,
    possible_enhancements: [
        Some(SpellEnhancement {
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
    target: SpellTarget::Ally {
        range: Range::Ranged(3),
        effect: SpellAllyEffect {
            healing: 3,
            apply: None,
        },
    },
    animation_color: GREEN,
};

pub const HEALING_NOVA: Spell = Spell {
    name: "Healing nova",
    description: "Restore health to nearby allies",
    icon: IconId::PlusPlus,
    action_point_cost: 2,
    mana_cost: 1,
    stamina_cost: 0,

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
    name: "Self heal",
    description: "Restore the caster's health and grants protection",
    icon: IconId::PlusPlus,
    action_point_cost: 2,
    mana_cost: 1,
    stamina_cost: 0,

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
    name: "Healing rain",
    description: "Restore health to allies in an area",
    icon: IconId::PlusPlus,
    action_point_cost: 2,
    mana_cost: 2,
    stamina_cost: 0,

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

pub const FIREBALL: Spell = Spell {
    name: "Fireball",
    description: "Hurl fire at an enemy, damaging them",
    icon: IconId::Fireball,
    action_point_cost: 3,
    mana_cost: 1,
    stamina_cost: 0,

    modifier: SpellModifier::Spell,
    target: SpellTarget::Enemy {
        reach: SpellReach::Range(Range::Float(4.0)),
        effect: SpellEnemyEffect {
            defense_type: Some(DefenseType::Evasion),
            damage: Some(SpellDamage::AtLeast(2)),
            on_hit: None,
        },
        impact_area: Some((
            Range::Melee,
            SpellEnemyEffect {
                defense_type: None,
                damage: Some(SpellDamage::Static(1)),
                on_hit: None,
            },
        )),
    },
    possible_enhancements: [
        Some(SpellEnhancement {
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
        Some(SpellEnhancement {
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
        }),
        Some(SpellEnhancement {
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
        }),
    ],

    animation_color: RED,
};

pub const KILL: Spell = Spell {
    name: "Kill",
    description: "Kill an enemy",
    icon: IconId::Fireball,
    action_point_cost: 4,
    mana_cost: 0,
    stamina_cost: 0,

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
