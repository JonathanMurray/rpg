use std::{collections::HashMap, hash::Hash};

use macroquad::{
    color::WHITE,
    math::Rect,
    texture::{draw_texture_ex, load_texture, DrawTextureParams, FilterMode, Texture2D},
};

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug)]
pub enum SpriteId {
    Character,
    Character2,
    Character3,
    Character4,
    Character5,
    Skeleton,
    Warhammer,
    Bow,
    Sword,
    Rapier,
    Dagger,
    Shield,
}

pub async fn load_all_sprites() -> HashMap<SpriteId, Texture2D> {
    load_sprites(vec![
        (SpriteId::Character, "character.png"),
        (SpriteId::Character2, "character2.png"),
        (SpriteId::Character3, "character3.png"),
        (SpriteId::Character4, "character4.png"),
        (SpriteId::Character5, "character5.png"),
        (SpriteId::Skeleton, "skeleton.png"),
        (SpriteId::Warhammer, "warhammer.png"),
        (SpriteId::Bow, "bow.png"),
        (SpriteId::Sword, "sword.png"),
        (SpriteId::Rapier, "rapier.png"),
        (SpriteId::Dagger, "dagger.png"),
        (SpriteId::Shield, "shield.png"),
    ])
    .await
}

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug)]
pub enum IconId {
    Fireball,
    Attack,
    Brace,
    Move,
    Scream,
    Mindblast,
    Parry,
    Sidestep,
    ShieldBash,
    Rage,
    CrushingStrike,
    CarefulAim,
    SpellAdvantage,
    Banshee,
    Dualcast,
    AllIn,
    Plus,
    PlusPlus,
    Go,
    X1point5,
    X2,
    X3,
    Extend,
    Radius,
    Precision,
    Equip,
    ShackledMind,
    Smite,
    QuickStrike,
    SweepAttack,
    LungeAttack,
    Slashing,
    Stabbing,
    Deceptive,
    Heal,
    Inferno,
    Energize,
}

pub async fn load_all_icons() -> HashMap<IconId, Texture2D> {
    load_and_init_textures(vec![
        (IconId::Fireball, "fireball_icon.png"),
        (IconId::Attack, "attack_icon.png"),
        (IconId::Brace, "brace_icon.png"),
        (IconId::Move, "move_icon.png"),
        (IconId::Scream, "scream_icon.png"),
        (IconId::Mindblast, "mindblast_icon.png"),
        (IconId::Go, "go_icon.png"),
        (IconId::Parry, "parry_icon.png"),
        (IconId::Sidestep, "sidestep_icon.png"),
        (IconId::ShieldBash, "shieldbash_icon.png"),
        (IconId::Rage, "rage_icon.png"),
        (IconId::CrushingStrike, "crushing_strike_icon.png"),
        (IconId::Banshee, "banshee_icon.png"),
        (IconId::Dualcast, "dualcast_icon.png"),
        (IconId::AllIn, "all_in_icon.png"),
        (IconId::CarefulAim, "careful_aim_icon.png"),
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
        (IconId::ShackledMind, "shackled_mind.png"),
        (IconId::QuickStrike, "quick_strike_icon.png"),
        (IconId::Heal, "heal_icon.png"),
        (IconId::SweepAttack, "sweep_attack_icon.png"),
        (IconId::LungeAttack, "lunge_attack_icon.png"),
        (IconId::Slashing, "slashing_icon.png"),
        (IconId::Stabbing, "stabbing_icon.png"),
        (IconId::Deceptive, "deceptive_icon.png"),
        (IconId::Inferno, "inferno_icon.png"),
        (IconId::Energize, "energize_icon.png"),
    ])
    .await
}

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug)]
pub enum PortraitId {
    Portrait1,
    Portrait2,
    Portrait3,
    Skeleton,
}

pub async fn load_all_portraits() -> HashMap<PortraitId, Texture2D> {
    load_and_init_textures(vec![
        (PortraitId::Portrait1, "portrait_1.png"),
        (PortraitId::Portrait2, "portrait_2.png"),
        (PortraitId::Portrait3, "portrait_3.png"),
        (PortraitId::Skeleton, "portrait_skeleton.png"),
    ])
    .await
}

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug)]
pub enum EquipmentIconId {
    Rapier,
    Warhammer,
    Bow,
    Dagger,
    Sword,
    SmallShield,
    LeatherArmor,
    ChainMail,
    Shirt,
}

pub async fn load_all_equipment_icons() -> HashMap<EquipmentIconId, Texture2D> {
    load_and_init_textures(vec![
        (EquipmentIconId::Rapier, "eq_rapier.png"),
        (EquipmentIconId::Warhammer, "eq_warhammer.png"),
        (EquipmentIconId::Bow, "eq_bow.png"),
        (EquipmentIconId::Dagger, "eq_dagger.png"),
        (EquipmentIconId::Sword, "eq_sword.png"),
        (EquipmentIconId::SmallShield, "eq_small_shield.png"),
        (EquipmentIconId::LeatherArmor, "eq_leather_armor.png"),
        (EquipmentIconId::ChainMail, "eq_chain_mail.png"),
        (EquipmentIconId::Shirt, "eq_shirt.png"),
    ])
    .await
}

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug)]
pub enum TerrainId {
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
}

pub fn draw_terrain(texture: &Texture2D, terrain_id: TerrainId, cell_w: f32, x: f32, y: f32) {
    let (w, h) = (32.0, 32.0);

    let (col, row, overflow) = match terrain_id {
        TerrainId::Bush => (0, 6, false),
        TerrainId::Boulder2 => (1, 5, false),
        TerrainId::TreeStump => (1, 6, false),

        TerrainId::Water => (2, 3, true),
        TerrainId::WaterBeachNorth => (2, 1, true),
        TerrainId::WaterBeachEast => (4, 3, true),
        TerrainId::WaterBeachSouth => (2, 4, true),
        TerrainId::WaterBeachWest => (1, 3, true),

        TerrainId::WaterBeachNorthEast => (4, 1, true),
        TerrainId::WaterBeachSouthEast => (4, 4, true),
        TerrainId::WaterBeachSouthWest => (1, 4, true),
        TerrainId::WaterBeachNorthWest => (1, 1, true),

        TerrainId::WaterBeachNorthEastSouth => (5, 2, true),
        TerrainId::WaterBeachEastSouthWest => (3, 5, true),
        TerrainId::WaterBeachSouthWestNorth => (0, 2, true),
        TerrainId::WaterBeachWestNorthEast => (3, 0, true),
    };

    let source_rect = if overflow {
        let margin = 2.0;
        Rect::new(
            col as f32 * w - margin,
            row as f32 * h - margin,
            w + 2.0 * margin,
            h + 2.0 * margin,
        )
    } else {
        Rect::new(col as f32 * w, row as f32 * h, w, h)
    };

    let size = source_rect.size();

    let dest_size = (cell_w * size.x / 32.0, cell_w * size.y / 32.0);

    let offset = ((cell_w - dest_size.0) / 2.0, (cell_w - dest_size.1) / 2.0);

    let params = DrawTextureParams {
        dest_size: Some(dest_size.into()),

        source: Some(source_rect),

        ..Default::default()
    };

    draw_texture_ex(texture, x + offset.0, y + offset.1, WHITE, params);
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
