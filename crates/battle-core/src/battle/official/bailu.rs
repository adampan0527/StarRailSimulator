use std::{cmp::Reverse, sync::Arc};

use super::{
    super::{Battle, support::AllyBuffKind},
    Mechanics, OfficialAbility, OfficialCharacter, OfficialKit, SkillPoints,
    healing::{Healing, MICROS},
};
use crate::{
    Action, BattleError,
    fixture::{Effect, Element, Target},
};

#[cfg(test)]
mod tests;

const BASIC: &str = "skill.121101";
const SKILL: &str = "skill.121102";
const ULTIMATE: &str = "skill.121103";
const TALENT: &str = "skill.121104";
const BP: u32 = 10_000;
const A2_MAX_HP_BP: u32 = 1_000;
const A6_REDUCTION_BP: u32 = 1_000;
const E1_ENERGY_MILLI: u32 = 8_000;
const E2_HEALING_BP: u32 = 1_500;
const E4_DAMAGE_PER_STACK_BP: u32 = 1_000;
const E4_MAX_DAMAGE_BP: u32 = 3_000;

#[derive(Clone, Default)]
pub(in crate::battle) struct BailuState {
    healing_bonus_until: u32,
    revives_used: u8,
}

#[derive(Clone, Copy)]
pub(in crate::battle) struct Invigoration {
    source: usize,
    expires_after_turn: u32,
    remaining_heals: u8,
    damage_reduction_bp: u32,
    full_hp_energy_milli: u32,
}

struct Bailu {
    skill_heal: Healing,
    bounce_scale_bp: u32,
    bounces: u8,
    ultimate_heal: Healing,
    invigoration_heal: Healing,
    invigoration_turns: u32,
    invigoration_charges: u8,
    invigoration_reduction_bp: u32,
    revive_heal: Healing,
    max_hp_trace: bool,
    outgoing_healing_bp: u32,
    eidolon: u8,
}

pub(super) fn build(input: &OfficialCharacter) -> Result<OfficialKit, BattleError> {
    if input.energy_capacity != 100 {
        return Err(BattleError::InvalidInput("Bailu energy capacity".into()));
    }
    let basic = input.parameters(BASIC, 1)?[0];
    let skill = input.parameters(SKILL, 4)?;
    let ultimate = input.parameters(ULTIMATE, 3)?;
    let talent = input.parameters(TALENT, 5)?;
    let multiplier = (basic * f64::from(BP)).round();
    if !(1.0..=f64::from(u16::MAX)).contains(&multiplier) || skill[2] > 1.0 {
        return Err(BattleError::InvalidInput("Bailu ability parameters".into()));
    }
    let has_trace = |id: &str| input.enabled_trace_ids.iter().any(|trace| trace == id);
    let mechanics = Bailu {
        skill_heal: Healing::from_data("Bailu", SKILL, skill[0], skill[1])?,
        bounce_scale_bp: BP - (skill[2] * f64::from(BP)).round() as u32,
        bounces: positive_count(SKILL, skill[3])?,
        ultimate_heal: Healing::from_data("Bailu", ULTIMATE, ultimate[0], ultimate[1])?,
        invigoration_heal: Healing::from_data("Bailu", TALENT, talent[0], talent[1])?,
        invigoration_turns: u32::from(positive_count(ULTIMATE, ultimate[2])?),
        invigoration_charges: positive_count(TALENT, talent[4])?
            .saturating_add(u8::from(has_trace("trace.1211102"))),
        invigoration_reduction_bp: if has_trace("trace.1211103") {
            A6_REDUCTION_BP
        } else {
            0
        },
        revive_heal: Healing::from_data("Bailu", TALENT, talent[2], talent[3])?,
        max_hp_trace: has_trace("trace.1211101"),
        outgoing_healing_bp: input.stats.healing_bp,
        eidolon: input.eidolon,
    };
    Ok(OfficialKit {
        max_energy_milli: 100_000,
        basic: OfficialAbility {
            target: Target::Enemy,
            energy_milli: 20_000,
            effects: vec![Effect::Hit {
                element: Element::Thunder,
                multiplier_bp: multiplier as u16,
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
        mechanics: Arc::new(mechanics),
    })
}

fn positive_count(skill: &str, value: f64) -> Result<u8, BattleError> {
    if !(1.0..=f64::from(u8::MAX)).contains(&value) || value.fract() != 0.0 {
        return Err(BattleError::InvalidInput(format!("Bailu {skill} count")));
    }
    Ok(value as u8)
}

impl Bailu {
    fn heal(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        target: usize,
        amount: Healing,
        scale: u64,
    ) {
        let actor_state = &battle.units[actor];
        let eidolon_bonus =
            if actor_state.bailu_state.healing_bonus_until > actor_state.finished_turns {
                E2_HEALING_BP
            } else {
                0
            };
        let bonus_micros =
            u64::from(self.outgoing_healing_bp + eidolon_bonus) * MICROS / u64::from(BP);
        let amount = amount.scaled_amount(battle.hp(actor).1, bonus_micros, scale);
        let (hp, max_hp) = battle.hp(target);
        battle.heal_unit(actor, target, amount);
        if self.max_hp_trace && amount > max_hp - hp {
            let base_hp = battle.plan.units[target].stats.base_hp;
            battle.apply_max_hp_increase(actor, target, base_hp * A2_MAX_HP_BP / BP, 2);
        }
    }

    fn skill(&self, battle: &mut Battle<'_>, actor: usize, target: usize) {
        let living: Vec<_> = battle
            .allies()
            .filter(|&unit| battle.is_targetable(unit))
            .collect();
        let mut recipient = target;
        let mut scale_micros = MICROS;
        for bounce in 0..=self.bounces {
            if bounce > 0 {
                recipient = living[battle.random_index(living.len())];
                scale_micros = scale_micros * u64::from(self.bounce_scale_bp) / u64::from(BP);
            }
            self.heal(battle, actor, recipient, self.skill_heal, scale_micros);
            if self.eidolon >= 4 {
                let bonus = battle.ally_buff_value(actor, recipient, AllyBuffKind::Damage)
                    + E4_DAMAGE_PER_STACK_BP;
                battle.apply_ally_buff(
                    actor,
                    recipient,
                    AllyBuffKind::Damage,
                    bonus.min(E4_MAX_DAMAGE_BP),
                    2,
                );
            }
        }
    }

    fn ultimate(&self, battle: &mut Battle<'_>, actor: usize) {
        for ally in battle.allies() {
            if !battle.is_targetable(ally) {
                continue;
            }
            self.heal(battle, actor, ally, self.ultimate_heal, MICROS);
            let unit = &mut battle.units[ally];
            if let Some(active) = &mut unit.invigoration {
                active.expires_after_turn += 1;
            } else {
                unit.invigoration = Some(Invigoration {
                    source: actor,
                    expires_after_turn: unit.normal_turns + self.invigoration_turns,
                    remaining_heals: self.invigoration_charges,
                    damage_reduction_bp: self.invigoration_reduction_bp,
                    full_hp_energy_milli: if self.eidolon >= 1 {
                        E1_ENERGY_MILLI
                    } else {
                        0
                    },
                });
            }
        }
        if self.eidolon >= 2 {
            let caster = &mut battle.units[actor];
            caster.bailu_state.healing_bonus_until = caster.normal_turns + 2;
        }
    }
}

impl Mechanics for Bailu {
    fn after_action(
        &self,
        battle: &mut Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) {
        match (action, target) {
            (Action::Skill, Some(target)) => self.skill(battle, actor, target),
            (Action::Ultimate, _) => self.ultimate(battle, actor),
            _ => {}
        }
    }

    fn after_enemy_attack_seen(
        &self,
        battle: &mut Battle<'_>,
        owner: usize,
        _attacker: usize,
        target: usize,
        _was_shielded: bool,
    ) {
        if let Some(status) = battle.units[target].invigoration.as_mut()
            && status.source == owner
        {
            status.remaining_heals -= 1;
            let exhausted = status.remaining_heals == 0;
            self.heal(battle, owner, target, self.invigoration_heal, MICROS);
            if exhausted {
                battle.end_invigoration(target);
            }
        }
    }

    fn prevent_ally_defeat(&self, battle: &mut Battle<'_>, owner: usize, target: usize) -> bool {
        let limit = 1 + u8::from(self.eidolon >= 6);
        if battle.units[owner].bailu_state.revives_used >= limit || battle.is_frozen(owner) {
            return false;
        }
        self.heal(battle, owner, target, self.revive_heal, MICROS);
        if battle.hp(target).0 == 0 {
            return false;
        }
        battle.units[owner].bailu_state.revives_used += 1;
        true
    }

    fn dpr_priority(
        &self,
        battle: &Battle<'_>,
        actor: usize,
        action: Action,
        target: Option<usize>,
    ) -> Option<u8> {
        match action {
            Action::Ultimate => Some(0),
            Action::Skill if self.eidolon >= 4 => {
                let preferred = battle
                    .allies()
                    .filter(|&unit| unit != actor && battle.is_targetable(unit))
                    .max_by_key(|&unit| (battle.plan.units[unit].stats.attack, Reverse(unit)));
                let can_buff = preferred.is_some_and(|unit| {
                    target == Some(unit)
                        && battle.ally_buff_value(actor, unit, AllyBuffKind::Damage)
                            < E4_MAX_DAMAGE_BP
                });
                can_buff.then_some(2)
            }
            _ => None,
        }
    }
}

impl Battle<'_> {
    pub(in crate::battle) fn invigoration_damage_reduction_bp(&self, target: usize) -> u32 {
        self.units[target]
            .invigoration
            .map_or(0, |status| status.damage_reduction_bp)
    }

    pub(in crate::battle) fn expire_invigoration(&mut self, target: usize) {
        let unit = &self.units[target];
        if unit
            .invigoration
            .is_some_and(|status| status.expires_after_turn <= unit.normal_turns)
        {
            self.end_invigoration(target);
        }
    }

    fn end_invigoration(&mut self, target: usize) {
        if let Some(status) = self.units[target].invigoration.take()
            && status.full_hp_energy_milli > 0
        {
            let (hp, max_hp) = self.hp(target);
            if hp == max_hp {
                self.restore_energy_flat(status.source, target, status.full_hp_energy_milli);
            }
        }
    }
}
