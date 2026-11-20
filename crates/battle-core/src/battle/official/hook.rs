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

const FORM: &str = "character.1109";
const BASIC: &str = "skill.110901";
const SKILL: &str = "skill.110902";
const ULTIMATE: &str = "skill.110903";
const TALENT: &str = "skill.110904";
const ENHANCED_SKILL: &str = "skill.110909";
const INNOCENCE: &str = "trace.1109101";
const NAIVETE: &str = "trace.1109102";
const PLAYING_WITH_FIRE: &str = "trace.1109103";

struct Burn {
    chance_bp: u32,
    turns: u32,
    multiplier_bp: u16,
}

impl Burn {
    fn from_skill(id: &str, parameters: &[f64], eidolon: u8) -> Result<Self, BattleError> {
        let chance = parameters[1];
        let turns = parameters[2];
        if chance > 1.0 || !(1.0..=f64::from(u8::MAX)).contains(&turns) || turns.fract() != 0.0 {
            return Err(invalid_parameter(id));
        }
        Ok(Self {
            chance_bp: (chance * 10_000.0).round() as u32,
            turns: turns as u32 + u32::from(eidolon >= 2),
            multiplier_bp: basis_points(id, parameters[3])?,
        })
    }

    fn apply(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        origin: Action,
        chance_bp: u32,
    ) {
        battle.apply_skill_dot(
            actor,
            target,
            chance_bp,
            SkillDot {
                element: Element::Fire,
                damage: DotScale::Attack(self.multiplier_bp),
                origin,
                turns: self.turns,
                max_stacks: 1,
            },
        );
    }
}

struct Hook {
    enhanced: OfficialAbility,
    skill_burn: Burn,
    enhanced_burn: Burn,
    talent_multiplier_bp: u16,
    talent_energy_milli: u32,
    eidolon: u8,
    innocence: bool,
    naivete: bool,
    playing_with_fire: bool,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 120 || input.eidolon > 6 {
        return Err(BattleError::InvalidInput(format!(
            "{FORM}: energy capacity or eidolon"
        )));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 4)?;
    let ultimate = input.parameters(ULTIMATE, 1)?;
    let talent = input.parameters(TALENT, 2)?;
    let enhanced = input.parameters(ENHANCED_SKILL, 5)?;
    if !(0.0..=f64::from(input.energy_capacity)).contains(&talent[1]) {
        return Err(invalid_parameter(TALENT));
    }
    let trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let hook = Hook {
        enhanced: attack(
            vec![
                fire_hit(basis_points(ENHANCED_SKILL, enhanced[0])?, 20),
                Effect::BlastAdjacent {
                    element: Element::Fire,
                    multiplier_bp: basis_points(ENHANCED_SKILL, enhanced[4])?,
                    toughness: 10,
                },
            ],
            30_000,
            SkillPoints::Spend,
        ),
        skill_burn: Burn::from_skill(SKILL, skill, input.eidolon)?,
        enhanced_burn: Burn::from_skill(ENHANCED_SKILL, enhanced, input.eidolon)?,
        talent_multiplier_bp: basis_points(TALENT, talent[0])?,
        talent_energy_milli: (talent[1] * 1_000.0).round() as u32,
        eidolon: input.eidolon,
        innocence: trace(INNOCENCE),
        naivete: trace(NAIVETE),
        playing_with_fire: trace(PLAYING_WITH_FIRE),
    };
    Ok(OfficialKit {
        max_energy_milli: 120_000,
        basic: attack(
            vec![fire_hit(basis_points(BASIC, basic[0])?, 10)],
            20_000,
            SkillPoints::Generate,
        ),
        skill: attack(
            vec![fire_hit(basis_points(SKILL, skill[0])?, 20)],
            30_000,
            SkillPoints::Spend,
        ),
        ultimate: attack(
            vec![fire_hit(basis_points(ULTIMATE, ultimate[0])?, 30)],
            5_000,
            SkillPoints::Unchanged,
        ),
        mechanics: Arc::new(hook),
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

fn attack(effects: Vec<Effect>, energy_milli: u32, skill_points: SkillPoints) -> OfficialAbility {
    OfficialAbility {
        target: Target::Enemy,
        energy_milli,
        effects,
        skill_points,
    }
}

fn fire_hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: Element::Fire,
        multiplier_bp,
        toughness,
    }
}

fn is_enhanced(battle: &Battle<'_>, actor: usize, action: Action) -> bool {
    action == Action::Skill && battle.units[actor].charged_skill
}

impl Hook {
    fn trigger_talent(&self, battle: &mut Battle<'_>, actor: usize, target: usize, action: Action) {
        battle.secondary_hit(
            actor,
            target,
            Element::Fire,
            self.talent_multiplier_bp,
            DamageKind::Additional,
            action,
        );
        battle.gain_energy(actor, actor, self.talent_energy_milli);
        if self.innocence {
            let (_, max_hp) = battle.hp(actor);
            battle.heal_unit(actor, actor, max_hp / 20);
        }
        if self.eidolon >= 4 {
            for neighbor in battle.adjacent_enemies(target) {
                self.skill_burn
                    .apply(battle, actor, neighbor, action, 10_000);
            }
        }
    }
}

impl Mechanics for Hook {
    fn ability_override(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
    ) -> Option<&OfficialAbility> {
        is_enhanced(battle, actor, action).then_some(&self.enhanced)
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        let Some(target) = target else { return };
        // Snapshot eligibility before damage, weakness break, and E4 can add new Burns.
        let burning = std::iter::once(target)
            .chain(
                battle
                    .adjacent_enemies(target)
                    .filter(|_| is_enhanced(battle, actor, action)),
            )
            .filter(|&enemy| battle.has_elemental_dot(enemy, Element::Fire))
            .collect();
        battle.units[actor].burning_targets_at_action_start = burning;
    }

    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        if action == Action::Skill
            && let Some(target) = target
        {
            let burn = if std::mem::take(&mut battle.units[actor].charged_skill) {
                &self.enhanced_burn
            } else {
                &self.skill_burn
            };
            burn.apply(battle, actor, target, action, burn.chance_bp);
        }

        let burning = std::mem::take(&mut battle.units[actor].burning_targets_at_action_start);
        for target in burning {
            self.trigger_talent(battle, actor, target, action);
        }

        if action == Action::Ultimate {
            battle.units[actor].charged_skill = true;
            if self.playing_with_fire {
                battle.advance(actor, 2_000);
                battle.gain_energy(actor, actor, 5_000);
            }
        }
    }

    fn control_resistance_bp(&self) -> u32 {
        if self.naivete { 3_500 } else { 0 }
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        target: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        let e1 = self.eidolon >= 1
            && kind == DamageKind::Direct
            && action.is_some_and(|action| is_enhanced(battle, actor, action));
        let e6 = self.eidolon >= 6 && battle.has_elemental_dot(target, Element::Fire);
        2_000 * (u32::from(e1) + u32::from(e6))
    }
}
