use rand::{self, Rng};

pub fn probability_of_d20_reaching(mut target: u32, bonus: DiceRollBonus) -> f32 {
    target = (target as i32 - bonus.flat_amount).min(21).max(1) as u32;

    let advantage_level = bonus.advantage;

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
    //if rand::rng().random_bool(0.5) {20} else {1}
    let mut rng = rand::rng();
    rng.random_range(1..=20)
}

#[derive(Default, Copy, Clone, Debug)]
pub struct DiceRollBonus {
    pub advantage: i32,
    pub flat_amount: i32,
}

impl DiceRollBonus {
    pub fn from_advantage(advantage: i32) -> Self {
        Self {
            advantage,
            flat_amount: 0,
        }
    }

    pub fn none() -> Self {
        Self {
            advantage: 0,
            flat_amount: 0,
        }
    }

    pub fn expected_outcome(&self) -> f32 {
        let expected_roll = expected_roll_with_adv(self.advantage);
        dbg!(self.advantage, expected_roll);
        expected_roll + self.flat_amount as f32
    }
}

pub fn expected_roll_with_adv(advantage: i32) -> f32 {
    if advantage == 0 {
        return 10.5;
    }

    // https://math.stackexchange.com/a/1696755

    // Roll n dice
    let n = (advantage.abs() + 1) as u32;

    // Expected value of the max dice
    let expected_max = {
        let mut subtractor = 0.0;
        for i in 1i32..20i32 {
            subtractor += (i as f32).powf(n as f32);
        }
        subtractor *= 20f32.powf(-(n as f32));
        20.0 - subtractor
    };

    // Expected value of the min dice
    let expected_min = 21.0 - expected_max;

    if advantage > 0 {
        expected_max
    } else {
        expected_min
    }
}
