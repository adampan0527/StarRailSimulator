use std::sync::Arc;

use crate::{
    Action, BattleError, DamageKind,
    fixture::{Effect, Element, Target},
};

use super::super::Battle;
use super::{Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints};

#[cfg(test)]
mod tests;

const BASIC: &str = "skill.101301";
const SKILL: &str = "skill.101302";
const ULTIMATE: &str = "skill.101303";
const TALENT: &str = "skill.101304";
const EFFICIENCY: &str = "trace.1013101";
const ICING: &str = "trace.1013103";

struct Herta {
    eidolon: u8,
    high_hp_skill_bonus_bp: u32,
    frozen_ultimate_bonus_bp: u32,
    follow_up_multiplier_bp: u16,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.eidolon > 6 || input.energy_capacity != 110 {
        return Err(BattleError::UnsupportedData(
            "character.1013 requires a valid eidolon and the 110-energy kit".into(),
        ));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 3)?;
    let ultimate = input.parameters(ULTIMATE, 1)?;
    let talent = input.parameters(TALENT, 2)?;
    if skill[1] != 0.5 || talent[0] != 0.5 {
        return Err(BattleError::UnsupportedData(
            "character.1013 requires the 50% HP thresholds for its skill and talent".into(),
        ));
    }
    let trace_enabled = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let efficiency_bp = if trace_enabled(EFFICIENCY) { 2_500 } else { 0 };
    let icing_bp = if trace_enabled(ICING) { 2_000 } else { 0 };
    let skill_multiplier_bp = to_basis_points(SKILL, skill[0])?;
    let first_hit_bp = ((u32::from(skill_multiplier_bp) * 3 + 5) / 10) as u16;

    Ok(OfficialKit {
        max_energy_milli: input.energy_capacity * 1_000,
        basic: attack(
            Target::Enemy,
            to_basis_points(BASIC, basic[0])?,
            10,
            20_000,
            SkillPoints::Generate,
        ),
        skill: OfficialAbility {
            target: Target::AllEnemies,
            energy_milli: 30_000,
            // Evaluate the target HP bonus again between the 30% and 70% hits.
            effects: vec![
                hit(first_hit_bp, 3),
                hit(skill_multiplier_bp - first_hit_bp, 7),
            ],
            skill_points: SkillPoints::Spend,
        },
        ultimate: attack(
            Target::AllEnemies,
            to_basis_points(ULTIMATE, ultimate[0])?,
            20,
            5_000,
            SkillPoints::Unchanged,
        ),
        mechanics: Arc::new(Herta {
            eidolon: input.eidolon,
            high_hp_skill_bonus_bp: u32::from(to_basis_points(SKILL, skill[2])?) + efficiency_bp,
            frozen_ultimate_bonus_bp: icing_bp,
            follow_up_multiplier_bp: to_basis_points(TALENT, talent[1])?,
        }),
    })
}

fn to_basis_points(skill_id: &str, ratio: f64) -> Result<u16, BattleError> {
    let rounded = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&rounded) {
        return Err(BattleError::UnsupportedData(format!(
            "character.1013 has an invalid damage ratio for {skill_id}"
        )));
    }
    Ok(rounded as u16)
}

fn attack(
    target: Target,
    multiplier_bp: u16,
    toughness: u16,
    energy_milli: u32,
    skill_points: SkillPoints,
) -> OfficialAbility {
    OfficialAbility {
        target,
        energy_milli,
        effects: vec![hit(multiplier_bp, toughness)],
        skill_points,
    }
}

fn hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: Element::Ice,
        multiplier_bp,
        toughness,
    }
}

impl Mechanics for Herta {
    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        _actor: usize,
        target: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        match (action, kind) {
            (_, DamageKind::FollowUp) if self.eidolon >= 4 => 1_000,
            (Some(Action::Skill), DamageKind::Direct) => {
                let (hp, max_hp) = battle.hp(target);
                if u64::from(hp) * 2 >= u64::from(max_hp) {
                    self.high_hp_skill_bonus_bp
                } else {
                    0
                }
            }
            (Some(Action::Ultimate), DamageKind::Direct) if battle.is_frozen(target) => {
                self.frozen_ultimate_bonus_bp
            }
            _ => 0,
        }
    }

    fn after_attack_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        action: Action,
        hp_before: u32,
    ) {
        let (hp_after, max_hp) = battle.hp(target);
        if self.eidolon >= 1
            && action == Action::Basic
            && hp_after > 0
            && u64::from(hp_before) * 2 <= u64::from(max_hp)
        {
            battle.secondary_hit(
                actor,
                target,
                Element::Ice,
                4_000,
                DamageKind::Additional,
                action,
            );
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _: Option<usize>,
    ) {
        if self.eidolon >= 6 && action == Action::Ultimate {
            battle.apply_attack_bonus(actor, 2_500, 1);
        }
    }

    fn after_attack(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _source: usize,
        crossed_half: &[usize],
        origin: Action,
    ) {
        if self.eidolon >= 2 {
            for _ in crossed_half {
                battle.add_crit_rate_stacks(owner, 300, 5);
            }
        }
        for _ in crossed_half {
            battle.follow_up(
                owner,
                Element::Ice,
                self.follow_up_multiplier_bp,
                5,
                origin,
                5_000,
            );
        }
    }
}
