use std::collections::BTreeSet;

use crate::{
    BattleError, InitialInput,
    fixture::{self, AllyKit, EnemyKit, Stats},
};

#[derive(Clone, Copy)]
pub(super) enum Kit {
    Ally(&'static AllyKit),
    Enemy(&'static EnemyKit),
}

impl Kit {
    pub fn max_energy_milli(self) -> u32 {
        match self {
            Self::Ally(kit) => kit.max_energy_milli,
            Self::Enemy(_) => 0,
        }
    }

    pub fn max_toughness(self) -> u16 {
        match self {
            Self::Ally(_) => 0,
            Self::Enemy(kit) => kit.max_toughness,
        }
    }
}

pub(super) struct UnitPlan {
    pub id: String,
    pub kit: Kit,
    pub stats: Stats,
    pub starting_energy_milli: u32,
}

/// Validated, owned input. Preparing a new session is the only step that resolves data IDs.
pub struct PreparedBattle {
    pub(super) units: Vec<UnitPlan>,
    pub(super) ally_count: usize,
    pub(super) wave_ends: Vec<usize>,
    pub(super) seed: u32,
    pub(super) starting_skill_points: u8,
}

impl PreparedBattle {
    pub fn new(input: &InitialInput) -> Result<Self, BattleError> {
        let actual = [
            &input.game_version,
            &input.rules_version,
            &input.data_version,
            &input.rng_version,
        ];
        for ((field, supported), value) in fixture::VERSIONS.into_iter().zip(actual) {
            if supported != value {
                return Err(BattleError::UnsupportedVersion {
                    field,
                    value: value.clone(),
                });
            }
        }
        if input.stage_id != "fixture.two_targets" {
            return Err(BattleError::UnsupportedData(format!(
                "stage {}",
                input.stage_id
            )));
        }
        if !(1..=4).contains(&input.team.len())
            || !(1..=8).contains(&input.waves.len())
            || input
                .waves
                .iter()
                .any(|wave| !(1..=5).contains(&wave.len()))
            || input.starting_skill_points > u32::from(fixture::SKILL_POINT_CAP)
        {
            return Err(BattleError::InvalidInput(
                "team size, wave size/count, or starting skill points".into(),
            ));
        }

        let mut ids = BTreeSet::new();
        let mut add_id = |id: &str| -> Result<String, BattleError> {
            if !valid_id(id) || !ids.insert(id.to_owned()) {
                return Err(BattleError::InvalidInput(format!(
                    "invalid or duplicate unit ID: {id}"
                )));
            }
            Ok(id.into())
        };
        let mut units = Vec::new();
        for ally in &input.team {
            let id = add_id(&ally.unit_id)?;
            let (kit, stats) = fixture::ally(&ally.character_form_id, &ally.config)?;
            if ally.starting_energy_milli > kit.max_energy_milli {
                return Err(BattleError::InvalidInput(format!(
                    "starting energy exceeds maximum for {id}"
                )));
            }
            units.push(UnitPlan {
                id,
                kit: Kit::Ally(kit),
                stats,
                starting_energy_milli: ally.starting_energy_milli,
            });
        }
        let mut wave_ends = Vec::with_capacity(input.waves.len());
        for wave in &input.waves {
            for enemy in wave {
                let id = add_id(&enemy.unit_id)?;
                let kit = fixture::enemy(&enemy.enemy_id, enemy.level)?;
                units.push(UnitPlan {
                    id,
                    kit: Kit::Enemy(kit),
                    stats: kit.stats,
                    starting_energy_milli: 0,
                });
            }
            wave_ends.push(units.len());
        }
        Ok(Self {
            units,
            ally_count: input.team.len(),
            wave_ends,
            seed: input.seed,
            starting_skill_points: input.starting_skill_points as u8,
        })
    }
}

fn valid_id(id: &str) -> bool {
    id.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && id.split('.').all(|part| {
            part.as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
                && part.bytes().all(|byte| {
                    byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte)
                })
        })
}
