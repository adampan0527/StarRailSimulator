//! Synthetic rules and data only. These values are not a character or stage release.

use crate::{Action, BattleError, contract::Config};

pub(crate) const VERSIONS: [(&str, &str); 4] = [
    ("game_version", "0.0"),
    ("rules_version", "fixture.rules.v1"),
    ("data_version", "fixture.data.v1"),
    ("rng_version", "fixture.rng.v1"),
];
pub(crate) const GAUGE_FULL: u64 = 10_000 * 1_000_000 * 1_000_000;
pub(crate) const SKILL_POINT_CAP: u8 = 5;
pub(crate) const TURN_LIMIT: u32 = 256;
pub(crate) const BREAK_DELAY_BP: u16 = 2_500;
pub(crate) const KILL_ENERGY_MILLI: u32 = 10_000;
pub(crate) const BREAK_BASE_AT_LEVEL_30: u32 = 100;

#[derive(Clone, Copy)]
pub(crate) enum Element {
    Physical,
    Wind,
    Ice,
}

impl Element {
    pub const fn index(self) -> usize {
        match self {
            Self::Physical => 0,
            Self::Wind => 1,
            Self::Ice => 2,
        }
    }

    pub const fn break_multiplier_bp(self) -> u16 {
        match self {
            Self::Physical => 20_000,
            Self::Wind => 15_000,
            Self::Ice => 10_000,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Stats {
    pub level: u16,
    pub hp: u32,
    pub attack: u32,
    pub defense: u32,
    pub base_speed_micros: u64,
    pub flat_speed_micros: u64,
    pub resist_bp: [u16; 3],
    pub crit_rate_bp: u16,
}

/// Initial training attributes before any effects are applied in combat.
pub struct TrainingAttributes {
    pub hp: u32,
    pub attack: u32,
    pub defense: u32,
    pub speed_micros: u64,
}

pub fn training_attributes(id: &str, config: &Config) -> Result<TrainingAttributes, BattleError> {
    let (_, stats) = ally(id, config)?;
    Ok(TrainingAttributes {
        hp: stats.hp,
        attack: stats.attack,
        defense: stats.defense,
        speed_micros: stats.base_speed_micros + stats.flat_speed_micros,
    })
}

#[derive(Clone, Copy)]
pub(crate) enum Target {
    Enemy,
    Ally,
    AllEnemies,
    AllAllies,
}

#[derive(Clone, Copy)]
pub(crate) enum Effect {
    Hit {
        element: Element,
        multiplier_bp: u16,
        toughness: u16,
    },
    Shield {
        amount: u32,
        turns: u32,
    },
    Vulnerability {
        bp: u32,
        turns: u32,
    },
    SpeedUp {
        bp: u32,
        turns: u32,
    },
    Heal(u32),
    Advance(u16),
    ImmediateTurn,
}

#[derive(Clone, Copy)]
pub(crate) struct Ability {
    pub target: Target,
    pub energy_milli: u32,
    pub effects: &'static [Effect],
}

pub(crate) struct AllyKit {
    pub stats: Stats,
    pub max_energy_milli: u32,
    basic: Ability,
    skill: Ability,
    ultimate: Ability,
}

impl AllyKit {
    pub const fn ability(&self, action: Action) -> Ability {
        match action {
            Action::Basic => self.basic,
            Action::Skill => self.skill,
            Action::Ultimate => self.ultimate,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum EnemyTarget {
    FirstLiving,
    RandomLiving,
}

pub(crate) struct EnemyKit {
    pub stats: Stats,
    pub max_toughness: u16,
    pub weakness: [bool; 3],
    pub target: EnemyTarget,
    pub hit_energy_milli: u32,
}

const fn ally_stats(attack: u32, speed: u64) -> Stats {
    Stats {
        level: 30,
        hp: 1_000,
        attack,
        defense: 520,
        base_speed_micros: speed * 1_000_000,
        flat_speed_micros: 0,
        resist_bp: [0, 0, 0],
        crit_rate_bp: 0,
    }
}

const fn enemy_stats(hp: u32, resist_bp: [u16; 3]) -> Stats {
    Stats {
        level: 32,
        hp,
        attack: 200,
        defense: 500,
        base_speed_micros: 100_000_000,
        flat_speed_micros: 0,
        resist_bp,
        crit_rate_bp: 0,
    }
}

pub(crate) const DUMMY_STATS: Stats = enemy_stats(1, [0; 3]);

const fn hit(element: Element, multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element,
        multiplier_bp,
        toughness,
    }
}

const fn ability(target: Target, energy: u32, effects: &'static [Effect]) -> Ability {
    Ability {
        target,
        energy_milli: energy * 1_000,
        effects,
    }
}

use Element::{Ice, Physical, Wind};
use Target::{AllAllies, AllEnemies, Ally, Enemy};

static TRAILBLAZER: AllyKit = AllyKit {
    stats: ally_stats(900, 90),
    max_energy_milli: 120_000,
    basic: ability(Enemy, 20, &[hit(Physical, 5_000, 10)]),
    skill: ability(Enemy, 30, &[hit(Physical, 12_000, 20)]),
    ultimate: ability(Enemy, 5, &[hit(Physical, 18_000, 30)]),
};
static MARCH: AllyKit = AllyKit {
    stats: ally_stats(400, 100),
    max_energy_milli: 120_000,
    basic: ability(Enemy, 20, &[hit(Ice, 5_000, 10)]),
    skill: ability(
        Ally,
        30,
        &[Effect::Shield {
            amount: 60,
            turns: 2,
        }],
    ),
    ultimate: ability(AllEnemies, 5, &[hit(Ice, 9_000, 20)]),
};
static DAN_HENG: AllyKit = AllyKit {
    stats: ally_stats(1_000, 125),
    max_energy_milli: 100_000,
    basic: ability(Enemy, 20, &[hit(Wind, 5_000, 10)]),
    skill: ability(
        Enemy,
        30,
        &[
            hit(Wind, 10_000, 20),
            Effect::Vulnerability {
                bp: 2_500,
                turns: 1,
            },
        ],
    ),
    ultimate: ability(Enemy, 5, &[hit(Wind, 18_000, 30)]),
};
static NATASHA: AllyKit = AllyKit {
    stats: ally_stats(600, 80),
    max_energy_milli: 90_000,
    basic: ability(Enemy, 20, &[hit(Physical, 5_000, 10)]),
    skill: ability(Ally, 30, &[Effect::Heal(250)]),
    ultimate: ability(AllAllies, 5, &[Effect::Heal(180)]),
};
static SUPPORT: AllyKit = AllyKit {
    stats: Stats {
        crit_rate_bp: 2_500,
        ..ally_stats(400, 100)
    },
    max_energy_milli: 100_000,
    basic: ability(Enemy, 20, &[hit(Wind, 5_000, 10)]),
    skill: ability(Ally, 30, &[Effect::Advance(2_500)]),
    ultimate: ability(
        Ally,
        5,
        &[
            Effect::ImmediateTurn,
            Effect::SpeedUp {
                bp: 2_500,
                turns: 2,
            },
        ],
    ),
};

static LEFT: EnemyKit = EnemyKit {
    stats: enemy_stats(1_000, [0, 0, 2_000]),
    max_toughness: 20,
    weakness: [true, true, false],
    target: EnemyTarget::FirstLiving,
    hit_energy_milli: 10_000,
};
static RIGHT: EnemyKit = EnemyKit {
    stats: enemy_stats(950, [0, 2_000, 2_000]),
    max_toughness: 10,
    weakness: [true, false, false],
    target: EnemyTarget::RandomLiving,
    hit_energy_milli: 10_000,
};

pub(crate) fn enemy(id: &str, level: u16) -> Result<&'static EnemyKit, BattleError> {
    match (id, level) {
        ("fixture.target_left", 32) => Ok(&LEFT),
        ("fixture.target_right", 32) => Ok(&RIGHT),
        _ => Err(BattleError::UnsupportedData(format!(
            "enemy {id} at level {level}"
        ))),
    }
}

pub(crate) fn ally(id: &str, config: &Config) -> Result<(&'static AllyKit, Stats), BattleError> {
    let kit = match id {
        "trailblazer.destruction" => &TRAILBLAZER,
        "march_7th.preservation" => &MARCH,
        "dan_heng.hunt" => &DAN_HENG,
        "natasha.abundance" => &NATASHA,
        "fixture.support" => &SUPPORT,
        _ => return Err(BattleError::UnsupportedData(format!("character form {id}"))),
    };
    let unsupported = |detail| BattleError::UnsupportedData(format!("{id}: {detail}"));
    if config.level != kit.stats.level
        || config.ascension != 1
        || config.eidolon != 0
        || !config.unlocked_trace_ids.is_empty()
        || config.skill_levels.len() != 4
        || ["basic", "skill", "ultimate", "talent"]
            .iter()
            .any(|&key| config.skill_levels.get(key) != Some(&1))
    {
        return Err(unsupported("character progression"));
    }
    let mut stats = kit.stats;
    if let Some(cone) = &config.light_cone {
        if id != "trailblazer.destruction"
            || cone.data_id != "fixture.destruction_cone"
            || cone.level != 20
            || cone.ascension != 0
            || cone.superimposition != 1
        {
            return Err(unsupported("light cone"));
        }
        stats.attack += 100;
    }
    let relics = &config.relics;
    if [
        &relics.head,
        &relics.hands,
        &relics.body,
        &relics.planar_sphere,
        &relics.link_rope,
    ]
    .into_iter()
    .any(Option::is_some)
    {
        return Err(unsupported("relic slot"));
    }
    if let Some(boots) = &relics.feet {
        if boots.data_id != "fixture.training_boots"
            || boots.level != 3
            || boots.main_stat != "attack_ratio"
            || boots.substats.len() > 1
            || boots
                .substats
                .iter()
                .any(|(name, value)| name != "speed" || !(1..=10_000_000).contains(value))
        {
            return Err(unsupported("training boots"));
        }
        stats.attack = stats.attack * 11 / 10;
        stats.flat_speed_micros = u64::from(boots.substats.get("speed").copied().unwrap_or(0));
    }
    Ok((kit, stats))
}
