use std::{cmp::Reverse, sync::Arc};

use super::{
    super::{Battle, support::AllyBuffKind},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};
use crate::{
    Action, BattleError,
    fixture::{Ability, Effect, Element, Target},
};

#[cfg(test)]
mod tests;

const FORM: &str = "character.1215";
const BASIC: &str = "skill.121501";
const SKILL: &str = "skill.121502";
const ULTIMATE: &str = "skill.121503";
const TALENT: &str = "skill.121504";
const SCRIVENER: &str = "trace.1215101";
const NETHERWORLD: &str = "trace.1215102";
const REANIMATED: &str = "trace.1215103";
const SPEED_RATIO_SCALE: u64 = 1_000_000;
const ACTIONS_PER_REFUND: u8 = 2;

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 140 {
        return Err(BattleError::InvalidInput(format!(
            "{FORM}: energy capacity"
        )));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 2)?;
    let ultimate = input.parameters(ULTIMATE, 3)?;
    let talent = input.parameters(TALENT, 2)?;
    let has_trace = |trace| input.enabled_trace_ids.iter().any(|id| id == trace);
    if !(0.0 < ultimate[2] && ultimate[2] <= 1.0) {
        return Err(BattleError::InvalidInput(format!(
            "{FORM}: {ULTIMATE} speed ratio"
        )));
    }
    let mechanics = Hanya {
        burden_refund_limit: count(SKILL, skill[1])?,
        ultimate_attack_bp: u32::from(basis_points(ULTIMATE, ultimate[0])?),
        ultimate_turns: u32::from(count(ULTIMATE, ultimate[1])?) + u32::from(input.eidolon >= 4),
        ultimate_speed_ratio_micros: (ultimate[2] * SPEED_RATIO_SCALE as f64).round() as u64,
        talent_damage_bp: u32::from(basis_points(TALENT, talent[0])?)
            + if input.eidolon >= 6 { 1_000 } else { 0 },
        talent_turns: u32::from(count(TALENT, talent[1])?),
        scrivener: has_trace(SCRIVENER),
        netherworld: has_trace(NETHERWORLD),
        reanimated: has_trace(REANIMATED),
        eidolon: input.eidolon,
    };
    let strike = |id, ratio, toughness, energy_milli, skill_points| {
        Ok(OfficialAbility {
            target: Target::Enemy,
            energy_milli,
            effects: vec![Effect::Hit {
                element: Element::Physical,
                multiplier_bp: basis_points(id, ratio)?,
                toughness,
            }],
            skill_points,
        })
    };
    Ok(OfficialKit {
        max_energy_milli: 140_000,
        basic: strike(BASIC, basic[0], 10, 20_000, SkillPoints::Generate)?,
        skill: strike(SKILL, skill[0], 20, 30_000, SkillPoints::Spend)?,
        ultimate: OfficialAbility {
            target: Target::Ally,
            energy_milli: 5_000,
            effects: Vec::new(),
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(mechanics),
    })
}

fn basis_points(id: &str, ratio: f64) -> Result<u16, BattleError> {
    let value = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&value) {
        return Err(BattleError::InvalidInput(format!("{FORM}: {id} ratio")));
    }
    Ok(value as u16)
}

fn count(id: &str, value: f64) -> Result<u8, BattleError> {
    if value.fract() != 0.0 || !(1.0..=f64::from(u8::MAX)).contains(&value) {
        return Err(BattleError::InvalidInput(format!("{FORM}: {id} count")));
    }
    Ok(value as u8)
}

#[derive(Clone, Copy)]
struct Burden {
    target: usize,
    qualifying_actions: u8,
    refunds: u8,
}

#[derive(Clone, Default)]
pub(in crate::battle) struct HanyaState {
    burden: Option<Burden>,
    action_targets_burden: bool,
    advance_turn: Option<u32>,
}

struct Hanya {
    burden_refund_limit: u8,
    ultimate_attack_bp: u32,
    ultimate_turns: u32,
    ultimate_speed_ratio_micros: u64,
    talent_damage_bp: u32,
    talent_turns: u32,
    scrivener: bool,
    netherworld: bool,
    reanimated: bool,
    eidolon: u8,
}

fn targets_burden(
    battle: &Battle<'_>,
    burden: usize,
    target: Option<usize>,
    ability: Ability<'_>,
) -> bool {
    match ability.target {
        Target::Enemy => {
            target == Some(burden)
                || ability.effects.iter().any(|effect| match effect {
                    Effect::RandomHits { .. } | Effect::DirectedAreaAttack => true,
                    Effect::BlastAdjacent { .. } | Effect::DirectedAttack { adjacent: true } => {
                        target.is_some_and(|unit| {
                            battle
                                .adjacent_enemies(unit)
                                .any(|neighbor| neighbor == burden)
                        })
                    }
                    _ => false,
                })
        }
        Target::AllEnemies => true,
        Target::SelfOnly | Target::Ally | Target::OtherAlly | Target::AllAllies => false,
    }
}

impl Mechanics for Hanya {
    fn before_ally_action(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        target: Option<usize>,
        ability: Ability<'_>,
    ) {
        let qualifies = battle.units[owner]
            .hanya_state
            .burden
            .is_some_and(|mark| targets_burden(battle, mark.target, target, ability));
        battle.units[owner].hanya_state.action_targets_burden = qualifies;
        if qualifies {
            battle.apply_ally_buff(
                owner,
                actor,
                AllyBuffKind::Damage,
                self.talent_damage_bp,
                self.talent_turns,
            );
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
        match action {
            Action::Basic => {}
            Action::Skill => {
                if battle.hp(target).0 > 0 {
                    let state = &mut battle.units[actor].hanya_state;
                    if state.burden.is_none_or(|mark| mark.target != target) {
                        // The applying Skill supplies the first action after damage is dealt.
                        state.burden = Some(Burden {
                            target,
                            qualifying_actions: 1,
                            refunds: 0,
                        });
                        state.action_targets_burden = false;
                    }
                }
                if self.eidolon >= 2 {
                    battle.apply_ally_buff(actor, actor, AllyBuffKind::SpeedPercent, 2_000, 1);
                }
            }
            Action::Ultimate | Action::EnhancedUltimate => {
                let speed = battle.speed_micros(actor) * self.ultimate_speed_ratio_micros
                    / SPEED_RATIO_SCALE;
                let speed = u32::try_from(speed).unwrap_or(u32::MAX);
                battle.apply_ally_buff(
                    actor,
                    target,
                    AllyBuffKind::SpeedFlat,
                    speed,
                    self.ultimate_turns,
                );
                battle.apply_ally_buff_with_id(
                    actor,
                    target,
                    AllyBuffKind::AttackPercent,
                    self.ultimate_attack_bp,
                    self.ultimate_turns,
                    Some(ULTIMATE),
                );
            }
        }
    }

    fn on_attack_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        action: Action,
    ) {
        // Complete the attack's refund before the queued follow-ups.
        self.on_ally_action_finished(battle, owner, actor, action);
    }

    fn on_ally_action_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        _action: Action,
    ) {
        let state = &mut battle.units[owner].hanya_state;
        if !std::mem::take(&mut state.action_targets_burden) {
            return;
        }
        let Some(mark) = &mut state.burden else {
            return;
        };
        mark.qualifying_actions += 1;
        if mark.qualifying_actions < ACTIONS_PER_REFUND {
            return;
        }
        mark.qualifying_actions = 0;
        mark.refunds += 1;
        if mark.refunds >= self.burden_refund_limit {
            state.burden = None;
        }
        battle.restore_skill_points(owner, 1);
        if self.scrivener {
            battle.apply_ally_buff(owner, actor, AllyBuffKind::AttackPercent, 1_000, 1);
        }
        if self.reanimated {
            battle.gain_energy(owner, owner, 2_000);
        }
    }

    fn on_enemy_defeat(&self, battle: &mut Battle<'_>, owner: usize, killer: usize, target: usize) {
        let state = &mut battle.units[owner].hanya_state;
        if let Some(mark) = state.burden.filter(|mark| mark.target == target) {
            state.burden = None;
            if self.netherworld && mark.refunds <= 1 {
                battle.restore_skill_points(owner, 1);
            }
        }
        if self.eidolon >= 1
            && battle.units[owner].hanya_state.advance_turn != Some(battle.normal_turns)
            && battle.has_ally_buff(owner, killer, AllyBuffKind::AttackPercent, ULTIMATE)
        {
            battle.units[owner].hanya_state.advance_turn = Some(battle.normal_turns);
            battle.advance(owner, 1_500);
        }
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) -> Option<u8> {
        match action {
            Action::Basic => None,
            Action::Skill => Some(if battle.units[actor].hanya_state.burden.is_none() {
                2
            } else {
                4
            }),
            Action::Ultimate | Action::EnhancedUltimate => {
                let preferred = battle
                    .allies()
                    .filter(|&unit| unit != actor && battle.is_targetable(unit))
                    .max_by_key(|&unit| (battle.plan.units[unit].stats.attack, Reverse(unit)))
                    .unwrap_or(actor);
                Some(if target == Some(preferred) { 0 } else { 4 })
            }
        }
    }
}
