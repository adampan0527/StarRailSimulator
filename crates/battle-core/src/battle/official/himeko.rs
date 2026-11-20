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

const FORM: &str = "character.1003";
const BASIC: &str = "skill.100301";
const SKILL: &str = "skill.100302";
const ULTIMATE: &str = "skill.100303";
const TALENT: &str = "skill.100304";
const STARFIRE: &str = "trace.1003101";
const MAGMA: &str = "trace.1003102";
const BENCHMARK: &str = "trace.1003103";
const MAX_CHARGE: u8 = 3;

struct Himeko {
    eidolon: u8,
    starfire: bool,
    magma: bool,
    benchmark: bool,
    talent_multiplier_bp: u16,
    ultimate_kill_energy_milli: u32,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 120 || input.eidolon > 6 {
        return Err(invalid("energy capacity or eidolon"));
    }
    let basic = input.parameters(BASIC, 1)?;
    let skill = input.parameters(SKILL, 2)?;
    let ultimate = input.parameters(ULTIMATE, 2)?;
    let talent = input.parameters(TALENT, 2)?;
    if talent[1] != f64::from(MAX_CHARGE) {
        return Err(invalid("talent charge limit"));
    }
    if !(0.0..=f64::from(input.energy_capacity)).contains(&ultimate[1]) {
        return Err(invalid("ultimate kill energy"));
    }

    let ultimate_bp = to_basis_points(ULTIMATE, ultimate[0])?;
    let mut ultimate_effects = vec![fire_hit(ultimate_bp, 20)];
    if input.eidolon >= 6 {
        ultimate_effects.push(Effect::RandomHits {
            element: Element::Fire,
            multiplier_bp: ((u32::from(ultimate_bp) * 4 + 5) / 10) as u16,
            toughness: 5,
            count: 2,
        });
    }

    let trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    Ok(OfficialKit {
        max_energy_milli: 120_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![fire_hit(to_basis_points(BASIC, basic[0])?, 10)],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 30_000,
            effects: vec![
                fire_hit(to_basis_points(SKILL, skill[0])?, 20),
                Effect::BlastAdjacent {
                    element: Element::Fire,
                    multiplier_bp: to_basis_points(SKILL, skill[1])?,
                    toughness: 10,
                },
            ],
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::AllEnemies,
            energy_milli: 5_000,
            effects: ultimate_effects,
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Himeko {
            eidolon: input.eidolon,
            starfire: trace(STARFIRE),
            magma: trace(MAGMA),
            benchmark: trace(BENCHMARK),
            talent_multiplier_bp: to_basis_points(TALENT, talent[0])?,
            ultimate_kill_energy_milli: (ultimate[1] * 1_000.0).round() as u32,
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn to_basis_points(skill: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(&format!("{skill} damage multiplier")));
    }
    Ok(bp as u16)
}

fn fire_hit(multiplier_bp: u16, toughness: u16) -> Effect {
    Effect::Hit {
        element: Element::Fire,
        multiplier_bp,
        toughness,
    }
}

impl Himeko {
    fn burn(battle: &mut Battle<'_>, actor: usize, target: usize, origin: Action) {
        battle.apply_skill_dot(
            actor,
            target,
            5_000,
            SkillDot {
                element: Element::Fire,
                damage: DotScale::Attack(3_000),
                origin,
                turns: 2,
                max_stacks: 1,
            },
        );
    }
}

impl Mechanics for Himeko {
    fn on_battle_start(&self, battle: &mut Battle<'_>, actor: usize) {
        battle.units[actor].himeko_charge = 1;
    }

    fn on_weakness_break(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        target: usize,
        origin: Action,
    ) {
        let charge = if battle.is_elite(target) {
            MAX_CHARGE
        } else {
            1
        };
        let extra = u8::from(
            self.eidolon >= 4
                && owner == source
                && origin == Action::Skill
                && !battle.in_follow_up(),
        );
        let total = &mut battle.units[owner].himeko_charge;
        *total = (*total + charge + extra).min(MAX_CHARGE);
    }

    fn on_attack_finished(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        source: usize,
        origin: Action,
    ) {
        if battle.units[owner].himeko_charge == MAX_CHARGE
            && (owner != source || !battle.in_follow_up())
            && !battle.is_frozen(owner)
            && battle.enemies().any(|unit| battle.hp(unit).0 > 0)
        {
            battle.units[owner].himeko_charge = 0;
            battle.follow_up(
                owner,
                Element::Fire,
                self.talent_multiplier_bp,
                10,
                origin,
                10_000,
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
        if !self.starfire {
            return;
        }
        match (action, target) {
            (Action::Basic, Some(target)) => Self::burn(battle, actor, target, action),
            (Action::Skill, Some(target)) => {
                for unit in std::iter::once(target).chain(battle.adjacent_enemies(target)) {
                    Self::burn(battle, actor, unit, action);
                }
            }
            (Action::Ultimate, None) => {
                for unit in battle.enemies() {
                    Self::burn(battle, actor, unit, action);
                }
            }
            _ => {}
        }
    }

    fn after_follow_up(&self, battle: &mut Battle<'_>, actor: usize, origin: Action) {
        if self.starfire {
            for target in battle.enemies() {
                Self::burn(battle, actor, target, origin);
            }
        }
        if self.eidolon >= 1 {
            battle.apply_speed_up(actor, actor, 2_000, 2);
        }
    }

    fn after_defeat(&self, battle: &mut Battle<'_>, actor: usize, _: usize) {
        if battle.current_action() == Some(Action::Ultimate) && !battle.in_follow_up() {
            battle.gain_energy(actor, actor, self.ultimate_kill_energy_milli);
        }
    }

    fn crit_rate_bonus_bp(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        _: usize,
        _: Option<Action>,
        _: DamageKind,
    ) -> u32 {
        let (hp, max_hp) = battle.hp(actor);
        if self.benchmark && u64::from(hp) * 5 >= u64::from(max_hp) * 4 {
            1_500
        } else {
            0
        }
    }

    fn damage_bonus_bp(
        &self,
        battle: &Battle<'_>,
        _: usize,
        target: usize,
        action: Option<Action>,
        kind: DamageKind,
    ) -> u32 {
        let (hp, max_hp) = battle.hp(target);
        let e2 = self.eidolon >= 2 && u64::from(hp) * 2 <= u64::from(max_hp);
        let magma = self.magma
            && kind == DamageKind::Direct
            && action == Some(Action::Skill)
            && battle.has_elemental_dot(target, Element::Fire);
        1_500 * u32::from(e2) + 2_000 * u32::from(magma)
    }
}
