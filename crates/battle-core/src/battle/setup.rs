use std::{collections::BTreeSet, sync::Arc};

use crate::{
    BattleError, InitialInput,
    fixture::{self, AllyKit, Element, EnemyKit, Stats},
};

use super::{
    enemy::{self, OfficialEnemy, OfficialEnemyKit},
    official::{self, CombatType, OfficialCharacter, OfficialKit},
};

#[derive(Clone)]
pub(super) enum Kit {
    Ally(&'static AllyKit),
    Official(Arc<OfficialKit>),
    Enemy(&'static EnemyKit),
    OfficialEnemy(Arc<OfficialEnemyKit>),
    Memosprite { owner: usize },
    Dummy { max_toughness: u16 },
}

impl Kit {
    pub fn max_energy_milli(&self) -> u32 {
        match self {
            Self::Ally(kit) => kit.max_energy_milli,
            Self::Official(kit) => kit.max_energy_milli,
            Self::Enemy(_)
            | Self::OfficialEnemy(_)
            | Self::Dummy { .. }
            | Self::Memosprite { .. } => 0,
        }
    }

    pub fn skill_point_cap_bonus(&self) -> u8 {
        match self {
            Self::Official(kit) => kit.mechanics.skill_point_cap_bonus(),
            Self::Ally(_)
            | Self::Enemy(_)
            | Self::OfficialEnemy(_)
            | Self::Dummy { .. }
            | Self::Memosprite { .. } => 0,
        }
    }

    pub fn max_toughness(&self) -> u16 {
        match self {
            Self::Ally(_) | Self::Official(_) | Self::Memosprite { .. } => 0,
            Self::Enemy(kit) => kit.max_toughness,
            Self::OfficialEnemy(kit) => kit.max_toughness,
            Self::Dummy { max_toughness } => *max_toughness,
        }
    }

    pub fn matches_weakness(&self, element: Element) -> bool {
        match self {
            Self::Ally(_) | Self::Official(_) | Self::Memosprite { .. } => false,
            Self::Enemy(kit) => kit.weakness[element.index()],
            Self::OfficialEnemy(kit) => kit.weakness[element.index()],
            Self::Dummy { .. } => true,
        }
    }

    pub fn initial_delay_ratio_bp(&self) -> u32 {
        match self {
            Self::OfficialEnemy(kit) => kit.initial_delay_ratio_bp,
            _ => 10_000,
        }
    }

    pub fn starts_dormant(&self) -> bool {
        matches!(self, Self::OfficialEnemy(kit) if kit.starts_dormant)
    }
}

#[derive(Clone)]
pub(super) struct UnitPlan {
    pub id: String,
    pub official_form: Option<String>,
    pub combat_type: Option<CombatType>,
    pub has_control_resistance: bool,
    pub elation_bp: u32,
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
    /// Creates the fixed DPR scenario without modifying the training/replay rules version.
    pub fn official_dpr(
        team: &[OfficialCharacter],
        seed: u32,
        starting_skill_points: u8,
    ) -> Result<Self, BattleError> {
        if !(1..=4).contains(&team.len()) {
            return Err(BattleError::InvalidInput(
                "fixed DPR team size or skill points".into(),
            ));
        }
        let mut ids = BTreeSet::new();
        let mut forms = BTreeSet::new();
        let mut units = Vec::with_capacity(team.len());
        for character in team {
            if !valid_id(&character.unit_id)
                || !ids.insert(&character.unit_id)
                || !forms.insert(&character.form_id)
            {
                return Err(BattleError::InvalidInput(
                    "duplicate or invalid fixed DPR unit".into(),
                ));
            }
            let stats = character.resolved_stats()?;
            let kit = official::build(character)?;
            let starting_energy_milli = kit.max_energy_milli / 2;
            units.push(UnitPlan {
                id: character.unit_id.clone(),
                official_form: Some(character.form_id.clone()),
                combat_type: Some(character.combat_type),
                has_control_resistance: false,
                elation_bp: character.stats.elation_bp,
                kit: Kit::Official(Arc::new(kit)),
                stats,
                starting_energy_milli,
            });
        }
        let skill_point_cap = fixture::SKILL_POINT_CAP
            + units
                .iter()
                .map(|unit| unit.kit.skill_point_cap_bonus())
                .sum::<u8>();
        if starting_skill_points > skill_point_cap {
            return Err(BattleError::InvalidInput(
                "fixed DPR starting skill points".into(),
            ));
        }
        let plan = Self {
            units,
            ally_count: team.len(),
            wave_ends: Vec::new(),
            seed,
            starting_skill_points,
        };
        plan.with_dummies(&super::STANDARD_DPR_SCENARIO.toughness)
    }

    /// Creates an encounter pairing official 4.7.0 characters with verified official enemies.
    pub fn official_encounter(
        team: &[OfficialCharacter],
        waves: &[Vec<OfficialEnemy>],
        seed: u32,
        starting_skill_points: u8,
    ) -> Result<Self, BattleError> {
        if !(1..=4).contains(&team.len())
            || !(1..=8).contains(&waves.len())
            || waves.iter().any(|wave| !(1..=5).contains(&wave.len()))
        {
            return Err(BattleError::InvalidInput(
                "official encounter team size or wave size/count".into(),
            ));
        }
        let mut ids = BTreeSet::new();
        let mut forms = BTreeSet::new();
        let mut units = Vec::with_capacity(team.len() + waves.iter().map(Vec::len).sum::<usize>());
        for character in team {
            if !valid_id(&character.unit_id)
                || !ids.insert(character.unit_id.clone())
                || !forms.insert(character.form_id.clone())
            {
                return Err(BattleError::InvalidInput(
                    "duplicate or invalid official encounter ally".into(),
                ));
            }
            let stats = character.resolved_stats()?;
            let kit = official::build(character)?;
            let starting_energy_milli = kit.max_energy_milli / 2;
            units.push(UnitPlan {
                id: character.unit_id.clone(),
                official_form: Some(character.form_id.clone()),
                combat_type: Some(character.combat_type),
                has_control_resistance: false,
                elation_bp: character.stats.elation_bp,
                kit: Kit::Official(Arc::new(kit)),
                stats,
                starting_energy_milli,
            });
        }
        let skill_point_cap = fixture::SKILL_POINT_CAP
            + units
                .iter()
                .map(|unit| unit.kit.skill_point_cap_bonus())
                .sum::<u8>();
        if starting_skill_points > skill_point_cap {
            return Err(BattleError::InvalidInput(
                "official encounter starting skill points".into(),
            ));
        }
        let mut wave_ends = Vec::with_capacity(waves.len());
        for wave in waves {
            let mut pending_summons: Vec<(String, String, u16)> = Vec::new();
            for enemy_input in wave {
                if !valid_id(&enemy_input.unit_id) || !ids.insert(enemy_input.unit_id.clone()) {
                    return Err(BattleError::InvalidInput(format!(
                        "invalid or duplicate enemy unit ID: {}",
                        enemy_input.unit_id
                    )));
                }
                let (kit, stats) = enemy::build_official_enemy(enemy_input)?;
                for summon_id in &kit.summon_specs {
                    pending_summons.push((
                        enemy_input.unit_id.clone(),
                        summon_id.clone(),
                        enemy_input.level,
                    ));
                }
                units.push(UnitPlan {
                    id: enemy_input.unit_id.clone(),
                    official_form: None,
                    combat_type: None,
                    has_control_resistance: false,
                    elation_bp: 0,
                    kit: Kit::OfficialEnemy(Arc::new(kit)),
                    stats,
                    starting_energy_milli: 0,
                });
            }
            let remaining_slots = 5_usize.saturating_sub(wave.len());
            for (slot, (owner_id, summon_enemy_id, level)) in pending_summons
                .into_iter()
                .take(remaining_slots)
                .enumerate()
            {
                let summon_input = OfficialEnemy {
                    unit_id: format!("{owner_id}:summon:{}", slot + 1),
                    enemy_id: summon_enemy_id,
                    level,
                };
                let (mut kit, stats) = enemy::build_official_enemy(&summon_input)?;
                kit.starts_dormant = true;
                units.push(UnitPlan {
                    id: summon_input.unit_id,
                    official_form: None,
                    combat_type: None,
                    has_control_resistance: false,
                    elation_bp: 0,
                    kit: Kit::OfficialEnemy(Arc::new(kit)),
                    stats,
                    starting_energy_milli: 0,
                });
            }
            wave_ends.push(units.len());
        }
        for owner in 0..team.len() {
            let master = &units[owner];
            let Kit::Official(kit) = &master.kit else {
                continue;
            };
            let Some(profile) = kit.mechanics.memosprite_profile(master.stats) else {
                continue;
            };
            units.push(UnitPlan {
                id: format!("{}:{}", master.id, profile.name),
                official_form: None,
                combat_type: master.combat_type,
                has_control_resistance: false,
                elation_bp: master.elation_bp,
                kit: Kit::Memosprite { owner },
                stats: profile.stats,
                starting_energy_milli: 0,
            });
        }
        Ok(Self {
            units,
            ally_count: team.len(),
            wave_ends,
            seed,
            starting_skill_points,
        })
    }

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
                combat_type: None,
                official_form: None,
                has_control_resistance: false,
                elation_bp: 0,
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
                    combat_type: None,
                    official_form: None,
                    has_control_resistance: false,
                    elation_bp: 0,
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

    pub(super) fn with_dummies(&self, toughness: &[u16]) -> Result<Self, BattleError> {
        if !(1..=5).contains(&toughness.len()) || toughness.contains(&0) {
            return Err(BattleError::InvalidInput(
                "DPR needs one to five dummies with positive toughness".into(),
            ));
        }
        let mut units = self.units[..self.ally_count].to_vec();
        let mut stats = fixture::DUMMY_STATS;
        if units
            .iter()
            .any(|unit| matches!(unit.kit, Kit::Official(_)))
        {
            stats.hp = 100_000;
        }
        units.extend(
            toughness
                .iter()
                .enumerate()
                .map(|(position, &max_toughness)| UnitPlan {
                    combat_type: None,
                    official_form: None,
                    has_control_resistance: false,
                    elation_bp: 0,
                    // The colon keeps internal analysis IDs distinct from input unit IDs.
                    id: format!("dpr:{}", position + 1),
                    kit: Kit::Dummy { max_toughness },
                    stats,
                    starting_energy_milli: 0,
                }),
        );
        let enemy_end = units.len();
        for owner in 0..self.ally_count {
            let master = &units[owner];
            let Kit::Official(kit) = &master.kit else {
                continue;
            };
            let Some(profile) = kit.mechanics.memosprite_profile(master.stats) else {
                continue;
            };
            units.push(UnitPlan {
                id: format!("{}:{}", master.id, profile.name),
                official_form: None,
                combat_type: master.combat_type,
                has_control_resistance: false,
                elation_bp: master.elation_bp,
                kit: Kit::Memosprite { owner },
                stats: profile.stats,
                starting_energy_milli: 0,
            });
        }
        Ok(Self {
            wave_ends: vec![enemy_end],
            units,
            ally_count: self.ally_count,
            seed: self.seed,
            starting_skill_points: self.starting_skill_points,
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
