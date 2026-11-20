use std::sync::Arc;

use super::super::{
    Battle,
    periodic::{DotScale, SkillDot},
};
use super::{Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints};
use crate::{
    Action, BattleError, DamageKind,
    fixture::{Effect, Element, Target},
};

#[cfg(test)]
mod tests;

const FORM: &str = "character.1111";
const BASIC: &str = "skill.111101";
const SKILL: &str = "skill.111102";
const ULTIMATE: &str = "skill.111103";
const TALENT: &str = "skill.111104";
const ENHANCED_BASIC: &str = "skill.111108";
const KINETIC_OVERLOAD: &str = "trace.1111101";
const CYCLE_BRAKING: &str = "trace.1111102";
const CRUSH_FIGHTING_WILL: &str = "trace.1111103";
const FIGHTING_WILL_CAP: u8 = 4;
const ENHANCED_COST: u8 = 2;
const JAB_COUNT: u8 = 3;
const E1_BONUS_BP: u32 = 1_500;
const E1_BONUS_TURNS: u32 = 2;
const E4_ATTACK_PER_STACK_BP: u32 = 500;
const E6_DETONATION_BP: u16 = 800;

struct Luka {
    enhanced_basic: OfficialAbility,
    jab_bp: u16,
    bleed: SkillDot,
    bleed_chance_bp: u32,
    vulnerability_bp: u32,
    vulnerability_turns: u32,
    vulnerability_chance_bp: u32,
    ordinary_fighting_will: u8,
    ultimate_fighting_will: u8,
    detonation_bp: u16,
    eidolon: u8,
    dispel_trace: bool,
    energy_trace: bool,
    extra_jab_trace: bool,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 130 || input.eidolon > 6 {
        return Err(invalid("energy capacity or eidolon"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 5)?;
    let ultimate = input.parameters(ULTIMATE, 5)?;
    let talent = input.parameters(TALENT, 2)?;
    let enhanced = input.parameters(ENHANCED_BASIC, 2)?;
    if skill[1] > 1.0 || skill[2] > 1.0 || ultimate[1] > 1.0 || talent[1] > 1.0 {
        return Err(invalid("application chance or bleed ratio"));
    }
    let jab_bp = basis_points(ENHANCED_BASIC, enhanced[0])?;
    let uppercut_bp = basis_points(ENHANCED_BASIC, enhanced[1])?;
    let enabled_trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);

    Ok(OfficialKit {
        max_energy_milli: 130_000,
        basic: attack(
            vec![hit(basis_points(BASIC, basic[0])?, 10)],
            20_000,
            SkillPoints::Generate,
        ),
        skill: attack(
            vec![hit(basis_points(SKILL, skill[0])?, 20)],
            30_000,
            SkillPoints::Spend,
        ),
        ultimate: attack(
            vec![hit(basis_points(ULTIMATE, ultimate[0])?, 30)],
            5_000,
            SkillPoints::Unchanged,
        ),
        mechanics: Arc::new(Luka {
            enhanced_basic: attack(
                vec![
                    hit(jab_bp, 3),
                    hit(jab_bp, 3),
                    hit(jab_bp, 3),
                    hit(uppercut_bp, 11),
                ],
                20_000,
                SkillPoints::Generate,
            ),
            jab_bp,
            bleed: SkillDot {
                element: Element::Physical,
                damage: DotScale::TargetHp {
                    hp_bp: basis_points(SKILL, skill[2])?,
                    attack_cap_bp: basis_points(SKILL, skill[3])?,
                },
                origin: Action::Skill,
                turns: whole_number(SKILL, skill[4], 30)?,
                max_stacks: 1,
            },
            bleed_chance_bp: u32::from(basis_points(SKILL, skill[1])?),
            vulnerability_bp: u32::from(basis_points(ULTIMATE, ultimate[2])?),
            vulnerability_turns: whole_number(ULTIMATE, ultimate[3], 30)?,
            vulnerability_chance_bp: u32::from(basis_points(ULTIMATE, ultimate[1])?),
            ordinary_fighting_will: whole_number(TALENT, talent[0], FIGHTING_WILL_CAP)? as u8,
            ultimate_fighting_will: whole_number(ULTIMATE, ultimate[4], FIGHTING_WILL_CAP)? as u8,
            detonation_bp: basis_points(TALENT, talent[1])?,
            eidolon: input.eidolon,
            dispel_trace: enabled_trace(KINETIC_OVERLOAD),
            energy_trace: enabled_trace(CYCLE_BRAKING),
            extra_jab_trace: enabled_trace(CRUSH_FIGHTING_WILL),
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: invalid {detail}"))
}

fn basis_points(skill: &str, ratio: f64) -> Result<u16, BattleError> {
    let rounded = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&rounded) {
        return Err(invalid(skill));
    }
    Ok(rounded as u16)
}

fn whole_number(skill: &str, value: f64, max: u8) -> Result<u32, BattleError> {
    if value.fract() != 0.0 || !(1.0..=f64::from(max)).contains(&value) {
        return Err(invalid(skill));
    }
    Ok(value as u32)
}

fn attack(effects: Vec<Effect>, energy_milli: u32, skill_points: SkillPoints) -> OfficialAbility {
    OfficialAbility {
        target: Target::Enemy,
        energy_milli,
        effects,
        skill_points,
    }
}

fn hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: Element::Physical,
        multiplier_bp,
        toughness,
    }
}

impl Luka {
    fn gain_fighting_will(&self, battle: &mut Battle<'_>, actor: usize, requested: u8) {
        let state = &mut battle.units[actor].luka_state;
        let gained = requested.min(FIGHTING_WILL_CAP - state.fighting_will);
        state.fighting_will += gained;
        if self.eidolon >= 4 {
            state.attack_stacks = (state.attack_stacks + gained).min(FIGHTING_WILL_CAP);
        }
        if self.energy_trace && gained > 0 {
            battle.gain_energy(actor, actor, 3_000 * u32::from(gained));
        }
    }

    fn detonate_bleeds(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        let state = &battle.units[actor].luka_state;
        let jabs = state.jabs + state.extra_jabs;
        battle.detonate_dots(
            actor,
            target,
            Element::Physical,
            self.detonation_bp,
            Action::Basic,
        );
        if self.eidolon >= 6 {
            for _ in 0..jabs {
                battle.detonate_dots(
                    actor,
                    target,
                    Element::Physical,
                    E6_DETONATION_BP,
                    Action::Basic,
                );
            }
        }
    }
}

impl Mechanics for Luka {
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        battle.units[actor].luka_state.fighting_will = 1;
    }

    fn ability_override(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        (action == Action::Basic && battle.units[actor].luka_state.fighting_will >= ENHANCED_COST)
            .then_some(&self.enhanced_basic)
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        let Some(target) = target else {
            return;
        };
        if self.eidolon >= 1 && battle.has_elemental_dot(target, Element::Physical) {
            let state = &mut battle.units[actor];
            state.luka_state.dmg_bonus_expires_after_turn = state.normal_turns + E1_BONUS_TURNS;
        }
        match action {
            Action::Basic => {
                let state = &mut battle.units[actor].luka_state;
                state.enhanced_basic = state.fighting_will >= ENHANCED_COST;
                if state.enhanced_basic {
                    state.fighting_will -= ENHANCED_COST;
                    state.jabs = 0;
                    state.extra_jabs = 0;
                }
            }
            Action::Skill => {
                if self.dispel_trace {
                    battle.dispel_one(actor, target);
                }
            }
            Action::Ultimate => {
                self.gain_fighting_will(battle, actor, self.ultimate_fighting_will);
                if battle.roll_effect(actor, target, self.vulnerability_chance_bp) {
                    battle.apply_vulnerability(
                        actor,
                        target,
                        self.vulnerability_bp,
                        self.vulnerability_turns,
                    );
                }
            }
        }
    }

    fn after_attack_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        action: Action,
        _hp_before: u32,
    ) {
        let state = &mut battle.units[actor].luka_state;
        if action != Action::Basic || !state.enhanced_basic {
            return;
        }
        if state.jabs == JAB_COUNT {
            self.detonate_bleeds(battle, actor, target);
        } else {
            state.jabs += 1;
            if self.extra_jab_trace && battle.hp(target).0 > 0 && battle.roll_fixed(5_000) {
                // The inserted hit runs the ordinary attack hooks. Disable the
                // enhanced-basic marker for that hit to avoid a second A6 roll.
                battle.units[actor].luka_state.enhanced_basic = false;
                battle.attack_hit(actor, target, Element::Physical, self.jab_bp, 0);
                let state = &mut battle.units[actor].luka_state;
                state.enhanced_basic = true;
                state.extra_jabs += 1;
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
            (Action::Basic, _) => {
                let state = &mut battle.units[actor].luka_state;
                if std::mem::take(&mut state.enhanced_basic) {
                    return;
                }
                self.gain_fighting_will(battle, actor, self.ordinary_fighting_will);
            }
            (Action::Skill, Some(target)) => {
                battle.apply_skill_dot(actor, target, self.bleed_chance_bp, self.bleed);
                let e2_stack = self.eidolon >= 2 && battle.has_weakness(target, Element::Physical);
                self.gain_fighting_will(
                    battle,
                    actor,
                    self.ordinary_fighting_will + u8::from(e2_stack),
                );
            }
            _ => {}
        }
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _target: usize,
        _action: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        let state = &battle.units[actor];
        if state.finished_turns < state.luka_state.dmg_bonus_expires_after_turn {
            E1_BONUS_BP
        } else {
            0
        }
    }

    fn bonus_attack_flat(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        let attack = battle.plan.units[actor].stats.base_attack;
        let stacks = u32::from(battle.units[actor].luka_state.attack_stacks);
        attack * stacks * E4_ATTACK_PER_STACK_BP / 10_000
    }
}
