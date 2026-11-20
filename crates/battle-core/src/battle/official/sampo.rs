use std::sync::Arc;

use crate::{
    Action, BattleError, DamageKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{
        Battle,
        periodic::{DotScale, SkillDot},
    },
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

const BASIC: &str = "skill.110801";
const SKILL: &str = "skill.110802";
const ULTIMATE: &str = "skill.110803";
const TALENT: &str = "skill.110804";
const TRAP: &str = "trace.1108101";
const DEFENSIVE_POSITION: &str = "trace.1108102";
const SPICE_UP: &str = "trace.1108103";

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 120 || input.eidolon > 6 {
        return Err(BattleError::InvalidInput(
            "Sampo energy capacity or eidolon".into(),
        ));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 2)?;
    let ultimate = input.parameters(ULTIMATE, 4)?;
    let talent = input.parameters(TALENT, 4)?;
    let traced = |id: &str| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let extra_bounces = count(SKILL, skill[0], 4)? + u8::from(input.eidolon >= 1);
    let skill_bp = basis_points(SKILL, skill[1])?;
    let talent_bp = basis_points(TALENT, talent[1])?;
    let talent_bp = talent_bp
        .checked_add(if input.eidolon >= 6 { 1_500 } else { 0 })
        .ok_or_else(|| BattleError::InvalidInput("Sampo Wind Shear multiplier".into()))?;
    Ok(OfficialKit {
        max_energy_milli: input.energy_capacity * 1_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: split_hits(basis_points(BASIC, basic[0])?, [30, 30, 40], [3, 3, 4]),
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 0,
            effects: vec![
                hit(skill_bp, 10),
                Effect::RandomHits {
                    element: Element::Wind,
                    multiplier_bp: skill_bp,
                    toughness: 5,
                    count: extra_bounces,
                },
            ],
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::AllEnemies,
            energy_milli: if traced(DEFENSIVE_POSITION) {
                15_000
            } else {
                5_000
            },
            effects: split_hits(basis_points(ULTIMATE, ultimate[0])?, [25; 4], [5; 4]),
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Sampo {
            eidolon: input.eidolon,
            wind_shear_chance_bp: chance(TALENT, talent[0])?,
            wind_shear_bp: talent_bp,
            wind_shear_turns: u32::from(count(TALENT, talent[2], 3)?) + u32::from(traced(TRAP)),
            wind_shear_limit: count(TALENT, talent[3], 5)?,
            dot_vulnerability_chance_bp: chance(ULTIMATE, ultimate[3])?,
            dot_vulnerability_bp: u32::from(basis_points(ULTIMATE, ultimate[1])?),
            dot_vulnerability_turns: u32::from(count(ULTIMATE, ultimate[2], 2)?),
            spice_up: traced(SPICE_UP),
        }),
    })
}

fn split_hits<const N: usize>(
    multiplier_bp: u16,
    percentages: [u16; N],
    toughness: [u16; N],
) -> Vec<Effect> {
    let mut allocated = 0;
    percentages
        .into_iter()
        .zip(toughness)
        .enumerate()
        .map(|(index, (percent, toughness))| {
            let segment = if index + 1 == N {
                multiplier_bp - allocated
            } else {
                (u32::from(multiplier_bp) * u32::from(percent) / 100) as u16
            };
            allocated += segment;
            hit(segment, toughness)
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

fn basis_points(skill: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if (1.0..=f64::from(u16::MAX)).contains(&bp) {
        Ok(bp as u16)
    } else {
        Err(BattleError::InvalidInput(format!(
            "Sampo {skill} multiplier"
        )))
    }
}

fn chance(skill: &str, ratio: f64) -> Result<u32, BattleError> {
    if ratio <= 1.0 {
        Ok(u32::from(basis_points(skill, ratio)?))
    } else {
        Err(BattleError::InvalidInput(format!(
            "Sampo {skill} base chance"
        )))
    }
}

fn count(skill: &str, value: f64, expected: u8) -> Result<u8, BattleError> {
    if value == f64::from(expected) {
        Ok(expected)
    } else {
        Err(BattleError::InvalidInput(format!(
            "Sampo {skill} count or duration"
        )))
    }
}

struct Sampo {
    eidolon: u8,
    wind_shear_chance_bp: u32,
    wind_shear_bp: u16,
    wind_shear_turns: u32,
    wind_shear_limit: u8,
    dot_vulnerability_chance_bp: u32,
    dot_vulnerability_bp: u32,
    dot_vulnerability_turns: u32,
    spice_up: bool,
}

impl Sampo {
    fn wind_shear(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        action: Action,
        chance_bp: u32,
    ) {
        battle.apply_skill_dot(
            actor,
            target,
            chance_bp,
            SkillDot {
                element: Element::Wind,
                damage: DotScale::Attack(self.wind_shear_bp),
                origin: action,
                turns: self.wind_shear_turns,
                max_stacks: self.wind_shear_limit,
            },
        );
    }
}

impl Mechanics for Sampo {
    fn after_attack_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        action: Action,
        _hp_before: u32,
    ) {
        if action == Action::Skill {
            battle.gain_energy(actor, actor, 6_000);
        }
        if battle.hp(target).0 == 0 {
            return;
        }
        // Check the stack threshold before this hit's Talent applies its next stack.
        if action == Action::Skill
            && self.eidolon >= 4
            && battle.dot_stacks(target, Element::Wind) >= 5
        {
            battle.detonate_dots(actor, target, Element::Wind, 800, action);
        }
        if battle.hp(target).0 > 0 {
            self.wind_shear(battle, actor, target, action, self.wind_shear_chance_bp);
        }
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        _target: Option<usize>,
    ) {
        if action != Action::Ultimate {
            return;
        }
        for target in battle.enemies() {
            if battle.hp(target).0 > 0
                && battle.roll_effect(actor, target, self.dot_vulnerability_chance_bp)
            {
                battle.apply_dot_vulnerability(
                    actor,
                    target,
                    self.dot_vulnerability_bp,
                    self.dot_vulnerability_turns,
                );
            }
        }
    }

    fn on_enemy_defeat(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _killer: usize,
        target: usize,
    ) {
        if self.eidolon < 2 || !battle.has_elemental_dot(target, Element::Wind) {
            return;
        }
        let action = battle.current_action().unwrap_or(Action::Skill);
        for recipient in battle.enemies() {
            if battle.hp(recipient).0 > 0 {
                self.wind_shear(battle, owner, recipient, action, 10_000);
            }
        }
    }

    fn incoming_damage_reduction_bp(
        &self,
        battle: &Battle<'_>,
        _owner: usize,
        source: usize,
        _kind: DamageKind,
    ) -> u32 {
        if self.spice_up && battle.has_elemental_dot(source, Element::Wind) {
            1_500
        } else {
            0
        }
    }
}
