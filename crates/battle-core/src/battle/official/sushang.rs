use std::sync::Arc;

use super::super::Battle;
use super::{Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints};
use crate::{
    Action, BattleError, DamageKind,
    fixture::{Effect, Element, Target},
};

#[cfg(test)]
mod tests;

const BASIC: &str = "skill.120601";
const SKILL: &str = "skill.120602";
const ULTIMATE: &str = "skill.120603";
const TALENT: &str = "skill.120604";
const RIPOSTE: &str = "trace.1206102";
const VANQUISHER: &str = "trace.1206103";
const RIPOSTE_STACK_BP: u32 = 250;
const RIPOSTE_CAP: u8 = 10;
const VANQUISHER_ADVANCE_BP: u16 = 1_500;

struct Sushang {
    eidolon: u8,
    stance_multiplier_bp: u16,
    stance_chance_bp: u32,
    extra_stance_multiplier_bp: u16,
    ultimate_turns: u32,
    ultimate_attack_bp: u32,
    talent_speed_bp: u32,
    talent_turns: u32,
    riposte: bool,
    vanquisher: bool,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 120 {
        return Err(invalid_parameter("energy capacity"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 3)?;
    let ultimate = input.parameters(ULTIMATE, 4)?;
    let talent = input.parameters(TALENT, 2)?;
    if skill[2] > 1.0 || ultimate[1] != 2.0 || ultimate[2] > 1.0 || talent[1] != 2.0 {
        return Err(invalid_parameter("chance, scaling or duration"));
    }
    let skill_multiplier_bp = basis_points(SKILL, skill[0])?;
    let first_hit_bp = ((u32::from(skill_multiplier_bp) * 30 + 50) / 100) as u16;
    let stance_multiplier_bp = basis_points(SKILL, skill[1])?;
    let extra_stance_scale_bp = basis_points(ULTIMATE, ultimate[2])?;
    let extra_stance_multiplier_bp =
        ((u32::from(stance_multiplier_bp) * u32::from(extra_stance_scale_bp) + 5_000) / 10_000)
            as u16;
    let trace_enabled = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);

    Ok(OfficialKit {
        max_energy_milli: 120_000,
        basic: attack(
            vec![hit(basis_points(BASIC, basic[0])?, 10)],
            20_000,
            SkillPoints::Generate,
        ),
        skill: attack(
            vec![
                hit(first_hit_bp, 6),
                hit(skill_multiplier_bp - first_hit_bp, 14),
            ],
            30_000,
            SkillPoints::Spend,
        ),
        ultimate: attack(
            vec![hit(basis_points(ULTIMATE, ultimate[0])?, 30)],
            5_000,
            SkillPoints::Unchanged,
        ),
        mechanics: Arc::new(Sushang {
            eidolon: input.eidolon,
            stance_multiplier_bp,
            stance_chance_bp: u32::from(basis_points(SKILL, skill[2])?),
            extra_stance_multiplier_bp,
            ultimate_turns: ultimate[1] as u32,
            ultimate_attack_bp: u32::from(basis_points(ULTIMATE, ultimate[3])?),
            talent_speed_bp: u32::from(basis_points(TALENT, talent[0])?),
            talent_turns: talent[1] as u32,
            riposte: trace_enabled(RIPOSTE),
            vanquisher: trace_enabled(VANQUISHER),
        }),
    })
}

fn invalid_parameter(parameter: &str) -> BattleError {
    BattleError::InvalidInput(format!("character.1206: {parameter}"))
}

fn basis_points(id: &str, ratio: f64) -> Result<u16, BattleError> {
    let points = (ratio * 10_000.0).round();
    if (1.0..=f64::from(u16::MAX)).contains(&points) {
        Ok(points as u16)
    } else {
        Err(invalid_parameter(id))
    }
}

fn hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: Element::Physical,
        multiplier_bp,
        toughness,
    }
}

fn attack(effects: Vec<Effect>, energy_milli: u32, skill_points: SkillPoints) -> OfficialAbility {
    OfficialAbility {
        target: Target::Enemy,
        energy_milli,
        effects,
        skill_points,
    }
}

fn is_broken(battle: &Battle<'_>, target: usize) -> bool {
    battle.units[target].toughness == 0 && battle.plan.units[target].kit.max_toughness() > 0
}

impl Sushang {
    fn add_speed_stack(&self, battle: &mut Battle<'_>, actor: usize) {
        let unit = &mut battle.units[actor];
        let expirations = &mut unit.sushang_state.speed_expirations;
        // Replace the oldest stack. E0 always refreshes the only available slot.
        let slot = usize::from(self.eidolon >= 6 && expirations[0] > expirations[1]);
        expirations[slot] = unit.normal_turns + self.talent_turns;
    }

    fn resolve_skill(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        let unit = &mut battle.units[actor];
        let started_broken = std::mem::take(&mut unit.sushang_state.target_was_broken);
        let enhanced = unit.sushang_state.ultimate_expires_after_turn > unit.finished_turns;
        let broken_after_hit = is_broken(battle, target);

        self.try_stance(
            battle,
            actor,
            target,
            broken_after_hit,
            self.stance_multiplier_bp,
        );
        if enhanced {
            for _ in 0..2 {
                self.try_stance(
                    battle,
                    actor,
                    target,
                    broken_after_hit,
                    self.extra_stance_multiplier_bp,
                );
            }
        }
        if self.eidolon >= 1 && started_broken {
            battle.restore_skill_points(actor, 1);
        }
    }

    fn try_stance(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        guaranteed: bool,
        multiplier_bp: u16,
    ) {
        if battle.hp(target).0 == 0 || !(guaranteed || battle.roll_fixed(self.stance_chance_bp)) {
            return;
        }
        let unit = &mut battle.units[actor];
        if self.riposte {
            let stacks = &mut unit.sushang_state.sword_stance_stacks;
            *stacks = stacks.saturating_add(1).min(RIPOSTE_CAP);
        }
        if self.eidolon >= 2 {
            unit.sushang_state.damage_reduction_expires_after_turn = unit.normal_turns + 1;
        }
        battle.secondary_hit(
            actor,
            target,
            Element::Physical,
            multiplier_bp,
            DamageKind::Additional,
            Action::Skill,
        );
    }
}

impl Mechanics for Sushang {
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.eidolon >= 6 {
            self.add_speed_stack(battle, actor);
        }
    }

    fn on_weakness_break(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _source: usize,
        _target: usize,
        _origin: Action,
    ) {
        self.add_speed_stack(battle, owner);
    }

    fn speed_bonus_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        let unit = &battle.units[actor];
        let active = unit
            .sushang_state
            .speed_expirations
            .into_iter()
            .filter(|&expiry| expiry > unit.finished_turns)
            .count() as u32;
        active * self.talent_speed_bp
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        if action == Action::Skill {
            battle.units[actor].sushang_state.target_was_broken =
                target.is_some_and(|target| is_broken(battle, target));
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match action {
            Action::AssistSkill | Action::ElationSkill => {}
            Action::Skill | Action::EnhancedSkill => {
                if let Some(target) = target {
                    self.resolve_skill(battle, actor, target);
                }
            }
            Action::Ultimate | Action::EnhancedUltimate => {
                let unit = &mut battle.units[actor];
                unit.sushang_state.ultimate_expires_after_turn =
                    unit.normal_turns + self.ultimate_turns;
                battle.apply_attack_bonus(actor, self.ultimate_attack_bp, self.ultimate_turns);
                battle.advance_immediate(actor);
            }
            Action::Basic => {}
        }
        if self.vanquisher
            && matches!(action, Action::Basic | Action::Skill)
            && battle
                .enemies()
                .any(|target| battle.hp(target).0 > 0 && is_broken(battle, target))
        {
            battle.advance(actor, VANQUISHER_ADVANCE_BP);
        }
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _target: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        if action == Some(Action::Skill) && kind == DamageKind::Additional {
            u32::from(battle.units[actor].sushang_state.sword_stance_stacks) * RIPOSTE_STACK_BP
        } else {
            0
        }
    }

    fn incoming_damage_reduction_bp(
        &self,
        battle: &Battle<'_>,
        owner: usize,
        _source: usize,
        _kind: DamageKind,
    ) -> u32 {
        let unit = &battle.units[owner];
        if unit.sushang_state.damage_reduction_expires_after_turn > unit.finished_turns {
            2_000
        } else {
            0
        }
    }

    fn break_effect_bonus_bp(&self, _battle: &Battle<'_>, _actor: usize) -> u32 {
        if self.eidolon >= 4 { 4_000 } else { 0 }
    }
}
