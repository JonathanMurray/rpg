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
    textures::{load_all_sprites, load_and_init_texture, EffectId, PortraitId, Sprite, SpriteId},
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
        let night_time = true;
        if night_time {
            replace_color(&terrain_atlas, |[r, g, b, a]| {
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

fn replace_color(texture: &Texture2D, f: impl Fn([u8; 4]) -> [u8; 4]) {
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
