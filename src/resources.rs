use std::{collections::HashMap, f32::consts::PI, hash::Hash, sync::OnceLock};

use macroquad::{
    color::{BLACK, WHITE},
    math::{Rect, Vec2},
    miniquad::StencilFaceState,
    shapes::draw_circle,
    text::{load_ttf_font, Font},
    texture::{draw_texture_ex, load_texture, DrawTextureParams, FilterMode, Image, Texture2D},
    time::get_time,
};
use serde::{Deserialize, Serialize};

use crate::{
    core::CoreGame,
    game_ui::UserInterface,
    game_ui_connection::GameUserInterfaceConnection,
    init_fight_map::GameInitState,
    pathfind::{Elevation, Surface, SurfaceBrushSize, TerrainType, CELLS_PER_ENTITY},
    sounds::SoundPlayer,
    util::{COL_GREEN_3, COL_RED, COL_RED_BRIGHT},
};

#[derive(Clone)]
pub struct GameResources {
    pub sprites: HashMap<SpriteId, Sprite>,
    pub simple_font: Font,
    pub big_font: Font,
    pub decorative_font: Font,
    pub tiny_font: Font,
    pub terrain_atlas: Texture2D,
}

impl GameResources {
    pub async fn load() -> Self {
        //let font_path = "manaspace/manaspc.ttf";
        //let font_path = "yoster-island/yoster.ttf"; // <-- looks like yoshi's island. Not very readable
        //let font_path = "pixy/PIXY.ttf"; // <-- only uppercase, looks a bit too sci-fi?
        //let font_path = "return-of-ganon/retganon.ttf";
        //let font_path = "press-start/prstart.ttf";
        //let font_path = "lunchtime-doubly-so/lunchds.ttf";
        //let font_path = "chonkypixels/ChonkyPixels.ttf";
        let sprites = load_all_sprites().await;
        let font_path = "delicatus/Delicatus.ttf"; // <-- not bad! very thin and readable
        let simple_font = load_font(font_path).await;
        let big_font = load_font("manaspace/manaspc.ttf").await;
        let decorative_font = load_font("dpcomic/dpcomic.ttf").await;
        let tiny_font = load_font("micro/Micro5-Regular.ttf").await;
        //let tiny_font = load_font("tiny5/Tiny5-SolidRegular.ttf").await;

        let terrain_atlas = load_and_init_texture("terrain_atlas.png").await;

        // Change background tiles to "night time"
        let night_time = false;
        if night_time {
            replace_color_fn(&terrain_atlas, |[r, g, b, a]| {
                if r > g && r > b {
                    [r, g, b, a]
                } else {
                    [
                        (r as f32 * 0.1) as u8,
                        (g as f32 * 0.8) as u8,
                        (b as f32 * 0.8) as u8,
                        a,
                    ]
                }
            });
        }

        Self {
            sprites,
            simple_font,
            big_font,
            decorative_font,
            tiny_font,
            terrain_atlas,
        }
    }
}

fn replace_color_fn(texture: &Texture2D, f: impl Fn([u8; 4]) -> [u8; 4]) {
    let mut img = texture.get_texture_data();
    for pixel in img.get_image_data_mut() {
        *pixel = f(*pixel);
    }
    texture.update(&img);
}

pub fn init_core_game(
    resources: GameResources,
    sound_player: SoundPlayer,
    init_state: GameInitState,
) -> CoreGame {
    let mut game_ui = GameUserInterfaceConnection::uninitialized();
    let core_game = CoreGame::new(game_ui.clone(), &init_state);
    let gfx_user_interface = UserInterface::new(&core_game, resources, init_state, sound_player);
    game_ui.init(gfx_user_interface);
    core_game
}

async fn load_font(path: &str) -> Font {
    let path = format!("fonts/{path}");
    let mut font = load_ttf_font(&path).await.unwrap();
    font.set_filter(FilterMode::Nearest);
    font
}

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
    SkeletonBoss,
    Enslaved,
    Draug,
    Lootgoblin,
    Ghoul,
    Scoundrel,
    Pyromaniac,
    Ogre,
    Magi,
    Warhammer,
    Waraxe,
    Bossaxe,
    Staff,
    Bow,
    Sword,
    MagicSword,
    HungeringBlade,
    Rapier,
    Dagger,
    Shield,
    SpikeShield,
    CharacterShadow,
}

#[derive(Clone)]
pub struct Sprite {
    pub regular: Texture2D,
    pub white_highlight: Texture2D,
    pub red_highlight: Texture2D,
    pub bright: Texture2D,
}

pub async fn load_all_sprites() -> HashMap<SpriteId, Sprite> {
    let paths = vec![
        (SpriteId::Alice, "alice.png"),
        (SpriteId::Bob, "bob.png"),
        (SpriteId::Clara, "clara.png"),
        (SpriteId::Huldra, "huldra.png"),
        (SpriteId::Skeleton, "skeleton.png"),
        (SpriteId::SkeletonBoss, "skeleton_boss.png"),
        (SpriteId::Enslaved, "enslaved.png"),
        (SpriteId::Draug, "draug.png"),
        (SpriteId::Lootgoblin, "lootgoblin.png"),
        (SpriteId::Ghoul, "ghoul.png"),
        (SpriteId::Scoundrel, "scoundrel.png"),
        (SpriteId::Pyromaniac, "pyromaniac.png"),
        (SpriteId::Ogre, "ogre.png"),
        (SpriteId::Magi, "magi.png"),
        (SpriteId::Warhammer, "warhammer.png"),
        (SpriteId::Waraxe, "waraxe.png"),
        (SpriteId::Bossaxe, "boss_axe.png"),
        (SpriteId::Staff, "staff.png"),
        (SpriteId::Bow, "bow.png"),
        (SpriteId::Sword, "sword.png"),
        (SpriteId::MagicSword, "magic_sword.png"),
        (SpriteId::HungeringBlade, "hungering_blade.png"),
        (SpriteId::Rapier, "rapier.png"),
        (SpriteId::Dagger, "dagger.png"),
        (SpriteId::Shield, "shield.png"),
        (SpriteId::SpikeShield, "spiked_shield.png"),
        (SpriteId::CharacterShadow, "character_shadow.png"),
    ];

    let paths: Vec<(SpriteId, String)> = paths
        .iter()
        .map(|p| (p.0, format!("sprites/{}", p.1)))
        .collect();
    let sprites = load_sprites(paths).await;

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

            let bright = Texture2D::from_image(&texture.get_texture_data());
            bright.set_filter(FilterMode::Nearest);
            brighten(&bright, highlight_color);

            (
                id,
                Sprite {
                    regular: texture,
                    white_highlight: white,
                    red_highlight: red,
                    bright,
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
        SpriteId::SkeletonBoss => 29,
        SpriteId::Enslaved => 26,
        SpriteId::Ogre => 26,
        SpriteId::Magi => 25,
        SpriteId::Ghoul => 24,
        SpriteId::Scoundrel => 24,
        SpriteId::Pyromaniac => 24,
        SpriteId::Draug => 26,
        SpriteId::Lootgoblin => 24,

        // TODO:
        SpriteId::PinkMan => 25,
        SpriteId::AlsoWeirdOrangeMan => 25,
        SpriteId::WeirdOrangeMan => 25,

        unhandled => panic!("undefined character sprite height: {:?}", unhandled),
    }
}

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug, Deserialize, Serialize)]
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
    Bloodlust,
    NearDeath,
    Dead,
    Wet,
    Poisoned,
    Treasure,
    HungeringBladeSouls,
    Advantage,
    Mana,
    Blade,
    Frost,
}

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug)]
pub enum IconId {
    Undefined,

    Undead,

    Fireball,
    Icebolt,
    SearingLight,
    MeleeAttack,
    RangedAttack,
    PiercingShot,
    Block,
    Brace,
    Move,
    Dash,

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
    Bonk,
    Plus,
    PlusPlus,
    QuickActions,
    Go,
    Extend,
    Knockback,
    Escape,
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
    Bless,
    BloodMagic,
    SpikeTrap,

    Inferno,
    Energize,
    Inspire,
    Bloodlust,
    LightningBolt,
    Execute,

    DraugClaw,
    DraugHaunt,

    SwampDweller,

    HardenedSkin,
    WeaponProficiency,
    CriticalCharge,
    Reaper,
}

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug)]
pub enum PortraitId {
    Alice,
    Bob,
    Clara,
    Skeleton,
    Draug,
    Magi,
    Huldra,
    Ghoul,
    Scoundrel,
    Lootgoblin,
    Pyromaniac,
    Enslaved,
    Ogre,
}

fn portrait_cell(portrait: PortraitId) -> (i32, i32) {
    match portrait {
        PortraitId::Skeleton => (0, 0),
        PortraitId::Alice => (0, 3),
        PortraitId::Bob => (3, 2),
        PortraitId::Clara => (2, 2),
        PortraitId::Draug => (2, 0),
        PortraitId::Magi => (1, 0),
        PortraitId::Huldra => (3, 0),
        PortraitId::Ghoul => (0, 1),
        PortraitId::Scoundrel => (1, 1),
        PortraitId::Lootgoblin => (2, 1),
        PortraitId::Pyromaniac => (3, 1),
        PortraitId::Enslaved => (1, 3),
        PortraitId::Ogre => (1, 2),
    }
}

const PORTRAIT_SRC_SIZE: (f32, f32) = (32.0, 40.0);

pub fn portrait_texture(portrait: PortraitId) -> Texture2D {
    let texture = PORTRAITS_TEXTURE.get().unwrap();
    let (col, row) = portrait_cell(portrait);
    let w = PORTRAIT_SRC_SIZE.0;
    let h = PORTRAIT_SRC_SIZE.1;

    let rect = Rect::new(col as f32 * w, row as f32 * h, w, h);
    let texture = Texture2D::from_image(&texture.get_texture_data().sub_image(rect));
    texture.set_filter(FilterMode::Nearest);
    texture
}

pub fn draw_portrait(pos: (f32, f32), portrait: PortraitId, dest_size: Option<(f32, f32)>) {
    let x = pos.0.floor();
    let y = pos.1.floor();
    let texture = PORTRAITS_TEXTURE.get().unwrap();
    let (col, row) = portrait_cell(portrait);
    let w = PORTRAIT_SRC_SIZE.0;
    let h = PORTRAIT_SRC_SIZE.1;
    let dest_size = dest_size.map(|s| s.into()).unwrap_or((w, h).into());
    draw_texture_ex(
        texture,
        x,
        y,
        WHITE,
        DrawTextureParams {
            dest_size: Some(dest_size),
            source: Some(Rect::new(col as f32 * w, row as f32 * h, w, h)),
            ..Default::default()
        },
    );
}

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug)]
pub enum EquipmentIconId {
    Undefined,
    Rapier,
    Warhammer,
    Staff,
    Bow,
    ArcaneBow,
    Dagger,
    Sword,
    MagicSword,
    HungeringBlade,
    SmallShield,
    SpikeShield,
    Waraxe,
    MediumShield,
    LeatherArmor,
    ChainMail,
    Shirt,
    Robe,
    DruidCoat,
    PenetratingArrow,
    BarbedArrow,
    FireArrow,
    ColdArrow,
    ExplodingArrow,

    HealthPotion,
    ManaPotion,
    AdrenalinPotion,
    EnergyPotion,
    ArcanePotion,
    ToxicGrenade,
    LiquidFire,

    PlaceholderOffhand,
    PlaceholderMainhand,
    PlaceholderArmor,
    PlaceholderArrows,
    PlaceholderTrinket,

    ArcaneRing,
    NecroticBand,
    HuntersCharm,
    DraugNecklace,
    BattleMageSigil,
    AssassinsVial,
    Saphire,
    Ruby,
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
pub enum LiquidType {
    Water,
    Poison,
}

impl LiquidType {
    pub fn from_surface(surface: Surface) -> Option<Self> {
        match surface {
            Surface::Water => Some(Self::Water),
            Surface::Poison => Some(Self::Poison),
            Surface::Spikes | Surface::Fire => None,
        }
    }
}

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug, Serialize, Deserialize)]
pub enum WallOrientation {
    Single,
    ConcaveNorthWest,
    North,
    ConcaveNorthEast,
    West,
    East,
    ConcaveSouthWest,
    South,
    ConcaveSouthEast,
    ConvexNorthWest,
    ConvexNorthEast,
    ConvexSouthWest,
    ConvexSouthEast,
    Inner,

    NorthMezzanine,
    ConvexSouthEastMezzanine,
    ConvexSouthWestMezzanine,
}

impl WallOrientation {
    pub fn is_wall_highground_mezzanine(&self) -> bool {
        matches!(
            self,
            WallOrientation::NorthMezzanine
                | WallOrientation::ConvexSouthEastMezzanine
                | WallOrientation::ConvexSouthWestMezzanine
        )
    }
}

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug, Serialize, Deserialize)]
pub enum TerrainId {
    GroundBorderNorth,
    GroundBorderEast,
    Grass,
    Grass2,
    Grass3,
    Grass4,
    Floor,
    Floor2,
    Floor3,
    Floor4,
    Floor5,
    Floor6,
    Floor7,
    Floor8,
    Floor9,
    Floor10,
    Dirt,
    SpikesSingleCell,
    FireSurfaceSingleCell,
    StoneWall(WallOrientation),
    HighgroundBorder(WallOrientation),

    Staircase,
    StaircaseLeft,
    StaircaseTall,

    Bush,
    DeadBush,
    WallPlant,
    FloorDrain,
    SmallObstacle,
    BarrelsObstacle,
    BarrelsObstacle2,
    BarrelsObstacle3,
    BarrelsObstacle4,
    SackObstacle,
    BoxesObstacle,
    SmallStone,
    TombStone,
    DirtMound,
    Boulder2,
    TreeStump,
    Table,

    BookShelf,
    WineShelf,
    Barrel,
    HangingWeapons,
    HangingWeapons2,
    WallPainting,
    WallPainting2,
    WallFlag,
    Mat,
    Cauldron,
    Cauldron2,
    Chest,
    FenceHor,
    WallOpeningNorth,
    WallOpeningEast,
    WallOpeningWest,
    WallWindow,
    WallWindow2,
    WallWindow2Left,
    WallWindow2Right,
    SuitOfArmor,
    AnimalHead,
    Torch,
    Lantern,
    FoodPlate,
    FoodPlate2,

    NewWater(WaterOrientation, LiquidType),

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

    pub fn is_surface(&self) -> bool {
        matches!(self, TerrainId::NewWater(..) | TerrainId::SpikesSingleCell)
    }

    pub fn surface(&self) -> Option<(Surface, SurfaceBrushSize)> {
        match self {
            TerrainId::NewWater(..) => Some((Surface::Water, SurfaceBrushSize::ThreeByThree)),
            TerrainId::SpikesSingleCell => Some((Surface::Spikes, SurfaceBrushSize::SingleCell)),
            TerrainId::FireSurfaceSingleCell => Some((Surface::Fire, SurfaceBrushSize::SingleCell)),
            _ => None,
        }
    }

    pub fn is_wall(&self) -> bool {
        match self {
            TerrainId::StoneWall(..) => true,
            TerrainId::HighgroundBorder(..) => true,
            _ => false,
        }
    }

    pub fn elevation(&self) -> Option<(Elevation, WallOrientation)> {
        match self {
            TerrainId::StoneWall(orientation) => Some((Elevation::Wall, *orientation)),
            TerrainId::HighgroundBorder(orientation) => Some((Elevation::High, *orientation)),
            _ => None,
        }
    }

    pub fn from_elevation(elevation: Elevation, orientation: WallOrientation) -> Self {
        match elevation {
            Elevation::Wall => TerrainId::StoneWall(orientation),
            Elevation::High => TerrainId::HighgroundBorder(orientation),
        }
    }

    pub fn terrain_type(&self) -> TerrainType {
        match self {
            Self::StoneWall(orientation) => {
                TerrainType::ElevationBorder(Elevation::Wall, *orientation)
            }
            Self::HighgroundBorder(orientation) => {
                TerrainType::ElevationBorder(Elevation::High, *orientation)
            }
            _ => TerrainType::Other,
        }
    }

    pub fn is_staircase(&self) -> bool {
        matches!(
            self,
            TerrainId::Staircase | TerrainId::StaircaseTall | TerrainId::StaircaseLeft
        )
    }

    pub fn occupied_cells(
        &self,
    ) -> Option<(std::ops::RangeInclusive<i32>, std::ops::RangeInclusive<i32>)> {
        let (xs, ys) = match self.elevation() {
            Some((elevation, orientation)) => {
                //println!("(occupied cells) Low wall orientation: {:?}", orientation);

                match orientation {
                    WallOrientation::Single => (-1..=1, -1..=1),
                    // left column
                    WallOrientation::ConvexNorthWest => (-1..=-1, -1..=1),
                    // right column
                    WallOrientation::ConvexNorthEast => (1..=1, -1..=1),
                    // left column
                    WallOrientation::East => (-1..=-1, -1..=1),
                    // right column
                    WallOrientation::West => (1..=1, -1..=1),
                    WallOrientation::Inner => return None,

                    WallOrientation::North
                    | WallOrientation::ConvexSouthEast
                    | WallOrientation::ConvexSouthWest => match elevation {
                        Elevation::High => {
                            // two lower rows
                            (-1..=1, 0..=1)
                        }
                        Elevation::Wall => {
                            // all 9 cells
                            (-1..=1, -1..=1)
                        }
                    },
                    WallOrientation::South => return None,
                    WallOrientation::ConcaveSouthWest => return None,
                    WallOrientation::ConcaveSouthEast => return None,
                    // part of left column
                    WallOrientation::ConcaveNorthEast => (-1..=-1, 0..=1),
                    // part of right column
                    WallOrientation::ConcaveNorthWest => (1..=1, 0..=1),
                    // two upper rows
                    WallOrientation::NorthMezzanine => (-1..=1, -1..=0),
                    // two upper rows
                    WallOrientation::ConvexSouthEastMezzanine => (-1..=1, -1..=0),
                    // two upper rows
                    WallOrientation::ConvexSouthWestMezzanine => (-1..=1, -1..=0),
                }
            }
            None => {
                match self {
                    // two lower rows
                    TerrainId::SmallObstacle => (-1..=1, 0..=1),
                    // two lower rows
                    TerrainId::BarrelsObstacle => (-1..=1, 0..=1),
                    // two left columns
                    TerrainId::BarrelsObstacle2 => (-1..=0, -1..=1),
                    // part of two lower rows
                    TerrainId::BarrelsObstacle3 => (-1..=0, 0..=1),
                    // part of two lower rows
                    TerrainId::BarrelsObstacle4 => (-1..=0, 0..=1),
                    // part of two lower rows
                    TerrainId::SackObstacle => (-1..=0, 0..=1),
                    // two lower rows
                    TerrainId::BoxesObstacle => (-1..=1, 0..=1),
                    // two lower rows
                    TerrainId::TombStone => (-1..=1, 0..=1),
                    // two lower rows
                    TerrainId::Chest => (-1..=1, 0..=1),
                    // bottom row
                    TerrainId::FenceHor => (-1..=1, 1..=1),
                    // two lower rows
                    TerrainId::Cauldron2 => (-1..=1, 0..=1),
                    // two lower rows
                    TerrainId::DeadBush => (-1..=1, 0..=1),
                    _ => (-1..=1, -1..=1),
                }
            }
        };

        Some((xs, ys))
    }
}

pub fn draw_terrain(
    texture: &Texture2D,
    terrain_id: TerrainId,
    rect_w: u32,
    screen_pos: (f32, f32),
) {
    let (rotation, src_rect) = terrain_atlas_area(terrain_id, screen_pos);
    let (w, h) = (32.0, 32.0);
    let (x, y) = screen_pos;
    let src_rect_size = src_rect.size();
    let dst_size = (
        rect_w as f32 * src_rect_size.x / w,
        rect_w as f32 * src_rect_size.y / h,
    );

    let params = DrawTextureParams {
        source: Some(src_rect),
        dest_size: Some(dst_size.into()),
        rotation,
        ..Default::default()
    };

    let cell_w = rect_w as f32 / 3.0;

    draw_texture_ex(
        texture,
        (x - cell_w).floor(),
        (y - cell_w).floor(),
        WHITE,
        params,
    );
}

pub fn terrain_atlas_area(terrain_id: TerrainId, screen_pos: (f32, f32)) -> (f32, Rect) {
    let (w, h) = (32.0, 32.0);

    let mut rotation = 0.0;
    let mut is_poison = false;
    let (mut col, mut row) = match terrain_id {
        TerrainId::GroundBorderNorth => (10, 0),
        TerrainId::GroundBorderEast => (9, 0),
        TerrainId::Grass => (11, 0),
        TerrainId::Grass2 => (12, 0),
        TerrainId::Grass3 => (13, 0),
        TerrainId::Grass4 => (14, 0),

        TerrainId::Floor => (11, 1),
        TerrainId::Floor2 => (11, 2),
        TerrainId::Floor3 => (11, 3),
        TerrainId::Floor4 => (12, 1),
        TerrainId::Floor5 => (12, 2),
        TerrainId::Floor6 => (13, 2),
        TerrainId::Floor7 => (12, 3),
        TerrainId::Floor8 => (13, 3),
        TerrainId::Floor9 => (14, 3),
        TerrainId::Floor10 => (13, 1),
        TerrainId::Dirt => (11, 4),
        TerrainId::NewWater(orientation, type_) => {
            is_poison = type_ == LiquidType::Poison;
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

        TerrainId::StoneWall(WallOrientation::Single) => (1, 7),
        TerrainId::StoneWall(WallOrientation::ConcaveNorthWest) => (0, 12),
        TerrainId::StoneWall(WallOrientation::North) => (1, 12),
        TerrainId::StoneWall(WallOrientation::ConcaveNorthEast) => (2, 12),
        TerrainId::StoneWall(WallOrientation::West) => (0, 13),
        TerrainId::StoneWall(WallOrientation::East) => (2, 13),
        TerrainId::StoneWall(WallOrientation::ConcaveSouthWest) => (0, 14),
        TerrainId::StoneWall(WallOrientation::South) => (1, 14),
        TerrainId::StoneWall(WallOrientation::ConcaveSouthEast) => (2, 14),
        TerrainId::StoneWall(WallOrientation::ConvexNorthWest) => (3, 12),
        TerrainId::StoneWall(WallOrientation::ConvexNorthEast) => (5, 12),
        TerrainId::StoneWall(WallOrientation::ConvexSouthWest) => (3, 14),
        TerrainId::StoneWall(WallOrientation::ConvexSouthEast) => (5, 14),
        TerrainId::StoneWall(WallOrientation::Inner) => (4, 13),
        TerrainId::StoneWall(WallOrientation::NorthMezzanine) => (1, 15),
        TerrainId::StoneWall(WallOrientation::ConvexSouthEastMezzanine) => (2, 15),
        TerrainId::StoneWall(WallOrientation::ConvexSouthWestMezzanine) => (0, 15),

        TerrainId::HighgroundBorder(WallOrientation::Single) => (14, 13),
        TerrainId::HighgroundBorder(WallOrientation::ConcaveNorthWest) => (8, 13),
        TerrainId::HighgroundBorder(WallOrientation::North) => (13, 15),
        TerrainId::HighgroundBorder(WallOrientation::ConcaveNorthEast) => (10, 13),
        TerrainId::HighgroundBorder(WallOrientation::West) => (14, 14),
        TerrainId::HighgroundBorder(WallOrientation::East) => (11, 14),
        TerrainId::HighgroundBorder(WallOrientation::ConcaveSouthWest) => (8, 15),
        TerrainId::HighgroundBorder(WallOrientation::South) => (13, 13),
        TerrainId::HighgroundBorder(WallOrientation::ConcaveSouthEast) => (10, 15),
        TerrainId::HighgroundBorder(WallOrientation::ConvexNorthWest) => (11, 13),
        TerrainId::HighgroundBorder(WallOrientation::ConvexNorthEast) => (14, 13),
        TerrainId::HighgroundBorder(WallOrientation::ConvexSouthWest) => (11, 15),
        TerrainId::HighgroundBorder(WallOrientation::ConvexSouthEast) => (14, 15),
        TerrainId::HighgroundBorder(WallOrientation::Inner) => (13, 14),
        TerrainId::HighgroundBorder(
            WallOrientation::NorthMezzanine
            | WallOrientation::ConvexSouthEastMezzanine
            | WallOrientation::ConvexSouthWestMezzanine,
        ) => {
            panic!("highground mezzanine?")
        }

        TerrainId::Staircase => (12, 15),
        TerrainId::StaircaseLeft => (6, 15),
        TerrainId::StaircaseTall => (7, 13),

        TerrainId::Bush => (0, 7),
        TerrainId::DeadBush => (7, 7),
        TerrainId::WallPlant => (0, 8),
        TerrainId::SmallStone => (1, 8),
        TerrainId::TombStone => (2, 8),
        TerrainId::DirtMound => (3, 8),
        TerrainId::FloorDrain => (0, 9),
        TerrainId::SmallObstacle => (1, 9),
        TerrainId::BarrelsObstacle => (0, 10),
        TerrainId::BarrelsObstacle2 => (1, 10),
        TerrainId::BarrelsObstacle3 => (11, 7),
        TerrainId::BarrelsObstacle4 => (12, 7),
        TerrainId::SackObstacle => (13, 7),
        TerrainId::BoxesObstacle => (14, 7),
        TerrainId::Boulder2 => (0, 6),
        TerrainId::TreeStump => (1, 6),
        TerrainId::Table => (2, 7),

        TerrainId::BookShelf => (2, 6),
        TerrainId::WineShelf => (7, 5),
        TerrainId::Barrel => (8, 5),
        TerrainId::HangingWeapons => (9, 5),
        TerrainId::HangingWeapons2 => (9, 4),
        TerrainId::WallPainting => (3, 6),
        TerrainId::WallPainting2 => (5, 5),
        TerrainId::WallFlag => (4, 6),
        TerrainId::Torch => (8, 6),
        TerrainId::Lantern => (10, 6),
        TerrainId::Mat => (4, 8),
        TerrainId::Cauldron => (5, 8),
        TerrainId::Cauldron2 => (6, 8),
        TerrainId::Chest => (9, 8),
        TerrainId::FenceHor => (9, 7),
        TerrainId::WallWindow => (11, 8),
        TerrainId::WallWindow2 => (12, 8),
        TerrainId::WallWindow2Left => (13, 8),
        TerrainId::WallWindow2Right => (14, 8),

        TerrainId::WallOpeningNorth => (5, 6),
        TerrainId::WallOpeningEast => (6, 6),
        TerrainId::WallOpeningWest => (6, 7),
        TerrainId::SuitOfArmor => (4, 7),
        TerrainId::AnimalHead => (5, 7),
        TerrainId::FoodPlate => (6, 5),
        TerrainId::FoodPlate2 => (7, 6),
        TerrainId::SpikesSingleCell => (1, 11),
        TerrainId::FireSurfaceSingleCell => (9, 3),

        TerrainId::Water => (2, 3),
        TerrainId::WaterBeachNorth => (2, 1),
        TerrainId::WaterBeachEast => (4, 3),
        TerrainId::WaterBeachSouth => (2, 4),
        TerrainId::WaterBeachWest => (1, 3),

        TerrainId::WaterBeachNorthEast => (4, 1),
        TerrainId::WaterBeachSouthEast => (4, 4),
        TerrainId::WaterBeachSouthWest => (1, 4),
        TerrainId::WaterBeachNorthWest => (1, 1),

        TerrainId::WaterBeachNorthEastSouth => (5, 2),
        TerrainId::WaterBeachEastSouthWest => (3, 5),
        TerrainId::WaterBeachSouthWestNorth => (0, 2),
        TerrainId::WaterBeachWestNorthEast => (3, 0),

        TerrainId::WaterBeachWestEast => (6, 1),
        TerrainId::WaterBeachNorthSouth => (6, 3),
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

    // animate cauldron bubbles
    if (t * 0.6) % (t * 0.6).floor() < 0.5 && (col, row) == (6, 8) {
        (col, row) = (7, 8);
    }

    // animate torch fire
    if (t * 0.6) % (t * 0.6).floor() < 0.5 && (col, row) == (8, 6) {
        (col, row) = (9, 6);
    }

    // animate lantern glow
    if (t * 0.8) % (t * 0.8).floor() < 0.5 && (col, row) == (10, 6) {
        (col, row) = (11, 6);
    }

    // animate fire surface
    if (col, row) == (9, 3) {
        if (t * 0.8) % (t * 0.8).floor() < 0.5 {
            (col, row) = (10, 3);
        }
    }

    let src_sides = [
        col as f32 * w,
        row as f32 * h,
        col as f32 * w + w,
        row as f32 * h + h,
    ];

    let src_rect = Rect::new(
        src_sides[0],
        src_sides[1],
        src_sides[2] - src_sides[0],
        src_sides[3] - src_sides[1],
    );
    (rotation, src_rect)
}

async fn load_sprites(paths: Vec<(SpriteId, impl AsRef<str>)>) -> HashMap<SpriteId, Texture2D> {
    let mut textures: HashMap<SpriteId, Texture2D> = Default::default();
    for (id, path) in paths {
        textures.insert(id, load_and_init_texture(path.as_ref()).await);
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
    replace_color(&texture, [0, 0, 0, 255], COL_GREEN_3.into());
    TINY_FONT_GREEN_TEXTURE.get_or_init(|| texture);

    let texture = load_and_init_texture("tiny_font.png").await;
    replace_color(&texture, [0, 0, 0, 255], [255, 100, 100, 255]); //[255, 100, 100, 255]);
    TINY_FONT_RED_TEXTURE.get_or_init(|| texture);

    let texture = load_and_init_texture("tiny_font.png").await;
    replace_color(&texture, [0, 0, 0, 255], [0, 0, 0, 255]); //[255, 100, 100, 255]);
    TINY_FONT_BLACK_TEXTURE.get_or_init(|| texture);
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

fn make_grayscale(texture: &Texture2D) {
    let mut img = texture.get_texture_data();
    for pixel in img.get_image_data_mut() {
        let avg = ((pixel[0] as f32 + pixel[1] as f32 + pixel[2] as f32) / 3.0) as u8;
        *pixel = [avg, avg, avg, pixel[3]];
    }
    texture.update(&img);
}

fn brighten(texture: &Texture2D, highlight_color: [u8; 4]) {
    let mut img = texture.get_texture_data();
    for pixel in img.get_image_data_mut() {
        if pixel == &highlight_color {
            *pixel = [255, 255, 255, 255];
        } else {
            for i in 0..3 {
                pixel[i] = ((255.0 + pixel[i] as f32 * 2.0) / 3.0) as u8;
            }
        }
    }
    texture.update(&img);
}

pub enum TinyFontColor {
    Green,
    Red,
    Black,
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
        TinyFontColor::Black => TINY_FONT_BLACK_TEXTURE.get().unwrap(),
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
        } else if ch == '+' {
            10
        } else if ch == '-' {
            11
        } else if ch == '^' {
            12
        } else if ch == 'v' {
            13
        } else if ch == 'd' {
            14
        } else {
            15
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

pub async fn load_and_init_static_resources() {
    load_and_init_tiny_font().await;
    load_and_init_font_symbols().await;
    load_and_init_ui_textures().await;

    load_and_init_texture_into("status.png", &STATUS_ICONS_TEXTURE).await;
    load_and_init_texture_into("icon.png", &ICONS_TEXTURE).await;

    let icon_faded_atlas = load_and_init_texture("icon.png").await;
    make_grayscale(&icon_faded_atlas);
    ICONS_FADED_TEXTURE.get_or_init(|| icon_faded_atlas);

    load_and_init_texture_into("equipment_icons.png", &EQUIPMENT_ICONS_TEXTURE).await;
    load_and_init_texture_into("effects.png", &EFFECTS_TEXTURE).await;
    load_and_init_texture_into("portraits.png", &PORTRAITS_TEXTURE).await;
    load_and_init_texture_into("fx_beam.png", &BEAM_FX).await;

    load_static_fonts().await;
}

pub fn draw_eq_icon(icon: EquipmentIconId, x: f32, y: f32, dest_size: Option<(f32, f32)>) {
    let x = x.floor();
    let y = y.floor();
    let texture = EQUIPMENT_ICONS_TEXTURE.get().unwrap();
    let (col, row): (i32, i32) = match icon {
        EquipmentIconId::Undefined => (4, 4),
        EquipmentIconId::Rapier => (4, 0),
        EquipmentIconId::Warhammer => (2, 1),
        EquipmentIconId::Staff => (5, 1),
        EquipmentIconId::Bow => (3, 1),
        EquipmentIconId::ArcaneBow => (8, 1),
        EquipmentIconId::Dagger => (1, 1),
        EquipmentIconId::Sword => (0, 1),
        EquipmentIconId::MagicSword => (5, 0),
        EquipmentIconId::HungeringBlade => (6, 0),
        EquipmentIconId::SmallShield => (4, 1),
        EquipmentIconId::SpikeShield => (6, 1),
        EquipmentIconId::Waraxe => (7, 1),
        EquipmentIconId::MediumShield => (0, 2),
        EquipmentIconId::LeatherArmor => (2, 2),
        EquipmentIconId::ChainMail => (1, 2),
        EquipmentIconId::Shirt => (3, 2),
        EquipmentIconId::Robe => (4, 2),
        EquipmentIconId::DruidCoat => (6, 2),
        EquipmentIconId::PenetratingArrow => (0, 0),
        EquipmentIconId::BarbedArrow => (1, 0),
        EquipmentIconId::FireArrow => (2, 0),
        EquipmentIconId::ColdArrow => (3, 0),
        EquipmentIconId::ExplodingArrow => (7, 0),
        EquipmentIconId::HealthPotion => (5, 4),
        EquipmentIconId::ToxicGrenade => (6, 4),
        EquipmentIconId::LiquidFire => (7, 4),
        EquipmentIconId::ManaPotion => (0, 4),
        EquipmentIconId::AdrenalinPotion => (1, 4),
        EquipmentIconId::EnergyPotion => (2, 4),
        EquipmentIconId::ArcanePotion => (3, 4),
        EquipmentIconId::PlaceholderArrows => (0, 3),
        EquipmentIconId::PlaceholderMainhand => (1, 3),
        EquipmentIconId::PlaceholderArmor => (2, 3),
        EquipmentIconId::PlaceholderOffhand => (3, 3),
        EquipmentIconId::PlaceholderTrinket => (4, 3),
        EquipmentIconId::ArcaneRing => (0, 5),
        EquipmentIconId::NecroticBand => (1, 5),
        EquipmentIconId::HuntersCharm => (2, 5),
        EquipmentIconId::DraugNecklace => (3, 5),
        EquipmentIconId::BattleMageSigil => (4, 5),
        EquipmentIconId::AssassinsVial => (5, 5),
        EquipmentIconId::Saphire => (6, 5),
        EquipmentIconId::Ruby => (6, 6),
    };
    let icon_w = 20.0;
    let icon_h = 20.0;
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

fn icon_cell(icon: IconId) -> (i32, i32) {
    match icon {
        IconId::Undefined => (1, 3),
        IconId::Undead => (2, 3),
        IconId::Fireball => (0, 1),
        IconId::Icebolt => (3, 3),
        IconId::SearingLight => (2, 1),
        IconId::MeleeAttack => (6, 8),
        IconId::RangedAttack => (6, 7),
        IconId::SpikeTrap => (6, 6),
        IconId::PiercingShot => (7, 7),
        IconId::Block => (8, 7),
        IconId::Brace => (5, 2),
        IconId::LightningBolt => (6, 2),
        IconId::DraugClaw => (7, 2),
        IconId::DraugHaunt => (8, 2),
        IconId::SwampDweller => (9, 2),
        IconId::Move => (0, 4),
        IconId::Dash => (1, 5),
        IconId::Scream => (7, 1),
        IconId::Mindblast => (8, 1),
        IconId::NecroticInfluence => (1, 1),
        IconId::Parry => (9, 7),
        IconId::Sidestep => (2, 7),
        IconId::Tackle => (3, 7),
        IconId::ShieldBash => (5, 7),
        IconId::Rage => (2, 6),
        IconId::CrushingStrike => (4, 7),
        IconId::CarefulAim => (8, 8),
        IconId::Execute => (9, 8),
        IconId::CripplingShot => (7, 8),
        IconId::TrueStrike => (5, 6),
        IconId::SpellAdvantage => (3, 1),
        IconId::Banshee => (0, 2),
        IconId::Dualcast => (1, 2),
        IconId::Bonk => (4, 7),
        IconId::Plus => (4, 9),
        IconId::PlusPlus => (5, 9),
        IconId::QuickActions => (8, 9),
        IconId::Go => (3, 4),
        IconId::Extend => (0, 6),
        IconId::Knockback => (2, 5),
        IconId::Escape => (0, 5),
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
        IconId::Bless => (6, 3),
        IconId::BloodMagic => (0, 3),
        IconId::Inferno => (3, 2),
        IconId::Energize => (4, 1),
        IconId::Inspire => (4, 2),
        IconId::Bloodlust => (4, 3),
        IconId::HardenedSkin => (3, 0),
        IconId::WeaponProficiency => (2, 0),
        IconId::CriticalCharge => (1, 0),
        IconId::Reaper => (0, 0),
    }
}

pub fn draw_icon(icon: IconId, x: f32, y: f32, dest_size: Option<(f32, f32)>, faded: bool) {
    let x = x.floor();
    let y = y.floor();
    let texture = if faded {
        ICONS_FADED_TEXTURE.get().unwrap()
    } else {
        ICONS_TEXTURE.get().unwrap()
    };
    let (col, row) = icon_cell(icon);
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

#[derive(Debug)]
pub struct FlipbookAnimation {
    pub texture: Texture2D,
    pub source_pos: (f32, f32),
    pub source_size: (f32, f32),
    pub num_frames: u32,
    pub draw_size_cell_w: f32,
}

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug)]
pub enum EffectId {
    Zap,
    Bonk,
    Blood,
    ArmorClank,
    Poof,
    Magic,
    MagicSpinUp,
    PurpleMagic,
}

impl FlipbookAnimation {
    pub fn from_effect(effect: EffectId) -> Self {
        let row = match effect {
            EffectId::Zap => 0,
            EffectId::Bonk => 1,
            EffectId::Blood => 2,
            EffectId::ArmorClank => 3,
            EffectId::Poof => 4,
            EffectId::Magic => 5,
            EffectId::MagicSpinUp => 6,
            EffectId::PurpleMagic => 7,
        };
        let source_size = (32.0, 32.0);

        let draw_size_cell_w = match effect {
            EffectId::PurpleMagic => 3.0,
            _ => 2.0,
        };

        Self {
            texture: EFFECTS_TEXTURE.get().unwrap().clone(),
            source_pos: (0.0, row as f32 * source_size.1),
            source_size,
            num_frames: 5,
            draw_size_cell_w,
        }
    }

    pub fn source_rect(&self, animation_idx: u32) -> Rect {
        Rect::new(
            self.source_pos.0 + self.source_size.0 * animation_idx as f32,
            self.source_pos.1,
            self.source_size.0,
            self.source_size.1,
        )
    }

    pub fn draw(&self, screen_pos: (f32, f32), animation_idx: u32, dest_size: f32) {
        let source = self.source_rect(animation_idx);
        let dest_size = (dest_size, dest_size);
        //let dest_size = (self.source_size.0 * 2.0, self.source_size.1 * 2.0);
        draw_texture_ex(
            &self.texture,
            screen_pos.0 - dest_size.0 / 2.0,
            screen_pos.1 - dest_size.1 / 2.0,
            WHITE,
            DrawTextureParams {
                source: Some(source),
                dest_size: Some(dest_size.into()),
                ..Default::default()
            },
        );
    }
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
        StatusId::Bloodlust => (2, 4),
        StatusId::NearDeath => (4, 2),
        StatusId::Dead => (2, 3),
        StatusId::Wet => (3, 3),
        StatusId::Poisoned => (4, 3),
        StatusId::Treasure => (5, 0),
        StatusId::HungeringBladeSouls => (5, 1),
        StatusId::Advantage => (5, 2),
        StatusId::Mana => (5, 3),
        StatusId::Blade => (0, 4),
        StatusId::Frost => (1, 4),
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
    AP_SYMBOL.get_or_init(|| symbol(5, 1));
    LOSE_AP_SYMBOL.get_or_init(|| symbol(6, 1));
    HEART_SYMBOL.get_or_init(|| symbol(0, 1));
    STAMINA_SYMBOL.get_or_init(|| symbol(1, 1));
    STAMINA_SMALL_SYMBOL.get_or_init(|| symbol(1, 3));
    MANA_SYMBOL.get_or_init(|| symbol(2, 1));
    MANA_SMALL_SYMBOL.get_or_init(|| symbol(0, 3));
    SWORD_SYMBOL.get_or_init(|| symbol(0, 2));
    BOOT_SYMBOL.get_or_init(|| symbol(1, 2));
    HELMET_SYMBOL.get_or_init(|| symbol(2, 3));
    TOUGHNESS_SYMBOL.get_or_init(|| symbol(1, 4));
    EVASION_SYMBOL.get_or_init(|| symbol(2, 4));
    WILL_SYMBOL.get_or_init(|| symbol(3, 4));
    LIGHTNING_SYMBOL.get_or_init(|| symbol(4, 4));
    FIRE_SYMBOL.get_or_init(|| symbol(5, 4));
    WEIGHT_SYMBOL.get_or_init(|| symbol(3, 3));
    SINGLE_TARGET_SYMBOL.get_or_init(|| symbol(4, 3));
    MULTI_TARGET_SYMBOL.get_or_init(|| symbol(5, 3));
    RANGE_SYMBOL.get_or_init(|| symbol(6, 3));
    RADIUS_SYMBOL.get_or_init(|| symbol(6, 2));
    CONFIRM_SYMBOL.get_or_init(|| symbol(3, 1));
    TARGET_SYMBOL.get_or_init(|| symbol(6, 4));
    REACTION_SYMBOL.get_or_init(|| symbol(0, 4));
    ENHANCEMENT_SYMBOL.get_or_init(|| symbol(0, 5));
    RIGHT_CLICK_SYMBOL.get_or_init(|| symbol(1, 5));
    LEFT_CLICK_SYMBOL.get_or_init(|| symbol(2, 5));
}

async fn load_and_init_texture_into(path: &str, dst: &OnceLock<Texture2D>) {
    let texture = load_and_init_texture(path).await;
    dst.get_or_init(|| texture);
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
pub static AP_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static LOSE_AP_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static HEART_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static STAMINA_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static STAMINA_SMALL_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static MANA_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static MANA_SMALL_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static SWORD_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static BOOT_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static HELMET_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static TOUGHNESS_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static EVASION_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static WILL_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static LIGHTNING_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static FIRE_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static WEIGHT_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static SINGLE_TARGET_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static MULTI_TARGET_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static RANGE_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static RADIUS_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static CONFIRM_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static TARGET_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static REACTION_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static ENHANCEMENT_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static RIGHT_CLICK_SYMBOL: OnceLock<Texture2D> = OnceLock::new();
pub static LEFT_CLICK_SYMBOL: OnceLock<Texture2D> = OnceLock::new();

pub static TINY_FONT_GREEN_TEXTURE: OnceLock<Texture2D> = OnceLock::new();
pub static TINY_FONT_RED_TEXTURE: OnceLock<Texture2D> = OnceLock::new();
pub static TINY_FONT_BLACK_TEXTURE: OnceLock<Texture2D> = OnceLock::new();

pub static STATUS_ICONS_TEXTURE: OnceLock<Texture2D> = OnceLock::new();
pub static ICONS_TEXTURE: OnceLock<Texture2D> = OnceLock::new();
pub static ICONS_FADED_TEXTURE: OnceLock<Texture2D> = OnceLock::new();
pub static EQUIPMENT_ICONS_TEXTURE: OnceLock<Texture2D> = OnceLock::new();
pub static EFFECTS_TEXTURE: OnceLock<Texture2D> = OnceLock::new();
pub static PORTRAITS_TEXTURE: OnceLock<Texture2D> = OnceLock::new();

pub static UI_BG_TEXTURE: OnceLock<Texture2D> = OnceLock::new();
pub static UI_ARROW_TEXTURE: OnceLock<Texture2D> = OnceLock::new();
pub static PORTRAIT_BG_TEXTURE: OnceLock<Texture2D> = OnceLock::new();
pub static PORTRAIT_ENEMY_BG_TEXTURE: OnceLock<Texture2D> = OnceLock::new();
pub static UI_RESOURCE_BAR_TEXTURE: OnceLock<Texture2D> = OnceLock::new();
pub static UI_ACTION_BUTTON_TEXTURE: OnceLock<Texture2D> = OnceLock::new();
pub static UI_ACTION_BUTTON_SMALL_TEXTURE: OnceLock<Texture2D> = OnceLock::new();
pub static UI_ACTION_BUTTON_BG_TEXTURE: OnceLock<Texture2D> = OnceLock::new();
pub static UI_ACTION_BUTTON_SMALL_BG_TEXTURE: OnceLock<Texture2D> = OnceLock::new();
pub static UI_SPELLBOOK_TEXTURE: OnceLock<Texture2D> = OnceLock::new();

async fn load_and_init_ui_textures() {
    load_and_init_texture_into("ui/user_interface.png", &UI_BG_TEXTURE).await;
    load_and_init_texture_into("ui/ui_arrow.png", &UI_ARROW_TEXTURE).await;
    load_and_init_texture_into("ui/spellbook.png", &UI_SPELLBOOK_TEXTURE).await;
    load_and_init_texture_into("ui/resource_bar.png", &UI_RESOURCE_BAR_TEXTURE).await;
    load_and_init_texture_into("ui/action_button.png", &UI_ACTION_BUTTON_TEXTURE).await;
    load_and_init_texture_into(
        "ui/action_button_small.png",
        &UI_ACTION_BUTTON_SMALL_TEXTURE,
    )
    .await;
    load_and_init_texture_into("ui/action_button_bg.png", &UI_ACTION_BUTTON_BG_TEXTURE).await;
    load_and_init_texture_into(
        "ui/action_button_small_bg.png",
        &UI_ACTION_BUTTON_SMALL_BG_TEXTURE,
    )
    .await;
    load_and_init_texture_into("portrait_bg.png", &PORTRAIT_BG_TEXTURE).await;
    load_and_init_texture_into("portrait_enemy_bg.png", &PORTRAIT_ENEMY_BG_TEXTURE).await;
}

pub static BEAM_FX: OnceLock<Texture2D> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BeamFxId {
    Purple,
    Lightning,
    Chains,
}

pub fn draw_beam_fx(from: (f32, f32), to: (f32, f32), id: BeamFxId, texture_draw_size: (f32, f32)) {
    let texture = BEAM_FX.get().unwrap();
    let texture_size = (32.0, 32.0);

    let dist = Vec2::from(from).distance(Vec2::from(to));
    let num_textures = dist / texture_draw_size.0; // + 0.5;

    let num_full_textures = (num_textures).floor() as usize;
    let last_texture_fraction = num_textures.fract();

    let rotation = -(to.0 - from.0).atan2(to.1 - from.1) + 0.5 * PI;

    let params = DrawTextureParams {
        dest_size: Some(texture_draw_size.into()),
        rotation,
        ..Default::default()
    };

    let row_idx = match id {
        BeamFxId::Purple => 0,
        BeamFxId::Lightning => 1,
        BeamFxId::Chains => 2,
    };

    for i in 0..=num_full_textures {
        let mut params = params.clone();
        let center_pos;
        if i < num_full_textures {
            params.source = Some(Rect::new(
                0.0,
                row_idx as f32 * texture_size.1,
                texture_size.0,
                texture_size.1,
            ));
            center_pos = Vec2::from(from)
                .move_towards(Vec2::from(to), texture_draw_size.0 * ((i as f32) + 0.5));
        } else {
            params.source = Some(Rect::new(
                0.0,
                row_idx as f32 * texture_size.1,
                texture_size.0 * last_texture_fraction,
                texture_size.1,
            ));

            params.dest_size = Some(Vec2::new(
                texture_draw_size.0 * last_texture_fraction,
                texture_draw_size.1,
            ));

            //texture_draw_size.0 *= last_partial_texture_w;

            center_pos = Vec2::from(from).move_towards(
                Vec2::from(to),
                texture_draw_size.0 * ((i as f32) + 0.5 * last_texture_fraction),
            );
        }

        let dest_size = params.dest_size.unwrap();

        draw_texture_ex(
            texture,
            center_pos.x - dest_size.x / 2.0,
            center_pos.y - dest_size.y / 2.0,
            WHITE,
            params,
        );
    }
}

async fn load_static_fonts() {
    let simple_font = load_font("delicatus/Delicatus.ttf").await;
    SIMPLE_FONT.set(simple_font).unwrap();
    let big_font = load_font("manaspace/manaspc.ttf").await;
    BIG_FONT.set(big_font).unwrap();
    let decorative_font = load_font("dpcomic/dpcomic.ttf").await;
    DECORATIVE_FONT.set(decorative_font).unwrap();
}

pub static SIMPLE_FONT: OnceLock<Font> = OnceLock::new();
pub static BIG_FONT: OnceLock<Font> = OnceLock::new();
pub static DECORATIVE_FONT: OnceLock<Font> = OnceLock::new();
