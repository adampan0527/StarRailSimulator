use std::sync::Arc;

use super::{
    super::{Battle, combat_status::DebuffKind, support::AllyBuffKind},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};
use crate::{
    Action, BattleError,
    fixture::{Effect, Element, Target},
};

#[cfg(test)]
mod tests;

const BASIC: &str = "skill.130101";
const SKILL: &str = "skill.130102";
const ULTIMATE: &str = "skill.130103";
const TALENT: &str = "skill.130104";
const ENHANCED_BASIC: &str = "skill.130108";
const NOVEL_CONCOCTION: &str = "trace.1301101";
const ORGANIC_YEAST: &str = "trace.1301102";
const BOTTOMS_UP: &str = "trace.1301103";
const BP: u32 = 10_000;
const MICROS: u64 = 1_000_000;
const E6_BREAK_BONUS_BP: u32 = 2_000;

#[derive(Clone, Default)]
pub(in crate::battle) struct GallagherState {
    enhanced_basic: bool,
    besotted_hits: u32,
}

struct Gallagher {
    enhanced_basic: OfficialAbility,
    skill_heal_micros: u64,
    talent_heal_micros: u64,
    healing_bonus_bp: u32,
    break_effect_to_healing_bp: Option<u32>,
    besotted_bp: u32,
    besotted_turns: u32,
    attack_down_bp: u32,
    attack_down_turns: u32,
    immediate_turn: bool,
    team_heal: bool,
    eidolon: u8,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 110 || input.eidolon > 6 {
        return Err(BattleError::InvalidInput(
            "Gallagher energy capacity or eidolon".into(),
        ));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 1)?;
    let ultimate = input.parameters(ULTIMATE, 2)?;
    let talent = input.parameters(TALENT, 2)?;
    let enhanced = input.parameters(ENHANCED_BASIC, 3)?;
    let trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let break_effect = input
        .stats
        .break_effect_bp
        .saturating_add(break_bonus(input.eidolon));
    let break_effect_to_healing_bp = trace(NOVEL_CONCOCTION).then_some(break_effect);
    let toughness = |base: u16| base + if input.eidolon >= 6 { base / 5 } else { 0 };
    Ok(OfficialKit {
        max_energy_milli: 110_000,
        basic: fire_attack(
            Target::Enemy,
            multiplier(BASIC, basic[0])?,
            toughness(10),
            20_000,
            SkillPoints::Generate,
        ),
        skill: OfficialAbility {
            target: Target::Ally,
            energy_milli: 30_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Spend,
        },
        ultimate: fire_attack(
            Target::AllEnemies,
            multiplier(ULTIMATE, ultimate[0])?,
            toughness(20),
            5_000,
            SkillPoints::Unchanged,
        ),
        mechanics: Arc::new(Gallagher {
            enhanced_basic: fire_attack(
                Target::Enemy,
                multiplier(ENHANCED_BASIC, enhanced[0])?,
                toughness(30),
                20_000,
                SkillPoints::Generate,
            ),
            skill_heal_micros: flat_heal(SKILL, skill[0])?,
            talent_heal_micros: flat_heal(TALENT, talent[1])?,
            healing_bonus_bp: input.stats.healing_bp,
            break_effect_to_healing_bp,
            besotted_bp: ratio_bp(TALENT, talent[0])?,
            besotted_turns: duration(ULTIMATE, ultimate[1])? + u32::from(input.eidolon >= 4),
            attack_down_bp: ratio_bp(ENHANCED_BASIC, enhanced[1])?,
            attack_down_turns: duration(ENHANCED_BASIC, enhanced[2])?,
            immediate_turn: trace(ORGANIC_YEAST),
            team_heal: trace(BOTTOMS_UP),
            eidolon: input.eidolon,
        }),
    })
}

fn fire_attack(
    target: Target,
    multiplier_bp: u16,
    toughness: u16,
    energy_milli: u32,
    skill_points: SkillPoints,
) -> OfficialAbility {
    OfficialAbility {
        target,
        energy_milli,
        effects: vec![Effect::Hit {
            element: Element::Fire,
            multiplier_bp,
            toughness,
        }],
        skill_points,
    }
}

fn invalid_parameter(skill: &str) -> BattleError {
    BattleError::InvalidInput(format!("Gallagher {skill} has an out-of-range parameter"))
}

fn multiplier(skill: &str, value: f64) -> Result<u16, BattleError> {
    let rounded = (value * f64::from(BP)).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&rounded) {
        return Err(invalid_parameter(skill));
    }
    Ok(rounded as u16)
}

fn ratio_bp(skill: &str, value: f64) -> Result<u32, BattleError> {
    if value > 1.0 {
        return Err(invalid_parameter(skill));
    }
    Ok((value * f64::from(BP)).round() as u32)
}

fn duration(skill: &str, value: f64) -> Result<u32, BattleError> {
    if !(1.0..=30.0).contains(&value) || value.fract() != 0.0 {
        return Err(invalid_parameter(skill));
    }
    Ok(value as u32)
}

fn flat_heal(skill: &str, value: f64) -> Result<u64, BattleError> {
    if !(0.0..=1_000_000.0).contains(&value) {
        return Err(invalid_parameter(skill));
    }
    Ok((value * MICROS as f64).round() as u64)
}

fn break_bonus(eidolon: u8) -> u32 {
    if eidolon >= 6 { E6_BREAK_BONUS_BP } else { 0 }
}

impl Gallagher {
    fn heal(&self, battle: &mut Battle<'_>, owner: usize, target: usize, base_micros: u64) {
        if battle.hp(target).0 == 0 {
            return;
        }
        let converted_healing_bp = self.break_effect_to_healing_bp.map_or(0, |break_effect| {
            (break_effect.saturating_add(battle.support_break_effect(owner)) / 2).min(7_500)
        });
        let healing_bp = self.healing_bonus_bp.saturating_add(converted_healing_bp);
        let scaled = u128::from(base_micros) * (u128::from(BP) + u128::from(healing_bp));
        let amount =
            u32::try_from(scaled / (u128::from(MICROS) * u128::from(BP))).unwrap_or(u32::MAX);
        battle.heal_unit(owner, target, amount);
    }
}

impl Mechanics for Gallagher {
    fn ability_override(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        (action == Action::Basic && battle.units[actor].gallagher_state.enhanced_basic)
            .then_some(&self.enhanced_basic)
    }

    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.eidolon >= 1 {
            battle.gain_energy(actor, actor, 20_000);
        }
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _target: Option<usize>,
    ) {
        if action == Action::Ultimate {
            for target in battle.enemies() {
                battle.apply_debuff(
                    actor,
                    target,
                    DebuffKind::Besotted,
                    self.besotted_bp,
                    self.besotted_turns,
                );
            }
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match (action, target) {
            (Action::Ultimate, _) => {
                battle.units[actor].gallagher_state.enhanced_basic = true;
                if self.immediate_turn {
                    battle.advance_immediate(actor);
                }
            }
            (Action::Basic, Some(target)) if battle.units[actor].gallagher_state.enhanced_basic => {
                battle.apply_debuff(
                    actor,
                    target,
                    DebuffKind::AttackDown,
                    self.attack_down_bp,
                    self.attack_down_turns,
                );
            }
            (Action::Skill, Some(target)) => {
                if self.eidolon >= 2 {
                    battle.cleanse_one(actor, target);
                    battle.apply_ally_buff(actor, target, AllyBuffKind::EffectResistance, 3_000, 2);
                }
                self.heal(battle, actor, target, self.skill_heal_micros);
            }
            _ => {}
        }
    }

    fn on_attack_target(&self, battle: &mut Battle<'_>, owner: usize, target: usize) {
        // Record the mark before damage so a finishing blow still heals the attacker.
        if battle.has_besotted_from(target, owner) {
            battle.units[owner].gallagher_state.besotted_hits += 1;
        }
    }

    fn on_attack_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        origin: Action,
    ) {
        let state = &mut battle.units[owner].gallagher_state;
        let enhanced =
            actor == owner && origin == Action::Basic && std::mem::take(&mut state.enhanced_basic);
        let hits = std::mem::take(&mut state.besotted_hits);
        for _ in 0..hits {
            if enhanced && self.team_heal {
                for target in battle.allies() {
                    self.heal(battle, owner, target, self.talent_heal_micros);
                }
            } else {
                self.heal(battle, owner, actor, self.talent_heal_micros);
            }
        }
    }

    fn effect_resistance_bonus_bp(&self, _battle: &Battle<'_>, _actor: usize) -> u32 {
        if self.eidolon >= 1 { 5_000 } else { 0 }
    }

    fn break_effect_bonus_bp(&self, _battle: &Battle<'_>, _actor: usize) -> u32 {
        break_bonus(self.eidolon)
    }
}
