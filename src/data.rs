use crate::{
    core::{
        ApplyEffect, ArmorPiece, AttackAttribute, AttackEnhancement, AttackHitEffect, Condition,
        OnAttackedReaction, OnAttackedReactionEffect, OnHitReaction, OnHitReactionEffect, Range,
        SelfEffectAction, Shield, Spell, SpellEnhancement, SpellEnhancementEffect, SpellType,
        Weapon, WeaponGrip,
    },
    textures::{EquipmentIconId, IconId, SpriteId},
};

pub const LEATHER_ARMOR: ArmorPiece = ArmorPiece {
    name: "Leather armor",
    protection: 3,
    limit_defense_from_dex: None,
    icon: EquipmentIconId::LeatherArmor,
};

pub const CHAIN_MAIL: ArmorPiece = ArmorPiece {
    name: "Chain mail",
    protection: 5,
    limit_defense_from_dex: Some(4),
    icon: EquipmentIconId::ChainMail,
};

pub const DAGGER: Weapon = Weapon {
    name: "Dagger",
    range: Range::Melee,
    action_point_cost: 1,
    damage: 1,
    grip: WeaponGrip::Light,
    attack_attribute: AttackAttribute::Finesse,
    attack_enhancement: None,
    on_attacked_reaction: None,
    on_true_hit: Some(AttackHitEffect::Apply(ApplyEffect::Condition(
        Condition::Weakened(1),
    ))),
    sprite: Some(SpriteId::Dagger),
    icon: EquipmentIconId::Dagger,
};

pub const SWORD: Weapon = Weapon {
    name: "Sword",
    range: Range::Melee,
    action_point_cost: 2,
    damage: 1,
    grip: WeaponGrip::Versatile,
    attack_attribute: AttackAttribute::Finesse,
    attack_enhancement: None,
    on_attacked_reaction: Some(PARRY),
    on_true_hit: Some(AttackHitEffect::Apply(ApplyEffect::Condition(
        Condition::Bleeding,
    ))),
    sprite: Some(SpriteId::Sword),
    icon: EquipmentIconId::Sword,
};

pub const RAPIER: Weapon = Weapon {
    name: "Rapier",
    range: Range::Melee,
    action_point_cost: 2,
    damage: 1,
    grip: WeaponGrip::MainHand,
    attack_attribute: AttackAttribute::Finesse,
    attack_enhancement: None,
    on_attacked_reaction: Some(PARRY),
    on_true_hit: Some(AttackHitEffect::SkipExertion),
    sprite: Some(SpriteId::Rapier),
    icon: EquipmentIconId::Rapier,
};

pub const WAR_HAMMER: Weapon = Weapon {
    name: "War hammer",
    range: Range::Melee,
    action_point_cost: 2,
    damage: 2,
    grip: WeaponGrip::TwoHanded,
    attack_attribute: AttackAttribute::Strength,
    attack_enhancement: Some(AttackEnhancement {
        name: "All-in",
        description: "+1 damage",
        icon: IconId::AllIn,
        action_point_cost: 1,
        stamina_cost: 0,
        bonus_damage: 1,
        apply_on_self_before: None,
        on_hit_effect: None,
    }),
    on_attacked_reaction: Some(PARRY),
    on_true_hit: Some(AttackHitEffect::Apply(ApplyEffect::Condition(
        Condition::Dazed(1),
    ))),
    sprite: Some(SpriteId::Warhammer),
    icon: EquipmentIconId::Warhammer,
};

pub const BOW: Weapon = Weapon {
    name: "Bow",
    range: Range::Ranged(5),
    action_point_cost: 2,
    damage: 2,
    grip: WeaponGrip::TwoHanded,
    attack_attribute: AttackAttribute::Dexterity,
    attack_enhancement: Some(AttackEnhancement {
        name: "Careful aim",
        description: "Bonus advantage",
        icon: IconId::CarefulAim,
        action_point_cost: 1,
        stamina_cost: 0,
        bonus_damage: 0,
        apply_on_self_before: Some(Condition::CarefulAim),
        on_hit_effect: None,
    }),
    on_attacked_reaction: None,
    on_true_hit: Some(AttackHitEffect::Apply(ApplyEffect::Condition(
        Condition::Weakened(1),
    ))),
    sprite: Some(SpriteId::Bow),
    icon: EquipmentIconId::Bow,
};

pub const SMALL_SHIELD: Shield = Shield {
    name: "Small shield",
    sprite: Some(SpriteId::Shield),
    defense: 2,
    on_hit_reaction: Some(OnHitReaction {
        // TODO only in melee!
        name: "Shield bash",
        description: "Possibly daze attacker",
        icon: IconId::ShieldBash,
        action_point_cost: 1,
        effect: OnHitReactionEffect::ShieldBash,
    }),
};

pub const CRUSHING_STRIKE: AttackEnhancement = AttackEnhancement {
    name: "Crushing strike",
    description: "Target loses 1 AP",
    icon: IconId::CrushingStrike,
    action_point_cost: 0,
    stamina_cost: 1,
    bonus_damage: 0,
    apply_on_self_before: None,
    on_hit_effect: Some(ApplyEffect::RemoveActionPoints(1)),
};

pub const PARRY: OnAttackedReaction = OnAttackedReaction {
    name: "Parry",
    description: "Bonus defense",
    icon: IconId::Parry,
    action_point_cost: 1,
    stamina_cost: 0,
    effect: OnAttackedReactionEffect::Parry,
};

pub const SIDE_STEP: OnAttackedReaction = OnAttackedReaction {
    name: "Side step",
    description: "Bonus defense",
    icon: IconId::Sidestep,
    action_point_cost: 1,
    stamina_cost: 1,
    effect: OnAttackedReactionEffect::SideStep,
};

pub const RAGE: OnHitReaction = OnHitReaction {
    name: "Rage",
    description: "Bonus advantange on your next attack",
    icon: IconId::Rage,
    action_point_cost: 1,
    effect: OnHitReactionEffect::Rage,
};

pub const BRACE: SelfEffectAction = SelfEffectAction {
    name: "Brace",
    description: "Bonus defense the next time you're attacked",
    icon: IconId::Brace,
    action_point_cost: 1,
    effect: ApplyEffect::Condition(Condition::Braced),
};

pub const SCREAM: Spell = Spell {
    name: "Scream",
    description: "Daze the enemy",
    icon: IconId::Scream,
    action_point_cost: 2,
    mana_cost: 1,
    damage: 0,
    on_hit_effect: Some(ApplyEffect::Condition(Condition::Dazed(1))),
    spell_type: SpellType::Mental,
    possible_enhancement: Some(SpellEnhancement {
        name: "Shriek",
        description: "Target loses 2 AP",
        icon: IconId::Banshee,
        mana_cost: 1,
        effect: SpellEnhancementEffect::OnHitEffect(ApplyEffect::RemoveActionPoints(2)),
    }),
    range: Range::Ranged(4),
};

pub const MIND_BLAST: Spell = Spell {
    name: "Mind blast",
    description: "Damage and stagger the enemy",
    icon: IconId::Mindblast,
    action_point_cost: 2,
    mana_cost: 1,
    damage: 1,
    // TODO
    on_hit_effect: Some(ApplyEffect::RemoveActionPoints(99)),
    spell_type: SpellType::Mental,
    possible_enhancement: Some(SpellEnhancement {
        name: "Dualcast",
        description: "Spell is cast twice",
        icon: IconId::Dualcast,
        mana_cost: 1,
        effect: SpellEnhancementEffect::CastTwice,
    }),
    range: Range::Ranged(5),
};

pub const FIREBALL: Spell = Spell {
    name: "Fireball",
    description: "Hurl a fireball that damages the target",
    icon: IconId::Fireball,
    action_point_cost: 3,
    mana_cost: 1,
    damage: 2,
    on_hit_effect: None,
    spell_type: SpellType::Projectile,
    possible_enhancement: None,
    range: Range::Ranged(5),
};

pub const KILL: Spell = Spell {
    name: "Kill",
    description: "Kill the enemy",
    icon: IconId::Fireball,
    action_point_cost: 5,
    mana_cost: 0,
    damage: 99,
    on_hit_effect: None,
    spell_type: SpellType::Projectile,
    possible_enhancement: None,
    range: Range::Ranged(99),
};
