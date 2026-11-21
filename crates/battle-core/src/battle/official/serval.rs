use std::sync::Arc;

use crate::{
    Action, BattleError, DamageKind,
    fixture::{Effect, Element, Target},
};

use super::{
    super::Battle, Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

const BASIC: &str = "skill.110301";
const SKILL: &str = "skill.110302";
const ULTIMATE: &str = "skill.110303";
const TALENT: &str = "skill.110304";

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 100 || input.eidolon > 6 {
        return Err(BattleError::InvalidInput(
            "Serval energy capacity or eidolon".into(),
        ));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 5)?;
    let ultimate = input.parameters(ULTIMATE, 2)?;
    let talent = input.parameters(TALENT, 1)?;
    if skill[2] > 1.0 {
        return Err(BattleError::InvalidInput(
            "Serval skill base Shock chance".into(),
        ));
    }

    let enabled_trace = |id: &str| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let shock_chance_boost_bp = if enabled_trace("trace.1103101") {
        2_000
    } else {
        0
    };
    let mechanics = Serval {
        eidolon: input.eidolon,
        echo_multiplier_bp: basis_points(BASIC, basic[0] * 0.6)?,
        skill_shock_chance_bp: u32::from(basis_points(SKILL, skill[2])?) + shock_chance_boost_bp,
        skill_shock_turns: turns(SKILL, skill[3])?,
        skill_shock_multiplier_bp: basis_points(SKILL, skill[4])?,
        ultimate_extension_turns: turns(ULTIMATE, ultimate[1])?,
        talent_multiplier_bp: basis_points(TALENT, talent[0])?,
        starting_energy_trace: enabled_trace("trace.1103102"),
        on_defeat_attack_trace: enabled_trace("trace.1103103"),
    };
    Ok(OfficialKit {
        max_energy_milli: input.energy_capacity * 1_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![hit(basis_points(BASIC, basic[0])?, 10)],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 30_000,
            effects: vec![
                hit(basis_points(SKILL, skill[0])?, 20),
                Effect::BlastAdjacent {
                    element: Element::Thunder,
                    multiplier_bp: basis_points(SKILL, skill[1])?,
                    toughness: 10,
                },
            ],
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::AllEnemies,
            energy_milli: 5_000,
            effects: vec![hit(basis_points(ULTIMATE, ultimate[0])?, 20)],
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(mechanics),
    })
}

fn hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: Element::Thunder,
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
            "Serval {skill} multiplier or chance"
        )))
    }
}

fn turns(skill: &str, value: f64) -> Result<u32, BattleError> {
    if (1.0..=f64::from(u32::MAX)).contains(&value) && value.fract() == 0.0 {
        Ok(value as u32)
    } else {
        Err(BattleError::InvalidInput(format!(
            "Serval {skill} duration"
        )))
    }
}

struct Serval {
    eidolon: u8,
    echo_multiplier_bp: u16,
    skill_shock_chance_bp: u32,
    skill_shock_turns: u32,
    skill_shock_multiplier_bp: u16,
    ultimate_extension_turns: u32,
    talent_multiplier_bp: u16,
    starting_energy_trace: bool,
    on_defeat_attack_trace: bool,
}

impl Serval {
    fn echo(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        // Select among at most two valid adjacent targets, in field order.
        let mut targets = battle.adjacent_enemies(target);
        let Some(first) = targets.next() else {
            return;
        };
        let target = match targets.next() {
            Some(second) => [first, second][battle.random_index(2)],
            None => first,
        };
        battle.attack_extra_hit(
            actor,
            target,
            Element::Thunder,
            self.echo_multiplier_bp,
            10,
            Action::Basic,
        );
    }

    fn skill_shocks(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        for enemy in std::iter::once(target).chain(battle.adjacent_enemies(target)) {
            battle.apply_skill_shock(
                actor,
                enemy,
                Action::Skill,
                self.skill_shock_chance_bp,
                self.skill_shock_multiplier_bp,
                self.skill_shock_turns,
            );
        }
    }

    fn ultimate_shocks(&self, battle: &mut Battle<'_>, actor: usize) {
        for enemy in battle.enemies() {
            if battle.hp(enemy).0 == 0 {
                continue;
            }
            if self.eidolon >= 4 && !battle.has_elemental_dot(enemy, Element::Thunder) {
                battle.apply_skill_shock(
                    actor,
                    enemy,
                    Action::Ultimate,
                    10_000,
                    self.skill_shock_multiplier_bp,
                    self.skill_shock_turns,
                );
            }
            battle.extend_shocks(actor, enemy, self.ultimate_extension_turns);
        }
    }
}

impl Mechanics for Serval {
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        if self.starting_energy_trace {
            battle.gain_energy(actor, actor, 15_000);
        }
    }

    fn after_defeat(&self, battle: &mut Battle<'_>, actor: usize, _target: usize) {
        if self.on_defeat_attack_trace {
            battle.apply_attack_bonus(actor, 2_000, 2);
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
            (Action::Basic, Some(target)) if self.eidolon >= 1 => self.echo(battle, actor, target),
            (Action::Skill, Some(target)) => self.skill_shocks(battle, actor, target),
            (Action::Ultimate, _) => self.ultimate_shocks(battle, actor),
            _ => {}
        }
        for enemy in battle.enemies() {
            if battle.hp(enemy).0 > 0 && battle.has_elemental_dot(enemy, Element::Thunder) {
                battle.secondary_hit(
                    actor,
                    enemy,
                    Element::Thunder,
                    self.talent_multiplier_bp,
                    DamageKind::Additional,
                    action,
                );
                if self.eidolon >= 2 {
                    battle.gain_energy(actor, actor, 4_000);
                }
            }
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
        if self.eidolon >= 6 && battle.has_elemental_dot(target, Element::Thunder) {
            3_000
        } else {
            0
        }
    }
}
