use rand::{self, Rng};

pub fn probability_of_d20_reaching(target: u32, advantage_level: i32) -> f32 {
    assert!((1..=20).contains(&target));
    let p_miss = (target as f32 - 1f32) / 20f32;
    if advantage_level >= 0 {
        1f32 - p_miss.powi(advantage_level + 1)
    } else {
        (1f32 - p_miss).powi(advantage_level.abs() + 1)
    }
}

pub fn roll_d20_with_advantage(advantage_level: i32) -> u32 {
    // 0 => roll once
    // 1 => roll twice, take highest (i.e. 1x advantage)
    // -1 => roll twice, take lowest (i.e. 1x disadvantage)
    // etc
    let mut res = roll_d20();
    let additional_rolls = advantage_level.abs();
    for _ in 0..additional_rolls {
        let new = roll_d20();
        res = if advantage_level < 0 {
            res.min(new)
        } else {
            assert!(advantage_level > 0);
            res.max(new)
        };
    }
    res
}

fn roll_d20() -> u32 {
    let mut rng = rand::rng();
    rng.random_range(1..=20)
}
