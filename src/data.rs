use macroquad::color::{BLACK, BLUE, GREEN, PURPLE, RED};

use crate::{
    core::{
        ApplyEffect, ArmorPiece, AttackAttribute, AttackEnhancement, AttackEnhancementOnHitEffect,
        AttackHitEffect, Condition, OnAttackedReaction,
        OnAttackedReactionEffect, OnHitReaction, OnHitReactionEffect, Range, SelfEffectAction,
        Shield, Spell, SpellAllyEffect, SpellContestType, SpellEffect, SpellEnemyEffect,
        SpellEnhancement, SpellEnhancementEffect, SpellTargetType, Weapon, WeaponGrip, WeaponRange,
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
        description: "Possibly daze attacker (str vs [toughness])",
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
    description: Condition::Raging.description(),
    icon: IconId::Rage,
    action_point_cost: 1,
    effect: OnHitReactionEffect::Rage,
    must_be_melee: false,
};

pub const BRACED_DEFENSE_BONUS: u32 = 3;
pub const BRACE: SelfEffectAction = SelfEffectAction {
    name: "Brace",
    description: Condition::Braced.description(),
    icon: IconId::Brace,
    action_point_cost: 1,
    stamina_cost: 1,
    effect: ApplyEffect::Condition(Condition::Braced),
};

pub const SCREAM: Spell = Spell {
    name: "Scream",
    description: "Daze nearby enemies",
    icon: IconId::Scream,
    action_point_cost: 2,
    mana_cost: 1,
    possible_enhancements: [
        // TODO Let this increase the range of the spell
        Some(SpellEnhancement {
            name: "Shriek",
            description: "Targets also lose 1 AP",
            icon: IconId::Banshee,
            action_point_cost: 0,
            mana_cost: 1,
            bonus_damage: 0,
            effect: Some(SpellEnhancementEffect::OnHitEffect(
                ApplyEffect::RemoveActionPoints(1),
            )),
        }),
        None,
    ],

    target_type: SpellTargetType::NoTarget {
        self_area: Some((
            Range::Ranged(3),
            SpellEffect::Enemy(SpellEnemyEffect {
                contest_type: Some(SpellContestType::Mental),
                damage: None,
                on_hit_effect: Some(ApplyEffect::Condition(Condition::Dazed(1))),
            }),
        )),
        self_effect: None,
    },
    animation_color: BLUE,
};

pub const MIND_BLAST: Spell = Spell {
    name: "Mind blast",
    description: "Assault an enemy's mind, damaging and disrupting them",
    icon: IconId::Mindblast,
    action_point_cost: 2,
    mana_cost: 1,
    possible_enhancements: [
        Some(SpellEnhancement {
            name: "Dualcast",
            description: "Spell is cast twice",
            icon: IconId::Dualcast,
            action_point_cost: 1,
            mana_cost: 1,
            bonus_damage: 0,
            effect: Some(SpellEnhancementEffect::CastTwice),
        }),
        None,
    ],
    target_type: SpellTargetType::TargetEnemy {
        effect: SpellEnemyEffect {
            contest_type: Some(SpellContestType::Mental),
            damage: Some((1, false)),
            on_hit_effect: Some(ApplyEffect::RemoveActionPoints(1)),
        },
        impact_area: None,
        range: Range::Ranged(5),
    },
    animation_color: PURPLE,
};

pub const HEAL: Spell = Spell {
    name: "Heal",
    description: "Restore an ally's health",
    icon: IconId::Plus,
    action_point_cost: 2,
    mana_cost: 1,
    possible_enhancements: [None, None],
    target_type: SpellTargetType::TargetAlly {
        range: Range::Ranged(5),
        effect: SpellAllyEffect {
            healing: 1,
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
    possible_enhancements: [None, None],
    target_type: SpellTargetType::NoTarget {
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
    possible_enhancements: [None, None],
    target_type: SpellTargetType::NoTarget {
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
    possible_enhancements: [None, None],
    target_type: SpellTargetType::TargetArea {
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
    possible_enhancements: [
        Some(SpellEnhancement {
            name: "Big",
            description: "+1 damage",
            icon: IconId::Plus,
            action_point_cost: 0,
            mana_cost: 1,
            bonus_damage: 1,
            effect: None,
        }),
        Some(SpellEnhancement {
            name: "Massive",
            description: "+2 damage",
            icon: IconId::PlusPlus,
            action_point_cost: 0,
            mana_cost: 1,
            bonus_damage: 2,
            effect: None,
        }),
    ],
    target_type: SpellTargetType::TargetEnemy {
        effect: SpellEnemyEffect {
            contest_type: Some(SpellContestType::Projectile),
            damage: Some((2, true)),
            on_hit_effect: None,
        },
        impact_area: Some((
            Range::Melee,
            SpellEnemyEffect {
                contest_type: None,
                damage: Some((1, false)),
                on_hit_effect: None,
            },
        )),
        range: Range::Ranged(5),
    },
    animation_color: RED,
};

pub const KILL: Spell = Spell {
    name: "Kill",
    description: "Kill an enemy",
    icon: IconId::Fireball,
    action_point_cost: 5,
    mana_cost: 0,
    possible_enhancements: [None; 2],
    target_type: SpellTargetType::TargetEnemy {
        effect: SpellEnemyEffect {
            contest_type: None,
            damage: Some((99, false)),
            on_hit_effect: None,
        },
        impact_area: None,
        range: Range::Ranged(10),
    },
    animation_color: BLACK,
};
