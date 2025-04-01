use std::collections::HashMap;

use macroquad::texture::{load_texture, FilterMode, Texture2D};

#[derive(Hash, PartialEq, Eq, Copy, Clone, Debug)]
pub enum SpriteId {
    Character,
    Character2,
    Character3,
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
    Banshee,
    Dualcast,
    AllIn,
    Plus,
    PlusPlus,
    Go,
}

pub async fn load_all_icons() -> HashMap<IconId, Texture2D> {
    load_icons(vec![
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
        (IconId::Plus, "plus_icon.png"),
        (IconId::PlusPlus, "plus_plus_icon.png"),
    ])
    .await
}

async fn load_sprites(paths: Vec<(SpriteId, &str)>) -> HashMap<SpriteId, Texture2D> {
    let mut textures: HashMap<SpriteId, Texture2D> = Default::default();
    for (id, path) in paths {
        textures.insert(id, load_and_init_texture(path).await);
    }
    textures
}

pub async fn load_icons(paths: Vec<(IconId, &str)>) -> HashMap<IconId, Texture2D> {
    let mut textures: HashMap<IconId, Texture2D> = Default::default();
    for (id, path) in paths {
        textures.insert(id, load_and_init_texture(path).await);
    }
    textures
}

pub async fn load_and_init_texture(path: &str) -> Texture2D {
    let texture = load_texture(path).await.unwrap();
    texture.set_filter(FilterMode::Nearest);
    texture
}
