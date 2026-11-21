use std::sync::Arc;

use crate::{
    Action, BattleError, DamageKind,
    fixture::{Effect, Element, Target},
};

use super::super::Battle;
use super::{Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints};

#[cfg(test)]
mod tests;

const BASIC: &str = "skill.100201";
const SKILL: &str = "skill.100202";
const ULTIMATE: &str = "skill.100203";
const TALENT: &str = "skill.100204";
const FASTER_THAN_LIGHT: &str = "trace.1002102";
const HIGH_GALE: &str = "trace.1002103";

struct DanHeng {
    eidolon: u8,
    slow_chance_bp: u32,
    slow_magnitude_bp: u32,
    slow_turns: u32,
    slowed_ultimate_bonus_bp: u16,
    talent_penetration_bp: u32,
    talent_cooldown_turns: u32,
    speed_up_trace: bool,
    bonus_basic_damage_trace: bool,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 100 || input.eidolon > 6 {
        return Err(BattleError::InvalidInput(
            "dan_heng.hunt: energy capacity or eidolon".into(),
        ));
    }

    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 4)?;
    let ultimate = input.parameters(ULTIMATE, 2)?;
    let talent = input.parameters(TALENT, 2)?;
    let ultimate_multiplier_bp = basis_points(ULTIMATE, ultimate[0])?;
    let slowed_ultimate_bonus_bp = basis_points(ULTIMATE, ultimate[1])?;
    if ultimate_multiplier_bp
        .checked_add(slowed_ultimate_bonus_bp)
        .is_none()
        || skill[1] <= 0.0
        || skill[1] + 0.08 >= 1.0
        || skill[2] != 2.0
        || skill[3] > 1.0
        || talent[0] > 1.0
        || talent[1] != 2.0
    {
        return Err(BattleError::InvalidInput(
            "dan_heng.hunt: conditional damage or status parameters".into(),
        ));
    }

    let enabled_trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let eidolon_slow_bp = if input.eidolon >= 6 { 800 } else { 0 };
    Ok(OfficialKit {
        max_energy_milli: 100_000,
        basic: attack(
            split_hits(basis_points(BASIC, basic[0])?, [45, 55], [5, 5]),
            20_000,
            SkillPoints::Generate,
        ),
        skill: attack(
            split_hits(
                basis_points(SKILL, skill[0])?,
                [30, 15, 15, 40],
                [6, 3, 3, 8],
            ),
            30_000,
            SkillPoints::Spend,
        ),
        ultimate: attack(
            vec![hit(ultimate_multiplier_bp, 30)],
            5_000,
            SkillPoints::Unchanged,
        ),
        mechanics: Arc::new(DanHeng {
            eidolon: input.eidolon,
            slow_chance_bp: u32::from(basis_points(SKILL, skill[3])?),
            slow_magnitude_bp: u32::from(basis_points(SKILL, skill[1])?) + eidolon_slow_bp,
            slow_turns: skill[2] as u32,
            slowed_ultimate_bonus_bp,
            talent_penetration_bp: u32::from(basis_points(TALENT, talent[0])?),
            talent_cooldown_turns: talent[1] as u32 - u32::from(input.eidolon >= 2),
            speed_up_trace: enabled_trace(FASTER_THAN_LIGHT),
            bonus_basic_damage_trace: enabled_trace(HIGH_GALE),
        }),
    })
}

fn basis_points(skill: &str, ratio: f64) -> Result<u16, BattleError> {
    let rounded = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&rounded) {
        return Err(BattleError::InvalidInput(format!(
            "dan_heng.hunt: invalid ratio for {skill}"
        )));
    }
    Ok(rounded as u16)
}

fn attack(effects: Vec<Effect>, energy_milli: u32, skill_points: SkillPoints) -> OfficialAbility {
    OfficialAbility {
        target: Target::Enemy,
        energy_milli,
        effects,
        skill_points,
    }
}

fn split_hits<const N: usize>(
    multiplier_bp: u16,
    shares: [u16; N],
    toughness: [u16; N],
) -> Vec<Effect> {
    // Round cumulative damage shares so the last segment preserves the total.
    let mut points_allocated = 0;
    let mut percent_allocated = 0;
    shares
        .into_iter()
        .zip(toughness)
        .map(|(share, toughness)| {
            percent_allocated += u32::from(share);
            let cumulative = ((u32::from(multiplier_bp) * percent_allocated + 50) / 100) as u16;
            let effect = hit(cumulative - points_allocated, toughness);
            points_allocated = cumulative;
            effect
        })
        .collect()
}

fn hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: Element::Wind,
        multiplier_bp,
        toughness,
    }
}

impl Mechanics for DanHeng {
    fn crit_rate_bonus_bp(
        &self,
        battle: &Battle<'_>,
        _actor: usize,
        target: usize,
        _action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        let (hp, max_hp) = battle.hp(target);
        if self.eidolon >= 1 && kind == DamageKind::Direct && u64::from(hp) * 2 >= u64::from(max_hp)
        {
            1_200
        } else {
            0
        }
    }

    fn after_critical_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        action: Action,
    ) {
        if action == Action::Skill
            && battle.hp(target).0 > 0
            && battle.roll_effect(actor, target, self.slow_chance_bp)
        {
            battle.apply_speed_down(actor, target, self.slow_magnitude_bp, self.slow_turns);
        }
    }

    fn damage_multiplier_bp(
        &self,
        battle: &Battle<'_>,
        _actor: usize,
        target: usize,
        action: Option<Action>,
        kind: DamageKind,
        base: u32,
    ) -> u32 {
        if action == Some(Action::Ultimate)
            && kind == DamageKind::Direct
            && battle.is_slowed(target)
        {
            base.saturating_add(u32::from(self.slowed_ultimate_bonus_bp))
                .min(u32::from(u16::MAX))
        } else {
            base
        }
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        _actor: usize,
        target: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        if self.bonus_basic_damage_trace
            && action == Some(Action::Basic)
            && kind == DamageKind::Direct
            && battle.is_slowed(target)
        {
            4_000
        } else {
            0
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        _action: Action,
        _target: Option<usize>,
    ) {
        if self.speed_up_trace && battle.roll_fixed(5_000) {
            battle.apply_speed_up(actor, actor, 2_000, 2);
        }
    }

    fn on_ally_targeted(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _source: usize,
        _action: Action,
    ) {
        battle.arm_resistance_penetration(
            owner,
            self.talent_penetration_bp,
            self.talent_cooldown_turns,
        );
    }

    fn after_defeat(&self, battle: &mut Battle<'_>, actor: usize, _target: usize) {
        if self.eidolon >= 4 && battle.current_action() == Some(Action::Ultimate) {
            battle.advance_immediate(actor);
        }
    }
}
