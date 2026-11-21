mod acheron;
mod argenti;
mod arlan;
mod asta;
mod aventurine;
mod bailu;
mod black_swan;
mod bronya;
mod clara;
mod dan_heng;
mod dr_ratio;
mod feixiao;
mod fu_xuan;
mod gallagher;
mod gepard;
mod guinaifen;
mod hanya;
mod healing;
mod herta;
mod himeko;
mod hook;
mod huohuo;
mod jiaoqiu;
mod jing_yuan;
mod kafka;
mod luka;
mod luocha;
mod lynx;
mod march_hunt;
mod march_preservation;
mod misha;
mod moze;
mod natasha;
mod pela;
mod qingque;
mod sampo;
mod seele;
mod serval;
mod silver_wolf;
mod sparkle;
mod sushang;
mod tingyun;
mod topaz;
mod welt;
mod xueyi;
mod yanqing;
mod yukong;

pub(super) use {
    acheron::AcheronState,
    aventurine::AventurineState,
    bailu::{BailuState, Invigoration},
    black_swan::BlackSwanTarget,
    dr_ratio::DrRatioState,
    feixiao::FeixiaoState,
    fu_xuan::FuXuanState,
    hanya::HanyaState,
    huohuo::HuohuoState,
    jiaoqiu::Field as JiaoqiuField,
    jing_yuan::JingYuanState,
    march_hunt::MarchHuntState,
    misha::MishaState,
    moze::MozeState,
    qingque::QingqueState,
    seele::SeeleState,
    sparkle::SparkleState,
    topaz::TopazState,
    welt::WeltState,
    xueyi::XueyiState,
    yukong::YukongState,
};

use std::{collections::BTreeMap, ops::RangeInclusive, sync::Arc};

use super::{
    Battle,
    periodic::{DotEvent, SpecialDotTrigger},
    resolve::DamageTargetAura,
};
use crate::{
    Action, BattleError, DamageKind,
    fixture::{Ability, Effect, Element, Stats, Target},
};
pub(super) use {gallagher::GallagherState, luocha::LuochaState, lynx::LynxResponse};

/// Static stats resolved by the profile layer before a fixed-target simulation.
pub struct OfficialStats {
    pub hp: u32,
    pub base_hp: u32,
    pub attack: u32,
    pub base_attack: u32,
    pub defense: u32,
    pub base_defense: u32,
    pub base_aggro: u16,
    pub base_speed_micros: u64,
    pub speed_micros: u64,
    pub crit_rate_bp: u32,
    pub crit_damage_bp: u32,
    pub element_damage_bp: [u32; Element::COUNT],
    pub break_effect_bp: u32,
    pub effect_hit_rate_bp: u32,
    pub energy_regeneration_bp: u32,
    pub healing_bp: u32,
    pub effect_resistance_bp: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CombatPath {
    Destruction,
    Hunt,
    Erudition,
    Harmony,
    Nihility,
    Preservation,
    Abundance,
    Remembrance,
    Elation,
}

impl CombatPath {
    pub fn from_id(id: &str) -> Option<Self> {
        Some(match id {
            "destruction" => Self::Destruction,
            "hunt" => Self::Hunt,
            "erudition" => Self::Erudition,
            "harmony" => Self::Harmony,
            "nihility" => Self::Nihility,
            "preservation" => Self::Preservation,
            "abundance" => Self::Abundance,
            "remembrance" => Self::Remembrance,
            "elation" => Self::Elation,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy)]
pub struct CombatType {
    pub path: CombatPath,
    pub element: Element,
}

/// Frozen effective character parameters from the supported game data version.
pub struct OfficialCharacter {
    pub unit_id: String,
    pub form_id: String,
    pub combat_type: CombatType,
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
            || (self.energy_capacity == 0
                && !matches!(self.form_id.as_str(), acheron::FORM | feixiao::FORM))
            || self.energy_capacity > 999
            || stats.hp == 0
            || stats.hp > 100_000
            || stats.base_hp == 0
            || stats.base_hp > stats.hp
            || stats.attack == 0
            || stats.attack > 100_000
            || stats.base_attack == 0
            || stats.base_attack > 100_000
            || stats.defense > 100_000
            || stats.base_defense > 100_000
            || !(1..=1_000).contains(&stats.base_aggro)
            || stats.base_speed_micros == 0
            || stats.speed_micros < stats.base_speed_micros
            || stats.speed_micros > 1_000_000_000
            || [
                stats.crit_rate_bp,
                stats.crit_damage_bp,
                stats.break_effect_bp,
                stats.effect_hit_rate_bp,
                stats.energy_regeneration_bp,
                stats.healing_bp,
                stats.effect_resistance_bp,
            ]
            .into_iter()
            .chain(stats.element_damage_bp)
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
            base_hp: stats.base_hp,
            attack: stats.attack,
            base_attack: stats.base_attack,
            defense: stats.defense,
            base_defense: stats.base_defense,
            base_aggro: stats.base_aggro,
            base_speed_micros: stats.base_speed_micros,
            flat_speed_micros: stats.speed_micros - stats.base_speed_micros,
            resist_bp: [0; Element::COUNT],
            crit_rate_bp: stats.crit_rate_bp,
            crit_damage_bp: stats.crit_damage_bp,
            element_damage_bp: stats.element_damage_bp,
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
    pub fn ability(&self, action: Action) -> Option<(Ability<'_>, SkillPoints)> {
        let ability = match action {
            Action::Basic => &self.basic,
            Action::Skill => &self.skill,
            Action::Ultimate => &self.ultimate,
            Action::EnhancedUltimate => return None,
        };
        Some(ability.resolve())
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
    /// Lower bound is both the charge threshold and the energy consumed.
    fn ultimate_energy_range(&self, _action: Action) -> Option<RangeInclusive<u32>> {
        None
    }

    fn alternate_ultimate_resource_ready(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
    ) -> Option<bool> {
        None
    }

    fn can_use_skill(&self, _battle: &Battle<'_>, _actor: usize) -> bool {
        true
    }

    /// An ordinary action can leave the current turn open for another command.
    fn keeps_turn(&self, _action: Action) -> bool {
        false
    }

    fn ability_override(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        _action: Action,
    ) -> Option<&OfficialAbility> {
        None
    }

    fn resolve_special_attack(&self, _battle: &mut Battle<'_>, _actor: usize, _target: usize) {}

    fn observes_enemy_debuffs(&self) -> bool {
        false
    }

    fn on_enemy_debuffs_applied(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _source: usize,
        _targets: &[usize],
    ) {
    }

    fn on_battle_start(&self, _battle: &mut Battle<'_>, _actor: usize) {}

    fn on_summon_turn(&self, _battle: &mut Battle<'_>, _owner: usize) {}

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

    fn before_attack_hit(
        &self,
        _battle: &mut Battle<'_>,
        _actor: usize,
        _target: usize,
        _action: Action,
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

    fn random_hit_count(&self, _battle: &Battle<'_>, _actor: usize, base: u8) -> u8 {
        base
    }

    fn aggro_multiplier_bp(&self, _battle: &Battle<'_>, _actor: usize) -> u32 {
        10_000
    }

    fn defense_ignore_bp(&self, _battle: &Battle<'_>, _actor: usize, _target: usize) -> u32 {
        0
    }

    fn capture_ultimate_damage(
        &self,
        _battle: &mut Battle<'_>,
        _actor: usize,
        _target: usize,
        _amount: u32,
        _assists: &[(usize, u32)],
    ) {
    }

    fn on_triggered_skill(&self, _battle: &mut Battle<'_>, _actor: usize, _target: usize) {}

    fn after_defeat(&self, _battle: &mut Battle<'_>, _actor: usize, _target: usize) {}

    fn on_enemy_defeat(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _killer: usize,
        _target: usize,
    ) {
    }

    fn on_ally_defeat(&self, _battle: &mut Battle<'_>, _owner: usize) {}

    fn on_defeated(&self, _battle: &mut Battle<'_>, _actor: usize) {}

    fn after_enemy_action(&self, _battle: &mut Battle<'_>, _owner: usize) {}

    fn record_team_hp_loss(&self, _battle: &mut Battle<'_>, _owner: usize, _hp_lost: u32) {}

    fn block_enemy_control(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _attacker: usize,
        _target: usize,
    ) -> bool {
        false
    }

    fn prevent_team_defeat(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _attacker: usize,
        _target: usize,
    ) -> bool {
        false
    }

    fn ally_max_hp_bonus(&self, _battle: &Battle<'_>, _owner: usize, _target: usize) -> u32 {
        0
    }

    fn ally_damage_reduction_bp(&self, _battle: &Battle<'_>, _owner: usize) -> u32 {
        0
    }

    fn damage_distribution_bp(&self, _battle: &Battle<'_>, _owner: usize) -> u16 {
        0
    }

    // A flat contribution to the same hit's base damage, in damage units times 10,000.
    fn flat_damage_base_bp(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        _action: Option<Action>,
        _kind: DamageKind,
    ) -> u64 {
        0
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
        base: u32,
    ) -> u32 {
        base
    }

    fn action_damage_factor_bp(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        _action: Option<Action>,
        _kind: DamageKind,
    ) -> u16 {
        10_000
    }

    fn effect_hit_rate_aura_bp(&self) -> u32 {
        0
    }

    fn effect_hit_rate_bonus_bp(&self, _battle: &Battle<'_>, _actor: usize) -> u32 {
        0
    }

    fn effect_resistance_aura_bp(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        _target: usize,
    ) -> u32 {
        0
    }

    fn additional_debuff_count(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        _target: usize,
    ) -> usize {
        0
    }

    fn prevent_debuff(&self, _battle: &mut Battle<'_>, _actor: usize) -> bool {
        false
    }

    fn control_resistance_bp(&self) -> u32 {
        0
    }

    fn on_extra_turn_start(&self, _battle: &mut Battle<'_>, _actor: usize) {}

    fn on_ally_turn_opening(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _actor: usize,
        _extra: bool,
    ) {
    }

    fn on_turn_start(&self, _battle: &mut Battle<'_>, _owner: usize, _actor: usize) {}

    fn before_enemy_attack_seen(&self, _battle: &mut Battle<'_>, _owner: usize, _target: usize) {}

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

    fn prevent_ally_defeat(&self, _battle: &mut Battle<'_>, _owner: usize, _target: usize) -> bool {
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

    fn ignores_weakness(&self, _action: Option<Action>, _kind: DamageKind) -> bool {
        false
    }

    fn alternate_weakness(&self, _battle: &Battle<'_>, _actor: usize) -> Option<Element> {
        None
    }

    fn toughness_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        _action: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        0
    }

    fn on_toughness_reduced(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _source: usize,
        _amount: u16,
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

    fn on_ally_attack(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _source: usize,
        _origin: Action,
        _primary_target: Option<usize>,
    ) {
    }

    fn on_attack_target(&self, _battle: &mut Battle<'_>, _owner: usize, _target: usize) {}

    fn on_first_attack_hit(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _target: usize,
        _primary: bool,
        _follow_up: bool,
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

    fn before_follow_up(&self, _battle: &mut Battle<'_>, _actor: usize) {}

    fn after_follow_up(&self, _battle: &mut Battle<'_>, _actor: usize, _origin: Action) {}

    fn after_hp_loss(&self, _battle: &mut Battle<'_>, _actor: usize, _hp_lost: u32) {}

    fn after_ally_hp_loss(&self, _battle: &mut Battle<'_>, _owner: usize, _target: usize) {}

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

    fn damage_bonus_from_break_effect_bp(&self, _break_effect_bp: u32) -> u32 {
        0
    }

    fn ally_speed_bonus_bp(&self, _battle: &Battle<'_>, _owner: usize, _target: usize) -> u32 {
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

    fn ally_crit_rate_bonus_bp(&self, _battle: &Battle<'_>, _owner: usize, _target: usize) -> u32 {
        0
    }

    fn ally_crit_damage_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        _target: usize,
    ) -> u32 {
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

    fn scales_with_defense(&self) -> bool {
        false
    }

    fn crit_rate_from_defense_bp(&self, _defense: u32) -> u32 {
        0
    }

    fn attack_conversion_from_defense_bp(&self) -> u32 {
        0
    }

    fn after_ordinary_turn(&self, _battle: &mut Battle<'_>, _owner: usize, _actor: usize) {}

    fn on_ally_energy_full(&self, _battle: &mut Battle<'_>, _owner: usize, _actor: usize) {}

    fn skill_point_cap_bonus(&self) -> u8 {
        0
    }

    fn on_ally_skill_points_spent(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _actor: usize,
        _points: u8,
    ) {
    }

    fn before_ally_action(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _actor: usize,
        _target: Option<usize>,
        _ability: Ability<'_>,
    ) {
    }

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

    fn on_enemy_turn_opening(&self, _battle: &mut Battle<'_>, _owner: usize, _target: usize) {}

    fn trigger_special_dot(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _target: usize,
        _trigger: SpecialDotTrigger,
    ) {
    }

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

    fn target_crit_damage_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        _enemy: usize,
    ) -> u32 {
        0
    }

    fn ultimate_counts_as_follow_up(&self) -> bool {
        false
    }

    fn primary_attack_counts_as_ultimate(&self, action: Action) -> bool {
        action.is_ultimate()
    }

    fn follow_up_counts_as_ultimate(&self) -> bool {
        false
    }

    fn ultimate_resistance_penetration_bp(&self) -> u32 {
        0
    }

    fn ultimate_defense_ignore_bp(&self) -> u32 {
        0
    }

    fn primary_attack_counts_as_follow_up(&self, action: Action) -> bool {
        action.is_ultimate() && self.ultimate_counts_as_follow_up()
    }

    fn follow_up_vulnerability_bp(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        _enemy: usize,
    ) -> u32 {
        0
    }

    fn follow_up_crit_damage_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        _enemy: usize,
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

    fn damage_target_aura(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        _attacker: usize,
    ) -> DamageTargetAura {
        DamageTargetAura::default()
    }

    fn all_resistance_down_bp(&self, _battle: &Battle<'_>, _owner: usize, _target: usize) -> u32 {
        0
    }

    fn elemental_resistance_down_bp(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        _target: usize,
        _element: Element,
    ) -> u32 {
        0
    }

    fn attack_reduction_bp(&self, _battle: &Battle<'_>, _owner: usize, _enemy: usize) -> u32 {
        0
    }

    fn enemy_damage_reduction_bp(&self, _battle: &Battle<'_>, _owner: usize) -> u32 {
        0
    }
}

pub fn supports_official_dpr(form_id: &str) -> bool {
    kit_builder(form_id).is_some()
}

pub fn supports_official_dpr_variant(form_id: &str, variant: &str) -> bool {
    supports_official_dpr(form_id)
        && (variant == "base"
            || (variant == "novaflare"
                && matches!(
                    form_id,
                    "character.1005"
                        | welt::FORM
                        | black_swan::FORM
                        | silver_wolf::FORM
                        | huohuo::FORM
                        | seele::FORM
                        | sparkle::FORM
                )))
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
        welt::FORM => Some(welt::build),
        "character.1005" => Some(kafka::build),
        silver_wolf::FORM => Some(silver_wolf::build),
        "character.1008" => Some(arlan::build),
        "character.1009" => Some(asta::build),
        "character.1013" => Some(herta::build),
        "character.1101" => Some(bronya::build),
        seele::FORM => Some(seele::build),
        "character.1103" => Some(serval::build),
        "character.1104" => Some(gepard::build),
        "character.1106" => Some(pela::build),
        "character.1107" => Some(clara::build),
        "character.1108" => Some(sampo::build),
        "character.1109" => Some(hook::build),
        "character.1110" => Some(lynx::build),
        "character.1111" => Some(luka::build),
        "character.1112" => Some(topaz::build),
        qingque::FORM => Some(qingque::build),
        "character.1202" => Some(tingyun::build),
        jing_yuan::FORM => Some(jing_yuan::build),
        luocha::FORM => Some(luocha::build),
        "character.1206" => Some(sushang::build),
        "character.1207" => Some(yukong::build),
        fu_xuan::FORM => Some(fu_xuan::build),
        "character.1210" => Some(guinaifen::build),
        "character.1211" => Some(bailu::build),
        xueyi::FORM => Some(xueyi::build),
        "character.1215" => Some(hanya::build),
        huohuo::FORM => Some(huohuo::build),
        "character.1218" => Some(jiaoqiu::build),
        feixiao::FORM => Some(feixiao::build),
        "character.1223" => Some(moze::build),
        "character.1301" => Some(gallagher::build),
        argenti::FORM => Some(argenti::build),
        aventurine::FORM => Some(aventurine::build),
        "character.1305" => Some(dr_ratio::build),
        sparkle::FORM => Some(sparkle::build),
        black_swan::FORM => Some(black_swan::build),
        acheron::FORM => Some(acheron::build),
        "character.1312" => Some(misha::build),
        "dan_heng.hunt" => Some(dan_heng::build),
        march_hunt::FORM => Some(march_hunt::build),
        "march_7th.preservation" => Some(march_preservation::build),
        "natasha.abundance" => Some(natasha::build),
        "character.1209" => Some(yanqing::build),
        _ => None,
    }
}
