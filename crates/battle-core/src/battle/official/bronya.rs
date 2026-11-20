use std::{cmp::Reverse, sync::Arc};

use crate::{
    Action, BattleError, DamageKind,
    fixture::{Effect, Element, SKILL_POINT_CAP, Target},
};

use super::super::{Battle, resolve::FollowUpAttack, support::AllyBuffKind};
use super::{Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints};

#[cfg(test)]
mod tests;

const BASIC: &str = "skill.110101";
const SKILL: &str = "skill.110102";
const ULTIMATE: &str = "skill.110103";
const TALENT: &str = "skill.110104";
const COMMAND: &str = "trace.1101101";
const BATTLEFIELD: &str = "trace.1101102";
const MILITARY_MIGHT: &str = "trace.1101103";

struct Bronya {
    eidolon: u8,
    skill_damage_bp: u32,
    skill_turns: u32,
    ultimate_attack_bp: u32,
    ultimate_crit_ratio_bp: u16,
    ultimate_crit_flat_bp: u32,
    talent_advance_bp: u16,
    follow_up_damage_bp: u16,
    guaranteed_basic_crit: bool,
    opening_defense: bool,
    damage_aura: bool,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 4)?;
    let ultimate = input.parameters(ULTIMATE, 4)?;
    let talent = input.parameters(TALENT, 1)?;
    if input.energy_capacity != 120 || input.eidolon > 6 || skill[2] != 1.0 || ultimate[3] != 2.0 {
        return Err(BattleError::InvalidInput(
            "character.1101: energy capacity, eidolon or buff duration".into(),
        ));
    }
    let basic_multiplier_bp = basis_points(BASIC, basic[0])?;
    let enabled = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    Ok(OfficialKit {
        max_energy_milli: 120_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![Effect::Hit {
                element: Element::Wind,
                multiplier_bp: basic_multiplier_bp,
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
        mechanics: Arc::new(Bronya {
            eidolon: input.eidolon,
            skill_damage_bp: u32::from(basis_points(SKILL, skill[0])?),
            skill_turns: skill[2] as u32 + u32::from(input.eidolon >= 6),
            ultimate_attack_bp: u32::from(basis_points(ULTIMATE, ultimate[0])?),
            ultimate_crit_ratio_bp: basis_points(ULTIMATE, ultimate[1])?,
            ultimate_crit_flat_bp: u32::from(basis_points(ULTIMATE, ultimate[2])?),
            talent_advance_bp: basis_points(TALENT, talent[0])?,
            follow_up_damage_bp: (u32::from(basic_multiplier_bp) * 8 / 10) as u16,
            guaranteed_basic_crit: enabled(COMMAND),
            opening_defense: enabled(BATTLEFIELD),
            damage_aura: enabled(MILITARY_MIGHT),
        }),
    })
}

fn basis_points(skill: &str, ratio: f64) -> Result<u16, BattleError> {
    let rounded = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&rounded) {
        return Err(BattleError::InvalidInput(format!(
            "character.1101: invalid ratio for {skill}"
        )));
    }
    Ok(rounded as u16)
}

impl Bronya {
    fn redeploy(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        battle.cleanse_one(actor, target);
        battle.apply_ally_buff(
            actor,
            target,
            AllyBuffKind::Damage,
            self.skill_damage_bp,
            self.skill_turns,
        );
        if actor != target {
            battle.advance_ally_immediate(actor, target);
        }
        if self.eidolon >= 2 {
            battle.units[actor].bronya_state.pending_speed_target = Some(target);
        }

        let turn = battle.units[actor].normal_turns;
        if self.eidolon >= 1
            && turn >= battle.units[actor].bronya_state.skill_point_ready_at
            && battle.roll_fixed(5_000)
        {
            battle.set_skill_points(actor, (battle.skill_points + 1).min(SKILL_POINT_CAP));
            battle.units[actor].bronya_state.skill_point_ready_at = turn + 2;
        }
    }

    fn rally(&self, battle: &mut Battle<'_>, actor: usize) {
        let source_crit_bp = u64::from(battle.effective_crit_damage_bp(actor));
        let scaled_bp = (source_crit_bp * u64::from(self.ultimate_crit_ratio_bp) / 10_000) as u32;
        let crit_bp = self.ultimate_crit_flat_bp.saturating_add(scaled_bp);
        for target in battle.allies() {
            if battle.hp(target).0 > 0 {
                battle.apply_ally_buff(actor, target, AllyBuffKind::CritDamage, crit_bp, 2);
                battle.apply_ally_buff(
                    actor,
                    target,
                    AllyBuffKind::AttackPercent,
                    self.ultimate_attack_bp,
                    2,
                );
            }
        }
    }

    fn activate_pending_speed(battle: &mut Battle<'_>, owner: usize, actor: usize) {
        let pending = &mut battle.units[owner].bronya_state.pending_speed_target;
        if *pending == Some(actor) {
            *pending = None;
            battle.apply_ally_buff(owner, actor, AllyBuffKind::SpeedPercent, 3_000, 1);
        }
    }
}

impl Mechanics for Bronya {
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.opening_defense {
            for target in battle.allies() {
                battle.apply_ally_buff(actor, target, AllyBuffKind::DefensePercent, 2_000, 2);
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
            (Action::Basic, _) => battle.advance(actor, self.talent_advance_bp),
            (Action::Skill, Some(target)) => self.redeploy(battle, actor, target),
            (Action::Ultimate, _) => self.rally(battle, actor),
            (Action::Skill, None) => {}
        }
    }

    fn after_ordinary_turn(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        Self::activate_pending_speed(battle, owner, actor);
    }

    fn on_ally_action_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        actor: usize,
        action: Action,
    ) {
        if action == Action::Ultimate && !battle.in_follow_up() {
            Self::activate_pending_speed(battle, owner, actor);
        }
    }

    fn on_attack_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        origin: Action,
    ) {
        let turn = battle.units[owner].normal_turns;
        if self.eidolon < 4
            || owner == source
            || origin != Action::Basic
            || battle.in_follow_up()
            || turn < battle.units[owner].bronya_state.follow_up_ready_at
        {
            return;
        }
        if let Some(target) = battle.current_target()
            && battle.hp(target).0 > 0
            && battle.has_weakness(target, Element::Wind)
        {
            battle.units[owner].bronya_state.follow_up_ready_at = turn + 1;
            battle.follow_up_single(
                owner,
                target,
                origin,
                FollowUpAttack {
                    element: Element::Wind,
                    multiplier_bp: self.follow_up_damage_bp,
                    toughness: 10,
                    energy_milli: 5_000,
                    first_hit: None,
                },
            );
        }
    }

    fn crit_rate_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        _target: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        if self.guaranteed_basic_crit && action == Some(Action::Basic) && kind == DamageKind::Direct
        {
            10_000
        } else {
            0
        }
    }

    fn ally_damage_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        _target: usize,
        _element: Element,
    ) -> u32 {
        if self.damage_aura { 1_000 } else { 0 }
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
            Action::Ultimate => Some(0),
            Action::Skill => {
                let preferred = battle
                    .allies()
                    .filter(|&unit| unit != actor && battle.is_targetable(unit))
                    .max_by_key(|&unit| (battle.plan.units[unit].stats.attack, Reverse(unit)));
                Some(if target.is_some() && target == preferred {
                    2
                } else {
                    4
                })
            }
        }
    }
}
