use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use macroquad::{
    audio::{load_sound, play_sound, stop_sound, PlaySoundParams, Sound},
    rand::ChooseRandom,
};

#[derive(Clone)]
pub struct SoundPlayer {
    sounds: Rc<HashMap<SoundId, SoundContainer>>,
    pub enabled: Rc<Cell<bool>>,
    scheduled: Rc<RefCell<Vec<(SoundId, f64)>>>,
    time: f64,
}

struct SoundContainer {
    sounds: Vec<Sound>,
    volume: f32,
}

impl SoundPlayer {
    pub fn noop() -> Self {
        Self {
            sounds: Default::default(),
            enabled: Default::default(),
            scheduled: Default::default(),
            time: 0.0,
        }
    }

    pub async fn load() -> Self {
        Self::load_with_progress(async |_i, _total| {}).await
    }

    pub async fn load_with_progress(on_progress: impl AsyncFn(usize, usize)) -> Self {
        let mut sounds_by_id = HashMap::new();

        let sound_definitions = [
            (SoundId::Execute, 1.6, vec!["fl_execute.ogg"]),
            (SoundId::Coin, 0.05, vec!["coin"]),
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
            (SoundId::Explosion, 0.4, vec!["explosion"]),
            (SoundId::ShieldBash, 1.5, vec!["fl_shield_bash.ogg"]),
            // Close enough
            (SoundId::Collision, 1.5, vec!["fl_shield_bash.ogg"]),
            (SoundId::SweepAttack, 1.3, vec!["fl_sweep_attack.ogg"]),
            (SoundId::FireballHit, 0.8, vec!["fl_fireball_hit.ogg"]),
            (SoundId::LightningHit, 0.25, vec!["fl_lightning_hit.ogg"]),
            (SoundId::Crit, 0.15, vec!["fl_crit.ogg"]),
            (SoundId::Powerup, 0.4, vec!["fl_spell_buff.ogg"]),
            (SoundId::Gulp, 0.4, vec!["fl_gulp.ogg"]),
            (SoundId::BuffBrace, 1.0, vec!["fl_buff_brace.ogg"]),
            (SoundId::Heal, 0.7, vec!["fl_heal.ogg"]),
            (SoundId::MeleeAttack, 0.3, vec!["melee_attack"]),
            (SoundId::AttackMiss, 0.3, vec!["fl_miss.ogg"]),
            (SoundId::Resist, 1.0, vec!["fl_resist.ogg"]),
            (SoundId::ArmorAbsorbed, 1.0, vec!["fl_armor_absorbed.ogg"]),
            //(SoundId::ShootArrow, 1.0, vec!["shoot_arrow_2"]),
            (SoundId::ShootArrow, 1.0, vec!["fl_shoot_arrow.ogg"]),
            //(SoundId::HitArrow, 1.0, vec!["hit_arrow"]),
            (SoundId::HitArrow, 1.0, vec!["fl_thump.ogg"]),
            (SoundId::UndeadRise, 1.0, vec!["fl_undead_rise.ogg"]),
            (
                SoundId::Walk,
                1.2,
                vec![
                    "fl_footsteps_1.ogg",
                    "fl_footsteps_2.ogg",
                    "fl_footsteps_3.ogg",
                    "fl_footsteps_4.ogg",
                ],
            ),
            (
                SoundId::WalkWater,
                0.6,
                vec!["fl_splash.ogg", "fl_splash_4.ogg", "fl_splash_3.ogg"],
            ),
            (SoundId::Debuff, 0.5, vec!["fl_spell_debuff.ogg"]),
            (SoundId::BottleSplash, 0.3, vec!["fl_bottle_splash.ogg"]),
            (SoundId::NoisyCrash, 0.15, vec!["fl_noisy_crash.ogg"]),
            //(SoundId::ShootSpell, 1.0, vec!["fl_spell_projectile_2.ogg"]),
            (SoundId::Dash, 0.3, vec!["fl_spell_projectile_2.ogg"]),
            (SoundId::ShootSpell, 0.7, vec!["fl_shoot_swoosh.ogg"]),
            (SoundId::ThrowGrenade, 0.2, vec!["fl_falling_whistle.ogg"]),
            (
                SoundId::StartChannelingAbility,
                0.8,
                vec!["fl_swoosh_2.ogg"],
            ),
            (SoundId::OpenBottle, 0.6, vec!["fl_bottle_open.ogg"]),
            (SoundId::MetalTool, 0.4, vec!["fl_metal_tool.ogg"]),
            (SoundId::MetalEquipment, 0.4, vec!["fl_metal_equipment.ogg"]),
            //(SoundId::Death, 1.0, vec!["fl_death.ogg"]),
            (SoundId::Death, 1.0, vec!["fl_death_2.ogg"]),
            (SoundId::Ding, 1.0, vec!["fl_ding.ogg"]),
            //(SoundId::SheetOpen, 1.0, vec!["sheet_open"]),
            (SoundId::SheetOpen, 0.7, vec!["fl_page_open.ogg"]),
            //(SoundId::SheetClose, 1.0, vec!["sheet_close"]),
            (SoundId::SheetClose, 0.7, vec!["fl_page_close2.ogg"]),
            (SoundId::Burning, 0.15, vec!["fire"]),
            (SoundId::Invalid, 0.3, vec!["invalid"]),
            (SoundId::EndTurn, 1.0, vec!["end_turn"]),
            (SoundId::YourTurn, 0.3, vec!["your_turn3"]),
            (SoundId::ChooseReaction, 0.3, vec!["fl_blip_2.ogg"]),
            //(SoundId::FireCrackle, 1.0, vec!["looping_effect.ogg"]),
            (SoundId::FireCrackle, 1.0, vec!["fl_crackling_noise_2.ogg"]),
            (SoundId::Poison, 1.0, vec!["fl_poison.ogg"]),
            (SoundId::MechanicNoise, 1.0, vec!["fl_wobble.ogg"]),
            (SoundId::SelectTarget, 0.5, vec!["fl_blip_3.ogg"]),
            (
                SoundId::GainedResource,
                0.7,
                vec!["fl_happy_blip_shortest.ogg"],
            ),
            (SoundId::HoverTarget, 1.0, vec!["fl_blip_short_3.ogg"]),
            (SoundId::Scale1, 0.5, vec!["fl_scale_1.ogg"]),
            (SoundId::Scale2, 0.5, vec!["fl_scale_2.ogg"]),
            (SoundId::Scale3, 0.5, vec!["fl_scale_3.ogg"]),
            (SoundId::Scale4, 0.5, vec!["fl_scale_4.ogg"]),
            (SoundId::Scale5, 0.5, vec!["fl_scale_5.ogg"]),
            (
                SoundId::Damage,
                1.0,
                vec![
                    "fl_damage_4.ogg",
                    "fl_damage_a1.ogg",
                    "fl_damage_a2.ogg",
                    "fl_damage_a3.ogg",
                    "fl_damage_a4.ogg",
                ],
            ),
            (
                SoundId::DamageHuldra,
                1.0,
                vec![
                    "fl_damage_b1.ogg",
                    "fl_damage_b2.ogg",
                    "fl_damage_b3.ogg",
                    "fl_damage_b4.ogg",
                ],
            ),
            (
                SoundId::DamageOgre,
                1.0,
                vec!["fl_damage_c1.ogg", "fl_damage_c2.ogg", "fl_damage_c3.ogg"],
            ),
            (SoundId::DamageBob, 1.0, vec!["fl_damage_5.ogg"]),
            (SoundId::DamageFemale, 1.0, vec!["fl_damage_8.ogg"]),
            (SoundId::React, 1.0, vec!["fl_react.ogg"]),
            (SoundId::Victory, 1.0, vec!["fl_fanfare.ogg"]),
            (SoundId::Defeat, 1.0, vec!["fl_defeat.ogg"]),
            (SoundId::Battle, 1.0, vec!["fl_battle.ogg"]),
            (SoundId::Laugh, 0.8, vec!["fl_laugh.ogg"]),
            (SoundId::Infect, 0.8, vec!["fl_infect.ogg"]),
        ];

        let total = sound_definitions.len();
        on_progress(0, total).await;
        for (i, (id, volume, names)) in sound_definitions.iter().enumerate() {
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
            on_progress(i + 1, total).await;
        }

        Self {
            sounds: Rc::new(sounds_by_id),
            enabled: Rc::new(Cell::new(true)),
            scheduled: Default::default(),
            time: 0.0,
        }
    }

    pub fn play(&self, sound_id: SoundId) {
        dbg!((sound_id, self.time));
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

    pub fn schedule(&self, sound_id: SoundId, delay: f32) {
        if delay == 0.0 {
            self.play(sound_id);
        } else {
            let target_time = self.time + delay as f64;

            let mut scheduled = self.scheduled.borrow_mut();
            for (other_sound, other_target_time) in scheduled.iter() {
                if sound_id == *other_sound {
                    let dt = target_time - other_target_time;
                    if dt.abs() < 0.1 {
                        println!("Skipping sound '{:?}'. Found identical sound, scheduled at relative time {}", sound_id, dt);
                        return;
                    }
                }
            }

            scheduled.push((sound_id, target_time));
        }
    }

    pub fn update(&mut self, elapsed: f32) {
        self.time += elapsed as f64;
        let mut scheduled = self.scheduled.borrow_mut();
        for (sound_id, target_time) in scheduled.iter() {
            if self.time >= *target_time {
                self.play(*sound_id);
            }
        }
        scheduled.retain(|(_sound_id, target_time)| *target_time > self.time);
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
        if self.enabled.get() {
            let sound = &self.sounds[&sound_id].sounds[0];
            stop_sound(sound);
        }
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
    Collision,
    SweepAttack,
    FireballHit,
    LightningHit,
    Crit,
    Powerup,
    Gulp,
    BuffBrace,
    Heal,
    MeleeAttack,
    AttackMiss,
    Resist,
    ArmorAbsorbed,
    ShootArrow,
    HitArrow,
    UndeadRise,
    Walk,
    WalkWater,
    Debuff,
    BottleSplash,
    NoisyCrash,
    ShootSpell,
    ThrowGrenade,
    StartChannelingAbility,
    OpenBottle,
    MetalTool,
    MetalEquipment,
    Dash,
    Death,
    Ding,
    SheetOpen,
    SheetClose,
    Burning,
    Invalid,
    EndTurn,
    YourTurn,
    ChooseReaction,
    FireCrackle,
    Poison,
    MechanicNoise,
    SelectTarget,
    HoverTarget,
    GainedResource,
    Scale1,
    Scale2,
    Scale3,
    Scale4,
    Scale5,
    Damage,
    DamageHuldra,
    DamageOgre,
    DamageBob,
    DamageFemale,
    React,
    Victory,
    Defeat,
    Battle,
    Laugh,
    Infect,
}
