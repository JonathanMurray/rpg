use std::{cell::Cell, collections::HashMap, rc::Rc};

use macroquad::{
    audio::{load_sound, play_sound, play_sound_once, stop_sound, PlaySoundParams, Sound},
    rand::ChooseRandom,
};

#[derive(Clone)]
pub struct SoundPlayer {
    sounds: Rc<HashMap<SoundId, Vec<Sound>>>,
    pub enabled: Rc<Cell<bool>>,
}

impl SoundPlayer {
    pub async fn new() -> Self {
        let mut sounds_by_id = HashMap::new();

        for (id, names) in &[
            (
                SoundId::HoverButton,
                vec![
                    "fl_click_1.ogg",
                    "fl_click_2.ogg",
                    "fl_click_3.ogg",
                    "fl_click_4.ogg",
                    "fl_click_5.ogg",
                ],
            ),
            (SoundId::ClickButton, vec!["fl_low_click.ogg"]),
            (SoundId::DragEquipment, vec!["click_2"]),
            (SoundId::DropEquipment, vec!["click_3"]),
            (SoundId::Explosion, vec!["explosion"]),
            (SoundId::FireballHit, vec!["fl_fireball_hit.ogg"]),
            (SoundId::Powerup, vec!["fl_spell_buff.ogg"]),
            (SoundId::BuffBrace, vec!["fl_buff_brace.ogg"]),
            (SoundId::Heal, vec!["fl_heal.ogg"]),
            (SoundId::MeleeAttack, vec!["melee_attack"]),
            (SoundId::AttackMiss, vec!["fl_miss.ogg"]),
            (SoundId::Resist, vec!["fl_resist.ogg"]),
            (SoundId::ArmorAbsorbed, vec!["fl_armor_absorbed.ogg"]),
            (SoundId::ShootArrow, vec!["shoot_arrow_2"]),
            (SoundId::HitArrow, vec!["hit_arrow"]),
            (SoundId::Walk, vec!["walk3"]),
            (SoundId::Debuff, vec!["fl_spell_debuff.ogg"]),
            (SoundId::ShootSpell, vec!["fl_spell_projectile_2.ogg"]),
            (SoundId::Death, vec!["fl_death.ogg"]),
            (SoundId::SheetOpen, vec!["sheet_open"]),
            (SoundId::SheetClose, vec!["sheet_close"]),
            (SoundId::Burning, vec!["fire"]),
            (SoundId::Invalid, vec!["invalid"]),
            (SoundId::EndTurn, vec!["end_turn"]),
            (SoundId::YourTurn, vec!["your_turn3"]),
            //(SoundId::FireCrackle, vec!["looping_effect.ogg"]),
            (SoundId::FireCrackle, vec!["fl_crackling_noise_2.ogg"]),
            (SoundId::MechanicNoise, vec!["fl_wobble.ogg"]),
            (SoundId::SelectTarget, vec!["fl_blip_3.ogg"]),
            (SoundId::HoverTarget, vec!["fl_blip_short_3.ogg"]),
            (SoundId::Scale1, vec!["fl_scale_1.ogg"]),
            (SoundId::Scale2, vec!["fl_scale_2.ogg"]),
            (SoundId::Scale3, vec!["fl_scale_3.ogg"]),
            (SoundId::Scale4, vec!["fl_scale_4.ogg"]),
            (SoundId::Scale5, vec!["fl_scale_5.ogg"]),
        ] {
            let mut sounds = vec![];
            for name in names {
                let name = if name.ends_with(".ogg") {
                    name.to_string()
                } else {
                    name.to_string() + ".wav"
                };
                let sound = load_sound(&format!("sounds/{name}")).await.unwrap();
                sounds.push(sound);
            }
            sounds_by_id.insert(*id, sounds);
        }

        Self {
            sounds: Rc::new(sounds_by_id),
            enabled: Rc::new(Cell::new(true)),
        }
    }

    pub fn play(&self, sound_id: SoundId) {
        if !self.enabled.get() {
            return;
        }
        dbg!(sound_id);
        let sounds = &self.sounds[&sound_id];
        let sound = if sounds.len() == 1 {
            &sounds[0]
        } else {
            sounds.choose().unwrap()
        };
        play_sound_once(sound);
    }

    pub fn play_looping(&self, sound_id: SoundId) {
        if !self.enabled.get() {
            return;
        }
        let sound = &self.sounds[&sound_id][0];
        play_sound(
            sound,
            PlaySoundParams {
                looped: true,
                volume: 1.0,
            },
        );
    }

    pub fn stop(&self, sound_id: SoundId) {
        let sound = &self.sounds[&sound_id][0];
        stop_sound(sound);
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Hash, Eq)]
pub enum SoundId {
    HoverButton,
    ClickButton,
    DragEquipment,
    DropEquipment,
    Explosion,
    FireballHit,
    Powerup,
    BuffBrace,
    Heal,
    MeleeAttack,
    AttackMiss,
    Resist,
    ArmorAbsorbed,
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
    EndTurn,
    YourTurn,
    FireCrackle,
    MechanicNoise,
    SelectTarget,
    HoverTarget,
    Scale1,
    Scale2,
    Scale3,
    Scale4,
    Scale5,
}
