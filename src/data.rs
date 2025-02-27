use crate::core::{
    ApplyEffect, ArmorPiece, AttackAttribute, AttackEnhancement, AttackHitEffect, Condition,
    OnAttackedReaction, OnAttackedReactionEffect, OnHitReaction, OnHitReactionEffect, Shield,
    Spell, SpellEnhancement, SpellEnhancementEffect, SpellType, Weapon, WeaponGrip,
};

pub const LEATHER_ARMOR: ArmorPiece = ArmorPiece {
    protection: 3,
    limit_defense_from_dex: None,
};

pub const CHAIN_MAIL: ArmorPiece = ArmorPiece {
    protection: 5,
    limit_defense_from_dex: Some(4),
};

pub const DAGGER: Weapon = Weapon {
    name: "Dagger",
    action_point_cost: 1,
    damage: 1,
    grip: WeaponGrip::Light,
    attack_attribute: AttackAttribute::Finesse,
    attack_enhancement: None,
    on_attacked_reaction: None,
    on_true_hit: Some(AttackHitEffect::Apply(ApplyEffect::Condition(
        Condition::Weakened(1),
    ))),
};

pub const SWORD: Weapon = Weapon {
    name: "Sword",
    action_point_cost: 2,
    damage: 1,
    grip: WeaponGrip::Versatile,
    attack_attribute: AttackAttribute::Finesse,
    attack_enhancement: None,
    on_attacked_reaction: Some(PARRY),
    on_true_hit: Some(AttackHitEffect::Apply(ApplyEffect::Condition(
        Condition::Bleeding,
    ))),
};

pub const RAPIER: Weapon = Weapon {
    name: "Rapier",
    action_point_cost: 2,
    damage: 1,
    grip: WeaponGrip::MainHand,
    attack_attribute: AttackAttribute::Finesse,
    attack_enhancement: None,
    on_attacked_reaction: Some(PARRY),
    on_true_hit: Some(AttackHitEffect::SkipExertion),
};

pub const WAR_HAMMER: Weapon = Weapon {
    name: "War hammer",
    action_point_cost: 2,
    damage: 2,
    grip: WeaponGrip::TwoHanded,
    attack_attribute: AttackAttribute::Strength,
    attack_enhancement: Some(AttackEnhancement {
        name: "All-in attack",
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
};

pub const BOW: Weapon = Weapon {
    name: "Bow",
    action_point_cost: 2,
    damage: 2,
    grip: WeaponGrip::TwoHanded,
    attack_attribute: AttackAttribute::Dexterity,
    attack_enhancement: Some(AttackEnhancement {
        name: "Careful aim",
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
};

pub const SMALL_SHIELD: Shield = Shield {
    name: "Small shield",
    defense: 2,
    on_hit_reaction: Some(OnHitReaction {
        name: "Shield bash",
        action_point_cost: 1,
        effect: OnHitReactionEffect::ShieldBash,
    }),
};

pub const CRUSHING_STRIKE: AttackEnhancement = AttackEnhancement {
    name: "Crushing strike",
    action_point_cost: 0,
    stamina_cost: 1,
    bonus_damage: 0,
    apply_on_self_before: None,
    on_hit_effect: Some(ApplyEffect::RemoveActionPoints(1)),
};

pub const PARRY: OnAttackedReaction = OnAttackedReaction {
    name: "Parry",
    action_point_cost: 1,
    stamina_cost: 0,
    effect: OnAttackedReactionEffect::Parry,
};

pub const SIDE_STEP: OnAttackedReaction = OnAttackedReaction {
    name: "Side step",
    action_point_cost: 1,
    stamina_cost: 1,
    effect: OnAttackedReactionEffect::SideStep,
};

pub const RAGE: OnHitReaction = OnHitReaction {
    name: "Rage",
    action_point_cost: 1,
    effect: OnHitReactionEffect::Rage,
};

pub const SCREAM: Spell = Spell {
    name: "Scream",
    action_point_cost: 2,
    mana_cost: 1,
    damage: 0,
    on_hit_effect: Some(ApplyEffect::Condition(Condition::Dazed(1))),
    spell_type: SpellType::Mental,
    possible_enhancement: Some(SpellEnhancement {
        name: "Take action points",
        mana_cost: 1,
        effect: SpellEnhancementEffect::OnHitEffect(ApplyEffect::RemoveActionPoints(2)),
    }),
};

pub const MIND_BLAST: Spell = Spell {
    name: "Mind blast",
    action_point_cost: 2,
    mana_cost: 1,
    damage: 1,
    on_hit_effect: Some(ApplyEffect::RemoveActionPoints(1)),
    spell_type: SpellType::Mental,
    possible_enhancement: Some(SpellEnhancement {
        name: "Double cast",
        mana_cost: 1,
        effect: SpellEnhancementEffect::CastTwice,
    }),
};

pub const FIREBALL: Spell = Spell {
    name: "Fireball",
    action_point_cost: 3,
    mana_cost: 1,
    damage: 2,
    on_hit_effect: None,
    spell_type: SpellType::Projectile,
    possible_enhancement: None,
};
