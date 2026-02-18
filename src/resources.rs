use std::collections::HashMap;

use macroquad::{
    text::{load_ttf_font, Font},
    texture::{FilterMode, Texture2D},
};

use crate::{
    core::CoreGame,
    game_ui::UserInterface,
    game_ui_connection::GameUserInterfaceConnection,
    init_fight_map::GameInitState,
    sounds::SoundPlayer,
    textures::{
        load_all_equipment_icons, load_all_icons, load_all_portraits, load_all_sprites,
        load_all_status_textures, load_and_init_texture, EquipmentIconId, IconId, PortraitId,
        SpriteId, StatusId,
    },
};

#[derive(Clone)]
pub struct GameResources {
    pub sprites: HashMap<SpriteId, Texture2D>,
    pub simple_font: Font,
    pub big_font: Font,
    pub decorative_font: Font,
    pub terrain_atlas: Texture2D,
    pub status_textures: HashMap<StatusId, Texture2D>,
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
        let terrain_atlas = load_and_init_texture("terrain_atlas.png").await;
        let status_textures = load_all_status_textures().await;

        Self {
            sprites,
            simple_font,
            big_font,
            decorative_font,
            terrain_atlas,
            status_textures,
        }
    }
}

#[derive(Clone)]
pub struct UiResources {
    pub equipment_icons: HashMap<EquipmentIconId, Texture2D>,
    pub icons: HashMap<IconId, Texture2D>,
    pub portrait_textures: HashMap<PortraitId, Texture2D>,
}

impl UiResources {
    pub async fn load() -> Self {
        let equipment_icons = load_all_equipment_icons().await;
        let icons = load_all_icons().await;
        let portrait_textures = load_all_portraits().await;

        Self {
            equipment_icons,
            icons,
            portrait_textures,
        }
    }
}

pub fn init_core_game(
    resources: GameResources,
    ui_resources: UiResources,
    sound_player: SoundPlayer,
    init_state: GameInitState,
) -> CoreGame {
    let mut game_ui = GameUserInterfaceConnection::uninitialized();
    let core_game = CoreGame::new(game_ui.clone(), &init_state);
    let gfx_user_interface = UserInterface::new(
        &core_game,
        resources,
        ui_resources,
        init_state,
        sound_player,
    );
    game_ui.init(gfx_user_interface);
    core_game
}

async fn load_font(path: &str) -> Font {
    let path = format!("fonts/{path}");
    let mut font = load_ttf_font(&path).await.unwrap();
    font.set_filter(FilterMode::Nearest);
    font
}
