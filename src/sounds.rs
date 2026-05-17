use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use macroquad::{
    audio::{load_sound, play_sound, stop_sound, PlaySoundParams, Sound},
    rand::ChooseRandom,
    time::get_time,
};
use serde::de;

#[derive(Clone)]
pub struct SoundPlayer {
    sounds: Rc<HashMap<SoundId, SoundContainer>>,
    pub enabled: Rc<Cell<bool>>,
    queued: Rc<RefCell<Vec<(SoundId, f64)>>>,
}

struct SoundContainer {
    sounds: Vec<Sound>,
    volume: f32,
}

impl SoundPlayer {
    pub async fn new() -> Self {
        let mut sounds_by_id = HashMap::new();

        for (id, volume, names) in &[
            (SoundId::Execute, 1.6, vec!["fl_execute.ogg"]),
            (SoundId::Coin, 0.2, vec!["coin"]),
            (
                SoundId::HoverButton,
                1.0,
                vec![
                    "fl_click_1.ogg",
                    "fl_click_2.ogg",
                    "fl_click_3.ogg",
                    "fl_click_4.ogg",
                    "fl_click_5.ogg",
                ],
            ),
            (SoundId::ClickButton, 1.0, vec!["fl_low_click.ogg"]),
            (SoundId::DragEquipment, 1.0, vec!["click_2"]),
            (SoundId::DropEquipment, 1.0, vec!["click_3"]),
            (SoundId::Explosion, 1.0, vec!["explosion"]),
            (SoundId::ShieldBash, 1.5, vec!["fl_shield_bash.ogg"]),
            (SoundId::SweepAttack, 1.0, vec!["fl_sweep_attack.ogg"]),
            (SoundId::FireballHit, 0.6, vec!["fl_fireball_hit.ogg"]),
            (SoundId::LightningHit, 0.6, vec!["fl_lightning_hit.ogg"]),
            (SoundId::Powerup, 1.0, vec!["fl_spell_buff.ogg"]),
            (SoundId::BuffBrace, 1.0, vec!["fl_buff_brace.ogg"]),
            (SoundId::Heal, 1.0, vec!["fl_heal.ogg"]),
            (SoundId::MeleeAttack, 0.3, vec!["melee_attack"]),
            (SoundId::AttackMiss, 1.0, vec!["fl_miss.ogg"]),
            (SoundId::Resist, 1.0, vec!["fl_resist.ogg"]),
            (SoundId::ArmorAbsorbed, 1.0, vec!["fl_armor_absorbed.ogg"]),
            //(SoundId::ShootArrow, 1.0, vec!["shoot_arrow_2"]),
            (SoundId::ShootArrow, 1.0, vec!["fl_shoot_arrow.ogg"]),
            //(SoundId::HitArrow, 1.0, vec!["hit_arrow"]),
            (SoundId::HitArrow, 1.0, vec!["fl_thump.ogg"]),
            (SoundId::Walk, 1.0, vec!["walk3"]),
            (SoundId::Debuff, 1.0, vec!["fl_spell_debuff.ogg"]),
            //(SoundId::ShootSpell, 1.0, vec!["fl_spell_projectile_2.ogg"]),
            (SoundId::ShootSpell, 1.0, vec!["fl_shoot_swoosh.ogg"]),
            //(SoundId::Death, 1.0, vec!["fl_death.ogg"]),
            (SoundId::Death, 1.0, vec!["fl_death_2.ogg"]),
            (SoundId::Ding, 1.0, vec!["fl_ding.ogg"]),
            //(SoundId::SheetOpen, 1.0, vec!["sheet_open"]),
            (SoundId::SheetOpen, 0.7, vec!["fl_page_open.ogg"]),
            //(SoundId::SheetClose, 1.0, vec!["sheet_close"]),
            (SoundId::SheetClose, 0.7, vec!["fl_page_close2.ogg"]),
            (SoundId::Burning, 1.0, vec!["fire"]),
            (SoundId::Invalid, 0.3, vec!["invalid"]),
            (SoundId::EndTurn, 1.0, vec!["end_turn"]),
            (SoundId::YourTurn, 0.4, vec!["your_turn3"]),
            //(SoundId::FireCrackle, 1.0, vec!["looping_effect.ogg"]),
            (SoundId::FireCrackle, 1.0, vec!["fl_crackling_noise_2.ogg"]),
            (SoundId::Poison, 1.0, vec!["fl_poison.ogg"]),
            (SoundId::MechanicNoise, 1.0, vec!["fl_wobble.ogg"]),
            (SoundId::SelectTarget, 1.0, vec!["fl_blip_3.ogg"]),
            (SoundId::GainedAP, 1.0, vec!["fl_blip_3.ogg"]),
            (SoundId::HoverTarget, 1.0, vec!["fl_blip_short_3.ogg"]),
            (SoundId::Scale1, 1.0, vec!["fl_scale_1.ogg"]),
            (SoundId::Scale2, 1.0, vec!["fl_scale_2.ogg"]),
            (SoundId::Scale3, 1.0, vec!["fl_scale_3.ogg"]),
            (SoundId::Scale4, 1.0, vec!["fl_scale_4.ogg"]),
            (SoundId::Scale5, 1.0, vec!["fl_scale_5.ogg"]),
            (
                SoundId::Damage,
                1.0,
                vec!["fl_damage_4.ogg", "fl_damage_7.ogg"],
            ),
            (SoundId::DamageBob, 1.0, vec!["fl_damage_5.ogg"]),
            (SoundId::DamageFemale, 1.0, vec!["fl_damage_8.ogg"]),
            (SoundId::Victory, 1.0, vec!["fl_fanfare.ogg"]),
            (SoundId::Defeat, 1.0, vec!["fl_defeat.ogg"]),
            (SoundId::Battle, 1.0, vec!["fl_battle.ogg"]),
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
            sounds_by_id.insert(
                *id,
                SoundContainer {
                    sounds,
                    volume: *volume,
                },
            );
        }

        Self {
            sounds: Rc::new(sounds_by_id),
            enabled: Rc::new(Cell::new(true)),
            queued: Default::default(),
        }
    }

    pub fn play(&self, sound_id: SoundId) {
        if !self.enabled.get() {
            return;
        }
        let container = &self.sounds[&sound_id];
        let sounds = &container.sounds;
        let sound = if sounds.len() == 1 {
            &sounds[0]
        } else {
            sounds.choose().unwrap()
        };

        play_sound(
            sound,
            PlaySoundParams {
                looped: false,
                volume: container.volume,
            },
        );
    }

    pub fn play_delayed(&self, sound_id: SoundId, delay: f64) {
        if delay == 0.0 {
            self.play(sound_id);
        } else {
            let target_time = get_time() + delay;
            self.queued.borrow_mut().push((sound_id, target_time));
        }
    }

    pub fn update(&self) {
        let mut queued = self.queued.borrow_mut();
        let t = get_time();
        for (sound_id, target_time) in queued.iter() {
            if t >= *target_time {
                self.play(*sound_id);
            }
        }
        queued.retain(|(_sound_id, target_time)| *target_time > t);
    }

    pub fn play_looping(&self, sound_id: SoundId) {
        if !self.enabled.get() {
            return;
        }

        let container = &self.sounds[&sound_id];
        let sound = &container.sounds[0];
        play_sound(
            sound,
            PlaySoundParams {
                looped: true,
                volume: container.volume,
            },
        );
    }

    pub fn stop(&self, sound_id: SoundId) {
        let sound = &self.sounds[&sound_id].sounds[0];
        stop_sound(sound);
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Hash, Eq)]
pub enum SoundId {
    Execute,
    Coin,
    HoverButton,
    ClickButton,
    DragEquipment,
    DropEquipment,
    Explosion,
    ShieldBash,
    SweepAttack,
    FireballHit,
    LightningHit,
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
    Ding,
    SheetOpen,
    SheetClose,
    Burning,
    Invalid,
    EndTurn,
    YourTurn,
    FireCrackle,
    Poison,
    MechanicNoise,
    SelectTarget,
    HoverTarget,
    GainedAP,
    Scale1,
    Scale2,
    Scale3,
    Scale4,
    Scale5,
    Damage,
    DamageBob,
    DamageFemale,
    Victory,
    Defeat,
    Battle,
}
