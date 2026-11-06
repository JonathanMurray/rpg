use std::{
    borrow::Cow,
    cell::OnceCell,
    sync::{LazyLock, OnceLock},
};

use macroquad::color::{
    Color, BLACK, BLUE, BROWN, DARKGRAY, GRAY, GREEN, LIME, PURPLE, RED, SKYBLUE, WHITE, YELLOW,
};

use crate::{
    core::{
        Ability, AbilityAnimation, AbilityAttackEffect, AbilityChargeFx, AbilityDamage,
        AbilityEnhancement, AbilityId, AbilityReach, AbilityRollType, AbilityTarget,
        AbilityTargetEffect, AbilityVisualReaction, ApplyCondition, ApplyEffect, AreaEffect,
        AreaShape, ArmorPiece, Arrow, AttackAttribute, AttackEnhancement, AttackEnhancementEffect,
        AttackEnhancementOnHitEffect, AttackHitEffect, AttackType, AudioVisuals, CircleEffect,
        Circumstance, Condition, Consumable, DamageSource, DamageType, DefenseType,
        EnvironmentEffect, EquipEffect, EquipmentRequirement, Fraction, HandType,
        OnAttackedReaction, OnAttackedReactionEffect, OnAttackedReactionId,
        OnAttackedReactionTarget, OnHitReaction, OnHitReactionEffect, Range, Shield,
        SpellEnhancementEffect, SpellTargetEffect, SpellTravelAnimation, TargetAcquisition,
        Trinket, Weapon, WeaponGrip, WeaponRange, WeaponType,
    },
    grid::ParticleShape,
    pathfind::Surface,
    resources::{
        BeamFxId, CharacterSpriteId, EffectId, EquipmentIconId, EquipmentSpriteId, IconId,
    },
    sounds::SoundId,
    tooltip::Keyword,
    util::{COL_BLUE, COL_GREEN_1, COL_GREEN_3, COL_RED},
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

pub const DRUID_COAT: ArmorPiece = ArmorPiece {
    name: "Druid coat",
    protection: 1,
    limit_evasion_from_agi: None,
    icon: EquipmentIconId::DruidCoat,
    weight: 1,
    equip: EquipEffect {
        thorns: 1,
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

pub const GOOD_CHAIN_MAIL: ArmorPiece = ArmorPiece {
    name: "Good chain mail",
    protection: 4,
    limit_evasion_from_agi: None,
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
    name: "Dagger",
    range: WeaponRange::Melee,
    action_point_cost: 3,
    damage: 3,
    grip: WeaponGrip::Light,
    attack_attribute: AttackAttribute::Finesse,
    attack_enhancement: None,
    on_attacked_reaction: None,
    bonus_melee_evasion: 0,
    on_damage: None,
    on_kill_apply_self: None,
    sprite: Some(EquipmentSpriteId::Dagger),
    icon: EquipmentIconId::Dagger,
    weight: 1,
};

pub const DAGGER: Weapon = Weapon {
    name: "Dagger",
    range: WeaponRange::Melee,
    action_point_cost: 3,
    damage: 5,
    grip: WeaponGrip::Light,
    attack_attribute: AttackAttribute::Finesse,
    attack_enhancement: None, //Some(STABBING),
    on_attacked_reaction: None,
    bonus_melee_evasion: 0,
    on_damage: None,
    on_kill_apply_self: None,
    sprite: Some(EquipmentSpriteId::Dagger),
    icon: EquipmentIconId::Dagger,
    weight: 1,
};

pub const SLASHING: LazyLock<AttackEnhancement> = LazyLock::new(|| AttackEnhancement {
    name: "Slashing",
    description: format!(
        "Inflict |<value>4| {}.",
        Condition::Bleeding.name_with_icon()
    )
    .leak(),
    icon: IconId::Slashing,
    stamina_cost: 2,
    effect: AttackEnhancementEffect {
        on_damage_effect: Some(AttackEnhancementOnHitEffect::Target(
            None,
            apply_condition_stacks(Condition::Bleeding, 4),
        )),
        //inflict_x_condition_per_damage: Some((Fraction::new(1, 2), Condition::Bleeding)),
        ..AttackEnhancementEffect::default()
    },
    ..AttackEnhancement::default()
});

pub const ENEMY_SLASHING: AttackEnhancement = AttackEnhancement {
    name: "Slashing",
    icon: IconId::Slashing,
    effect: AttackEnhancementEffect {
        on_damage_effect: Some(AttackEnhancementOnHitEffect::Target(
            None,
            apply_condition_stacks(Condition::Bleeding, 2),
        )),
        roll_modifier: -5,
        //inflict_x_condition_per_damage: Some((Fraction::new(1, 2), Condition::Bleeding)),
        ..AttackEnhancementEffect::default()
    },
    ..AttackEnhancement::default()
};

pub const BAD_SWORD: LazyLock<Weapon> = LazyLock::new(|| Weapon {
    name: "Sword",
    range: WeaponRange::Melee,
    action_point_cost: 3,
    damage: 4,
    grip: WeaponGrip::Versatile,
    attack_attribute: AttackAttribute::Finesse,
    attack_enhancement: Some(*SLASHING),
    on_attacked_reaction: None,
    bonus_melee_evasion: 3,
    on_damage: None,
    on_kill_apply_self: None,
    sprite: Some(EquipmentSpriteId::Sword),
    icon: EquipmentIconId::Sword,
    weight: 2,
});

pub const ENSLAVED_SWORD: LazyLock<Weapon> = LazyLock::new(|| Weapon {
    name: "Enslaved Sword",
    range: WeaponRange::Melee,
    action_point_cost: 4,
    damage: 4,
    grip: WeaponGrip::Versatile,
    attack_attribute: AttackAttribute::Finesse,
    attack_enhancement: Some(*SLASHING),
    on_attacked_reaction: None,
    bonus_melee_evasion: 3,
    on_damage: None,
    on_kill_apply_self: None,
    sprite: Some(EquipmentSpriteId::Sword),
    icon: EquipmentIconId::Sword,
    weight: 2,
});

pub const ENSLAVED_RAPIER: LazyLock<Weapon> = LazyLock::new(|| Weapon {
    name: "Enslaved Rapier",
    range: WeaponRange::Melee,
    action_point_cost: 4,
    damage: 4,
    grip: WeaponGrip::Versatile,
    attack_attribute: AttackAttribute::Finesse,
    attack_enhancement: Some(*SLASHING),
    on_attacked_reaction: None,
    bonus_melee_evasion: 3,
    on_damage: None,
    on_kill_apply_self: None,
    sprite: Some(EquipmentSpriteId::Rapier),
    icon: EquipmentIconId::Rapier,
    weight: 2,
});

pub const SWORD: LazyLock<Weapon> = LazyLock::new(|| Weapon {
    name: "Sword",
    range: WeaponRange::Melee,
    action_point_cost: 3,
    damage: 6,
    grip: WeaponGrip::Versatile,
    attack_attribute: AttackAttribute::Finesse,
    attack_enhancement: Some(*SLASHING),
    on_attacked_reaction: None,
    bonus_melee_evasion: 3,
    on_damage: None,
    on_kill_apply_self: None,
    sprite: Some(EquipmentSpriteId::Sword),
    icon: EquipmentIconId::Sword,
    weight: 2,
});

pub const MAGIC_SWORD: Weapon = Weapon {
    name: "Magic sword",
    range: WeaponRange::Melee,
    action_point_cost: 3,
    damage: 6,
    grip: WeaponGrip::Versatile,
    attack_attribute: AttackAttribute::Finesse,
    attack_enhancement: None,
    on_attacked_reaction: None,
    bonus_melee_evasion: 3,
    on_damage: Some(AttackHitEffect::ApplySelf(ApplyEffect::GainMana(1))),
    on_kill_apply_self: None,
    sprite: Some(EquipmentSpriteId::MagicSword),
    icon: EquipmentIconId::MagicSword,
    weight: 2,
};

pub const HUNGERING_BLADE: Weapon = Weapon {
    name: "Hungering blade",
    range: WeaponRange::Melee,
    action_point_cost: 3,
    damage: 6,
    grip: WeaponGrip::Versatile,
    attack_attribute: AttackAttribute::Finesse,
    attack_enhancement: Some(AttackEnhancement {
        name: "Unleash souls",
        description:
            "Spend your |<keyword>Captured souls| for |<value>2| additional damage per stack",
        icon: IconId::Banshee,
        action_point_cost: 0,
        mana_cost: 0,
        stamina_cost: 1,
        weapon_requirement: Some(WeaponType::Melee),
        effect: AttackEnhancementEffect {
            bonus_damage_per_self_condition: Some((Condition::HungeringBladeSouls, 2)),
            ..AttackEnhancementEffect::default()
        },
    }),
    on_attacked_reaction: None,
    bonus_melee_evasion: 3,
    on_damage: Some(AttackHitEffect::ApplySelf(ApplyEffect::Condition(
        ApplyCondition {
            condition: Condition::HungeringBladeSouls,
            stacks: Some(1),
            duration_rounds: None,
        },
    ))),
    on_kill_apply_self: None,
    sprite: Some(EquipmentSpriteId::HungeringBlade),
    icon: EquipmentIconId::HungeringBlade,
    weight: 2,
};

pub const ZERO_SWORD: LazyLock<Weapon> = LazyLock::new(|| Weapon {
    name: "Useless sword",
    range: WeaponRange::Melee,
    action_point_cost: 0,
    damage: 2,
    grip: WeaponGrip::Versatile,
    attack_attribute: AttackAttribute::Finesse,
    attack_enhancement: Some(*SLASHING),
    on_attacked_reaction: None,
    bonus_melee_evasion: 3,
    on_damage: None,
    on_kill_apply_self: None,
    sprite: Some(EquipmentSpriteId::Sword),
    icon: EquipmentIconId::Sword,
    weight: 2,
});

pub const ZERO_BOW: Weapon = Weapon {
    name: "Useless bow",
    range: WeaponRange::Ranged(15.5),
    action_point_cost: 0,
    damage: 1,
    grip: WeaponGrip::TwoHanded,
    attack_attribute: AttackAttribute::Agility,
    attack_enhancement: Some(CAREFUL_AIM),
    on_attacked_reaction: None,
    bonus_melee_evasion: 0,
    on_damage: None,
    on_kill_apply_self: None,
    sprite: Some(EquipmentSpriteId::Bow),
    icon: EquipmentIconId::Bow,
    weight: 2,
};

const FEINT: AttackEnhancement = AttackEnhancement {
    name: "Feint",
    description: "Reduce the target's defense by 6 against the next attack.",
    icon: IconId::Feint,
    effect: AttackEnhancementEffect {
        roll_modifier: -3,
        on_target: Some(apply_condition_rounds(Condition::Distracted, 1)),
        ..AttackEnhancementEffect::default()
    },
    ..AttackEnhancement::default()
};

pub const BAD_RAPIER: Weapon = Weapon {
    name: "Rapier",
    range: WeaponRange::Melee,
    action_point_cost: 3,
    damage: 5,
    grip: WeaponGrip::MainHand,
    attack_attribute: AttackAttribute::Finesse,
    attack_enhancement: Some(FEINT),
    on_attacked_reaction: None,
    bonus_melee_evasion: 3,
    on_damage: None,
    on_kill_apply_self: None,
    sprite: Some(EquipmentSpriteId::Rapier),
    icon: EquipmentIconId::Rapier,
    weight: 2,
};

pub const SLASHING_RAPIER: Weapon = Weapon {
    name: "Slashing rapier",
    range: WeaponRange::Melee,
    action_point_cost: 3,
    damage: 5,
    grip: WeaponGrip::MainHand,
    attack_attribute: AttackAttribute::Finesse,
    attack_enhancement: Some(FEINT),
    on_attacked_reaction: None,
    bonus_melee_evasion: 3,
    on_damage: Some(AttackHitEffect::ApplyTarget(ApplyEffect::Condition(
        ApplyCondition {
            condition: Condition::Bleeding,
            stacks: Some(2),
            duration_rounds: None,
        },
    ))),
    on_kill_apply_self: None,
    sprite: Some(EquipmentSpriteId::Rapier),
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
    on_attacked_reaction: None,
    bonus_melee_evasion: 3,
    on_damage: None,
    on_kill_apply_self: None,
    sprite: Some(EquipmentSpriteId::Rapier),
    icon: EquipmentIconId::Rapier,
    weight: 2,
};

const BONK: AttackEnhancement = AttackEnhancement {
    name: "Bonk",
    description: "Target loses |<value>1| |<ap>| AP.",
    icon: IconId::Bonk,
    stamina_cost: 1,
    effect: AttackEnhancementEffect {
        on_damage_effect: Some(AttackEnhancementOnHitEffect::Target(
            None,
            ApplyEffect::LoseActionPoints(1),
        )),
        //bonus_damage: 3, //TODO
        visual_effect: Some(EffectId::Bonk),
        ..AttackEnhancementEffect::default()
    },
    ..AttackEnhancement::default()
};

pub const BAD_WAR_HAMMER: Weapon = Weapon {
    name: "War hammer",
    range: WeaponRange::Melee,
    action_point_cost: 3,
    damage: 6,
    grip: WeaponGrip::TwoHanded,
    attack_attribute: AttackAttribute::Strength,
    attack_enhancement: Some(BONK),
    on_attacked_reaction: None,
    bonus_melee_evasion: 3,
    on_damage: None,
    on_kill_apply_self: None,
    sprite: Some(EquipmentSpriteId::Warhammer),
    icon: EquipmentIconId::Warhammer,
    weight: 5,
};

pub const BOSS_AXE: Weapon = Weapon {
    name: "Boss axe",
    range: WeaponRange::Melee,
    action_point_cost: 3,
    damage: 7,
    grip: WeaponGrip::TwoHanded,
    attack_attribute: AttackAttribute::Strength,
    attack_enhancement: None,
    on_attacked_reaction: None,
    bonus_melee_evasion: 3,
    on_damage: None,
    on_kill_apply_self: None,
    sprite: Some(EquipmentSpriteId::Bossaxe),
    icon: EquipmentIconId::Undefined,
    weight: 5,
};

pub const WAR_AXE: Weapon = Weapon {
    name: "War axe",
    range: WeaponRange::Melee,
    action_point_cost: 3,
    damage: 7,
    grip: WeaponGrip::TwoHanded,
    attack_attribute: AttackAttribute::Strength,
    attack_enhancement: None,
    on_attacked_reaction: None,
    bonus_melee_evasion: 3,
    on_damage: None,
    on_kill_apply_self: None,
    sprite: Some(EquipmentSpriteId::Waraxe),
    icon: EquipmentIconId::Waraxe,
    weight: 5,
};

pub const WAR_HAMMER: Weapon = Weapon {
    name: "War hammer",
    range: WeaponRange::Melee,
    action_point_cost: 3,
    damage: 7,
    grip: WeaponGrip::TwoHanded,
    attack_attribute: AttackAttribute::Strength,
    attack_enhancement: Some(BONK),
    on_attacked_reaction: None,
    bonus_melee_evasion: 3,
    on_damage: None,
    on_kill_apply_self: None,
    sprite: Some(EquipmentSpriteId::Warhammer),
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
    attack_enhancement: Some(BONK),
    on_attacked_reaction: None,
    bonus_melee_evasion: 3,
    on_damage: Some(AttackHitEffect::ApplyTarget(ApplyEffect::Condition(
        ApplyCondition {
            condition: Condition::Dazed,
            stacks: None,
            duration_rounds: Some(1),
        },
    ))),
    on_kill_apply_self: None,
    sprite: Some(EquipmentSpriteId::Warhammer),
    icon: EquipmentIconId::Warhammer,
    weight: 7,
};

pub const STAFF: Weapon = Weapon {
    name: "Staff",
    range: WeaponRange::Melee,
    action_point_cost: 3,
    damage: 4,
    grip: WeaponGrip::TwoHanded,
    attack_attribute: AttackAttribute::Agility,
    attack_enhancement: None,
    on_attacked_reaction: None,
    bonus_melee_evasion: 2,
    on_damage: None,
    on_kill_apply_self: None,
    sprite: Some(EquipmentSpriteId::Staff),
    icon: EquipmentIconId::Staff,
    weight: 2,
};

pub const BAD_BOW: Weapon = Weapon {
    name: "Bow",
    range: WeaponRange::Ranged(15.5),
    action_point_cost: 3,
    damage: 4,
    grip: WeaponGrip::TwoHanded,
    attack_attribute: AttackAttribute::Agility,
    attack_enhancement: Some(CAREFUL_AIM),
    on_attacked_reaction: None,
    bonus_melee_evasion: 0,
    on_damage: None,
    on_kill_apply_self: None,
    sprite: Some(EquipmentSpriteId::Bow),
    icon: EquipmentIconId::Bow,
    weight: 2,
};

pub const CHEAT_BOW: Weapon = Weapon {
    name: "Bow",
    range: WeaponRange::Ranged(50.0),
    action_point_cost: 0,
    damage: 99,
    grip: WeaponGrip::TwoHanded,
    attack_attribute: AttackAttribute::Agility,
    attack_enhancement: Some(CAREFUL_AIM),
    on_attacked_reaction: None,
    bonus_melee_evasion: 0,
    on_damage: None,
    on_kill_apply_self: None,
    sprite: Some(EquipmentSpriteId::Bow),
    icon: EquipmentIconId::Bow,
    weight: 2,
};

pub const BOW: Weapon = Weapon {
    name: "Bow",
    range: WeaponRange::Ranged(15.5),
    action_point_cost: 3,
    damage: 6,
    grip: WeaponGrip::TwoHanded,
    attack_attribute: AttackAttribute::Agility,
    attack_enhancement: Some(CAREFUL_AIM),
    on_attacked_reaction: None,
    bonus_melee_evasion: 0,
    on_damage: None,
    on_kill_apply_self: None,
    sprite: Some(EquipmentSpriteId::Bow),
    icon: EquipmentIconId::Bow,
    weight: 2,
};

pub const ARCANE_BOW: Weapon = Weapon {
    name: "Arcane bow",
    range: WeaponRange::Ranged(15.5),
    action_point_cost: 3,
    damage: 6,
    grip: WeaponGrip::TwoHanded,
    attack_attribute: AttackAttribute::Agility,
    attack_enhancement: Some(CAREFUL_AIM),
    on_attacked_reaction: None,
    bonus_melee_evasion: 0,
    on_damage: None,
    on_kill_apply_self: Some(ApplyEffect::GainMana(1)),
    sprite: Some(EquipmentSpriteId::Bow),
    icon: EquipmentIconId::ArcaneBow,
    weight: 2,
};

pub const ELUSIVE_BOW: Weapon = Weapon {
    name: "Elusive bow",
    range: WeaponRange::Ranged(18.5),
    action_point_cost: 3,
    damage: 7,
    grip: WeaponGrip::TwoHanded,
    attack_attribute: AttackAttribute::Agility,
    attack_enhancement: Some(CAREFUL_AIM),
    on_attacked_reaction: None,
    bonus_melee_evasion: 0,
    on_damage: None,
    on_kill_apply_self: None,
    sprite: Some(EquipmentSpriteId::Bow),
    icon: EquipmentIconId::Bow,
    weight: 2,
};

pub const PENETRATING_ARROWS: Arrow = Arrow {
    name: "Penetrating arrows",
    description: "Bypasses |<value>3| of the target's |<shield>| armor.",
    icon: EquipmentIconId::PenetratingArrow,
    bonus_penetration: 3,
    on_damage_apply: None,
    circle_effect: None,
};

pub const BARBED_ARROWS: Arrow = Arrow {
    name: "Barbed arrows",
    description: "Inflicts |<value>3| |<keyword>Bleeding|.",
    icon: EquipmentIconId::BarbedArrow,
    bonus_penetration: 0,
    on_damage_apply: Some(apply_condition_stacks(Condition::Bleeding, 3)),
    circle_effect: None,
};

pub const COLD_ARROWS: Arrow = Arrow {
    name: "Cold arrows",
    description: "Inflicts |<keyword>Hindered|.",
    icon: EquipmentIconId::ColdArrow,
    bonus_penetration: 0,
    on_damage_apply: Some(apply_condition_rounds(Condition::Hindered, 1)),
    circle_effect: None,
};

pub const FIRE_ARROWS: Arrow = Arrow {
    name: "Fire arrows",
    description: "Deals |<value>2| |<fire>|fire damage in an area and sets the ground on fire",
    icon: EquipmentIconId::FireArrow,
    bonus_penetration: 0,
    on_damage_apply: None,
    circle_effect: Some(CircleEffect::new(
        Range::Melee,
        TargetAcquisition::Any,
        AbilityTargetEffect::Spell(SpellTargetEffect {
            damage: Some(AbilityDamage::Fixed(2, DamageType::Fire)),
            ..SpellTargetEffect::default()
        }),
        Some(Surface::Fire),
    )),
};

pub const EXPLODING_ARROWS: Arrow = Arrow {
    name: "Exploding arrows",
    description: "Deals |<value>2| |<fire>|fire damage in an area and pushes affected targets.",
    icon: EquipmentIconId::ExplodingArrow,
    bonus_penetration: 0,
    on_damage_apply: None,

    circle_effect: Some(CircleEffect::new(
        Range::Melee,
        TargetAcquisition::Any,
        AbilityTargetEffect::Spell(SpellTargetEffect {
            damage: Some(AbilityDamage::Fixed(2, DamageType::Fire)),
            defense_type: Some(DefenseType::Toughness),
            on_hit: &[ApplyEffect::Pushed(2)],
            ..SpellTargetEffect::default()
        }),
        None,
    )),
};

pub const ARCANE_RING: Trinket = Trinket {
    name: "Arcane ring",
    sprite: None,
    equip: EquipEffect {
        bonus_spell_modifier: 3,
        ..EquipEffect::default()
    },
    icon: EquipmentIconId::ArcaneRing,
};

pub const NECROTIC_BAND: Trinket = Trinket {
    name: "Necrotic band",
    sprite: None,
    equip: EquipEffect {
        heal_on_kill: 2,
        ..EquipEffect::default()
    },
    icon: EquipmentIconId::NecroticBand,
};

pub const HUNTERS_CHARM: Trinket = Trinket {
    name: "Hunter's charm",
    sprite: None,
    equip: EquipEffect {
        bonus_move_speed: 2,
        bonus_stamina_regen: 1,
        ..EquipEffect::default()
    },
    icon: EquipmentIconId::HuntersCharm,
};

pub const DRAUG_NECKLACE: Trinket = Trinket {
    name: "Draug necklace",
    sprite: None,
    equip: EquipEffect {
        // TODO: upon killing or critting a bleeding target, gain some health
        attack_advantage_against_bleeding_target: true,
        ..EquipEffect::default()
    },
    icon: EquipmentIconId::DraugNecklace,
};

pub const BATTLEMAGE_SIGIL: Trinket = Trinket {
    name: "Battle mage sigil",
    sprite: None,
    equip: EquipEffect {
        on_attack_damage_apply_self: Some(apply_condition(Condition::ArcaneBlessing)),
        ..EquipEffect::default()
    },
    icon: EquipmentIconId::BattleMageSigil,
};

pub const ASSASSINS_VIAL: Trinket = Trinket {
    name: "Assassin's vial",
    sprite: None,
    equip: EquipEffect {
        on_attack_damage_apply: Some(apply_condition_rounds(Condition::Poisoned, 3)),
        ..EquipEffect::default()
    },
    icon: EquipmentIconId::AssassinsVial,
};

pub const SAPHIRE: Trinket = Trinket {
    name: "Saphire",
    sprite: None,
    equip: EquipEffect {
        start_of_turn_apply_self: Some(apply_condition_stacks(Condition::SpellFinesse, 1)),
        ..EquipEffect::default()
    },
    icon: EquipmentIconId::Saphire,
};

pub const RUBY: Trinket = Trinket {
    name: "Ruby",
    sprite: None,
    equip: EquipEffect {
        start_of_turn_maybe_apply_self: Some((
            Circumstance::AdjacentToMultipleEnemies,
            apply_condition_stacks(Condition::Bloodlust, 1),
        )),
        ..EquipEffect::default()
    },
    icon: EquipmentIconId::Ruby,
};

pub const LIQUID_FIRE: Consumable = Consumable {
    name: "Liquid fire",
    icon: EquipmentIconId::LiquidFire,
    weight: 0,
    ability: &LIQUID_FIRE_ABILITY,
};

const LIQUID_FIRE_ABILITY: Ability = grenade_ability(
    AbilityId::Grenade,
    "Liquid fire",
    AbilityTarget::PositionCircle {
        range: Range::Tenths(135),
        circle_effect: CircleEffect::new(
            Range::Melee,
            TargetAcquisition::Any,
            AbilityTargetEffect::Spell(SpellTargetEffect {
                defense_type: None,
                damage: Some(AbilityDamage::Fixed(2, DamageType::Fire)),
                on_hit: &[],
                environment_effect: None,
            }),
            Some(Surface::Fire),
        ),
    },
    COL_RED,
);

pub const TOXIC_GRENADE: Consumable = Consumable {
    name: "Toxic grenade",
    icon: EquipmentIconId::ToxicGrenade,
    weight: 0,
    ability: &TOXIC_GRENADE_ABILITY,
};

const TOXIC_GRENADE_ABILITY: Ability = grenade_ability(
    AbilityId::Grenade,
    "Toxic grenade",
    AbilityTarget::Character {
        acquisition: TargetAcquisition::Enemies,
        reach: AbilityReach::Range(Range::f32(13.5)),
        effect: AbilityTargetEffect::Spell(SpellTargetEffect {
            defense_type: Some(DefenseType::Evasion),
            damage: Some(AbilityDamage::Fixed(3, DamageType::Regular)),
            on_hit: &[apply_condition_rounds(Condition::Poisoned, 2)],
            ..SpellTargetEffect::default()
        }),
        circle_effect: None,
        environment_effect: None,
    },
    COL_GREEN_3,
);

const fn grenade_ability(
    id: AbilityId,
    name: &'static str,
    target: AbilityTarget,
    color: Color,
) -> Ability {
    Ability {
        id,
        name,

        description: "",
        icon: IconId::UseConsumable,
        action_point_cost: 2,
        mana_cost: 0,
        stamina_cost: 0,
        requirement: None,

        roll: Some(AbilityRollType::Spell),
        possible_enhancements: &[],
        target: target,
        audio_visuals: AudioVisuals {
            speech: "Hey, catch!",
            animation_color: color,
            animation: AbilityAnimation::ThrowGrenade(color),
            initiate_sound: Some(SoundId::ThrowGrenade),
            resolve_sound: Some(SoundId::BottleSplash),
            // TODO should depend on which grenade
            character_target_effect: Some((EffectId::FireBomb, 0.5)),
            charge_fx: Some(AbilityChargeFx {
                particle_shape: ParticleShape::Circle,
            }),
            ..AudioVisuals::default()
        },
    }
}

pub const BAD_SMALL_SHIELD: Shield = Shield {
    name: "Small shield",
    sprite: Some(EquipmentSpriteId::Shield),
    icon: EquipmentIconId::SmallShield,
    evasion: 2,
    armor: 0,
    on_hit_reaction: None,
    on_attacked_reaction: None,
    weight: 2,
};

pub const SHIELD_BASH_KNOCKBACK: AbilityEnhancement = AbilityEnhancement {
    name: "Knockback",
    description: "Attempt to |<keyword>Push| the target.",
    icon: IconId::Knockback,
    stamina_cost: 1,
    spell_effect: Some(SpellEnhancementEffect {
        target_on_hit: &[ApplyEffect::Pushed(3)],
        ..SpellEnhancementEffect::default()
    }),
    ..AbilityEnhancement::default()
};

static SHIELD_BASH: OnceLock<Ability> = OnceLock::new();
pub fn shield_bash() -> &'static Ability {
    let damage = 3;
    let condition = Condition::Dazed;

    SHIELD_BASH.get_or_init(|| Ability {
        id: AbilityId::ShieldBash,
        name: "Shield bash",

        description: format!(
            "Deal |<value>{}| damage and inflict {} for |<value>1| round.",
            damage,
            condition.name_with_icon()
        )
        .leak(),
        icon: IconId::ShieldBash,
        action_point_cost: 2,
        stamina_cost: 1,
        mana_cost: 0,
        requirement: Some(EquipmentRequirement::Shield),
        possible_enhancements: &[&SHIELD_BASH_KNOCKBACK],

        roll: Some(AbilityRollType::RollAbilityWithAttackModifier),
        target: AbilityTarget::Character {
            acquisition: TargetAcquisition::Enemies,
            reach: AbilityReach::Range(Range::Melee),
            effect: AbilityTargetEffect::Spell(SpellTargetEffect {
                defense_type: Some(DefenseType::Toughness),
                damage: Some(AbilityDamage::Dynamic(damage, DamageType::Regular)),
                on_hit: vec![apply_condition_rounds(condition, 1)].leak(),
                ..SpellTargetEffect::default()
            }),
            circle_effect: None,
            environment_effect: None,
        },
        audio_visuals: AudioVisuals {
            animation_color: GRAY,
            animation: AbilityAnimation::ShieldAttack,
            initiate_sound: None,
            resolve_sound: Some(SoundId::ShieldBash),
            charge_fx: None,
            ..AudioVisuals::default()
        },
    })
}

pub const ENEMY_TACKLE: Ability = Ability {
    id: AbilityId::Tackle,
    name: "Tackle",

    description: "",
    icon: IconId::Tackle,
    action_point_cost: 3,
    stamina_cost: 0,
    mana_cost: 0,
    requirement: None,
    possible_enhancements: &[],

    roll: Some(AbilityRollType::RollAbilityWithAttackModifier),
    target: AbilityTarget::Character {
        acquisition: TargetAcquisition::Enemies,
        reach: AbilityReach::Range(Range::Melee),
        effect: AbilityTargetEffect::Spell(SpellTargetEffect {
            defense_type: Some(DefenseType::Toughness),
            damage: Some(AbilityDamage::Dynamic(3, DamageType::Regular)),
            on_hit: &[
                apply_condition_rounds(Condition::Dazed, 1),
                // Better keep this somewhat low. Critting while knocking into an obstacle can lead to crazy numbers.
                ApplyEffect::Pushed(2),
            ],
            ..SpellTargetEffect::default()
        }),
        circle_effect: None,
        environment_effect: None,
    },
    audio_visuals: AudioVisuals {
        animation_color: GRAY,
        animation: AbilityAnimation::MeleeWeaponAttack,
        initiate_sound: None,
        resolve_sound: Some(SoundId::Explosion),
        charge_fx: None,
        ..AudioVisuals::default()
    },
};

pub const ENEMY_SLASHING_ATTACK: Ability = Ability {
    id: AbilityId::EnemySlashingAttack,
    name: "Slashing attack",

    description: "",
    icon: IconId::Slashing,
    action_point_cost: 3,
    stamina_cost: 0,
    mana_cost: 0,
    requirement: Some(EquipmentRequirement::Weapon(WeaponType::Melee)),
    possible_enhancements: &[],

    roll: Some(AbilityRollType::RollDuringAttack(0)),
    target: AbilityTarget::Character {
        acquisition: TargetAcquisition::Enemies,
        reach: AbilityReach::Range(Range::Melee),
        effect: AbilityTargetEffect::PerformAttack(AbilityAttackEffect {
            override_weapon_damage: Some(2),
            on_hit: Some(apply_condition_stacks(Condition::Bleeding, 2)),
            ..AbilityAttackEffect::default()
        }),
        circle_effect: None,
        environment_effect: None,
    },
    audio_visuals: AudioVisuals {
        animation_color: GRAY,
        animation: AbilityAnimation::MeleeWeaponAttack,
        initiate_sound: None,
        resolve_sound: Some(SoundId::Explosion),
        charge_fx: None,
        ..AudioVisuals::default()
    },
};

pub const SMALL_SHIELD: Shield = Shield {
    name: "Small shield",
    sprite: Some(EquipmentSpriteId::Shield),
    icon: EquipmentIconId::SmallShield,
    evasion: 3,
    armor: 0,
    on_hit_reaction: None,
    on_attacked_reaction: Some(BLOCK),
    weight: 2,
};

pub const SPIKE_SHIELD: Shield = Shield {
    name: "Spike shield",
    sprite: Some(EquipmentSpriteId::SpikeShield),
    icon: EquipmentIconId::SpikeShield,
    evasion: 3,
    armor: 0,
    on_hit_reaction: None,
    on_attacked_reaction: Some(BLOCK_2_WITH_THORNS),
    weight: 2,
};

pub const MEDIUM_SHIELD: Shield = Shield {
    name: "Medium shield",
    sprite: Some(EquipmentSpriteId::Shield),
    icon: EquipmentIconId::MediumShield,
    evasion: 3,
    armor: 1,
    on_hit_reaction: None,
    on_attacked_reaction: Some(BLOCK),
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
        bonus_damage: 3,
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
            Some(DefenseType::Toughness),
            ApplyEffect::LoseActionPoints(2),
        )),
        ..AttackEnhancementEffect::default()
    },
    ..AttackEnhancement::default()
};

pub const CAREFUL_AIM: AttackEnhancement = AttackEnhancement {
    name: "Careful aim",
    description: "Roll with |<keyword>Advantage| and bypass |<value>3| enemy |<shield>| armor.",
    icon: IconId::CarefulAim,
    action_point_cost: 1,
    effect: AttackEnhancementEffect {
        roll_advantage: 1,
        armor_penetration: 3,
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
        range_bonus: 6,
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
    description: "Attempt to |<keyword>Hinder| the target for |<value>1| round.",
    icon: IconId::CripplingShot,
    stamina_cost: 2,
    effect: AttackEnhancementEffect {
        on_damage_effect: Some(AttackEnhancementOnHitEffect::Target(
            Some(DefenseType::Toughness),
            apply_condition_rounds(Condition::Hindered, 1),
        )),
        ..AttackEnhancementEffect::default()
    },
    weapon_requirement: Some(WeaponType::Ranged),
    ..AttackEnhancement::default()
};

pub const PARRY_2: OnAttackedReaction = OnAttackedReaction {
    id: OnAttackedReactionId::Parry,
    name: "Parry 2",
    granted_from: None,
    description: "Protect yourself |<faded>(1 melee attack per round)|.",
    icon: IconId::Parry,
    action_point_cost: 0,
    stamina_cost: 2,
    effect: OnAttackedReactionEffect {
        bonus_evasion: 0,
        bonus_armor: 0,
        damage_prevention: 2,
        thorns: 0,
    },
    required_attack_type: Some(AttackType::Melee),
    used_hand: Some(HandType::MainHand),
    target: OnAttackedReactionTarget::OnlySelf,
};

pub const BLOCK: OnAttackedReaction = OnAttackedReaction {
    id: OnAttackedReactionId::Block,
    name: "Block",
    granted_from: None,
    description: "Protect yourself or adjacent ally, preventing |<value>2| attack damage |<faded>(once per round)|.",
    icon: IconId::Block,
    action_point_cost: 0,
    stamina_cost: 1,
    effect: OnAttackedReactionEffect {
        bonus_evasion: 0,
        bonus_armor: 0,
        damage_prevention: 2,
        thorns: 0,
    },
    required_attack_type: None,
    used_hand: Some(HandType::OffHand),
    target: OnAttackedReactionTarget::SelfOrAdjacentAlly,
};

pub const BLOCK_2_WITH_THORNS: OnAttackedReaction = OnAttackedReaction {
    id: OnAttackedReactionId::Block,
    name: "Spike block",
    granted_from: None,
    description: "Protect yourself, preventing |<value>2| damage and dealing |<value>3| damage back to the attacker |<faded>(once per round)|.",
    icon: IconId::Block,
    action_point_cost: 0,
    stamina_cost: 1,
    effect: OnAttackedReactionEffect {
        bonus_evasion: 0,
        bonus_armor: 0,
        damage_prevention: 2,
        thorns: 3,
    },
    required_attack_type: None,
    used_hand: Some(HandType::OffHand),
    target: OnAttackedReactionTarget::OnlySelf,
};

pub const SIDE_STEP: OnAttackedReaction = OnAttackedReaction {
    id: OnAttackedReactionId::SideStep,
    name: "Side step",
    granted_from: None,
    description: "Attempt to dodge an incoming attack.",
    icon: IconId::Sidestep,
    action_point_cost: 1,
    stamina_cost: 2,
    effect: OnAttackedReactionEffect {
        bonus_evasion: 10,
        bonus_armor: 0,
        damage_prevention: 0,
        thorns: 0,
    },
    required_attack_type: None,
    used_hand: None,
    target: OnAttackedReactionTarget::OnlySelf,
};

pub const RAGE: OnHitReaction = OnHitReaction {
    name: "Rage",
    description: Condition::Raging.description(),
    icon: IconId::Rage,
    action_point_cost: 1,
    stamina_cost: 1,
    effect: OnHitReactionEffect::Rage,
    required_attack_type: None,
};

pub const SWEEP_ATTACK_PRECISE: AbilityEnhancement = AbilityEnhancement {
    name: "Precise",
    description: "Increase your precision",
    icon: IconId::Precision,
    stamina_cost: 1,
    attack_effect: Some(AttackEnhancementEffect {
        roll_modifier: 3,
        ..AttackEnhancementEffect::default()
    }),
    ..AbilityEnhancement::default()
};
pub const SWEEP_ATTACK_PUSH: AbilityEnhancement = AbilityEnhancement {
    name: "Push",
    description: "|<keyword>Push| targets away from you.",
    icon: IconId::Extend,
    stamina_cost: 1,
    attack_effect: Some(AttackEnhancementEffect {
        on_damage_effect: Some(AttackEnhancementOnHitEffect::Target(
            None,
            ApplyEffect::Pushed(2),
        )),
        ..AttackEnhancementEffect::default()
    }),
    ..AbilityEnhancement::default()
};

pub const SWEEP_ATTACK: Ability = Ability {
    id: AbilityId::SweepAttack,
    name: "Sweeping attack",

    description: "Attack all surrounding enemies. Deals your |<sword>| weapon damage |<value>-1|.",
    icon: IconId::SweepAttack,
    action_point_cost: 3,
    mana_cost: 0,
    stamina_cost: 2,
    requirement: Some(EquipmentRequirement::Weapon(WeaponType::Melee)),

    roll: Some(AbilityRollType::RollDuringAttack(0)),
    possible_enhancements: &[&SWEEP_ATTACK_PRECISE, &SWEEP_ATTACK_PUSH],
    target: AbilityTarget::None {
        self_circle: Some(CircleEffect::new(
            Range::f32(2.5),
            TargetAcquisition::Enemies,
            AbilityTargetEffect::PerformAttack(AbilityAttackEffect {
                damage_delta: -1,
                ..AbilityAttackEffect::default()
            }),
            None,
        )),
        self_effect: None,
        environment_effect: None,
    },
    audio_visuals: AudioVisuals {
        animation_color: BLACK,
        animation: AbilityAnimation::SpinAttack,
        initiate_sound: Some(SoundId::SweepAttack),
        resolve_sound: None,
        charge_fx: Some(AbilityChargeFx {
            particle_shape: ParticleShape::Circle,
        }),
        ..AudioVisuals::default()
    },
};

pub const LUNGE_ATTACK_HEAVY_IMPACT: AbilityEnhancement = AbilityEnhancement {
    name: "Heavy impact",
    description: "",
    icon: IconId::CrushingStrike,
    stamina_cost: 1,
    attack_effect: Some(AttackEnhancementEffect {
        on_damage_effect: Some(AttackEnhancementOnHitEffect::Target(
            Some(DefenseType::Toughness),
            ApplyEffect::LoseActionPoints(1),
        )),
        ..AttackEnhancementEffect::default()
    }),
    ..AbilityEnhancement::default()
};
pub const LUNGE_ATTACK_REACH: AbilityEnhancement = AbilityEnhancement {
    name: "Reach",
    description: "",
    icon: IconId::Extend,
    stamina_cost: 1,
    // TODO surely we don't need to increase the range both with SpellEnhancementEffect and AttackEnhancementEffect
    // One of them should be the correct way.
    spell_effect: Some(SpellEnhancementEffect {
        increased_range_tenths: 10,
        ..SpellEnhancementEffect::default()
    }),
    attack_effect: Some(AttackEnhancementEffect {
        range_bonus: 1,
        ..AttackEnhancementEffect::default()
    }),
    ..AbilityEnhancement::default()
};
pub const LUNGE_ATTACK: Ability = Ability {
    id: AbilityId::LungeAttack,
    name: "Lunge attack",

    description: "Move to target in an unobstructed path, before attacking.",
    icon: IconId::LungeAttack,
    action_point_cost: 3,
    mana_cost: 0,
    stamina_cost: 1,
    requirement: Some(EquipmentRequirement::Weapon(WeaponType::Melee)),

    roll: Some(AbilityRollType::RollDuringAttack(0)),
    possible_enhancements: &[&LUNGE_ATTACK_HEAVY_IMPACT, &LUNGE_ATTACK_REACH],
    target: AbilityTarget::Character {
        acquisition: TargetAcquisition::Enemies,
        reach: AbilityReach::MoveIntoMelee(Range::f32(10.0)),
        effect: AbilityTargetEffect::PerformAttack(AbilityAttackEffect::default()),
        circle_effect: None,
        environment_effect: None,
    },
    audio_visuals: AudioVisuals {
        animation_color: GRAY,
        animation: AbilityAnimation::MeleeWeaponAttack,
        initiate_sound: None,
        resolve_sound: Some(SoundId::Explosion),
        charge_fx: Some(AbilityChargeFx {
            particle_shape: ParticleShape::Circle,
        }),
        ..AudioVisuals::default()
    },
};

pub const EXECUTE_BLOODLUST: AbilityEnhancement = AbilityEnhancement {
    name: "Bloodlust",
    description: "Also gain |<keyword>Ferocity| if it kills.",
    icon: IconId::Rage,
    stamina_cost: 1,
    attack_effect: Some(AttackEnhancementEffect {
        on_kill_apply_self: Some(apply_condition_stacks(Condition::Ferocity, 1)),
        ..AttackEnhancementEffect::default()
    }),
    ..AbilityEnhancement::default()
};

pub const EXECUTE: Ability = Ability {
    id: AbilityId::Execute,
    name: "Execute",

    description: "Attack with advantage, and regain |<value>2| AP if it kills.",
    icon: IconId::Execute,
    action_point_cost: 3,
    mana_cost: 0,
    stamina_cost: 2,
    requirement: Some(EquipmentRequirement::Weapon(WeaponType::Melee)),
    roll: Some(AbilityRollType::RollDuringAttack(1)),
    possible_enhancements: &[&EXECUTE_BLOODLUST],
    target: AbilityTarget::Character {
        acquisition: TargetAcquisition::Enemies,
        reach: AbilityReach::Range(Range::Melee),
        effect: AbilityTargetEffect::PerformAttack(AbilityAttackEffect {
            on_kill_apply_self: Some(ApplyEffect::GainActionPoints(2)),
            ..AbilityAttackEffect::default()
        }),
        circle_effect: None,
        environment_effect: None,
    },
    audio_visuals: AudioVisuals {
        animation_color: GRAY,
        animation: AbilityAnimation::MeleeWeaponAttack,
        initiate_sound: Some(SoundId::Execute),
        resolve_sound: None,
        charge_fx: Some(AbilityChargeFx {
            particle_shape: ParticleShape::Circle,
        }),
        ..AudioVisuals::default()
    },
};

pub const ENEMY_BRACE: Ability = Ability {
    id: AbilityId::Brace,
    name: "Brace",

    description: "Take reduced damage from one attack.",
    icon: IconId::Block,
    action_point_cost: 2,
    mana_cost: 0,
    stamina_cost: 0,
    requirement: Some(EquipmentRequirement::Shield),
    roll: None,
    possible_enhancements: &[],
    target: AbilityTarget::None {
        self_circle: None,

        self_effect: Some(AbilityTargetEffect::Spell(SpellTargetEffect {
            defense_type: None,
            damage: None,
            on_hit: &[ApplyEffect::Condition(ApplyCondition {
                condition: Condition::Protected,
                stacks: Some(2),
                duration_rounds: Some(1),
            })],
            environment_effect: None,
        })),

        environment_effect: None,
    },
    audio_visuals: AudioVisuals {
        animation_color: GRAY,
        animation: AbilityAnimation::CastSpell(SpellTravelAnimation::None),
        initiate_sound: None,
        resolve_sound: Some(SoundId::BuffBrace),
        charge_fx: None,
        ..AudioVisuals::default()
    },
};

pub const BRACE: Ability = Ability {
    id: AbilityId::Brace,
    name: "Brace",

    description: "",
    icon: IconId::Brace,
    // TODO:
    action_point_cost: 0,
    mana_cost: 0,
    stamina_cost: 0,
    requirement: Some(EquipmentRequirement::Shield),
    roll: None,
    possible_enhancements: &[],
    target: AbilityTarget::None {
        self_circle: None,
        self_effect: Some(uncontested_spell_effect(&[ApplyEffect::Condition(
            ApplyCondition {
                condition: Condition::Protected,
                stacks: Some(2),
                duration_rounds: Some(1),
            },
        )])),
        environment_effect: None,
    },
    audio_visuals: AudioVisuals {
        animation_color: GRAY,
        animation: AbilityAnimation::CastSpell(SpellTravelAnimation::None),
        initiate_sound: None,
        resolve_sound: Some(SoundId::BuffBrace),
        charge_fx: Some(AbilityChargeFx {
            particle_shape: ParticleShape::Circle,
        }),
        // TODO
        character_target_effect: Some((EffectId::Bonk, 0.3)),
        ..AudioVisuals::default()
    },
};

pub const SCREAM_SHRIEK: AbilityEnhancement = AbilityEnhancement {
    name: "Shriek",
    description: "Increased range",
    icon: IconId::Banshee,
    mana_cost: 1,
    attack_effect: None,
    spell_effect: Some(SpellEnhancementEffect {
        increased_radius_tenths: 45,
        ..SpellEnhancementEffect::default()
    }),
    ..AbilityEnhancement::default()
};
pub const SCREAM: Ability = Ability {
    id: AbilityId::Scream,
    name: "Scream",

    description: "Daze nearby enemies.",
    icon: IconId::Scream,
    action_point_cost: 2,
    mana_cost: 1,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    target: AbilityTarget::None {
        self_circle: Some(CircleEffect::new(
            Range::of(7),
            TargetAcquisition::Enemies,
            AbilityTargetEffect::Spell(SpellTargetEffect {
                defense_type: Some(DefenseType::Will),
                on_hit: &[apply_condition_rounds(Condition::Dazed, 1)],
                ..SpellTargetEffect::default()
            }),
            None,
        )),
        self_effect: None,
        environment_effect: None,
    },
    possible_enhancements: &[&SCREAM_SHRIEK],

    audio_visuals: AudioVisuals {
        animation_color: BLUE,
        animation: AbilityAnimation::CastSpell(SpellTravelAnimation::None),
        initiate_sound: None,
        resolve_sound: Some(SoundId::Explosion),
        charge_fx: Some(AbilityChargeFx {
            particle_shape: ParticleShape::Circle,
        }),
        ..AudioVisuals::default()
    },
};

pub const SHACKLED_MIND: Ability = Ability {
    id: AbilityId::ShackledMind,
    name: "Shackle",

    description: "Attempt to |<keyword>Slow| and |<keyword>Expose| an enemy.",
    icon: IconId::ShackledMind,
    action_point_cost: 3,
    mana_cost: 2,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    target: AbilityTarget::Character {
        acquisition: TargetAcquisition::Enemies,
        reach: AbilityReach::Range(Range::f32(12.5)),
        effect: AbilityTargetEffect::Spell(SpellTargetEffect {
            defense_type: Some(DefenseType::Will),

            on_hit: &[
                apply_condition_rounds(Condition::Slowed, 1),
                apply_condition_rounds(Condition::Exposed, 2),
            ],
            ..SpellTargetEffect::default()
        }),
        circle_effect: None,
        environment_effect: None,
    },
    possible_enhancements: &[],
    /*
        Some(AbilityEnhancement {
            name: "Reach",
            description: "",
            icon: IconId::Extend,
            mana_cost: 1,
            attack_effect: None,
            spell_effect: Some(SpellEnhancementEffect {
                increased_range_tenths: 30,
                ..SpellEnhancementEffect::default()
            }),
            ..AbilityEnhancement::default()
        }),
        Some(AbilityEnhancement {
            name: "Focus",
            description: "",
            icon: IconId::SpellAdvantage,
            mana_cost: 1,
            attack_effect: None,
            spell_effect: Some(SpellEnhancementEffect {
                bonus_advantage: 1,
                ..SpellEnhancementEffect::default()
            }),
            ..AbilityEnhancement::default()
        }),
        None,
    ],
     */
    audio_visuals: AudioVisuals {
        animation_color: Color::new(0.2, 0.1, 0.5, 1.00),
        animation: AbilityAnimation::CastSpell(SpellTravelAnimation::Beam(BeamFxId::Chains)),
        initiate_sound: Some(SoundId::ShootSpell),
        resolve_sound: Some(SoundId::Debuff),
        character_target_effect: Some((EffectId::PurpleMagic, 0.25)),
        charge_fx: Some(AbilityChargeFx {
            particle_shape: ParticleShape::Rect,
        }),
        ..AudioVisuals::default()
    },
};

pub const MIND_BLAST: Ability = Ability {
    id: AbilityId::MindBlast,
    name: "Mind blast",

    description: "Damage an enemy and make them lose AP.",
    icon: IconId::Mindblast,
    action_point_cost: 3,
    mana_cost: 1,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    possible_enhancements: &[],
    /*
        Some(AbilityEnhancement {
            name: "Dualcast",
            description: "Spell is cast twice",
            icon: IconId::Dualcast,
            action_point_cost: 1,
            mana_cost: 1,
            spell_effect: Some(SpellEnhancementEffect {
                cast_twice: true,
                ..SpellEnhancementEffect::default()
            }),
            ..AbilityEnhancement::default()
        }),
        None,
        None,
    ],
     */
    target: AbilityTarget::Character {
        acquisition: TargetAcquisition::Enemies,
        reach: AbilityReach::Range(Range::f32(13.5)),
        effect: AbilityTargetEffect::Spell(SpellTargetEffect {
            defense_type: Some(DefenseType::Will),
            damage: Some(AbilityDamage::Fixed(4, DamageType::Regular)),
            on_hit: &[ApplyEffect::LoseActionPoints(1)],
            ..SpellTargetEffect::default()
        }),
        circle_effect: None,
        environment_effect: None,
    },
    audio_visuals: AudioVisuals {
        animation_color: PURPLE,
        animation: AbilityAnimation::CastSpell(SpellTravelAnimation::None),
        initiate_sound: Some(SoundId::ShootSpell),
        resolve_sound: Some(SoundId::Debuff),
        charge_fx: Some(AbilityChargeFx {
            particle_shape: ParticleShape::Circle,
        }),
        ..AudioVisuals::default()
    },
};

pub const INFLICT_WOUNDS_NECROTIC_INFLUENCE: AbilityEnhancement = AbilityEnhancement {
    name: "Necrotic influence",
    description: "Heal and grow stronger for each affected target.",
    icon: IconId::NecroticInfluence,
    mana_cost: 1,
    spell_effect: Some(SpellEnhancementEffect::default()),
    apply_on_self_per_area_target_hit: &[
        ApplyEffect::GainHealth(1),
        apply_condition_stacks(Condition::ArcaneSurge, 1),
    ],
    ..AbilityEnhancement::default()
};
pub const INFLICT_WOUNDS: Ability = Ability {
    id: AbilityId::InflictWounds,
    name: "Inflict wounds",

    description: "Deal damage and cause |<keyword>Bleeding| in an area.",
    icon: IconId::NecroticInfluence,
    action_point_cost: 3,
    mana_cost: 0, // TODO: 1,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    possible_enhancements: &[&INFLICT_WOUNDS_NECROTIC_INFLUENCE],
    target: AbilityTarget::PositionCircle {
        range: Range::f32(7.5),
        circle_effect: CircleEffect::new(
            Range::f32(2.5),
            TargetAcquisition::Enemies,
            AbilityTargetEffect::Spell(SpellTargetEffect {
                defense_type: Some(DefenseType::Toughness),
                damage: Some(AbilityDamage::Dynamic(4, DamageType::Regular)),
                on_hit: &[apply_condition_stacks(Condition::Bleeding, 1)],
                ..SpellTargetEffect::default()
            }),
            None,
        ),
    },
    audio_visuals: AudioVisuals {
        animation_color: BROWN,
        animation: AbilityAnimation::CastSpell(SpellTravelAnimation::None),
        initiate_sound: Some(SoundId::ShootSpell),
        resolve_sound: Some(SoundId::Debuff),
        charge_fx: Some(AbilityChargeFx {
            particle_shape: ParticleShape::Circle,
        }),
        ..AudioVisuals::default()
    },
};

pub const DRAUG_HAUNT: Ability = Ability {
    id: AbilityId::DraugHaunt,
    name: "Haunt",

    description: "Deal |<value>3| damage in a small area.",
    icon: IconId::DraugHaunt,
    action_point_cost: 3,
    mana_cost: 0,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    possible_enhancements: &[],

    target: AbilityTarget::Character {
        acquisition: TargetAcquisition::Enemies,
        reach: AbilityReach::Range(Range::Melee),
        effect: AbilityTargetEffect::Spell(SpellTargetEffect {
            ..SpellTargetEffect::default()
        }),
        circle_effect: Some(CircleEffect::new(
            Range::of(4),
            TargetAcquisition::Enemies,
            AbilityTargetEffect::Spell(SpellTargetEffect {
                defense_type: Some(DefenseType::Will),
                damage: Some(AbilityDamage::Dynamic(3, DamageType::Regular)),
                ..SpellTargetEffect::default()
            }),
            None,
        )),
        environment_effect: None,
    },
    audio_visuals: AudioVisuals {
        animation_color: PURPLE,
        animation: AbilityAnimation::MeleeWeaponAttack,
        initiate_sound: Some(SoundId::ShootSpell),
        resolve_sound: Some(SoundId::Debuff),
        charge_fx: None,
        ..AudioVisuals::default()
    },
};

pub const DRAUG_CLAW: Ability = Ability {
    id: AbilityId::DraugClaw,
    name: "Claw",
    description: "Deal |<value>4| damage and inflict |<keyword>Blinded|.",
    icon: IconId::DraugClaw,
    action_point_cost: 3,
    mana_cost: 0,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    possible_enhancements: &[],
    target: AbilityTarget::Character {
        acquisition: TargetAcquisition::Enemies,
        reach: AbilityReach::Range(Range::Melee),
        effect: AbilityTargetEffect::Spell(SpellTargetEffect {
            defense_type: Some(DefenseType::Evasion),
            damage: Some(AbilityDamage::Dynamic(3, DamageType::Regular)),
            on_hit: &[apply_condition_rounds(Condition::Blinded, 1)],
            ..SpellTargetEffect::default()
        }),
        circle_effect: None,
        environment_effect: None,
    },
    audio_visuals: AudioVisuals {
        animation_color: PURPLE,
        animation: AbilityAnimation::MeleeWeaponAttack,
        initiate_sound: Some(SoundId::ShootSpell),
        resolve_sound: Some(SoundId::Debuff),
        charge_fx: None,
        ..AudioVisuals::default()
    },
};

pub const ENEMY_ESCAPE: Ability = Ability {
    id: AbilityId::EnemyEscape,
    name: "Escape",

    description: "",
    icon: IconId::Escape,
    action_point_cost: 4,
    mana_cost: 0,
    stamina_cost: 0,
    requirement: None,

    roll: None,
    possible_enhancements: &[],
    target: AbilityTarget::None {
        self_circle: None,
        self_effect: Some(AbilityTargetEffect::Spell(SpellTargetEffect {
            defense_type: None,
            damage: None,
            on_hit: &[ApplyEffect::Escape],
            environment_effect: None,
        })),
        environment_effect: None,
    },
    audio_visuals: AudioVisuals {
        speech: "Don't mind me!",
        animation_color: BROWN,
        animation: AbilityAnimation::CastSpell(SpellTravelAnimation::None),
        initiate_sound: None,
        resolve_sound: Some(SoundId::Laugh),
        charge_fx: None,
        ..AudioVisuals::default()
    },
};

pub const HULDRA_INFECT: Ability = Ability {
    id: AbilityId::HuldraInfect,
    name: "Infect",

    description: "",
    icon: IconId::NecroticInfluence,
    action_point_cost: 3,
    mana_cost: 0,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    possible_enhancements: &[],
    target: AbilityTarget::Character {
        acquisition: TargetAcquisition::Enemies,
        effect: AbilityTargetEffect::Spell(SpellTargetEffect {
            defense_type: Some(DefenseType::Toughness),
            damage: Some(AbilityDamage::Dynamic(3, DamageType::Regular)),
            on_hit: &[apply_condition_rounds(Condition::Poisoned, 1)],
            ..SpellTargetEffect::default()
        }),
        circle_effect: None,
        environment_effect: None,
        reach: AbilityReach::Range(Range::f32(15.5)),
    },
    audio_visuals: AudioVisuals {
        animation_color: BROWN,
        animation: AbilityAnimation::CastSpell(SpellTravelAnimation::Line {
            impact_time_ratio: 1.0,
        }),
        initiate_sound: Some(SoundId::ShootSpell),
        resolve_sound: Some(SoundId::Infect),
        charge_fx: None,
        ..AudioVisuals::default()
    },
};

pub const HULDRA_INFLICT_HORRORS: Ability = Ability {
    id: AbilityId::HuldraInflictHorrors,
    name: "Curse",

    description: "",
    icon: IconId::Mindblast,
    action_point_cost: 2,
    mana_cost: 0,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    possible_enhancements: &[],
    target: AbilityTarget::Character {
        acquisition: TargetAcquisition::Enemies,
        reach: AbilityReach::Range(Range::of(22)),
        effect: AbilityTargetEffect::Spell(SpellTargetEffect {
            defense_type: Some(DefenseType::Will),

            on_hit: &[apply_condition_rounds(Condition::Slowed, 1)],
            ..SpellTargetEffect::default()
        }),
        circle_effect: None,
        environment_effect: None,
    },
    audio_visuals: AudioVisuals {
        animation_color: PURPLE,
        animation: AbilityAnimation::CastSpell(SpellTravelAnimation::None),
        initiate_sound: Some(SoundId::ShootSpell),
        resolve_sound: Some(SoundId::Debuff),
        charge_fx: None,
        ..AudioVisuals::default()
    },
};

pub static HULDRA_BLOOD_MAGIC: Ability = Ability {
    id: AbilityId::HuldraHeal,
    name: "Blood magic",

    description: "",
    icon: IconId::BloodMagic,
    action_point_cost: 3,
    mana_cost: 0,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    target: ally_spell_target(
        Range::f32(15.5),
        &[
            ApplyEffect::GainHealth(8),
            ApplyEffect::Condition(ApplyCondition {
                condition: Condition::Protected,
                stacks: Some(6),
                duration_rounds: Some(1),
            }),
        ],
    ),
    possible_enhancements: &[],
    audio_visuals: AudioVisuals {
        animation_color: LIME,
        animation: AbilityAnimation::CastSpell(SpellTravelAnimation::None),
        initiate_sound: Some(SoundId::ShootSpell),
        resolve_sound: Some(SoundId::Powerup),
        charge_fx: None,
        target_magic_sparks: Some(WHITE),
        target_reaction: AbilityVisualReaction::Enjoy,
        ..AudioVisuals::default()
    },
};

const fn ally_spell_target(range: Range, apply: &'static [ApplyEffect]) -> AbilityTarget {
    AbilityTarget::Character {
        acquisition: TargetAcquisition::Allies,
        reach: AbilityReach::Range(range),
        effect: uncontested_spell_effect(apply),
        circle_effect: None,
        environment_effect: None,
    }
}

pub const HEAL_ENERGIZE: AbilityEnhancement = AbilityEnhancement {
    name: "Energize",
    description: "Also restore |<value>4| |<stamina>| stamina.",
    icon: IconId::Energize,
    mana_cost: 1,
    spell_effect: Some(SpellEnhancementEffect {
        target_on_hit: &[ApplyEffect::GainStamina(4)],
        ..SpellEnhancementEffect::default()
    }),
    ..AbilityEnhancement::default()
};

static NATURES_BLESSING: OnceLock<Ability> = OnceLock::new();
pub fn natures_blessing() -> &'static Ability {
    NATURES_BLESSING.get_or_init(|| {
        Ability {
            id: AbilityId::Bless,
            name: "Nature's blessing",
            description: format!(
                "|<keyword>Protect| an ally from one attack, and cure them of {}, {}.",
                Condition::Bleeding.name_with_icon(),
                Condition::Poisoned.name_with_icon()
            )
            .leak(),
            icon: IconId::Bless,
            action_point_cost: 2,
            mana_cost: 1,
            stamina_cost: 0,
            requirement: None,

            roll: Some(AbilityRollType::Spell),
            target: ally_spell_target(
                Range::f32(9.5),
                &[
                    ApplyEffect::Condition(ApplyCondition {
                        condition: Condition::Protected,
                        stacks: Some(3),
                        duration_rounds: Some(1),
                    }),
                    ApplyEffect::LoseCondition {
                        condition: Condition::Bleeding,
                    },
                    ApplyEffect::LoseCondition {
                        condition: Condition::Poisoned,
                    },
                ],
            ),
            // TODO add enhancement that heals over time (1 per round for 3 turns?)
            possible_enhancements: &[&BLESS_REACH, &HEAL_ENERGIZE],

            audio_visuals: AudioVisuals {
                animation_color: DARKGRAY,
                charge_fx: Some(AbilityChargeFx {
                    particle_shape: ParticleShape::Circle,
                }),
                initiate_sound: Some(SoundId::ShootSpell),
                animation: AbilityAnimation::CastSpell(SpellTravelAnimation::StraightProjectile),
                target_magic_sparks: Some(WHITE),
                character_target_effect: Some((EffectId::Raven, 0.5)),
                resolve_sound: Some(SoundId::Heal),
                target_reaction: AbilityVisualReaction::Enjoy,
                ..AudioVisuals::default()
            },
        }
    })
}

pub const BLESS_REACH: AbilityEnhancement = AbilityEnhancement {
    name: "Reach",
    description: "",
    icon: IconId::Extend,
    mana_cost: 1,
    spell_effect: Some(SpellEnhancementEffect {
        increased_range_tenths: 60,
        ..SpellEnhancementEffect::default()
    }),
    ..AbilityEnhancement::default()
};

pub const HASTE: Ability = Ability {
    id: AbilityId::Haste,
    name: "Haste",

    description: "Grant an ally bonus AP and movement.",
    icon: IconId::Haste,
    action_point_cost: 2,
    mana_cost: 2,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    target: ally_spell_target(
        Range::f32(9.5),
        &[apply_condition_rounds(Condition::Hastened, 4)],
    ),
    possible_enhancements: &[],
    audio_visuals: AudioVisuals {
        animation_color: GREEN,
        animation: AbilityAnimation::CastSpell(SpellTravelAnimation::None),
        initiate_sound: Some(SoundId::ShootSpell),
        resolve_sound: Some(SoundId::Powerup),
        charge_fx: Some(AbilityChargeFx {
            particle_shape: ParticleShape::Circle,
        }),
        target_magic_sparks: Some(WHITE),
        ..AudioVisuals::default()
    },
};

pub const INSPIRE_BLOODLUST: AbilityEnhancement = AbilityEnhancement {
    name: "Bloodlust",
    description: "Also make targets more likely to score critical hits, for |<value>2| rounds.",
    icon: IconId::Bloodlust,
    mana_cost: 1,
    spell_effect: Some(SpellEnhancementEffect {
        area_on_hit: &[apply_condition_rounds(Condition::Bloodlust, 2)],
        ..SpellEnhancementEffect::default()
    }),
    ..AbilityEnhancement::default()
};

static INSPIRE: OnceLock<Ability> = OnceLock::new();

pub fn inspire() -> &'static Ability {
    let condition = Condition::Inspired;
    let duration = 3;

    INSPIRE.get_or_init(|| Ability {
        id: AbilityId::Inspire,
        name: "Inspire",

        description: format!(
            "Grant nearby allies {} for |<value>{}| rounds.",
            condition.name_with_icon(),
            duration
        )
        .leak(),
        icon: IconId::Inspire,
        action_point_cost: 2,
        mana_cost: 2,
        stamina_cost: 0,
        requirement: None,

        roll: Some(AbilityRollType::Spell),
        possible_enhancements: &[&INSPIRE_BLOODLUST],
        target: AbilityTarget::None {
            self_circle: Some(CircleEffect::new(
                Range::f32(12.5),
                TargetAcquisition::Allies,
                uncontested_spell_effect(vec![apply_condition_rounds(condition, duration)].leak()),
                None,
            )),
            self_effect: None,
            environment_effect: None,
        },
        audio_visuals: AudioVisuals {
            animation_color: GREEN,
            animation: AbilityAnimation::CastSpell(SpellTravelAnimation::None),
            initiate_sound: None,
            resolve_sound: Some(SoundId::Powerup),
            charge_fx: Some(AbilityChargeFx {
                particle_shape: ParticleShape::Circle,
            }),
            target_magic_sparks: Some(WHITE),
            target_reaction: AbilityVisualReaction::Enjoy,
            ..AudioVisuals::default()
        },
    })
}

const fn uncontested_spell_effect(apply: &'static [ApplyEffect]) -> AbilityTargetEffect {
    AbilityTargetEffect::Spell(SpellTargetEffect {
        defense_type: None,
        damage: None,
        on_hit: apply,
        environment_effect: None,
    })
}

pub const ENEMY_INSPIRE: Ability = Ability {
    id: AbilityId::Inspire,
    name: "Inspire",

    description: "|<keyword>Inspire| nearby allies for |<value>2| rounds.",
    icon: IconId::Inspire,
    action_point_cost: 3,
    mana_cost: 0,
    stamina_cost: 0,
    requirement: None,

    roll: None,
    possible_enhancements: &[],
    target: AbilityTarget::None {
        self_circle: Some(CircleEffect::new(
            Range::f32(12.5),
            TargetAcquisition::Allies,
            uncontested_spell_effect(&[apply_condition_rounds(Condition::Inspired, 2)]),
            None,
        )),
        self_effect: None,
        environment_effect: None,
    },
    audio_visuals: AudioVisuals {
        charge_fx: None,
        animation_color: GREEN,
        initiate_sound: None,
        animation: AbilityAnimation::CastSpell(SpellTravelAnimation::None),
        resolve_sound: Some(SoundId::Powerup),
        target_magic_sparks: Some(WHITE),
        target_reaction: AbilityVisualReaction::Enjoy,
        ..AudioVisuals::default()
    },
};

pub const HEALING_NOVA: Ability = Ability {
    id: AbilityId::HealingNova,
    name: "Healing nova",

    description: "Restore health to nearby allies.",
    icon: IconId::PlusPlus,
    action_point_cost: 3,
    mana_cost: 1,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    possible_enhancements: &[],
    target: AbilityTarget::None {
        self_circle: Some(CircleEffect::new(
            Range::of(12),
            TargetAcquisition::Allies,
            uncontested_spell_effect(&[ApplyEffect::GainHealth(1)]),
            None,
        )),
        self_effect: None,
        environment_effect: None,
    },
    audio_visuals: AudioVisuals {
        animation_color: GREEN,
        animation: AbilityAnimation::CastSpell(SpellTravelAnimation::None),
        initiate_sound: None,
        resolve_sound: Some(SoundId::Powerup),
        charge_fx: Some(AbilityChargeFx {
            particle_shape: ParticleShape::Circle,
        }),
        target_magic_sparks: Some(WHITE),
        ..AudioVisuals::default()
    },
};

pub const SPIKE_TRAP: Ability = Ability {
    id: AbilityId::Spikes,
    name: "Spike trap",
    description: "Throw sharp |<keyword>spikes| on the ground, slowing and damaging anyone that steps over them.",
    icon: IconId::SpikeTrap,
    action_point_cost: 2,
    mana_cost: 0,
    stamina_cost: 2,
    requirement: None,
    roll: Some(AbilityRollType::Spell),
    possible_enhancements: &[],

    target: AbilityTarget::PositionCircle {
        range: Range::f32(6.5),
        circle_effect: CircleEffect::new(
            Range::f32(3.5),
            TargetAcquisition::Any,
            AbilityTargetEffect::Spell(SpellTargetEffect {
                damage: Some(AbilityDamage::Fixed(1, DamageType::Regular)),
                ..SpellTargetEffect::default()
            }),
            Some(Surface::Spikes),
        )
        ,
    },
    audio_visuals: AudioVisuals {animation_color: GRAY,
    animation: AbilityAnimation::CastSpell(SpellTravelAnimation::StraightProjectile),
    //TODO
    initiate_sound: Some(SoundId::AttackMiss),
    //TODO
    resolve_sound: Some(SoundId::Crit),
    charge_fx: None,..AudioVisuals::default()}
};

pub const DASH: Ability = Ability {
    id: AbilityId::Dash,
    name: "Dash",

    description:
        "Move swiftly in a straight line.\nDoes not trigger |<reaction>| opportunity attacks.",
    icon: IconId::Dash,
    action_point_cost: 1,
    mana_cost: 0,
    stamina_cost: 2,
    requirement: None,
    roll: None,
    possible_enhancements: &[],
    target: AbilityTarget::Movement {
        range: Range::f32(8.0),
    },
    audio_visuals: AudioVisuals {
        animation_color: RED,
        animation: AbilityAnimation::Dash,
        initiate_sound: None,
        resolve_sound: None,
        charge_fx: None,
        ..AudioVisuals::default()
    },
};

pub const PIERCING_SHOT: Ability = Ability {
    id: AbilityId::PiercingShot,
    name: "Piercing shot",

    description:
        "Attack all enemies in a line. Deals |<value>1| less damage for each consecutive target.",
    icon: IconId::PiercingShot,
    action_point_cost: 3,
    mana_cost: 0,
    stamina_cost: 2,
    requirement: Some(EquipmentRequirement::Weapon(WeaponType::Ranged)),

    roll: Some(AbilityRollType::RollDuringAttack(0)),
    possible_enhancements: &[],

    target: AbilityTarget::Line {
        range: Range::f32(15.5),
        area_effect: AreaEffect {
            acquisition: TargetAcquisition::Enemies,
            target_effect: AbilityTargetEffect::PerformAttack(AbilityAttackEffect {
                damage_penalty_per_consecutive_target: 1,
                ..AbilityAttackEffect::default()
            }),
            create_surface: None,
        },
    },

    audio_visuals: AudioVisuals {
        animation_color: YELLOW,
        animation: AbilityAnimation::RangedWeaponAttackLine,
        initiate_sound: Some(SoundId::ShootArrow),
        resolve_sound: None, //Some(SoundId::HitArrow),
        charge_fx: Some(AbilityChargeFx {
            particle_shape: ParticleShape::Circle,
        }),
        ..AudioVisuals::default()
    },
};

pub const REACH: AbilityEnhancement = AbilityEnhancement {
    name: "Reach",
    description: "",
    icon: IconId::Extend,
    action_point_cost: 1,
    spell_effect: Some(SpellEnhancementEffect {
        increased_range_tenths: 120,
        ..SpellEnhancementEffect::default()
    }),
    ..AbilityEnhancement::default()
};

pub const MASSIVE_RADIUS: AbilityEnhancement = AbilityEnhancement {
    name: "Massive",
    description: "",
    icon: IconId::Radius,
    mana_cost: 1,
    spell_effect: Some(SpellEnhancementEffect {
        increased_radius_tenths: 40,
        ..SpellEnhancementEffect::default()
    }),
    ..AbilityEnhancement::default()
};
pub const FIREBALL_INFERNO: AbilityEnhancement = AbilityEnhancement {
    name: "Inferno",
    description: "",
    icon: IconId::Inferno,
    mana_cost: 1,
    spell_effect: Some(SpellEnhancementEffect {
        area_create_surface: Some(Surface::Fire),
        ..SpellEnhancementEffect::default()
    }),
    ..AbilityEnhancement::default()
};
pub const FIREBALL: Ability = Ability {
    id: AbilityId::Fireball,
    name: "Fireball",

    description:
        "Deal |<value>3| |<fire>|fire damage to an enemy and |<value>4| to its surroundings",
    icon: IconId::Fireball,
    action_point_cost: 3,
    mana_cost: 1,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    target: AbilityTarget::Character {
        acquisition: TargetAcquisition::Enemies,
        reach: AbilityReach::Range(Range::f32(12.5)),
        effect: AbilityTargetEffect::Spell(SpellTargetEffect {
            defense_type: Some(DefenseType::Evasion),
            damage: Some(AbilityDamage::Dynamic(3, DamageType::Fire)),
            ..SpellTargetEffect::default()
        }),
        circle_effect: Some(CircleEffect::new(
            Range::f32(3.5),
            TargetAcquisition::Any,
            AbilityTargetEffect::Spell(SpellTargetEffect {
                damage: Some(AbilityDamage::Fixed(4, DamageType::Fire)),
                ..SpellTargetEffect::default()
            }),
            None,
        )),
        environment_effect: None,
    },
    possible_enhancements: &[&REACH, &MASSIVE_RADIUS, &FIREBALL_INFERNO],

    audio_visuals: AudioVisuals {
        animation_color: RED,
        animation: AbilityAnimation::CastSpell(SpellTravelAnimation::StraightProjectile),
        initiate_sound: Some(SoundId::ShootSpell),
        resolve_sound: Some(SoundId::FireballHit),
        charge_fx: Some(AbilityChargeFx {
            particle_shape: ParticleShape::Circle,
        }),
        ..AudioVisuals::default()
    },
};

pub const FIRE_BOLT: Ability = Ability {
    id: AbilityId::FireBolt,
    name: "Fire bolt",
    description: "Shoot a projectile that deals |<value>3| |<fire>|fire damage \
    to the first target hit, and |<value>4| |<fire>|fire damage in a small area.",
    icon: IconId::Fireball,
    action_point_cost: 3,
    mana_cost: 1,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    target: AbilityTarget::Projectile {
        range: Range::f32(12.5),
        impact_effect: AbilityTargetEffect::Spell(SpellTargetEffect {
            defense_type: Some(DefenseType::Toughness),
            damage: Some(AbilityDamage::Dynamic(3, DamageType::Fire)),
            ..SpellTargetEffect::default()
        }),
        impact_circle: Some(CircleEffect::new(
            Range::f32(1.5),
            TargetAcquisition::Any,
            AbilityTargetEffect::Spell(SpellTargetEffect {
                damage: Some(AbilityDamage::Fixed(4, DamageType::Fire)),
                ..SpellTargetEffect::default()
            }),
            None,
        )),
    },
    possible_enhancements: &[&REACH, &MASSIVE_RADIUS, &FIREBALL_INFERNO],

    audio_visuals: AudioVisuals {
        animation_color: RED,
        animation: AbilityAnimation::CastSpell(SpellTravelAnimation::StraightProjectile),
        initiate_sound: Some(SoundId::ShootSpell),
        resolve_sound: Some(SoundId::FireballHit),
        charge_fx: Some(AbilityChargeFx {
            particle_shape: ParticleShape::Circle,
        }),
        // TODO: Not quite right - fire bomb should be reserved for grenade
        // This one needs a more "regular" fire animation
        // The "regular" fire animation can be used whenever someone takes damage from burning/fire surface also
        character_target_effect: Some((EffectId::FireBomb, 0.5)),
        ..AudioVisuals::default()
    },
};

pub const ICE_BOLT: Ability = Ability {
    id: AbilityId::IceBolt,
    name: "Ice bolt",

    description: "todo",
    icon: IconId::Icebolt,
    action_point_cost: 3,
    mana_cost: 1,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    target: AbilityTarget::Projectile {
        range: Range::f32(12.5),
        impact_effect: AbilityTargetEffect::Spell(SpellTargetEffect {
            defense_type: Some(DefenseType::Toughness),
            // TODO ice damage
            damage: Some(AbilityDamage::Dynamic(3, DamageType::Regular)),
            on_hit: &[apply_condition_rounds(Condition::Frosty, 1)],
            ..SpellTargetEffect::default()
        }),
        impact_circle: None,
    },
    possible_enhancements: &[&REACH],

    audio_visuals: AudioVisuals {
        animation_color: SKYBLUE,
        animation: AbilityAnimation::CastSpell(SpellTravelAnimation::StraightProjectile),
        initiate_sound: Some(SoundId::ShootSpell),
        resolve_sound: Some(SoundId::NoisyCrash),
        charge_fx: Some(AbilityChargeFx {
            particle_shape: ParticleShape::Circle,
        }),
        ..AudioVisuals::default()
    },
};

const fn apply_condition(condition: Condition) -> ApplyEffect {
    ApplyEffect::Condition(ApplyCondition::of(condition))
}

const fn apply_condition_stacks(condition: Condition, stacks: u32) -> ApplyEffect {
    ApplyEffect::Condition(ApplyCondition::stacks(condition, stacks))
}

const fn apply_condition_rounds(condition: Condition, rounds: u32) -> ApplyEffect {
    ApplyEffect::Condition(ApplyCondition::rounds(condition, rounds))
}

pub const ENEMY_BURNING_ARROW: Ability = Ability {
    id: AbilityId::EnemyBurningArrow,
    name: "Burning arrow",
    description: "Deal |<value>2| |<sword>| damage to an enemy and |<value>2| |<fire>|fire damage to its surroundings.",
    icon: IconId::Fireball,
    action_point_cost: 3,
    mana_cost: 0,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::RollAbilityWithAttackModifier),
    target: AbilityTarget::Character {acquisition: TargetAcquisition::Enemies,
        reach: AbilityReach::Range(Range::f32(12.5)),
        effect: AbilityTargetEffect::Spell(SpellTargetEffect {
            defense_type: Some(DefenseType::Evasion),
            damage: Some(AbilityDamage::Dynamic(2, DamageType::Regular)),
            ..SpellTargetEffect::default()
        }),
        circle_effect: Some(CircleEffect::new(
            Range::f32(3.5),
            TargetAcquisition::Any,
            AbilityTargetEffect::Spell(SpellTargetEffect {
                defense_type: Some(DefenseType::Toughness),
                damage: Some(AbilityDamage::Fixed(2, DamageType::Fire)),
                ..SpellTargetEffect::default()
            }),
            None,
        )),
        environment_effect: None,
    },
    possible_enhancements: &[],

    audio_visuals: AudioVisuals {
        animation_color: RED,
        animation: AbilityAnimation::CastSpell(SpellTravelAnimation::StraightProjectile),
        initiate_sound: Some(SoundId::ShootArrow),
        resolve_sound: Some(SoundId::FireballHit),
        charge_fx: Some(AbilityChargeFx {
            particle_shape: ParticleShape::Circle,
        }),
        ..AudioVisuals::default()
    }
};

pub const LIGHTNING_BOLT_REACH: AbilityEnhancement = AbilityEnhancement {
    name: "Reach",
    description: "",
    icon: IconId::Extend,
    action_point_cost: 1,
    mana_cost: 1,
    spell_effect: Some(SpellEnhancementEffect {
        increased_range_tenths: 80,
        ..SpellEnhancementEffect::default()
    }),
    ..AbilityEnhancement::default()
};

pub const LIGHTNING_BOLT: Ability = Ability {
    id: AbilityId::LightningBolt,
    name: "Lightning bolt",

    description: "Deal |<value>7| |<lightning>|lightning damage to all targets in a line.",
    icon: IconId::LightningBolt,
    action_point_cost: 3,
    mana_cost: 2,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),

    target: AbilityTarget::Line {
        range: Range::f32(12.0),
        area_effect: AreaEffect {
            acquisition: TargetAcquisition::Any,
            target_effect: AbilityTargetEffect::Spell(SpellTargetEffect {
                defense_type: Some(DefenseType::Evasion),
                damage: Some(AbilityDamage::Dynamic(7, DamageType::Lightning)),

                ..SpellTargetEffect::default()
            }),
            create_surface: None,
        },
    },

    possible_enhancements: &[&LIGHTNING_BOLT_REACH],

    audio_visuals: AudioVisuals {
        animation_color: YELLOW,
        animation: AbilityAnimation::CastSpell(SpellTravelAnimation::Beam(BeamFxId::Lightning)),
        initiate_sound: Some(SoundId::ShootSpell),
        resolve_sound: Some(SoundId::LightningHit),
        charge_fx: Some(AbilityChargeFx {
            particle_shape: ParticleShape::Circle,
        }),
        character_target_effect: Some((EffectId::Zap, 0.5)),
        ..AudioVisuals::default()
    },
};

pub const KILL: Ability = Ability {
    id: AbilityId::Kill,
    name: "Kill",

    description: "Kill all enemies.",
    icon: IconId::CarefulAim,
    action_point_cost: 1,
    mana_cost: 0,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    possible_enhancements: &[],
    target: AbilityTarget::None {
        self_circle: Some(CircleEffect::new(
            Range::of(50),
            TargetAcquisition::Enemies,
            AbilityTargetEffect::Spell(SpellTargetEffect {
                damage: Some(AbilityDamage::Fixed(99, DamageType::Regular)),
                ..SpellTargetEffect::default()
            }),
            None,
        )),
        self_effect: None,
        environment_effect: None,
    },
    audio_visuals: AudioVisuals {
        animation_color: BLACK,
        animation: AbilityAnimation::CastSpell(SpellTravelAnimation::None),
        initiate_sound: Some(SoundId::ShootSpell),
        resolve_sound: Some(SoundId::Explosion),
        charge_fx: Some(AbilityChargeFx {
            particle_shape: ParticleShape::Circle,
        }),
        ..AudioVisuals::default()
    },
};

pub const KILL_SELF: Ability = Ability {
    id: AbilityId::KillSelf,
    name: "Kill self",

    description: "Kill self.",
    icon: IconId::Fireball,
    action_point_cost: 4,
    mana_cost: 0,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    possible_enhancements: &[],
    target: AbilityTarget::None {
        self_circle: None,
        self_effect: Some(uncontested_spell_effect(&[ApplyEffect::LoseHealth(
            99,
            DamageSource::Unknown,
            DamageType::Regular,
        )])),
        environment_effect: None,
    },
    audio_visuals: AudioVisuals {
        animation_color: BLACK,
        animation: AbilityAnimation::CastSpell(SpellTravelAnimation::None),
        initiate_sound: Some(SoundId::ShootSpell),
        resolve_sound: Some(SoundId::Explosion),
        charge_fx: Some(AbilityChargeFx {
            particle_shape: ParticleShape::Circle,
        }),
        ..AudioVisuals::default()
    },
};

pub const SEARING_LIGHT_BURN: AbilityEnhancement = AbilityEnhancement {
    name: "Burn",
    description: "",
    icon: IconId::Inferno,
    mana_cost: 1,
    spell_effect: Some(SpellEnhancementEffect {
        target_on_hit: &[apply_condition_stacks(Condition::Burning, 2)],
        ..SpellEnhancementEffect::default()
    }),
    ..AbilityEnhancement::default()
};
pub const SEARING_LIGHT: Ability = Ability {
    id: AbilityId::SearingLight,
    name: "Searing light",

    description: "Envelops the target in blinding light.",
    icon: IconId::SearingLight,
    action_point_cost: 3,
    mana_cost: 1,
    stamina_cost: 0,
    requirement: None,

    roll: Some(AbilityRollType::Spell),
    possible_enhancements: &[&SEARING_LIGHT_BURN],
    target: AbilityTarget::Character {
        acquisition: TargetAcquisition::Enemies,
        reach: AbilityReach::Range(Range::of(9)),
        effect: AbilityTargetEffect::Spell(SpellTargetEffect {
            defense_type: Some(DefenseType::Toughness),
            damage: Some(AbilityDamage::Dynamic(3, DamageType::Regular)),
            on_hit: &[apply_condition_rounds(Condition::Blinded, 1)],
            ..SpellTargetEffect::default()
        }),
        circle_effect: None,
        environment_effect: None,
    },
    audio_visuals: AudioVisuals {
        animation_color: YELLOW,
        animation: AbilityAnimation::CastSpell(SpellTravelAnimation::None),
        initiate_sound: Some(SoundId::ShootSpell),
        resolve_sound: Some(SoundId::Explosion),
        charge_fx: Some(AbilityChargeFx {
            particle_shape: ParticleShape::Circle,
        }),
        ..AudioVisuals::default()
    },
};

pub const HEALTH_POTION: Consumable = Consumable {
    name: "Health potion",
    icon: EquipmentIconId::HealthPotion,
    weight: 0,
    ability: &potion_ability(
        AbilityId::HealthPotion,
        "Health potion",
        &[apply_condition_stacks(Condition::HealthPotionRecovering, 4)],
        COL_RED,
    ),
};

pub const MANA_POTION: Consumable = Consumable {
    name: "Mana potion",
    icon: EquipmentIconId::ManaPotion,
    weight: 0,
    ability: &potion_ability(
        AbilityId::ManaPotion,
        "Mana potion",
        &[ApplyEffect::GainMana(5)],
        COL_BLUE,
    ),
};

const fn potion_ability(
    id: AbilityId,
    name: &'static str,
    apply: &'static [ApplyEffect],
    color: Color,
) -> Ability {
    Ability {
        id,
        name,

        description: "",
        icon: IconId::UseConsumable,
        action_point_cost: 1,
        mana_cost: 0,
        stamina_cost: 0,
        requirement: None,

        roll: None,
        possible_enhancements: &[],
        target: AbilityTarget::None {
            self_circle: None,
            self_effect: Some(AbilityTargetEffect::Spell(SpellTargetEffect {
                defense_type: None,
                damage: None,
                on_hit: apply,
                environment_effect: None,
            })),
            environment_effect: None,
        },

        audio_visuals: AudioVisuals {
            speech: "Ahh...",
            initiate_sound: Some(SoundId::Gulp),
            animation_color: color,
            animation: AbilityAnimation::ConsumePotion,
            resolve_sound: Some(SoundId::Powerup),
            charge_fx: None,
            target_magic_sparks: Some(WHITE),
            target_reaction: AbilityVisualReaction::Enjoy,
            ..AudioVisuals::default()
        },
    }
}

pub const ADRENALIN_POTION: Consumable = Consumable {
    name: "Adrenaline potion",
    icon: EquipmentIconId::AdrenalinPotion,
    weight: 0,
    ability: &potion_ability(
        AbilityId::AdrenalinPotion,
        "Adrenaline potion    ",
        &[apply_condition_rounds(Condition::Adrenalin, 3)],
        YELLOW,
    ),
};

pub const ENERGY_POTION: Consumable = Consumable {
    name: "Energy potion",
    icon: EquipmentIconId::EnergyPotion,
    weight: 0,
    ability: &potion_ability(
        AbilityId::EnergyPotion,
        "Energy potion",
        &[ApplyEffect::GainStamina(10)],
        COL_GREEN_1,
    ),
};

pub const ARCANE_POTION: Consumable = Consumable {
    name: "Arcane potion",
    icon: EquipmentIconId::ArcanePotion,
    weight: 0,
    ability: &potion_ability(
        AbilityId::ArcanePotion,
        "Arcane potion",
        &[apply_condition_stacks(Condition::ArcaneSurge, 5)],
        PURPLE,
    ),
};

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PassiveSkill {
    HardenedSkin,
    WeaponProficiency,
    CriticalCharge,
    Reaper,
    BloodRage,
    ThrillOfBattle,
    Honorless,
    Vigilant,
    RisingTemper,
    SwampDweller,
    Slasher,
    Undead,
}

impl PassiveSkill {
    pub fn name(&self) -> &'static str {
        use PassiveSkill::*;
        match self {
            HardenedSkin => "Hardened skin",
            WeaponProficiency => "Weapon proficiency",
            CriticalCharge => "Critical charge",
            Reaper => "Reaper",
            BloodRage => "Blood rage",
            ThrillOfBattle => "Thrill of battle",
            Honorless => "Honorless",
            Vigilant => "Vigilant",
            RisingTemper => "Rising temper",
            SwampDweller => "Swamp dweller",
            Slasher => "Slasher",
            Undead => "Undead",
        }
    }

    pub fn icon(&self) -> IconId {
        use PassiveSkill::*;
        match self {
            HardenedSkin => IconId::HardenedSkin,
            WeaponProficiency => IconId::WeaponProficiency,
            CriticalCharge => IconId::CriticalCharge,
            Reaper => IconId::Reaper,
            // TODO: unique icon
            BloodRage => IconId::Rage,
            // TODO: unique icon
            ThrillOfBattle => IconId::MeleeAttack,
            // TODO: unique icon
            Honorless => IconId::RangedAttack,
            // TODO: unique icon
            Vigilant => IconId::MeleeAttack,
            // TODO: unique icon
            RisingTemper => IconId::Rage,
            SwampDweller => IconId::SwampDweller,

            Slasher => IconId::Slashing,
            Undead => IconId::Undead,
        }
    }

    pub fn description(&self) -> Cow<'static, str> {
        use PassiveSkill::*;
        match self {
            HardenedSkin => "|<value>+1| |<shield>| armor".into(),
            WeaponProficiency => "|<trigger>On strong / critical hit:| Attacks penetrate |<value>2| |<shield>| armor.".into(),
            CriticalCharge => "|<value>+3| |<blue_dice>| |<stat>Spell|, while at/below |<value>50%| |<mana>| mana.".into(),
            Reaper => "|<trigger>On kill:| gain |<value>2| |<stamina>| stamina and |<value>2| |<ap>| AP |<faded>(once per turn)|.".into(),
            BloodRage => "|<value>+3| |<red_dice>| Attack, while at/below |<value>50%| |<heart>| health. Immune to |<keyword>Near-death|.".into(),
            ThrillOfBattle => "|<value>+3| |<mixed_dice>||<stat>Attack/Spell|, while adjacent to more than one enemy. Immune to |<keyword>Flanked|.".into(),
            Honorless => "Attacks deal |<value>+1| damage against |<keyword>Flanked| targets.".into(),
            Vigilant => "Can |<reaction>| opportunity attack an enemy, even while engaging with another enemy.".into(),
            RisingTemper => format!("|<trigger>End of turn:| gain |<value>1| {}", Condition::Ferocity.name_with_icon()).into(),
            SwampDweller => format!("Immune to {}. While standing in water, the water is |<keyword>Poisonous|.", Condition::Poisoned.name_with_icon()).into(),
            Slasher => format!("Inflicts {} when dealing attack damage.", Condition::Bleeding.name_with_icon()).into(),
            Undead => format!("Immune to {}. |<value>-35%| |<boot>| movement speed.", Condition::Bleeding.name_with_icon()).into(),
        }
    }

    pub fn keywords(&self) -> &'static [Keyword] {
        use PassiveSkill::*;
        match self {
            BloodRage => &[Keyword::Cond(Condition::NearDeath, None)],
            RisingTemper => &[Keyword::Cond(Condition::Ferocity, None)],
            SwampDweller => &[Keyword::Cond(Condition::Poisoned, None)],
            Slasher => &[Keyword::Cond(Condition::Bleeding, None)],
            Undead => &[Keyword::Cond(Condition::Bleeding, None)],
            WeaponProficiency => &[Keyword::StrongWeak, Keyword::Crit],
            _ => &[],
        }
    }
}
