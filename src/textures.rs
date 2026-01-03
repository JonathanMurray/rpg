use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    hash::Hash,
    rc::Rc,
    sync::{Mutex, OnceLock},
};

use macroquad::{
    color::WHITE,
    math::Rect,
    texture::{draw_texture_ex, load_texture, DrawTextureParams, FilterMode, Texture2D},
};

use crate::pathfind::CELLS_PER_ENTITY;

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug)]
pub enum SpriteId {
    PinkMan,
    AlsoWeirdOrangeMan,
    WeirdOrangeMan,
    Alice,
    Bob,
    Clara,
    Skeleton,
    Skeleton2,
    Ghoul,
    Ogre,
    Magi,
    Warhammer,
    Bow,
    Sword,
    Rapier,
    Dagger,
    Shield,
}

pub async fn load_all_sprites() -> HashMap<SpriteId, Texture2D> {
    load_sprites(vec![
        (SpriteId::PinkMan, "character.png"),
        (SpriteId::AlsoWeirdOrangeMan, "character2.png"),
        (SpriteId::WeirdOrangeMan, "character3.png"),
        (SpriteId::Alice, "alice.png"),
        (SpriteId::Bob, "bob.png"),
        (SpriteId::Clara, "clara.png"),
        (SpriteId::Skeleton, "skeleton.png"),
        (SpriteId::Skeleton2, "skeleton2.png"),
        (SpriteId::Ghoul, "ghoul.png"),
        (SpriteId::Ogre, "ogre.png"),
        (SpriteId::Magi, "magi.png"),
        (SpriteId::Warhammer, "warhammer.png"),
        (SpriteId::Bow, "bow.png"),
        (SpriteId::Sword, "sword.png"),
        (SpriteId::Rapier, "rapier.png"),
        (SpriteId::Dagger, "dagger.png"),
        (SpriteId::Shield, "shield.png"),
    ])
    .await
}

pub fn character_sprite_height(sprite_id: SpriteId) -> u32 {
    match sprite_id {
        SpriteId::Clara => 26,
        SpriteId::Bob => 28,
        SpriteId::Alice => 28,
        SpriteId::Skeleton => 26,
        SpriteId::Skeleton2 => 26,
        SpriteId::Ogre => 26,
        SpriteId::Magi => 25,
        SpriteId::Ghoul => 24,

        // TODO:
        SpriteId::PinkMan => 25,
        SpriteId::AlsoWeirdOrangeMan => 25,
        SpriteId::WeirdOrangeMan => 25,
        SpriteId::Warhammer => panic!(),
        SpriteId::Bow => panic!(),
        SpriteId::Sword => panic!(),
        SpriteId::Rapier => panic!(),
        SpriteId::Dagger => panic!(),
        SpriteId::Shield => panic!(),
    }
}

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug)]
pub enum StatusId {
    PlaceholderNegative,
    PlaceholderPositive,

    Burning,
    Protected,
    Dazed,
    Bleeding,
    Healing,
    Blinded,
    Exposed,
    Slowed,
    Hastened,
    Inspired,
    ArcaneSurge,
    ReaperApCooldown,
    Rage,
    NearDeath,
    Dead,
}

pub async fn load_all_status_textures() -> HashMap<StatusId, Texture2D> {
    load_and_init_textures(vec![
        (
            StatusId::PlaceholderNegative,
            "status_placeholder_negative.png",
        ),
        (
            StatusId::PlaceholderPositive,
            "status_placeholder_positive.png",
        ),
        (StatusId::Burning, "status_burning.png"),
        (StatusId::Protected, "status_protected.png"),
        (StatusId::Dazed, "status_dazed.png"),
        (StatusId::Bleeding, "status_bleeding.png"),
        (StatusId::Healing, "status_healing.png"),
        (StatusId::Blinded, "status_blinded.png"),
        (StatusId::Exposed, "status_exposed.png"),
        (StatusId::Slowed, "status_slowed.png"),
        (StatusId::Hastened, "status_hastened.png"),
        (StatusId::Inspired, "status_inspired.png"),
        (StatusId::ArcaneSurge, "status_arcane_surge.png"),
        (StatusId::ReaperApCooldown, "status_reaper_cooldown.png"),
        (StatusId::Rage, "status_rage.png"),
        (StatusId::NearDeath, "status_near_death.png"),
        (StatusId::Dead, "status_dead.png"),
    ])
    .await
}

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug)]
pub enum IconId {
    Fireball,
    SearingLight,
    MeleeAttack,
    RangedAttack,
    Block,
    Brace,
    Move,
    Scream,
    Mindblast,
    NecroticInfluence,
    Parry,
    Sidestep,
    Tackle,
    ShieldBash,
    Rage,
    CrushingStrike,
    CarefulAim,
    CripplingShot,
    TrueStrike,

    SpellAdvantage,
    Banshee,
    Dualcast,
    AllIn,
    Plus,
    PlusPlus,
    Go,
    EndTurn,
    X1point5,
    X2,
    X3,
    Extend,
    Radius,
    Precision,
    Equip,
    UseConsumable,
    ShackledMind,
    Haste,
    Smite,
    QuickStrike,
    SweepAttack,
    LungeAttack,
    Slashing,
    Stabbing,
    Feint,
    Heal,
    Inferno,
    Energize,
    Inspire,

    HardenedSkin,
    WeaponProficiency,
    ArcaneSurge,
    Reaper,
}

pub async fn load_all_icons() -> HashMap<IconId, Texture2D> {
    load_and_init_textures(vec![
        (IconId::Fireball, "fireball_icon.png"),
        (IconId::SearingLight, "searing_light_icon.png"),
        (IconId::MeleeAttack, "attack_icon.png"),
        (IconId::RangedAttack, "ranged_attack_icon.png"),
        (IconId::Block, "block_icon.png"),
        (IconId::Brace, "brace_icon.png"),
        (IconId::Move, "move_icon.png"),
        (IconId::Scream, "scream_icon.png"),
        (IconId::Mindblast, "mindblast_icon.png"),
        (IconId::NecroticInfluence, "necrotic_influence_icon.png"),
        (IconId::Go, "go_icon.png"),
        (IconId::EndTurn, "endturn_icon.png"),
        (IconId::Parry, "parry_icon.png"),
        (IconId::Sidestep, "sidestep_icon.png"),
        (IconId::Tackle, "shove_icon.png"),
        (IconId::ShieldBash, "shieldbash_icon.png"),
        (IconId::Rage, "rage_icon.png"),
        (IconId::CrushingStrike, "crushing_strike_icon.png"),
        (IconId::Banshee, "banshee_icon.png"),
        (IconId::Dualcast, "dualcast_icon.png"),
        (IconId::AllIn, "all_in_icon.png"),
        (IconId::CarefulAim, "careful_aim_icon.png"),
        (IconId::CripplingShot, "crippling_shot_icon.png"),
        (IconId::TrueStrike, "true_strike_icon.png"),
        (IconId::SpellAdvantage, "spell_adv_icon.png"),
        (IconId::Plus, "plus_icon.png"),
        (IconId::PlusPlus, "plus_plus_icon.png"),
        (IconId::Smite, "smite_icon.png"),
        (IconId::X1point5, "x1_5.png"),
        (IconId::X2, "x2.png"),
        (IconId::X3, "x3.png"),
        (IconId::Extend, "extend.png"),
        (IconId::Radius, "radius.png"),
        (IconId::Precision, "precision_icon.png"),
        (IconId::Equip, "equip.png"),
        (IconId::UseConsumable, "use_consumable_icon.png"),
        (IconId::ShackledMind, "shackled_mind.png"),
        (IconId::Haste, "haste_icon.png"),
        (IconId::QuickStrike, "quick_strike_icon.png"),
        (IconId::Heal, "heal_icon.png"),
        (IconId::SweepAttack, "sweep_attack_icon.png"),
        (IconId::LungeAttack, "lunge_attack_icon.png"),
        (IconId::Slashing, "slashing_icon.png"),
        (IconId::Stabbing, "stabbing_icon.png"),
        (IconId::Feint, "deceptive_icon.png"),
        (IconId::Inferno, "inferno_icon.png"),
        (IconId::Energize, "energize_icon.png"),
        (IconId::HardenedSkin, "hardened_skin_icon.png"),
        (IconId::WeaponProficiency, "weapon_proficiency_icon.png"),
        (IconId::ArcaneSurge, "arcane_surge_icon.png"),
        (IconId::Reaper, "reaper_icon.png"),
        (IconId::Inspire, "inspire_icon.png"),
    ])
    .await
}

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug)]
pub enum PortraitId {
    Alice,
    Bob,
    Portrait3,
    Skeleton,
    Magi,
    Ghoul,
    Ogre,
}

pub async fn load_all_portraits() -> HashMap<PortraitId, Texture2D> {
    load_and_init_textures(vec![
        (PortraitId::Alice, "portrait_1.png"),
        (PortraitId::Bob, "portrait_2.png"),
        (PortraitId::Portrait3, "portrait_3.png"),
        (PortraitId::Skeleton, "portrait_skeleton.png"),
        (PortraitId::Magi, "portrait_magi.png"),
        (PortraitId::Ghoul, "portrait_ghoul.png"),
        (PortraitId::Ogre, "portrait_ogre.png"),
    ])
    .await
}

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug)]
pub enum EquipmentIconId {
    Undefined,
    Rapier,
    Warhammer,
    Bow,
    Dagger,
    Sword,
    SmallShield,
    MediumShield,
    LeatherArmor,
    ChainMail,
    Shirt,
    Robe,
    PenetratingArrow,
    BarbedArrow,
    ColdArrow,
    ExplodingArrow,

    HealthPotion,
    ManaPotion,
    AdrenalinPotion,
    EnergyPotion,
    ArcanePotion,

    PlaceholderOffhand,
    PlaceholderMainhand,
    PlaceholderArmor,
    PlaceholderArrows,
}

pub async fn load_all_equipment_icons() -> HashMap<EquipmentIconId, Texture2D> {
    load_and_init_textures(vec![
        (EquipmentIconId::Rapier, "eq_rapier.png"),
        (EquipmentIconId::Warhammer, "eq_warhammer.png"),
        (EquipmentIconId::Bow, "eq_bow.png"),
        (EquipmentIconId::Dagger, "eq_dagger.png"),
        (EquipmentIconId::Sword, "eq_sword.png"),
        (EquipmentIconId::SmallShield, "eq_small_shield.png"),
        (EquipmentIconId::MediumShield, "eq_medium_shield.png"),
        (EquipmentIconId::LeatherArmor, "eq_leather_armor.png"),
        (EquipmentIconId::ChainMail, "eq_chain_mail.png"),
        (EquipmentIconId::Shirt, "eq_shirt.png"),
        (EquipmentIconId::Robe, "eq_robe.png"),
        (
            EquipmentIconId::PenetratingArrow,
            "eq_penetrating_arrow.png",
        ),
        (EquipmentIconId::BarbedArrow, "eq_barbed_arrow.png"),
        (EquipmentIconId::ColdArrow, "eq_cold_arrow.png"),
        (EquipmentIconId::ExplodingArrow, "eq_exploding_arrow.png"),
        (EquipmentIconId::HealthPotion, "eq_health_potion.png"),
        (EquipmentIconId::ManaPotion, "eq_mana_potion.png"),
        (EquipmentIconId::AdrenalinPotion, "eq_adrenaline_potion.png"),
        (EquipmentIconId::EnergyPotion, "eq_energy_potion.png"),
        (EquipmentIconId::ArcanePotion, "eq_arcane_potion.png"),
        (
            EquipmentIconId::PlaceholderOffhand,
            "eq_placeholder_offhand.png",
        ),
        (
            EquipmentIconId::PlaceholderMainhand,
            "eq_placeholder_mainhand.png",
        ),
        (
            EquipmentIconId::PlaceholderArmor,
            "eq_placeholder_armor.png",
        ),
        (
            EquipmentIconId::PlaceholderArrows,
            "eq_placeholder_arrows.png",
        ),
    ])
    .await
}

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug)]
pub enum TerrainId {
    Grass,
    Grass2,
    Grass3,
    Grass4,

    Bush,
    Boulder2,
    TreeStump,
    Water,
    WaterBeachNorth,
    WaterBeachEast,
    WaterBeachSouth,
    WaterBeachWest,
    WaterBeachNorthEast,
    WaterBeachSouthEast,
    WaterBeachSouthWest,
    WaterBeachNorthWest,
    WaterBeachWestNorthEast,
    WaterBeachNorthEastSouth,
    WaterBeachEastSouthWest,
    WaterBeachSouthWestNorth,
    WaterBeachWestEast,
    WaterBeachNorthSouth,
}

pub fn draw_terrain(
    texture: &Texture2D,
    terrain_id: TerrainId,
    cell_w: f32,
    mut x: f32,
    mut y: f32,
) {
    let (w, h) = (32.0, 32.0);

    let src_margin = 2.0;
    let dst_margin = src_margin * cell_w / 32.0;
    let mut top_margin = false;
    let mut right_margin = false;
    let mut bot_margin = false;
    let mut left_margin = false;
    let (col, row) = match terrain_id {
        TerrainId::Grass => (0, 8),
        TerrainId::Grass2 => (1, 8),
        TerrainId::Grass3 => (2, 8),
        TerrainId::Grass4 => (3, 8),

        TerrainId::Bush => (0, 7),
        TerrainId::Boulder2 => (0, 6),
        TerrainId::TreeStump => (1, 6),

        TerrainId::Water => (2, 3),
        TerrainId::WaterBeachNorth => {
            top_margin = true;
            (2, 1)
        }
        TerrainId::WaterBeachEast => {
            right_margin = true;
            (4, 3)
        }
        TerrainId::WaterBeachSouth => {
            bot_margin = true;
            (2, 4)
        }
        TerrainId::WaterBeachWest => {
            left_margin = true;
            (1, 3)
        }

        TerrainId::WaterBeachNorthEast => (4, 1),
        TerrainId::WaterBeachSouthEast => (4, 4),
        TerrainId::WaterBeachSouthWest => (1, 4),
        TerrainId::WaterBeachNorthWest => (1, 1),

        TerrainId::WaterBeachNorthEastSouth => (5, 2),
        TerrainId::WaterBeachEastSouthWest => (3, 5),
        TerrainId::WaterBeachSouthWestNorth => (0, 2),
        TerrainId::WaterBeachWestNorthEast => (3, 0),

        TerrainId::WaterBeachWestEast => {
            left_margin = true;
            right_margin = true;
            (6, 1)
        }
        TerrainId::WaterBeachNorthSouth => {
            top_margin = true;
            bot_margin = true;
            (6, 3)
        }
    };

    let mut src_sides = [
        col as f32 * w,
        row as f32 * h,
        col as f32 * w + w,
        row as f32 * h + h,
    ];

    if top_margin {
        src_sides[1] -= src_margin;
        y -= dst_margin;
    }
    if right_margin {
        src_sides[2] += src_margin;
    }
    if bot_margin {
        src_sides[3] += src_margin;
    }
    if left_margin {
        src_sides[0] -= src_margin;
        x -= dst_margin;
    }
    let src_rect = Rect::new(
        src_sides[0],
        src_sides[1],
        src_sides[2] - src_sides[0],
        src_sides[3] - src_sides[1],
    );
    let src_rect_size = src_rect.size();
    let dst_size = (
        cell_w * src_rect_size.x / w * CELLS_PER_ENTITY as f32,
        cell_w * src_rect_size.y / h * CELLS_PER_ENTITY as f32,
    );

    let params = DrawTextureParams {
        source: Some(src_rect),
        dest_size: Some(dst_size.into()),
        ..Default::default()
    };

    draw_texture_ex(texture, x - cell_w, y - cell_w, WHITE, params);
}

async fn load_sprites(paths: Vec<(SpriteId, &str)>) -> HashMap<SpriteId, Texture2D> {
    let mut textures: HashMap<SpriteId, Texture2D> = Default::default();
    for (id, path) in paths {
        textures.insert(id, load_and_init_texture(path).await);
    }
    textures
}

pub async fn load_and_init_textures<T>(paths: Vec<(T, &str)>) -> HashMap<T, Texture2D>
where
    T: Hash + Eq,
{
    let mut textures: HashMap<T, Texture2D> = Default::default();
    for (id, path) in paths {
        textures.insert(id, load_and_init_texture(path).await);
    }
    textures
}

pub async fn load_and_init_texture(path: &str) -> Texture2D {
    let texture = load_texture(&format!("images/{}", path)).await.unwrap();
    texture.set_filter(FilterMode::Nearest);
    texture
}

pub async fn load_and_init_font_symbols() {
    let font_atlas = load_and_init_texture("font.png").await;
    let img = font_atlas.get_texture_data();

    let symbol = |x, y| {
        Texture2D::from_image(&img.sub_image(Rect::new(
            x as f32 * 16.0,
            y as f32 * 16.0,
            16.0,
            16.0,
        )))
    };

    DICE_SYMBOL.get_or_init(|| symbol(0, 0));
    SHIELD_SYMBOL.get_or_init(|| symbol(1, 0));
}

pub async fn load_and_init_user_interface_texture() {
    let texture = load_and_init_texture("user_interface.png").await;
    UI_TEXTURE.get_or_init(|| texture);
}

pub static DICE_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static SHIELD_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static UI_TEXTURE: OnceLock<Texture2D> = OnceLock::new();
