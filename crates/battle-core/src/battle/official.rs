mod acheron;
mod aeon_aha;
mod aglaea;
mod anaxa;
mod archer;
mod argenti;
mod arlan;
mod asta;
mod aventurine;
mod bailu;
mod black_swan;
mod blade;
mod boothill;
mod bronya;
mod castorice;
mod cerydra;
mod cipher;
mod clara;
mod cyrene;
mod dahlia;
mod dan_heng;
mod dan_heng_il;
mod dan_heng_permansor;
mod dr_ratio;
mod evernight;
mod feixiao;
mod firefly;
mod fu_xuan;
mod fugue;
mod gallagher;
mod gepard;
mod guinaifen;
mod hanya;
mod healing;
mod herta;
mod himeko;
mod hook;
mod huohuo;
mod hyacine;
mod hysilens;
mod jade;
mod jiaoqiu;
mod jing_yuan;
mod jingliu;
mod kafka;
mod lingsha;
mod luka;
mod luocha;
mod lynx;
mod march_hunt;
mod march_preservation;
mod misha;
mod moze;
mod mydei;
mod natasha;
mod pela;
mod phainon;
mod qingque;
mod rappa;
mod rin;
mod robin;
mod ruan_mei;
mod saber;
mod sampo;
mod seele;
mod serval;
mod silver_wolf;
mod sparkle;
mod sparxie;
mod sunday;
mod sushang;
mod the_herta;
mod tingyun;
mod topaz;
mod trailblazer_harmony;
mod trailblazer_preservation;
mod trailblazer_remembrance;
mod tribbie;
mod welt;
mod xueyi;
mod yanqing;
mod yao_guang;
mod yukong;
mod yunli;

pub(super) use {
    acheron::AcheronState,
    aeon_aha::AeonAhaState,
    aglaea::AglaeaState,
    anaxa::AnaxaState,
    archer::ArcherState,
    aventurine::AventurineState,
    bailu::{BailuState, Invigoration},
    black_swan::BlackSwanTarget,
    blade::BladeState,
    boothill::BoothillState,
    castorice::CastoriceState,
    cerydra::CerydraState,
    cipher::CipherState,
    cyrene::CyreneState,
    dahlia::DahliaState,
    dan_heng_il::DanHengIlState,
    dan_heng_permansor::PermansorState,
    dr_ratio::DrRatioState,
    evernight::EvernightState,
    feixiao::FeixiaoState,
    firefly::FireflyState,
    fu_xuan::FuXuanState,
    fugue::FugueState,
    hanya::HanyaState,
    huohuo::HuohuoState,
    hyacine::HyacineState,
    hysilens::HysilensState,
    jade::JadeState,
    jiaoqiu::Field as JiaoqiuField,
    jing_yuan::JingYuanState,
    jingliu::JingliuState,
    lingsha::LingshaState,
    march_hunt::MarchHuntState,
    misha::MishaState,
    moze::MozeState,
    phainon::PhainonState,
    qingque::QingqueState,
    rappa::RappaState,
    rin::RinState,
    robin::RobinState,
    ruan_mei::RuanMeiState,
    saber::SaberState,
    seele::SeeleState,
    sparkle::SparkleState,
    sparxie::SparxieState,
    sunday::SundayState,
    the_herta::TheHertaState,
    topaz::TopazState,
    trailblazer_harmony::HarmonyTrailblazerState,
    trailblazer_preservation::PreservationTrailblazerState,
    trailblazer_remembrance::RemembranceTrailblazerState,
    tribbie::TribbieState,
    welt::WeltState,
    xueyi::XueyiState,
    yukong::YukongState,
    yunli::YunliState,
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
pub(super) use {
    gallagher::GallagherState, luocha::LuochaState, lynx::LynxResponse, mydei::MydeiState,
};

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
    pub elation_bp: u32,
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
                && !matches!(
                    self.form_id.as_str(),
                    acheron::FORM
                        | castorice::FORM
                        | cyrene::FORM
                        | aeon_aha::FORM
                        | feixiao::FORM
                        | phainon::FORM
                ))
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
                stats.elation_bp,
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
            Action::EnhancedSkill | Action::EnhancedUltimate | Action::ElationSkill => return None,
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

pub(super) struct MemospriteProfile {
    pub name: &'static str,
    pub stats: crate::fixture::Stats,
}

pub(super) trait Mechanics: Send + Sync {
    fn elation_participant(&self) -> Option<u16> {
        None
    }

    fn aha_base_speed_micros(&self, _battle: &Battle<'_>, _actor: usize) -> Option<u64> {
        None
    }

    fn elation_ascension(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
    ) -> Option<super::elation::ElationAscension> {
        None
    }

    fn on_punchline_changed(&self, _battle: &mut Battle<'_>, _actor: usize) {}

    fn on_aha_instant_ending(
        &self,
        _battle: &mut Battle<'_>,
        _actor: usize,
        _natural: bool,
        _skill_damage_factor_bp: u16,
    ) {
    }

    fn elation_debt(&self) -> Option<super::elation::ElationDebt> {
        None
    }

    fn elation_punchline_bonus(&self, _battle: &Battle<'_>, _actor: usize) -> u32 {
        0
    }

    fn ally_defense_ignore_bp(&self, _battle: &Battle<'_>, _actor: usize) -> u32 {
        0
    }

    fn certified_banger_duration(&self) -> u32 {
        2
    }

    fn on_elation_skill(
        &self,
        _battle: &mut Battle<'_>,
        _actor: usize,
        _instant: super::elation::AhaInstant,
    ) {
    }

    fn on_aha_instant_finished(&self, _battle: &mut Battle<'_>, _actor: usize) {}

    fn personal_elation_bp(&self, _battle: &Battle<'_>, _actor: usize) -> u32 {
        0
    }

    fn elation_from_attack_bp(&self, _attack: u32) -> u32 {
        0
    }

    fn ally_elation_bp(&self, _battle: &Battle<'_>, _source: usize, _actor: usize) -> u32 {
        0
    }

    fn ally_merrymake_bp(&self, _battle: &Battle<'_>, _source: usize, _actor: usize) -> u32 {
        0
    }

    fn ally_elation_defense_ignore_bp(&self, _battle: &Battle<'_>, _source: usize) -> u32 {
        0
    }

    fn aha_extra_skill_damage_factor_bp(&self) -> u16 {
        10_000
    }

    fn memosprite_profile(&self, _master: crate::fixture::Stats) -> Option<MemospriteProfile> {
        None
    }

    fn on_memosprite_departed(&self, _battle: &mut Battle<'_>, _owner: usize, _sprite: usize) {}

    fn on_memosprite_turn_finished(&self, _battle: &mut Battle<'_>, _owner: usize, _sprite: usize) {
    }

    fn on_ally_energy_restored(&self, _battle: &mut Battle<'_>, _owner: usize, _amount_milli: u32) {
    }

    fn memosprite_true_damage_bp(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        _source: usize,
    ) -> u32 {
        0
    }

    fn memosprite_guarantees_critical(&self, _origin: Option<Action>) -> bool {
        false
    }

    fn memosprite_immune_to_enemy_control(&self, _battle: &Battle<'_>, _owner: usize) -> bool {
        false
    }

    fn memosprite_aggro_multiplier_bp(&self) -> u32 {
        10_000
    }

    fn memosprite_has_fixed_zero_speed(&self) -> bool {
        false
    }

    fn memosprite_immune_to_debuffs(&self) -> bool {
        false
    }

    fn memosprite_damage_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _sprite: usize,
        _origin: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        0
    }

    fn memosprite_resistance_penetration_bp(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        _element: Element,
    ) -> u32 {
        0
    }

    fn tracks_active_abilities(&self) -> bool {
        false
    }

    fn on_active_ability(&self, _battle: &mut Battle<'_>, _owner: usize, _actor: usize) {}

    fn on_countdown_turn(&self, _battle: &mut Battle<'_>, _owner: usize) {}

    fn memosprite_is_backup(&self) -> bool {
        false
    }

    fn memosprite_uses_owner_max_hp(&self) -> bool {
        false
    }

    fn memosprite_ignores_weakness(&self) -> bool {
        false
    }

    fn on_external_energy(&self, _battle: &mut Battle<'_>, _actor: usize) {}

    fn on_energy_overflow(&self, _battle: &mut Battle<'_>, _actor: usize, _amount_milli: u32) {}

    fn on_ally_sustain(&self, _battle: &mut Battle<'_>, _actor: usize) {}

    fn on_enemy_targeted(&self, _battle: &mut Battle<'_>, _actor: usize) {}

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

    fn can_use_skill_action(&self, battle: &Battle<'_>, actor: usize, _action: Action) -> bool {
        self.can_use_skill(battle, actor)
    }

    fn can_use_basic(&self, _battle: &Battle<'_>, _actor: usize) -> bool {
        true
    }

    fn can_use_ultimate(&self, _battle: &Battle<'_>, _actor: usize) -> bool {
        true
    }

    fn activate_special_ultimate(&self, _battle: &mut Battle<'_>, _actor: usize) {}

    fn cyrene_ode_applied(
        &self,
        _battle: &mut Battle<'_>,
        _source: usize,
        _actor: usize,
        _parameters: [u32; 4],
    ) {
    }

    fn cyrene_ode_parameters(&self, _form: &str) -> Option<[u32; 8]> {
        None
    }

    /// Suppresses scheduling and commands without preventing other units targeting the actor.
    fn suspends_turn(&self, _battle: &Battle<'_>, _actor: usize) -> bool {
        false
    }

    /// An ordinary action can leave the current turn open for another command.
    fn keeps_turn(&self, _action: Action) -> bool {
        false
    }

    /// A turn-ending command can skip ability and combat-action reactions.
    fn ends_turn_without_ability(&self, _action: Action) -> bool {
        false
    }

    /// Evaluated after an action, including its follow-ups and skill point changes.
    fn ends_retained_turn(&self, _battle: &Battle<'_>, _actor: usize) -> bool {
        false
    }

    fn forced_action_target(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        _action: Action,
    ) -> Option<usize> {
        None
    }

    fn prepare_automatic_skill(&self, _battle: &mut Battle<'_>, _actor: usize) -> bool {
        false
    }

    fn taunts_enemy(&self, _battle: &Battle<'_>, _owner: usize, _enemy: usize) -> bool {
        false
    }

    /// Handles a command that only prepares a later action, with no action triggers.
    fn prepare_action(&self, _battle: &mut Battle<'_>, _actor: usize, _action: Action) -> bool {
        false
    }

    /// Payment made by the character's own mechanic after the command is validated.
    fn shared_skill_point_cost(&self, _battle: &Battle<'_>, _actor: usize, _action: Action) -> u8 {
        0
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

    fn attack_source(
        &self,
        _battle: &Battle<'_>,
        actor: usize,
        _action: Action,
        _ability: Ability<'_>,
    ) -> usize {
        actor
    }

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

    fn tracks_attack_damage(&self) -> bool {
        false
    }

    fn tracks_ally_damage(&self) -> bool {
        false
    }

    fn on_ally_damage(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _source: usize,
        _target: usize,
        _kind: DamageKind,
        _amount: u32,
    ) {
    }

    fn after_ally_attack_damage(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _origin: Action,
        _damage: u64,
    ) {
    }

    fn on_summon_turn(&self, _battle: &mut Battle<'_>, _owner: usize) {}

    fn summon_holder(&self, _battle: &Battle<'_>, owner: usize) -> usize {
        owner
    }

    fn assisted_damage_comes_from_summon(&self, _battle: &Battle<'_>, _owner: usize) -> bool {
        false
    }

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

    fn on_party_hp_lost(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _target: usize,
        _amount: u32,
        _received_attack: Option<u32>,
    ) {
    }

    fn absorb_fatal_hp_overflow(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _target: usize,
        _overflow: u32,
    ) -> bool {
        false
    }

    fn on_party_healed(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _target: usize,
        _amount: u32,
    ) {
    }

    fn ally_incoming_healing_bonus_bp(&self, _battle: &Battle<'_>, _owner: usize) -> u32 {
        0
    }

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

    fn max_hp_multiplier_bp(&self, _battle: &Battle<'_>, _actor: usize) -> u32 {
        10_000
    }

    fn ally_max_hp_bonus(&self, _battle: &Battle<'_>, _owner: usize, _target: usize) -> u32 {
        0
    }

    fn memosprite_max_hp_bonus(&self, _battle: &Battle<'_>, _owner: usize, _sprite: usize) -> u32 {
        0
    }

    fn ally_damage_reduction_bp(&self, _battle: &Battle<'_>, _owner: usize) -> u32 {
        0
    }

    fn ally_damage_reduction_bp_for_target(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        _target: usize,
    ) -> u32 {
        self.ally_damage_reduction_bp(battle, owner)
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

    fn outgoing_damage_factor_bp(&self) -> u32 {
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

    fn immune_to_enemy_control(&self, _battle: &Battle<'_>, _actor: usize) -> bool {
        false
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

    fn incoming_vulnerability_bp(
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

    fn ally_toughness_bonus_bp(&self, _battle: &Battle<'_>, _owner: usize, _actor: usize) -> u32 {
        0
    }

    fn off_element_toughness_bp(&self, _battle: &Battle<'_>, _owner: usize, _actor: usize) -> u16 {
        0
    }

    fn ally_break_damage_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        _actor: usize,
    ) -> u32 {
        0
    }

    fn on_ally_first_attack_hit(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _actor: usize,
        _target: usize,
    ) {
    }

    fn prevent_toughness_recovery(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _target: usize,
    ) -> bool {
        false
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

    fn observes_weakness_implants(&self) -> bool {
        false
    }

    fn on_weakness_implanted(&self, _battle: &mut Battle<'_>, _owner: usize, _source: usize) {}

    fn provides_fixed_attack_toughness(&self) -> bool {
        false
    }

    fn before_super_breaks(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _actor: usize,
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

    fn can_execute_follow_up(&self, _battle: &Battle<'_>, _actor: usize) -> bool {
        true
    }

    fn before_follow_up(&self, _battle: &mut Battle<'_>, _actor: usize) {}

    fn after_follow_up(&self, _battle: &mut Battle<'_>, _actor: usize, _origin: Action) {}

    fn after_hp_loss(
        &self,
        _battle: &mut Battle<'_>,
        _actor: usize,
        _source: usize,
        _hp_lost: u32,
    ) {
    }

    fn after_hp_spent(&self, _battle: &mut Battle<'_>, _actor: usize, _amount: u32) {}

    fn incoming_healing_bonus_bp(&self, _battle: &Battle<'_>, _actor: usize) -> u32 {
        0
    }

    fn after_healing(&self, _battle: &mut Battle<'_>, _target: usize, _restored: u32) {}

    fn after_ally_hp_loss(&self, _battle: &mut Battle<'_>, _owner: usize, _target: usize) {}

    /// Positive incoming damage (including absorption) or actual voluntary HP consumption.
    /// Incoming hits from one attack share an index so observers can count the recipient once.
    fn on_ally_health_event(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _target: usize,
        _attack_index: Option<u32>,
    ) {
    }

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

    fn break_effect_from_attack_bp(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        _attack: Option<u32>,
    ) -> u32 {
        0
    }

    fn own_break_vulnerability_bp(&self, _battle: &Battle<'_>, _actor: usize) -> u32 {
        0
    }

    fn super_break_multiplier_for_effect(&self, multiplier_bp: u16, _effect_bp: u32) -> u16 {
        multiplier_bp
    }

    /// Critical rate and critical damage provided by the resolved break effect.
    fn crit_from_break_effect_bp(&self, _break_effect_bp: u32) -> (u32, u32) {
        (0, 0)
    }

    fn ally_break_effect_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        _target: usize,
    ) -> u32 {
        0
    }

    fn provides_super_break(&self) -> bool {
        false
    }

    fn allows_unbroken_super_break(&self, _battle: &Battle<'_>, _owner: usize) -> bool {
        false
    }

    fn follow_up_hit_super_break_bp(&self) -> u16 {
        0
    }

    fn super_break_multiplier_bp(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        _actor: usize,
        _target: usize,
    ) -> u16 {
        0
    }

    fn damage_bonus_from_break_effect_bp(&self, _break_effect_bp: u32) -> u32 {
        0
    }

    fn attack_bonus_from_break_effect_bp(&self, _break_effect_bp: u32) -> u32 {
        0
    }

    fn ally_speed_bonus_bp(&self, _battle: &Battle<'_>, _owner: usize, _target: usize) -> u32 {
        0
    }

    /// Flat speed in millionths of one speed point.
    fn ally_flat_speed_micros(&self, _battle: &Battle<'_>, _owner: usize, _target: usize) -> u64 {
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

    fn ally_target_attack_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        _enemy: usize,
    ) -> u32 {
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

    fn defense_is_zero(&self, _battle: &Battle<'_>, _actor: usize) -> bool {
        false
    }

    fn defense_bonus_bp(&self, _battle: &Battle<'_>, _actor: usize) -> u32 {
        0
    }

    fn scales_with_defense(&self) -> bool {
        false
    }

    fn follow_up_max_hp_multiplier_bp(&self) -> u16 {
        0
    }

    fn crit_rate_from_defense_bp(&self, _defense: u32) -> u32 {
        0
    }

    fn attack_conversion_from_defense_bp(&self) -> u32 {
        0
    }

    fn after_ordinary_turn(&self, _battle: &mut Battle<'_>, _owner: usize, _actor: usize) {}

    fn after_battle_turn(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _actor: usize,
        _extra: bool,
    ) {
    }

    fn on_ally_energy_full(&self, _battle: &mut Battle<'_>, _owner: usize, _actor: usize) {}

    fn skill_point_cap_bonus(&self) -> u8 {
        0
    }

    fn on_ally_skill_points_restored(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _actor: usize,
        _points: u8,
    ) {
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

    fn tracks_skill_copies(&self) -> bool {
        false
    }

    fn before_ally_skill_copy(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _actor: usize,
        _target: Option<usize>,
        _ability: Ability<'_>,
    ) {
    }

    fn can_copy_skill(&self, _battle: &Battle<'_>, _actor: usize, ability: Ability<'_>) -> bool {
        matches!(ability.target, Target::Enemy | Target::AllEnemies)
            && ability.effects.iter().any(|effect| effect.deals_damage())
    }

    fn resolve_copied_skill(
        &self,
        _battle: &mut Battle<'_>,
        _actor: usize,
        _target: Option<usize>,
    ) -> bool {
        false
    }

    fn ally_skill_damage_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        _actor: usize,
    ) -> u32 {
        0
    }

    fn ally_skill_damage_factor_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        _actor: usize,
    ) -> u32 {
        0
    }

    fn ally_skill_crit_damage_bp(&self, _battle: &Battle<'_>, _owner: usize, _actor: usize) -> u32 {
        0
    }

    fn ally_skill_defense_ignore_bp(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        _actor: usize,
    ) -> u32 {
        0
    }

    fn on_ally_action_finished(
        &self,
        _battle: &mut Battle<'_>,
        _owner: usize,
        _actor: usize,
        _action: Action,
    ) {
    }

    fn on_unit_action_finished(&self, _battle: &mut Battle<'_>, _owner: usize, _actor: usize) {}

    fn tracks_action_end_reactions(&self) -> bool {
        false
    }

    fn react_to_action_end(&self, _battle: &mut Battle<'_>, _owner: usize, _actor: usize) {}

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

    fn on_dot_batch_finished(&self, _battle: &mut Battle<'_>, _owner: usize) {}

    fn ally_dot_damage_factor_bp(&self, _battle: &Battle<'_>, _owner: usize) -> u32 {
        10_000
    }

    fn ally_memosprite_damage_factor_bp(&self, _battle: &Battle<'_>, _owner: usize) -> u32 {
        10_000
    }

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

    fn follow_up_counts_as_ultimate(&self, _origin: Option<Action>) -> bool {
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

    fn damage_comes_from_summon(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        _origin: Option<Action>,
        _kind: DamageKind,
    ) -> bool {
        false
    }

    fn all_resistance_down_bp(&self, _battle: &Battle<'_>, _owner: usize, _target: usize) -> u32 {
        0
    }

    fn target_defense_down_bp(&self, _battle: &Battle<'_>, _owner: usize, _target: usize) -> u32 {
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
                        | blade::FORM
                        | firefly::FORM
                        | silver_wolf::FORM
                        | huohuo::FORM
                        | jingliu::FORM
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
        aeon_aha::FORM => Some(aeon_aha::build),
        archer::FORM => Some(archer::build),
        rin::FORM => Some(rin::build),
        aglaea::FORM => Some(aglaea::build),
        "character.1003" => Some(himeko::build),
        sparxie::FORM => Some(sparxie::build),
        yao_guang::FORM => Some(yao_guang::build),
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
        blade::FORM => Some(blade::build),
        luocha::FORM => Some(luocha::build),
        "character.1206" => Some(sushang::build),
        "character.1207" => Some(yukong::build),
        fu_xuan::FORM => Some(fu_xuan::build),
        "character.1210" => Some(guinaifen::build),
        "character.1211" => Some(bailu::build),
        jingliu::FORM => Some(jingliu::build),
        xueyi::FORM => Some(xueyi::build),
        dan_heng_il::FORM => Some(dan_heng_il::build),
        dan_heng_permansor::FORM => Some(dan_heng_permansor::build),
        "character.1215" => Some(hanya::build),
        huohuo::FORM => Some(huohuo::build),
        hysilens::FORM => Some(hysilens::build),
        "character.1218" => Some(jiaoqiu::build),
        feixiao::FORM => Some(feixiao::build),
        lingsha::FORM => Some(lingsha::build),
        firefly::FORM => Some(firefly::build),
        "character.1223" => Some(moze::build),
        fugue::FORM => Some(fugue::build),
        dahlia::FORM => Some(dahlia::build),
        "character.1301" => Some(gallagher::build),
        argenti::FORM => Some(argenti::build),
        saber::FORM => Some(saber::build),
        ruan_mei::FORM => Some(ruan_mei::build),
        aventurine::FORM => Some(aventurine::build),
        "character.1305" => Some(dr_ratio::build),
        sparkle::FORM => Some(sparkle::build),
        black_swan::FORM => Some(black_swan::build),
        acheron::FORM => Some(acheron::build),
        robin::FORM => Some(robin::build),
        "character.1312" => Some(misha::build),
        rappa::FORM => Some(rappa::build),
        sunday::FORM => Some(sunday::build),
        jade::FORM => Some(jade::build),
        the_herta::FORM => Some(the_herta::build),
        tribbie::FORM => Some(tribbie::build),
        mydei::FORM => Some(mydei::build),
        anaxa::FORM => Some(anaxa::build),
        phainon::FORM => Some(phainon::build),
        cipher::FORM => Some(cipher::build),
        cerydra::FORM => Some(cerydra::build),
        castorice::FORM => Some(castorice::build),
        hyacine::FORM => Some(hyacine::build),
        evernight::FORM => Some(evernight::build),
        cyrene::FORM => Some(cyrene::build),
        boothill::FORM => Some(boothill::build),
        yunli::FORM => Some(yunli::build),
        "dan_heng.hunt" => Some(dan_heng::build),
        march_hunt::FORM => Some(march_hunt::build),
        "march_7th.preservation" => Some(march_preservation::build),
        "natasha.abundance" => Some(natasha::build),
        trailblazer_harmony::FORM => Some(trailblazer_harmony::build),
        trailblazer_preservation::FORM => Some(trailblazer_preservation::build),
        trailblazer_remembrance::FORM => Some(trailblazer_remembrance::build),
        "character.1209" => Some(yanqing::build),
        _ => None,
    }
}
