use std::sync::Arc;

use crate::{
    Action, BattleError,
    fixture::{Effect, Element, Target},
};

use super::{
    super::{
        Battle,
        periodic::{DotScale, SkillDot},
        support::AllyBuffKind,
    },
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
};

#[cfg(test)]
mod tests;

const FORM: &str = "character.1009";
const BASIC: &str = "skill.100901";
const SKILL: &str = "skill.100902";
const ULTIMATE: &str = "skill.100903";
const TALENT: &str = "skill.100904";
const SPARKS: &str = "trace.1009101";
const IGNITE: &str = "trace.1009102";
const CONSTELLATION: &str = "trace.1009103";
const MAX_CHARGE_STACKS: u8 = 5;
const CHARGES_LOST_PER_TURN: u8 = 3;
const SPEED_BUFF_TURNS: u32 = 2;

struct Asta {
    eidolon: u8,
    sparks: bool,
    ignite: bool,
    constellation: bool,
    burn_multiplier_bp: u16,
    attack_per_charge_bp: u16,
    ultimate_speed_points: u32,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 120 || input.eidolon > 6 {
        return Err(invalid("energy capacity or eidolon"));
    }
    let basic = to_basis_points(BASIC, input.parameters(BASIC, 1)?[0])?;
    let skill = to_basis_points(SKILL, input.parameters(SKILL, 1)?[0])?;
    let ultimate = input.parameters(ULTIMATE, 2)?;
    let talent = input.parameters(TALENT, 3)?;
    if !(1.0..=100.0).contains(&ultimate[0])
        || ultimate[0].fract() != 0.0
        || ultimate[1] != f64::from(SPEED_BUFF_TURNS)
        || talent[1] != f64::from(MAX_CHARGE_STACKS)
        || talent[2] != f64::from(CHARGES_LOST_PER_TURN)
    {
        return Err(invalid(
            "speed points, buff duration, or talent stack limits",
        ));
    }
    let has_trace = |id| input.enabled_trace_ids.iter().any(|trace| trace == id);
    Ok(OfficialKit {
        max_energy_milli: 120_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![Effect::Hit {
                element: Element::Fire,
                multiplier_bp: basic,
                toughness: 10,
            }],
            skill_points: SkillPoints::Generate,
        },
        skill: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 0,
            effects: vec![
                Effect::Hit {
                    element: Element::Fire,
                    multiplier_bp: skill,
                    toughness: 10,
                },
                Effect::RandomHits {
                    element: Element::Fire,
                    multiplier_bp: skill,
                    toughness: 5,
                    count: 4 + u8::from(input.eidolon >= 1),
                },
            ],
            skill_points: SkillPoints::Spend,
        },
        ultimate: OfficialAbility {
            target: Target::AllAllies,
            energy_milli: 5_000,
            effects: vec![],
            skill_points: SkillPoints::Unchanged,
        },
        mechanics: Arc::new(Asta {
            eidolon: input.eidolon,
            sparks: has_trace(SPARKS),
            ignite: has_trace(IGNITE),
            constellation: has_trace(CONSTELLATION),
            burn_multiplier_bp: basic.div_ceil(2),
            attack_per_charge_bp: to_basis_points(TALENT, talent[0])?,
            ultimate_speed_points: ultimate[0] as u32,
        }),
    })
}

fn invalid(detail: &str) -> BattleError {
    BattleError::InvalidInput(format!("{FORM}: {detail}"))
}

fn to_basis_points(skill: &str, ratio: f64) -> Result<u16, BattleError> {
    let bp = (ratio * 10_000.0).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&bp) {
        return Err(invalid(&format!("{skill} multiplier")));
    }
    Ok(bp as u16)
}

impl Mechanics for Asta {
    fn on_turn_start(&self, battle: &mut Battle<'_>, owner: usize, actor: usize) {
        if owner != actor {
            return;
        }
        let unit = &mut battle.units[owner];
        let state = &mut unit.asta_state;
        let preserve = std::mem::take(&mut state.preserve_charge_on_next_turn);
        if unit.normal_turns > 1 && !preserve {
            let loss = CHARGES_LOST_PER_TURN - u8::from(self.eidolon >= 6);
            state.charge_stacks = state.charge_stacks.saturating_sub(loss);
        }
    }

    fn before_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        _action: Action,
        _target: Option<usize>,
    ) {
        battle.units[actor].asta_state.hit_targets.clear();
    }

    fn after_attack_hit(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        action: Action,
        _hp_before: u32,
    ) {
        let charge = 1 + u8::from(battle.has_weakness(target, Element::Fire));
        let state = &mut battle.units[actor].asta_state;
        if !state.hit_targets.contains(&target) {
            state.hit_targets.push(target);
            state.charge_stacks = (state.charge_stacks + charge).min(MAX_CHARGE_STACKS);
        }

        match action {
            Action::Basic if self.sparks => {
                battle.apply_skill_dot(
                    actor,
                    target,
                    8_000,
                    SkillDot {
                        element: Element::Fire,
                        damage: DotScale::Attack(self.burn_multiplier_bp),
                        origin: action,
                        turns: 3,
                        max_stacks: 1,
                    },
                );
            }
            Action::Skill => battle.gain_energy(actor, actor, 6_000),
            _ => {}
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
        if self.eidolon >= 2 {
            battle.units[actor].asta_state.preserve_charge_on_next_turn = true;
        }
        for ally in battle.allies() {
            if battle.hp(ally).0 > 0 {
                battle.apply_ally_buff(
                    actor,
                    ally,
                    AllyBuffKind::SpeedFlat,
                    self.ultimate_speed_points,
                    SPEED_BUFF_TURNS,
                );
            }
        }
    }

    fn ally_attack_bonus_bp(&self, battle: &Battle<'_>, owner: usize, _target: usize) -> u32 {
        u32::from(battle.units[owner].asta_state.charge_stacks)
            * u32::from(self.attack_per_charge_bp)
    }

    fn ally_damage_bonus_bp(
        &self,
        _battle: &Battle<'_>,
        _owner: usize,
        _target: usize,
        element: Element,
    ) -> u32 {
        if self.ignite && element == Element::Fire {
            1_800
        } else {
            0
        }
    }

    fn defense_bonus_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        if self.constellation {
            u32::from(battle.units[actor].asta_state.charge_stacks) * 600
        } else {
            0
        }
    }

    fn energy_regeneration_bonus_bp(&self, battle: &Battle<'_>, actor: usize) -> u32 {
        if self.eidolon >= 4 && battle.units[actor].asta_state.charge_stacks >= 2 {
            1_500
        } else {
            0
        }
    }

    fn dpr_priority(
        &self,
        _battle: &Battle<'_>,
        _actor: usize,
        action: Action,
        _target: Option<usize>,
    ) -> Option<u8> {
        (action == Action::Ultimate).then_some(0)
    }
}
