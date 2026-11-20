mod herta;

use std::{collections::BTreeMap, sync::Arc};

use super::Battle;
use crate::{
    Action, BattleError, DamageKind,
    fixture::{Ability, Effect, Stats, Target},
};

/// Static stats resolved by the profile layer before a fixed-target simulation.
pub struct OfficialStats {
    pub hp: u32,
    pub attack: u32,
    pub base_attack: u32,
    pub defense: u32,
    pub base_speed_micros: u64,
    pub speed_micros: u64,
    pub crit_rate_bp: u32,
    pub crit_damage_bp: u32,
    pub element_damage_bp: u32,
    pub break_effect_bp: u32,
    pub effect_hit_rate_bp: u32,
    pub energy_regeneration_bp: u32,
}

/// Frozen effective character parameters from the supported game data version.
pub struct OfficialCharacter {
    pub unit_id: String,
    pub form_id: String,
    pub energy_capacity: u32,
    pub level: u16,
    pub stats: OfficialStats,
    pub skill_parameters: BTreeMap<String, Vec<f64>>,
    pub eidolon: u8,
    pub enabled_trace_ids: Vec<String>,
}

impl OfficialCharacter {
    pub(super) fn parameters(&self, id: &str, len: usize) -> Result<&[f64], BattleError> {
        self.skill_parameters
            .get(id)
            .filter(|values| {
                values.len() == len
                    && values
                        .iter()
                        .all(|value| value.is_finite() && *value >= 0.0)
            })
            .map(Vec::as_slice)
            .ok_or_else(|| BattleError::InvalidInput(format!("{}: {id} parameters", self.form_id)))
    }

    pub(super) fn resolved_stats(&self) -> Result<Stats, BattleError> {
        let stats = &self.stats;
        if self.level != 80
            || self.eidolon > 6
            || self.energy_capacity == 0
            || self.energy_capacity > 999
            || stats.hp == 0
            || stats.hp > 10_000_000
            || stats.attack == 0
            || stats.attack > 10_000_000
            || stats.base_attack == 0
            || stats.base_attack > 10_000_000
            || stats.base_speed_micros == 0
            || stats.speed_micros < stats.base_speed_micros
            || stats.speed_micros > 1_000_000_000
            || [
                stats.crit_rate_bp,
                stats.crit_damage_bp,
                stats.element_damage_bp,
                stats.break_effect_bp,
                stats.effect_hit_rate_bp,
                stats.energy_regeneration_bp,
            ]
            .into_iter()
            .any(|ratio| ratio > 1_000_000)
        {
            return Err(BattleError::InvalidInput(format!(
                "{}: fixed DPR character attributes or level",
                self.form_id
            )));
        }
        Ok(Stats {
            level: self.level,
            hp: stats.hp,
            attack: stats.attack,
            base_attack: stats.base_attack,
            defense: stats.defense,
            base_speed_micros: stats.base_speed_micros,
            flat_speed_micros: stats.speed_micros - stats.base_speed_micros,
            resist_bp: [0; 4],
            crit_rate_bp: stats.crit_rate_bp,
            crit_damage_bp: stats.crit_damage_bp,
            element_damage_bp: stats.element_damage_bp,
            break_effect_bp: stats.break_effect_bp,
            effect_hit_rate_bp: stats.effect_hit_rate_bp,
            energy_regeneration_bp: stats.energy_regeneration_bp,
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum SkillPoints {
    Generate,
    Spend,
    Unchanged,
}

pub(super) struct OfficialAbility {
    pub target: Target,
    pub energy_milli: u32,
    pub effects: Vec<Effect>,
    pub skill_points: SkillPoints,
}

pub(super) struct OfficialKit {
    pub max_energy_milli: u32,
    pub basic: OfficialAbility,
    pub skill: OfficialAbility,
    pub ultimate: OfficialAbility,
    pub mechanics: Arc<dyn Mechanics>,
}

impl OfficialKit {
    pub fn ability(&self, action: Action) -> (Ability<'_>, SkillPoints) {
        let ability = match action {
            Action::Basic => &self.basic,
            Action::Skill => &self.skill,
            Action::Ultimate => &self.ultimate,
        };
        (
            Ability {
                target: ability.target,
                energy_milli: ability.energy_milli,
                effects: &ability.effects,
            },
            ability.skill_points,
        )
    }
}

pub(super) trait Mechanics: Send + Sync {
    fn on_battle_start(&self, _battle: &mut Battle<'_>, _actor: usize) {}

    fn before_action(
        &self,
        _battle: &mut Battle<'_>,
        _actor: usize,
        _action: Action,
        _target: Option<usize>,
    ) {
    }

    fn after_action(
        &self,
        _battle: &mut Battle<'_>,
        _actor: usize,
        _action: Action,
        _target: Option<usize>,
    ) {
    }

    fn after_attack_hit(
        &self,
        _battle: &mut Battle<'_>,
        _actor: usize,
        _target: usize,
        _action: Action,
        _hp_before: u32,
    ) {
    }

    fn after_attack(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _source: usize,
        _crossed_half: &[usize],
        _origin: Action,
    ) {
    }

    fn damage_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        _target: usize,
        _action: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        0
    }

    fn blast_multiplier_bp(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        _action: Option<Action>,
        multiplier_bp: u16,
    ) -> u16 {
        multiplier_bp
    }

    fn after_defeat(&self, _battle: &mut Battle<'_>, _actor: usize, _target: usize) {}
}

// Register supported 4.7.0 character kits here after their acceptance review.
pub const fn supports_official_dpr(form_id: &str) -> bool {
    matches!(form_id.as_bytes(), b"character.1013")
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    match input.form_id.as_str() {
        "character.1013" => herta::build(input),
        id => Err(BattleError::UnsupportedData(format!("character form {id}"))),
    }
}
