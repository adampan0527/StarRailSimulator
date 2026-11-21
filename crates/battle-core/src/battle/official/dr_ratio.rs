use std::sync::Arc;

use super::{
    super::{Battle, resolve::FollowUpAttack},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};
use crate::{
    Action, BattleError, DamageKind, EventKind, StatusKind,
    fixture::{Effect, Element, Target},
};

#[cfg(test)]
mod tests;

const FORM: &str = "character.1305";
const BASIC: &str = "skill.130501";
const SKILL: &str = "skill.130502";
const ULTIMATE: &str = "skill.130503";
const TALENT: &str = "skill.130504";
const SUMMATION: &str = "trace.1305101";
const INFERENCE: &str = "trace.1305102";
const DEDUCTION: &str = "trace.1305103";
const FOLLY: StatusKind = StatusKind::WisemansFolly;

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 140 {
        return Err(invalid("energy capacity"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 1)?;
    let ultimate = input.parameters(ULTIMATE, 2)?;
    let talent = input.parameters(TALENT, 3)?;
    if ultimate[1] != 2.0 || talent[1] > 1.0 || talent[2] > 1.0 {
        return Err(invalid("ultimate trigger count or talent chance"));
    }
    let has_trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let strike = |id, ratio, toughness, energy_milli, skill_points| {
        Ok(OfficialAbility {
            target: Target::Enemy,
            energy_milli,
            effects: vec![Effect::Hit {
                element: Element::Imaginary,
                multiplier_bp: points(id, ratio)?,
                toughness,
            }],
            skill_points,
        })
    };
    Ok(OfficialKit {
        max_energy_milli: 140_000,
        basic: strike(BASIC, basic[0], 10, 20_000, SkillPoints::Generate)?,
        skill: strike(SKILL, skill[0], 20, 30_000, SkillPoints::Spend)?,
        ultimate: strike(ULTIMATE, ultimate[0], 30, 5_000, SkillPoints::Unchanged)?,
        mechanics: Arc::new(DrRatio {
            eidolon: input.eidolon,
            summation: has_trace(SUMMATION),
            inference: has_trace(INFERENCE),
            deduction: has_trace(DEDUCTION),
            follow_up_chance_bp: (talent[1] * 10_000.0).round() as u32,
            chance_per_debuff_bp: (talent[2] * 10_000.0).round() as u32,
            follow_up: FollowUpAttack {
                element: Element::Imaginary,
                multiplier_bp: points(TALENT, talent[0])?,
                toughness: 10,
                energy_milli: 5_000,
                first_hit: None,
            },
        }),
    })
}

fn invalid(part: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: invalid {part}"))
}

fn points(id: &str, ratio: f64) -> Result<u16, BattleError> {
    let value = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&value) {
        return Err(invalid(id));
    }
    Ok(value as u16)
}

#[derive(Clone, Copy)]
struct Mark {
    target: usize,
    remaining: u8,
}

#[derive(Clone, Default)]
pub(in crate::battle) struct DrRatioState {
    summation_stacks: u8,
    skill_debuffs: usize,
    mark: Option<Mark>,
}

struct DrRatio {
    eidolon: u8,
    summation: bool,
    inference: bool,
    deduction: bool,
    follow_up_chance_bp: u32,
    chance_per_debuff_bp: u32,
    follow_up: FollowUpAttack,
}

fn clear_mark(battle: &mut Battle<'_>, actor: usize) {
    if let Some(mark) = battle.units[actor].dr_ratio_state.mark.take() {
        battle.emit(
            EventKind::StatusExpired(FOLLY),
            Some(actor),
            Some(mark.target),
        );
    }
}

impl Mechanics for DrRatio {
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.eidolon >= 1 && self.summation {
            battle.units[actor].dr_ratio_state.summation_stacks = 4;
        }
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        if action != Action::Skill {
            return;
        }
        let Some(target) = target else { return };
        let count = battle.debuff_count(target);
        let state = &mut battle.units[actor].dr_ratio_state;
        state.skill_debuffs = count;
        if self.summation {
            let cap = if self.eidolon >= 1 { 10 } else { 6 };
            state.summation_stacks =
                (state.summation_stacks + count.min(usize::from(cap)) as u8).min(cap);
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        let Some(target) = target else { return };
        let target_survives = battle.hp(target).0 > 0;
        match action {
            Action::Basic => {}
            Action::Skill => {
                let count = if target_survives {
                    if self.inference && battle.roll_effect(actor, target, 10_000) {
                        battle.apply_effect_resistance_down(actor, target, 1_000, 2);
                    }
                    battle.debuff_count(target)
                } else {
                    // Defeat clears statuses before the Talent check.
                    battle.units[actor].dr_ratio_state.skill_debuffs
                };
                let count = u32::try_from(count).unwrap_or(u32::MAX);
                let chance = self
                    .follow_up_chance_bp
                    .saturating_add(self.chance_per_debuff_bp.saturating_mul(count));
                if battle.roll_fixed(chance) {
                    battle.follow_up_single_or_random(
                        actor,
                        Some(target),
                        action,
                        self.follow_up.clone(),
                    );
                }
            }
            Action::Ultimate => {
                let mark = battle.units[actor].dr_ratio_state.mark;
                if mark.is_some_and(|mark| mark.target != target) {
                    clear_mark(battle, actor);
                }
                if target_survives {
                    battle.units[actor].dr_ratio_state.mark = Some(Mark {
                        target,
                        remaining: if self.eidolon >= 6 { 3 } else { 2 },
                    });
                    battle.emit(EventKind::StatusApplied(FOLLY), Some(actor), Some(target));
                }
            }
        }
    }

    fn on_attack_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        origin: Action,
    ) {
        if source == owner {
            return;
        }
        let Some(mut mark) = battle.units[owner].dr_ratio_state.mark else {
            return;
        };
        if !battle.attack_targets().contains(&mark.target) {
            return;
        }
        battle.follow_up_single(owner, mark.target, origin, self.follow_up.clone());
        mark.remaining -= 1;
        if mark.remaining == 0 {
            clear_mark(battle, owner);
        } else {
            battle.units[owner].dr_ratio_state.mark = Some(mark);
        }
    }

    fn on_enemy_defeat(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _killer: usize,
        target: usize,
    ) {
        if battle.units[owner]
            .dr_ratio_state
            .mark
            .is_some_and(|mark| mark.target == target)
        {
            clear_mark(battle, owner);
        }
    }

    fn after_follow_up_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        origin: Action,
    ) {
        if self.eidolon >= 2 && battle.hp(target).0 > 0 {
            for _ in 0..battle.debuff_count(target).min(4) {
                battle.attack_extra_hit(actor, target, Element::Imaginary, 2_000, 0, origin);
            }
        }
    }

    fn after_follow_up(&self, battle: &mut Battle<'_>, actor: usize, _origin: Action) {
        if self.eidolon >= 4 {
            battle.gain_energy(actor, actor, 15_000);
        }
    }

    fn crit_rate_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _target: usize,
        _action: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        u32::from(battle.units[actor].dr_ratio_state.summation_stacks) * 250
    }

    fn crit_damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _target: usize,
        _action: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        u32::from(battle.units[actor].dr_ratio_state.summation_stacks) * 500
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        _actor: usize,
        target: usize,
        _action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        let deduction = if self.deduction {
            match battle.debuff_count(target) {
                0..=2 => 0,
                3 => 3_000,
                4 => 4_000,
                _ => 5_000,
            }
        } else {
            0
        };
        deduction
            + if self.eidolon >= 6 && kind == DamageKind::FollowUp {
                5_000
            } else {
                0
            }
    }
}
