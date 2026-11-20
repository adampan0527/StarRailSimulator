use std::sync::Arc;

use crate::{
    Action, BattleError, DamageKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{
        Battle,
        periodic::{DotEvent, DotScale, SkillDot},
    },
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

const FORM: &str = "character.1210";
const BASIC: &str = "skill.121001";
const SKILL: &str = "skill.121002";
const ULTIMATE: &str = "skill.121003";
const TALENT: &str = "skill.121004";
const HIGH_POLES: &str = "trace.1210101";
const BLADED_HOOP: &str = "trace.1210102";
const WALKING_ON_KNIVES: &str = "trace.1210103";

struct Guinaifen {
    eidolon: u8,
    high_poles: bool,
    bladed_hoop: bool,
    walking_on_knives: bool,
    burn_chance_bp: u32,
    burn_bp: u16,
    burn_on_burning_bp: u16,
    burn_turns: u32,
    detonation_bp: u16,
    firekiss_chance_bp: u32,
    firekiss_bp: u32,
    firekiss_limit_bp: u32,
    firekiss_turns: u32,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 120 || input.eidolon > 6 {
        return Err(BattleError::InvalidInput(format!(
            "{FORM}: energy capacity or eidolon"
        )));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 5)?;
    let ultimate = input.parameters(ULTIMATE, 2)?;
    let talent = input.parameters(TALENT, 6)?;
    let has_trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let burn_bp = basis_points(SKILL, skill[3])?;
    let burn_on_burning_bp = burn_bp
        .checked_add(if input.eidolon >= 2 { 4_000 } else { 0 })
        .ok_or_else(|| invalid_parameter(SKILL))?;
    let firekiss_bp = u32::from(basis_points(TALENT, talent[3])?);
    let stack_limit = count(TALENT, talent[5], 3)? + u32::from(input.eidolon >= 6);
    let mechanics = Guinaifen {
        eidolon: input.eidolon,
        high_poles: has_trace(HIGH_POLES),
        bladed_hoop: has_trace(BLADED_HOOP),
        walking_on_knives: has_trace(WALKING_ON_KNIVES),
        burn_chance_bp: chance(SKILL, skill[2])?,
        burn_bp,
        burn_on_burning_bp,
        burn_turns: count(SKILL, skill[4], 2)?,
        detonation_bp: basis_points(ULTIMATE, ultimate[1])?,
        firekiss_chance_bp: chance(TALENT, talent[0])?,
        firekiss_bp,
        firekiss_limit_bp: firekiss_bp * stack_limit,
        firekiss_turns: count(TALENT, talent[4], 3)?,
    };
    Ok(OfficialKit {
        max_energy_milli: 120_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![fire_hit(basis_points(BASIC, basic[0])?, 10)],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 30_000,
            effects: vec![
                fire_hit(basis_points(SKILL, skill[0])?, 20),
                Effect::BlastAdjacent {
                    element: Element::Fire,
                    multiplier_bp: basis_points(SKILL, skill[1])?,
                    toughness: 10,
                },
            ],
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::AllEnemies,
            energy_milli: 5_000,
            effects: vec![fire_hit(basis_points(ULTIMATE, ultimate[0])?, 20)],
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(mechanics),
    })
}

fn invalid_parameter(id: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: out-of-range {id} parameters"))
}

fn basis_points(id: &str, ratio: f64) -> Result<u16, BattleError> {
    let points = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&points) {
        return Err(invalid_parameter(id));
    }
    Ok(points as u16)
}

fn chance(id: &str, ratio: f64) -> Result<u32, BattleError> {
    if ratio > 1.0 {
        return Err(invalid_parameter(id));
    }
    Ok(u32::from(basis_points(id, ratio)?))
}

fn count(id: &str, value: f64, expected: u32) -> Result<u32, BattleError> {
    if value != f64::from(expected) {
        return Err(invalid_parameter(id));
    }
    Ok(expected)
}

fn fire_hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: Element::Fire,
        multiplier_bp,
        toughness,
    }
}

impl Guinaifen {
    fn apply_burn(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        action: Action,
        chance_bp: u32,
    ) {
        let multiplier_bp = if battle.has_elemental_dot(target, Element::Fire) {
            self.burn_on_burning_bp
        } else {
            self.burn_bp
        };
        battle.apply_skill_dot(
            actor,
            target,
            chance_bp,
            SkillDot {
                element: Element::Fire,
                damage: DotScale::Attack(multiplier_bp),
                origin: action,
                turns: self.burn_turns,
                max_stacks: 1,
            },
        );
    }
}

impl Mechanics for Guinaifen {
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.bladed_hoop {
            battle.advance(actor, 2_500);
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
            (Action::Basic, Some(target)) if self.high_poles => {
                self.apply_burn(battle, actor, target, action, 8_000);
            }
            (Action::Skill, Some(target)) => {
                if self.eidolon >= 1
                    && battle.hp(target).0 > 0
                    && battle.roll_effect(actor, target, 10_000)
                {
                    battle.apply_effect_resistance_down(actor, target, 1_000, 2);
                }
                for enemy in std::iter::once(target).chain(battle.adjacent_enemies(target)) {
                    self.apply_burn(battle, actor, enemy, action, self.burn_chance_bp);
                }
            }
            (Action::Ultimate, _) => {
                for enemy in battle.enemies() {
                    battle.detonate_dots(actor, enemy, Element::Fire, self.detonation_bp, action);
                }
            }
            _ => {}
        }
    }

    fn on_enemy_dot_damage(&self, battle: &mut Battle<'_>, owner: usize, event: DotEvent) {
        let DotEvent {
            source,
            target,
            element,
            ..
        } = event;
        if element != Element::Fire {
            return;
        }
        if self.eidolon >= 4 && source == owner {
            battle.gain_energy(owner, owner, 2_000);
        }
        if battle.hp(target).0 > 0 && battle.roll_effect(owner, target, self.firekiss_chance_bp) {
            let amount = (battle.vulnerability_from(owner, target) + self.firekiss_bp)
                .min(self.firekiss_limit_bp);
            battle.apply_vulnerability(owner, target, amount, self.firekiss_turns);
        }
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        _actor: usize,
        target: usize,
        _action: Option<Action>,
        _kind: DamageKind,
    ) -> u32 {
        if self.walking_on_knives && battle.has_elemental_dot(target, Element::Fire) {
            2_000
        } else {
            0
        }
    }
}
