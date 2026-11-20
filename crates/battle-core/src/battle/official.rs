mod arlan;
mod asta;
mod bronya;
mod clara;
mod dan_heng;
mod gepard;
mod guinaifen;
mod herta;
mod himeko;
mod hook;
mod kafka;
mod luka;
mod march_preservation;
mod natasha;
mod pela;
mod sampo;
mod serval;
mod sushang;
mod tingyun;
mod yanqing;

use std::{collections::BTreeMap, sync::Arc};

use super::{Battle, periodic::DotEvent};
use crate::{
    Action, BattleError, DamageKind,
    fixture::{Ability, Effect, Element, Stats, Target},
};

/// Static stats resolved by the profile layer before a fixed-target simulation.
pub struct OfficialStats {
    pub hp: u32,
    pub attack: u32,
    pub base_attack: u32,
    pub defense: u32,
    pub base_defense: u32,
    pub base_speed_micros: u64,
    pub speed_micros: u64,
    pub crit_rate_bp: u32,
    pub crit_damage_bp: u32,
    pub element_damage_bp: u32,
    pub thunder_damage_bp: Option<u32>,
    pub break_effect_bp: u32,
    pub effect_hit_rate_bp: u32,
    pub energy_regeneration_bp: u32,
    pub healing_bp: u32,
    pub effect_resistance_bp: u32,
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
            || stats.hp > 100_000
            || stats.attack == 0
            || stats.attack > 100_000
            || stats.base_attack == 0
            || stats.base_attack > 100_000
            || stats.defense > 100_000
            || stats.base_defense > 100_000
            || stats.base_speed_micros == 0
            || stats.speed_micros < stats.base_speed_micros
            || stats.speed_micros > 1_000_000_000
            || [
                stats.crit_rate_bp,
                stats.crit_damage_bp,
                stats.element_damage_bp,
                stats.thunder_damage_bp.unwrap_or_default(),
                stats.break_effect_bp,
                stats.effect_hit_rate_bp,
                stats.energy_regeneration_bp,
                stats.healing_bp,
                stats.effect_resistance_bp,
            ]
            .into_iter()
            .any(|ratio| ratio > 100_000)
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
            base_defense: stats.base_defense,
            base_speed_micros: stats.base_speed_micros,
            flat_speed_micros: stats.speed_micros - stats.base_speed_micros,
            resist_bp: [0; Element::COUNT],
            crit_rate_bp: stats.crit_rate_bp,
            crit_damage_bp: stats.crit_damage_bp,
            element_damage_bp: stats.element_damage_bp,
            thunder_damage_bp: stats.thunder_damage_bp,
            break_effect_bp: stats.break_effect_bp,
            effect_hit_rate_bp: stats.effect_hit_rate_bp,
            effect_resistance_bp: stats.effect_resistance_bp,
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

impl OfficialAbility {
    pub(super) fn resolve(&self) -> (Ability<'_>, SkillPoints) {
        (
            Ability {
                target: self.target,
                energy_milli: self.energy_milli,
                effects: &self.effects,
            },
            self.skill_points,
        )
    }
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
        ability.resolve()
    }
}

#[derive(Clone, Default)]
pub(super) struct SushangState {
    pub sword_stance_stacks: u8,
    pub ultimate_expires_after_turn: u32,
    pub damage_reduction_expires_after_turn: u32,
    pub speed_expirations: [u32; 2],
    pub target_was_broken: bool,
}

#[derive(Clone, Default)]
pub(super) struct YanqingState {
    pub soulsteel_expires_after_turn: u32,
    pub ultimate_expires_after_turn: u32,
    pub ultimate_crit_damage: bool,
    pub target_was_frozen: bool,
}

pub(super) trait Mechanics: Send + Sync {
    fn ability_override(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        _action: Action,
    ) -> Option<&OfficialAbility> {
        None
    }

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

    fn on_enemy_defeat(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _killer: usize,
        _target: usize,
    ) {
    }

    fn on_ally_targeted(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _source: usize,
        _action: Action,
    ) {
    }

    fn heal_over_time(
        &self,
        _battle: &mut Battle<'_>,
        _actor: usize,
        _target: usize,
        _origin: Action,
    ) {
    }

    fn after_enemy_attack(&self, _battle: &mut Battle<'_>, _target: usize) {}

    fn after_critical_hit(
        &self,
        _battle: &mut Battle<'_>,
        _actor: usize,
        _target: usize,
        _action: Action,
    ) {
    }

    fn crit_rate_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        _target: usize,
        _action: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        0
    }

    fn damage_multiplier_bp(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        _target: usize,
        _action: Option<Action>,
        _kind: DamageKind,
        base: u16,
    ) -> u16 {
        base
    }

    fn effect_hit_rate_aura_bp(&self) -> u32 {
        0
    }

    fn effect_resistance_aura_bp(&self) -> u32 {
        0
    }

    fn control_resistance_bp(&self) -> u32 {
        0
    }

    fn on_turn_start(&self, _battle: &mut Battle<'_>, _owner: usize, _actor: usize) {}

    fn after_enemy_attack_seen(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _attacker: usize,
        _target: usize,
        _was_shielded: bool,
    ) {
    }

    fn after_skill_freeze_ended(&self, _battle: &mut Battle<'_>, _owner: usize, _target: usize) {}

    fn prevent_defeat(&self, _battle: &mut Battle<'_>, _owner: usize) -> bool {
        false
    }

    fn incoming_damage_reduction_bp(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        _source: usize,
        _kind: DamageKind,
    ) -> u32 {
        0
    }

    fn bonus_attack_flat(&self, _battle: &Battle<'_>, _actor: usize) -> u32 {
        0
    }

    fn on_weakness_break(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _source: usize,
        _target: usize,
        _origin: Action,
    ) {
    }

    fn on_attack_finished(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _source: usize,
        _origin: Action,
    ) {
    }

    fn after_follow_up_hit(
        &self,
        _battle: &mut Battle<'_>,
        _actor: usize,
        _target: usize,
        _origin: Action,
    ) {
    }

    fn after_follow_up(&self, _battle: &mut Battle<'_>, _actor: usize, _origin: Action) {}

    fn after_hp_loss(&self, _battle: &mut Battle<'_>, _actor: usize, _hp_lost: u32) {}

    fn after_non_primary_critical_hit(
        &self,
        _battle: &mut Battle<'_>,
        _actor: usize,
        _target: usize,
        _action: Action,
        _kind: DamageKind,
    ) {
    }

    fn break_effect_bonus_bp(&self, _battle: &Battle<'_>, _actor: usize) -> u32 {
        0
    }

    fn speed_bonus_bp(&self, _battle: &Battle<'_>, _actor: usize) -> u32 {
        0
    }

    fn crit_damage_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        _target: usize,
        _action: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        0
    }

    fn resistance_penetration_bp(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        _element: Element,
    ) -> u32 {
        0
    }

    fn energy_regeneration_bonus_bp(&self, _battle: &Battle<'_>, _actor: usize) -> u32 {
        0
    }

    fn effect_resistance_bonus_bp(&self, _battle: &Battle<'_>, _actor: usize) -> u32 {
        0
    }

    fn ally_attack_bonus_bp(&self, _battle: &Battle<'_>, _owner: usize, _target: usize) -> u32 {
        0
    }

    fn ally_attack_bonus_flat(&self, _battle: &Battle<'_>, _owner: usize, _target: usize) -> u32 {
        0
    }

    fn ally_damage_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        _target: usize,
        _element: Element,
    ) -> u32 {
        0
    }

    fn defense_bonus_bp(&self, _battle: &Battle<'_>, _actor: usize) -> u32 {
        0
    }

    fn attack_conversion_from_defense_bp(&self) -> u32 {
        0
    }

    fn after_ordinary_turn(&self, _battle: &mut Battle<'_>, _owner: usize, _actor: usize) {}

    fn on_ally_action_finished(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _actor: usize,
        _action: Action,
    ) {
    }

    fn dpr_priority(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        _action: Action,
        _target: Option<usize>,
    ) -> Option<u8> {
        None
    }

    fn on_enemy_dot_damage(&self, _battle: &mut Battle<'_>, _owner: usize, _event: DotEvent) {}

    fn on_enemy_turn_start(&self, _battle: &mut Battle<'_>, _owner: usize, _target: usize) {}

    fn on_wave_start(&self, _battle: &mut Battle<'_>, _owner: usize) {}

    fn ally_dot_damage_bonus_bp(&self, _battle: &Battle<'_>, _owner: usize, _target: usize) -> u32 {
        0
    }

    fn ally_target_damage_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        _attacker: usize,
        _enemy: usize,
        _kind: DamageKind,
    ) -> u32 {
        0
    }

    fn ultimate_vulnerability_bp(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        _target: usize,
    ) -> u32 {
        0
    }

    fn all_resistance_down_bp(&self, _battle: &Battle<'_>, _owner: usize, _target: usize) -> u32 {
        0
    }

    fn attack_reduction_bp(&self, _battle: &Battle<'_>, _owner: usize, _enemy: usize) -> u32 {
        0
    }
}

pub fn supports_official_dpr(form_id: &str) -> bool {
    kit_builder(form_id).is_some()
}

pub fn supports_official_dpr_variant(form_id: &str, variant: &str) -> bool {
    supports_official_dpr(form_id)
        && (variant == "base" || (form_id == "character.1005" && variant == "novaflare"))
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    let build = kit_builder(&input.form_id)
        .ok_or_else(|| BattleError::UnsupportedData(format!("character form {}", input.form_id)))?;
    build(input)
}

type KitBuilder = fn(&OfficialCharacter) -> Result<OfficialKit, BattleError>;

fn kit_builder(form_id: &str) -> Option<KitBuilder> {
    match form_id {
        "character.1003" => Some(himeko::build),
        "character.1005" => Some(kafka::build),
        "character.1008" => Some(arlan::build),
        "character.1009" => Some(asta::build),
        "character.1013" => Some(herta::build),
        "character.1101" => Some(bronya::build),
        "character.1103" => Some(serval::build),
        "character.1104" => Some(gepard::build),
        "character.1106" => Some(pela::build),
        "character.1107" => Some(clara::build),
        "character.1108" => Some(sampo::build),
        "character.1109" => Some(hook::build),
        "character.1111" => Some(luka::build),
        "character.1202" => Some(tingyun::build),
        "character.1206" => Some(sushang::build),
        "character.1210" => Some(guinaifen::build),
        "dan_heng.hunt" => Some(dan_heng::build),
        "march_7th.preservation" => Some(march_preservation::build),
        "natasha.abundance" => Some(natasha::build),
        "character.1209" => Some(yanqing::build),
        _ => None,
    }
}
