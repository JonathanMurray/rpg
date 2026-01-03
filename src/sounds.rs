use std::collections::HashMap;

use macroquad::audio::{load_sound, play_sound, play_sound_once, PlaySoundParams, Sound};

#[derive(Clone)]
pub struct SoundPlayer {
    sounds: HashMap<SoundId, Sound>,
}

impl SoundPlayer {
    pub async fn new() -> Self {
        let mut sounds = HashMap::new();

        for (id, name) in &[
            (SoundId::HoverButton, "click_2"),
            (SoundId::ClickButton, "click_3"),
            (SoundId::DragEquipment, "click_2"),
            (SoundId::DropEquipment, "click_3"),
            (SoundId::Explosion, "explosion"),
            (SoundId::Powerup, "powerup"),
            (SoundId::MeleeAttack, "melee_attack"),
            (SoundId::ShootArrow, "shoot_arrow_2"),
            (SoundId::HitArrow, "hit_arrow"),
            (SoundId::Walk, "walk"),
            (SoundId::Debuff, "debuff"),
            (SoundId::ShootSpell, "shoot_spell"),
            (SoundId::Death, "death"),
            (SoundId::SheetOpen, "sheet_open"),
            (SoundId::SheetClose, "sheet_close"),
            (SoundId::Burning, "fire"),
            (SoundId::Invalid, "invalid"),
        ] {
            let sound = load_sound(&format!("sounds/{name}.wav")).await.unwrap();
            sounds.insert(*id, sound);
        }

        Self { sounds }
    }

    pub fn play(&self, sound_id: SoundId) {
        play_sound_once(&self.sounds[&sound_id]);
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Hash, Eq)]
pub enum SoundId {
    HoverButton,
    ClickButton,
    DragEquipment,
    DropEquipment,
    Explosion,
    Powerup,
    MeleeAttack,
    ShootArrow,
    HitArrow,
    Walk,
    Debuff,
    ShootSpell,
    Death,
    SheetOpen,
    SheetClose,
    Burning,
    Invalid,
}
