use std::sync::Arc;

use super::{
    super::{Battle, support::AllyBuffKind},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};
use crate::{
    Action, BattleError, EventKind, StatusKind,
    fixture::{Effect, Element, Target},
};

#[cfg(test)]
mod tests;

const BASIC: &str = "skill.111001";
const SKILL: &str = "skill.111002";
const ULTIMATE: &str = "skill.111003";
const TALENT: &str = "skill.111004";
const ADVANCE_SURVEYING: &str = "trace.1110101";
const EXPLORATION_TECHNIQUES: &str = "trace.1110102";
const SURVIVAL_IN_THE_EXTREME: &str = "trace.1110103";
const BP: u64 = 10_000;
const MICROS: u64 = 1_000_000;

#[derive(Clone, Copy)]
pub(in crate::battle) struct LynxResponse {
    source: usize,
    pub(in crate::battle) expires_after_turn: u32,
    guard: bool,
    aggro_bonus: u32,
    hp_bonus: u32,
}

#[derive(Clone, Copy)]
struct HpAmount {
    ratio_micros: u64,
    flat_micros: u64,
}

impl HpAmount {
    fn new(id: &str, ratio: f64, flat: f64) -> Result<Self, BattleError> {
        if ratio > 10.0 || flat > 1_000_000.0 {
            return Err(invalid(id));
        }
        Ok(Self {
            ratio_micros: (ratio * MICROS as f64).round() as u64,
            flat_micros: (flat * MICROS as f64).round() as u64,
        })
    }

    fn add(self, extra: Self) -> Self {
        Self {
            ratio_micros: self.ratio_micros + extra.ratio_micros,
            flat_micros: self.flat_micros + extra.flat_micros,
        }
    }

    fn amount(self, hp: u32, bonus_bp: u64) -> u32 {
        let base = u64::from(hp) * self.ratio_micros + self.flat_micros;
        let adjusted = u128::from(base) * u128::from(BP + bonus_bp) / u128::from(MICROS * BP);
        u32::try_from(adjusted).unwrap_or(u32::MAX)
    }
}

struct Lynx {
    maximum_hp_increase: HpAmount,
    skill_heal: HpAmount,
    ultimate_heal: HpAmount,
    continuous_heal: HpAmount,
    response_heal: HpAmount,
    response_turns: u32,
    heal_turns: u32,
    aggro_bonus: u32,
    outgoing_healing_bp: u64,
    recovery_on_hit: bool,
    control_resistance: bool,
    eidolon: u8,
}

fn invalid(id: &str) -> BattleError {
    BattleError::InvalidInput(format!("Lynx {id} parameters"))
}

fn duration(id: &str, value: f64) -> Result<u32, BattleError> {
    if !(1.0..=255.0).contains(&value) || value.fract() != 0.0 {
        return Err(invalid(id));
    }
    Ok(value as u32)
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    let basic = (input.parameters(BASIC, 1)?[0] * BP as f64).round();
    let skill = input.parameters(SKILL, 6)?;
    let ultimate = input.parameters(ULTIMATE, 3)?;
    let talent = input.parameters(TALENT, 5)?;
    if input.energy_capacity != 100 || !(1.0..=f64::from(u16::MAX)).contains(&basic) {
        return Err(invalid(BASIC));
    }
    if ultimate[0] != 1.0 {
        return Err(invalid(ULTIMATE));
    }
    if skill[5] > 10.0 || skill[5].fract() != 0.0 {
        return Err(invalid(SKILL));
    }
    let trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let e6_hp = if input.eidolon >= 6 { 0.06 } else { 0.0 };
    let lynx = Lynx {
        maximum_hp_increase: HpAmount::new(SKILL, skill[0] + e6_hp, skill[1])?,
        skill_heal: HpAmount::new(SKILL, skill[3], skill[4])?,
        ultimate_heal: HpAmount::new(ULTIMATE, ultimate[1], ultimate[2])?,
        continuous_heal: HpAmount::new(TALENT, talent[1], talent[2])?,
        response_heal: HpAmount::new(TALENT, talent[3], talent[4])?,
        response_turns: duration(SKILL, skill[2])?,
        heal_turns: duration(TALENT, talent[0])? + u32::from(trace(SURVIVAL_IN_THE_EXTREME)),
        aggro_bonus: skill[5] as u32,
        outgoing_healing_bp: u64::from(input.stats.healing_bp),
        recovery_on_hit: trace(ADVANCE_SURVEYING),
        control_resistance: trace(EXPLORATION_TECHNIQUES),
        eidolon: input.eidolon,
    };
    Ok(OfficialKit {
        max_energy_milli: 100_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![Effect::MaxHpHit {
                element: Element::Quantum,
                multiplier_bp: basic as u16,
                toughness: 10,
            }],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Ally,
            energy_milli: 30_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::AllAllies,
            energy_milli: 5_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(lynx),
    })
}

impl Lynx {
    fn heal(&self, battle: &mut Battle<'_>, actor: usize, target: usize, amount: HpAmount) {
        let (hp, max_hp) = battle.hp(target);
        let emergency_boost = 2_000 * u64::from(self.eidolon >= 1 && hp <= max_hp / 2);
        let healing = amount.amount(
            battle.hp(actor).1,
            self.outgoing_healing_bp + emergency_boost,
        );
        battle.heal_unit(actor, target, healing);
    }

    fn use_skill(&self, battle: &mut Battle<'_>, source: usize, target: usize) {
        // Refreshing a self-cast uses Max HP before the effect being replaced.
        let old_increase = battle.units[source]
            .lynx_response
            .filter(|response| source == target && response.source == source)
            .map_or(0, |response| response.hp_bonus);
        let caster_hp = battle.hp(source).1.saturating_sub(old_increase);
        let hp_bonus = self.maximum_hp_increase.amount(caster_hp, 0);
        let base_aggro = battle.plan.units[target].stats.base_aggro;
        let aggro_bonus = match base_aggro {
            // The locked promotion tables assign these weights only to Destruction and Preservation.
            125 | 150 => self.aggro_bonus * u32::from(base_aggro),
            _ => 0,
        };
        battle.units[target].lynx_response = Some(LynxResponse {
            source,
            expires_after_turn: battle.units[target].normal_turns + self.response_turns,
            guard: self.eidolon >= 2,
            aggro_bonus,
            hp_bonus,
        });
        battle.emit(
            EventKind::StatusApplied(StatusKind::SurvivalResponse),
            Some(source),
            Some(target),
        );
        battle.apply_max_hp_increase(source, target, hp_bonus, self.response_turns);
        if self.eidolon >= 4 {
            battle.apply_ally_buff(
                source,
                target,
                AllyBuffKind::AttackFlat,
                caster_hp * 3 / 100,
                1,
            );
        }
        if self.eidolon >= 6 {
            battle.apply_ally_buff(
                source,
                target,
                AllyBuffKind::EffectResistance,
                3_000,
                self.response_turns,
            );
        }
        self.heal(battle, source, target, self.skill_heal);
        battle.apply_shared_heal_over_time(source, target, self.heal_turns, Action::Skill);
    }
}

impl Mechanics for Lynx {
    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match action {
            Action::Skill | Action::EnhancedSkill => {
                if let Some(target) = target {
                    self.use_skill(battle, actor, target);
                }
            }
            Action::Ultimate | Action::EnhancedUltimate => {
                for ally in battle
                    .allies()
                    .filter(|&target| battle.hp(target).0 > 0)
                    .collect::<Vec<_>>()
                {
                    battle.cleanse_one(actor, ally);
                    self.heal(battle, actor, ally, self.ultimate_heal);
                    battle.apply_shared_heal_over_time(
                        actor,
                        ally,
                        self.heal_turns,
                        Action::Ultimate,
                    );
                }
            }
            Action::Basic => {}
        }
    }

    fn heal_over_time(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        _origin: Action,
    ) {
        let amount = if battle.units[target].lynx_response.is_some() {
            self.continuous_heal.add(self.response_heal)
        } else {
            self.continuous_heal
        };
        self.heal(battle, actor, target, amount);
    }

    fn after_enemy_attack_seen(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _attacker: usize,
        target: usize,
        _was_shielded: bool,
    ) {
        if self.recovery_on_hit
            && battle.units[target]
                .lynx_response
                .is_some_and(|response| response.source == owner)
        {
            battle.gain_energy(owner, owner, 2_000);
        }
    }

    fn control_resistance_bp(&self) -> u32 {
        3_500 * u32::from(self.control_resistance)
    }
}

impl Battle<'_> {
    pub(in crate::battle) fn lynx_aggro_bonus(&self, target: usize) -> u32 {
        self.units[target]
            .lynx_response
            .map_or(0, |response| response.aggro_bonus)
    }

    pub(in crate::battle) fn use_survival_response_guard(&mut self, target: usize) -> bool {
        self.units[target]
            .lynx_response
            .as_mut()
            .is_some_and(|response| std::mem::take(&mut response.guard))
    }

    pub(in crate::battle) fn expire_survival_response(&mut self, target: usize) {
        let state = &mut self.units[target];
        if let Some(response) = state.lynx_response
            && response.expires_after_turn <= state.normal_turns
        {
            state.lynx_response = None;
            self.emit(
                EventKind::StatusExpired(StatusKind::SurvivalResponse),
                Some(response.source),
                Some(target),
            );
        }
    }
}
