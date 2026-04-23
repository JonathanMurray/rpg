use macroquad::window::Conf;

use rpg::skill_tree::run_editor;

#[macroquad::main(window_conf)]
async fn main() {
    run_editor().await;
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Editor".to_owned(),
        window_width: 1920,
        //window_height: 960,
        window_height: 1200,
        high_dpi: true,

        window_resizable: false,
        ..Default::default()
    }
}
