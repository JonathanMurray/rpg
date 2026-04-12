use std::{collections::HashMap, f32::consts::PI, hash::Hash, sync::OnceLock};

use macroquad::{
    color::WHITE,
    math::Rect,
    texture::{draw_texture_ex, load_texture, DrawTextureParams, FilterMode, Texture2D},
    time::get_time,
};
use serde::{Deserialize, Serialize};

use crate::pathfind::{Liquid, TerrainType, CELLS_PER_ENTITY};

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug)]
pub enum SpriteId {
    PinkMan,
    AlsoWeirdOrangeMan,
    WeirdOrangeMan,
    Alice,
    Bob,
    Clara,
    Huldra,
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
    CharacterShadow,
}

#[derive(Clone)]
pub struct Sprite {
    pub regular: Texture2D,
    pub white_highlight: Texture2D,
    pub red_highlight: Texture2D,
}

pub async fn load_all_sprites() -> HashMap<SpriteId, Sprite> {
    let sprites = load_sprites(vec![
        (SpriteId::PinkMan, "character.png"),
        (SpriteId::AlsoWeirdOrangeMan, "character2.png"),
        (SpriteId::WeirdOrangeMan, "character3.png"),
        (SpriteId::Alice, "alice.png"),
        (SpriteId::Bob, "bob.png"),
        (SpriteId::Clara, "clara.png"),
        (SpriteId::Huldra, "huldra.png"),
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
        (SpriteId::CharacterShadow, "character_shadow.png"),
    ])
    .await;

    sprites
        .into_iter()
        .map(|(id, texture)| {
            let highlight_color = [201, 226, 118, 255];

            let white = Texture2D::from_image(&texture.get_texture_data());
            white.set_filter(FilterMode::Nearest);
            replace_color(&white, highlight_color, [255, 255, 255, 255]);

            let red = Texture2D::from_image(&texture.get_texture_data());
            red.set_filter(FilterMode::Nearest);
            replace_color(&red, highlight_color, [255, 100, 100, 255]);

            (
                id,
                Sprite {
                    regular: texture,
                    white_highlight: white,
                    red_highlight: red,
                },
            )
        })
        .collect()
}

pub fn character_sprite_height(sprite_id: SpriteId) -> u32 {
    match sprite_id {
        SpriteId::Clara => 26,
        SpriteId::Bob => 28,
        SpriteId::Alice => 28,
        SpriteId::Huldra => 26,
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
        SpriteId::CharacterShadow => panic!(),
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
    Hindered,
    Exposed,
    Slowed,
    Hastened,
    Inspired,
    CriticalCharge,
    ReaperApCooldown,
    Rage,
    NearDeath,
    Dead,
    Wet,
    Poisoned,
}

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug)]
pub enum IconId {
    Fireball,
    SearingLight,
    MeleeAttack,
    RangedAttack,
    PiercingShot,
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
    QuickActions,
    Go,
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
    LightningBolt,

    HardenedSkin,
    WeaponProficiency,
    CriticalCharge,
    Reaper,
}

enum GraphicsFxId {
    LightningBolt,
}

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug)]
pub enum EffectId {
    Pow,
}

pub async fn load_all_effects() -> HashMap<EffectId, Texture2D> {
    load_and_init_textures(vec![(EffectId::Pow, "effects.png")]).await
}

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug)]
pub enum PortraitId {
    Alice,
    Bob,
    Clara,
    Skeleton,
    Magi,
    Huldra,
    Ghoul,
    Ogre,
}

pub async fn load_all_portraits() -> HashMap<PortraitId, Texture2D> {
    load_and_init_textures(vec![
        (PortraitId::Alice, "portrait_alice.png"),
        (PortraitId::Bob, "portrait_bob.png"),
        (PortraitId::Clara, "portrait_clara.png"),
        (PortraitId::Skeleton, "portrait_skeleton.png"),
        (PortraitId::Magi, "portrait_magi.png"),
        (PortraitId::Huldra, "portrait_huldra.png"),
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

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug, Serialize, Deserialize)]
pub enum WaterOrientation {
    /*
       +XX
       XXX
    */
    NorthWestInverted,

    /*
       XX+
       XXX
    */
    NorthEastInverted,

    /*
       XXX
       +XX
    */
    SouthWestInverted,

    /*
       XXX
       XX+
    */
    SouthEastInverted,

    /*
       +----
       |XXX
       |XXX
    */
    NorthWest,

    /*
       ---
       XXX
       XXX
    */
    North,

    /*
        ---+
        XXX|
        XXX|
    */
    NorthEast,

    /*
       |XXX
       |XXX
    */
    West,

    /*
       XXX
       XXX
    */
    Center,

    /*
       XXX|
       XXX|
    */
    East,

    /*
       |XXX
       |XXX
       +---
    */
    SouthWest,

    /*
       XXX
       XXX
       ---
    */
    South,

    /*
       XXX|
       XXX|
       ---+
    */
    SouthEast,

    /*
       +---
       |XXX
       |XXX
       +---
    */
    ThinWest,

    /*
       ---+
       XXX|
       XXX|
       ---+
    */
    ThinEast,

    /*
       +---+
       |XXX|
       |XXX|

    */
    ThinNorth,

    /*

       |XXX|
       |XXX|
       +---+
    */
    ThinSouth,

    /*
       ---
       XXX
       XXX
       ---
    */
    ThinHor,

    /*
       |XXX|
       |XXX|
    */
    ThinVert,
}

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug, Serialize, Deserialize)]
pub enum WaterType {
    Water,
    Poison,
}

impl From<Liquid> for WaterType {
    fn from(liquid: Liquid) -> Self {
        match liquid {
            Liquid::Water => Self::Water,
            Liquid::Poison => Self::Poison,
        }
    }
}

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug, Serialize, Deserialize)]
pub enum TerrainId {
    Grass,
    Grass2,
    Grass3,
    Grass4,
    Floor,
    Floor2,
    Floor3,
    Floor4,

    StoneWall,
    StoneWallConcaveNorthWest,
    StoneWallNorth,
    StoneWallConcaveNorthEast,
    StoneWallWest,
    StoneWallEast,
    StoneWallConcaveSouthWest,
    StoneWallSouth,
    StoneWallConcaveSouthEast,
    StoneWallConvexNorthWest,
    StoneWallConvexNorthEast,
    StoneWallConvexSouthWest,
    StoneWallConvexSouthEast,
    StoneWallInner,
    Bush,
    Boulder2,
    TreeStump,
    Table,

    BookShelf,
    WallPainting,
    WallPainting2,
    WallFlag,
    Mat,
    Cauldron,
    Cauldron2,
    WallOpeningNorth,
    WallOpeningEast,
    WallOpeningWest,
    WallWindow,
    SuitOfArmor,
    AnimalHead,

    NewWater(WaterOrientation, WaterType),

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

impl TerrainId {
    pub fn is_new_water(&self) -> bool {
        match self {
            TerrainId::NewWater(..) => true,
            _ => false,
        }
    }

    pub fn is_stone_wall(&self) -> bool {
        match self {
            TerrainId::StoneWall => true,
            TerrainId::StoneWallConcaveNorthWest => true,
            TerrainId::StoneWallNorth => true,
            TerrainId::StoneWallConcaveNorthEast => true,
            TerrainId::StoneWallWest => true,
            TerrainId::StoneWallEast => true,
            TerrainId::StoneWallConcaveSouthWest => true,
            TerrainId::StoneWallSouth => true,
            TerrainId::StoneWallConcaveSouthEast => true,
            TerrainId::StoneWallConvexNorthWest => true,
            TerrainId::StoneWallConvexNorthEast => true,
            TerrainId::StoneWallConvexSouthWest => true,
            TerrainId::StoneWallConvexSouthEast => true,
            TerrainId::StoneWallInner => true,
            _ => false,
        }
    }

    pub fn terrain_type(&self) -> TerrainType {
        if self.is_stone_wall() {
            TerrainType::Tall
        } else {
            TerrainType::Low
        }
    }
}

pub fn draw_terrain(texture: &Texture2D, terrain_id: TerrainId, cell_w: f32, x: f32, y: f32) {
    let (rotation, src_rect) = terrain_atlas_area(terrain_id);
    let (w, h) = (32.0, 32.0);
    let src_rect_size = src_rect.size();
    let dst_size = (
        cell_w * src_rect_size.x / w * CELLS_PER_ENTITY as f32,
        cell_w * src_rect_size.y / h * CELLS_PER_ENTITY as f32,
    );

    let params = DrawTextureParams {
        source: Some(src_rect),
        dest_size: Some(dst_size.into()),
        rotation,
        ..Default::default()
    };

    draw_texture_ex(
        texture,
        (x - cell_w).floor(),
        (y - cell_w).floor(),
        WHITE,
        params,
    );
}

pub fn terrain_atlas_area(terrain_id: TerrainId) -> (f32, Rect) {
    let (w, h) = (32.0, 32.0);

    let src_margin = 2.0;
    //let dst_margin = src_margin * cell_w / 32.0;
    let mut top_margin = false;
    let mut right_margin = false;
    let mut bot_margin = false;
    let mut left_margin = false;
    let mut rotation = 0.0;
    let mut is_poison = false;
    let (mut col, mut row) = match terrain_id {
        TerrainId::Grass => (0, 8),
        TerrainId::Grass2 => (1, 8),
        TerrainId::Grass3 => (2, 8),
        TerrainId::Grass4 => (3, 8),

        TerrainId::Floor => (0, 9),
        TerrainId::Floor2 => (0, 10),
        TerrainId::Floor3 => (0, 11),
        TerrainId::Floor4 => (1, 9),
        TerrainId::NewWater(orientation, type_) => {
            is_poison = type_ == WaterType::Poison;
            match orientation {
                WaterOrientation::NorthWestInverted => (9, 9),
                WaterOrientation::NorthEastInverted => {
                    rotation = 0.5 * PI;
                    (9, 9)
                }
                WaterOrientation::SouthWestInverted => {
                    rotation = 1.5 * PI;
                    (9, 9)
                }
                WaterOrientation::SouthEastInverted => {
                    rotation = 1.0 * PI;
                    (9, 9)
                }
                WaterOrientation::NorthWest => (6, 9),
                WaterOrientation::North => {
                    rotation = 0.5 * PI;
                    (6, 10)
                }
                WaterOrientation::NorthEast => {
                    rotation = 0.5 * PI;
                    (6, 9)
                }
                WaterOrientation::West => (6, 10),
                WaterOrientation::Center => (7, 10),
                WaterOrientation::East => {
                    rotation = PI;
                    (6, 10)
                }
                WaterOrientation::SouthWest => {
                    rotation = 1.5 * PI;
                    (6, 9)
                }
                WaterOrientation::South => {
                    rotation = 1.5 * PI;
                    (6, 10)
                }
                WaterOrientation::SouthEast => {
                    rotation = PI;
                    (6, 9)
                }
                WaterOrientation::ThinWest => (7, 11),
                WaterOrientation::ThinEast => {
                    rotation = PI;
                    (7, 11)
                }
                WaterOrientation::ThinNorth => {
                    rotation = 0.5 * PI;
                    (7, 11)
                }
                WaterOrientation::ThinSouth => {
                    rotation = 1.5 * PI;
                    (7, 11)
                }
                WaterOrientation::ThinHor => (8, 9),
                WaterOrientation::ThinVert => {
                    rotation = 0.5 * PI;
                    (8, 9)
                }
            }
        }

        TerrainId::StoneWall => (1, 7),
        TerrainId::StoneWallConcaveNorthWest => (0, 12),
        TerrainId::StoneWallNorth => (1, 12),
        TerrainId::StoneWallConcaveNorthEast => (2, 12),
        TerrainId::StoneWallWest => (0, 13),
        TerrainId::StoneWallEast => (2, 13),
        TerrainId::StoneWallConcaveSouthWest => (0, 14),
        TerrainId::StoneWallSouth => (1, 14),
        TerrainId::StoneWallConcaveSouthEast => (2, 14),
        TerrainId::StoneWallConvexNorthWest => (3, 12),
        TerrainId::StoneWallConvexNorthEast => (5, 12),
        TerrainId::StoneWallConvexSouthWest => (3, 14),
        TerrainId::StoneWallConvexSouthEast => (5, 14),
        TerrainId::StoneWallInner => (4, 13),

        TerrainId::Bush => (0, 7),
        TerrainId::Boulder2 => (0, 6),
        TerrainId::TreeStump => (1, 6),
        TerrainId::Table => (2, 7),

        TerrainId::BookShelf => (2, 6),
        TerrainId::WallPainting => (3, 6),
        TerrainId::WallPainting2 => (5, 5),
        TerrainId::WallFlag => (4, 6),
        TerrainId::WallWindow => (7, 6),
        TerrainId::Mat => (4, 8),
        TerrainId::Cauldron => (5, 8),
        TerrainId::Cauldron2 => (6, 8),
        TerrainId::WallOpeningNorth => (5, 6),
        TerrainId::WallOpeningEast => (6, 6),
        TerrainId::WallOpeningWest => (6, 7),
        TerrainId::SuitOfArmor => (4, 7),
        TerrainId::AnimalHead => (5, 7),

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

    let t = get_time();
    // animate water
    if ((t * 0.5) % (t * 0.5).floor()) < 0.5 {
        if (col, row) == (6, 10) {
            (col, row) = (6, 11);
        } else if (col, row) == (6, 9) {
            (col, row) = (7, 9);
        } else if (col, row) == (7, 11) {
            (col, row) = (8, 11);
        } else if (col, row) == (8, 9) {
            (col, row) = (8, 10);
        } else if (col, row) == (9, 9) {
            (col, row) = (9, 10);
        }
    }

    if is_poison {
        col -= 4;
    }

    // animate table candle
    if (t * 0.7) % (t * 0.7).floor() < 0.5 && (col, row) == (2, 7) {
        (col, row) = (3, 7);
    }

    // cauldron bubbles
    if (t * 0.6) % (t * 0.6).floor() < 0.5 && (col, row) == (6, 8) {
        (col, row) = (7, 8);
    }

    let src_sides = [
        col as f32 * w,
        row as f32 * h,
        col as f32 * w + w,
        row as f32 * h + h,
    ];

    /*
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
     */
    let src_rect = Rect::new(
        src_sides[0],
        src_sides[1],
        src_sides[2] - src_sides[0],
        src_sides[3] - src_sides[1],
    );
    (rotation, src_rect)
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

async fn load_and_init_tiny_font() {
    let texture = load_and_init_texture("tiny_font.png").await;
    replace_color(&texture, [0, 0, 0, 255], [100, 200, 100, 255]);
    TINY_FONT_GREEN_TEXTURE.get_or_init(|| texture);

    let texture = load_and_init_texture("tiny_font.png").await;
    replace_color(&texture, [0, 0, 0, 255], [255, 100, 100, 255]);
    TINY_FONT_RED_TEXTURE.get_or_init(|| texture);
}

fn replace_color(texture: &Texture2D, from: [u8; 4], to: [u8; 4]) {
    let mut img = texture.get_texture_data();
    for pixel in img.get_image_data_mut() {
        if pixel == &from {
            *pixel = to;
        }
    }
    texture.update(&img);
}

pub enum TinyFontColor {
    Green,
    Red,
}

const TINY_FONT_W: f32 = 6.0;
const TINY_FONT_H: f32 = 7.0;

pub fn measure_tiny_font(text: &str) -> (f32, f32) {
    (text.len() as f32 * TINY_FONT_W, TINY_FONT_H)
}

pub fn draw_tiny_font(text: &str, x: f32, y: f32, color: TinyFontColor) {
    let texture = match color {
        TinyFontColor::Green => TINY_FONT_GREEN_TEXTURE.get().unwrap(),
        TinyFontColor::Red => TINY_FONT_RED_TEXTURE.get().unwrap(),
    };

    let x = x.floor();
    let y = y.floor();
    let mut x0 = x;
    let ch_w = TINY_FONT_W;
    let ch_h = TINY_FONT_H;
    let vert_i = 2;
    for ch in text.chars() {
        let hor_i = if ch.is_ascii_digit() {
            ch as u8 - b'0'
        } else if ch == '%' {
            10
        } else {
            11
        };
        draw_texture_ex(
            texture,
            x0,
            y - ch_h,
            WHITE,
            DrawTextureParams {
                source: Some(Rect::new(
                    hor_i as f32 * ch_w,
                    vert_i as f32 * ch_h,
                    ch_w,
                    ch_h,
                )),
                ..Default::default()
            },
        );
        x0 += ch_w;
    }
}

pub async fn load_and_init_static() {
    load_and_init_tiny_font().await;
    load_and_init_font_symbols().await;
    load_and_init_ui_textures().await;

    let status_icon_atlas = load_and_init_texture("status.png").await;
    STATUS_ICONS_TEXTURE.get_or_init(|| status_icon_atlas);

    let icon_atlas = load_and_init_texture("icon.png").await;
    ICONS_TEXTURE.get_or_init(|| icon_atlas);

    let lightning_bolt_fx = load_and_init_texture("fx_lightning_bolt.png").await;
    LIGHTNING_BOLT_FX.get_or_init(|| lightning_bolt_fx);
}

pub fn draw_icon(icon: IconId, x: f32, y: f32, dest_size: Option<(f32, f32)>) {
    let x = x.floor();
    let y = y.floor();
    let texture = ICONS_TEXTURE.get().unwrap();
    let (col, row): (i32, i32) = match icon {
        IconId::Fireball => (0, 1),
        IconId::SearingLight => (2, 1),
        IconId::MeleeAttack => (6, 8),
        IconId::RangedAttack => (6, 7),
        IconId::PiercingShot => (7, 7),
        IconId::Block => (8, 7),
        IconId::Brace => (5, 2),
        IconId::LightningBolt => (6, 2),
        IconId::Move => (0, 4),
        IconId::Scream => (6, 1),
        IconId::Mindblast => (7, 1),
        IconId::NecroticInfluence => (1, 1),
        IconId::Parry => (9, 7),
        IconId::Sidestep => (2, 7),
        IconId::Tackle => (3, 7),
        IconId::ShieldBash => (5, 7),
        IconId::Rage => (2, 6),
        IconId::CrushingStrike => (4, 7),
        IconId::CarefulAim => (8, 8),
        IconId::CripplingShot => (7, 8),
        IconId::TrueStrike => (5, 6),
        IconId::SpellAdvantage => (3, 1),
        IconId::Banshee => (0, 2),
        IconId::Dualcast => (1, 2),
        IconId::AllIn => (4, 7),
        IconId::Plus => (4, 9),
        IconId::PlusPlus => (5, 9),
        IconId::QuickActions => (8, 9),
        IconId::Go => (3, 4),
        IconId::Extend => (0, 6),
        IconId::Radius => (3, 6),
        IconId::Precision => (4, 6),
        IconId::Equip => (1, 4),
        IconId::UseConsumable => (2, 4),
        IconId::ShackledMind => (9, 1),
        IconId::Haste => (2, 2),
        IconId::Smite => (2, 8),
        IconId::QuickStrike => (3, 8),
        IconId::SweepAttack => (4, 8),
        IconId::LungeAttack => (5, 8),
        IconId::Slashing => (0, 8),
        IconId::Stabbing => (1, 8),
        IconId::Feint => (0, 7),
        IconId::Heal => (6, 1),
        IconId::Inferno => (3, 2),
        IconId::Energize => (4, 1),
        IconId::Inspire => (4, 2),
        IconId::HardenedSkin => (3, 0),
        IconId::WeaponProficiency => (2, 0),
        IconId::CriticalCharge => (1, 0),
        IconId::Reaper => (0, 0),
    };
    let icon_w = 30.0;
    let icon_h = 24.0;
    let dest_size = dest_size
        .map(|s| s.into())
        .unwrap_or((icon_w, icon_h).into());
    draw_texture_ex(
        texture,
        x,
        y,
        WHITE,
        DrawTextureParams {
            dest_size: Some(dest_size),
            source: Some(Rect::new(
                col as f32 * icon_w,
                row as f32 * icon_h,
                icon_w,
                icon_h,
            )),
            ..Default::default()
        },
    );
}

pub fn draw_status_icon(status: StatusId, x: f32, y: f32, dest_size: Option<(f32, f32)>) {
    let x = x.floor();
    let y = y.floor();
    let texture = STATUS_ICONS_TEXTURE.get().unwrap();
    let (col, row) = match status {
        StatusId::PlaceholderNegative => (0, 0),
        StatusId::PlaceholderPositive => (1, 0),
        StatusId::Burning => (0, 2),
        StatusId::Protected => (3, 0),
        StatusId::Dazed => (1, 2),
        StatusId::Bleeding => (2, 2),
        StatusId::Healing => (3, 2),
        StatusId::Blinded => (0, 3),
        StatusId::Hindered => (1, 3),
        StatusId::Exposed => (2, 0),
        StatusId::Slowed => (4, 0),
        StatusId::Hastened => (0, 1),
        StatusId::Inspired => (1, 1),
        StatusId::CriticalCharge => (3, 1),
        StatusId::ReaperApCooldown => (4, 1),
        StatusId::Rage => (2, 1),
        StatusId::NearDeath => (4, 2),
        StatusId::Dead => (2, 3),
        StatusId::Wet => (3, 3),
        StatusId::Poisoned => (4, 3),
    };
    let icon_w = 10.0;
    let dest_size = dest_size
        .map(|s| s.into())
        .unwrap_or((icon_w, icon_w).into());
    draw_texture_ex(
        texture,
        x,
        y,
        WHITE,
        DrawTextureParams {
            dest_size: Some(dest_size),
            source: Some(Rect::new(
                col as f32 * icon_w,
                row as f32 * icon_w,
                icon_w,
                icon_w,
            )),
            ..Default::default()
        },
    );
}

async fn load_and_init_font_symbols() {
    let symbol_atlas = load_and_init_texture("font.png").await;
    let img = symbol_atlas.get_texture_data();

    let symbol = |x, y| {
        let texture = Texture2D::from_image(&img.sub_image(Rect::new(
            x as f32 * 16.0,
            y as f32 * 16.0,
            16.0,
            16.0,
        )));

        texture.set_filter(FilterMode::Nearest);
        texture
    };

    UNCHECKED_SYMBOL.get_or_init(|| symbol(4, 0));
    CHECKED_SYMBOL.get_or_init(|| symbol(5, 0));
    BLUE_DICE_SYMBOL.get_or_init(|| symbol(0, 0));
    RED_DICE_SYMBOL.get_or_init(|| symbol(3, 2));
    MIXED_DICE_SYMBOL.get_or_init(|| symbol(5, 2));
    SHIELD_SYMBOL.get_or_init(|| symbol(1, 0));
    ALT_KEY_SYMBOL.get_or_init(|| symbol(2, 0));
    WARNING_SYMBOL.get_or_init(|| symbol(3, 0));
    INFO_SYMBOL.get_or_init(|| symbol(4, 1));
    HEART_SYMBOL.get_or_init(|| symbol(0, 1));
    STAMINA_SYMBOL.get_or_init(|| symbol(1, 1));
    STAMINA_SMALL_SYMBOL.get_or_init(|| symbol(1, 3));
    MANA_SYMBOL.get_or_init(|| symbol(2, 1));
    MANA_SMALL_SYMBOL.get_or_init(|| symbol(0, 3));
    SWORD_SYMBOL.get_or_init(|| symbol(0, 2));
    BOOT_SYMBOL.get_or_init(|| symbol(1, 2));
    HELMET_SYMBOL.get_or_init(|| symbol(2, 3));
    WEIGHT_SYMBOL.get_or_init(|| symbol(3, 3));
    CONFIRM_SYMBOL.get_or_init(|| symbol(3, 1));
}

async fn load_and_init_ui_textures() {
    let texture = load_and_init_texture("user_interface.png").await;
    UI_TEXTURE.get_or_init(|| texture);

    let texture = load_and_init_texture("portrait_bg.png").await;
    PORTRAIT_BG_TEXTURE.get_or_init(|| texture);

    let texture = load_and_init_texture("portrait_enemy_bg.png").await;
    PORTRAIT_ENEMY_BG_TEXTURE.get_or_init(|| texture);
}

pub static UNCHECKED_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static CHECKED_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static BLUE_DICE_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static RED_DICE_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static MIXED_DICE_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static SHIELD_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static ALT_KEY_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static WARNING_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static INFO_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static HEART_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static STAMINA_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static STAMINA_SMALL_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static MANA_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static MANA_SMALL_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static SWORD_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static BOOT_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static HELMET_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static WEIGHT_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static CONFIRM_SYMBOL: OnceLock<Texture2D> = OnceLock::new();

pub static TINY_FONT_GREEN_TEXTURE: OnceLock<Texture2D> = OnceLock::new();
pub static TINY_FONT_RED_TEXTURE: OnceLock<Texture2D> = OnceLock::new();

pub static STATUS_ICONS_TEXTURE: OnceLock<Texture2D> = OnceLock::new();
pub static ICONS_TEXTURE: OnceLock<Texture2D> = OnceLock::new();

pub static UI_TEXTURE: OnceLock<Texture2D> = OnceLock::new();
pub static PORTRAIT_BG_TEXTURE: OnceLock<Texture2D> = OnceLock::new();
pub static PORTRAIT_ENEMY_BG_TEXTURE: OnceLock<Texture2D> = OnceLock::new();

pub static LIGHTNING_BOLT_FX: OnceLock<Texture2D> = OnceLock::new();
